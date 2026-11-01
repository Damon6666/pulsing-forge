//! forge-cli — `forge run <scenario.toml>`: 场景即数据, 一条命令一个多agent回路。
//! Phase 0 内置确定性店员(脚本层, 无LLM): 潮汐层(codex)经 adapter 接入(见docs)。

mod codex_adapter;
mod dashboard;

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
    // forge init
    if args.len() >= 2 && args[1] == "init" {
        let name = args.get(2).map(|s| s.as_str()).unwrap_or("my-scenario");
        let tmpl = format!(
            r#"# 场景: {name}
# forge run {name}.toml
[meta]
name = "{name}"

[task]
template = "写一个函数完成你的任务"
hidden_check = "描述领域标准要求但任务信未提及的检查"
hidden_keyword = "某个关键词"
n_reviewers = 4

[[reviewers]]
name = "review_a"
duty = "检查项A"
keyword = "keyword_a"

[[reviewers]]
name = "review_b"
duty = "检查项B"
keyword = "keyword_b"

[[reviewers]]
name = "review_c"
duty = "检查项C"
keyword = "keyword_c"

[[reviewers]]
name = "review_d"
duty = "检查项D"
keyword = "keyword_d"
"#
        );
        let fname = format!("{name}.toml");
        std::fs::write(&fname, tmpl).expect("写模板");
        println!("✅ 已创建 {fname} — 编辑后 forge run {fname}");
        return;
    }
    if args.len() >= 2 && args[1] == "serve" {
        // forge serve — 启动潮汐层服务器(控制口+店调度器)
        let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(12400);
        let log_dir = format!("/tmp/forge-tide-{}", std::process::id());
        let mut hub = forge_runtime::TideHub::new(&log_dir, port).expect("创建TideHub");
        // 默认加 judge + writer 店(可用环境变量扩展)
        hub.add_shop("judge", "glm-5-turbo");
        hub.add_shop("writer", "glm-5-turbo");
        hub.codex_bin = std::env::var("FORGE_CODEX_BIN")
            .unwrap_or_else(|_| "/home/daimeng/bin/codex".to_string());
        hub.provider_path = std::env::var("FORGE_PROVIDER")
            .unwrap_or_else(|_| "/tmp/forge-tide/config/provider.toml".to_string());
        let hub = std::sync::Arc::new(hub);
        println!("潮汐层服务器: 控制口 :{port} 日志 {log_dir}");
        println!("环境变量: FORGE_CODEX_BIN={} FORGE_PROVIDER={}",
            hub.codex_bin, hub.provider_path);
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async {
            if let Err(e) = forge_runtime::run_tide(hub).await {
                eprintln!("潮汐层错误: {e}");
            }
        });
        return;
    }
    if args.len() >= 2 && args[1] == "test-tide" {
        // forge test-tide — 验证 TCP 控制口 + 4 工具管道(不需要 LLM)
        test_tide();
        return;
    }
    if args.len() < 2 || args[1] != "run" || args.len() < 3 {
        eprintln!(
            "用法:
  forge init [name]        创建场景模板
  forge run <file.toml>    运行场景(确定性层)
  forge serve [port]       启动潮汐层服务器
  forge test-tide          验证 TCP+MCP 管道"
        );
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
    // 内置大屏: Mermaid 泳道时序图
    let mut journal: Vec<serde_json::Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&run_dir) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("events-") || !name.ends_with(".jsonl") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                for line in content.lines() {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                        if v["ev"] == "reply_out" {
                            journal.push(v);
                        }
                    }
                }
            }
        }
    }
    dashboard::render_sequence_html(&journal, &sc.meta.name, &format!("{run_dir}/sequence.html"));
    println!("大屏: {run_dir}/sequence.html");
}

/// 验证 TCP 控制口 + 4 工具管道: 不需要 LLM/codex, 只测 plumbing
fn test_tide() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpStream;

    let port = 12499u16;
    let log_dir = "/tmp/forge-test-tide";
    let _ = std::fs::remove_dir_all(log_dir);

    // 1. 创建 TideHub 并在后台线程启动 TCP 服务器
    let mut hub = forge_runtime::TideHub::new(log_dir, port).expect("TideHub");
    hub.add_shop("alpha", "test");
    hub.add_shop("beta", "test");
    let acl = forge_runtime::Acl::from_pairs(&[
        ("alpha", &["beta", "alpha"]),
        ("beta", &["alpha"]),
    ]);
    hub.acl = acl;
    // 不设置 codex_bin → run_turn 会失败, 但 TCP 管道可验证
    let hub = std::sync::Arc::new(hub);

    let hub2 = hub.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("rt");
        rt.block_on(async {
            let _ = forge_runtime::serve_control(hub2, port).await;
        });
    });
    std::thread::sleep(std::time::Duration::from_millis(500));

    let mut pass = 0;
    let mut fail = 0;

    let send_cmd = |json: &str| -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("连接");
        s.write_all(format!("{json}\n").as_bytes()).expect("写");
        let mut line = String::new();
        BufReader::new(s).read_line(&mut line).expect("读");
        line.trim().to_string()
    };

    // 2. 测试 pulsing_send (alpha → beta, 静默)
    let r = send_cmd(r#"{"tool":"pulsing_send","args":{"_shop":"alpha","to":"beta","message":"hello","silent":true}}"#);
    if r.contains("已送达") { pass += 1; println!("✅ pulsing_send: {r}"); }
    else { fail += 1; println!("❌ pulsing_send: {r}"); }

    // 3. 测试 pulsing_drain_mail (beta 读信)
    let r = send_cmd(r#"{"tool":"pulsing_drain_mail","args":{"_shop":"beta"}}"#);
    if r.contains("hello") { pass += 1; println!("✅ pulsing_drain_mail: 信件已读到"); }
    else { fail += 1; println!("❌ pulsing_drain_mail: {r}"); }

    // 4. 测试 ACL (alpha 只能发 beta/alpha, 不能发 unknown)
    let r = send_cmd(r#"{"tool":"pulsing_send","args":{"_shop":"alpha","to":"unknown","message":"x","silent":true,"_from_codex":true}}"#);
    if r.contains("不在") { pass += 1; println!("✅ ACL 越权拒绝: {r}"); }
    else { fail += 1; println!("❌ ACL: {r}"); }

    // 5. 测试 pulsing_broadcast (alpha → beta, 白名单收拢)
    let r = send_cmd(r#"{"tool":"pulsing_broadcast","args":{"_shop":"alpha","to_list":["beta","unknown"],"message":"bc","silent":true,"_from_codex":true}}"#);
    if r.contains("已群发 1") { pass += 1; println!("✅ pulsing_broadcast 收拢: {r}"); }
    else { fail += 1; println!("❌ broadcast: {r}"); }

    // 6. 测试 pulsing_health
    let r = send_cmd(r#"{"tool":"pulsing_health","args":{}}"#);
    if r.contains("店=2") { pass += 1; println!("✅ pulsing_health: {r}"); }
    else { fail += 1; println!("❌ health: {r}"); }

    // 7. 验证账本
    let journal = hub.journal.replay();
    if journal.len() >= 2 { pass += 1; println!("✅ 账本: {} 封信", journal.len()); }
    else { fail += 1; println!("❌ 账本: {} 封", journal.len()); }

    println!("--- 管道验证: {pass} PASS / {fail} FAIL ---");
    if fail > 0 { std::process::exit(1); }
}
