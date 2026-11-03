#!/usr/bin/env bash
# 104场景总战役: 全量100原题重考 + 4个Super-agent挑战
set -u
cd /mnt/c/Users/daimeng/.zcode/workspace/default/forge
export FORGE_API_KEY="${FORGE_API_KEY:?请通过环境变量传入key}"
export FORGE_BASE_URL=http://127.0.0.1:4015/v1
OUT=/mnt/d/tmp/forge-compare/campaign100.tsv
: > "$OUT"

guard_port() {  # 端口预清场: 杀掉12410的陈旧占用者
  local pid=$(ss -tlnp 2>/dev/null | grep ':12410' | grep -oP 'pid=\K[0-9]+' | head -1)
  if [ -n "$pid" ]; then echo "  [清场] 杀陈旧占用 pid=$pid"; kill "$pid" 2>/dev/null; sleep 3; fi
}

run_one() {
  local sid=$1 tmo=$2 toml=$3 checker=$4
  guard_port
  echo "[$(date '+%H:%M:%S')] ===== $sid ====="
  local out
  out=$(FORGE_TIMEOUT=$tmo timeout $((tmo+90)) ./target/release/forge run "$toml" 2>/dev/null)
  local state=$(echo "$out" | grep -oP 'state: \K\S+' | head -1)
  local turns=$(echo "$out" | grep -oP 'turns: \K\d+' | head -1)
  local rund=$(ls -t runs | grep "^$sid" | head -1)
  local verdict="-" detail=""
  if [ -n "$rund" ] && [ -d "runs/$rund" ]; then
    if [ "$checker" = "sa" ]; then
      read -r verdict detail <<< "$(python3 compare/check_sa.py "$sid" "runs/$rund" | tr '\t' ' ')"
    else
      read -r verdict detail <<< "$(python3 compare/check100.py "$sid" "runs/$rund" "${turns:-0}" | tr '\t' ' ')"
    fi
  fi
  echo -e "$sid\t${state:-NO-RUN}\t$verdict\tturns=${turns:-0}\t$detail" >> "$OUT"
  echo "$sid state=$state verdict=$verdict $detail"
}

# ── 全量100(拓扑序) ──
SIDS=$(ls scenarios/full100 | sed 's/\.toml$//' | sort)
for sid in $SIDS; do
  tmo=900
  case $sid in T10-*) tmo=600;; esac
  run_one "$sid" "$tmo" "scenarios/full100/$sid.toml" "c100"
done
# ── 4个Super-agent挑战 ──
run_one SA1-noise-admission 900 scenarios/sa/SA1-noise-admission.toml sa
run_one SA2-recall-chain    900 scenarios/sa/SA2-recall-chain.toml    sa
run_one SA3-pingpong-resilience 900 scenarios/sa/SA3-pingpong-resilience.toml sa
run_one SA4-storm-batching  600 scenarios/sa/SA4-storm-batching.toml  sa

echo CAMPAIGN-DONE
python3 - <<'EOF'
rows = [l.rstrip("\n").split("\t") for l in open("/mnt/d/tmp/forge-compare/campaign100.tsv")]
p = sum(1 for r in rows if len(r) > 2 and r[2] == "PASS")
f = sum(1 for r in rows if len(r) > 2 and r[2] == "FAIL")
d = sum(1 for r in rows if len(r) > 2 and r[2] == "DEGENERATE")
print(f"总战绩: {p} PASS / {f} FAIL / {d} DEGENERATE (共{len(rows)})")
sa = [r for r in rows if r[0].startswith("SA")]
print("SA挑战:", " ".join(f"{r[0][-6:]}={r[2]}" for r in sa))
EOF