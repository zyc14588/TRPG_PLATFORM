import http.server
import json
import os
import ssl


MODEL_ID = os.environ["AR11_MODEL_ID"]
TOKEN = open("/provider/token", encoding="utf-8").read()
PRIVATE_CANARY = os.environ.get(
    "AR11_PRIVATE_CANARY",
    "AR11_PRIVATE_CANARY_MISSING",
)


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/v1/models" or not self.authorized():
            self.reply(403, {"error": "forbidden"})
            return
        self.reply(200, {"data": [{"id": MODEL_ID}]})

    def do_POST(self):
        if self.path != "/v1/chat/completions" or not self.authorized():
            self.reply(403, {"error": "forbidden"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > 1_048_576:
                raise ValueError("invalid request size")
            request = json.loads(self.rfile.read(length))
            messages = request.get("messages", [])
            system = next(
                (
                    message.get("content", "")
                    for message in messages
                    if message.get("role") == "system"
                ),
                "",
            )
        except (ValueError, json.JSONDecodeError):
            self.reply(400, {"error": "invalid_request"})
            return

        human_kp = "authority=HUMAN_KP" in system
        decision = {
            "kind": "npc_turn",
            "player_visible_text": (
                f"私密检定草案已由 Keeper 审阅。{PRIVATE_CANARY}"
                if human_kp
                else "AI Keeper 建议先核对档案索引，再检查封存书库的门锁。"
            ),
            "tool": None,
        }
        self.reply(
            200,
            {
                "choices": [
                    {"message": {"content": json.dumps(decision, ensure_ascii=False)}}
                ],
                "usage": {"prompt_tokens": 64, "completion_tokens": 24},
            },
        )

    def authorized(self):
        return self.headers.get("Authorization") == "Bearer " + TOKEN

    def reply(self, status, payload):
        body = json.dumps(payload, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        return


server = http.server.ThreadingHTTPServer(("0.0.0.0", 9443), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain("/provider/server.crt", "/provider/server.key")
server.socket = context.wrap_socket(server.socket, server_side=True)
server.serve_forever()
