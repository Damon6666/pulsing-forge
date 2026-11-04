#!/usr/bin/env bash
# codex-fleet — 1沙盒×1真codex×N并行sleep, codex层触发
# 用法: codex-fleet.sh <总秒数> <沙盒数> <每沙盒并行sleep数>
set -eu
TOTAL=${1:-10800}; N=${2:-10}; SUB=${3:-50}
PER=$(( TOTAL / (N * SUB) ))
[ "$PER" -lt 1 ] && { echo "ERR: PER=$PER"; exit 1; }

KEY=$(grep -oP 'API_KEY=\K[^ ]+' /tmp/babysit-4015.sh | head -1)
GW="http://env-gateway.ailab.ailab.ai"
IMG="registry.h.pjlab.org.cn/ailab-sys/pulsing_forge:v1"

echo "═══ ${N}沙盒 × 1codex × ${SUB}并行sleep = ${N}codex/${N}*${SUB}agent ═══"
echo "总工作量 ${TOTAL}s, 每sleep ${PER}s"

# codex config (直连 bigmodel, chat_completions 格式)
cat > /tmp/codex-config.toml << EOF
model = "glm-5-turbo"
model_provider = "bigmodel"

[model_providers.bigmodel]
name = "bigmodel"
base_url = "https://open.bigmodel.cn/api/paas/v1"
env_key = "BIGMODEL_API_KEY"
wire_api = "chat_completions"
EOF

python3 -c "
import base64,json
b=base64.b64encode(open('/tmp/codex-config.toml','rb').read()).decode()
json.dump({'target_path':'/root/.codex/config.toml','content_b64':b},open('/tmp/uc.json','w'))"

# 建沙盒
declare -a E=() U=()
echo -n "[1] 建${N}沙盒... "
for i in $(seq 1 $N); do
  R=$(curl -sS -X POST "$GW/envs" -H "Content-Type: application/json" \
    -d "{\"env\":\"$IMG\",\"user\":\"autoolchain\",\"cluster_name\":\"pbox\",\"ttl_seconds\":600}" --max-time 15)
  E+=("$(echo "$R" | grep -oP '"env_id":"\K[0-9a-f]+')")
  U+=("$(echo "$R" | grep -oP '"url":"\K[^"]+')")
done; echo "OK"

echo -n "[2] 等ready... "
for i in $(seq 0 $((N-1))); do
  while :; do S=$(curl -s "$GW/envs/${E[$i]}" --max-time 8 | grep -oP '"status":"\K\w+' || true); [ "$S" = "ready" ] && break; sleep 2; done
done; echo "OK"

echo -n "[3] 上传config... "
for i in $(seq 0 $((N-1))); do
  curl -sS -X POST "${U[$i]%/}/upload" -H "Content-Type: application/json" -d @/tmp/uc.json --max-time 30 > /dev/null &
done; wait; echo "OK"

# 点火: 每沙盒跑 1 个 codex, 让它自己决定开 ${SUB} 个并行 sleep
echo "[4] 点火 ${N} 个 codex..."
PROMPT="Run exactly ${SUB} sleep processes in parallel, each sleeping ${PER} seconds, then wait for ALL to finish, then echo ALL_DONE. Use a single shell command."

T0=$(python3 -c "import time;print(time.time())")
for i in $(seq 0 $((N-1))); do
  (curl -sS -X POST "${U[$i]%/}/exec" -H "Content-Type: application/json" \
    -d "$(python3 -c "
import json
cmd = f'export BIGMODEL_API_KEY=\"$KEY\" && codex exec --skip-git-repo-check --ephemeral --dangerously-bypass-approvals-and-sandbox \'$PROMPT\' 2>&1 | tail -3'
print(json.dumps({'command': cmd, 'timeout_sec': 300}))" \
    )" --max-time 330 > /tmp/cx_out_$i.log 2>&1) &
done
wait
T1=$(python3 -c "import time;print(time.time())")
WALL=$(python3 -c "print(f'{$T1-$T0:.1f}')")

P=0; for i in $(seq 0 $((N-1))); do grep -qi "ALL_DONE\|done\|completed" /tmp/cx_out_$i.log 2>/dev/null && P=$((P+1)); done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " ${N}沙盒 × 1codex × ${SUB}并行 = $((N*SUB)) sleep单元"
echo " 总工作量: ${TOTAL}s ($(( TOTAL/3600 ))小时$(( TOTAL%3600/60 ))分)"
echo " 完成耗时: ${WALL}s"
echo " 成功: ${P}/${N}沙盒"
echo " 加速比: $(python3 -c "print(f'{$TOTAL/$WALL:.0f}')")×"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "RESULT	${N}	${SUB}	$((N*SUB))	${TOTAL}	${PER}	${WALL}	$(python3 -c "print(f'{$TOTAL/$WALL:.2f}')")	${P}"

# 清理
for i in $(seq 0 $((N-1))); do curl -s -X DELETE "$GW/envs/${E[$i]}" --max-time 8 > /dev/null & done; wait
echo "沙盒已清理"
