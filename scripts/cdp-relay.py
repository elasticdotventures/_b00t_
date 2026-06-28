#!/usr/bin/env python3
"""CDP relay — forwards WSL port :9223 → Windows host CDP :9222.
   Enables b00t-rpa to control Chrome running on Windows from WSL.

Usage:
  python3 scripts/cdp-relay.py            # foreground
  just cdp-relay &                         # background via just
  CDP_HOST=192.168.1.5 python3 scripts/cdp-relay.py  # custom host

Requires Chrome running on Windows with:
  chrome.exe --remote-debugging-port=9222 --remote-allow-origins=*
"""
import socket, threading, sys, os, signal

HOST = "0.0.0.0"
PORT = 9223
TARGET = os.environ.get("CDP_HOST", "172.30.64.1")
TARGET_PORT = 9222
POLL_INTERVAL = 5  # seconds


def relay(src, dst, name):
    """Bidirectional copy between two sockets."""
    try:
        while True:
            data = src.recv(65536)
            if not data:
                break
            dst.sendall(data)
    except (ConnectionResetError, BrokenPipeError, OSError):
        pass
    finally:
        for s in (src, dst):
            try:
                s.close()
            except OSError:
                pass


def check_backend():
    """Test connectivity to the Windows CDP endpoint."""
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(3)
        s.connect((TARGET, TARGET_PORT))
        s.sendall(b'GET /json/version HTTP/1.0\r\nHost: localhost\r\n\r\n')
        resp = s.recv(4096)
        s.close()
        if b'WebKit' in resp or b'Chrome' in resp or b'"Browser"' in resp:
            return True
    except Exception:
        pass
    return False


def main():
    # Ignore SIGINT in threads so Ctrl+C kills only the main thread
    signal.signal(signal.SIGINT, signal.SIG_DFL)

    srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind((HOST, PORT))
    srv.listen(20)
    srv.settimeout(1)  # allow periodic checks

    print(f"🔌 CDP relay: WSL :{PORT} → {TARGET}:{TARGET_PORT}")

    if check_backend():
        print(f"   ✅ Backend reachable — Chrome CDP is live")
    else:
        print(f"   ⚠️  Cannot reach {TARGET}:{TARGET_PORT} — start Chrome with:")
        print(f"      chrome.exe --remote-debugging-port={TARGET_PORT} --remote-allow-origins=*")
        print(f"   Relay will wait and retry every {POLL_INTERVAL}s")

    last_check = 0
    connections = 0

    while True:
        try:
            client, addr = srv.accept()
        except socket.timeout:
            # Periodic backend health check
            if int(__import__('time').time()) - last_check > POLL_INTERVAL:
                last_check = int(__import__('time').time())
                if not check_backend():
                    print(f"   ⚠️  Backend {TARGET}:{TARGET_PORT} still unreachable")
            continue

        backend = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            backend.settimeout(5)
            backend.connect((TARGET, TARGET_PORT))
            connections += 1
            threading.Thread(target=relay, args=(client, backend, "c→b"), daemon=True).start()
            threading.Thread(target=relay, args=(backend, client, "b→c"), daemon=True).start()
            if connections == 1:
                print(f"   🔗 First connection established — CDP bridge active")
        except Exception as e:
            print(f"   ⚠️  Connection failed: {e}")
            client.close()


if __name__ == "__main__":
    main()
