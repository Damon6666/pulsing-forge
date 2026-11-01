//! forge-runtime::tide_server — TCP 控制口 + 回合调度器。
//! codex 的 MCP 工具调用经 attach(TCP) 到达这里; 非静默信触发回合。

use crate::tide::TideHub;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// 控制口单条命令的回执
#[derive(serde::Serialize)]
pub struct ToolReply {
    pub ok: bool,
    pub result: String,
    pub error: String,
}

impl ToolReply {
    fn ok(result: &str) -> Self {
        ToolReply { ok: true, result: result.to_string(), error: String::new() }
    }
    fn err(e: &str) -> Self {
        ToolReply { ok: false, result: String::new(), error: e.to_string() }
    }
}

/// 处理一行控制命令, 返回 JSON 回执
pub fn handle_line(hub: &TideHub, line: &str) -> String {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return serde_json::to_string(&ToolReply::err(&format!("JSON解析: {e}"))).unwrap(),
    };
    let tool = v["tool"].as_str().unwrap_or("");
    let args = &v["args"];
    let from_codex = args.get("_from_codex").is_some();

    match tool {
        "pulsing_send" => {
            let to = args["to"].as_str().unwrap_or("");
            let msg = args["message"].as_str().unwrap_or("");
            let silent = args["silent"].as_bool().unwrap_or(false);
            // _shop 是调用方身份(attach 注入)
            let from = args["_shop"].as_str().unwrap_or("unknown");
            match hub.send(from, to, msg, silent, from_codex) {
                Ok(_) => serde_json::to_string(&ToolReply::ok("已送达并记账")).unwrap(),
                Err(e) => serde_json::to_string(&ToolReply::err(&e)).unwrap(),
            }
        }
        "pulsing_broadcast" => {
            let from = args["_shop"].as_str().unwrap_or("unknown");
            let msg = args["message"].as_str().unwrap_or("");
            let silent = args["silent"].as_bool().unwrap_or(false);
            let targets: Vec<String> = args["to_list"].as_array()
                .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            if targets.is_empty() {
                return serde_json::to_string(&ToolReply::err("缺 to_list")).unwrap();
            }
            let sent = hub.broadcast(from, &targets, msg, silent, from_codex);
            serde_json::to_string(&ToolReply::ok(&format!("已群发 {} 店", sent.len()))).unwrap()
        }
        "pulsing_drain_mail" => {
            let shop = args["_shop"].as_str().unwrap_or("unknown");
            let letters = hub.drain(shop);
            let bodies: Vec<String> = letters.iter()
                .map(|l| format!("[{}→{}] {}", l.from, l.to, &l.body[..l.body.len().min(200)]))
                .collect();
            serde_json::to_string(&ToolReply::ok(&bodies.join("\n---\n"))).unwrap()
        }
        "pulsing_health" => {
            let shops = hub.shops.len();
            let turns: u64 = hub.shops.values()
                .map(|s| s.turns_done.load(std::sync::atomic::Ordering::Relaxed))
                .sum();
            serde_json::to_string(&ToolReply::ok(&format!(
                "店={shops} 回合={turns} 账本写失败={}",
                hub.journal.write_fails.load(std::sync::atomic::Ordering::Relaxed)
            ))).unwrap()
        }
        _ => serde_json::to_string(&ToolReply::err(&format!("未知工具: {tool}"))).unwrap(),
    }
}

/// TCP 控制口服务器: 每连接一行命令一行回执
pub async fn serve_control(hub: Arc<TideHub>, port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    println!("tide 控制口 listen=127.0.0.1:{port}");
    loop {
        let (socket, _) = listener.accept().await?;
        let hub = hub.clone();
        tokio::spawn(async move {
            let (rd, mut wr) = socket.into_split();
            let mut reader = BufReader::new(rd);
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                let reply = handle_line(&hub, line.trim());
                let _ = wr.write_all(format!("{reply}\n").as_bytes()).await;
            }
        });
    }
}

/// 回合调度: 监听店的非静默信到达, 触发 codex 回合
pub async fn shop_scheduler(hub: Arc<TideHub>, shop: String) {
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // 检查信箱是否有非静默信(最近 1 秒内到达的)
        let letters = hub.mailboxes.get(&shop)
            .map(|m| m.lock().unwrap().clone())
            .unwrap_or_default();
        if let Some(trigger) = letters.iter().rev().find(|l| !l.body.starts_with("KICK=")) {
            // 有待处理的信 → 触发回合(在 blocking 线程跑 codex)
            let hub2 = hub.clone();
            let trig = trigger.clone();
            let shop2 = shop.clone();
            let _ = tokio::task::spawn_blocking(move || {
                hub2.run_turn_blocking(&shop2, &trig)
            }).await;
        }
    }
}

/// 启动完整潮汐层: 控制口 + 全部店调度器
pub async fn run_tide(hub: Arc<TideHub>) -> anyhow::Result<()> {
    let port = hub.listen_port;
    // 控制口
    let ctrl = tokio::spawn(serve_control(hub.clone(), port));
    // 店调度器
    for shop in hub.shops.keys() {
        let hub2 = hub.clone();
        let s = shop.clone();
        tokio::spawn(shop_scheduler(hub2, s));
    }
    ctrl.await??;
    Ok(())
}

/// attach: MCP stdio → TCP 控制口桥
/// codex 的 MCP 子进程; 读 stdin JSON-RPC, 转发工具调用到控制口, 写回 stdout
pub async fn run_attach(control_port: u16, shop: Option<String>) -> anyhow::Result<()> {
    use tokio::io::AsyncReadExt;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];

    loop {
        let n = stdin.read(&mut chunk).await?;
        if n == 0 { break; }
        buf.extend_from_slice(&chunk[..n]);
        // 按行分割处理
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line_str = String::from_utf8_lossy(&line).trim().to_string();
            if line_str.is_empty() { continue; }

            let msg: serde_json::Value = match serde_json::from_str(&line_str) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let method = msg["method"].as_str().unwrap_or("");
            let id = msg.get("id").cloned();

            let response = match method {
                "initialize" => Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "forge-tide", "version": "0.1.0"}
                    }
                })),
                "notifications/initialized" | "notifications/cancelled" => None,
                "ping" => Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id, "result": {}
                })),
                "tools/list" => Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id,
                    "result": {"tools": tools_schema()}
                })),
                "tools/call" => {
                    let params = &msg["params"];
                    let tool = params["name"].as_str().unwrap_or("");
                    let mut args = params["arguments"].clone();
                    // 注入调用方身份
                    if let (Some(s), Some(o)) = (shop.as_ref(), args.as_object_mut()) {
                        o.insert("_shop".into(), serde_json::json!(s));
                        o.insert("_from_codex".into(), serde_json::json!(true));
                    }
                    // 转发到控制口
                    let fwd = serde_json::json!({"tool": tool, "args": args});
                    let reply = forward_to_control(control_port, &fwd.to_string()).await;
                    Some(serde_json::json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": {"content": [{"type": "text", "text": reply}]}
                    }))
                }
                _ => Some(serde_json::json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": format!("Unknown method: {method}")}
                })),
            };

            if let Some(resp) = response {
                let mut out = serde_json::to_string(&resp).unwrap();
                out.push('\n');
                let _ = stdout.write_all(out.as_bytes()).await;
                let _ = stdout.flush().await;
            }
        }
    }
    Ok(())
}

async fn forward_to_control(port: u16, line: &str) -> String {
    match TcpStream::connect(("127.0.0.1", port)).await {
        Ok(mut stream) => {
            use tokio::io::AsyncWriteExt;
            if stream.write_all(format!("{line}\n").as_bytes()).await.is_err() {
                return r#"{"ok":false,"error":"控制口写入失败"}"#.to_string();
            }
            let mut buf = Vec::new();
            use tokio::io::AsyncReadExt;
            let _ = stream.read_to_end(&mut buf).await;
            String::from_utf8_lossy(&buf).trim().to_string()
        }
        Err(e) => format!(r#"{{"ok":false,"error":"控制口连接失败: {e}"}}"#),
    }
}

fn tools_schema() -> serde_json::Value {
    serde_json::json!([
        {"name": "pulsing_send",
         "description": "发送一封信给指定店",
         "inputSchema": {"type": "object", "properties": {
             "to": {"type": "string", "description": "收件店名"},
             "message": {"type": "string", "description": "信件正文"},
             "silent": {"type": "boolean", "description": "true=只入箱不唤醒"}
         }, "required": ["to", "message"]}},
        {"name": "pulsing_broadcast",
         "description": "群发一封信给多个店",
         "inputSchema": {"type": "object", "properties": {
             "to_list": {"type": "array", "items": {"type": "string"}},
             "message": {"type": "string"},
             "silent": {"type": "boolean"}
         }, "required": ["to_list", "message"]}},
        {"name": "pulsing_drain_mail",
         "description": "取走并清空自己信箱里的全部信",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "pulsing_health",
         "description": "查看系统健康状态",
         "inputSchema": {"type": "object", "properties": {}}},
    ])
}
