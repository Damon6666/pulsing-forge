//! forge-runtime::tide — 潮汐层: 信箱分片 + 回合管理 + codex 子进程。
//! 从 pulsing-mcp shard.rs 提炼的 ~300 行核心, 剥离分布式 actor 依赖。
//!
//! 架构(单进程, 零外部网络):
//!   控制口(TCP :listen+400) ← attach(stdio MCP) ← codex exec(每回合)
//!   ↓
//!   TideHub(信箱+ACL+账本+回合管理) → codex 子进程(spawn/timeout/retry/stderr)

use forge_core::Letter;
use crate::hub::{Acl, Journal};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 店状态: 回合计数 + 发送/读取标记(回合完成判定用)
#[derive(Debug)]
pub struct ShopState {
    pub name: String,
    pub model: String,
    pub sent_count: AtomicU64,
    pub drained_last: AtomicU64,
    pub turns_done: AtomicU64,
}

impl ShopState {
    pub fn new(name: &str, model: &str) -> Self {
        ShopState {
            name: name.to_string(),
            model: model.to_string(),
            sent_count: AtomicU64::new(0),
            drained_last: AtomicU64::new(0),
            turns_done: AtomicU64::new(0),
        }
    }
    pub fn log(&self, msg: String) {
        println!("[{}] shard {}: {}", 
            chrono_now(), self.name, msg);
    }
}

fn chrono_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    format!("{:02}:{:02}:{:02}", (secs/3600)%24, (secs/60)%60, secs%60)
}

/// 潮汐中枢: 全部店 + 信箱 + 账本 + ACL + codex 进程管理
pub struct TideHub {
    pub shops: HashMap<String, Arc<ShopState>>,
    pub mailboxes: HashMap<String, std::sync::Mutex<Vec<Letter>>>,
    pub journal: Journal,
    pub acl: Acl,
    pub codex_bin: String,
    pub provider_path: String,
    pub max_turns: u32,
    pub clerk_limit: u32,
    pub log_dir: String,
    pub listen_port: u16,
    /// 审查店→仲裁者回信自动静默(prefix, arbitrator)
    pub auto_silence: Option<(String, String)>,
    next_ts: AtomicU64,
    /// stderr 是否落账(诊断用)
    pub capture_stderr: bool,
}

impl TideHub {
    pub fn new(log_dir: &str, listen_port: u16) -> std::io::Result<Self> {
        Ok(TideHub {
            shops: HashMap::new(),
            mailboxes: HashMap::new(),
            journal: Journal::new(log_dir)?,
            acl: Acl::default(),
            codex_bin: String::new(),
            provider_path: String::new(),
            max_turns: 30,
            clerk_limit: 6,
            log_dir: log_dir.to_string(),
            listen_port,
            auto_silence: None,
            next_ts: AtomicU64::new(1),
            capture_stderr: true,
        })
    }

    pub fn add_shop(&mut self, name: &str, model: &str) {
        self.shops.insert(name.to_string(), Arc::new(ShopState::new(name, model)));
        self.mailboxes.insert(name.to_string(), std::sync::Mutex::new(Vec::new()));
    }

    fn tick(&self) -> u64 {
        self.next_ts.fetch_add(1, Ordering::SeqCst)
    }

    /// 发信: ACL → 自回声熔断 → 自动静默 → 记账 → 入箱
    pub fn send(&self, from: &str, to: &str, body: &str, silent: bool, from_codex: bool) -> Result<(), String> {
        if from_codex {
            self.acl.check_send(from, to)?;
            if from == to && !silent {
                return Err("禁止给自己发非静默信".into());
            }
        }
        let silent = if let Some((prefix, arb)) = &self.auto_silence {
            from_codex && from.starts_with(prefix) && to == arb
        } else { silent };

        let l = Letter {
            ts_ms: self.tick(),
            from: from.to_string(),
            to: to.to_string(),
            body: body.to_string(),
        };
        self.journal.append(&l);
        if let Some(mb) = self.mailboxes.get(to) {
            mb.lock().unwrap().push(l);
        }
        if !silent {
            // 触发回合(由调用方处理 — async)
        }
        Ok(())
    }

    /// 广播: ACL 收拢(不报错, 白名单外自动略去)
    pub fn broadcast(&self, from: &str, targets: &[String], body: &str, silent: bool, from_codex: bool) -> Vec<String> {
        let effective = if from_codex {
            self.acl.collapse(from, targets)
        } else {
            targets.to_vec()
        };
        for t in &effective {
            let _ = self.send(from, t, body, silent, from_codex);
        }
        effective
    }

    /// 破坏性读(账本即登记处)
    pub fn drain(&self, shop: &str) -> Vec<Letter> {
        if let Some(st) = self.shops.get(shop) {
            st.drained_last.store(1, Ordering::SeqCst);
        }
        self.mailboxes.get(shop)
            .map(|m| std::mem::take(&mut *m.lock().unwrap()))
            .unwrap_or_default()
    }

    /// 回合提示词: 嵌入唤醒信
    pub fn turn_prompt(&self, shop: &str, letter: &Letter) -> String {
        format!(
            "你是协作团队中的一员。店名: {shop}。职责: {shop}\n\n\
             你收到了一封来自 {from} 的信件, 全文如下:\n--- 信件开始 ---\n{body}\n--- 信件结束 ---\n\n\
             【必须调用工具】按信件指示调用工具完成任务\
             (可多次调用: pulsing_send/pulsing_broadcast/pulsing_drain_mail/pulsing_health);\
             若信件未指定动作, 则调用 pulsing_send 发送结果后结束。禁止纯文字回复、禁止总结、禁止解释。",
            shop = shop, from = letter.from, body = letter.body
        )
    }

    /// codex 子进程配置
    pub fn codex_config(&self, shop: &str, ctrl_addr: &str) -> String {
        // forge 二进制路径(attach 子命令用 — codex 会 spawn 它作为 MCP server)
        let forge_bin = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "forge".to_string());
        format!(
            "# generated by forge-runtime tide\nmodel = \"glm-5-turbo\"\nreview_model = \"glm-5-turbo\"\n\
             model_provider = \"pulsing-relay\"\n\n\
             [model_providers.pulsing-relay]\nname = \"pulsing-relay\"\n\
             base_url = \"{provider_base}\"\nenv_key = \"PULSING_API_KEY\"\nwire_api = \"responses\"\n\n\
             [mcp_servers.pulsing]\ncommand = \"{forge_bin}\"\n\
             args = [\"attach\", \"--control\", \"{ctrl}\", \"--shop\", \"{shop}\"]\n\
             startup_timeout_sec = 60\n",
            provider_base = self.provider_base_url(),
            forge_bin = forge_bin,
            ctrl = ctrl_addr,
            shop = shop,
        )
    }

    fn provider_base_url(&self) -> String {
        std::fs::read_to_string(&self.provider_path)
            .ok()
            .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
            .and_then(|v| v.get("base_url").and_then(|b| b.as_str().map(String::from)))
            .unwrap_or_else(|| "http://127.0.0.1:4011/v1".to_string())
    }

    /// codex 回合: spawn → 等待 → 检查产出 → 未读信重试
    /// (同步版: 调用方在 tokio::task::spawn_blocking 里调)
    pub fn run_turn_blocking(&self, shop: &str, trigger: &Letter) -> Result<(), String> {
        let st = self.shops.get(shop).ok_or("unknown shop")?;
        let turns = st.turns_done.fetch_add(1, Ordering::SeqCst) + 1;
        if turns > self.max_turns as u64 {
            return Err(format!("回合限频({})", self.max_turns));
        }

        let sent_before = st.sent_count.load(Ordering::SeqCst);
        let prompt = self.turn_prompt(shop, trigger);
        let hdir = format!("{}/codex-home-{}", self.log_dir, shop);
        let _ = std::fs::remove_dir_all(&hdir);
        std::fs::create_dir_all(&hdir).map_err(|e| e.to_string())?;

        let ctrl_addr = self.listen_port.to_string();
        std::fs::write(format!("{hdir}/config.toml"), self.codex_config(shop, &ctrl_addr))
            .map_err(|e| e.to_string())?;

        let pv = std::fs::read_to_string(&self.provider_path).ok();
        let api_key = pv.as_ref()
            .and_then(|s| toml::from_str::<toml::Value>(s).ok())
            .and_then(|v| v.get("api_key").and_then(|k| k.as_str().map(String::from)))
            .unwrap_or_default();

        let mut last_err = String::new();
        for _attempt in 0..3u8 {
            let output = std::process::Command::new(&self.codex_bin)
                .args(["exec", "--skip-git-repo-check", "--ephemeral",
                       "--dangerously-bypass-approvals-and-sandbox", &prompt])
                .env("CODEX_HOME", &hdir)
                .env("PULSING_API_KEY", &api_key)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output();

            match output {
                Ok(out) => {
                    if self.capture_stderr && !out.stderr.is_empty() {
                        let se = String::from_utf8_lossy(&out.stderr);
                        self.journal.append_stderr(shop, &se);
                    }
                }
                Err(e) => last_err = e.to_string(),
            }

            let sent = st.sent_count.load(Ordering::SeqCst) != sent_before;
            let drained = st.drained_last.load(Ordering::SeqCst) != 0;
            if sent || drained {
                return Ok(());
            }
            // 未读信且零发送: 仓内重试(taxonomy #5)
            st.log("回合未读信, 仓内重试".to_string());
        }
        Err(format!("回合3次均无产出: {last_err}"))
    }

    /// 完整回路校验(假REPORT免疫)
    pub fn report_is_final(&self, judge: &str, reviewer_prefix: &str,
                           min_fanout: usize, min_replies: usize) -> bool {
        let ls = self.journal.replay();
        let fan: std::collections::HashSet<_> = ls.iter()
            .filter(|l| l.from == judge && l.to.starts_with(reviewer_prefix))
            .map(|l| l.to.clone()).collect();
        let rep: std::collections::HashSet<_> = ls.iter()
            .filter(|l| l.from.starts_with(reviewer_prefix) && l.to == judge
                     && !l.body.starts_with("KICK="))
            .map(|l| l.from.clone()).collect();
        fan.len() >= min_fanout && rep.len() >= min_replies
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(dir: &str) { let _ = std::fs::remove_dir_all(dir); }

    /// 回归 #1+#2: ACL + 自回声熔断 + from_codex 标记
    #[test]
    fn acl_and_echo_guard() {
        clean("/tmp/forge-tide-test1");
        let mut hub = TideHub::new("/tmp/forge-tide-test1", 12400).unwrap();
        hub.add_shop("writer", "m");
        hub.add_shop("judge", "m");
        hub.add_shop("r1", "m");
        hub.acl = Acl::from_pairs(&[("r1", &["judge"]), ("judge", &["*"]), ("writer", &["judge"])]);

        // codex 调用: 越权拒绝
        assert!(hub.send("r1", "writer", "x", false, true).is_err());
        // codex 自发非静默: 拒绝
        assert!(hub.send("writer", "writer", "x", false, true).is_err());
        // codex 正常发送: 放行
        assert!(hub.send("writer", "judge", "DRAFT=x", false, true).is_ok());
        // 非codex(编排器): 不受限
        assert!(hub.send("orch", "writer", "TASK=x", false, false).is_ok());
    }

    /// 回归 #4: 审查回信自动静默
    #[test]
    fn auto_silence() {
        clean("/tmp/forge-tide-test2");
        let mut hub = TideHub::new("/tmp/forge-tide-test2", 12400).unwrap();
        hub.add_shop("r1", "m");
        hub.add_shop("judge", "m");
        hub.acl = Acl::from_pairs(&[("r*", &["judge"])]);
        hub.auto_silence = Some(("r".into(), "judge".into()));
        // r1→judge 非静默被转为静默(不触发回合)
        assert!(hub.send("r1", "judge", "CLEAN=r1", false, true).is_ok());
        // 账本记录了这封信
        assert_eq!(hub.journal.replay().len(), 1);
    }

    /// 回归 #2b: 广播收拢
    #[test]
    fn broadcast_collapse() {
        clean("/tmp/forge-tide-test3");
        let mut hub = TideHub::new("/tmp/forge-tide-test3", 12400).unwrap();
        hub.add_shop("judge", "m");
        hub.add_shop("r1", "m");
        hub.add_shop("r2", "m");
        hub.add_shop("writer", "m");
        hub.acl = Acl::from_pairs(&[("r1", &["judge"]), ("judge", &["*"])]);
        let sent = hub.broadcast("r1",
            &["judge".into(), "writer".into(), "r2".into()],
            "REVIEW", false, true);
        assert_eq!(sent, vec!["judge"]);  // 只发到白名单内的 judge
    }

    /// 回归 #3: drain 破坏性读后账本仍在
    #[test]
    fn ledger_survives_drain() {
        clean("/tmp/forge-tide-test4");
        let mut hub = TideHub::new("/tmp/forge-tide-test4", 12400).unwrap();
        hub.add_shop("a", "m");
        hub.add_shop("b", "m");
        hub.send("a", "b", "hello", true, false).unwrap();
        assert_eq!(hub.drain("b").len(), 1);
        assert!(hub.drain("b").is_empty());
        assert_eq!(hub.journal.replay().len(), 1);
    }

    /// 回归 #8: 假REPORT免疫(完整回路校验)
    #[test]
    fn fake_report_rejected() {
        clean("/tmp/forge-tide-test5");
        let mut hub = TideHub::new("/tmp/forge-tide-test5", 12400).unwrap();
        hub.add_shop("judge", "m");
        for i in 1..=4 { hub.add_shop(&format!("r{i}"), "m"); }
        // 只有派单, 没有群发和回信
        hub.send("judge", "judge", "REPORT=已完成派单", true, false).unwrap();
        assert!(!hub.report_is_final("judge", "r", 4, 4));
        // 补齐群发+回信
        for i in 1..=4 {
            hub.send("judge", &format!("r{i}"), "REVIEW=群审", false, false).unwrap();
            hub.send(&format!("r{i}"), "judge", "CLEAN", true, false).unwrap();
        }
        hub.send("judge", "judge", "REPORT=全部通过", true, false).unwrap();
        assert!(hub.report_is_final("judge", "r", 4, 4));
    }

    /// 回合提示词包含信件全文和店名
    #[test]
    fn turn_prompt_embeds_letter() {
        let mut hub = TideHub::new("/tmp/forge-tide-test6", 12400).unwrap();
        hub.add_shop("writer", "m");
        let l = Letter {
            ts_ms: 1, from: "judge".into(), to: "writer".into(),
            body: "TASK=写一个函数".into(),
        };
        let p = hub.turn_prompt("writer", &l);
        assert!(p.contains("TASK=写一个函数"));
        assert!(p.contains("writer"));
        assert!(p.contains("pulsing_send"));
    }
}
