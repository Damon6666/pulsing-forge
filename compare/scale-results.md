# 多 Agent 扩展性基准(沙盒实测)

**场景**: 总工作量 = 睡 600 秒后 "done"; broadcast 编排器切成 N 段并行, 每店真实 sleep 600/N。
**环境**: pbox 沙盒, 镜像 registry.h.pjlab.org.cn/ailab-sys/pulsing_forge:v1, 协议层全真(信箱/并发调度/收齐闸门/机器闭合/账本), 执行器=定时睡眠(FORGE_SLEEP_EXEC, 零 LLM — 与 Super-agent 文章消融实验同方法论)。

| N | 墙钟(实测) | 理论 600/N+开销 | 加速比 |
|---|---|---|---|
| 1 | 611s | 610s | 1.00x |
| 5 | 131.5s | 130s | 4.65x |
| 10 | 71.5s | 70s | 8.55x |
| 20 | 41.5s | 40s | 14.7x |
| 40 | 26.5s | 25s | 23.1x |
| 80 | 21.5s | 17.5s | 28.4x |

结论: N=5..40 近乎完美线性; N≥80 进入 ~10-15s 协议开销地板(轮询周期+去抖窗), 即 Amdahl 常量项 — 扩展瓶颈在协议开销而非 agent 数, 可通过缩短轮询/去抖进一步压低。
复现: scenarios/scale/sleep-N.toml + `FORGE_SLEEP_EXEC=1 forge run scenarios/scale/sleep-<N>.toml`（N=1 需 FORGE_TIMEOUT=900）。
