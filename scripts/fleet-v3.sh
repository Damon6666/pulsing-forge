#!/usr/bin/env bash
# fleet v3 — 极简: 并行同步exec, 以curl完成时刻计时
set -eu
TOTAL=${1:-60}; N=${2:-1}; K=${3:-5}
IMAGE="registry.h.pjlab.org.cn/ailab-sys/pulsing_forge:v1"
GW="http://env-gateway.ailab.ailab.ai"
FORGE="/mnt/c/Users/daimeng/.zcode/workspace/default/forge/target/release/forge"
PER=$(( TOTAL / (N * K) ))
[ "$PER" -lt 1 ] && { echo "ERR: PER=$PER"; exit 1; }

echo "═══ ${N}沙盒×${K}actor=$((N*K))agent, 总${TOTAL}s, 每agent${PER}s ═══"

# 场景
cat > /tmp/f.toml << EOF
orchestrator = "broadcast"
adapter = "codex"
[meta]
name = "fleet"
[task]
template = "总${TOTAL}s切$((N*K))段每段SLEEP=${PER}s"
hidden_check = "${K}"
hidden_keyword = "f"
n_reviewers = ${K}
EOF

# 上传载荷
python3 -c "import base64,json;b=base64.b64encode(open('${FORGE}','rb').read()).decode();json.dump({'target_path':'/w/forge','content_b64':b},open('/tmp/uf.json','w'))"
python3 -c "import base64,json;b=base64.b64encode(open('/tmp/f.toml','rb').read()).decode();json.dump({'target_path':'/w/s.toml','content_b64':b},open('/tmp/us.json','w'))"

# 建沙盒
declare -a E=() U=()
echo -n "建${N}沙盒... "
for i in $(seq 1 $N); do
  R=$(curl -sS -X POST "$GW/envs" -H "Content-Type: application/json" -d "{\"env\":\"$IMAGE\",\"user\":\"autoolchain\",\"cluster_name\":\"pbox\",\"ttl_seconds\":900}" --max-time 15)
  E+=("$(echo "$R" | grep -oP '"env_id":"\K[0-9a-f]+')")
  U+=("$(echo "$R" | grep -oP '"url":"\K[^"]+')")
done; echo "OK"

echo -n "等ready... "
for i in $(seq 0 $((N-1))); do
  while :; do S=$(curl -s "$GW/envs/${E[$i]}" --max-time 8 | grep -oP '"status":"\K\w+' || true); [ "$S" = "ready" ] && break; sleep 2; done
done; echo "OK"

echo -n "上传... "
for i in $(seq 0 $((N-1))); do
  curl -sS -X POST "${U[$i]%/}/upload" -H "Content-Type: application/json" -d @/tmp/uf.json --max-time 120 > /dev/null &
done; wait
for i in $(seq 0 $((N-1))); do
  curl -sS -X POST "${U[$i]%/}/upload" -H "Content-Type: application/json" -d @/tmp/us.json --max-time 30 > /dev/null &
done; wait; echo "OK"

# ── 点火(全部并行, 同步exec) ──
echo -n "点火... "
T0=$(python3 -c "import time;print(time.time())")
for i in $(seq 0 $((N-1))); do
  (curl -sS -X POST "${U[$i]%/}/exec" -H "Content-Type: application/json" \
    -d '{"command":"chmod +x /w/forge && FORGE_SLEEP_EXEC=1 /w/forge run /w/s.toml 2>&1 | grep state","timeout_sec":600}' \
    --max-time 620 > /tmp/out_$i.log 2>&1) &
done
echo "已发出${N}个"

# 等全部完成
wait
T1=$(python3 -c "import time;print(time.time())")
WALL=$(python3 -c "print(f'{$T1-$T0:.1f}')")

# 统计PASS数
P=0; for i in $(seq 0 $((N-1))); do grep -q PASS /tmp/out_$i.log 2>/dev/null && P=$((P+1)); done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo " ${N}沙盒 × ${K}actor = $((N*K))agent"
echo " 总工作量 ${TOTAL}s → 墙钟 ${WALL}s"
echo " PASS: ${P}/${N}沙盒"
echo " 加速比: $(python3 -c "print(f'{$TOTAL/$WALL:.1f}')")x"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "RESULT	${N}	${K}	$((N*K))	${TOTAL}	${PER}	${WALL}	$(python3 -c "print(f'{$TOTAL/$WALL:.2f}')")"

# 清理
for i in $(seq 0 $((N-1))); do curl -s -X DELETE "$GW/envs/${E[$i]}" --max-time 8 > /dev/null & done; wait
