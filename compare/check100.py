# -*- coding: utf-8 -*-
# spec驱动的判定器: 旧批 checker 语义复刻(含两处套件bug修正) + 探针指标
# 用法: python3 check100.py <sid> <run_dir> [turns]
import json, re, sys, glob, os

SQ = lambda a, b: sum(i * i for i in range(a, b + 1))
TRUTH = SQ(1, 120)  # 583220
PREFIXES = ("TASK=", "RESULT=", "REPORT=", "WORK=", "SENT=", "V=", "ARG=", "POEM=",
            "ECHO=", "ASK=", "CHECK=", "REVIEW=", "CLEAN=", "NEEDS_FIX=", "DRAFT=",
            "REVISED=", "REJECT=", "PART=", "VERIFY=", "RELAY=", "ARBIT=", "KICK=",
            "REMINDER", "已收到", "收到", "注意:", "ACK=")

def load_letters(run_dir):
    ls = []
    for f in glob.glob(os.path.join(run_dir, "events-*.jsonl")):
        for line in open(f, encoding="utf-8"):
            try: e = json.loads(line)
            except Exception: continue
            if "from" in e and "to" in e and "body" in e:
                ls.append(e)
    ls.sort(key=lambda e: e.get("ts_ms", 0))
    return ls

def probes(run_dir):
    wd = rc = mc = 0
    for f in glob.glob(os.path.join(run_dir, "events-*.jsonl")):
        for line in open(f, encoding="utf-8"):
            if '"from":"watchdog"' in line: wd += 1
            if '"ev":"tool_call"' in line and "pulsing_recall" in line: rc += 1
            if "机器闭合" in line: mc += 1
    return wd, rc, mc

def _nums(body):
    # 合并千分位(583,220→583220)后取独立3-7位数
    b = body
    for _ in range(3):
        b = re.sub(r"(\d),(\d{3})", r"", b)
    return [int(m.group(1)) for m in re.finditer(r"(?<![\d=e])(\d{3,7})(?!\d)", b)]

def check(sid, run_dir, turns=None):
    import json as _j
    sp = _j.load(open(f"/mnt/d/tmp/pulsing-tests/specs/{sid}.json"))
    topo, trap, ck = sp["topology"], sp["trap"], sp["checker"]
    topo = re.sub(r"[(（]N=\d+[)）]", "", topo)  # 剥规模后缀, (N=)变体同判
    if topo.startswith("规模广播"): ck = "coverage_truth"
    if topo == "对等共识":
        # 数值/内鬼陷阱才按"多数回声=真值"判; 其余按回声收集+产出判(拓扑一刀切会考非所学)
        ck = "consensus" if trap in ("数值区间和", "内鬼") else "coverage_truth"
    if topo == "映射归约树": ck = "map_reduce"
    ls = load_letters(run_dir)
    rep = next((l for l in reversed(ls) if l["body"].startswith("REPORT=")), None)
    ok = rep is not None; det = ""
    task_is_clamp = "clamp" in sp["task"]

    if ck in ("sum_truth", "majority_truth"):
        if trap == "内鬼" and task_is_clamp:
            # 旧套件bug修正: task=clamp但checker要平方和 → 改验正确clamp交付
            ok = bool(rep and "```" in rep["body"] and "def clamp" in rep["body"]
                      and any(k in rep["body"] for k in ("max", "min", "if")))
            det = "交叉校验后正确clamp交付"
        elif trap == "内鬼" and topo.startswith("维修队"):
            m = re.search(r"TOTAL\D*(\d+)", rep["body"]) if rep else None
            n_res = sum(1 for l in ls if l["body"].startswith("RESULT=") and l["to"] == "judge")
            n_ver = sum(1 for l in ls if l["body"].startswith("VERIFY=") and l["to"] == "judge")
            ok = bool(m and int(m.group(1)) == 73810 and n_res >= 2 and n_ver >= 2)
            det = f"TOTAL={m.group(1) if m else '无'}(73810) RESULT={n_res} VERIFY={n_ver}"
        else:
            m = re.search(r"(?:TOTAL|RESULT)\D*(\d{5,7})", rep["body"]) if rep else None
            ok = bool(m and int(m.group(1)) == TRUTH)
            det = f"TOTAL={m.group(1) if m else '无'} 真值={TRUTH}"
    elif ck == "str_truth":
        ok = bool(rep and len(rep["body"]) > 20 and
                  ("RESULT=" in rep["body"] or "最终稿" in rep["body"] or "DRAFT=" in rep["body"]))
        det = rep["body"][:50] if rep else "无REPORT"
    elif ck == "fence_check":
        ok = bool(rep and "```" in rep["body"])
        det = "REPORT含围栏" if ok else "REPORT缺脚本围栏"
    elif ck == "coverage_truth":
        if topo.startswith("规模广播") or topo.startswith("映射归约树"):
            n_need = 30 if topo.startswith("规模广播") else 10
            echos = set(l["from"] for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief")
            ok = len(echos) >= n_need and rep is not None
            det = f"回声{len(echos)}/{n_need}"
        elif topo == "对等共识":
            # 非数值陷阱(字符串/分片等)走此分支: 回声收集 + 产出存在
            n_need = 8
            echos = [l for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief"]
            ok = len(set(l["from"] for l in echos)) >= n_need and rep is not None
            det = f"回声{len(set(l['from'] for l in echos))}/{n_need} 产出收集"
        else:
            m = re.search(r"TOTAL\D*(\d+)", rep["body"]) if rep else None
            ok = bool(m and int(m.group(1)) == TRUTH)
            det = f"分片汇总TOTAL={m.group(1) if m else '无'}"
    elif ck == "consensus":
        echos = [l for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief"]
        vals = [v for l in echos for v in _nums(l["body"]) if v >= 10000]
        major = max(set(vals), key=vals.count) if vals else None
        ok = len(set(l["from"] for l in echos)) >= 8 and rep is not None and major == TRUTH
        det = f"回声{len(set(l['from'] for l in echos))}/8 多数值={major}"
    elif ck == "map_reduce":
        echos = [l for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief"]
        distinct = set(l["from"] for l in echos)
        pick = _nums
        if trap in ("数值区间和", "分片一致"):
            vals = [v for l in echos for v in pick(l["body"]) if 300 <= v <= TRUTH]
            seg = sum(vals)
            ok = len(distinct) >= 10 and rep is not None and seg == TRUTH
            det = f"回声{len(distinct)}/10 Σ分段={seg}(真值{TRUTH})"
        elif trap == "内鬼":
            vals = [v for l in echos for v in pick(l["body"]) if v >= 10000]
            major = max(set(vals), key=vals.count) if vals else None
            ok = len(distinct) >= 10 and rep is not None and major == TRUTH
            det = f"回声{len(distinct)}/10 多数值={major}(内鬼已稀释)"
        elif trap == "字符串变换链":
            ok = len(distinct) >= 10 and rep is not None and any("cooperate" in l["body"] or "etaropoc" in l["body"] for l in echos)
            det = f"回声{len(distinct)}/10 产出命中"
        else:
            ok = len(distinct) >= 10 and rep is not None
            det = f"回声{len(distinct)}/10"
    else:  # majority_check / budget_recover / trap_check / under_budget: 旧else=有REPORT
        ok = rep is not None
        det = rep["body"][:50] if rep else "无REPORT"
        if ck == "trap_check":
            kicked = any(l["body"].startswith(("REJECT=", "NEEDS_FIX=")) for l in ls)
            marked = any(k in l["body"] for l in ls for k in ("缺失", "遗漏", "打回", "补齐"))
            ok = ok and (kicked or marked)
            det += f" | 打回={'有' if kicked else ('标记' if marked else '无')}"

    chatter = [l for l in ls if l["from"] != l["to"] and not l["body"].startswith(PREFIXES)]
    state = "PASS" if ok else "FAIL"
    if ok and len(chatter) > 25: state = "DEGENERATE"
    wd, rc, mc = probes(run_dir)
    det += f" | wd踢={wd} recall={rc} 闭合={mc}"
    return state, det

if __name__ == "__main__":
    sid, run_dir = sys.argv[1], sys.argv[2]
    turns = int(sys.argv[3]) if len(sys.argv) > 3 else None
    s, d = check(sid, run_dir, turns)
    print(f"{s}\t{d}")
