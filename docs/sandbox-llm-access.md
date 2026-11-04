# 沙盒 LLM 访问指南（2026-11 突破）

结论：**PBox 沙盒内可以真 codex + 真 LLM 跑 forge 场景**，已 E2E 验证
（broadcast 3 店，chief 广播 → 2 worker 真实 sleep 5s → MCP 回信 → 收口，
state=PASS，70~135s，10~18 个真实 LLM 请求）。

## 三个关键发现

1. **端点用国际版 `api.z.ai`，不用 `open.bigmodel.cn`**
   同一把 key 两边通用；但经公司代理出口（美国 IP）访问 open.bigmodel.cn
   时网关对 `/api/paas/v4/*` 返回 404（到达的服务器不认这个路径）。
   `api.z.ai/api/paas/v4/chat/completions` 稳定 200。
   模型名 `glm-5-turbo` / `glm-4.7` 均可用。

2. **沙盒 DNS 是坑：代理域名解析到公网 IP，丢包严重**
   `proxy.pjlab.org.cn` 在沙盒内解析为 `180.163.156.36`（公网，约一半连接
   超时）；本机解析为 `10.12.113.61`（内网，稳定 6/6，延迟 0~3s）。
   **沙盒内固定用 `10.12.113.61:23128`**，凭据从平台注入的
   `https_proxy` 环境变量里提取（mini_proxy 自动做，不落盘）。

3. **codex fork 的 MCP 工具是 namespace 类型，chat API 不认**
   codex 发出的 tools 里 MCP 工具包在 `{"type":"namespace","name":"mcp__pulsing",
   "tools":[...]}` 里，z.ai 只收 `type=function`。且 codex 的 function_call
   回项带独立 `namespace` 字段（`ToolName::new(namespace, name)` 路由）。
   mini_proxy v6 做双向转换：扁平名 `mcp__pulsing__pulsing_send` ↔ 结构化
   `{namespace:"mcp__pulsing", name:"pulsing_send"}`。

## 沙盒内跑真 codex 的完整命令

```bash
# 1. 起 mini_proxy（凭据自动从 env 提取，或显式 MINI_PROXY）
BIGMODEL_API_KEY=$KEY nohup python3 /w/mini_proxy.py > /tmp/mini.log 2>&1 &

# 2. forge run（注意: forge run 只认这两个环境变量, 不读 provider.toml）
FORGE_API_KEY=$KEY \
FORGE_BASE_URL="http://127.0.0.1:4020/v1" \
FORGE_CODEX_BIN=/usr/local/bin/codex \
/w/forge run /w/s.toml
```

## 链路

```
shop codex (Responses API, CODEX_HOME 由 forge 生成)
  → mini_proxy v6 :4020   (Responses→Chat, namespace 工具拍平/还原)
  → 内网代理 10.12.113.61:23128 (凭据来自平台 env)
  → api.z.ai /api/paas/v4/chat/completions (glm-5-turbo)
```

## 已知残留

- `web_search` 工具被 DROP（z.ai 不支持该类型），对信箱协议无影响。
- z.ai 无 `/responses` 端点，所以 mini_proxy 转换层是长期必需的。
- exec 接口对同一沙盒串行排队，调试时注意前一个长任务没跑完会顶到超时。
