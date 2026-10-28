#!/usr/bin/env python3
# Checks the Web page's cluck path in headless Chromium with a stand-in AudioContext, so it can record what is scheduled.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/check-web-sfx.py [--port 9418] [--chromium /usr/bin/chromium]
#
# Needs dist/web from tools/build-web.sh, Bun (to read the expected voices from assets/sfx/cluck.ts) and Chromium; the DevTools
# client is the one in tools/progress-report/capture-web.py. A script injected before the page loads replaces `AudioContext`
# with a recorder, so the page runs its real code against it. The level is not played: the idle world's first cluck is the one the
# slice transcript lists first, at tick 197. The checks are:
#   1. no key pressed: no AudioContext is made and nothing is scheduled, though the cluck at tick 197 has passed;
#   2. a key pressed first: one context, resumed, and the cluck's oscillators match the variant's voices in assets/sfx/cluck.ts;
#   3. M mutes: the button says off, nothing is scheduled, the choice is in localStorage and survives a reload; M turns it on again.
# This proves what the page asks the browser to play. It does not hear anything: the sound has not been listened to.
import functools
import http.server
import importlib.util
import json
import os
import re
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
spec = importlib.util.spec_from_file_location("capture_web", os.path.join(ROOT, "tools", "progress-report", "capture-web.py"))
capture = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

KEYS = {"ShiftLeft": 16, "KeyM": 77}
KEY_TEXT = {"ShiftLeft": "Shift", "KeyM": "m"}

# Replaces AudioContext before the page's scripts run and records what is scheduled into window.__audio.
STUB = """
(() => {
  const log = { made: 0, resumed: 0, osc: [], noise: 0, panners: 0 };
  window.__audio = log;
  const param = (onSet) => ({ value: 0, setValueAtTime(v, t) { if (onSet) onSet(v, t); }, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} });
  const node = (extra = {}) => ({ connect() {}, start() {}, stop() {}, ...extra });
  window.AudioContext = class {
    constructor() { log.made++; this.state = 'suspended'; this.currentTime = 0; this.sampleRate = 44100; this.destination = node(); }
    resume() { log.resumed++; this.state = 'running'; return Promise.resolve(); }
    suspend() { this.state = 'suspended'; return Promise.resolve(); }
    createOscillator() { const o = { type: 'sine', hz: null }; log.osc.push(o);
      return node({ type: 'sine', frequency: param((v) => { if (o.hz === null) o.hz = v; }), start() { o.type = this.type; } }); }
    createGain() { return node({ gain: param() }); }
    createBiquadFilter() { return node({ type: 'lowpass', frequency: param(), Q: param() }); }
    createStereoPanner() { log.panners++; return node({ pan: param() }); }
    createChannelMerger() { return node(); }
    createBufferSource() { log.noise++; return node({ buffer: null, loop: false, playbackRate: param() }); }
    createBuffer(c, n) { return { getChannelData: () => new Float32Array(n) }; }
    createConvolver() { return node({ buffer: null }); }
    createDelay() { return node({ delayTime: param() }); }
    decodeAudioData() { return Promise.reject(new Error('unused')); }
  };
})();
"""


def expected_voices(variant):
    """The tone voices (type, exact Hz) and the noise count of a variant, from the source the page imports."""
    script = (
        "import {CLUCK} from './assets/sfx/cluck.ts'; import {voiceHz,isNoise} from './assets/sfx/voice.ts';"
        "const v=CLUCK[%d]; console.log(JSON.stringify({tones:v.filter(x=>!isNoise(x.n)).map(x=>[x.w??'triangle',voiceHz(x)]),noise:v.filter(x=>isNoise(x.n)).length}))" % variant
    )
    out = subprocess.run(["bun", "-e", script], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def golden_first_cluck():
    for line in open(os.path.join(ROOT, "crates", "fizz-ep2", "slice-replay.txt"), encoding="utf-8"):
        m = re.match(r"EVENT (\d+) cluck (\d+)", line)
        if m:
            return int(m.group(1)), int(m.group(2))
    raise SystemExit("no EVENT line in slice-replay.txt")


class Page:
    def __init__(self, ws, url):
        self.ws, self.url = ws, url

    def js(self, expression):
        return self.ws.call("Runtime.evaluate", expression=expression, returnByValue=True)["result"].get("value")

    def load(self):
        self.ws.call("Page.navigate", url=self.url)
        deadline = time.time() + 20
        while time.time() < deadline:
            # The static page already prints TICK 0, so wait for the module to be running the level.
            if self.js("typeof window.__audio === 'object' && Number(/TICK (\\d+)/.exec(document.getElementById('hud').textContent)[1]) >= 2"):
                return
            time.sleep(0.05)
        raise RuntimeError("the page did not start")

    def key(self, code):
        args = dict(code=code, key=KEY_TEXT[code], windowsVirtualKeyCode=KEYS[code])
        self.ws.call("Input.dispatchKeyEvent", type="keyDown", **args)
        self.ws.call("Input.dispatchKeyEvent", type="keyUp", **args)

    def wait_tick(self, tick, seconds=30):
        deadline = time.time() + seconds
        while time.time() < deadline:
            t = self.js("Number(/TICK (\\d+)/.exec(document.getElementById('hud').textContent)[1])")
            if t is not None and t >= tick:
                return
            time.sleep(0.05)
        raise RuntimeError("tick %d not reached" % tick)

    def audio(self):
        return json.loads(self.js("JSON.stringify(window.__audio)"))

    def button(self):
        return self.js("document.getElementById('sound').textContent")


failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def main(argv):
    port = int(argv[argv.index("--port") + 1]) if "--port" in argv else 9418
    chromium = argv[argv.index("--chromium") + 1] if "--chromium" in argv else "/usr/bin/chromium"
    site = os.path.join(ROOT, "dist", "web")
    if not os.path.exists(os.path.join(site, "index.html")):
        print("check-web-sfx: no dist/web; run tools/build-web.sh", file=sys.stderr)
        return 2
    tick, variant = golden_first_cluck()
    want = expected_voices(variant)

    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass

    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(("127.0.0.1", port), functools.partial(Quiet, directory=site))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    profile = tempfile.mkdtemp(prefix="fz2sfx.", dir="/tmp")
    debug = capture.free_port()
    proc = subprocess.Popen([chromium, "--headless", "--no-sandbox", "--disable-gpu", "--user-data-dir=" + profile, "--window-size=720,560",
                             "--remote-debugging-port=%d" % debug, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=dict(os.environ, TMPDIR=profile))
    try:
        for _ in range(100):
            try:
                pages = json.load(urllib.request.urlopen("http://127.0.0.1:%d/json/list" % debug, timeout=2))
                target = next(p for p in pages if p.get("type") == "page")
                break
            except Exception:
                time.sleep(0.1)
        else:
            raise RuntimeError("Chromium did not open its debugging port")
        ws = capture.WebSocket(target["webSocketDebuggerUrl"])
        ws.call("Page.enable")
        ws.call("Page.addScriptToEvaluateOnNewDocument", source=STUB)
        page = Page(ws, "http://127.0.0.1:%d/index.html" % port)

        # 1. No key: silent, and no context is made.
        page.load()
        page.js("localStorage.clear()")
        page.load()
        check(page.button() == "Sound: on, M to toggle", "the button starts as 'Sound: on, M to toggle'")
        page.wait_tick(tick + 20)
        a = page.audio()
        check(a["made"] == 0 and not a["osc"] and a["noise"] == 0, "before any key the cluck at tick %d makes no context and schedules nothing" % tick)

        # 2. A key first: the cluck plays, as the right variant.
        page.load()
        page.key("ShiftLeft")
        page.wait_tick(tick + 20)
        a = page.audio()
        check(a["made"] == 1 and a["resumed"] >= 1, "the first key makes one context and resumes it")
        got = [[o["type"], o["hz"]] for o in a["osc"]]
        same = len(got) == len(want["tones"]) and all(g[0] == w[0] and abs(g[1] - w[1]) < 1e-6 for g, w in zip(got, want["tones"]))
        check(same and a["noise"] == want["noise"] and a["panners"] in (0, len(want["tones"]) + want["noise"]),
              "the cluck at tick %d schedules variant %d: %d tones and %d noise bursts, placed (a panner on every voice, or none when the chicken is dead centre)" % (tick, variant, len(want["tones"]), want["noise"]))

        # 3. M mutes, and the choice is remembered.
        page.load()
        page.key("ShiftLeft")
        page.key("KeyM")
        check(page.button() == "Sound: off, M to toggle", "M turns the button to off")
        page.wait_tick(tick + 20)
        a = page.audio()
        check(not a["osc"] and a["noise"] == 0, "muted, the cluck schedules nothing")
        check(page.js("localStorage.getItem('fizz2-sound')") == "off", "the mute choice is in localStorage")
        page.load()
        check(page.button() == "Sound: off, M to toggle", "after a reload the page is still muted")
        page.key("KeyM")
        check(page.button() == "Sound: on, M to toggle", "M turns sound on again")
        page.js("localStorage.clear()")
    finally:
        proc.terminate()
        proc.wait(timeout=10)
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    if failures:
        print("check-web-sfx: %d check(s) failed" % len(failures), file=sys.stderr)
        return 1
    print("check-web-sfx: ok (nothing was heard: the stand-in records what the page schedules)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
