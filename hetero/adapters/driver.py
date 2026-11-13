#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""driver.py — 外部驱动：注入 ISSUE、派发审查任务、收集回执
用法: python3 driver.py --hub 127.0.0.1:12410 [--issue issue.md] [--timeout 1800]
"""
import argparse, json, socket, sys, time

def hub_call(addr, tool, args, timeout=30):
    host, port = addr.split(":")
    s = socket.create_connection((host, int(port)), timeout=timeout)
    try:
        s.sendall((json.dumps({"tool": tool, "args": args}) + "\n").encode())
        b = b""
        while not b.endswith(b"\n"):
            c = s.recv(65536)
            if not c:
                break
            b += c
        return b.decode(errors="replace").strip()
    finally:
        s.close()

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--hub", default="127.0.0.1:12410")
    ap.add_argument("--issue", default="demo/issue.md")
    ap.add_argument("--timeout", type=int, default=1800)
    ap.add_argument("--review-verdict", default="", help="若提供, 直接给 e1 派发 REVIEW-TASK")
    a = ap.parse_args()

    try:
        issue = open(a.issue, encoding="utf-8").read()
    except FileNotFoundError:
        issue = "ISSUE: 公开仓库泄露内部拓扑（细节见 demo/issue-repair.toml）"

    t0 = time.time()
    print(f"[driver] 注入 ISSUE -> chief ({len(issue)} chars)")
    r = hub_call(a.hub, "pulsing_send",
                 {"to": "chief", "message": "ISSUE\n" + issue, "_shop": "driver"})
    print("[driver] send:", r[:120])

    if a.review_verdict:
        print("[driver] 派发 REVIEW-TASK -> e1")
        hub_call(a.hub, "pulsing_send",
                 {"to": "e1", "message": f"REVIEW-TASK\n{a.review_verdict}", "_shop": "driver"})

    deadline = time.time() + a.timeout
    echoes = {}
    while time.time() < deadline:
        raw = hub_call(a.hub, "pulsing_drain_mail", {"_shop": "driver"})
        try:
            res = json.loads(raw).get("result", "")
        except Exception:
            res = ""
        for line in res.split("\n"):
            if line.startswith("ECHO=") or "ECHO=" in line:
                echoes[line.strip()] = True
        # 从 chief 回执侧观察也可另行扩展；此处只打印 driver 收到的信件
        time.sleep(2)

    print(f"[driver] done, {len(echoes)} echoes observed, wall={int(time.time()-t0)}s")

if __name__ == "__main__":
    main()
