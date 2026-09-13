"""Background NATS listener + ring buffer for b00t-nats-relay.

Dependency-free: talks raw NATS protocol over a stdlib socket so nothing
needs adding to Hermes's shared venv. Same subject convention as the
companion Claude Code channel (nats-channel.ts): b00t.hive.mesh.channel.>
"""

import json
import os
import socket
import threading
import time
from collections import deque

_BUFFER = deque(maxlen=200)
_LOCK = threading.Lock()
_CONNECTED = False

_SUBJECT = "b00t.hive.mesh.channel.>"
_HIVE_NATS_ENV = os.path.expanduser("~/.b00t/secrets/hive-nats.env")


def _load_creds():
    creds = {}
    with open(_HIVE_NATS_ENV) as f:
        for line in f:
            if "=" in line:
                k, v = line.strip().split("=", 1)
                creds[k] = v
    return creds


def _listener_loop(logger):
    global _CONNECTED
    while True:
        try:
            creds = _load_creds()
            sock = socket.create_connection(("127.0.0.1", 4222), timeout=10)
            sock.settimeout(30)
            buf = b""

            def readline():
                nonlocal buf
                while b"\r\n" not in buf:
                    chunk = sock.recv(65536)
                    if not chunk:
                        raise ConnectionError("NATS connection closed")
                    buf += chunk
                line, buf2 = buf.split(b"\r\n", 1)
                buf = buf2
                return line

            info_line = readline()  # INFO {...}
            connect_opts = json.dumps(
                {
                    "user": creds.get("HIVE_NATS_USER"),
                    "pass": creds.get("HIVE_NATS_PASSWORD"),
                    "verbose": False,
                    "pedantic": False,
                }
            )
            sock.sendall(f"CONNECT {connect_opts}\r\n".encode())
            sock.sendall(f"SUB {_SUBJECT} 1\r\n".encode())
            sock.sendall(b"PING\r\n")

            with _LOCK:
                _CONNECTED = True
            logger.info("b00t-nats-relay: connected, subscribed to %s", _SUBJECT)

            while True:
                line = readline()
                if line.startswith(b"MSG "):
                    parts = line.decode().split(" ")
                    subject = parts[1]
                    n_bytes = int(parts[-1])
                    while len(buf) < n_bytes + 2:
                        buf += sock.recv(65536)
                    payload = buf[:n_bytes]
                    buf = buf[n_bytes + 2:]
                    with _LOCK:
                        _BUFFER.append(
                            {
                                "subject": subject,
                                "text": payload.decode(errors="replace"),
                                "ts": time.time(),
                            }
                        )
                elif line.startswith(b"PING"):
                    sock.sendall(b"PONG\r\n")
        except Exception as exc:  # noqa: BLE001 — background loop must never die silently
            with _LOCK:
                _CONNECTED = False
            logger.warning("b00t-nats-relay: listener error, retrying in 10s: %s", exc)
            time.sleep(10)


def start_background_listener(logger):
    t = threading.Thread(target=_listener_loop, args=(logger,), daemon=True)
    t.start()


def recent_handler(args, **kwargs):
    limit = int(args.get("limit") or 20)
    with _LOCK:
        connected = _CONNECTED
        items = list(_BUFFER)[-limit:]
    return json.dumps({"connected": connected, "count": len(items), "messages": items})
