#!/usr/bin/env python3
"""mini_proxy: codex Responses API → bigmodel Chat Completions 转发器
在沙盒内运行, 监听 127.0.0.1:4020"""
import json, os, sys, urllib.request
from http.server import HTTPServer, BaseHTTPRequestHandler

BIGMODEL = "https://open.bigmodel.cn/api/paas/v1/chat/completions"
API_KEY = os.environ.get("BIGMODEL_API_KEY", "")
PORT = int(os.environ.get("PROXY_PORT", "4020"))

def call_bigmodel(messages, model="glm-5-turbo", tools=None):
    """Call bigmodel chat completions, return (text, tool_calls)."""
    req_body = {"model": model, "messages": messages}
    if tools:
        req_body["tools"] = [
            {"type": "function",
             "function": {"name": t["function"]["name"],
                          "description": t["function"].get("description",""),
                          "parameters": t["function"].get("parameters",{})}}
            for t in tools if t.get("type") == "function"
        ]
    req = urllib.request.Request(
        BIGMODEL, data=json.dumps(req_body).encode(),
        headers={"Content-Type": "application/json",
                 "Authorization": f"Bearer {API_KEY}"})
    resp = urllib.request.urlopen(req, timeout=60)
    data = json.loads(resp.read())
    msg = data["choices"][0]["message"]
    tool_calls = msg.get("tool_calls", [])
    text = msg.get("content", "") or ""
    return text, tool_calls

def to_sse(text, tool_calls):
    """Convert to Responses API SSE events."""
    events = [
        {"type":"response.created","response":{"id":"p","object":"response","created_at":0,"status":"in_progress","model":"m","output":[]}},
        {"type":"response.in_progress","response":{"id":"p","object":"response","status":"in_progress","model":"m","output":[]}},
    ]
    if tool_calls:
        for i, tc in enumerate(tool_calls):
            fn = tc.get("function", {})
            events.append({"type":"response.output_item.added","output_index":i,
                "item":{"type":"function_call","id":f"c{i}","call_id":f"c{i}",
                        "name":fn.get("name",""),"arguments":fn.get("arguments","{}")}})
            events.append({"type":"response.function_call_arguments.delta",
                "item_id":f"c{i}","delta":fn.get("arguments","{}")})
            events.append({"type":"response.function_call_arguments.done",
                "item_id":f"c{i}","arguments":fn.get("arguments","{}")})
            events.append({"type":"response.output_item.done","output_index":i,
                "item":{"type":"function_call","id":f"c{i}","call_id":f"c{i}",
                        "name":fn.get("name",""),"arguments":fn.get("arguments","{}"),"status":"completed"}})
        events.append({"type":"response.completed","response":{"id":"p","object":"response","created_at":0,
            "status":"completed","model":"m",
            "output":[{"type":"function_call","id":f"c{i}","call_id":f"c{i}",
                       "name":tc["function"]["name"],"arguments":tc["function"]["arguments"]}
                      for i,tc in enumerate(tool_calls)]}})
    else:
        msg = {"type":"message","id":"m","role":"assistant","status":"completed",
               "content":[{"type":"output_text","text":text}]}
        events.append({"type":"response.output_item.added","output_index":0,
            "item":{"type":"message","id":"m","role":"assistant","status":"in_progress","content":[]}})
        events.append({"type":"response.content_part.added","item_id":"m","output_index":0,
            "content_index":0,"part":{"type":"output_text","text":text}})
        events.append({"type":"response.output_item.done","output_index":0,"item":msg})
        events.append({"type":"response.completed","response":{"id":"p","object":"response","created_at":0,
            "status":"completed","model":"m","output":[msg]}})

    lines = [f"data: {json.dumps(e, ensure_ascii=False)}\n" for e in events]
    lines.append("\n")
    return "".join(lines).encode()

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length)) if length else {}

        # Extract messages from Responses API input
        messages = []
        raw_input = body.get("input", [])
        if isinstance(raw_input, str):
            messages.append({"role": "user", "content": raw_input})
        elif isinstance(raw_input, list):
            for item in raw_input:
                if isinstance(item, str):
                    messages.append({"role": "user", "content": item})
                elif isinstance(item, dict):
                    role = item.get("role", "user")
                    content = item.get("content", "")
                    if isinstance(content, list):
                        content = " ".join(c.get("text","") for c in content if isinstance(c, dict))
                    messages.append({"role": role, "content": str(content)})

        tools = body.get("tools", [])
        model = body.get("model", "glm-5-turbo")

        try:
            text, tool_calls = call_bigmodel(messages, model, tools)
            sse = to_sse(text, tool_calls)
        except Exception as e:
            sse = to_sse(f"Error: {e}", [])

        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        for line in sse.split(b"\n")[:-1]:
            if line:
                chunk = line + b"\n"
                self.wfile.write(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
        self.wfile.write(b"0\r\n\r\n")

    def log_message(self, fmt, *args):
        pass

class TS(HTTPServer):
    daemon_threads = True
    def handle_error(self, *a): pass

if __name__ == "__main__":
    print(f"mini_proxy :{PORT} → bigmodel", flush=True)
    TS(("127.0.0.1", PORT), Handler).serve_forever()
