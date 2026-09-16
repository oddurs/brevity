#!/usr/bin/env python3
"""A fake LLM endpoint, so you can work on brevity without spending money.

    python3 scripts/mock-provider.py &
    BREVITY_PROVIDER=openai-compat \
    BREVITY_BASE_URL=http://127.0.0.1:8799/v1 \
    BREVITY_MODEL=mock \
      cargo run -- --stdin --no-replace <<< "some long text"

It answers all three wire shapes brevity speaks and picks the right one from the
request path, so it works for every provider:

    /v1/messages                      Anthropic
    /v1beta/models/<m>:generateContent  Gemini
    /chat/completions                 OpenAI-compatible

Flags:
    --port N        listen elsewhere (default 8799)
    --reply TEXT    what the fake model should say
    --status N      answer every request with this HTTP status instead
    --delay SECS    stall before replying, to exercise BREVITY_TIMEOUT_SECS
    --echo          print the request headers and body it received
"""

import argparse
import json
import socketserver
import sys
import time
from http.server import BaseHTTPRequestHandler

ARGS = None

INTERESTING_HEADERS = {
    "authorization", "x-api-key", "anthropic-version", "x-goog-api-key",
    "content-type", "user-agent", "x-title", "http-referer",
}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):  # noqa: N802  (stdlib naming)
        length = int(self.headers.get("content-length", 0))
        raw = self.rfile.read(length) or b"{}"

        if ARGS.echo:
            body = json.loads(raw)
            headers = {k.lower(): v for k, v in self.headers.items()
                       if k.lower() in INTERESTING_HEADERS}
            print(f"--> {self.path}", flush=True)
            print(f"    headers {json.dumps(headers, sort_keys=True)}", flush=True)
            print(f"    body    {json.dumps(body, sort_keys=True)}", flush=True)

        if ARGS.delay:
            time.sleep(ARGS.delay)

        if ARGS.status and ARGS.status >= 400:
            self.respond(ARGS.status, {"error": {"message": f"mock provider says {ARGS.status}"}})
            return

        self.respond(200, self.reply_for(self.path))

    def reply_for(self, path):
        text = ARGS.reply
        if "/v1/messages" in path:  # Anthropic
            return {
                "id": "msg_mock", "type": "message", "role": "assistant",
                "model": "mock", "stop_reason": "end_turn",
                "content": [{"type": "text", "text": text}],
                "usage": {"input_tokens": 10, "output_tokens": 5},
            }
        if "generateContent" in path:  # Gemini
            return {"candidates": [
                {"content": {"parts": [{"text": text}], "role": "model"}, "finishReason": "STOP"}
            ]}
        return {  # OpenAI-compatible
            "id": "chatcmpl-mock", "object": "chat.completion", "model": "mock",
            "choices": [{"index": 0, "finish_reason": "stop",
                         "message": {"role": "assistant", "content": text}}],
        }

    def respond(self, status, payload):
        raw = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *_args):
        pass


def main():
    global ARGS
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=8799)
    parser.add_argument("--reply", default="The mock provider replied.")
    parser.add_argument("--status", type=int, default=0)
    parser.add_argument("--delay", type=float, default=0.0)
    parser.add_argument("--echo", action="store_true")
    ARGS = parser.parse_args()

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", ARGS.port), Handler) as server:
        print(f"mock provider on http://127.0.0.1:{ARGS.port}", file=sys.stderr, flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
