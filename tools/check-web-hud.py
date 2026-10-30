#!/usr/bin/env python3
# Checks the Web page's in-picture HUD in headless Chromium: what it shows, that it follows the game, and that it stays off Ben.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/check-web-hud.py [--port 9419] [--chromium /usr/bin/chromium] [--shots DIR]
#
# Needs dist/web from tools/build-web.sh and Chromium. The page, the fake clock, the key presses and the DevTools client are the
# ones of tools/progress-report/capture-web.py, so the game plays the golden replay's ticks exactly. At 1920x1080, 1280x720 and
# 540x1140 it checks:
#   1. the HUD is there on load, inside the picture, hidden from assistive technology, with Fizz 5, Score 00000, Deaths 0 and the
#      key tray: an empty socket for the red and the blue key, none for the kinds the slice lacks;
#   2. it has no animation or transition, and it sits below the H panel and the toast in the stacking order;
#   3. firing takes the Fizz from 5 to 4 and the number stays light magenta; firing on to zero turns it red, with its black outline;
#   4. through the whole golden replay, on every tick, it agrees with the H panel's AMMO, KEY and DEATHS lines, the red socket shows
#      the key only when it is carried, the box never changes size, and it never touches Ben's sprite; the +N pops are 1, 1, 1, 3
#      and 8, the score ends at 10, and each key pickup rings its socket for twelve ticks;
#   5. the live region says "Out of Fizz", "Red key picked up" and "Blue key picked up" once each, and nothing else (no per-tick chatter);
#   6. the H panel still has all of its lines (TICK, HASH, AMMO, KEY, DOOR, DEATHS, EXIT), which tools/check-web-sfx.py and
#      tools/progress-report/capture.py read.
# With --shots DIR it also saves screenshots of each size at the start, with the key and with the Fizz out.
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

SIZES = [(1920, 1080), (1280, 720), (540, 1140)]

# Everything the checks read from the page, in one call: the HUD's box and text, the key chip, the panel's lines and Ben's sprite,
# all in CSS pixels of the window.
PROBE = """
(() => {
  const r = (el) => { const b = el.getBoundingClientRect(); return [b.left, b.top, b.width, b.height]; };
  const hud = document.getElementById('play-hud');
  const canvas = document.getElementById('screen');
  const c = canvas.getBoundingClientRect();
  const s = c.width / canvas.width;
  const [bx, by, bw, bh] = window.__benBox();
  const shown = (el) => el && getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
  const socks = [...document.querySelectorAll('#hud-tray .sock')].map((el) => [
    el.dataset.kind, shown(el), el.dataset.held === 'true', el.dataset.ring === 'true', shown(el.querySelector('.on')), shown(el.querySelector('.off')),
    el.querySelectorAll('.pips i').length, el.querySelector('.cnt').textContent,
  ]);
  const fizz = document.getElementById('hud-fizz');
  const pre = document.getElementById('hud').textContent;
  return JSON.stringify({
    tick: Number(/TICK (\\d+)/.exec(pre)[1]),
    pre,
    hud: r(hud),
    stage: r(document.getElementById('stage')),
    ben: [c.left + bx * s, c.top + by * s, bw * s, bh * s],
    fizz: fizz.textContent,
    out: fizz.hasAttribute('data-out'),
    fizzColour: getComputedStyle(fizz).color,
    deaths: document.getElementById('hud-deaths').textContent,
    fizzShadow: getComputedStyle(fizz).textShadow,
    score: document.getElementById('hud-score').textContent,
    scoreColour: getComputedStyle(document.getElementById('hud-score')).color,
    word: shown(document.getElementById('hud-score-word')) && getComputedStyle(document.getElementById('hud-score-word')).visibility === 'visible',
    pop: document.getElementById('hud-score-pop').textContent,
    socks,
    side: hud.dataset.side || 'left',
  });
})()
"""

# Counts what the live region is told.
SPY = """
(() => {
  window.__said = [];
  new MutationObserver(() => { const t = document.getElementById('hud-status').textContent; if (t) window.__said.push(t); })
    .observe(document.getElementById('hud-status'), { childList: true, characterData: true, subtree: true });
})()
"""

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def touches(a, b):
    return a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and b[1] < a[1] + a[3]


def inside(inner, outer):
    return inner[0] >= outer[0] - 0.5 and inner[1] >= outer[1] - 0.5 and inner[0] + inner[2] <= outer[0] + outer[2] + 0.5 and inner[1] + inner[3] <= outer[1] + outer[3] + 0.5


def probe(p):
    return json.loads(p.js(PROBE))


def panel_value(pre, name):
    return re.search(r"^%s\s+(.*)$" % name, pre, re.M).group(1).strip()


def run_size(p, w, h, per_tick, shots):
    tag = "%dx%d" % (w, h)
    p.load(w, h)
    p.js(SPY)
    s = probe(p)

    # 1. On load.
    check(s["fizz"] == "5" and s["deaths"] == "0" and not s["out"], "%s: the HUD starts as Fizz 5, Deaths 0" % tag)
    check(s["score"] == "00000" and s["word"] and s["pop"] == "" and s["scoreColour"] == "rgb(255, 255, 85)", "%s: the score starts as five yellow digits, 00000, under the word Score" % tag)
    check(s["hud"][2] > 0 and inside(s["hud"], s["stage"]), "%s: the HUD is inside the picture" % tag)
    check(s["hud"][0] - s["stage"][0] < 40 and s["hud"][1] - s["stage"][1] < 40, "%s: it sits in the top left corner" % tag)
    tray = [(k, shown, held, off) for k, shown, held, ring, on, off, pips, cnt in s["socks"]]
    check(tray == [("red", True, False, True), ("blue", True, False, True), ("green", False, False, False), ("gold", False, False, False)],
          "%s: the tray has an empty socket for the red and the blue key and none for green or gold (%s)" % (tag, tray))
    start_size = s["hud"][2:]
    check(p.js("document.getElementById('play-hud').getAttribute('aria-hidden')") == "true", "%s: the HUD is aria-hidden" % tag)
    check(p.js("var e=document.getElementById('hud-status'); e.getAttribute('role')+'/'+e.getAttribute('aria-live')") == "status/polite", "%s: the live region is a polite status" % tag)
    scale_px = p.js("getComputedStyle(document.getElementById('hud-fizz')).fontSize")
    want = "44px" if h >= 900 else "33px" if h >= 480 else "22px"
    check(scale_px == want, "%s: the digits are Fizz 1's scale for this height (%s)" % (tag, want))
    check(p.js("document.fonts.check('33px \"Fizz Mono\"') && document.fonts.check('22px Fizz')"), "%s: the Fizz fonts are loaded" % tag)
    if shots:
        p.shot(shots, "hud-%s-start.png" % tag)

    # 2. No motion; below the panel and the toast.
    motion = json.loads(p.js("JSON.stringify([...document.querySelectorAll('#play-hud, #play-hud *')].map(e => { const c = getComputedStyle(e); return [c.animationName, c.transitionDuration, c.transitionProperty]; }))"))
    check(all(a == "none" and d.replace("0s", "").replace(",", "").strip() == "" for a, d, _ in motion), "%s: no animation or transition on the HUD" % tag)
    z = json.loads(p.js("JSON.stringify(['play-hud','toast','panel','panel-toggle'].map(i => { const z = getComputedStyle(document.getElementById(i)).zIndex; return z === 'auto' ? 0 : Number(z); }))"))
    check(z[0] < z[1] < z[2] < z[3], "%s: the stacking order is HUD, toast, panel, panel button (%s)" % (tag, z))

    # 3. Fire: 5 to 4, then on to zero.
    for t in range(2):
        p.step("F")
    p.step("")
    s = probe(p)
    check(s["fizz"] == "4" and not s["out"] and "AMMO 4" in s["pre"], "%s: after Fire the HUD shows 4, light magenta" % tag)
    check(s["fizzColour"] == "rgb(255, 85, 255)", "%s: the number is light magenta (the Fizz accent, var(--fizz))" % tag)
    check(s["fizzShadow"].count("rgb(0, 0, 0)") == 8, "%s: the Fizz number has a black outline on all eight sides" % tag)
    check(abs(s["hud"][2] - start_size[0]) < 0.5 and abs(s["hud"][3] - start_size[1]) < 0.5, "%s: the box is the same size after firing" % tag)
    for _ in range(12):
        for _ in range(2):
            p.step("F")
        for _ in range(30):
            p.step("")
        if probe(p)["fizz"] == "0":
            break
    s = probe(p)
    check(s["fizz"] == "0" and s["out"] and s["fizzColour"] == "rgb(255, 85, 85)", "%s: at zero the number turns light red" % tag)
    if shots:
        p.shot(shots, "hud-%s-out.png" % tag)
    time.sleep(0.3)
    said = json.loads(p.js("JSON.stringify(window.__said)"))
    check(said == ["Out of Fizz"], "%s: the live region said only 'Out of Fizz' for five shots (said: %s)" % (tag, said))

    # 4. The golden replay, tick by tick.
    p.load(w, h)
    p.js(SPY)
    key_ticks, flips, nearest = [], 0, 1e9
    last_side, last_key, last_deaths, last_out = "left", False, 0, False
    expected = []
    bad = []
    got_key_shot = False
    pops, rings, last_blue, last_ring = [], {"red": 0, "blue": 0}, False, {"red": False, "blue": False}
    sizes = set()
    for t, held in enumerate(per_tick):
        p.step(held)
        s = probe(p)
        ammo, deaths = int(panel_value(s["pre"], "AMMO")), int(panel_value(s["pre"], "DEATHS"))
        carried = panel_value(s["pre"], "KEY") == "carried"
        socks = {k: (shown, held, ring, on, off, pips, cnt) for k, shown, held, ring, on, off, pips, cnt in s["socks"]}
        red, blue = socks["red"], socks["blue"]
        if s["fizz"] != str(min(99, ammo)) or s["deaths"] != str(min(99, deaths)) or red[1] != carried or s["out"] != (ammo == 0):
            bad.append((s["tick"], "values", s["fizz"], ammo, s["deaths"], deaths, red[1], carried))
        for k in ("red", "blue"):
            sh, hd, rg, on, off, pips, cnt = socks[k]
            if not sh or on != hd or off == hd or (rg and not hd) or pips or cnt:
                bad.append((s["tick"], "socket", k, socks[k]))
            if rg:
                rings[k] += 1
        sizes.add((round(s["hud"][2]), round(s["hud"][3])))
        if s["pop"] and (not pops or pops[-1][0] != s["pop"] or pops[-1][1] < s["tick"] - 1):
            pops.append([s["pop"], s["tick"]])
        elif s["pop"]:
            pops[-1][1] = s["tick"]
        if bool(s["pop"]) == s["word"]:
            bad.append((s["tick"], "the word Score and the +N are not swapped", s["pop"], s["word"]))
        if blue[1] and not last_blue:
            expected.append("Blue key picked up")
            if shots:
                p.shot(shots, "hud-%s-blue.png" % tag)
        last_blue = blue[1]
        if touches(s["hud"], s["ben"]):
            bad.append((s["tick"], "HUD over Ben", s["hud"], s["ben"]))
        gap = max(s["ben"][0] - (s["hud"][0] + s["hud"][2]), s["hud"][0] - (s["ben"][0] + s["ben"][2]), s["ben"][1] - (s["hud"][1] + s["hud"][3]), s["hud"][1] - (s["ben"][1] + s["ben"][3]))
        nearest = min(nearest, gap)
        if s["side"] != last_side:
            flips += 1
            last_side = s["side"]
        if carried and not last_key:
            key_ticks.append(s["tick"])
            expected.append("Red key picked up")
            if shots and not got_key_shot:
                p.shot(shots, "hud-%s-key.png" % tag)
                got_key_shot = True
        if deaths > last_deaths:
            expected.append("Deaths %d" % deaths)
        if ammo == 0 and not last_out:
            expected.append("Out of Fizz")
        last_key, last_deaths, last_out = carried, deaths, ammo == 0
    check(not bad, "%s: on all %d ticks of the replay the HUD agrees with the panel, shows the key only when carried and stays off Ben%s" % (tag, len(per_tick), "" if not bad else " (first: %s)" % (bad[0],)))
    check(bool(key_ticks), "%s: the replay picks the key up (at tick %s)" % (tag, key_ticks))
    check(len(sizes) == 1, "%s: the box is one size on every tick of the replay (%s)" % (tag, sorted(sizes)))
    check([x[0] for x in pops] == ["+1", "+1", "+1", "+3", "+8"], "%s: the snacks pop +1, +1, +1, +3 and +8 (%s)" % (tag, [x[0] for x in pops]))
    check(rings == {"red": 24, "blue": 12}, "%s: each pickup rings its socket for twelve ticks (red twice, blue once: %s)" % (tag, rings))
    for _ in range(70):
        p.step("")
    s = probe(p)
    check(s["score"] == "00010" and s["word"] and s["pop"] == "", "%s: the score settles at 00010 and the word Score comes back" % tag)
    print("      %s: nearest the HUD came to Ben: %.0f px; it changed corner %d time(s)" % (tag, nearest, flips))
    time.sleep(0.3)
    said = json.loads(p.js("JSON.stringify(window.__said)"))
    check(said == expected, "%s: the live region spoke exactly the moments that matter (%s)" % (tag, said))

    # 6. The panel keeps its lines.
    lines = [ln.split()[0] for ln in p.js("document.getElementById('hud').textContent").split("\n")]
    check(lines == ["TICK", "HASH", "AMMO", "KEY", "DOOR", "DEATHS", "EXIT"], "%s: the H panel still lists TICK, HASH, AMMO, KEY, DOOR, DEATHS and EXIT" % tag)


def main(argv):
    port = int(argv[argv.index("--port") + 1]) if "--port" in argv else 9419
    chromium = argv[argv.index("--chromium") + 1] if "--chromium" in argv else "/usr/bin/chromium"
    shots = os.path.abspath(argv[argv.index("--shots") + 1]) if "--shots" in argv else None
    site = os.path.join(ROOT, "dist", "web")
    if not os.path.exists(os.path.join(site, "index.html")):
        print("check-web-hud: no dist/web; run tools/build-web.sh", file=sys.stderr)
        return 2
    if shots:
        os.makedirs(shots, exist_ok=True)

    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass

    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(("127.0.0.1", port), functools.partial(Quiet, directory=site))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    profile = tempfile.mkdtemp(prefix="fz2hud.", dir="/tmp")
    debug = capture.free_port()
    proc = subprocess.Popen([chromium, "--headless", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--user-data-dir=" + profile, "--window-size=960,540",
                             "--remote-debugging-port=%d" % debug, "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=dict(os.environ, TMPDIR=profile))
    per_tick = [b for n, b in capture.slice_script() for _ in range(n)]
    try:
        for _ in range(100):
            try:
                pages = json.load(urllib.request.urlopen("http://127.0.0.1:%d/json/list" % debug, timeout=2))
                target = next(x for x in pages if x.get("type") == "page")
                break
            except Exception:
                time.sleep(0.1)
        else:
            raise RuntimeError("Chromium did not open its debugging port")
        ws = capture.WebSocket(target["webSocketDebuggerUrl"])
        ws.call("Page.enable")
        ws.call("Page.addScriptToEvaluateOnNewDocument", source=capture.CLOCK)
        p = capture.Page(ws, "http://127.0.0.1:%d/index.html" % port)
        for w, h in SIZES:
            run_size(p, w, h, per_tick, shots)
    finally:
        proc.terminate()
        proc.wait(timeout=10)
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    if failures:
        print("check-web-hud: %d check(s) failed" % len(failures), file=sys.stderr)
        return 1
    print("check-web-hud: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
