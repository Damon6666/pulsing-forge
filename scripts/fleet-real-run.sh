#!/bin/bash
# real-run2.sh <K> <PER> <TIMEOUT> — 幂等worker版
K=$1; PER=$2; TMO=$3
KEY="${BIGMODEL_API_KEY:?set-me}"

cat > /w/s.toml << EOF
orchestrator = "broadcast"
adapter = "codex"
[meta]
name = "fleet-real2"
[task]
template = "并行sleep演示(幂等): ${K}个worker。worker规程: ①收到任务信后用shell真实执行一次 sleep ${PER} 秒(只执行这一次) ②睡醒后立即用 pulsing_send 给 chief 发一封信 ECHO=<自己的店名>|slept (只发这一次) ③之后无论再收到什么信, 都不要再睡、不要再发ECHO、不要再报名, 直接结束回合保持安静。chief规程: 广播任务, 收齐${K}个不同店的ECHO后立即REPORT收口。"
hidden_check = "${K}"
hidden_keyword = "slept"
n_reviewers = ${K}
EOF

pkill -f "python3 /w/mini_proxy.py" 2>/dev/null
sleep 1
BIGMODEL_API_KEY="$KEY" nohup python3 /w/mini_proxy.py > /tmp/mini.log 2>&1 &
sleep 1.5
chmod +x /w/forge
cd /w
FORGE_API_KEY="$KEY" \
FORGE_BASE_URL="http://127.0.0.1:4020/v1" \
FORGE_CODEX_BIN=/usr/local/bin/codex \
FORGE_TIMEOUT=$TMO \
/w/forge run /w/s.toml > /tmp/forge.out 2>&1
grep -E "state:|终局|超时|letters:|turns:|duration:" /tmp/forge.out
echo "REQS=$(grep -c '^REQ' /tmp/mini.log)"
echo "ECHODIST=$(grep -ho 'ECHO=e[0-9]*|slept' /w/runs/fleet-real2-*/events-*.jsonl 2>/dev/null | sort -u | wc -l)"
