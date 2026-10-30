#!/usr/bin/env python3
# Captures screenshots of the Web slice in headless Chromium, with real key events and a fake clock, for the progress report's media.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/capture-web.py OUTDIR [--port 9417] [--chromium /usr/bin/chromium] [--explore] [--manual]
#
# Standard library only. Needs dist/web from tools/build-web.sh. dist/web is served on 127.0.0.1:PORT by `http.server` in a thread,
# Chromium runs headless with a remote-debugging port, and this script speaks the DevTools protocol over a minimal WebSocket client.
# A script injected before the page loads replaces `performance.now` and `requestAnimationFrame` with a clock that this script
# advances by exactly one 60 Hz tick per frame (at the whole millisecond where the pacer's integer arithmetic makes the next tick due), so the page plays the same ticks every run: keys are sent with Input.dispatchKeyEvent
# (the page sees ordinary keydown and keyup events with their `code`) between frames, and Page.captureScreenshot saves the picture.
# The `replay` scenes press the keys of `SLICE_SCRIPT` (read from crates/fizz-ep2/src/replay.rs) one tick at a time, so the page
# reaches the golden transcript's states; the HUD's HASH at tick 60 is compared with the golden file's before anything is saved.
# Each shot is listed with its scene, window size, tick and the page's HUD in OUTDIR/web.json. --explore saves a shot every
# 10 ticks of the replay instead, to choose ticks from. --manual takes the manual's screenshots (MANUAL_SCENES, 1280 by 720 and a phone
# window) instead of the archive's, under the names docs/images uses; the page's HUD for each is still listed in web.json.
import base64
import functools
import http.server
import json
import os
import re
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
CODES = {"L": "ArrowLeft", "R": "ArrowRight", "U": "ArrowUp", "D": "ArrowDown", "J": "ControlLeft", "P": "AltLeft", "F": "Space"}
KEYS = {"ArrowRight": 39, "ArrowDown": 40, "ArrowLeft": 37, "ArrowUp": 38, "Space": 32, "ControlLeft": 17, "AltLeft": 18}
KEY_TEXT = {"Space": " ", "ControlLeft": "Control", "AltLeft": "Alt"}
# Keys of the page's own screens, pressed one at a time by the "menu" and "panel" scenes: code -> (virtual key, key).
PAGE_KEYS = {"Escape": (27, "Escape"), "ArrowDown": (40, "ArrowDown"), "Enter": (13, "Enter"), "KeyH": (72, "h")}
MODIFIER = {"ControlLeft": 2, "AltLeft": 1}

# Replaces the page's clock before its scripts run: time moves only when __frame(ms) is called, and sets the clock to `ms`.
CLOCK = """
(() => {
  let now = 0;
  let queue = [];
  performance.now = () => now;
  window.requestAnimationFrame = (cb) => { queue.push(cb); return queue.length; };
  window.__frame = (ms) => { now = ms; const run = queue; queue = []; run.forEach((cb) => cb(now)); };
})();
"""

# Scenes: (file, window width, window height, kind, argument). Kind "hold" takes ([(first tick, end tick, buttons)], ticks to
# play) and presses the buttons for those ticks; kind "replay" plays SLICE_SCRIPT and stops at the tick given, or, for
# ("death", n), n ticks after the HUD first reads DEATHS 1. Kind "ledge" plays the first n ticks of SLICE_SCRIPT, which leave Ben
# standing on the first ledge, then presses buttons as "hold" does, counted from that tick: (n, spans, ticks to play). Kind "menu" plays
# n ticks with nothing held, then presses the page keys in the list one after another ("Escape" opens Paused, then "ArrowDown" and "Enter"
# open Options); kind "panel" plays n ticks and presses H to show the panel; kind "effects" does the same and then lists every entry of the
# "Hear an effect" picker at once, since headless Chromium does not draw a drop-down open.
SCENES = [
    ("web-start.png", 960, 540, "hold", ([], 40)),
    ("web-portrait.png", 390, 780, "hold", ([], 40)),
    ("web-wide.png", 1600, 520, "hold", ([], 40)),
    ("web-look-up.png", 960, 540, "ledge", (150, [(0, 90, "U")], 90)),
    ("web-aim-up.png", 960, 540, "hold", ([(30, 31, "UF")], 36)),
    ("web-puff.png", 960, 540, "replay", 108),
    ("web-cluck.png", 960, 540, "replay", 201),
    ("web-stars.png", 960, 540, "replay", 300),
    ("web-nook.png", 1280, 720, "replay", 1195),
    ("web-nook-boing.png", 1280, 720, "replay", 1354),
    ("web-can.png", 1280, 720, "replay", 1183),
    ("web-hud-snack.png", 1280, 720, "replay", 1530),
    ("web-hud-key.png", 1280, 720, "replay", 1583),
    ("web-effects.png", 1280, 720, "effects", 40),
    ("web-paused.png", 960, 540, "menu", (60, ["Escape"])),
    ("web-options.png", 960, 540, "menu", (60, ["Escape", "ArrowDown", "Enter"])),
]

# The manual's screenshots: (file under docs/images, window width, height, kind, argument), the same kinds.
MANUAL_SCENES = [
    ("web-slice-fire.png", 1280, 720, "hold", ([(38, 39, "F")], 40)),
    ("web-cluck.png", 1280, 720, "replay", 201),
    ("web-stun.png", 1280, 720, "replay", 300),
    ("web-look-up.png", 1280, 720, "ledge", (150, [(0, 60, "U")], 60)),
    ("web-look-down.png", 1280, 720, "ledge", (236, [(0, 90, "D")], 90)),
    ("web-aim-down.png", 1280, 720, "hold", ([(30, 50, "J"), (37, 39, "DF")], 41)),
    ("web-puff.png", 1280, 720, "replay", 108),
    ("web-can.png", 1280, 720, "replay", 1183),
    ("web-hud-key.png", 1280, 720, "replay", 1583),
    ("web-panel.png", 1280, 720, "panel", 40),
    ("web-portrait.png", 540, 1140, "replay", 300),
    ("web-paused.png", 1280, 720, "menu", (40, ["Escape"])),
    ("web-options.png", 1280, 720, "menu", (40, ["Escape", "ArrowDown", "Enter"])),
]


def slice_script():
    """SLICE_SCRIPT as a list of (ticks, buttons), buttons being a string of the letters L R U D J P F."""
    with open(os.path.join(ROOT, "crates", "fizz-ep2", "src", "replay.rs"), encoding="utf-8") as f:
        text = f.read()
    body = text[text.index("pub const SLICE_SCRIPT"):text.index("/// The number of ticks the transcript covers")]
    runs = []
    for n, expr in re.findall(r"^\s*\((\d+), ([^\n]*)\),\s*(?://.*)?$", body, re.M):
        parts = re.findall(r"\b(IDLE|[UDLRJFP])\b", expr.replace(".union", " "))
        runs.append((int(n), "".join("" if p == "IDLE" else p for p in parts)))
    return runs


def golden_hash(tick):
    with open(os.path.join(ROOT, "crates", "fizz-ep2", "slice-replay.txt"), encoding="utf-8") as f:
        for line in f:
            m = re.match(r"TICK (\d+) HASH ([0-9a-f]{16})", line)
            if m and int(m.group(1)) == tick:
                return m.group(2)
    return None


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


class Page:
    def __init__(self, ws, url):
        self.ws, self.url, self.down, self.frames = ws, url, set(), 0

    def js(self, expression):
        return self.ws.call("Runtime.evaluate", expression=expression, returnByValue=True)["result"].get("value")

    def load(self, width, height):
        self.ws.call("Emulation.setDeviceMetricsOverride", width=width, height=height, deviceScaleFactor=1, mobile=False)
        self.ws.call("Page.navigate", url=self.url)
        deadline = time.time() + 20
        while time.time() < deadline:
            if self.js("typeof window.__frame === 'function' && !!document.getElementById('screen')"):
                break
            time.sleep(0.05)
        time.sleep(0.6)  # the module's start, and the debounced fit of the view to the window (80 ms)
        self.down, self.frames = set(), 0
        self.js("window.__frame(0)")  # the first frame only starts the pacer's clock

    def frame(self):
        # Frame k is at ceil(k * 1000 / 60) ms: the first whole millisecond at which tick k is due.
        self.frames += 1
        self.js("window.__frame(%d)" % -(-self.frames * 1000 // 60))

    def tick(self):
        return int(self.js("Number(/TICK (\\d+)/.exec(document.getElementById('hud').textContent)[1])"))

    def hud(self):
        return self.js("document.getElementById('hud').textContent")

    def hold(self, letters):
        want = {CODES[c] for c in letters}
        for code in sorted(self.down - want):
            self.ws.call("Input.dispatchKeyEvent", type="keyUp", code=code, key=KEY_TEXT.get(code, code), windowsVirtualKeyCode=KEYS[code], modifiers=0)
        for code in sorted(want - self.down):
            mods = sum(MODIFIER.get(c, 0) for c in want if c in MODIFIER)
            self.ws.call("Input.dispatchKeyEvent", type="keyDown", code=code, key=KEY_TEXT.get(code, code), windowsVirtualKeyCode=KEYS[code], modifiers=mods)
        self.down = want

    def step(self, letters):
        """One tick with `letters` held: set the keys, run a frame, and check that the page advanced by exactly one tick."""
        before = self.tick()
        self.hold(letters)
        self.frame()
        if self.tick() != before + 1:
            raise RuntimeError("the page moved from tick %d to %d in one frame" % (before, self.tick()))

    def press(self, code):
        vk, key = PAGE_KEYS[code]
        for kind in ("keyDown", "keyUp"):
            self.ws.call("Input.dispatchKeyEvent", type=kind, code=code, key=key, windowsVirtualKeyCode=vk)

    def shot(self, out, name):
        png = base64.b64decode(self.ws.call("Page.captureScreenshot", format="png")["data"])
        with open(os.path.join(out, name), "wb") as f:
            f.write(png)


def main(argv):
    if len(argv) < 2:
        print(__doc__ or "usage: capture-web.py OUTDIR [--port N] [--chromium PATH]", file=sys.stderr)
        return 2
    out = os.path.abspath(argv[1])
    port = int(argv[argv.index("--port") + 1]) if "--port" in argv else 9417
    chromium = argv[argv.index("--chromium") + 1] if "--chromium" in argv else "/usr/bin/chromium"
    explore = "--explore" in argv
    scenes = MANUAL_SCENES if "--manual" in argv else SCENES
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
    proc = subprocess.Popen([chromium, "--headless", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--user-data-dir=" + profile, "--window-size=960,540",
                             "--remote-debugging-port=%d" % debug, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=dict(os.environ, TMPDIR=profile))
    shots = []
    per_tick = [b for n, b in slice_script() for _ in range(n)]
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
        ws.call("Page.addScriptToEvaluateOnNewDocument", source=CLOCK)
        p = Page(ws, "http://127.0.0.1:%d/index.html" % port)
        for name, w, h, kind, arg in scenes:
            if explore and kind != "replay":
                continue
            p.load(w, h)
            if kind == "hold":
                spans, until = arg
                for t in range(until):
                    p.step("".join(b for a, z, b in spans if a <= t < z))
            elif kind in ("menu", "panel", "effects"):
                for _ in range(arg if kind != "menu" else arg[0]):
                    p.step("")
                for code in ["KeyH"] if kind != "menu" else arg[1]:
                    p.press(code)
                    if kind == "menu":
                        p.frame()  # the game is held, so the frame draws the screen and runs no tick
                if kind == "effects":
                    # The page's list is a drop-down, which headless Chromium does not draw open; the capture lists all nineteen
                    # entries at once (the select's size attribute) and chooses effect 14, crunch, without playing it.
                    p.js("(() => { const s = document.getElementById('effect-picker'); s.size = s.options.length; s.selectedIndex = 14; })()")
            elif kind == "ledge":
                lead, spans, until = arg
                for held in per_tick[:lead]:
                    p.step(held)
                for t in range(until):
                    p.step("".join(b for a, z, b in spans if a <= t < z))
            else:
                if p.tick() != 0:
                    raise RuntimeError("the page is at tick %d, not 0" % p.tick())
                stop = None
                for t, held in enumerate(per_tick):
                    p.step(held)
                    if t + 1 == 60:
                        want, got = golden_hash(60), re.search(r"HASH ([0-9a-f]{16})", p.hud()).group(1)
                        if want != got:
                            raise RuntimeError("the page's hash at tick 60 is %s, the golden file's is %s" % (got, want))
                    if explore and (t + 1) % 10 == 0:
                        p.shot(out, "explore-%04d.png" % (t + 1))
                    if isinstance(arg, tuple):
                        if stop is None and "DEATHS 1" in p.hud():
                            stop = t + 1 + arg[1]
                        if stop is not None and t + 1 == stop and not explore:
                            break
                    elif t + 1 == arg and not explore:
                        break
            if explore:
                break
            hud = p.hud()
            p.shot(out, name)
            shots.append({"file": name, "width": w, "height": h, "scene": kind, "tick": p.tick(), "hud": hud})
            print("%s  %dx%d  %s" % (name, w, h, hud.replace("\n", " | ")))
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
