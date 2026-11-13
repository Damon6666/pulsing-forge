// claude-host.mjs — Claude Code 侧信箱桥宿主
// 为每个工号创建一套 MCP 工具闭包，_shop 身份焊死在闭包里（模型不可见、不可伪造）。
// 用法: PULSING_KEY=... node claude-host.mjs --hub 127.0.0.1:12410 --workers e1 --model glm-4.6
import { query, createSdkMcpServer } from "@anthropic-ai/claude-agent-sdk";
import net from "net";
import { z } from "zod";

const args = process.argv.slice(2);
const get = (k, d) => { const i = args.indexOf("--" + k); return i >= 0 ? args[i + 1] : d; };
const HUB_ADDR = get("hub", "127.0.0.1:12410");
const [HUB_HOST, HUB_PORT] = [HUB_ADDR.split(":")[0], HUB_ADDR.split(":")[1] || "12410"];
const WORKERS = (get("workers", "e1")).split(",");
const MODEL = get("model", "glm-4.6");
const KEY = process.env.PULSING_KEY || process.env.ANTHROPIC_AUTH_TOKEN || "";
const POLL_SECS = parseInt(get("poll", "5"), 10);

function hubCall(tool, hubArgs, timeoutMs = 30000) {
  return new Promise((resolve) => {
    const s = net.connect(parseInt(HUB_PORT, 10), HUB_HOST, () => {
      s.write(JSON.stringify({ tool, args: hubArgs }) + "\n");
    });
    let buf = "";
    s.on("data", (c) => { buf += c; });
    s.on("end", () => resolve(buf.trim()));
    s.on("error", (e) => resolve(JSON.stringify({ ok: false, error: String(e) })));
    setTimeout(() => { s.destroy(); resolve(JSON.stringify({ ok: false, error: "timeout" })); }, timeoutMs);
  });
}

// 工具闭包：每个工号一套，from 焊死在闭包里
function mailboxServerFor(id) {
  return createSdkMcpServer({
    name: "pulsing-" + id,
    version: "1.0.0",
    tools: [
      {
        name: "pulsing_send",
        description: "给某个身份发一封信（发件人已绑定为你，无需指定）",
        inputSchema: { to: z.string().describe("收件人身份"), message: z.string().describe("信件正文") },
        handler: async ({ to, message }) => {
          const r = await hubCall("pulsing_send", { to, message, _shop: id });
          return { content: [{ type: "text", text: "[回执] " + r.slice(0, 300) }] };
        },
      },
      {
        name: "pulsing_drain_mail",
        description: "取走并清空你自己信箱里的全部信件（自动绑定身份）",
        inputSchema: {},
        handler: async () => {
          const r = await hubCall("pulsing_drain_mail", { _shop: id });
          return { content: [{ type: "text", text: "[你的信件] " + (r.slice(0, 4000) || "(空)") }] };
        },
      },
      {
        name: "pulsing_recall",
        description: "按信件编号或关键词从账本取信全文",
        inputSchema: { letter_id: z.number().optional(), query: z.string().optional() },
        handler: async ({ letter_id, query: q }) => {
          const r = await hubCall("pulsing_recall", { letter_id: letter_id || 0, query: q || "", _shop: id });
          return { content: [{ type: "text", text: r.slice(0, 4000) }] };
        },
      },
      {
        name: "pulsing_health",
        description: "信箱中枢健康检查",
        inputSchema: {},
        handler: async () => ({
          content: [{ type: "text", text: "[health] " + (await hubCall("pulsing_health", {})).slice(0, 200) }],
        }),
      },
    ],
  });
}

const WORK_PROMPT = (id) => `你是工号 ${id} 的 worker。严格按序执行：
1. 调用 pulsing_drain_mail 取走你信箱里的全部任务信（没有就等待，每 ${POLL_SECS} 秒重试，最多 30 分钟）。
2. 对每封任务信逐字执行其中指令（运行命令用 Bash，读写文件用 Read/Write/Edit）。
3. 每完成一封任务信，调用 pulsing_send(to="chief", message="ECHO=${id}|<任务号>|done|<一句话结果>")。
4. 全部完成后结束。除任务信要求的 ECHO 外不发任何信。`;

async function runWorker(id) {
  const t0 = Date.now();
  console.log(`[host] worker ${id} 启动`);
  try {
    const q = query({
      prompt: WORK_PROMPT(id),
      options: {
        model: MODEL,
        mcpServers: { pulsing: mailboxServerFor(id) },
        allowedTools: ["mcp__pulsing__*", "Bash", "Read", "Write", "Edit", "Grep"],
        env: {
          ...process.env,
          ANTHROPIC_BASE_URL: process.env.ANTHROPIC_BASE_URL || "https://api.z.ai/api/anthropic",
          ANTHROPIC_AUTH_TOKEN: KEY,
          DISABLE_TELEMETRY: "1",
        },
      },
    });
    for await (const m of q) {
      if (m.type === "assistant" && m.message?.content) {
        for (const c of m.message.content) {
          if (c.type === "text" && c.text) console.log(`[${id}] ${c.text.slice(0, 160)}`);
          if (c.type === "tool_use") console.log(`[${id}] tool ${c.name} ${JSON.stringify(c.input || {}).slice(0, 100)}`);
        }
      }
    }
    console.log(`[host] worker ${id} 完成, 耗时 ${((Date.now() - t0) / 1000).toFixed(0)}s`);
  } catch (e) {
    console.log(`[host] worker ${id} 出错: ${String(e).slice(0, 300)}`);
  }
}

console.log(`[host] hub=${HUB_ADDR} workers=${WORKERS.join(",")} model=${MODEL}`);
await Promise.all(WORKERS.map(runWorker));
console.log("[host] ALL-DONE");
