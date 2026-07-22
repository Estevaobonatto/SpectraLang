"""Real OTLP/HTTP collector process for the R-2707 integration gate."""
from __future__ import annotations

import argparse
import json
import signal
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path


class Collector(BaseHTTPRequestHandler):
    payloads: list[bytes] = []
    invalid_requests = 0

    def do_POST(self) -> None:  # noqa: N802
        if self.path == "/shutdown":
            self.send_response(200)
            self.end_headers()
            threading.Thread(target=self.server.shutdown, daemon=True).start()
            return
        length = int(self.headers.get("Content-Length", "0"))
        payload = self.rfile.read(length)
        if self.path != "/v1/traces" or self.headers.get("Content-Type") != "application/x-protobuf":
            type(self).invalid_requests += 1
            self.send_response(400)
            self.end_headers()
            return
        type(self).payloads.append(payload)
        self.send_response(200)
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802
        self.send_response(405)
        self.end_headers()

    def log_message(self, *_args: object) -> None:
        return


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--port-file", required=True)
    args = parser.parse_args()
    Collector.payloads = []
    Collector.invalid_requests = 0
    server = HTTPServer(("127.0.0.1", args.port), Collector)
    Path(args.port_file).write_text(str(server.server_port), encoding="utf-8")
    def stop(_signum: int, _frame: object) -> None:
        threading.Thread(target=server.shutdown, daemon=True).start()
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        Path(args.output).write_text(
            json.dumps({
                "requests": len(Collector.payloads) + Collector.invalid_requests,
                "invalid_requests": Collector.invalid_requests,
                "payloads": [payload.hex() for payload in Collector.payloads],
            }, indent=2) + "\n",
            encoding="utf-8",
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
