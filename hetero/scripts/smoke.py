#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""smoke.py — 冒烟：健康检查 + 8 个身份可达性
用法: python3 smoke.py --hub 127.0.0.1:12410 [--shops chief,e1,...,e8]
"""
import argparse, json, socket, sys

def hub_call(addr, tool, args, timeout=10):
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
    ap.add_argument("--shops", default="chief,e1,e2,e3,e4,e5,e6,e7,e8")
    a = ap.parse_args()

    try:
        h = hub_call(a.hub, "pulsing_health", {})
    except Exception as e:
        print(f"FAIL hub unreachable: {e}")
        sys.exit(1)
    print("health:", h[:160])

    shops = [s.strip() for s in a.shops.split(",") if s.strip()]
    ok = 0
    for s in shops:
        try:
            r = hub_call(a.hub, "pulsing_send",
                         {"to": s, "message": "SMOKE-PING", "_shop": "smoke"})
            good = '"ok":true' in r or '"ok": true' in r
            print(f"  {s}: {'OK' if good else r[:80]}")
            ok += good
        except Exception as e:
            print(f"  {s}: ERR {e}")
    print(f"RESULT {ok}/{len(shops)} reachable")
    sys.exit(0 if ok == len(shops) else 1)

if __name__ == "__main__":
    main()
