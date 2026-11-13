# 异构多 Agent · 一个任务、三种框架、一张账本

![mailflow](assets/mailflow.gif)

> codex、Claude Code、Aider 三个框架的 Agent 通过中央信箱协作完成同一个任务。
> 身份、信箱、账本统一，接入方不改框架代码。

## 这是什么

多 Agent 的常见做法是同一框架加开几个实例：执行者和审查者用同一个模型、同一套判断标准，容易共享同一个盲区——审查者不会否决自己也会犯的错误。本项目把三种不同框架、不同档位模型的 Agent 接进同一张信箱网络，用角色分工与独立审查换取质量。

已验证案例：一次真实安全 Issue 修复。执行环节（Aider + glm-4-flash）给出过一份"机械检查全绿、但把 284 行 9 个函数砍成 28 行 0 个函数"的坏修复；异构审查员（Claude Code + glm-4.6）以"功能丧失、非最小修复"一票否决；流程自动进入自愈，最终交付"泄露清零 + 函数数量不变"的修复，并把函数计数写进验收标准。全程 20 封信入账，一把 API Key，端到端 1019 秒。

## 三种接入形态

| 沙盒 | 框架 | 模型 | 角色 | 接入方式 |
|---|---|---|---|---|
| S1 | codex（改造版） | glm-5-turbo | chief 总编排；e3–e8 线程（诊断/终验/自愈） | 内核补丁，线程身份自动登记 |
| S2 | Claude Code 2.1.3 | glm-4.6 | e1 独立审查员（一票否决） | MCP 桥 + attach 注入身份 |
| S3 | Aider 0.86 | glm-4-flash | e2 低成本修复工 | TCP 直连信箱（无 MCP） |

三条路径工程成本不同，接入后拿到的东西一致：统一身份（chief · e1 · e2 · e3–e8）、可寻址信箱、全量账本。接入之后，框架差异只体现在模型能力上，不影响协作方式。

## 快速开始

前提：三台主机/沙盒（都装有对应框架），一个可用的 API Key（**只走环境变量，禁止写进任何文件**）。

```bash
cp configs/fleet.example.toml configs/fleet.toml   # 按实际 IP / 模型填写

# S1（codex + Hub）
bash scripts/up-hub.sh

# S2（Claude Code）
PULSING_KEY=... bash scripts/up-claude.sh

# S3（Aider）
PULSING_KEY=... bash scripts/up-aider.sh

# 冒烟：确认 8 个身份在线、信箱可投
python3 scripts/smoke.py --hub <S1_IP>:12410
```

演示流程见 `demo/issue-repair.toml` 与 `demo/issue.md`。

## 目录

```
hetero/
├─ demo/         协作拓扑与演示任务定义
├─ configs/      集群/角色/模型映射模板
├─ scripts/      Hub 启动、三种 worker 启动、冒烟、停止
├─ bridge/       Claude Code 的 MCP 桥宿主（身份闭包注入）
├─ adapters/     三种框架的 worker 包装脚本
├─ tools/        账本回放、动画生成
├─ datasets/     一次真实运行的 20 封信账本（已脱敏）
└─ assets/       架构图、动画
```

## 账本回放

```bash
python3 tools/replay.py datasets/ledger-sample.json
```

输出每个 Agent 的收发时间线、四阶段（诊断/修复/审查/自愈）耗时、最终 verdict。账本是 append-only JSONL，崩溃后可从日志重放出待回复状态。

## 边界与说明

- **异构的价值主张是质量、抗盲区与可审计性，不是加速。** 3 节点规模下未见显著加速，本项目不作加速承诺。
- 接入需要模型 API；本仓库不包含任何 Key，示例中的内网地址均已脱敏。
- 审查门与机械门必须分开：机械门（grep、编译）只管"泄没泄露、能不能跑"，审查门管"是否还在正常工作"。只保留机械门的流程会放过"删光式修复"。

## 相关

- 集成层地图（五层调用链、补丁位置、rebase 回归清单）：`docs/codex-integration-map.md`
- 同构扩展性实验与透明分布式 codex：见仓库 docs/ 与 scripts/
