#!/usr/bin/env bash
# stop.sh — 停止异构组件（精确匹配，避免误杀）
set -uo pipefail
pkill -f "forge run /w/hetero.toml" 2>/dev/null || true
pkill -f "claude-host.mjs" 2>/dev/null || true
pkill -f "aider-worker[.]sh" 2>/dev/null || true
echo "stopped: hub / claude-host / aider-worker"
