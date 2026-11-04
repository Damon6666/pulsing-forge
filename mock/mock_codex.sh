#!/usr/bin/env bash
# mock codex v3 — 带调试日志(写stderr, forge会落账)
set -eu

SHOP=$(basename "${CODEX_HOME:-unknown}" | sed 's/codex-home-//')
CTRL="${FORGE_CTRL:-12410}"
NSHOPS="${FORGE_MOCK_SHOPS:-5}"

# prompt = 最后一个非 -- 开头的参数
PROMPT=""
for arg in "$@"; do
  case "$arg" in --*) ;; -*) ;; *) PROMPT="$arg";; esac
done

# SLEEP=N (只取第一个匹配, 去掉换行)
SECS=$(echo "$PROMPT" | grep -oP 'SLEEP=\K\d+' | head -1 | tr -d '[:space:]')
[ -z "$SECS" ] && SECS=1

echo "MOCK_CODEX: shop=$SHOP ctrl=$CTRL nshops=$NSHOPS secs=$SECS prompt_len=${#PROMPT}" >&2

if [ "$SHOP" = "chief" ]; then
  # chief: 判断是点火(广播)还是收尾(ARBIT)
  if echo "$PROMPT" | grep -q "ARBIT"; then
    # ARBIT: 机器闭合 → REPORT
    exec python3 -c "
import socket, json
s = socket.socket(); s.settimeout(10)
s.connect(('127.0.0.1', $CTRL))
msg = json.dumps({'tool':'pulsing_send','args':{'_shop':'chief','to':'chief','message':'REPORT=COUNT=done||mock闭合','silent':True}})
s.sendall((msg + '\n').encode())
s.recv(4096); s.close()
"
  else
    # 点火: 广播到全部echo店
    exec python3 -c "
import socket, json
s = socket.socket(); s.settimeout(10)
s.connect(('127.0.0.1', $CTRL))
shops = [f'e{i}' for i in range(1, $NSHOPS + 1)]
msg = json.dumps({'tool':'pulsing_broadcast','args':{'_shop':'chief','to_list':shops,'message':'ECHO=报名||任务:SLEEP=$SECS','silent':False}})
s.sendall((msg + '\n').encode())
s.recv(4096); s.close()
"
  fi
else
  # echo店: 真实sleep → 发ECHO信
  sleep "$SECS"
  exec python3 -c "
import socket, json
s = socket.socket(); s.settimeout(10)
s.connect(('127.0.0.1', $CTRL))
msg = json.dumps({'tool':'pulsing_send','args':{'_shop':'$SHOP','to':'chief','message':'ECHO=$SHOP|slept=${SECS}s','silent':True}})
s.sendall((msg + '\n').encode())
s.recv(4096); s.close()
"
fi
