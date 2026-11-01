// run_tide_orchestrated — 编排器 + 真 LLM(codex) 的完整产品路径
// 这是 Forge 的最终形态: 确定性层验证协议 → 潮汐层执行语义, 两层用同一套编排器

use forge_core::{Orchestrator, OrchestratorCtx, FinalState, OrchestratorAction};
use forge_runtime::{TideHub, Acl};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 构建编排器(根据场景 toml 的 orchestrator 字段)
fn build_orchestrator(sc: &crate::Scenario) -> (Vec<String>, Box<dyn Orchestrator>) {
    match sc.orchestrator.as_str() {
        "relay" => {
            let stages: Vec<(String, String)> = sc.stages.iter()
                .map(|s| (s.shop.clone(), s.duty.clone())).collect();
            let shops: Vec<String> = stages.iter().map(|(s, _)| s.clone()).collect();
            let input = if sc.meta.relay_input.is_empty() {
                "pulsing makes agents cooperate".to_string()
            } else {
                sc.meta.relay_input.clone()
            };
            (shops, Box::new(forge_core::RelayChainOrchestrator::new(stages, &input)))
        }
        "debate" => {
            let shops = vec!["judge".into(), "pro_a".into(), "pro_b".into()];
            (shops, Box::new(forge_core::DebateOrchestrator {
                judge: "judge".into(),
                topic: if sc.debate_topic.is_empty() { "test".into() } else { sc.debate_topic.clone() },
            }))
        }
        "pipeline" => {
            let stages: Vec<(String, String)> = sc.stages.iter()
                .map(|s| (s.shop.clone(), s.duty.clone())).collect();
            let shops: Vec<String> = stages.iter().map(|(s, _)| s.clone()).collect();
            (shops, Box::new(forge_core::PipelineOrchestrator { stages }))
        }
        "saboteur" => {
            let shops = vec!["judge".into(), "coder".into(), "breaker".into(),
                             "verifier_a".into(), "verifier_b".into()];
            (shops, Box::new(forge_core::SaboteurOrchestrator { judge: "judge".into() }))
        }
        "review" => {
            let reviewers: Vec<(String, String, String)> = sc.reviewers.iter()
                .map(|r| (r.name.clone(), r.duty.clone(), r.keyword.clone())).collect();
            let shops = vec!["judge".into(), "writer".into()]
                .into_iter()
                .chain(reviewers.iter().map(|(n, _, _)| n.clone()))
                .collect();
            (shops, Box::new(forge_core::ReviewOrchestrator {
                judge: "judge".into(), writer: "writer".into(),
                reviewers, reviewer_prefix: "review".into(),
            }))
        }
        other => {
            eprintln!("未知编排器: {other}");
            std::process::exit(2);
        }
    }
}

/// 构建 ACL(按编排器类型)
fn build_acl(orch_type: &str, shops: &[String]) -> Acl {
    let rules: Vec<(String, Vec<String>)> = match orch_type {
        "relay" => {
            let mut r = Vec::new();
            for i in 0..shops.len() {
                let mut allow = vec!["orchestrator".to_string(), shops[i].clone()];
                if i + 1 < shops.len() { allow.push(shops[i+1].clone()); }
                r.push((shops[i].clone(), allow));
            }
            r
        }
        "debate" => vec![
            ("judge".into(), vec!["*".into()]),
            ("pro_a".into(), vec!["judge".into()]),
            ("pro_b".into(), vec!["judge".into()]),
        ],
        "pipeline" => {
            let mut r = Vec::new();
            for i in 0..shops.len() {
                let mut allow = vec![shops[i].clone()];
                if i + 1 < shops.len() { allow.push(shops[i+1].clone()); }
                r.push((shops[i].clone(), allow));
            }
            r
        }
        "saboteur" => vec![
            ("judge".into(), vec!["*".into()]),
            ("coder".into(), vec!["judge".into()]),
            ("breaker".into(), vec!["judge".into()]),
            ("verifier_a".into(), vec!["judge".into()]),
            ("verifier_b".into(), vec!["judge".into()]),
        ],
        "review" => {
            let mut r = vec![
                ("judge".into(), vec!["*".into()]),
                ("writer".into(), vec!["judge".into()]),
            ];
            // 审查店只回 judge
            r.push(("review_*".into(), vec!["judge".into()]));
            r
        }
        _ => vec![],
    };
    Acl { rules }
}

/// 编排器 + 真 LLM 的场景运行
pub fn run_tide_orchestrated(sc: &crate::Scenario, run_dir: &str) -> String {
    let port = 12410u16;
    let _ = std::fs::remove_dir_all(run_dir);
    std::fs::create_dir_all(format!("{run_dir}/config")).expect("创建目录");

    // provider.toml
    let api_key = std::env::var("FORGE_API_KEY")
        .unwrap_or_else(|_| "missing-key".to_string());
    let base_url = std::env::var("FORGE_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4011/v1".to_string());
    std::fs::write(format!("{run_dir}/config/provider.toml"),
        format!("base_url = \"{base_url}\"\napi_key = \"{api_key}\"\nwire_api = \"responses\"\n")).unwrap();

    // 1. 构建编排器 + 店册
    let (shops, orch) = build_orchestrator(sc);
    println!("编排器: {} ({} 店)", sc.orchestrator, shops.len());

    // 2. 创建 TideHub(真 codex 回合管理)
    let mut hub = TideHub::new(run_dir, port).expect("TideHub");
    for shop in &shops { hub.add_shop(shop, "glm-5-turbo"); }
    hub.codex_bin = std::env::var("FORGE_CODEX_BIN")
        .unwrap_or_else(|_| "/home/daimeng/bin/codex".to_string());
    hub.provider_path = format!("{run_dir}/config/provider.toml");
    hub.acl = build_acl(&sc.orchestrator, &shops);
    hub.auto_silence = Some(("review".into(), "judge".into()));

    let hub = Arc::new(hub);

    // 3. 启动 TCP 控制口 + 店调度器(后台线程)
    let hub2 = hub.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("rt");
        rt.block_on(async {
            let h = hub2.clone();
            tokio::spawn(async move {
                if let Err(e) = forge_runtime::serve_control(h, port).await {
                    eprintln!("控制口: {e}");
                }
            });
            let shop_list: Vec<String> = hub2.shops.keys().cloned().collect();
            for s in shop_list {
                let h = hub2.clone();
                let shop = s.clone();
                tokio::spawn(async move {
                    forge_runtime::shop_scheduler(h, shop).await;
                });
            }
            loop { tokio::time::sleep(Duration::from_secs(3600)).await; }
        });
    });
    std::thread::sleep(Duration::from_millis(1500));

    // 4. 预置规程信(经 TCP 控制口, 触发非静默投递)
    let empty: Vec<forge_core::Letter> = vec![];
    let setup_ctx = OrchestratorCtx { letters: &empty, shops: &shops };
    for s in orch.setup(&setup_ctx) {
        tide_send(&hub, port, &s.shop, &s.to, &s.body, s.silent);
    }

    // 5. 点火(经 TCP, 非静默 → 触发第一个 codex 回合)
    let (ign_to, ign_body) = orch.ignition();
    println!("点火 → {ign_to}: {}", ign_body.chars().take(30).collect::<String>().as_str());
    tide_send(&hub, port, "orchestrator", &ign_to, &ign_body, false);

    // 6. 编排器循环: 轮询账本、检测闸门、注入指令(5 秒一查, codex 回合需时间)
    let timeout_secs: u64 = std::env::var("FORGE_TIMEOUT")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    let start = Instant::now();
    let mut final_state = FinalState::Timeout;

    loop {
        std::thread::sleep(Duration::from_secs(5));
        let elapsed = start.elapsed().as_secs();

        let letters = hub.journal.replay();
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };

        // 处理编排器回调(on_letter)
        let mut injected = false;
        if let Some(last) = letters.last() {
            for action in orch.on_letter(last, &ctx) {
                match action {
                    OrchestratorAction::Inject { from, to, body, silent } => {
                        println!("  [编排器] 注入 → {to}: {}", body.chars().take(25).collect::<String>().as_str());
                        tide_send(&hub, port, &from, &to, &body, silent);
                        injected = true;
                    }
                    OrchestratorAction::Kick { shop, reason } => {
                        println!("  [编排器] 踢 → {shop}: {reason}");
                        tide_send(&hub, port, "orchestrator", &shop,
                            &format!("KICK=继续任务: {reason}"), false);
                        injected = true;
                    }
                    OrchestratorAction::Complete { state, detail } => {
                        println!("  [编排器] 终局: {state} — {detail}");
                        final_state = match state.as_str() {
                            "PASS" => FinalState::Pass,
                            _ => FinalState::Degenerate(detail),
                        };
                    }
                    _ => {}
                }
            }
        }

        // 注入后重读(可能同步触发了新回合)
        let letters2 = if injected { hub.journal.replay() } else { hub.journal.replay() };
        let ctx2 = OrchestratorCtx { letters: &letters2, shops: &shops };
        let state = orch.finalize(&ctx2);
        if state != FinalState::Timeout {
            final_state = state;
            break;
        }

        if elapsed > timeout_secs {
            println!("  超时 {timeout_secs}s ({})", letters.len());
            final_state = FinalState::Timeout;
            break;
        }
        if elapsed % 30 == 0 {
            println!("  等待中: {elapsed}s, {} 封信", letters.len());
        }
    }

    // 7. 判分输出
    let letters = hub.journal.replay();
    let turns: u64 = hub.shops.values()
        .map(|s| s.turns_done.load(std::sync::atomic::Ordering::Relaxed))
        .sum();
    let state_str = match &final_state {
        FinalState::Pass => "PASS".to_string(),
        FinalState::Degenerate(d) => format!("DEGENERATE({d})"),
        FinalState::Fail(f) => format!("FAIL({f})"),
        FinalState::Timeout => "TIMEOUT".to_string(),
    };

    format!(
        "scenario: {} ({} + codex)\nstate: {}\nletters: {}\nturns: {}\nduration: {}s\n",
        sc.meta.name, sc.orchestrator, state_str, letters.len(), turns,
        start.elapsed().as_secs()
    )
}

/// 经 TCP 控制口发信(非直接调 hub.send — codex 的 MCP 工具走这条路)
fn tide_send(hub: &Arc<TideHub>, port: u16, from: &str, to: &str, body: &str, silent: bool) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("连接控制口");
    let cmd = serde_json::json!({
        "tool": "pulsing_send",
        "args": {"_shop": from, "to": to, "message": body, "silent": silent}
    });
    stream.write_all(format!("{cmd}\n").as_bytes()).expect("写");
    let mut buf = [0u8; 1024];
    let _ = stream.read(&mut buf);
}
