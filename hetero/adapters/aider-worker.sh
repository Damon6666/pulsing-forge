#!/usr/bin/env bash
# aider-worker.sh — Aider 修复工（身份 e2）：轮询信箱 → 执行 FIX-TASK → 回执 diff
# 环境变量: HUB_ADDR, WORKER_ID, MODEL, REPO_PATH, OPENAI_API_KEY, OPENAI_API_BASE
set -uo pipefail

HUB_HOST="${HUB_ADDR%%:*}"
HUB_PORT="${HUB_ADDR##*:}"
ID="${WORKER_ID:-e2}"
REPO="${REPO_PATH:-/w/repo}"
MAX_WAIT="${MAX_WAIT:-1800}"     # 等任务信的最长秒数
POLL="${POLL:-5}"

hub() {  # hub <tool> <json_args>
  python3 - "$HUB_HOST" "$HUB_PORT" "$@" <<'PY'
import json, socket, sys
host, port, tool, args = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
s = socket.create_connection((host, port), timeout=30)
s.sendall((json.dumps({"tool": tool, "args": json.loads(args)}) + "\n").encode())
b = b""
while not b.endswith(b"\n"):
    c = s.recv(65536)
    if not c:
        break
    b += c
s.close()
print(b.decode(errors="replace").strip())
PY
}

echo "[$ID] waiter start (hub=$HUB_HOST:$HUB_PORT repo=$REPO)"

TASK=""
WAITED=0
while [ -z "$TASK" ] && [ "$WAITED" -lt "$MAX_WAIT" ]; do
  R=$(hub pulsing_drain_mail "{\"_shop\":\"$ID\"}")
  TASK=$(printf '%s' "$R" | python3 -c '
import json, sys, re
try:
    s = json.load(sys.stdin).get("result", "")
except Exception:
    s = sys.stdin.read()
m = re.search(r"FIX-TASK[^\\n]*", s)
print(m.group(0) if m else "")')
  [ -z "$TASK" ] && sleep "$POLL" && WAITED=$((WAITED + POLL))
done

if [ -z "$TASK" ]; then echo "[$ID] NO-TASK timeout"; exit 1; fi
echo "[$ID] got FIX-TASK (${#TASK} chars)"
cd "$REPO" || exit 1

# 备份用于 diff
BK=/tmp/aider.orig; mkdir -p "$BK"
for f in mock/mini_proxy.py docs/sandbox-llm-access.md; do
  [ -f "$f" ] && cp --parents "$f" "$BK/" 2>/dev/null || true
done

# 执行修复（任务信全文作为指令；文件清单由任务信给出）
timeout "${FIX_TIMEOUT:-600}" aider --model "$MODEL" --no-git --yes-always --no-auto-commits \
  --message "你是修复工。严格按以下任务信修改指定文件（只做最小修改）：$TASK" \
  mock/mini_proxy.py docs/sandbox-llm-access.md 2>&1 | tail -4

# 本地机械验证
GREP_HITS=$(grep -rc "10[.]12[.]113[.]61" . --include="*.py" --include="*.md" 2>/dev/null | grep -v ":0" | wc -l)
COMPILE=$(python3 -m py_compile mock/mini_proxy.py >/dev/null 2>&1 && echo PASS || echo FAIL)
FUNCS=$(python3 - <<'PY'
import ast
try:
    t = ast.parse(open("mock/mini_proxy.py", encoding="utf-8").read())
    print(sum(isinstance(n, ast.FunctionDef) for n in ast.walk(t)))
except Exception:
    print(-1)
PY
)

# diff 回执
DIFF=$(git diff --no-color -- mock/mini_proxy.py docs/sandbox-llm-access.md 2>/dev/null | head -c 3500)
RECEIPT="ECHO=${ID}|done|fix-applied|grep命中=${GREP_HITS}|py_compile=${COMPILE}|funcs=${FUNCS}|diff↓↓↓\n${DIFF}"
hub pulsing_send "{\"to\":\"chief\",\"message\":$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$RECEIPT"),\"_shop\":\"$ID\"}" >/dev/null

echo "[$ID] receipt sent (grep=$GREP_HITS compile=$COMPILE funcs=$FUNCS)"
