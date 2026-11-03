# -*- coding: utf-8 -*-
# Super-agent 挑战场景判定
import json, re, sys, glob, os

def load(run_dir):
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
    wd = rc = 0
    for f in glob.glob(os.path.join(run_dir, "events-*.jsonl")):
        for line in open(f, encoding="utf-8"):
            if '"from":"watchdog"' in line: wd += 1
            if '"ev":"tool_call"' in line and "pulsing_recall" in line: rc += 1
    return wd, rc

def check(sid, run_dir):
    ls = load(run_dir)
    rep = next((l for l in reversed(ls) if l["body"].startswith("REPORT=")), None)
    wd, rc = probes(run_dir)
    if sid == "SA1-noise-admission":
        echos = set(l["from"] for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief")
        ok = len(echos) >= 20 and rep is not None
        return ("PASS" if ok else "FAIL"), f"回声{len(echos)}/20 noise5家 | wd踢={wd}"
    if sid == "SA2-recall-chain":
        ok = bool(rep and "FORGE-7749" in rep["body"]) and rc >= 1
        secret = "FORGE-7749" in (rep["body"] if rep else "")
        return ("PASS" if ok else "FAIL"), f"密令命中={secret} recall={rc}"
    if sid == "SA3-pingpong-resilience":
        ok = bool(rep and "```" in rep["body"]) and wd >= 1
        fence = bool(rep and "```" in rep["body"])
        return ("PASS" if ok else "FAIL"), f"围栏={fence} wd踢={wd}(敌对店应被踢)"
    if sid == "SA4-storm-batching":
        shops = set(l["from"] for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief")
        n_echo = sum(1 for l in ls if l["body"].startswith("ECHO=") and l["to"] == "chief" and l["from"] != "chief")
        chief_turn_letters = sum(1 for l in ls if l["from"] == "chief")
        ok = len(shops) >= 30 and rep is not None
        return ("PASS" if ok else "FAIL"), f"店{len(shops)}/30 扇入信{n_echo} | chief发信{chief_turn_letters}"
    return "FAIL", "未知场景"

if __name__ == "__main__":
    s, d = check(sys.argv[1], sys.argv[2])
    print(f"{s}\t{d}")
