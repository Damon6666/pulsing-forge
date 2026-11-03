#!/usr/bin/env bash
# 二冲自动复跑: 等额度窗口重置(11:14:54)后自触发; key运行时从/tmp/babysit-4015.sh读取, 不落文件
set -u
TARGET=$(date -d "today 11:15" +%s)
while [ "$(date +%s)" -lt "$TARGET" ]; do sleep 60; done
KEY=$(grep -oP 'API_KEY=\K[^ ]+' /tmp/babysit-4015.sh | head -1)
echo "[$(date '+%H:%M:%S')] 额度验证..."
for i in 1 2 3; do
  R=$(curl -s -X POST http://127.0.0.1:4015/v1/responses -H "Content-Type: application/json" \
      -H "Authorization: Bearer $KEY" -d '{"model":"glm-5-turbo","input":"只回ok"}' --max-time 30 | head -c 200)
  echo "$R" | grep -q 1308 || break
  echo "[$(date '+%H:%M:%S')] 仍限流, 等10分钟重试($i/3)"; sleep 600
done
echo "$R" | grep -q 1308 && { echo "额度未恢复, 放弃"; exit 1; }
echo "[$(date '+%H:%M:%S')] 额度OK, 开跑二冲44场景"
cd /mnt/c/Users/daimeng/.zcode/workspace/default/forge
export FORGE_API_KEY="$KEY" FORGE_BASE_URL=http://127.0.0.1:4015/v1
OUT=/mnt/d/tmp/forge-compare/wave2.tsv
: > "$OUT"
guard_port() { local pid=$(ss -tlnp 2>/dev/null | grep ':12410' | grep -oP 'pid=\K[0-9]+' | head -1); [ -n "$pid" ] && { kill "$pid" 2>/dev/null; sleep 3; }; return 0; }
RETRY='T1-V1-001 T1-V10-002 T1-V4-005 T1-V5-006 T1-V9-010 T2-V1-018 T2-V2-020 T2-V9-027 T2-数值区间和-SS1 T2-数值区间和-SS2 T2-数值区间和-SS3 T3-V3-031 T3-V4-032 T3-V9-037 T4-V1-038 T4-V2-040 T4-V3-041 T4-V4-042 T4-V5-043 T4-数值区间和-SS10 T5-V1-048 T5-V2-050 T5-V5-051 T5-V9-053 T5-数值区间和-SS4 T6-V1-054 T6-V7-061 T6-V9-063 T7-V3-065 T7-V4-066 T7-V5-067 T8-V1-071 T8-V2-073 T8-V3-074 T8-V4-075 T8-V5-076 T8-V9-080 T9-V1-081 T9-V3-084 T9-V4-085 T9-V5-086 T9-数值区间和-SS8 SA2-recall-chain'
for sid in $RETRY; do
  tmo=900
  guard_port
  echo "[$(date '+%H:%M:%S')] ===== $sid ====="
  if [ "$sid" = "SA2-recall-chain" ]; then toml=scenarios/sa/SA2-recall-chain.toml; else toml=scenarios/full100/$sid.toml; fi
  out=$(FORGE_TIMEOUT=$tmo timeout $((tmo+90)) ./target/release/forge run "$toml" 2>/dev/null)
  state=$(echo "$out" | grep -oP 'state: \K\S+' | head -1)
  turns=$(echo "$out" | grep -oP 'turns: \K\d+' | head -1)
  rund=$(ls -t runs | grep "^$sid" | head -1)
  verdict="-" detail=""
  if [ -n "$rund" ]; then
    if [ "$sid" = "SA2-recall-chain" ]; then
      read -r verdict detail <<< "$(python3 compare/check_sa.py "$sid" "runs/$rund" | tr '\t' ' ')"
    else
      read -r verdict detail <<< "$(python3 compare/check100.py "$sid" "runs/$rund" "${turns:-0}" | tr '\t' ' ')"
    fi
  fi
  echo -e "$sid\t${state:-NO-RUN}\t$verdict\tturns=${turns:-0}\t$detail" >> "$OUT"
  echo "$sid state=$state verdict=$verdict $detail"
done
p=$(grep -c PASS "$OUT")
echo "WAVE2-DONE 二冲PASS: $p/44"
