//! Orchestrator — 编排即策略: 用 trait 替代硬编码循环。
//! 每种协作拓扑是一个 Orchestrator 实现, 由闸门原语组合而成。
//! 场景 toml 的 [orchestrator] type 决定用哪个。

use crate::Letter;
use std::collections::HashMap;

/// 编排器在每封信到达后可触发的动作
#[derive(Debug, Clone)]
pub enum OrchestratorAction {
    Continue,
    Kick { shop: String, reason: String },
    /// 机器代发一封信(如 ARBIT 裁决表 / REJECT 代投 / REPORT 默认收尾)
    Inject { from: String, to: String, body: String, silent: bool },
    /// 终局
    Complete { state: String, detail: String },
}

/// 终局判定
#[derive(Debug, Clone, PartialEq)]
pub enum FinalState {
    Pass,
    Degenerate(String),
    Fail(String),
    Timeout,
}

/// 编排器上下文: 传递给编排器的只读视图
pub struct OrchestratorCtx<'a> {
    pub letters: &'a [Letter],
    pub shops: &'a [String],
}

impl<'a> OrchestratorCtx<'a> {
    pub fn latest_from(&self, shop: &str) -> Option<&Letter> {
        self.letters.iter().rev().find(|l| l.from == shop)
    }
    pub fn latest_to(&self, shop: &str) -> Option<&Letter> {
        self.letters.iter().rev().find(|l| l.to == shop)
    }
    pub fn letters_from(&self, shop: &str) -> Vec<&Letter> {
        self.letters.iter().filter(|l| l.from == shop).collect()
    }
    pub fn letters_between(&self, from: &str, to: &str) -> Vec<&Letter> {
        self.letters.iter().filter(|l| l.from == from && l.to == to).collect()
    }
    /// 找以某前缀开头的最新信
    pub fn find_latest(&self, prefix: &str) -> Option<&Letter> {
        self.letters.iter().rev().find(|l| l.body.starts_with(prefix))
    }
    /// 波次时间戳: 最新群发信(或指定前缀)的 ts
    pub fn wave_ts(&self, from: &str, prefix: &str, after: u64) -> u64 {
        self.letters.iter()
            .filter(|l| l.from == from && l.body.starts_with(prefix) && l.ts_ms > after)
            .map(|l| l.ts_ms)
            .min().unwrap_or(0)
    }
}

/// 编排器 trait: 场景生命周期的三个钩子 + 终局判定
pub trait Orchestrator: Send + Sync {
    /// 场景初始化: 返回要预置的规程信列表
    fn setup(&self, ctx: &OrchestratorCtx) -> Vec<SetupLetter>;

    /// 点火信
    fn ignition(&self) -> (String, String); // (to_shop, body)

    /// 每封信到达后的回调: 返回 0~N 个动作
    fn on_letter(&self, letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction>;

    /// 终局判定
    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState;
}

/// 预置信
#[derive(Debug, Clone)]
pub struct SetupLetter {
    pub shop: String,
    pub to: String,
    pub body: String,
    pub silent: bool,
}

// ── 内置编排器 ──────────────────────────────────────────

/// 写审回路: writer → reviewers → judge (仲裁+打回+复审)
/// 闸门: CollectGate + VerdictTable + RegistryRelay + DualClosure
pub struct ReviewOrchestrator {
    pub judge: String,
    pub writer: String,
    pub reviewers: Vec<(String, String, String)>, // (name, duty, keyword)
    pub reviewer_prefix: String,
}

impl Orchestrator for ReviewOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        let mut seeds = Vec::new();
        let rv_names: Vec<String> = self.reviewers.iter().map(|r| r.0.clone()).collect();
        seeds.push(SetupLetter {
            shop: self.judge.clone(), to: self.judge.clone(), silent: true,
            body: format!(
                "【指挥规程{judge}】见TASK=: pulsing_send(to='{writer}', silent=false, message='TASK=原文||写手动作:写脚本放```python围栏后 pulsing_send(to=\"{judge}\",silent=false,message=\"DRAFT=```python全文```\")')\n\
                 见DRAFT=/REVISED=: pulsing_broadcast(to_list={rv_names:?}, message='REVIEW=群审||各查一事||底稿如下||'+脚本原文)\n\
                 见ARBIT=: 全CLEAN→REPORT收尾; 有不通→一封REJECT打回\n\
                 铁律: 不自己写代码; 收件人只填店名。",
                judge = self.judge, writer = self.writer, rv_names = rv_names
            ),
        });
        seeds.push(SetupLetter {
            shop: self.writer.clone(), to: self.writer.clone(), silent: true,
            body: format!(
                "【写手规程{w}】见TASK=: 按任务写python函数, ```python围栏, pulsing_send(to='{j}', silent=false, message='DRAFT=```python全文```')\n\
                 见REJECT=: 按意见补齐后重交 REVISED=```python全文```\n\
                 铁律: 只给{j}发信; 必须带```围栏。",
                w = self.writer, j = self.judge
            ),
        });
        for (name, duty, _kw) in &self.reviewers {
            seeds.push(SetupLetter {
                shop: name.clone(), to: name.clone(), silent: true,
                body: format!(
                    "【审查规程{n}】见REVIEW=: 只查「{d}」一事。\n\
                     合格: pulsing_send(to='{j}', silent=true, message='CLEAN={n}||{d}: 已覆盖')\n\
                     不合格: pulsing_send(to='{j}', silent=true, message='NEEDS_FIX={n}||{d}: 缺失')\n\
                     铁律: 只回{j}且silent=true。",
                    n = name, d = duty, j = self.judge
                ),
            });
        }
        seeds
    }

    fn ignition(&self) -> (String, String) {
        ("judge".to_string(), "TASK=开始代码审查。先drain读规程, 然后派单给writer。".to_string())
    }

    fn on_letter(&self, letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        // 审查店回信到达 → 检查是否收齐
        if letter.from.starts_with(&self.reviewer_prefix) && letter.to == self.judge {
            let n = self.reviewers.len();
            let latest_draft_ts = ctx.letters.iter()
                .filter(|l| l.from == self.writer && (l.body.starts_with("DRAFT=") || l.body.starts_with("REVISED=")))
                .map(|l| l.ts_ms).max().unwrap_or(0);
            let wave = ctx.wave_ts(&self.judge, "REVIEW=", latest_draft_ts);
            let replied: std::collections::HashSet<&str> = ctx.letters.iter()
                .filter(|l| l.from.starts_with(&self.reviewer_prefix) && l.to == self.judge
                         && l.ts_ms >= wave && !l.body.starts_with("KICK="))
                .map(|l| l.from.as_str()).collect();
            if replied.len() >= n {
                // 收齐 → 机器发 ARBIT 裁决表
                let table: Vec<String> = self.reviewers.iter()
                    .map(|(name, duty, _)| {
                        let last = ctx.letters.iter().rev()
                            .find(|l| l.from == *name && l.to == self.judge && l.ts_ms >= wave);
                        let verdict = last.map(|l| {
                            if l.body.starts_with("CLEAN") { "CLEAN" } else { "NEEDS_FIX" }
                        }).unwrap_or("?");
                        format!("{name}: {verdict} ({duty})")
                    }).collect();
                let nfix = table.iter().filter(|t| t.contains("NEEDS_FIX")).count();
                let action = if nfix == 0 {
                    "pulsing_send(to='judge', silent=true, message='REPORT=全部审查通过||最终脚本||三句汇报') 收尾。".to_string()
                } else {
                    format!(
                        "把全部NEEDS_FIX意见汇总成一封 pulsing_send(to='{}', silent=false, message='REJECT=意见汇总||缺失检查必须补齐||写手动作:补齐后重交REVISED=```python全文```')",
                        self.writer
                    )
                };
                actions.push(OrchestratorAction::Inject {
                    from: "orchestrator".to_string(),
                    to: self.judge.clone(),
                    body: format!(
                        "ARBIT=审查回信已收齐{}家。机器裁决表:\n{}\n结论: {}家不通过。下一步唯一动作: {}",
                        n, table.join("\n"), nfix, action
                    ),
                    silent: false,
                });
            }
        }
        // REPORT 到达 → 终局
        if letter.from == self.judge && letter.body.starts_with("REPORT=") {
            let complete = crate::report_is_final(
                ctx.letters, &self.judge, &self.reviewer_prefix,
                self.reviewers.len(), self.reviewers.len()
            );
            if complete {
                actions.push(OrchestratorAction::Complete {
                    state: "PASS".to_string(),
                    detail: letter.body.chars().take(80).collect::<String>(),
                });
            }
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let complete = crate::report_is_final(
            ctx.letters, &self.judge, &self.reviewer_prefix,
            self.reviewers.len(), self.reviewers.len()
        );
        let has_report = ctx.letters.iter().any(|l| l.from == self.judge && l.body.starts_with("REPORT="));
        if complete && has_report { FinalState::Pass }
        else if has_report { FinalState::Degenerate("REPORT存在但回路不完整".into()) }
        else { FinalState::Fail("无REPORT".into()) }
    }
}

/// 流水线: 顺序接力, 每步完成后自动嵌入下一步指令
pub struct PipelineOrchestrator {
    pub stages: Vec<(String, String)>, // (店名, 工序描述)
}

impl Orchestrator for PipelineOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        self.stages.iter().enumerate().map(|(i, (shop, duty))| {
            let next = if i + 1 < self.stages.len() {
                format!("完成后 pulsing_send(to='{}', silent=false, message='WORK=你的工序产物||下一步: {}')",
                    self.stages[i+1].0, self.stages[i+1].1)
            } else {
                format!("完成后 pulsing_send(to='{}', silent=true, message='REPORT=RESULT=最终产物||处理链完成')", shop)
            };
            SetupLetter {
                shop: shop.clone(), to: shop.clone(), silent: true,
                body: format!(
                    "【{shop}】见WORK=: 执行你的工序「{duty}」。{next}\n铁律: 只给下游或自己发信; REPORT只有最后一步能发。",
                    shop = shop, duty = duty, next = next
                ),
            }
        }).collect()
    }

    fn ignition(&self) -> (String, String) {
        let first = &self.stages[0].0;
        (first.clone(), format!(
            "TASK=开始流水线。先drain读规程, 执行你的工序「{}」, 然后转发给下游。",
            self.stages[0].1
        ))
    }

    fn on_letter(&self, _letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        // 最后一个店发 REPORT → 终局
        let last_shop = &self.stages.last().unwrap().0;
        if let Some(rep) = ctx.latest_from(last_shop) {
            if rep.body.starts_with("REPORT=") {
                actions.push(OrchestratorAction::Complete {
                    state: "PASS".to_string(),
                    detail: rep.body.chars().take(60).collect::<String>(),
                });
            }
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let last_shop = &self.stages.last().unwrap().0;
        // 先查缺失工序(优先级高于格式问题)
        let missing: Vec<&str> = self.stages.iter().enumerate()
            .filter(|(i, (s, _))| {
                let is_last = *i == self.stages.len() - 1;
                !ctx.letters.iter().any(|l| l.from == *s &&
                    (l.body.starts_with("WORK=") || (is_last && l.body.starts_with("REPORT="))))
            })
            .map(|(_, (s, _))| s.as_str()).collect();
        let has_report = ctx.letters.iter().any(|l| l.from == *last_shop && l.body.starts_with("REPORT="));
        if !missing.is_empty() && has_report {
            return FinalState::Fail(format!("工序未完成: {missing:?}"));
        }
        if !missing.is_empty() { return FinalState::Timeout }  // 工序在进行中
        if has_report { FinalState::Pass }
        else { FinalState::Timeout }  // 全工序完成但REPORT在路上
    }
}

/// 辩论: 正方+反方各一论据 → judge 裁定收尾
/// 解决 T7: CollectGate(2) → 自动 REPORT
pub struct DebateOrchestrator {
    pub judge: String,
    pub topic: String,
}

impl Orchestrator for DebateOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        vec![
            SetupLetter { shop: "pro_a".into(), to: "pro_a".into(), silent: true,
                body: "【正方pro_a】见ASK=: 就议题给一条支持论据(两句), pulsing_send(to='judge', silent=false, message='ARG=支持||论据')后结束。只回judge。".into() },
            SetupLetter { shop: "pro_b".into(), to: "pro_b".into(), silent: true,
                body: "【反方pro_b】见ASK=: 就议题给一条反对论据(两句), pulsing_send(to='judge', silent=false, message='ARG=反对||论据')后结束。只回judge。".into() },
            SetupLetter { shop: self.judge.clone(), to: self.judge.clone(), silent: true,
                body: "【裁判judge】见TASK=: 给两位辩手各发ASK=。收齐两份ARG后 REPORT 收尾(含正反论据+一句话裁定)。".into() },
        ]
    }

    fn ignition(&self) -> (String, String) {
        (self.judge.clone(), format!(
            "TASK=辩论: {}。先drain读规程, 给pro_a和pro_b各发一封ASK=(pulsing_send(to='pro_a', silent=false, message='ASK=请给支持论据')。", self.topic
        ))
    }

    fn on_letter(&self, letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        // ARG 到达 → 检查是否收齐 2 份
        if letter.body.starts_with("ARG=") && letter.to == self.judge {
            let args: std::collections::HashSet<&str> = ctx.letters.iter()
                .filter(|l| l.body.starts_with("ARG=") && l.to == self.judge)
                .map(|l| l.from.as_str()).collect();
            if args.len() >= 2 {
                // 收齐 → 机器注入收尾指令(解决"不知何时结束")
                actions.push(OrchestratorAction::Inject {
                    from: "orchestrator".into(),
                    to: self.judge.clone(),
                    body: "ARBIT=两份论据已收齐。立即 pulsing_send(to='judge', silent=true, message='REPORT=辩论完成||正方论据||反方论据||一句话裁定') 收尾。".into(),
                    silent: false,
                });
            }
        }
        if letter.from == self.judge && letter.body.starts_with("REPORT=") {
            actions.push(OrchestratorAction::Complete { state: "PASS".into(), detail: "辩论完成".into() });
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let has_report = ctx.letters.iter().any(|l| l.from == self.judge && l.body.starts_with("REPORT="));
        let has_both_args = ctx.letters.iter()
            .filter(|l| l.body.starts_with("ARG=") && l.to == self.judge)
            .map(|l| l.from.as_str()).collect::<std::collections::HashSet<_>>().len() >= 2;
        if has_report && has_both_args { FinalState::Pass }
        else if has_report { FinalState::Degenerate("REPORT但论据不全".into()) }
        else { FinalState::Timeout }  // codex需时间, 不提前判负
    }
}

/// 维修队(内鬼): coder+breaker 提交, verifier 独立核算, 机器多数表决
/// 解决 T9: VerdictTable 交叉校验
pub struct SaboteurOrchestrator {
    pub judge: String,
}

impl Orchestrator for SaboteurOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        vec![
            SetupLetter { shop: "coder".into(), to: "coder".into(), silent: true,
                body: "【coder】见TASK=<a>-<b>: python3算a..b平方和, pulsing_send(to='judge', silent=false, message='RESULT=coder||<a>-<b>||<和>')后结束。只回judge。".into() },
            SetupLetter { shop: "breaker".into(), to: "breaker".into(), silent: true,
                body: "【breaker】见TASK=<a>-<b>: 故意报一个偏离的数, pulsing_send(to='judge', silent=false, message='RESULT=breaker||<a>-<b>||<错数>')后结束。只回judge。".into() },
            SetupLetter { shop: "verifier_a".into(), to: "verifier_a".into(), silent: true,
                body: "【verifier_a】见CHECK=: python3独立核算, pulsing_send(to='judge', silent=true, message='VERIFY=<a>-<b>||<正确和>')后结束。".into() },
            SetupLetter { shop: "verifier_b".into(), to: "verifier_b".into(), silent: true,
                body: "【verifier_b】见CHECK=: python3独立核算, pulsing_send(to='judge', silent=true, message='VERIFY=<a>-<b>||<正确和>')后结束。".into() },
            SetupLetter { shop: self.judge.clone(), to: self.judge.clone(), silent: true,
                body: "【judge】见TASK=: 分两段区间给coder和breaker; 同时给两位verifier发CHECK=。收齐后以VERIFY为准聚合 REPORT=TOTAL=<正确总和>。".into() },
        ]
    }

    fn ignition(&self) -> (String, String) {
        (self.judge.clone(),
         "TASK=计算1..120平方和, 分两段给coder(1-60)和breaker(61-120), 同时给verifier_a和verifier_b发CHECK=两段区间。先drain读规程。".into())
    }

    fn on_letter(&self, letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        // VERIFY 到达 → 检查是否收齐全部(2 RESULT + 2 VERIFY)
        if letter.body.starts_with("VERIFY=") && letter.to == self.judge {
            let results: Vec<&Letter> = ctx.letters.iter()
                .filter(|l| l.body.starts_with("RESULT=") && l.to == self.judge).collect();
            let verifies: Vec<&Letter> = ctx.letters.iter()
                .filter(|l| l.body.starts_with("VERIFY=") && l.to == self.judge).collect();
            if results.len() >= 2 && verifies.len() >= 2 {
                // 机器交叉校验: 用 VERIFY 的值校对 RESULT, 剔除错值
                let correct_sum: u64 = verifies.iter()
                    .filter_map(|l| l.body.split("||").nth(1))
                    .filter_map(|s| s.trim().parse::<u64>().ok())
                    .sum();
                actions.push(OrchestratorAction::Inject {
                    from: "orchestrator".into(),
                    to: self.judge.clone(),
                    body: format!(
                        "ARBIT=全部回信已收齐。机器交叉校验: VERIFY独立核算总和={correct_sum}。\
                         立即 pulsing_send(to='judge', silent=true, message='REPORT=TOTAL={correct_sum}||内鬼已被纠正') 收尾。"
                    ),
                    silent: false,
                });
            }
        }
        if letter.from == self.judge && letter.body.starts_with("REPORT=TOTAL=") {
            actions.push(OrchestratorAction::Complete { state: "PASS".into(), detail: letter.body.chars().take(40).collect::<String>() });
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let rep = ctx.letters.iter().find(|l| l.from == self.judge && l.body.starts_with("REPORT=TOTAL="));
        if let Some(r) = rep {
            if r.body.contains("583220") { FinalState::Pass }
            else { FinalState::Degenerate(format!("TOTAL错误: {}", &r.body.chars().take(40).collect::<String>())) }
        } else { FinalState::Timeout }  // codex需时间
    }
}


/// 中继链: 信息隔离 + 逐跳推进 + 转发校验
pub struct RelayChainOrchestrator {
    pub steps: Vec<(String, String)>,
    pub input: String,
    pub current_step: std::sync::atomic::AtomicUsize,
}

impl RelayChainOrchestrator {
    pub fn new(steps: Vec<(String, String)>, input: &str) -> Self {
        RelayChainOrchestrator {
            steps,
            input: input.to_string(),
            current_step: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn make_instruction(&self, step: usize) -> String {
        let transform = &self.steps[step].1;
        let is_last = step == self.steps.len() - 1;
        let input_text = if step == 0 { self.input.clone() } else { "上游文本".to_string() };
        if is_last {
            let shop = &self.steps[step].0;
            format!(
                "TASK=你的信箱里已有指令信                 第二步: 对以下文本执行变换「{}」: {}                 第三步【唯一正确动作】: 调用 pulsing_send 工具, 参数 to={}, silent=true, message=REPORT=RELAY_RESULT=你变换后的实际输出结果                 铁律: 只做这一个变换; 禁止纯文字回复; 必须调用 pulsing_send 工具",
                transform, input_text, shop
            )
        } else {
            let next = &self.steps[step + 1].0;
            format!(
                "TASK=你的信箱里已有指令信                 第二步: 对以下文本执行变换「{}」: {}                 第三步【唯一正确动作】: 调用 pulsing_send 工具, 参数 to={}, silent=false, message=RELAY=你变换后的实际输出结果                 铁律: 只做这一个变换; 禁止纯文字回复; 必须调用 pulsing_send 工具; 收件人只能是 {}",
                transform, input_text, next, next
            )
        }
    }
}

impl Orchestrator for RelayChainOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        self.steps.iter().map(|(shop, transform)| {
            SetupLetter {
                shop: shop.clone(),
                to: shop.clone(),
                silent: true,
                body: format!("你收到文本和一个变换指令 执行[{}] 只做这一个变换", transform),
            }
        }).collect()
    }

    fn ignition(&self) -> (String, String) {
        let shop = self.steps[0].0.clone();
        (shop, self.make_instruction(0))
    }

    fn on_letter(&self, letter: &Letter, _ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        let current = self.current_step.load(std::sync::atomic::Ordering::SeqCst);

        if letter.body.starts_with("RELAY=") && current + 1 < self.steps.len() {
            self.current_step.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let next = current + 1;
            let output: String = letter.body.chars().skip(6).take(200).collect();
            let instruction = self.make_instruction(next).replace("上游文本", &output);
            actions.push(OrchestratorAction::Inject {
                from: "orchestrator".to_string(),
                to: self.steps[next].0.clone(),
                body: instruction,
                silent: false,
            });
        }

        if letter.body.starts_with("REPORT=RELAY_RESULT=") {
            actions.push(OrchestratorAction::Complete {
                state: "PASS".to_string(),
                detail: format!("中继链{}步完成", self.steps.len()),
            });
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let relay_count = ctx.letters.iter()
            .filter(|l| l.body.starts_with("RELAY="))
            .count();
        let has_report = ctx.letters.iter().any(|l| l.body.starts_with("REPORT=RELAY_RESULT="));
        let total = self.steps.len();
        if has_report && relay_count >= total - 1 {
            FinalState::Pass
        } else if has_report {
            FinalState::Degenerate(format!("REPORT但只有{} / {}跳RELAY", relay_count, total - 1))
        } else if relay_count > 0 {
            // 链在走(有RELAY但还没到REPORT) — 不要提前判负, 给codex时间完成
            FinalState::Timeout
        } else {
            // 没有任何RELAY — 也给首个codex回合时间(LLM需10-60秒), 不立即Fail
            FinalState::Timeout
        }
    }
}

/// 规模广播: chief 广播 → N 店回声 → 收齐计数
/// 解决 T10 并发瓶颈: 编排器识别广播模式, 批量触发 + 回声计数
pub struct BroadcastOrchestrator {
    pub chief: String,
    pub echo_shops: Vec<String>,
    pub message: String,
}

impl Orchestrator for BroadcastOrchestrator {
    fn setup(&self, _ctx: &OrchestratorCtx) -> Vec<SetupLetter> {
        let mut seeds = Vec::new();
        // 每家回声店的规程
        for shop in &self.echo_shops {
            seeds.push(SetupLetter {
                shop: shop.clone(), to: shop.clone(), silent: true,
                body: format!(
                    "[{}] 见ECHO=: pulsing_send(to={}, silent=true, message=ECHO={}|收到) 后结束",
                    shop, self.chief, shop
                ),
            });
        }
        // chief 规程: 广播一次, 收齐后 REPORT
        seeds.push(SetupLetter {
            shop: self.chief.clone(), to: self.chief.clone(), silent: true,
            body: format!(
                "[{}] 见TASK=: pulsing_broadcast(to_list={:?}, message=ECHO=报名, silent=false) 一次; 收齐全部{}个ECHO后 REPORT=COUNT={} 收尾",
                self.chief, self.echo_shops, self.echo_shops.len(), self.echo_shops.len()
            ),
        });
        seeds
    }

    fn ignition(&self) -> (String, String) {
        (self.chief.clone(),
         format!("TASK=广播{}家店并收集回声", self.echo_shops.len()))
    }

    fn on_letter(&self, letter: &Letter, ctx: &OrchestratorCtx) -> Vec<OrchestratorAction> {
        let mut actions = Vec::new();
        // ECHO 到达 chief → 检查是否收齐
        if letter.body.starts_with("ECHO=") && letter.to == self.chief {
            let echoes: std::collections::HashSet<&str> = ctx.letters.iter()
                .filter(|l| l.body.starts_with("ECHO=") && l.to == self.chief)
                .map(|l| l.from.as_str()).collect();
            let n = self.echo_shops.len();
            if echoes.len() >= n {
                actions.push(OrchestratorAction::Inject {
                    from: "orchestrator".into(),
                    to: self.chief.clone(),
                    body: format!(
                        "ARBIT=全部{}家ECHO已收齐({}/{}) 立即 pulsing_send(to='{}', silent=true, message='REPORT=COUNT={}|全部{}家回声') 收尾",
                        n, echoes.len(), n, self.chief, n, n
                    ),
                    silent: false,
                });
            }
        }
        // REPORT 到达 → 终局
        if letter.from == self.chief && letter.body.starts_with("REPORT=COUNT=") {
            actions.push(OrchestratorAction::Complete {
                state: "PASS".into(),
                detail: letter.body.chars().take(40).collect(),
            });
        }
        actions
    }

    fn finalize(&self, ctx: &OrchestratorCtx) -> FinalState {
        let echoes: std::collections::HashSet<&str> = ctx.letters.iter()
            .filter(|l| l.body.starts_with("ECHO=") && l.to == self.chief)
            .map(|l| l.from.as_str()).collect();
        let n = self.echo_shops.len();
        let has_report = ctx.letters.iter()
            .any(|l| l.from == self.chief && l.body.starts_with("REPORT=COUNT="));
        if has_report && echoes.len() >= n {
            FinalState::Pass
        } else if has_report {
            FinalState::Degenerate(format!("REPORT但只有{} / {}家回声", echoes.len(), n))
        } else if echoes.len() > 0 {
            FinalState::Timeout  // 回声在来, 不要提前判负
        } else {
            FinalState::Timeout  // 广播刚发出, 给时间
        }
    }
}

/// 信件 Schema: 发信时校验必需内容(解决 REPORT 缺围栏 / DRAFT 缺代码)
/// 校验失败返回教学性错误(工具层拒绝, 模型在同一回合内自我纠正)
pub struct LetterSchema {
    /// 必需前缀 (如 "REPORT=" / "DRAFT=")
    pub prefix: String,
    /// 必需包含的关键词 (如 ["```python", "return"])
    pub required_contains: Vec<String>,
    /// 禁止包含 (如 ["undefined"] 防止垃圾输出)
    pub forbidden_contains: Vec<String>,
}

impl LetterSchema {
    pub fn new(prefix: &str, required: &[&str]) -> Self {
        LetterSchema {
            prefix: prefix.to_string(),
            required_contains: required.iter().map(|s| s.to_string()).collect(),
            forbidden_contains: Vec::new(),
        }
    }

    /// 校验信件正文: 通过返回 Ok, 不通过返回教学性错误
    pub fn validate(&self, body: &str) -> Result<(), String> {
        if !body.starts_with(&self.prefix) {
            return Err(format!(
                "信件必须以 {} 开头; 当前正文前10字符: {}",
                self.prefix,
                body.chars().take(10).collect::<String>()
            ));
        }
        for kw in &self.required_contains {
            if !body.contains(kw.as_str()) {
                return Err(format!(
                    "信件缺少必需内容: {}; 请检查后重发(如 REPORT 须包含完整脚本围栏)",
                    kw
                ));
            }
        }
        for kw in &self.forbidden_contains {
            if body.contains(kw.as_str()) {
                return Err(format!("信件包含禁止内容: {}", kw));
            }
        }
        Ok(())
    }

    /// 常用 schema 快捷构造
    pub fn report_with_code() -> Self {
        Self::new("REPORT=", &["```"])
    }
    pub fn draft_code() -> Self {
        Self::new("DRAFT=", &["```"])
    }
    pub fn arbit() -> Self {
        Self::new("ARBIT=", &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(ts: u64, from: &str, to: &str, body: &str) -> Letter {
        Letter { ts_ms: ts, from: from.into(), to: to.into(), body: body.into() }
    }

    /// 回归 T7: 辩论 — 收齐2份ARG后必须自动注入ARBIT(解"不知何时结束")
    #[test]
    fn debate_collects_and_closes() {
        let orch = DebateOrchestrator { judge: "judge".into(), topic: "test".into() };
        let shops = vec!["judge".into(), "pro_a".into(), "pro_b".into()];
        let letters = vec![
            l(1, "judge", "pro_a", "ASK=请给支持论据"),
            l(2, "judge", "pro_b", "ASK=请给反对论据"),
            l(3, "pro_a", "judge", "ARG=支持||AI利大于弊"),
            l(4, "pro_b", "judge", "ARG=反对||AI有风险"),
        ];
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        // 第4封信(ARG from pro_b)到达 → 应触发 ARBIT 注入
        let actions = orch.on_letter(&letters[3], &ctx);
        assert!(actions.iter().any(|a| matches!(a, OrchestratorAction::Inject { body, .. } if body.contains("ARBIT"))),
            "收齐2份ARG后应注入ARBIT: {actions:?}");
        // 终局: judge REPORT → PASS
        let with_report = [letters.clone(), vec![l(5, "judge", "judge", "REPORT=辩论完成")]].concat();
        let ctx2 = OrchestratorCtx { letters: &with_report, shops: &shops };
        let actions2 = orch.on_letter(&with_report[4], &ctx2);
        assert!(actions2.iter().any(|a| matches!(a, OrchestratorAction::Complete { state, .. } if state == "PASS")));
    }

    /// 回归 T9: 维修队 — VERIFY到达后自动交叉校验并注入正确TOTAL
    #[test]
    fn saboteur_cross_verifies() {
        let orch = SaboteurOrchestrator { judge: "judge".into() };
        let shops = vec!["judge".into(), "coder".into(), "breaker".into(),
                         "verifier_a".into(), "verifier_b".into()];
        let letters = vec![
            l(1, "judge", "coder", "TASK=1-60"),
            l(2, "judge", "breaker", "TASK=61-120"),
            l(3, "judge", "verifier_a", "CHECK=1-60"),
            l(4, "judge", "verifier_b", "CHECK=61-120"),
            l(5, "coder", "judge", "RESULT=coder||1-60||73810"),
            l(6, "breaker", "judge", "RESULT=breaker||61-120||99999"),  // 错值
            l(7, "verifier_a", "judge", "VERIFY=1-60||73810"),
            l(8, "verifier_b", "judge", "VERIFY=61-120||509410"),
        ];
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        let actions = orch.on_letter(&letters[7], &ctx);
        // 应注入包含正确总和(73810+509410=583220)的ARBIT
        assert!(actions.iter().any(|a| matches!(a, OrchestratorAction::Inject { body, .. } if body.contains("583220"))),
            "VERIFY收齐后应注入正确TOTAL=583220: {actions:?}");
    }

    /// 回归 T3: 流水线 — 最后工序的WORK信到达后应可判定完成
    #[test]
    fn pipeline_detects_completion() {
        let orch = PipelineOrchestrator {
            stages: vec![
                ("planner".into(), "写三行大纲".into()),
                ("writer".into(), "按大纲写五句".into()),
                ("editor".into(), "改一句表达".into()),
            ],
        };
        let shops = vec!["planner".into(), "writer".into(), "editor".into()];
        let letters = vec![
            l(1, "planner", "writer", "WORK=大纲||1.AI协作 2.信件通信 3.写审回路"),
            l(2, "writer", "editor", "WORK=五句短文||AI agents cooperate through letters..."),
            l(3, "editor", "editor", "REPORT=RESULT=最终五句||处理链完成"),
        ];
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        let state = orch.finalize(&ctx);
        assert_eq!(state, FinalState::Pass, "三步全走完+REPORT → 应PASS");
        // 缺writer的WORK → 应FAIL并指出缺失工序
        let partial = vec![letters[0].clone(), letters[2].clone()];
        let ctx2 = OrchestratorCtx { letters: &partial, shops: &shops };
        let state2 = orch.finalize(&ctx2);
        assert!(matches!(state2, FinalState::Fail(ref msg) if msg.contains("writer")),
            "缺writer工序应FAIL: {state2:?}");
    }

    /// 回归 T6: 写审 — complete loop + REPORT → PASS
    #[test]
    fn review_completes() {
        let orch = ReviewOrchestrator {
            judge: "judge".into(), writer: "writer".into(),
            reviewers: vec![
                ("review_a".into(), "检查A".into(), "kw_a".into()),
                ("review_b".into(), "检查B".into(), "kw_b".into()),
            ],
            reviewer_prefix: "review".into(),
        };
        let shops = vec!["judge".into(), "writer".into(), "review_a".into(), "review_b".into()];
        let mut letters = vec![
            l(1, "judge", "writer", "TASK=写代码"),
            l(2, "writer", "judge", "DRAFT=```python\npass\n```"),
            l(3, "judge", "review_a", "REVIEW=群审||底稿"),
            l(4, "judge", "review_b", "REVIEW=群审||底稿"),
            l(5, "review_a", "judge", "NEEDS_FIX=review_a||检查A: 缺失"),
            l(6, "review_b", "judge", "CLEAN=review_b||检查B: 已覆盖"),
        ];
        // 第一波收齐 → 应注入ARBIT(有NEEDS_FIX → REJECT方向)
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        let actions = orch.on_letter(&letters[5], &ctx);
        assert!(actions.iter().any(|a| matches!(a, OrchestratorAction::Inject { body, .. } if body.contains("ARBIT"))),
            "2/2回信收齐应注入ARBIT");
        // 修正后第二波全CLEAN → REPORT → PASS
        letters.extend(vec![
            l(7, "judge", "writer", "REJECT=补检查A"),
            l(8, "writer", "judge", "REVISED=```python\nassert kw_a\n```"),
            l(9, "judge", "review_a", "REVIEW=群审||修正稿"),
            l(10, "judge", "review_b", "REVIEW=群审||修正稿"),
            l(11, "review_a", "judge", "CLEAN=review_a||检查A: 已覆盖"),
            l(12, "review_b", "judge", "CLEAN=review_b||检查B: 已覆盖"),
            l(13, "judge", "judge", "REPORT=全部审查通过"),
        ]);
        let ctx2 = OrchestratorCtx { letters: &letters, shops: &shops };
        let state = orch.finalize(&ctx2);
        assert_eq!(state, FinalState::Pass);
    }
}

#[cfg(test)]
mod new_tests {
    use super::*;

    fn l(ts: u64, from: &str, to: &str, body: &str) -> Letter {
        Letter { ts_ms: ts, from: from.into(), to: to.into(), body: body.into() }
    }

    /// 回归 T10: 规模广播 — 收齐 N 家回声后自动注入 ARBIT
    #[test]
    fn broadcast_collects_echoes() {
        let orch = BroadcastOrchestrator {
            chief: "chief".into(),
            echo_shops: vec!["e1".into(), "e2".into(), "e3".into()],
            message: "报名".into(),
        };
        let shops = vec!["chief".into(), "e1".into(), "e2".into(), "e3".into()];
        let letters = vec![
            l(1, "chief", "e1", "ECHO=报名"),
            l(2, "chief", "e2", "ECHO=报名"),
            l(3, "chief", "e3", "ECHO=报名"),
            l(4, "e1", "chief", "ECHO=e1|收到"),
            l(5, "e2", "chief", "ECHO=e2|收到"),
            l(6, "e3", "chief", "ECHO=e3|收到"),
        ];
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        // 第6封信(e3的ECHO)到达 → 3/3收齐 → 注入ARBIT
        let actions = orch.on_letter(&letters[5], &ctx);
        assert!(actions.iter().any(|a| matches!(a, OrchestratorAction::Inject { body, .. } if body.contains("ARBIT"))),
            "3/3 ECHO收齐应注入ARBIT: {actions:?}");
        // REPORT到达 → Complete
        let with_report = letters.clone();
        let report = l(7, "chief", "chief", "REPORT=COUNT=3|全部3家回声");
        let ctx2 = OrchestratorCtx {
            letters: &[with_report.clone(), vec![report.clone()]].concat(),
            shops: &shops,
        };
        let actions2 = orch.on_letter(&report, &ctx2);
        assert!(actions2.iter().any(|a| matches!(a, OrchestratorAction::Complete { state, .. } if state == "PASS")));
    }

    /// 回归 T10: 部分回声 → 不判负(Timeout)
    #[test]
    fn broadcast_partial_not_fail() {
        let orch = BroadcastOrchestrator {
            chief: "chief".into(),
            echo_shops: vec!["e1".into(), "e2".into(), "e3".into()],
            message: "报名".into(),
        };
        let shops = vec!["chief".into(), "e1".into(), "e2".into(), "e3".into()];
        let letters = vec![
            l(1, "e1", "chief", "ECHO=e1|收到"),
            l(2, "e2", "chief", "ECHO=e2|收到"),
            // e3 没回
        ];
        let ctx = OrchestratorCtx { letters: &letters, shops: &shops };
        let state = orch.finalize(&ctx);
        assert_eq!(state, FinalState::Timeout, "部分回声不应判负");
    }

    /// 回归 T6/T1-V3: LetterSchema — REPORT 缺围栏被拒
    #[test]
    fn schema_rejects_missing_fence() {
        let schema = LetterSchema::report_with_code();
        // 有围栏 → 通过
        assert!(schema.validate("REPORT=全部通过\n```python\ndef f(): pass\n```").is_ok());
        // 无围栏 → 拒绝(教学性错误)
        let err = schema.validate("REPORT=全部通过，代码已完成").unwrap_err();
        assert!(err.contains("缺少必需内容"), "错误信息应说明缺失: {err}");
    }

    /// LetterSchema — 错误前缀被拒
    #[test]
    fn schema_rejects_wrong_prefix() {
        let schema = LetterSchema::draft_code();
        // 正确前缀 → 通过
        assert!(schema.validate("DRAFT=```python\ncode\n```").is_ok());
        // 错误前缀 → 拒绝
        let err = schema.validate("这是我写的代码").unwrap_err();
        assert!(err.contains("必须以"), "错误信息应说明前缀: {err}");
    }

    /// LetterSchema — 多关键词校验
    #[test]
    fn schema_multiple_keywords() {
        let schema = LetterSchema::new("REVISED=", &["```python", "return"]);
        assert!(schema.validate("REVISED=```python\ndef f(): return True\n```").is_ok());
        assert!(schema.validate("REVISED=```python\npass\n```").is_err());
    }
}
