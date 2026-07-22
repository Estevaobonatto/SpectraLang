"""Reserved collector fixture for the Redis tracing integration lane."""
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        self.rfile.read(length)
        self.send_response(200)
        self.end_headers()

    def log_message(self, *_args):
        return


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", 4318), Handler).serve_forever()
