#!/usr/bin/env python3
"""Mock LLM server v4: stdlib HTTP, proper chunked SSE."""
import json, re, sys, threading
from http.server import HTTPServer, BaseHTTPRequestHandler

SHELL = "shell"
SEND = "pulsing_send"
BCAST = "pulsing_broadcast"

def func_call(call_id, name, args):
    a = json.dumps(args)
    return [
        {"type":"response.created","response":{"id":"m","object":"response","created_at":0,"status":"in_progress","model":"mock","output":[]}},
        {"type":"response.in_progress","response":{"id":"m","object":"response","status":"in_progress","model":"mock","output":[]}},
        {"type":"response.output_item.added","output_index":0,"item":{"type":"function_call","id":call_id,"call_id":call_id,"name":name,"arguments":a}},
        {"type":"response.function_call_arguments.delta","item_id":call_id,"delta":a},
        {"type":"response.function_call_arguments.done","item_id":call_id,"arguments":a},
        {"type":"response.output_item.done","output_index":0,"item":{"type":"function_call","id":call_id,"call_id":call_id,"name":name,"arguments":a,"status":"completed"}},
        {"type":"response.completed","response":{"id":"m","object":"response","created_at":0,"status":"completed","model":"mock","output":[{"type":"function_call","id":call_id,"call_id":call_id,"name":name,"arguments":a}]}},
    ]

def text_done(text):
    msg = {"type":"message","id":"msg","role":"assistant","status":"completed","content":[{"type":"output_text","text":text}]}
    return [
        {"type":"response.created","response":{"id":"m","object":"response","created_at":0,"status":"in_progress","model":"mock","output":[]}},
        {"type":"response.in_progress","response":{"id":"m","object":"response","status":"in_progress","model":"mock","output":[]}},
        {"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":"msg","role":"assistant","status":"in_progress","content":[]}},
        {"type":"response.content_part.added","item_id":"msg","output_index":0,"content_index":0,"part":{"type":"output_text","text":text}},
        {"type":"response.output_item.done","output_index":0,"item":msg},
        {"type":"response.completed","response":{"id":"m","object":"response","created_at":0,"status":"completed","model":"mock","output":[msg]}},
    ]

class MockHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    
    def do_POST(self):
        length = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(length) if length else b''
        
        try: req = json.loads(body)
        except: req = {}
        
        # 发现工具名
        global SHELL, SEND, BCAST
        for t in req.get("tools", []):
            fn = t.get("function", {})
            n = fn.get("name", t.get("name", ""))
            if "shell" in n.lower() or "exec" in n.lower() or "container" in n.lower(): SHELL = n
            if "pulsing_send" in n: SEND = n
            if "pulsing_broadcast" in n: BCAST = n
        
        inp = json.dumps(req.get("input", req.get("messages", "")))
        
        # 阶段判断
        if "SLEEP=" in inp and "function_call_output" not in inp and "output" not in str(req.get("input",""))[0:50]:
            # Phase 1: 收到任务 → sleep
            m = re.search(r"SLEEP=(\d+)", inp)
            secs = m.group(1) if m else "1"
            events = func_call("c1", SHELL, {"command":["sleep",secs]})
        elif "function_call_output" in inp:
            if "sent" in inp.lower() or "pulsing" in inp.lower():
                # Phase 3: MCP完成 → 结束
                events = text_done("Done.")
            else:
                # Phase 2: shell完成 → 发信
                events = func_call("c2", SEND, {"to":"chief","message":"ECHO=done|slept","silent":True})
        elif "TASK=" in inp or "ECHO" in inp:
            # Chief: 广播
            m = re.search(r"SLEEP=(\d+)", inp)
            secs = m.group(1) if m else "1"
            shops = [f"e{i}" for i in range(1,51)]
            events = func_call("c0", BCAST, {"to_list":shops,"message":f"ECHO=报名||SLEEP={secs}","silent":False})
        else:
            events = text_done("OK")
        
        # 构造SSE
        lines = []
        for e in events:
            lines.append(f"data: {json.dumps(e, ensure_ascii=False)}\n")
        lines.append("\n")
        sse = "".join(lines).encode()
        
        # 发送(chunked)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Cache-Control', 'no-cache')
        self.send_header('Transfer-Encoding', 'chunked')
        self.end_headers()
        for line in sse.split(b'\n')[:-1]:  # 去掉最后的空行
            chunk = line + b'\n'
            self.wfile.write(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
        self.wfile.write(b"0\r\n\r\n")
    
    def log_message(self, fmt, *args):
        pass

class ThreadingHTTPServer(HTTPServer):
    daemon_threads = True
    def handle_error(self, request, client_address):
        pass

if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 9999
    server = ThreadingHTTPServer(("127.0.0.1", port), MockHandler)
    print(f"Mock LLM v4 :{port}", flush=True)
    server.serve_forever()
