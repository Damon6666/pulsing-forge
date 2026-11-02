# -*- coding: utf-8 -*-
# 终表: compare.tsv 去重(同sid取末次) → 旧栈 vs Forge 对照
import sys
old = {
 "T1-V1-001":"FAIL","T1-V2-003":"FAIL","T1-V3-004":"FAIL","T1-V5-006":"FAIL",
 "T1-V6-007":"FAIL","T1-V9-010":"FAIL","T2-V3-021":"FAIL","T2-V4-022":"FAIL",
 "T2-V5-023":"FAIL","T2-V6-024":"FAIL","T2-V9-027":"FAIL","T3-V1-028":"FAIL",
 "T3-V2-030":"FAIL","T3-V3-031":"FAIL","T3-V5-033":"FAIL","T5-V1-048":"DEGENERATE",
 "T6-V3-057":"FAIL","T7-V8-070":"FAIL","T8-V8-079":"DEGENERATE","T9-V5-086":"FAIL",
 "T10-V1-011":"FAIL","T10-V10-012":"FAIL","T10-V2-013":"FAIL","T10-V3-014":"FAIL",
 "T10-V6-015":"FAIL","T10-V8-016":"FAIL","T10-V9-017":"FAIL",
}
rows = {}
for line in open('/mnt/d/tmp/forge-compare/compare.tsv'):
    r = line.rstrip('\n').split('\t')
    if len(r) >= 3:
        rows[r[0]] = r  # 同sid后写覆盖前写 = 取末次
order = ["T1-V1-001","T1-V2-003","T1-V3-004","T1-V5-006","T1-V6-007","T1-V9-010",
         "T2-V3-021","T2-V4-022","T2-V5-023","T2-V6-024","T2-V9-027",
         "T3-V1-028","T3-V2-030","T3-V3-031","T3-V5-033",
         "T5-V1-048","T6-V3-057","T7-V8-070","T8-V8-079","T9-V5-086",
         "T10-V1-011","T10-V2-013","T10-V3-014","T10-V6-015","T10-V8-016","T10-V9-017","T10-V10-012"]
p = 0
print(f"{'场景':<13}{'旧栈':<12}{'Forge':<8} 细节")
for sid in order:
    if sid not in rows:
        print(f"{sid:<13}{old[sid]:<12}{'未跑':<8}")
        continue
    r = rows[sid]
    verdict = r[2]
    detail = r[3][:52] if len(r) > 3 else ''
    p += (verdict == 'PASS')
    print(f"{sid:<13}{old[sid]:<12}{verdict:<8} {detail}")
print(f"\nForge 原题重考: {p}/{len(order)} PASS (旧栈 0/27)")
