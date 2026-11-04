#!/usr/bin/env python3
"""mini_proxy v4: 全用json.dumps构建, 零f-string"""
import json, os, sys, urllib.request
from http.server import HTTPServer, BaseHTTPRequestHandler

BIGMODEL = "https://open.bigmodel.cn/api/paas/v1/chat/completions"
API_KEY = os.environ.get("BIGMODEL_API_KEY", "")
PORT = int(os.environ.get("PROXY_PORT", "4020"))

def call_bigmodel(messages, model, tools=None):
    req_body = {"model": model, "messages": messages}
    if tools:
        # 直接透传工具定义(bigmodel是OpenAI兼容的,不需要转换)
        req_body["tools"] = tools
        print("TOOLS: " + json.dumps([{"type": t.get("type","?"), "name": t.get("function",{}).get("name", t.get("name","?"))} for t in tools]), file=sys.stderr, flush=True)
    req = urllib.request.Request(BIGMODEL, data=json.dumps(req_body).encode(),
        headers={"Content-Type":"application/json","Authorization":"Bearer " + API_KEY})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    resp = opener.open(req, timeout=60)
    return json.loads(resp.read())

def J(obj):
    return json.dumps(obj, ensure_ascii=False)

def build_sse(body):
    msg = body["choices"][0]["message"]
    tcs = msg.get("tool_calls", [])
    text = msg.get("content", "") or ""
    rid = body.get("id", "resp_proxy")
    model = body.get("model", "m")
    created = body.get("created", 0)
    
    events = []
    
    # response.created
    events.append(J({"type":"response.created","response":{
        "id":rid,"object":"response","created_at":created,
        "status":"in_progress","model":model,"output":[]}}))
    
    if tcs:
        outputs = []
        for i, tc in enumerate(tcs):
            fn = tc["function"]
            call_item = {"type":"function_call","id":"c"+str(i),
                        "call_id":"c"+str(i),"name":fn["name"],
                        "arguments":fn["arguments"]}
            events.append(J({"type":"response.output_item.added",
                "output_index":i,"item":call_item}))
            done_item = dict(call_item)
            done_item["status"] = "completed"
            events.append(J({"type":"response.output_item.done",
                "output_index":i,"item":done_item}))
            outputs.append(done_item)
        events.append(J({"type":"response.completed","response":{
            "id":rid,"object":"response","created_at":created,
            "status":"completed","model":model,"output":outputs}}))
    else:
        msg_item = {"type":"message","id":"m","role":"assistant",
                    "status":"completed","content":[
                        {"type":"output_text","text":text}]}
        events.append(J({"type":"response.output_item.added",
            "output_index":0,"item":{"type":"message","id":"m",
            "role":"assistant","status":"in_progress","content":[]}}))
        events.append(J({"type":"response.output_item.done",
            "output_index":0,"item":msg_item}))
        events.append(J({"type":"response.completed","response":{
            "id":rid,"object":"response","created_at":created,
            "status":"completed","model":model,"output":[msg_item]}}))
    
    lines = ["data: " + e for e in events]
    lines.append("data: [DONE]")
    lines.append("")
    return ("\n\n".join(lines) + "\n").encode()

class H(BaseHTTPRequestHandler):
    def do_POST(self):
        n = int(self.headers.get("Content-Length",0))
        raw = self.rfile.read(n) if n else b''
        print("REQ " + self.path + " " + str(n) + "B", file=sys.stderr, flush=True)
        
        try:
            body = json.loads(raw)
        except:
            body = {}
        
        msgs = []
        inp = body.get("input",[])
        if isinstance(inp,str):
            msgs.append({"role":"user","content":inp})
        elif isinstance(inp,list):
            for it in inp:
                if isinstance(it,str):
                    msgs.append({"role":"user","content":it})
                elif isinstance(it,dict):
                    c = it.get("content","")
                    if isinstance(c,list):
                        c = " ".join(x.get("text","") for x in c if isinstance(x,dict))
                    msgs.append({"role":it.get("role","user"),"content":str(c)})
        
        try:
            resp_body = call_bigmodel(msgs, body.get("model","glm-5-turbo"), body.get("tools"))
            sse = build_sse(resp_body)
            ntc = len(resp_body["choices"][0]["message"].get("tool_calls",[]))
            print("RESP " + str(len(sse)) + "B tool_calls=" + str(ntc), file=sys.stderr, flush=True)
        except Exception as e:
            print("ERR " + str(e), file=sys.stderr, flush=True)
            sse = build_sse({"choices":[{"message":{"content":"Error: " + str(e)}}],
                           "id":"err","model":"m","created":0})
        
        self.send_response(200)
        self.send_header("Content-Type","text/event-stream")
        self.send_header("Cache-Control","no-cache")
        self.send_header("Content-Length",str(len(sse)))
        self.end_headers()
        self.wfile.write(sse)
    
    def log_message(self,*a): pass

class TS(HTTPServer):
    daemon_threads=True
    def handle_error(self,*a): pass

if __name__=="__main__":
    print("mini_proxy v4 :" + str(PORT), flush=True)
    TS(("127.0.0.1",PORT),H).serve_forever()
