#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""replay.py — 账本回放：时间线、阶段、统计
用法: python3 replay.py datasets/ledger-sample.json [--full]
"""
import argparse, json, sys

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--full", action="store_true", help="打印完整信件正文")
    a = ap.parse_args()

    try:
        letters = json.load(open(a.file, encoding="utf-8"))
    except Exception as e:
        print(f"FAIL load {a.file}: {e}")
        sys.exit(1)

    print(f"== 账本回放：{a.file}（{len(letters)} 封信）==\n")
    actors = {}
    for l in letters:
        seq, frm, to, body = l.get("seq", "?"), l.get("from", "?"), l.get("to", "?"), str(l.get("body", ""))
        actors[frm] = actors.get(frm, 0) + 1
        head = body.replace("\n", " ")[:70]
        print(f"#{seq:<3} {frm:>8} -> {to:<8} | {head}")
        if a.full:
            print("        " + body[:500].replace("\n", "\n        "))

    print("\n== 发信统计 ==")
    for k, v in sorted(actors.items(), key=lambda x: -x[1]):
        print(f"  {k}: {v}")

    # 简单归类（按关键词）
    cats = {"ISSUE": 0, "审计/方案": 0, "FIX": 0, "REVIEW/CONCERNS": 0, "终验/自愈": 0, "ECHO": 0}
    for l in letters:
        b = str(l.get("body", ""))
        if b.startswith("ISSUE"):
            cats["ISSUE"] += 1
        elif "FIX-TASK" in b:
            cats["FIX"] += 1
        elif "REVIEW" in b or "CONCERNS" in b:
            cats["REVIEW/CONCERNS"] += 1
        elif "终验" in b or "自愈" in b or "重修" in b:
            cats["终验/自愈"] += 1
        elif "ECHO" in b:
            cats["ECHO"] += 1
        else:
            cats["审计/方案"] += 1
    print("\n== 类别 ==")
    for k, v in cats.items():
        if v:
            print(f"  {k}: {v}")

if __name__ == "__main__":
    main()
