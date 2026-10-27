#!/usr/bin/env python3
# Captures screenshots of the Milestone 0 Web page in headless Chromium, with real key presses, for the progress report's media.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/capture-web.py OUTDIR [--port 9417] [--chromium /usr/bin/chromium]
#
# Standard library only. Needs dist/web from tools/build-web.sh. dist/web is served on 127.0.0.1:PORT by `http.server` in a thread,
# Chromium runs headless with a remote-debugging port, and this script speaks the DevTools protocol over a minimal WebSocket client:
# it opens index.html, waits in real time, sends keys with Input.dispatchKeyEvent (the page sees ordinary keydown and keyup events
# with their `code`), and saves Page.captureScreenshot as PNG. Each shot is listed with the time and keys in OUTDIR/web.json.
import base64
import functools
import http.server
import json
import os
import shutil
import socket
import socketserver
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
KEYS = {"ArrowRight": 39, "ArrowDown": 40, "ArrowLeft": 37, "ArrowUp": 38, "Space": 32}
# (file, [(action, argument)]): actions run in order; "wait" is seconds of real time.
PLAN = [
    ("web-start.png", [("wait", 1.5)]),
    ("web-moved.png", [("down", "ArrowRight"), ("wait", 1.2), ("up", "ArrowRight"), ("down", "ArrowDown"), ("wait", 0.6), ("up", "ArrowDown"), ("wait", 0.3)]),
    ("web-dash.png", [("down", "Space"), ("wait", 0.1), ("up", "Space"), ("wait", 0.5)]),
]


class WebSocket:
    """Just enough of RFC 6455 for the DevTools protocol: one text frame out (masked), whole text frames in."""

    def __init__(self, url):
        host, _, path = url[len("ws://"):].partition("/")
        h, _, p = host.partition(":")
        self.s = socket.create_connection((h, int(p)), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall(("GET /%s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (path, host, key)).encode())
        head = b""
        while b"\r\n\r\n" not in head:
            head += self.s.recv(1)
        if b" 101 " not in head.split(b"\r\n")[0]:
            raise RuntimeError("WebSocket upgrade refused: %r" % head[:80])
        self.next_id = 0

    def _read(self, n):
        buf = b""
        while len(buf) < n:
            chunk = self.s.recv(n - len(buf))
            if not chunk:
                raise RuntimeError("WebSocket closed")
            buf += chunk
        return buf

    def recv(self):
        data = b""
        while True:
            b0, b1 = self._read(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._read(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._read(8))[0]
            data += self._read(n)
            if b0 & 0x80:
                return json.loads(data.decode())

    def call(self, method, **params):
        self.next_id += 1
        payload = json.dumps({"id": self.next_id, "method": method, "params": params}).encode()
        mask = os.urandom(4)
        n = len(payload)
        head = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + struct.pack(">H", n) if n < 65536 else bytes([0x80 | 127]) + struct.pack(">Q", n))
        self.s.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))
        while True:
            msg = self.recv()
            if msg.get("id") == self.next_id:
                if "error" in msg:
                    raise RuntimeError("%s: %s" % (method, msg["error"]))
                return msg.get("result", {})


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def key(ws, kind, code):
    args = {"type": "keyDown" if kind == "down" else "keyUp", "code": code, "key": " " if code == "Space" else code, "windowsVirtualKeyCode": KEYS[code]}
    ws.call("Input.dispatchKeyEvent", **args)


def main(argv):
    if len(argv) < 2:
        print(__doc__ or "usage: capture-web.py OUTDIR [--port N] [--chromium PATH]", file=sys.stderr)
        return 2
    out = os.path.abspath(argv[1])
    port = int(argv[argv.index("--port") + 1]) if "--port" in argv else 9417
    chromium = argv[argv.index("--chromium") + 1] if "--chromium" in argv else "/usr/bin/chromium"
    site = os.path.join(ROOT, "dist", "web")
    if not os.path.exists(os.path.join(site, "index.html")):
        print("capture-web: no dist/web; run tools/build-web.sh", file=sys.stderr)
        return 2
    os.makedirs(out, exist_ok=True)
    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass
    handler = functools.partial(Quiet, directory=site)
    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(("127.0.0.1", port), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    # Chromium's singleton socket path must be short, so its profile lives under /tmp.
    profile = tempfile.mkdtemp(prefix="fz2cr.", dir="/tmp")
    debug = free_port()
    proc = subprocess.Popen([chromium, "--headless", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--user-data-dir=" + profile, "--window-size=720,560",
                             "--remote-debugging-port=%d" % debug, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=dict(os.environ, TMPDIR=profile))
    shots = []
    try:
        for _ in range(100):
            try:
                pages = json.load(urllib.request.urlopen("http://127.0.0.1:%d/json/list" % debug, timeout=2))
                page = next(p for p in pages if p.get("type") == "page")
                break
            except Exception:
                time.sleep(0.1)
        else:
            raise RuntimeError("Chromium did not open its debugging port")
        ws = WebSocket(page["webSocketDebuggerUrl"])
        ws.call("Page.enable")
        ws.call("Emulation.setDeviceMetricsOverride", width=720, height=560, deviceScaleFactor=1, mobile=False)
        ws.call("Page.navigate", url="http://127.0.0.1:%d/index.html" % port)
        t0 = time.time()
        for name, steps in PLAN:
            for action, arg in steps:
                if action == "wait":
                    time.sleep(arg)
                else:
                    key(ws, action, arg)
            hud = ws.call("Runtime.evaluate", expression="document.getElementById('hud').textContent", returnByValue=True)["result"].get("value")
            png = base64.b64decode(ws.call("Page.captureScreenshot", format="png")["data"])
            with open(os.path.join(out, name), "wb") as f:
                f.write(png)
            shots.append({"file": name, "seconds": round(time.time() - t0, 1), "steps": steps, "hud": hud})
            print("%s  %s" % (name, (hud or "").replace("\n", " | ")))
    finally:
        proc.terminate()
        proc.wait(timeout=10)
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    with open(os.path.join(out, "web.json"), "w", encoding="utf-8") as f:
        json.dump({"chromium": subprocess.run([chromium, "--version"], capture_output=True, text=True).stdout.strip(), "shots": shots}, f, indent=1)
        f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
