#!/usr/bin/env bash
# up-aider.sh — S3: 启动 Aider worker（TCP 直连信箱，无 MCP）
# 环境变量:
#   HUB_ADDR       Hub 地址，默认 127.0.0.1:12410
#   WORKER_ID      工号，默认 e2
#   MODEL          aider 模型名，默认 openai/glm-4-flash
#   REPO_PATH      待修复仓库路径，默认 /w/repo
#   OPENAI_API_KEY 模型 Key（只从环境读）
set -euo pipefail

: "${OPENAI_API_KEY:?请在环境变量中提供 OPENAI_API_KEY}"
HUB_ADDR="${HUB_ADDR:-127.0.0.1:12410}"
export HUB_ADDR
export WORKER_ID="${WORKER_ID:-e2}"
export MODEL="${MODEL:-openai/glm-4-flash}"
export REPO_PATH="${REPO_PATH:-/w/repo}"
export OPENAI_API_BASE="${OPENAI_API_BASE:-https://api.z.ai/api/paas/v4}"

ADAPTER="${ADAPTER:-$(cd "$(dirname "$0")/.." && pwd)/adapters/aider-worker.sh}"
echo "aider worker: hub=$HUB_ADDR id=$WORKER_ID model=$MODEL repo=$REPO_PATH"
exec bash "$ADAPTER"
