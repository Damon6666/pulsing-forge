#!/bin/bash
# cell-hub.sh <K> — 中央沙盒: TideHub(0.0.0.0) + chief codex + mini_proxy
K=$1
KEY="${BIGMODEL_API_KEY:?set-me}"
REMOTE=$(seq -s, 1 $K | sed 's/^/e/; s/,/,e/g')

cat > /w/s.toml << EOF
orchestrator = "broadcast"
adapter = "codex"
[meta]
name = "cell-demo"
[task]
template = "跨沙盒并行sleep演示: ${K}个远程worker。worker规程(由worker自行drain执行): 收到任务信后用shell真实执行一次 sleep 15 秒, 完成后立即pulsing_send给chief, 内容 ECHO=<店名>|slept (只发一次)。chief规程: 广播任务, 收齐${K}个不同店的ECHO后立即REPORT收口。"
hidden_check = "${K}"
hidden_keyword = "slept"
n_reviewers = ${K}
EOF

pkill -f "python3 /w/mini_proxy.py" 2>/dev/null; sleep 1
BIGMODEL_API_KEY="$KEY" nohup python3 /w/mini_proxy.py > /tmp/mini.log 2>&1 &
sleep 1.5
chmod +x /w/forge
cd /w
FORGE_API_KEY="$KEY" \
FORGE_BASE_URL="http://127.0.0.1:4020/v1" \
FORGE_CODEX_BIN=/usr/local/bin/codex \
FORGE_BIND=0.0.0.0 \
FORGE_REMOTE_SHOPS="$REMOTE" \
FORGE_TIMEOUT=420 \
nohup /w/forge run /w/s.toml > /w/hub.out 2>&1 &
echo "hub-started pid=$! K=$K remote=$REMOTE"
