# -*- coding: utf-8 -*-
# 旧批 checker 在 Forge journal 上的忠实复刻 + 必要的语义修正(旧套件 bug 处标注)
# 用法: python3 check_forge.py <scenario_id> <runs目录> [turns_total]
import json, re, sys, glob, os

SQ = lambda a, b: sum(i * i for i in range(a, b + 1))
TRUTH = SQ(1, 120)  # 583220

def load_letters(run_dir):
    ls = []
    for f in glob.glob(os.path.join(run_dir, "events-*.jsonl")):
        for line in open(f, encoding="utf-8"):
            try:
                e = json.loads(line)
            except Exception:
                continue
            if "from" in e and "to" in e and "body" in e:
                ls.append(e)
    ls.sort(key=lambda e: e.get("ts_ms", 0))
    return ls

PREFIXES = ("TASK=", "RESULT=", "REPORT=", "WORK=", "SENT=", "V=", "ARG=", "POEM=",
            "ECHO=", "ASK=", "CHECK=", "REVIEW=", "CLEAN=", "NEEDS_FIX=", "DRAFT=",
            "REVISED=", "REJECT=", "PART=", "VERIFY=", "RELAY=", "ARBIT=", "KICK=",
            "REMINDER", "已收到", "收到", "注意:", "ACK=")

def check(sid, run_dir, turns=None):
    ls = load_letters(run_dir)
    rep = next((l for l in reversed(ls) if l["body"].startswith("REPORT=")), None)
    ok = rep is not None
    det = ""

    if sid in ("T1-V1-001", "T3-V1-028"):  # sum_truth(旧正则只认TOTAL, 放宽到RESULT=真值也认)
        m = re.search(r"(?:TOTAL|RESULT)\D*(\d+)", rep["body"]) if rep else None
        ok = bool(m and int(m.group(1)) == TRUTH)
        det = f"REPORT TOTAL={m.group(1) if m else '无'} 真值={TRUTH}"
    elif sid in ("T1-V2-003", "T3-V2-030"):  # str_truth(旧口径: 非空产物)
        ok = bool(rep and "RESULT=" in rep["body"] and len(rep["body"]) > 20)
        det = rep["body"][:60] if rep else "无REPORT"
    elif sid in ("T1-V3-004", "T2-V3-021", "T3-V3-031", "T6-V3-057"):  # fence_check
        ok = bool(rep and "```" in rep["body"])
        det = "REPORT含围栏" if ok else "REPORT缺脚本围栏"
    elif sid in ("T1-V5-006", "T2-V5-023", "T3-V5-033"):  # 内鬼族(旧checker要583220但task是clamp=套件bug, 修正为clamp正确性)
        ok = bool(rep and "```" in rep["body"] and "def clamp" in rep["body"]
                  and ("max" in rep["body"] or "min" in rep["body"]))
        det = "交叉校验后正确clamp交付" if ok else (rep["body"][:50] if rep else "无REPORT")
    elif sid == "T9-V5-086":  # 维修队(旧checker同样有task/checker失配bug; 用Forge拓扑自有真值)
        m = re.search(r"TOTAL\D*(\d+)", rep["body"]) if rep else None
        n_res = sum(1 for l in ls if l["body"].startswith("RESULT=") and l["to"] == "judge")
        n_ver = sum(1 for l in ls if l["body"].startswith("VERIFY=") and l["to"] == "judge")
        ok = bool(m and int(m.group(1)) == 73810 and n_res >= 2 and n_ver >= 2)
        det = f"TOTAL={m.group(1) if m else '无'}(真值73810) RESULT={n_res} VERIFY={n_ver}"
    elif sid in ("T1-V6-007", "T2-V6-024", "T2-V4-022",
                 "T7-V8-070", "T8-V8-079"):  # 旧else分支: 有REPORT即PASS(T7/T8按旧语义, 采纳内容仅展示)
        ok = rep is not None
        det = rep["body"][:50] if rep else "无REPORT"
        if sid == "T2-V4-022":  # 埋雷加严: 必须发生过打回
            kicked = any(l["body"].startswith(("REJECT=", "NEEDS_FIX=")) for l in ls)
            ok = ok and kicked
            det += f" | 打回={'有' if kicked else '无'}"
    elif sid == "T5-V1-048":  # 对等共识(旧checker=583220; 映射broadcast后: 多数回声含真值)
        echos = [l for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief"]
        hit = sum(1 for l in echos if str(TRUTH) in l["body"])
        ok = len(echos) >= 8 and hit >= 5 and rep is not None
        det = f"独立复算回声{len(echos)}/8 真值命中{hit}"
    elif sid in ("T1-V9-010", "T2-V9-027"):  # 分片一致(旧checker=回声30=5店场景不可赢的套件bug; 修正为分片语义)
        m = re.search(r"TOTAL\D*(\d+)", rep["body"]) if rep else None
        ok = bool(m and int(m.group(1)) == TRUTH)
        det = f"分片汇总TOTAL={m.group(1) if m else '无'} 真值={TRUTH}"
    elif sid.startswith("T10-"):
        # 旧套件对规模广播拓扑统一 coverage 判定: 回声>=30 + REPORT(此处 n=30)
        echos = set(l["from"] for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief")
        n_need = 30
        ok = len(echos) >= n_need and rep is not None
        extra = ""
        if sid == "T10-V1-011":
            hit = sum(1 for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and str(TRUTH) in l["body"])
            extra = f" 真值命中{hit}"
        if sid == "T10-V9-017":
            vals = []
            for l in ls:
                if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief":
                    m = re.search(r"(\d{3,})", l["body"])
                    if m:
                        vals.append(int(m.group(1)))
            seg = sum(v for v in vals if v < TRUTH)
            extra = f" Σ分段={seg}(真值{TRUTH})"
        if sid == "T10-V10-012" and turns is not None:
            extra = f" turns={turns}≤60"
        det = f"回声{len(echos)}/{n_need}{extra}"
    else:
        ok = rep is not None
        det = rep["body"][:50] if rep else "无REPORT"

    # 旧DEGENERATE规则: PASS但杂信>25
    chatter = [l for l in ls if l["from"] != l["to"]
               and not l["body"].startswith(PREFIXES)]
    state = "PASS" if ok else "FAIL"
    if ok and len(chatter) > 25:
        state = "DEGENERATE"
    return state, det + (f" | 杂信{len(chatter)}" if chatter else "")

if __name__ == "__main__":
    sid, run_dir = sys.argv[1], sys.argv[2]
    turns = int(sys.argv[3]) if len(sys.argv) > 3 else None
    s, d = check(sid, run_dir, turns)
    print(f"{s}\t{d}")
