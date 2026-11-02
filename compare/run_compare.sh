#!/usr/bin/env bash
# 原题重考批: 27个旧FAIL场景 → Forge 真 LLM 重跑 → 旧checker复刻判定 → 对比表
set -u
cd /mnt/c/Users/daimeng/.zcode/workspace/default/forge
export FORGE_API_KEY=f6434600b75f40109f0f178c2acfd12f.oL8O5g46t8kwZIGP
export FORGE_BASE_URL=http://127.0.0.1:4015/v1
OUT=/mnt/d/tmp/forge-compare
mkdir -p "$OUT"
SMALL="T1-V1-001 T1-V2-003 T1-V3-004 T1-V5-006 T1-V6-007 T1-V9-010 T2-V3-021 T2-V4-022 T2-V5-023 T2-V6-024 T2-V9-027 T3-V1-028 T3-V2-030 T3-V3-031 T3-V5-033 T5-V1-048 T6-V3-057 T7-V8-070 T8-V8-079 T9-V5-086"
BIG="T10-V1-011 T10-V2-013 T10-V3-014 T10-V6-015 T10-V8-016 T10-V9-017 T10-V10-012"

run_one() {
  local sid=$1 tmo=$2
  echo "[$(date '+%H:%M:%S')] ===== $sid ====="
  local out
  out=$(FORGE_TIMEOUT=$tmo timeout $((tmo+60)) ./target/release/forge run "scenarios/compare/$sid.toml" 2>/dev/null)
  local state=$(echo "$out" | grep -oP 'state: \K\S+' | head -1)
  local turns=$(echo "$out" | grep -oP 'turns: \K\d+' | head -1)
  local rund=$(ls -t runs | grep "^$sid" | head -1)
  local verdict
  verdict=$(python3 compare/check_forge.py "$sid" "runs/$rund" "${turns:-0}")
  echo -e "$sid\t${state:-NO-RUN}\t$verdict" >> "$OUT/compare.tsv"
  echo "$sid forge-state=$state checker=$verdict"
}

> "$OUT/compare.tsv"
for sid in $SMALL; do run_one "$sid" 300; done
for sid in $BIG;   do run_one "$sid" 600; done

echo "===== 最终对比 ====="
python3 - <<'EOF'
old = {
 "T1-V1-001":"FAIL","T1-V2-003":"FAIL","T1-V3-004":"FAIL","T1-V5-006":"FAIL",
 "T1-V6-007":"FAIL","T1-V9-010":"FAIL","T2-V3-021":"FAIL","T2-V4-022":"FAIL",
 "T2-V5-023":"FAIL","T2-V6-024":"FAIL","T2-V9-027":"FAIL","T3-V1-028":"FAIL",
 "T3-V2-030":"FAIL","T3-V3-031":"FAIL","T3-V5-033":"FAIL","T5-V1-048":"DEGENERATE",
 "T6-V3-057":"FAIL","T7-V8-070":"FAIL","T8-V8-079":"DEGENERATE","T9-V5-086":"FAIL",
 "T10-V1-011":"FAIL","T10-V10-012":"FAIL","T10-V2-013":"FAIL","T10-V3-014":"FAIL",
 "T10-V6-015":"FAIL","T10-V8-016":"FAIL","T10-V9-017":"FAIL",
}
rows = [l.rstrip("\n").split("\t") for l in open("/mnt/d/tmp/forge-compare/compare.tsv")]
w = max(len(r[0]) for r in rows)
p = sum(1 for r in rows if r[2] == "PASS")
print(f"{'场景':<{w}}  旧栈        Forge")
for r in rows:
    print(f"{r[0]:<{w}}  {old.get(r[0],'?'):<11} {r[2]}  {r[3][:60]}")
print(f"\nForge 复考: {p}/{len(rows)} PASS")
EOF
