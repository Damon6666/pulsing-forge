#!/usr/bin/env bash
# codex-chief.sh — S1: 启动 chief codex（内核补丁版；线程 e3..e8 自动登记身份）
# 环境变量:
#   HUB_ADDR      Hub 地址（FORGE_CTRL_ADDR）
#   CODEX_BIN     默认 /w/codex-native
#   MODEL         默认 glm-5-turbo
#   RELAY_URL     API 中继
#   PULSING_KEY   模型 Key（只从环境读）
#   SHOP_BASE     线程起始偏移（默认 2，线程身份从 e3 开始，e1/e2 留给外部框架）
set -euo pipefail

: "${PULSING_KEY:?请在环境变量中提供 PULSING_KEY}"
CODEX_BIN="${CODEX_BIN:-/w/codex-native}"
HUB_ADDR="${HUB_ADDR:-127.0.0.1:12410}"
RELAY_URL="${RELAY_URL:-http://127.0.0.1:4020/v1}"
MODEL="${MODEL:-glm-5-turbo}"
SHOP_BASE="${SHOP_BASE:-2}"

H="${CODEX_HOME:-/tmp/hetero-chief}"; rm -rf "$H"; mkdir -p "$H"
cat > "$H/config.toml" <<EOF
model = "$MODEL"
model_provider = "pulsing-relay"
[model_providers.pulsing-relay]
name = "pulsing-relay"
base_url = "$RELAY_URL"
env_key = "PULSING_API_KEY"
wire_api = "responses"
[agents]
max_concurrent_threads_per_session = 16
EOF

PROMPT="${CHIEF_PROMPT:-你是 chief 总编排。读取你自己信箱里的 ISSUE 信，拆解为诊断/方案任务，通过 pulsing_send 派发给 e3..e8 线程（每个线程一封），收到 ECHO 回执后汇总并向 orchestrator 发 REPORT。不要执行仓库改动。}"

PULSING_API_KEY="$PULSING_KEY" CODEX_HOME="$H" \
FORGE_CTRL_ADDR="$HUB_ADDR" FORGE_SHOP="chief" FORGE_SHOP_BASE="$SHOP_BASE" \
PULSING_AUTO_DISTRIBUTE="${PULSING_AUTO_DISTRIBUTE:-0}" \
exec "$CODEX_BIN" exec --skip-git-repo-check --ephemeral \
  --dangerously-bypass-approvals-and-sandbox "$PROMPT"
