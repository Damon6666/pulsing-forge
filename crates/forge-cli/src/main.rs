//! forge-cli — `forge run <scenario.toml>`: 场景即数据, 一条命令一个多agent回路。
//! Phase 0 内置确定性店员(脚本层, 无LLM): 潮汐层(codex)经 adapter 接入(见docs)。

use forge_core::{report_is_final, CollectGate, VerdictTable};
use forge_runtime::{Acl, Hub, OutLetter};
use std::collections::HashMap;

#[derive(Debug, serde::Deserialize)]
struct Scenario {
    meta: Meta,
    task: Task,
    #[serde(default)]
    reviewers: Vec<Reviewer>,
}
#[derive(Debug, serde::Deserialize)]
struct Meta {
    name: String,
}
#[derive(Debug, serde::Deserialize)]
struct Task {
    template: String,
    /// 埋雷: 任务不含但领域标准要求的检查(审查店应抓出)
    hidden_check: String,
    hidden_keyword: String,
    #[serde(default = "default_n")]
    n_reviewers: usize,
}
#[derive(Debug, serde::Deserialize)]
struct Reviewer {
    name: String,
    duty: String,
    keyword: String,
}
fn default_n() -> usize {
    4
}

fn out(to: &str, body: &str, silent: bool) -> OutLetter {
    OutLetter {
        to: to.into(),
        body: body.into(),
        silent,
    }
}

/// 确定性写审回路(脚本店员版): 完整走 写→审→裁决→打回→修正→复审→双通道收尾。
/// 每一步都是 taxonomy 里对应原语的调用现场。
pub fn run(sc: &Scenario, run_dir: &str) -> String {
    let mut shops = vec!["judge".to_string(), "writer".to_string()];
    for r in &sc.reviewers {
        shops.push(r.name.clone());
    }
    // ACL: 写手只寄仲裁者; 审查店只寄仲裁者; 仲裁者全通
    let mut acl_pairs: Vec<(String, Vec<String>)> = vec![
        ("writer".into(), vec!["judge".into()]),
        ("judge".into(), vec!["*".into()]),
    ];
    for r in &sc.reviewers {
        acl_pairs.push((format!("{}*", r.name), vec!["judge".into()]));
    }
    // ACL 规则名用前缀匹配审查店
    let acl = Acl { rules: acl_pairs };

    let mut hub = Hub::new(run_dir, shops, acl.clone()).unwrap();
    hub.auto_silence = Some(("review".into(), "judge".into()));

    // ── 店员(确定性) ──
    let task_text = format!("TASK={}", sc.task.template);
    let hidden = sc.task.hidden_keyword.clone();
    let trap_desc = sc.task.hidden_check.clone();
    let n = sc.task.n_reviewers;

    // 写手: 初稿故意不含隐藏检查(照任务信字面); 打回后按意见照单全收
    let all_keywords: Vec<String> = sc.reviewers.iter().map(|r| r.keyword.clone()).collect();
    hub.register(
        "writer",
        Box::new(move |trig, _| {
            let b = &trig.body;
            if b.starts_with("TASK=") {
                let draft = "DRAFT=```python\ndef validate(x):\n    # 按任务信实现\n    pass\n```"
                    .to_string();
                vec![out("judge", &draft, false)]
            } else if b.starts_with("REJECT=") {
                // 确定性修正: REJECT裁决表中点名的每个检查各补一行
                let mut checks = String::new();
                for k in &all_keywords {
                    if b.contains(k) {
                        checks.push_str(&format!("\n    assert '{k}'  # 已补"));
                    }
                }
                if checks.is_empty() {
                    checks = format!(
                        "\n    assert '{k}'",
                        k = all_keywords.first().cloned().unwrap_or_default()
                    );
                }
                let draft =
                    format!("REVISED=```python\ndef validate(x):{checks}\n    return True\n```");
                vec![out("judge", &draft, false)]
            } else {
                vec![]
            }
        }),
    );

    // 审查店: 各查一件事 — duty_keyword 在稿中即 CLEAN, 否则 NEEDS_FIX
    for r in &sc.reviewers {
        let kw = r.keyword.clone();
        let duty = r.duty.clone();
        let name = r.name.clone();
        hub.register(
            &r.name,
            Box::new(move |trig, _| {
                if !trig.body.contains("DRAFT") && !trig.body.contains("REVISED") {
                    return vec![];
                }
                let verdict = if trig.body.contains(&kw) {
                    format!("CLEAN={name}||{duty}: 已覆盖")
                } else {
                    format!("NEEDS_FIX={name}||{duty}: 缺失({kw})")
                };
                vec![out("judge", &verdict, false)] // hub 的 auto_silence 会转静默
            }),
        );
    }

    // ── 机器侧: 闸门驱动(这正是 forge-core 的调用现场) ──
    // judge 店员是"弱仲裁者"(只转发): 真正的协议由下面的闸门循环执行 —
    // 双通道收尾的"机器通道"即本函数, 不依赖任何 LLM 回合。
    let reviewers_prefix = "review".to_string();
    let gate = CollectGate {
        reviewers_prefix: reviewers_prefix.clone(),
        arbitrator: "judge".into(),
        min_replies: n,
    };

    hub.ignite("writer", &task_text).unwrap();

    // 主循环: 波次驱动(简化潮汐 — 每波 = 群发 → 收信 → 裁决)
    let mut waves = 0;
    let mut final_report: Option<String> = None;
    while waves < 3 && final_report.is_none() {
        waves += 1;
        // 1) 取最新稿(登记处: 账本即registry, 不依赖信箱)
        let letters = hub.letters();
        let draft_ts = letters
            .iter()
            .filter(|l| {
                l.from == "writer"
                    && (l.body.starts_with("DRAFT=") || l.body.starts_with("REVISED="))
            })
            .map(|l| l.ts_ms)
            .max()
            .unwrap_or(0);
        if draft_ts == 0 {
            break;
        }
        let draft_body = letters
            .iter()
            .rev()
            .find(|l| l.from == "writer" && l.ts_ms == draft_ts)
            .unwrap()
            .body
            .clone();
        // 2) 群审(机器代发: 底稿逐字嵌入 — taxonomy #11)
        let names: Vec<String> = sc.reviewers.iter().map(|r| r.name.clone()).collect();
        let fan_body = format!("REVIEW=群审||底稿如下||{draft_body}");
        hub.broadcast("judge", &names, &fan_body, false);
        // 等审查店回合完成(确定性: 广播即同步完成)
        // 3) 收齐闸门 + 裁决表(taxonomy #12)
        let letters = hub.letters();
        // 波锚=最新稿之后的第一封群发信: 同步店员下回信会夹在本波群发之间,
        // 用max会把自己的回信排除在波外(实测answered=1/4)
        let wave_ts = letters
            .iter()
            .filter(|l| l.from == "judge" && l.body.starts_with("REVIEW=") && l.ts_ms > draft_ts)
            .map(|l| l.ts_ms)
            .min()
            .unwrap_or(0);
        let (answered, done) = gate.collect(&letters, wave_ts);
        eprintln!(
            "[debug] wave_ts={wave_ts} answered={} done={done} n={n}",
            answered.len()
        );
        if !done {
            final_report = Some("REPORT=BUDGET_OUT||回信未收齐".into());
            break;
        }
        let vt = VerdictTable { gate: &gate };
        let (table, nfix) = vt.render(&letters, wave_ts);
        if nfix == 0 {
            // 4a) 双通道收尾: 机器默认合账(taxonomy #9)
            let _ = hub.send(
                "judge",
                "judge",
                &format!("REPORT=全部审查通过||裁决表:\n{table}"),
                true,
            );
            final_report = Some(format!("REPORT=全部审查通过(波次{waves})"));
        } else {
            // 4b) 机器代投打回: 从裁决表生成REJECT(taxonomy #10), 交给writer修正
            let rej = format!("REJECT=意见汇总||{table}||写手动作: 补齐后重交");
            let _ = hub.send("judge", "writer", &rej, false);
        }
    }
    if final_report.is_none() {
        final_report = Some("REPORT=FAIL||超过3波未收敛".into());
    }

    // ── 判分: 完整回路校验 + 真值(taxonomy #8 + checker) ──
    let letters = hub.letters();
    let complete = report_is_final(&letters, "judge", &reviewers_prefix, n, n);
    let final_script_ok = letters
        .iter()
        .rev()
        .find(|l| {
            l.from == "writer" && (l.body.starts_with("DRAFT=") || l.body.starts_with("REVISED="))
        })
        .map(|l| l.body.contains(&hidden))
        .unwrap_or(false);
    let closed_ok = letters
        .iter()
        .any(|l| l.from == "judge" && l.body.starts_with("REPORT=全部审查通过"));
    let state = if complete && final_script_ok && closed_ok {
        "PASS"
    } else if complete && closed_ok {
        "DEGENERATE"
    } else {
        "FAIL"
    };
    let turns: u64 = hub.turns.values().sum();
    let report = format!(
        "scenario: {}\nstate: {}\nwaves: {}\nturns: {}\nletters: {}\nfinal_report: {}\ncomplete_loop: {}\nhidden_check_present: {}\n",
        sc.meta.name, state, waves, turns, letters.len(), final_report.unwrap(), complete, final_script_ok);
    report
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] != "run" || args.len() < 3 {
        eprintln!("用法: forge run <scenario.toml>");
        std::process::exit(2);
    }
    let content = std::fs::read_to_string(&args[2]).expect("读场景文件");
    let sc: Scenario = toml::from_str(&content).expect("场景解析");
    let run_dir = format!(
        "runs/{}-{}",
        sc.meta.name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    println!("{}", run(&sc, &run_dir));
    println!("产物: {run_dir}/events-*.jsonl");
}
