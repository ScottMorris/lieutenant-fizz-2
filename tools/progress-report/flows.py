# The input path and frame path diagrams, the captured screenshots and transcript, and the replay script table.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The boxes of the two path diagrams name a file and a function in the tree; `check_paths` refuses a box whose file is missing or no
# longer contains the name, so the drawing cannot outlive the code. Screenshots come from media/ and are shown only with their record
# in media/captures.json (what it shows, how it was made, the commit and the date).
import json
import os

from drawings import chart, esc, num, titled

HERE = os.path.dirname(os.path.abspath(__file__))
MEDIA = os.path.join(HERE, "media")

# (row, label, file, name that must appear in the file)
INPUT_DOS = [
    ("Key pressed", "the 8042 keyboard controller", None, None),
    ("IRQ 1 handler", "dos/rt/src/kbd.rs", "dos/rt/src/kbd.rs", "fn on_irq1"),
    ("Byte ring", "dos/rt/src/ring.rs", "dos/rt/src/ring.rs", "struct"),
    ("Set1Decoder", "fizz-hal scancode.rs", "crates/fizz-hal/src/scancode.rs", "Set1Decoder"),
    ("key_to_buttons", "fizz-present-dos", "crates/fizz-present-dos/src/lib.rs", "fn key_to_buttons"),
]
INPUT_WEB = [
    ("keydown event", "the browser", None, None),
    ("KEYS table", "web/main.js", "web/main.js", "const KEYS"),
    ("App.key(bits, down)", "fizz-present-web", "crates/fizz-present-web/src/lib.rs", "pub fn key"),
]
INPUT_SHARED = [
    ("InputLatch", "fizz-sim input.rs", "crates/fizz-sim/src/input.rs", "pub struct InputLatch"),
    ("InputFrame", "held and pressed", "crates/fizz-sim/src/input.rs", "pub struct InputFrame"),
    ("step(world, frame)", "fizz-hello", "crates/fizz-hello/src/lib.rs", "pub fn step"),
]
FRAME_DOS = [
    ("World", "fizz-hello", "crates/fizz-hello/src/lib.rs", "pub struct World"),
    ("render", "fizz-present-dos", "crates/fizz-present-dos/src/lib.rs", "pub fn render"),
    ("Frame 320x200", "indexed bytes", "crates/fizz-present-dos/src/frame.rs", "pub fn pixels"),
    ("present", "Display::present", "crates/fizz-present-dos/src/lib.rs", "pub fn present"),
    ("DosDisplay", "dos/fizz2/src/hal.rs", "dos/fizz2/src/hal.rs", "fn present"),
    ("vga.rs", "copy to A000h", "dos/rt/src/vga.rs", "pub fn present"),
]
FRAME_WEB = [
    ("World", "fizz-hello", "crates/fizz-hello/src/lib.rs", "pub struct World"),
    ("App getters", "marker_x, marker_y, hash_hex", "crates/fizz-present-web/src/lib.rs", "pub fn marker_x"),
    ("draw(app)", "web/main.js", "web/main.js", "function draw"),
    ("canvas 320x200", "2D context", "web/index.html", "canvas"),
]
PALETTE = ("ega_palette", "16 colours as Rgb6", "crates/fizz-present-dos/src/palette.rs", "fn ega_palette")


def check_paths(root):
    problems = []
    for row in INPUT_DOS + INPUT_WEB + INPUT_SHARED + FRAME_DOS + FRAME_WEB + [PALETTE]:
        _, _, path, needle = row
        if not path:
            continue
        full = os.path.join(root, path)
        if not os.path.exists(full):
            problems.append("%s does not exist" % path)
            continue
        with open(full, encoding="utf-8") as f:
            if needle not in f.read():
                problems.append("%s no longer contains %r" % (path, needle))
    return problems


def _box(o, x, y, w, h, kind, l1, l2):
    o.append('<rect x="%g" y="%g" width="%g" height="%g" rx="5" class="a-%s" stroke-width="1.5"/>' % (x, y, w, h, kind))
    o.append('<text x="%g" y="%g" text-anchor="middle" font-size="12.5" class="a-t1 a-t1-%s">%s</text>' % (x + w / 2, y + h / 2 - 2, kind, esc(l1)))
    o.append('<text x="%g" y="%g" text-anchor="middle" font-size="10.5" class="a-t2 a-t2-%s">%s</text>' % (x + w / 2, y + h / 2 + 13, kind, esc(l2)))


def _row(o, items, y, x0, w, gap, marker):
    xs = []
    for i, (l1, l2, path, _) in enumerate(items):
        x = x0 + i * (w + gap)
        _box(o, x, y, w, 46, "built" if path else "ext", l1, l2)
        if i:
            o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.4" marker-end="url(#%s)"/>' % (x - gap, y + 23, x - 2, y + 23, marker))
        xs.append(x)
    return xs


def arrow_defs(name):
    return '<defs><marker id="%s" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>' % name


def input_path(D, root):
    problems = check_paths(root)
    if problems:
        raise ValueError("the input and frame path diagrams name code that moved: " + "; ".join(problems))
    W, w, gap = 980, 150, 30
    o = ['<svg viewBox="0 0 %d 300" role="img" class="diagram" aria-label="%s">%s' % (W, esc(
        "The input path. On DOS: a key press, the IRQ 1 handler, the byte ring, the Set 1 decoder and key_to_buttons. On Web: a keydown event, the KEYS table in main.js and App.key. "
        "Both feed the InputLatch in fizz-sim, which hands an InputFrame of held and pressed buttons to step once per tick."), arrow_defs("in-arrow"))]
    o.append('<text x="0" y="16" font-size="13" class="a-t1">DOS: FIZZ2.EXE /LIVE</text>')
    dos = _row(o, INPUT_DOS, 26, 0, w, gap, "in-arrow")
    o.append('<text x="0" y="104" font-size="13" class="a-t1">Web: index.html</text>')
    web = _row(o, INPUT_WEB, 114, 0, w, gap, "in-arrow")
    o.append('<text x="0" y="192" font-size="13" class="a-t1">Shared rules, once per 60 Hz tick</text>')
    shared = _row(o, INPUT_SHARED, 202, 2 * (w + gap), w, gap, "in-arrow")
    o.append('<path d="M%g,72 C%g,150 %g,150 %g,200" class="d-line" stroke-width="1.4" fill="none" marker-end="url(#in-arrow)"/>' % (dos[-1] + w / 2, dos[-1] + w / 2, shared[0] + w / 2, shared[0] + w / 2))
    o.append('<path d="M%g,137 C%g,137 %g,180 %g,200" class="d-line" stroke-width="1.4" fill="none" marker-end="url(#in-arrow)"/>' % (web[-1] + w, web[-1] + w + 40, shared[0] + 30, shared[0] + 30))
    o.append('<text x="0" y="290" font-size="11.5" class="d-text2">Blue: code in the tree. Grey: hardware or the browser. A tap shorter than one tick still counts: the rules read held | pressed.</text></svg>')
    cap = ("Each blue box is a file and a name that <code>flows.py</code> finds in the tree when the site is built. The DOS path was exercised by DOSBox's AUTOTYPE in <code>tools/check-dos.sh</code> (<code>MOVED yes</code>); "
           "the Web path by the DevTools key events of the Web screenshots below. A real keyboard has driven neither.")
    return chart("".join(o), cap, "Input path", 900)


def frame_path(D, root):
    problems = check_paths(root)
    if problems:
        raise ValueError("the frame path diagram names code that moved: " + "; ".join(problems))
    W, w, gap = 980, 140, 22
    o = ['<svg viewBox="0 0 %d 210" role="img" class="diagram" aria-label="%s">%s' % (W, esc(
        "The frame path. On DOS: World, render into a 320 by 200 indexed Frame with the 16-colour EGA palette, present through the Display trait to DosDisplay and vga.rs, which copies it to video memory. "
        "On Web: World, the App getters, draw in main.js and the canvas."), arrow_defs("fr-arrow"))]
    o.append('<text x="0" y="16" font-size="13" class="a-t1">DOS: mode 13h, integers only</text>')
    _row(o, FRAME_DOS, 26, 0, w, gap, "fr-arrow")
    _box(o, 1 * (w + gap), 82, w, 40, "built", PALETTE[0], PALETTE[1])
    o.append('<line x1="%g" y1="82" x2="%g" y2="74" class="d-line" stroke-width="1.2" stroke-dasharray="3 3"/>' % (1 * (w + gap) + w / 2, 1 * (w + gap) + w / 2))
    o.append('<text x="0" y="146" font-size="13" class="a-t1">Web: canvas, floats allowed for display</text>')
    _row(o, FRAME_WEB, 156, 0, w, gap, "fr-arrow")
    o.append("</svg>")
    cap = ("Each blue box is a file and a name found in the tree at build time. <code>fizz-present-dos</code> denies <code>clippy::float_arithmetic</code>; the Web page draws from the <code>f64</code> marker position. "
           "The screenshots below are the two ends of these paths.")
    return chart("".join(o), cap, "Frame path", 900)


# ---- captures ------------------------------------------------------------------------------------------------------------------

def load_captures():
    path = os.path.join(MEDIA, "captures.json")
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def check_captures():
    """Every file in media/ has a record, every record has its file and the fields that say how it was made."""
    problems = []
    caps = load_captures()
    names = {c["file"] for c in caps}
    on_disk = {f for f in os.listdir(MEDIA) if f != "captures.json"} if os.path.isdir(MEDIA) else set()
    problems += ["media/%s has no record in captures.json" % f for f in sorted(on_disk - names)]
    problems += ["captures.json lists %s, which is not in media/" % f for f in sorted(names - on_disk)]
    for c in caps:
        for k in ("file", "kind", "track", "source", "shows", "command", "commit", "date"):
            if not c.get(k):
                problems.append("captures.json: %s has no %s" % (c.get("file"), k))
    return problems


def shots(D, names):
    caps = {c["file"]: c for c in load_captures()}
    out = []
    for n in names:
        c = caps.get(n)
        if not c:
            raise ValueError("no capture record for %s" % n)
        hud = (" The page's HUD read <code>%s</code> just before the grab." % esc(c["hud"].replace("\n", " · "))) if c.get("hud") else ""
        out.append('<figure class="shot"><div class="shot-ega"><img src="media/%s" alt="%s" loading="lazy"></div><figcaption>%s%s<span class="src">%s; %s at %s, %s.</span></figcaption></figure>' % (
            esc(n), esc(c["shows"]), esc(c["shows"]), hud, esc(c["source"]), esc(c["command"]), esc(c["commit"]), esc(c["date"])))
    return '<div class="shotrow">%s</div>' % "".join(out)


def dos_transcript(D):
    caps = {c["file"]: c for c in load_captures()}
    c = caps.get("dos-replay.txt")
    if not c:
        raise ValueError("no capture record for dos-replay.txt")
    with open(os.path.join(MEDIA, "dos-replay.txt"), encoding="utf-8") as f:
        text = f.read()
    gold = D["facts"]["golden"].get("text") or ""
    want = [l for l in gold.splitlines() if l.startswith(("TICK ", "FINAL "))]
    got = [l for l in text.splitlines() if l.startswith(("TICK ", "FINAL "))]
    if got != want:
        raise ValueError("media/dos-replay.txt does not match the golden transcript")
    lines = "".join('<span class="ln%s">%s</span>\n' % (" ok" if l.startswith(("FPU", "BIOS", "VECTORS")) else "", esc(l)) for l in text.splitlines())
    return ('<figure><pre class="term" tabindex="0" aria-label="Output of FIZZ2.EXE /REPLAY in DOSBox"><code>C:\\&gt; FIZZ2.EXE /REPLAY\n%s</code></pre><figcaption>The standard output of the run that the DOS screenshots come from, '
            'unedited: %d <code>TICK</code> lines and <code>FINAL</code> equal to <code>crates/fizz-hello/m0-replay.txt</code> (the build checks this), then the three DOS platform lines. %s; %s at %s, %s.</figcaption></figure>') % (
        lines, len([l for l in got if l.startswith("TICK")]), esc(c["source"]), esc(c["command"]), esc(c["commit"]), esc(c["date"]))


def script_table(D):
    runs = D["facts"].get("script") or []
    rows = "".join('<tr><td class="n">%d</td><td class="n">%d</td><td class="n">%d</td><td>%s</td></tr>' % (
        i + 1, r["start"], r["frames"], esc(" + ".join(r["held"]) or "nothing") + (" <b>(a dash: Action held for one frame)</b>" if "Action" in r["held"] and r["frames"] == 1 else ""))
        for i, r in enumerate(runs))
    total = sum(r["frames"] for r in runs)
    return ('<div class="tbl"><table class="script"><caption class="sr">The runs of M0_SCRIPT</caption><thead><tr><th scope="col" class="n">Run</th><th scope="col" class="n">From tick</th>'
            '<th scope="col" class="n">Frames</th><th scope="col">Buttons held</th></tr></thead><tbody>%s</tbody></table></div><p class="how">Parsed from <code>M0_SCRIPT</code> in '
            '<code>crates/fizz-hello/src/lib.rs</code>: %d runs, %s frames in all.</p>') % (rows, len(runs), num(total))
