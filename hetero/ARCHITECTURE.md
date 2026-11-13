# 架构：三种框架，一套身份、信箱与账本

## 1. 角色与拓扑

```
                    ┌──────────────────────────┐
                    │  S1 · TideHub 中央信箱    │
                    │  TCP :12410 · 单行 JSON   │
                    │  花名册 chief,e1..e8      │
                    │  全量账本(append-only)    │
                    └────────────┬─────────────┘
                                 │
        ┌────────────────────────┼────────────────────────┐
        │                        │                        │
┌───────┴────────┐      ┌────────┴─────────┐     ┌────────┴─────────┐
│ S1 codex 内核   │      │ S2 Claude Code   │     │ S3 Aider         │
│ chief 总编排    │      │ e1 审查员        │     │ e2 修复工        │
│ e3-e8 线程      │      │ MCP 桥 + attach  │     │ TCP 直连(无MCP)  │
│ 内核补丁自动登记 │      │ glm-4.6          │     │ glm-4-flash      │
│ glm-5-turbo     │      └──────────────────┘     └──────────────────┘
└─────────────────┘
```

身份命名空间全局唯一：`chief`（编排）、`e1`（Claude 审查）、`e2`（Aider 修复）、`e3–e8`（codex 线程：诊断/终验/自愈）。

## 2. 消息协议

单行 JSON over TCP，一行请求、一行回执：

```json
{"tool":"pulsing_send","args":{"to":"e2","message":"FIX-TASK ...","_shop":"chief"}}
{"tool":"pulsing_drain_mail","args":{"_shop":"e2"}}
{"tool":"pulsing_recall","args":{"query":"FIX-TASK","_shop":"e2"}}
{"tool":"pulsing_health","args":{}}
```

- `_shop` 是发件人身份，由各接入层自动注入，模型不可见也无需填写（Claude 走 MCP 闭包、codex 走内核补丁、Aider 由驱动脚本直投）。
- 回执统一为 `{"ok":true,"result":"...","error":""}`。
- 长信截断后带信件编号，接收方用 `pulsing_recall` 按编号取全文。

## 3. 流水线（demo/issue-repair.toml）

| 阶段 | 执行者 | 产出 | 门槛 |
|---|---|---|---|
| A 诊断 | e3 审计、e4 方案（codex 双线程并行） | 泄露审计报告、修复方案 | 主编排汇总 |
| B 修复 | e2（Aider） | 文件改动 + diff 回执 | 机械门：grep 残留、编译通过 |
| C 审查 | e1（Claude，一票否决） | verdict=PASS/CONCERNS + 修改边界 | 审查门：是否最小修复、功能是否保全 |
| D 自愈 | e5 终验、e6 重修 | 最小重修 + 新验收门 | 终验三项：地址清零、函数数一致、编译通过 |

关键设计：**机械门与审查门分离**。机械门只回答"泄没泄露、能不能跑"，审查门回答"是不是还在正常工作"。只保留机械门的流程会放过"删光式修复"（把文件的函数全部删掉，泄露自然清零）。

## 4. 账本

- append-only JSONL，事件类型 INIT / TEAM / PENDING / REPLIED / GIVEUP / EXIT。
- 消息路径不落盘（写后旁路），正文靠重发保证一致；账本只存元数据与信件索引。
- 崩溃恢复：重放日志重建待回复状态与任务计数，末行没有 EXIT 按崩溃处理。
- 回放工具：`tools/replay.py`，输出每个身份的时间线与阶段耗时。

## 5. 各框架接入要点

### codex（内核补丁）
- 线程创建时自动登记身份 `e{n}`（spawn 补丁），每个线程一个信箱。
- pulsing 工具族（send/drain/exec/health）注册进模型工具列表；exec 为同步语义。
- 环境变量控制：`FORGE_CTRL_ADDR`、`FORGE_SHOP`、`FORGE_SHOP_BASE`。

### Claude Code（MCP 桥）
- `bridge/claude-host.mjs` 用 SDK 的 `createSdkMcpServer` 为每个工号生成**一套工具闭包**，`_shop` 焊死在闭包里——模型只能以自己的身份发信。
- 启动：`node bridge/claude-host.mjs --hub <ip>:12410 --workers e1 --model glm-4.6`。

### Aider（无 MCP，驱动侧直投）
- 由 `adapters/aider-worker.sh` 轮询自身信箱（drain），把任务信全文作为 aider 的 message，改动后回执 diff。
- 身份由 `_shop` 参数直接携带，不需要框架支持。

## 6. 已知坑（都在脚本里处理了）

- **沙盒代理环境变量**：`http_proxy` 会让指向内网的请求被劫持，worker 里对本地调用显式绕过代理。
- **pkill 自匹配**：`pkill -f worker` 会匹配到启动脚本自身的命令行；脚本统一改用 `[w]orker` 写法或按 PID 处理。
- **Key 管理**：所有 Key 只从环境变量读（`PULSING_KEY` / `FORGE_API_KEY`），仓库内不出现任何凭据。
- **启动波次**：多个 Agent 同时启动会撞模型并发上限，脚本按批 spawn、批间留间隔。
