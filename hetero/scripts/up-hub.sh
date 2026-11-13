#!/usr/bin/env bash
# up-hub.sh — S1: 启动 TideHub 中央信箱（0.0.0.0 对外），店册 chief + e1..e8
# 环境变量:
#   HUB_PORT      默认 12410
#   FORGE_BIN     forge 二进制路径，默认 /w/forge
#   RELAY_URL     API 中继地址，默认 http://127.0.0.1:4020/v1
#   FORGE_API_KEY 模型 Key（只从环境读，禁止写死）
#   SHOPS         店册，默认 chief,e1,...,e8
set -euo pipefail

HUB_PORT="${HUB_PORT:-12410}"
FORGE_BIN="${FORGE_BIN:-/w/forge}"
RELAY_URL="${RELAY_URL:-http://127.0.0.1:4020/v1}"
SHOPS="${SHOPS:-chief,e1,e2,e3,e4,e5,e6,e7,e8}"
N=$(python3 - "$SHOPS" <<'PY'
import sys
print(len([s for s in sys.argv[1].split(",") if s and s != "chief"]))
PY
)

WORKDIR="${WORKDIR:-/w}"
SCENARIO="$WORKDIR/hetero.toml"

cat > "$SCENARIO" <<EOF
orchestrator = "broadcast"
adapter = "codex"
[meta]
name = "hetero-issue"
[task]
template = "异构多 Agent 修 issue: chief 拆解派信; e1=ClaudeCode 审查, e2=Aider 修复, e3..e8=codex 线程(诊断/测试). 各店完成任务后立即 pulsing_send 回 chief 一封 ECHO 信(格式 ECHO=<店名>|done|摘要), 只发一次. chief 收齐 ${N} 封 ECHO 后向 orchestrator 发 REPORT 收口."
hidden_check = "ECHO ${N}/${N}"
hidden_keyword = "ECHO"
n_reviewers = ${N}
EOF

# 幂等重启: 精确匹配本场景, 不误杀其他 forge 进程
pkill -f "forge run $SCENARIO" 2>/dev/null || true
sleep 1

cd "$WORKDIR"
FORGE_API_KEY="${FORGE_API_KEY:-placeholder}" \
FORGE_BASE_URL="$RELAY_URL" \
FORGE_CODEX_BIN="${FORGE_CODEX_BIN:-/usr/local/bin/codex}" \
FORGE_BIND=0.0.0.0 \
FORGE_REMOTE_SHOPS="$SHOPS" \
FORGE_TIMEOUT="${FORGE_TIMEOUT:-7200}" \
nohup "$FORGE_BIN" run "$SCENARIO" > "$WORKDIR/hub.out" 2>&1 &

echo "hub started: $FORGE_BIN run $SCENARIO (port $HUB_PORT, shops: $SHOPS)"
sleep 2
tail -5 "$WORKDIR/hub.out" || true
