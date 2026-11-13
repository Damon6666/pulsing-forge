# codex 集成层地图

pulsing 在 codex fork 里的全部改动落点。五层由外到内，每层说明：管什么、调用链、补丁位置、升级风险。

## 总览

| 层 | 核心文件 | 职责 | 补丁类型 |
|---|---|---|---|
| L1 入口与配置 | exec/src/{main,cli,lib}.rs；core/src/config/；config-schema | 命令行、进程内协议、配置与 provider 解析 | 零补丁（环境变量控制） |
| L2 回合与生命周期 | core/src/session/（turn、session、inject、input_queue）；core/src/agent/（control、registry） | 一次 turn 的组装执行；线程/子代理注册与限额 | 改上游：attach_root 调用点 |
| L3 工具层 | core/src/tools/（registry、router、parallel、handlers/） | 工具注册、路由、并行执行；MCP 接入 | 新增：multi_agents_v2 seam |
| L4 上下文层 | core/src/context/、context_manager/、compact*.rs、context_window.rs、token_budget.rs | 提示词片段、窗口预算、压缩、注入 | 轻改：信件进提示 |
| L5 观测账本（横切） | core/src/pulsing/（mod、ledger、guardrail、peer） | 事件、回复台账、看门狗、判词、注入防御 | 全新：pulsing 模块 |

三个总原则：

1. 上游 codex 已带原生多智能体基础设施（AgentRegistry、spawn_agent、SubAgentSource），我们的工作是在其上增加第二套语义（信箱公民权），不是从零实现多 Agent。
2. pulsing 的开关是环境变量 `CODEX_PULSING`，未开启时所有集成点走上游原逻辑，这是 fork 能持续 rebase 的前提。
3. 账本是写后旁路（write-behind），消息路径不碰磁盘。

## L1 入口与配置

进程启动、参数解析、配置与 provider 读取。基本未改动，但它是所有环境变量的入口（`CODEX_PULSING` 系列十余个变量在 `pulsing/mod.rs` 读取），rebase 后首先确认这里仍识别这些变量。

exec 输出约定：默认模式 stdout 只输出最终消息，`--json` 模式 stdout 为逐行 JSONL，其余走 stderr。

## L2 回合与生命周期

一次 turn 从 `run_turn` 开始：组装输入、进上下文、调模型、循环执行工具、收口。pulsing 在本层两个关键落点：

- `session.rs` 的 `attach_root`：会话初始化时把真实 `agent_control` 与 `thread_id` 换进 pulsing Runtime（`peer_control`、`peer_root` 两个槽位热替换），之后 judge 会话才能收信件注入。
- `inject.rs` 的 `inject_if_running`：把外来输入注入正在运行的回合（需要 active_turn 锁）。

风险：`run_turn` 循环结构与 `active_turn` 锁语义，任一处变化都会破坏注入的原子性前提。

## L3 工具层

工具注册在 `registry.rs`，分发在 `router.rs`，并行在 `parallel.rs`，实现在 `handlers/`。上游有 v1/v2 两套多智能体工具，pulsing 接缝在 v2：

- `message_tool.rs`：pulsing 开启时发信改走 `pulsing::send_remote`；
- `fanout_task.rs`：团队模式下调用 `pulsing::fanout_roster_letters` 零参数群发（收件人从花名册枚举、正文从锚点底稿逐字拼装）。

协作闸门（底稿未登记不群发、收件白名单、打回在途守卫）也在本层。

风险：v1/v2 命名空间前缀、ToolInvocation/ToolPayload/ToolOutput 签名、parallel 取消语义。

## L4 上下文层

提示词拼装、窗口预算、压缩。改动较轻：信件进提示在 `pulsing/mod.rs` 组装，超长正文截断后带信件编号，模型用召回工具按编号取全文；账本只存元数据，正文靠重发保证一致。

风险：上游提示词结构调整会使基于提示词行为的判据（组队规范、闸门提示）失效；compact 策略改版影响长回合的账本一致性假设。

## L5 观测账本（横切）

代码全部在 `core/src/pulsing/`：

- `mod.rs`：语义总线（emit_event、回合判词、mesh 健康、看门狗、降级、验收）；
- `ledger.rs`：写后账本（append-only JSONL；事件 INIT/TEAM/PENDING/REPLIED/GIVEUP/EXIT；崩溃后重放日志重建状态）；
- `guardrail.rs`：提示注入第一道闸（正则 tripwire，剥掉越权句并上报）；
- `peer.rs`：线格式 `InterAgentWire`（from_shop、from_thread、to_shop、body、trigger_turn，外加 MailSource 区分信件来源权威性）。

风险最高的是序列化兼容：wire 字段或 ledger schema 一旦改动，新旧二进制混网会静默丢失语义。

## rebase 回归清单

| 顺序 | 检查点 | 回归方法 |
|---|---|---|
| 1 | CODEX_PULSING* 环境变量仍被读取（L1） | 单店启动冒烟：设 NAME/ROLE/LISTEN |
| 2 | attach_root 与 inject_if_running 语义（L2） | 注入冒烟：向运行中会话投信，确认进回合并回复 |
| 3 | AgentRegistry 限额与 spawn_agent 签名（L2） | 多线程 spawn 冒烟：身份 e{n} 不冲突 |
| 4 | multi_agents_v2 工具名与路由签名（L3） | 单机 3-codex 演示 + fanout 覆盖审计 |
| 5 | context fragment 顺序与压缩策略（L4） | 固定输入单回合，比对提示词快照 |
| 6 | wire 格式与 ledger schema（L5） | 混合版本组网；judge 崩溃恢复回放 |

五层的改动集中在 L2 和 L3，L1 基本未动；风险最高的是 L5 的序列化兼容和 L2 的注入语义——任一处变化都会导致账本和唤醒静默失真，排查难度高于编译错误。
