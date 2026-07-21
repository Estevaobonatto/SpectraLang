import socket
import threading
import sys

listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
listener.bind(("127.0.0.1", 0))
listener.listen(64)
print(listener.getsockname()[1], flush=True)

def serve(conn):
    try:
        data = conn.recv(5)
        if data == b"PING\n":
            conn.sendall(b"PONG\n")
        conn.recv(1)
    finally:
        conn.close()

try:
    while True:
        conn, _ = listener.accept()
        threading.Thread(target=serve, args=(conn,), daemon=True).start()
except KeyboardInterrupt:
    pass
