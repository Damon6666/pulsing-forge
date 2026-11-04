#!/usr/bin/env bash
# forge fleet v2 — 修正版: 同步exec点火 + 并行轮询
set -eu
TOTAL=${1:-60}
N=${2:-1}
K=${3:-5}
IMAGE=${4:-"registry.h.pjlab.org.cn/ailab-sys/pulsing_forge:v1"}
GATEWAY="http://env-gateway.ailab.ailab.ai"
FORGE_BIN="/mnt/c/Users/daimeng/.zcode/workspace/default/forge/target/release/forge"

PER=$(( TOTAL / (N * K) ))
[ "$PER" -lt 1 ] && { echo "错误: TOTAL/(N*K)=$PER <1"; exit 1; }

echo "═══ Forge Fleet v2 ═══"
echo "任务: sleep ${TOTAL}s → done | ${N}沙盒 × ${K}actor = $((N*K))agent, 每agent ${PER}s"

# 场景 TOML
SCEN="/tmp/fleet-${N}x${K}.toml"
cat > "$SCEN" << EOF
orchestrator = "broadcast"
adapter = "codex"
[meta]
name = "fleet"
[task]
template = "Fleet基准: 总睡眠${TOTAL}s切成$((N*K))段, 每段SLEEP=${PER}s, 每店真实sleep后回ECHO"
hidden_check = "${K}/${K}"
hidden_keyword = "fleet"
n_reviewers = ${K}
EOF

# 上传载荷
python3 -c "
import base64,json
b = base64.b64encode(open('${FORGE_BIN}','rb').read()).decode()
json.dump({'target_path':'/w/forge','content_b64':b}, open('/tmp/up_f.json','w'))
"
python3 -c "
import base64,json
b = base64.b64encode(open('${SCEN}','rb').read()).decode()
json.dump({'target_path':'/w/s.toml','content_b64':b}, open('/tmp/up_s.json','w'))
"

# ── 创建沙盒 ──
declare -a EIDS=() URLS=()
echo -n "[1] 建${N}沙盒... "
T0=$(date +%s.%N)
for i in $(seq 1 "$N"); do
  R=$(curl -sS -X POST "$GATEWAY/envs" -H "Content-Type: application/json" \
    -d "{\"env\":\"$IMAGE\",\"user\":\"autoolchain\",\"cluster_name\":\"pbox\",\"ttl_seconds\":900}" --max-time 15)
  EID=$(echo "$R" | grep -oP '"env_id":"\K[0-9a-f]+')
  URL=$(echo "$R" | grep -oP '"url":"\K[^"]+')
  [ -z "$EID" ] && { echo "沙盒$i失败"; exit 1; }
  EIDS+=("$EID"); URLS+=("$URL")
done
echo "OK ($(python3 -c "print(f'{$(date +%s.%N)-$T0:.1f}')")s)"

# ── 等 ready ──
echo -n "[2] 等ready... "
for idx in $(seq 0 $((N-1))); do
  for j in $(seq 1 60); do
    S=$(curl -s "$GATEWAY/envs/${EIDS[$idx]}" --max-time 8 | grep -oP '"status":"\K\w+' | head -1)
    [ "$S" = "ready" ] && break
    sleep 2
  done
done
echo "OK"

# ── 上传(并行) ──
echo -n "[3] 上传... "
for idx in $(seq 0 $((N-1))); do
  curl -sS -X POST "${URLS[$idx]}%/upload" -H "Content-Type: application/json" -d @/tmp/up_f.json --max-time 120 > /dev/null &
done
wait
for idx in $(seq 0 $((N-1))); do
  curl -sS -X POST "${URLS[$idx]}%/upload" -H "Content-Type: application/json" -d @/tmp/up_s.json --max-time 30 > /dev/null &
done
wait
echo "OK"

# ── 点火(同步exec, 并行发出) ──
echo "[4] 点火 ${N}沙盒×${K}actor... "
T3=$(date +%s.%N)
for idx in $(seq 0 $((N-1))); do
  (
    curl -sS -X POST "${URLS[$idx]}%/exec" -H "Content-Type: application/json" \
      -d '{"command":"chmod +x /w/forge && cd /w && FORGE_SLEEP_EXEC=1 FORGE_TIMEOUT=600 /w/forge run /w/s.toml 2>&1 | tail -5 > /w/done.txt","timeout_sec":600}' \
      --max-time 620 > /dev/null 2>&1
  ) &
done
# 等全部点火请求发出(不等完成)
sleep 2
echo "  全部已发出, 计时..."

# ── 轮询(并行检查, 检查done.txt非空) ──
DONE=0
for poll in $(seq 1 240); do
  sleep 3
  DONE=0
  PIDS=()
  for idx in $(seq 0 $((N-1))); do
    (
      R=$(curl -sS -X POST "${URLS[$idx]}%/exec" -H "Content-Type: application/json" \
        -d '{"command":"test -s /w/done.txt && echo Y || echo N","timeout_sec":8}' \
        --max-time 15 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get('stdout','N').strip()[-1])" 2>/dev/null || echo N)
      [ "$R" = "Y" ] && touch "/tmp/done_${idx}"
    ) &
    PIDS+=($!)
  done
  wait
  DONE=$(ls /tmp/done_[0-9]* 2>/dev/null | wc -l)
  if [ "$DONE" -ge "$N" ]; then break; fi
  [ $((poll % 10)) -eq 0 ] && echo "  $DONE/$N 完成 (${poll}x3s)"
done
T_TOTAL=$(python3 -c "print(f'{$(date +%s.%N)-$T3:.1f}')")
rm -f /tmp/done_[0-9]*

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  配置:       ${N}沙盒×${K}actor=$((N*K))agent"
echo "  总工作量:   ${TOTAL}s"
echo "  任务耗时:   ${T_TOTAL}s"
echo "  理论耗时:   ${PER}s(+开销)"
echo "  单agent基线: ${TOTAL}s"
echo "  加速比:     $(python3 -c "print(f'{$TOTAL/$T_TOTAL:.1f}')")x"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "RESULT: ${N} ${K} $((N*K)) ${TOTAL} ${PER} ${T_TOTAL} $(python3 -c "print(f'{$TOTAL/$T_TOTAL:.2f}')")"

# 清理(后台)
for idx in $(seq 0 $((N-1))); do
  curl -s -X DELETE "$GATEWAY/envs/${EIDS[$idx]}" --max-time 8 > /dev/null &
done
wait
