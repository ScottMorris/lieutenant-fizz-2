#!/usr/bin/env python3
# Checks the Web page's Paused and Options screens in headless Chromium: how they open, that they hold the game, what they change and remember.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/check-web-options.py [--port 9420] [--chromium /usr/bin/chromium] [--shots DIR]
#
# Needs dist/web from tools/build-web.sh and Chromium. The page, the fake clock, the key presses and the DevTools client are the ones
# of tools/progress-report/capture-web.py, so the game plays the golden replay's ticks exactly. The checks are:
#   1. Esc opens Paused (Resume, Options, Restart), the dialog and menu roles are there and the focus is inside it; the game's TICK is
#      frozen while it is up however long the clock runs; Esc resumes, the focus returns to where it was, and the first frame after
#      resuming runs no ticks (no catch-up burst);
#   2. the Pause key opens Paused as well and closes it; Options opens from Paused, and Esc goes back to Paused, then to the game;
#   3. each change applies at once and is remembered in one localStorage blob, fizz2-options: captions, keys (the page's hint line),
#      sound (M too), music, placement and motion (which stops the plate's animation); V and M still work; the live region says the change;
#   4. the choices survive a reload; the four legacy keys are migrated into the blob and removed;
#   5. ?audio=classic shows "Classic *" and the override note, is not written to the blob, and changing the row clears both;
#   6. the mouse: hovering selects a row, clicking steps it, clicking an arrow steps it that way, and the panel's "Options (Esc)" button opens
#      the screen;
#   7. prefers-reduced-motion stops the plate shimmer and the bobbing can;
#   8. the golden replay, with Paused and Options opened and changed in the middle of it, ends on the golden hash (e3d629fda94149ab);
#   9. the H panel still lists TICK, HASH, AMMO, KEY, DOOR, DEATHS and EXIT.
# With --shots DIR it also saves screenshots of Paused, Options and the override note.
import functools
import http.server
import importlib.util
import json
import os
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

KEYS = {"Escape": (27, "Escape"), "Pause": (19, "Pause"), "ArrowDown": (40, "ArrowDown"), "ArrowUp": (38, "ArrowUp"), "ArrowLeft": (37, "ArrowLeft"),
        "ArrowRight": (39, "ArrowRight"), "Enter": (13, "Enter"), "KeyM": (77, "m"), "KeyV": (86, "v"), "KeyH": (72, "h")}
GOLDEN_FINAL = "e3d629fda94149ab"
NOTE = "* set by a switch; changing it applies to this run"

PROBE = """
(() => {
  const o = document.getElementById('options');
  const rows = [...o.querySelectorAll('.row')].map((r) => ({ row: r.dataset.row, label: r.querySelector('.lbl').textContent, value: r.querySelector('.vt').textContent, sel: r.classList.contains('sel') }));
  const note = o.querySelector('.opt-note');
  const a = document.activeElement;
  return JSON.stringify({
    open: !o.hidden, title: o.querySelector('h2').textContent, rows, note: note.hidden ? '' : note.textContent,
    focusIn: o.contains(a), active: a.id || a.tagName,
    dialog: o.querySelector('[role=dialog]') !== null && o.querySelector('[role=dialog]').getAttribute('aria-modal') === 'true',
    menu: o.querySelector('[role=menu]') !== null, items: o.querySelectorAll('[role=menuitem]').length,
    status: document.getElementById('options-status').textContent, rm: o.classList.contains('rm'),
  });
})()
"""

failures = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


class Driver:
    def __init__(self, page):
        self.p = page

    def key(self, code):
        vk, key = KEYS[code]
        args = dict(code=code, key=key, windowsVirtualKeyCode=vk)
        self.p.ws.call("Input.dispatchKeyEvent", type="keyDown", **args)
        self.p.ws.call("Input.dispatchKeyEvent", type="keyUp", **args)

    def state(self):
        return json.loads(self.p.js(PROBE))

    def row(self, name):
        return next((r for r in self.state()["rows"] if r["row"] == name), None)

    def value(self, name):
        r = self.row(name)
        return r["value"] if r else None

    def blob(self):
        raw = self.p.js("localStorage.getItem('fizz2-options')")
        return json.loads(raw) if raw else None

    def go_to(self, name):
        """Moves the cursor to a row of the open screen with Up and Down."""
        for _ in range(12):
            r = self.state()["rows"]
            if next(x for x in r if x["sel"])["row"] == name:
                return
            self.key("ArrowDown")
        raise RuntimeError("no row %s" % name)

    def open_options(self):
        self.key("Escape")
        self.key("ArrowDown")
        self.key("Enter")

    def close_all(self):
        while self.state()["open"]:
            self.key("Escape")

    def rect(self, selector):
        return json.loads(self.p.js("(() => { const b = document.querySelector(%r).getBoundingClientRect(); return JSON.stringify([b.left, b.top, b.width, b.height]); })()" % selector))

    def mouse(self, x, y, kind="move"):
        types = {"move": [("mouseMoved", 0)], "click": [("mouseMoved", 0), ("mousePressed", 1), ("mouseReleased", 1)]}[kind]
        for t, clicks in types:
            self.p.ws.call("Input.dispatchMouseEvent", type=t, x=x, y=y, button="left" if clicks else "none", clickCount=clicks)

    def settle(self):
        time.sleep(0.2)  # the live region speaks after a short real-time delay


def run(p, d, per_tick, shots):
    # 1. Esc opens Paused and holds the game.
    p.load(1280, 720)
    for _ in range(30):
        p.step("")
    s = d.state()
    check(not s["open"], "the screens are closed on load")
    p.js("document.getElementById('panel-toggle').focus()")
    t0 = p.tick()
    d.key("Escape")
    s = d.state()
    check(s["open"] and s["title"] == "Paused" and [r["label"] for r in s["rows"]] == ["Resume", "Options", "Restart"], "Esc opens Paused: Resume, Options, Restart")
    check(s["dialog"] and s["menu"] and s["items"] == 3, "it is a modal dialog holding a menu of three items")
    check(s["focusIn"], "the focus moves into the dialog")
    if shots:
        p.shot(shots, "options-paused.png")
    base = p.frames
    for _ in range(40):
        p.frame()
    check(p.tick() == t0, "TICK is frozen while Paused is up (%d after 40 frames)" % p.tick())
    d.key("Escape")
    s = d.state()
    check(not s["open"] and s["active"] == "panel-toggle", "Esc on Paused resumes and the focus returns to the page where it was (%s)" % s["active"])
    # The first frame back only restarts the pacer, however long the clock ran; the next ticks one.
    p.js("window.__frame(%d)" % (p.frames * 1000 // 60 + 600000))
    check(p.tick() == t0, "ten minutes of clock while away and the first frame back run no ticks (no catch-up burst)")
    p.js("window.__frame(%d)" % (p.frames * 1000 // 60 + 600000 + 17))
    check(p.tick() == t0 + 1, "the next frame runs one tick")

    # 2. The Pause key; Options from Paused; Esc steps back.
    p.load(1280, 720)
    d.key("Pause")
    check(d.state()["open"] and d.state()["title"] == "Paused", "the Pause key opens Paused")
    d.key("Pause")
    check(not d.state()["open"], "the Pause key closes it")
    d.key("Escape")
    d.key("ArrowDown")
    d.key("Enter")
    s = d.state()
    labels = [r["label"] for r in s["rows"]]
    check(s["title"] == "Options" and labels[:6] == ["Sound", "Music", "Placement", "Motion", "Captions", "Keys"] and labels[-1] == "Back", "Options opens from Paused: %s" % ", ".join(labels))
    check(d.value("sound") == "On" and d.value("music") == "On" and d.value("placement") == "Enhanced" and d.value("motion") == "System"
          and d.value("captions") == "On" and d.value("keys") == "Keen", "a fresh page starts at the defaults: sound and music On, Enhanced, motion System, captions On, Keen keys")
    if shots:
        p.shot(shots, "options-1280x720.png")
    d.key("Escape")
    check(d.state()["open"] and d.state()["title"] == "Paused", "Esc on Options goes back to Paused")
    d.key("Escape")
    check(not d.state()["open"], "and Esc again resumes")

    # 3. Changes apply at once and are remembered.
    p.js("localStorage.clear()")
    p.load(1280, 720)
    d.open_options()
    d.go_to("captions")
    d.key("ArrowRight")
    d.settle()
    s = d.state()
    check(d.value("captions") == "Off" and (d.blob() or {}).get("captions") == "off" and (d.blob() or {}).get("v") == 1, "Captions Off is remembered in the fizz2-options blob")
    check(s["status"] == "Captions: Off", "the live region says the change ('%s')" % s["status"])
    d.go_to("keys")
    d.key("ArrowRight")
    check(d.value("keys") == "Modern" and "W A S D" in p.js("document.getElementById('hint').textContent"), "Keys Modern applies at once: the hint line lists W A S D")
    d.key("ArrowLeft")
    check(d.value("keys") == "Keen" and "Alt" in p.js("document.getElementById('hint').textContent"), "Left steps back to Keen")
    d.go_to("sound")
    d.key("Enter")
    check(d.value("sound") == "Off" and d.blob()["sound"] == "off", "Enter steps Sound to Off")
    d.go_to("music")
    d.key("ArrowRight")
    check(d.value("music") == "Off" and d.blob()["music"] == "off", "Music Off is remembered")
    d.go_to("placement")
    d.key("ArrowRight")
    check(d.value("placement") == "Classic" and d.blob()["placement"] == "classic", "Placement Classic is remembered")
    d.go_to("motion")
    d.key("ArrowRight")
    check(d.value("motion") == "Reduced" and d.blob()["motion"] == "reduced" and d.state()["rm"], "Motion Reduced is remembered and the overlay stops its own motion")
    d.key("ArrowRight")
    check(d.value("motion") == "Full" and d.blob()["motion"] == "full" and not d.state()["rm"], "Motion Full")
    d.key("ArrowRight")
    check(d.value("motion") == "System" and d.blob()["motion"] == "system", "and back round to System")
    d.go_to("back")
    d.key("Enter")
    check(d.state()["title"] == "Paused", "choosing Back returns to Paused")
    d.close_all()
    # V and M keep working in play.
    before = dict(d.blob())
    d.key("KeyV")
    d.key("KeyM")
    after = d.blob()
    check(after["captions"] == "on" and after["sound"] == "on", "V and M still toggle captions and sound in play (captions %s, sound %s)" % (after["captions"], after["sound"]))
    check(before["captions"] == "off" and before["sound"] == "off", "…from the Off the screen left them in")
    d.key("KeyM")
    d.key("KeyV")

    # 4. A reload keeps the choices; the legacy keys migrate.
    want = dict(d.blob())
    p.load(1280, 720)
    d.open_options()
    check(d.blob() == want and d.value("captions") == "Off" and d.value("sound") == "Off" and d.value("music") == "Off" and d.value("placement") == "Classic",
          "the choices survive a reload")
    p.js("localStorage.clear(); localStorage.setItem('fizz2-layout','modern'); localStorage.setItem('fizz2-captions','off'); "
         "localStorage.setItem('fizz2-motion','reduced'); localStorage.setItem('fizz2-audio','classic'); localStorage.setItem('fizz2-sound','off'); localStorage.setItem('fizz2-music','off')")
    p.load(1280, 720)
    d.open_options()
    check(d.value("keys") == "Modern" and d.value("captions") == "Off" and d.value("motion") == "Reduced" and d.value("placement") == "Classic"
          and d.value("sound") == "Off" and d.value("music") == "Off", "the old single-purpose keys are read on the first load")
    legacy = json.loads(p.js("JSON.stringify(['fizz2-layout','fizz2-captions','fizz2-motion','fizz2-audio','fizz2-sound','fizz2-music'].map(k => localStorage.getItem(k)))"))
    check(d.blob() is not None and legacy == [None] * 6, "they were written into the blob and removed")

    # 5. ?audio= is a per-visit override.
    p.js("localStorage.clear()")
    saved = p.url
    p.url = saved + "?audio=classic"
    p.load(1280, 720)
    d.open_options()
    s = d.state()
    check(d.value("placement") == "Classic *" and s["note"] == NOTE, "?audio=classic shows 'Classic *' and the note under the rows")
    check(d.blob() is None, "and nothing is written to localStorage by it")
    if shots:
        d.go_to("placement")
        p.shot(shots, "options-override.png")
    d.go_to("placement")
    d.key("ArrowRight")
    s = d.state()
    check(d.value("placement") == "Enhanced" and s["note"] == "" and d.blob()["placement"] == "enhanced", "changing the row clears the star and the note, and is remembered")
    p.url = saved

    # 6. The mouse.
    p.js("localStorage.clear()")
    p.load(1280, 720)
    p.js("document.getElementById('panel-toggle').click()")
    check(p.js("document.getElementById('open-options').textContent") == "Options (Esc)", "the H panel has an 'Options (Esc)' button")
    p.js("document.getElementById('open-options').click()")
    s = d.state()
    check(s["open"] and s["title"] == "Options" and p.js("document.getElementById('panel').hidden"), "it opens Options directly and puts the panel away")
    x, y, w, h = d.rect("#options [data-row=music]")
    d.mouse(x + w / 2, y + h / 2)
    check(next(r for r in d.state()["rows"] if r["sel"])["row"] == "music", "hovering a row selects it")
    d.mouse(x + w / 2, y + h / 2, "click")
    check(d.value("music") == "Off" and d.blob()["music"] == "off", "clicking a row steps it")
    ax, ay, aw, ah = d.rect("#options [data-row=music] .arrow.l")
    d.mouse(ax + aw / 2, ay + ah / 2, "click")
    check(d.value("music") == "On", "clicking the left arrow steps it back")
    ax, ay, aw, ah = d.rect("#options [data-row=music] .arrow.r")
    d.mouse(ax + aw / 2, ay + ah / 2, "click")
    check(d.value("music") == "Off", "clicking the right arrow steps it forward")
    bx, by, bw, bh = d.rect("#options [data-row=back]")
    d.mouse(bx + bw / 2, by + bh / 2, "click")
    check(not d.state()["open"], "clicking Back from the panel's Options resumes play")
    d.key("Escape")
    rx, ry, rw, rh = d.rect("#options [data-row=restart]")
    p.js("window.__frame(%d)" % (p.frames * 1000 // 60))
    d.mouse(rx + rw / 2, ry + rh / 2, "click")
    check(not d.state()["open"] and p.tick() == 0, "clicking Restart starts the level again and resumes")

    # 7. Reduced motion stops the shimmer and the bob.
    p.load(1280, 720)
    d.open_options()
    anim = lambda: json.loads(p.js("JSON.stringify(['.row.sel .plate', '.row.sel .bullet'].map(q => getComputedStyle(document.querySelector('#options ' + q)).animationName))"))
    check(anim() == ["opt-plate", "opt-bob"], "the selected row's plate shimmers and its can bobs (%s)" % anim())
    colours = p.js("(() => { const c = getComputedStyle(document.querySelector('#options .row.sel .plate')); return c.backgroundColor + '|' + getComputedStyle(document.querySelector('#options .row.sel .val')).color; })()")
    check(colours.split("|")[0] in ("rgb(255, 85, 255)", "rgb(255, 119, 255)", "rgb(255, 153, 255)") and colours.split("|")[1] == "rgb(26, 0, 26)", "the plate is a shade of the Fizz accent with dark ink (%s)" % colours)
    p.ws.call("Emulation.setEmulatedMedia", features=[{"name": "prefers-reduced-motion", "value": "reduce"}])
    check(anim() == ["none", "none"], "prefers-reduced-motion stops both")
    p.ws.call("Emulation.setEmulatedMedia", features=[{"name": "prefers-reduced-motion", "value": "no-preference"}])

    # 8. The golden replay with the screens used in the middle of it.
    p.load(1280, 720)
    pause_at = next(t for t in range(400, len(per_tick)) if not per_tick[t] and not per_tick[t - 1])
    for t, held in enumerate(per_tick):
        if t == pause_at:
            d.open_options()
            d.go_to("captions")
            d.key("ArrowRight")
            d.go_to("motion")
            d.key("ArrowRight")
            d.close_all()
            # The first frame back only restarts the pacer, at the clock's time then; the frames after it follow from there, so the
            # replay's ticks stay one to a frame.
            base = -(-p.frames * 1000 // 60) + 1
            p.js("window.__frame(%d)" % base)
            count = [0]

            def frame():
                count[0] += 1
                p.js("window.__frame(%d)" % (base + -(-count[0] * 1000 // 60)))

            p.frame = frame
            if p.tick() != pause_at:
                raise RuntimeError("the frame back ran ticks: %d, wanted %d" % (p.tick(), pause_at))
        p.step(held)
    del p.frame
    final = [ln for ln in p.hud().split("\n") if ln.startswith("HASH")][0].split()[1]
    check(p.tick() == len(per_tick) and final == GOLDEN_FINAL, "the replay, paused and changed at tick %d, still ends on %s at tick %d" % (pause_at, GOLDEN_FINAL, p.tick()))

    # 9. The panel keeps its lines.
    lines = [ln.split()[0] for ln in p.hud().split("\n")]
    check(lines == ["TICK", "HASH", "AMMO", "KEY", "DOOR", "DEATHS", "EXIT"], "the H panel still lists TICK, HASH, AMMO, KEY, DOOR, DEATHS and EXIT")

    # A narrow window, for the screenshot.
    if shots:
        p.js("localStorage.clear()")
        p.load(540, 1140)
        d.open_options()
        d.go_to("motion")
        p.shot(shots, "options-540x1140.png")


def main(argv):
    port = int(argv[argv.index("--port") + 1]) if "--port" in argv else 9420
    chromium = argv[argv.index("--chromium") + 1] if "--chromium" in argv else "/usr/bin/chromium"
    shots = os.path.abspath(argv[argv.index("--shots") + 1]) if "--shots" in argv else None
    site = os.path.join(ROOT, "dist", "web")
    if not os.path.exists(os.path.join(site, "index.html")):
        print("check-web-options: no dist/web; run tools/build-web.sh", file=sys.stderr)
        return 2
    if shots:
        os.makedirs(shots, exist_ok=True)

    class Quiet(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass

    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(("127.0.0.1", port), functools.partial(Quiet, directory=site))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    profile = tempfile.mkdtemp(prefix="fz2opt.", dir="/tmp")
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
        run(p, Driver(p), per_tick, shots)
    finally:
        proc.terminate()
        proc.wait(timeout=10)
        httpd.shutdown()
        shutil.rmtree(profile, ignore_errors=True)
    if failures:
        print("check-web-options: %d check(s) failed" % len(failures), file=sys.stderr)
        return 1
    print("check-web-options: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
