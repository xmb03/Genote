#!/usr/bin/env python3
"""Fake LLM API for Genote offline tests.

Logs every request as one JSONL line (method, path, headers, body) to --log.
Replies with the canonical JSON shape each provider expects. --spec is a JSON
file mapping request paths to {"fail": N, "text": "..."}: fail = serve Nx
HTTP 500 first (tests retry logic), text = response override (default: a
28-line note that satisfies genote's small size check).
"""

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

DEFAULT_TEXT = "# Test Note\n" + "\n".join("line %d" % i for i in range(1, 28))


def make_responder(spec):
    fails = {p: s.get("fail", 0) for p, s in spec.items()}

    def responder(path):
        cfg = spec.get(path, {})
        if fails.get(path, 0) > 0:
            fails[path] -= 1
            return 500, b'{"error": "mock: temporary failure"}'
        text = cfg.get("text", DEFAULT_TEXT)
        if path == "/api/generate":
            body = {"response": text, "eval_count": 42}
        elif path == "/api/chat":
            body = {"message": {"content": text}, "eval_count": 42}
        elif path == "/v1/chat/completions":
            body = {"choices": [{"message": {"content": text}}], "usage": {"total_tokens": 42}}
        elif path == "/completion":
            body = {"content": text, "timings": {"predicted_n": 42}}
        elif path == "/v1/messages":
            body = {"content": [{"type": "text", "text": text}],
                    "usage": {"input_tokens": 1, "output_tokens": 2}}
        elif path.endswith(":generateContent"):
            body = {"candidates": [{"content": {"parts": [{"text": text}]}}],
                    "usageMetadata": {"totalTokenCount": 42}}
        else:
            return 404, b"mock: unknown endpoint"
        return 200, json.dumps(body).encode()

    return responder


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--log", default="")
    ap.add_argument("--spec", default="")
    ap.add_argument("--port", type=int, default=0)
    args = ap.parse_args()
    spec = json.load(open(args.spec)) if args.spec else {}
    responder = make_responder(spec)
    logf = open(args.log, "a") if args.log else None

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            raw = self.rfile.read(length)
            try:
                body = json.loads(raw)
            except ValueError:
                body = raw.decode("utf-8", "replace")
            status, payload = responder(self.path)
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            if logf:
                logf.write(json.dumps({"method": "POST", "path": self.path,
                                       "headers": dict(self.headers), "body": body}) + "\n")
                logf.flush()

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(server.server_address[1], flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
