# 潮汐层安装 — 让 forge run 接上真 LLM agent

forge 的确定性层（内置店员）开箱即用、零外部依赖。要接上真 LLM agent（codex），需额外构建 pulsing-mcp——它是信箱分片进程，管理每回合临时 codex 子进程。

## 前置条件

| 依赖 | 用途 | 获取 |
|---|---|---|
| Rust ≥ 1.75 | 编译 pulsing-mcp | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| codex 二进制 | agent 运行时 | [openai/codex](https://github.com/openai/codex) 或任何 MCP 兼容 CLI |
| LLM API key | 模型调用 | 任意 OpenAI 兼容端点 |

## 构建 pulsing-mcp

```bash
git clone https://github.com/DeepLink-org/Pulsing.git
cd Pulsing
cargo build --release -p pulsing-mcp
# 产物: target/release/pulsing-mcp
```

## 配置

```bash
# provider.toml — LLM 端点（不要提交到任何仓库）
cat > /tmp/forge-tide/config/provider.toml <<EOF
base_url = "http://your-proxy:4011/v1"
api_key = "your-key"
wire_api = "responses"
EOF

# 告诉 forge 潮汐层二进制在哪
export FORGE_PULSING_MCP=/path/to/Pulsing/target/release/pulsing-mcp
export FORGE_CODEX_BIN=/path/to/codex
export FORGE_PROVIDER=/tmp/forge-tide/config/provider.toml
```

## 运行

```bash
# 场景 toml 中 adapter = "codex" 时走潮汐层
forge run scenarios/code-review-codex.toml
```

## 为什么不直接 vendor 进 forge

pulsing-actor（pulsing-mcp 的依赖）有 70 个源文件、30+ 外部 crate（含 TLS/HTTP2/OpenTelemetry/推理负载均衡），vendor 进来会把 forge 从"轻量产品"变成"小型操作系统"。正确路径是：

- **短期**：pulsing-mcp 作为独立构建产物，forge 经 codex_adapter 引用
- **中期**：把 shard.rs 的核心（信箱路由/回合管理/ACL）重写为 forge-runtime 的 tide 模块，只保留 ~300 行核心逻辑，剥离分布式 actor 依赖
- **长期**：pulsing-mcp 发布为 crates.io crate，forge 直接 `use pulsing_mcp`
