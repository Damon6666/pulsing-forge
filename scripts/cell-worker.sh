#!/bin/bash
# cell-worker.sh <SHOP> <HUB_ADDR> — 工人沙盒: 轮询 drain + 执行 + 回信
SHOP=$1
HUB=$2
KEY="${BIGMODEL_API_KEY:?set-me}"

pkill -f "python3 /w/mini_proxy.py" 2>/dev/null; sleep 1
BIGMODEL_API_KEY="$KEY" nohup python3 /w/mini_proxy.py > /tmp/mini.log 2>&1 &
sleep 1.5
chmod +x /w/forge

H=/tmp/cell-home
mkdir -p $H
cat > $H/config.toml << EOF
model = "glm-5-turbo"
model_provider = "pulsing-relay"

[model_providers.pulsing-relay]
name = "pulsing-relay"
base_url = "http://127.0.0.1:4020/v1"
env_key = "PULSING_API_KEY"
wire_api = "responses"

[mcp_servers.pulsing]
command = "/w/forge"
args = ["attach", "--control", "${HUB}", "--shop", "${SHOP}"]
startup_timeout_sec = 60
EOF

export PULSING_API_KEY="$KEY"
for i in $(seq 1 40); do
  CODEX_HOME=$H timeout 180 codex exec --skip-git-repo-check --ephemeral \
    --dangerously-bypass-approvals-and-sandbox \
    "你是分布式团队的远程worker, 店名: ${SHOP}, 通过MCP工具连接中央信箱。本回合流程(严格照做): 第一步调用 pulsing_drain_mail 取走你信箱里的全部信件。第二步检查信件: 若其中有任务信要求执行 sleep, 就用shell真实执行该秒数的sleep(只执行一次); 若要求回信, 睡醒后调用 pulsing_send 工具参数 to=chief, message=ECHO=${SHOP}|slept。第三步若信箱为空或没有可执行的任务, 什么都不做直接结束本回合。不要重复执行已做过的动作。" \
    < /dev/null >> /tmp/worker.log 2>&1
  sleep 3
done
echo "worker-$SHOP done" >> /tmp/worker.log
