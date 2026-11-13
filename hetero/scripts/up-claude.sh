#!/usr/bin/env bash
# up-claude.sh — S2: 启动 Claude Code worker（MCP 桥接，身份闭包注入）
# 环境变量:
#   HUB_ADDR        Hub 地址，默认 127.0.0.1:12410
#   WORKERS         工号列表，默认 e1
#   MODEL           默认 glm-4.6
#   PULSING_KEY     模型 Key（只从环境读）
#   ANTHROPIC_BASE_URL 默认 https://api.z.ai/api/anthropic
# 依赖: node >= 20, @anthropic-ai/claude-agent-sdk
set -euo pipefail

: "${PULSING_KEY:?请在环境变量中提供 PULSING_KEY}"
HUB_ADDR="${HUB_ADDR:-127.0.0.1:12410}"
WORKERS="${WORKERS:-e1}"
MODEL="${MODEL:-glm-4.6}"
HOST_JS="${HOST_JS:-$(cd "$(dirname "$0")/.." && pwd)/bridge/claude-host.mjs}"

export ANTHROPIC_BASE_URL="${ANTHROPIC_BASE_URL:-https://api.z.ai/api/anthropic}"
export ANTHROPIC_AUTH_TOKEN="$PULSING_KEY"
export DISABLE_TELEMETRY=1

echo "claude worker: hub=$HUB_ADDR workers=$WORKERS model=$MODEL"
node "$HOST_JS" --hub "$HUB_ADDR" --workers "$WORKERS" --model "$MODEL"
