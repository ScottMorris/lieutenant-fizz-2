#!/usr/bin/env python3
# Checks the Web page's cluck path in headless Chromium with a stand-in AudioContext, so it can record what is scheduled.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/check-web-sfx.py [--port 9418] [--chromium /usr/bin/chromium]
#
# Needs dist/web from tools/build-web.sh, Bun (to read the expected voices from assets/sfx/effects.ts) and Chromium; the DevTools
# client is the one in tools/progress-report/capture-web.py. A script injected before the page loads replaces `AudioContext`
# with a recorder, so the page runs its real code against it. The level is not played: the idle world's first cluck is the one the
# slice transcript lists first, at tick 197. The checks are:
#   1. no key pressed: no AudioContext is made and nothing is scheduled, though the cluck at tick 197 has passed;
#   2. a key pressed first: one context, resumed, and the cluck's oscillators match the variant's voices in assets/sfx/effects.ts;
#   3. M mutes: the button says off, nothing is scheduled, the choice is in localStorage and survives a reload; M turns it on again;
#   4. the placeholder music (assets/music, web/music.ts): nothing is audible before a key; the first key schedules bar 1's notes
#      (the bass, its partner, the pad and the hats of assets/music/events.ts) and the shared context is made once; M suspends the
#      context; a muted page does not start the song; the Music option (Esc, Options) turns the song off, remembered in the fizz2-options blob, and back on. The mute and music
#      choices are read from the options screen, so the check also proves the page holds and resumes around it.
#   5. the audition list in the H panel (web/index.html, web/main.js): all 15 effects in id order, silent before a key, and each choice
#      schedules exactly that effect's oscillators (a carrier and a modulator for an FM voice) and noise bursts, centred; Play repeats
#      it and a muted page plays nothing. The music is turned off first and the stand-in's clock is moved on between effects.
# The stand-in's clock stays at 0, so only the first fraction of a second of the song is scheduled: bar 1's first beat.
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

KEYS = {"ShiftLeft": 16, "KeyM": 77, "Escape": 27, "ArrowDown": 40, "Enter": 13}
KEY_TEXT = {"ShiftLeft": "Shift", "KeyM": "m", "Escape": "Escape", "ArrowDown": "ArrowDown", "Enter": "Enter"}
NUDGE = 0.01  # seconds: the largest nudge of a part (the partner voices start 2 ms late)
START_LATENCY = 0.1  # seconds: Undertone's loop starts its pattern this long after it is called

# Replaces AudioContext before the page's scripts run and records what is scheduled into window.__audio.
STUB = """
(() => {
  const log = { made: 0, resumed: 0, suspended: 0, state: 'none', osc: [], noise: 0, noiseStarts: [], panners: 0, limiters: 0 };
  window.__audio = log;
  const param = (onSet) => ({ value: 0, setValueAtTime(v, t) { if (onSet) onSet(v, t); }, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} });
  const node = (extra = {}) => ({ connect() {}, start() {}, stop() {}, ...extra });
  window.AudioContext = class {
    constructor() { log.made++; Object.defineProperty(log, 'ctx', { value: this, enumerable: false }); this.state = 'suspended'; log.state = this.state; this.currentTime = 0; this.sampleRate = 44100; this.destination = node(); }
    resume() { log.resumed++; this.state = log.state = 'running'; return Promise.resolve(); }
    suspend() { log.suspended++; this.state = log.state = 'suspended'; return Promise.resolve(); }
    createDynamicsCompressor() { log.limiters++; return node({ threshold: param(), knee: param(), ratio: param(), attack: param(), release: param() }); }
    createOscillator() { const o = { type: 'sine', hz: null }; log.osc.push(o);
      return node({ type: 'sine', frequency: param((v) => { if (o.hz === null) o.hz = v; }), start(t) { o.type = this.type; o.t = t; } }); }
    createGain() { return node({ gain: param() }); }
    createBiquadFilter() { return node({ type: 'lowpass', frequency: param(), Q: param() }); }
    createStereoPanner() { log.panners++; return node({ pan: param() }); }
    createChannelMerger() { return node(); }
    createBufferSource() { log.noise++; return node({ buffer: null, loop: false, playbackRate: param(), start(t) { log.noiseStarts.push(t); } }); }
    createBuffer(c, n) { return { getChannelData: () => new Float32Array(n) }; }
    createConvolver() { return node({ buffer: null }); }
    createDelay() { return node({ delayTime: param() }); }
    decodeAudioData() { return Promise.reject(new Error('unused')); }
  };
})();
"""


def expected_voices(effect_id):
    """The oscillators (type, first Hz) and the noise count of effect `effect_id`, from the source the page imports. A voice with `fm` is a
    carrier and then its modulator (the oscillator's own wave, then the modulator's at ratio times the pitch); pitch steps start at step 0."""
    script = (
        "import {EFFECTS} from './assets/sfx/effects.ts'; import {voiceHz,isNoise,stepOffset} from './assets/sfx/voice.ts';"
        "const v=EFFECTS[%d]; const hz=x=>voiceHz(x)*2**(stepOffset(x,0)/12);"
        "console.log(JSON.stringify({tones:v.filter(x=>!isNoise(x.n)).flatMap(x=>[[x.w??'triangle',hz(x)],...(x.fm?[[x.fm.wave??'sine',hz(x)*x.fm.ratio]]:[])]),"
        "noise:v.filter(x=>isNoise(x.n)).length,voices:v.length,names:null}))" % effect_id
    )
    out = subprocess.run(["bun", "-e", script], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def expected_first_beat():
    """The notes of the song at tick 0, from assets/music: sorted Hz of the tones, and the number of noise bursts."""
    script = (
        "import {levelSong} from './assets/music/songs.ts'; import {noteEvents} from './assets/music/events.ts';"
        "const e=noteEvents(levelSong()).filter(x=>x.tick===0);"
        "console.log(JSON.stringify({hz:e.filter(x=>x.midi!==null).map(x=>440*2**((x.midi-69)/12)).sort((a,b)=>a-b),noise:e.filter(x=>x.midi===null).length,parts:[...new Set(e.map(x=>x.part))]}))"
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

    def open_options(self):
        """Esc opens Paused, Down moves to Options, Enter opens it. The game is held until it is closed again."""
        self.key("Escape")
        self.key("ArrowDown")
        self.key("Enter")

    def close_options(self):
        self.key("Escape")  # back to Paused
        self.key("Escape")  # resume

    def option(self, row):
        """The value a row of the Options screen shows ('On', 'Off', ...), read from the overlay after opening it."""
        self.open_options()
        value = self.js("document.querySelector('#options [data-row=%s] .vt').textContent" % row)
        self.close_options()
        return value

    def toggle(self, row):
        """Clicks a row of the Options screen, which steps a two-way choice to the other value."""
        self.open_options()
        self.js("document.querySelector('#options [data-row=%s]').click()" % row)
        self.close_options()

    def stored(self, field):
        """A field of the remembered blob in localStorage, or None."""
        return self.js("(() => { try { return JSON.parse(localStorage.getItem('fizz2-options'))[%r] ?? null } catch { return null } })()" % field)


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
    beat = expected_first_beat()

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
        page.wait_tick(tick + 20)
        a = page.audio()
        check(a["made"] == 0 and not a["osc"] and a["noise"] == 0, "before any key the cluck at tick %d makes no context and schedules nothing" % tick)
        # Reading an option opens the screen with keys, which are the gesture, so this comes after the silence is proved.
        check(page.option("sound") == "On", "the Sound option starts On")

        # 2. A key first: the cluck plays, as the right variant.
        page.load()
        page.key("ShiftLeft")
        time.sleep(0.5)
        music = page.audio()  # the song's first beat, scheduled by the key; the clock is frozen so nothing more follows
        page.wait_tick(tick + 20)
        a = page.audio()
        check(a["made"] == 1 and a["resumed"] >= 1 and a["limiters"] == 1, "the first key makes one context with its limiter, and resumes it")
        # The cluck's voices are the sources after the song's.
        got = [[o["type"], o["hz"]] for o in a["osc"][len(music["osc"]):]]
        same = len(got) == len(want["tones"]) and all(g[0] == w[0] and abs(g[1] - w[1]) < 1e-6 for g, w in zip(got, want["tones"]))
        check(same and a["noise"] - music["noise"] == want["noise"] and a["panners"] - music["panners"] in (0, want["voices"]),
              "the cluck at tick %d schedules variant %d: %d tones and %d noise bursts, placed (a panner on every voice, or none when the chicken is dead centre)" % (tick, variant, len(want["tones"]), want["noise"]))

        # 3. M mutes, and the choice is remembered.
        page.load()
        page.key("ShiftLeft")
        time.sleep(0.5)
        before = page.audio()
        page.key("KeyM")
        check(page.option("sound") == "Off", "M turns the Sound option to Off")
        page.wait_tick(tick + 20)
        a = page.audio()
        check(a["state"] == "suspended" and a["suspended"] >= 1, "M suspends the shared context, so the song and the cluck stop together")
        check(len(a["osc"]) == len(before["osc"]) and a["noise"] == before["noise"], "muted, the cluck schedules nothing")
        check(page.stored("sound") == "off", "the mute choice is in the Options blob in localStorage")
        page.load()
        check(page.option("sound") == "Off", "after a reload the page is still muted")
        page.key("KeyM")
        check(page.option("sound") == "On", "M turns sound on again")
        page.js("localStorage.clear()")

        # 4. The music. Nothing before a key (check 1 covers the cluck; the song makes no source either); the first key starts bar 1.
        page.load()
        time.sleep(0.5)
        a = page.audio()
        check(a["made"] == 0 and not a["osc"] and a["noise"] == 0 and a["limiters"] == 0, "before any key the song makes no context and schedules nothing")
        page.key("ShiftLeft")
        time.sleep(0.5)
        a = page.audio()
        hz = sorted(o["hz"] for o in a["osc"] if o["t"] is not None and abs(o["t"] - START_LATENCY) < NUDGE)
        check(len(hz) == len(beat["hz"]) and all(abs(x - y) < 1e-6 for x, y in zip(hz, beat["hz"])) and sum(1 for t in a["noiseStarts"] if abs(t - START_LATENCY) < NUDGE) == beat["noise"],
              "the first key schedules bar 1's first beat: %d tones (%s) and %d hat burst" % (len(beat["hz"]), ", ".join("%.1f Hz" % x for x in beat["hz"]), beat["noise"]))
        check(a["made"] == 1 and a["limiters"] == 1, "the clucks and the song share one context")
        check(page.option("music") == "On", "the Music option starts On")
        # The Music option turns only the song off, and the choice is remembered.
        page.toggle("music")
        check(page.option("music") == "Off" and page.stored("music") == "off", "the Music option turns the song off and remembers it")
        check(page.audio()["state"] == "running", "turning the music off leaves the context running for the clucks")
        page.load()
        check(page.option("music") == "Off", "after a reload the music is still off")
        page.key("ShiftLeft")
        time.sleep(0.5)
        a = page.audio()
        check(not a["osc"] and a["noise"] == 0, "with the music off, the first key schedules no song")
        page.toggle("music")
        time.sleep(0.5)
        a = page.audio()
        check(len(a["osc"]) > 0 and page.stored("music") == "on", "the Music option turns the song back on, from bar 1")
        # A muted page does not start the song at its first key. The mute is the old fizz2-sound key, which the page migrates.
        page.js("localStorage.clear()")
        page.js("localStorage.setItem('fizz2-sound', 'off')")
        page.load()
        page.key("ShiftLeft")
        time.sleep(0.5)
        a = page.audio()
        check(not a["osc"] and a["noise"] == 0, "a page that starts muted schedules no song at its first key")
        page.js("localStorage.clear()")

        # 5. The audition list in the H panel plays any effect by its id, on its own. It exists, lists the 15 effects by id, makes no
        # sound before a key, and each choice schedules exactly that effect's voices (a carrier and a modulator for an FM voice).
        page.load()
        check(page.js("document.getElementById('effect-picker').options.length") == 15, "the H panel's audition list has the 15 effects")
        check(page.js("[...document.getElementById('effect-picker').options].map(o => o.value).join(',')") == ",".join(str(i) for i in range(15)),
              "the audition list is in effect-id order, 0 to 14")
        check(page.audio()["made"] == 0 and not page.audio()["osc"], "the audition list makes no sound before a key or click")
        page.key("ShiftLeft")
        time.sleep(0.3)
        page.toggle("music")  # the song's scheduler would add sources as the stand-in's clock is moved on below
        time.sleep(0.3)
        for effect in range(15):
            want_fx = expected_voices(effect)
            page.js("window.__audio.ctx.currentTime += 5")  # the earlier effect is over, so the player's cap of six at once is not in the way
            before = page.audio()
            page.js("(() => { const s = document.getElementById('effect-picker'); s.value = '%d'; s.dispatchEvent(new Event('change')); })()" % effect)
            time.sleep(0.05)
            after = page.audio()
            got = [[o["type"], o["hz"]] for o in after["osc"][len(before["osc"]):]]
            ok = len(got) == len(want_fx["tones"]) and all(g[0] == w[0] and abs(g[1] - w[1]) < 1e-3 * max(1, w[1]) for g, w in zip(got, want_fx["tones"]))
            check(ok and after["noise"] - before["noise"] == want_fx["noise"] and after["panners"] == before["panners"],
                  "audition %d plays its %d oscillators and %d noise bursts, centred" % (effect, len(want_fx["tones"]), want_fx["noise"]))
        # The Play button repeats the chosen effect, and a muted page stays silent.
        page.js("window.__audio.ctx.currentTime += 5")
        before = page.audio()
        page.js("document.getElementById('effect-play').click()")
        time.sleep(0.05)
        check(len(page.audio()["osc"]) > len(before["osc"]), "the Play button plays the chosen effect again")
        page.key("KeyM")
        before = page.audio()
        page.js("document.getElementById('effect-play').click()")
        time.sleep(0.05)
        check(len(page.audio()["osc"]) == len(before["osc"]) and page.audio()["noise"] == before["noise"], "a muted page auditions nothing")
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
