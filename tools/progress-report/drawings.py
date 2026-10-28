# The report's figures and step-through explainers as HTML strings, each drawn from the gathered data or a file in the tree.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The stepper, the state icons, the anatomy drawing and the chart frame are adapted from Desk's tools/progress-report (explainers.py,
# explainers_fw.py and charts.py; same author, same licence). Colours come from classes in site.css, so every drawing follows the
# page's theme. A step-through explainer is plain HTML that reads without script: every step's text is in an ordered list and the
# stage shows its final state; site.js adds the controls. A figure that is derived rather than recorded refuses to render when the
# derivation does not reproduce the recorded numbers.
import datetime
import json
import os
import re

from mdlite import Raw, esc, inline

HERE = os.path.dirname(os.path.abspath(__file__))

try:
    from zoneinfo import ZoneInfo
    LOCAL = ZoneInfo("America/Toronto")
except Exception:  # no tz database: the offset of the day the repository began (EDT)
    LOCAL = datetime.timezone(datetime.timedelta(hours=-4))


def when(iso):
    return datetime.datetime.fromisoformat(iso.replace("Z", "+00:00")).astimezone(LOCAL)


def hhmm(iso):
    return when(iso).strftime("%H:%M")


def day_label(iso):
    d = when(iso)
    return "%d %s %d" % (d.day, d.strftime("%B"), d.year)


def num(v):
    return "{:,}".format(v) if isinstance(v, int) else str(v)


def titled(svg, title):
    """Adds a <title> and a <desc> (the root's aria-label) to the first drawing, for readers and tools that use them."""
    m = re.match(r"<svg\b[^>]*>", svg)
    if not m or "<title>" in svg[:m.end() + 8]:
        return svg
    desc = re.search(r'aria-label="([^"]*)"', m.group(0))
    return svg[:m.end()] + "<title>%s</title><desc>%s</desc>" % (esc(title), desc.group(1) if desc else esc(title)) + svg[m.end():]


def chart(svg, caption, label, width=640, cls=""):
    """A drawing in a scrolling frame: it never shrinks below `width` and never grows past 1.2 times its own viewBox width."""
    svg = titled(svg, label)
    vb = re.search(r'viewBox="0 0 ([\d.]+) ', svg)
    cap = int(float(vb.group(1)) * 1.2) if vb else 1400
    return ('<figure class="chartfig %s"><div class="chart scroll" tabindex="0" aria-label="%s, scrolls sideways when narrow"><div style="min-width:%dpx;max-width:%dpx">%s</div></div>'
            '<figcaption>%s</figcaption></figure>') % (cls, esc(label), min(width, cap), cap, svg, caption)


def stepper(xid, label, stage, steps, caption, ctx, cls=""):
    items = "".join(
        '<li class="x-step" data-i="%d"><h4 class="x-title">%s</h4><div class="x-text">%s</div></li>' % (
            i, s[0] if isinstance(s[0], Raw) else inline(s[0], ctx), s[1] if isinstance(s[1], Raw) else inline(s[1], ctx))
        for i, s in enumerate(steps))
    return ('<figure class="x %s" data-x="%s" data-n="%d" role="group" aria-label="%s"><div class="x-grid"><div class="x-stage">%s</div>'
            '<div class="x-side"><div class="x-ctl" hidden></div><p class="x-live" aria-live="polite"></p><ol class="x-steps">%s</ol></div></div>'
            '<figcaption>%s</figcaption></figure>') % (cls, xid, len(steps), esc(label), stage, items, caption)


def console(lines, steps, label):
    spans = "".join('<span class="ln" data-from="%d" data-at="%d">%s</span>' % (s, s, esc(l)) for l, s in zip(lines, steps))
    return '<pre class="term x-console" tabindex="0" aria-label="%s"><code>%s</code></pre>' % (esc(label), spans)


# ---- state icons and the legend -------------------------------------------------------------------------------------------------

STATE_LABEL = {
    "done": "Catalogued: merged, CI green, and not hardware-dependent",
    "done-emu": "Catalogued on the host and in emulators; real hardware uncatalogued",
    "review": "In an open pull request: built, not merged",
    "progress": "Partly done, or merged without a green CI run on main yet",
    "planned": "Wish list: planned",
    "research": "Wish list: an idea or research",
    "decision": "Open decision",
}
STATE_SHORT = {"done": "catalogued", "done-emu": "catalogued in emulators", "review": "built, in review", "progress": "partly done",
               "planned": "planned", "research": "idea", "decision": "open decision"}
STATE_ORDER = list(STATE_LABEL)


def state_icon(state, size=14):
    c = 'class="si si-%s"' % state
    if state == "done":
        body = '<circle cx="7" cy="7" r="6.5" class="si-fill"/><path d="M3.8 7.2 L6 9.4 L10.2 4.8" class="si-tick"/>'
    elif state == "done-emu":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring"/><path d="M4.2 7.2 L6.1 9.1 L9.8 5" class="si-ring"/>'
    elif state == "review":
        body = '<rect x="1.5" y="1.5" width="11" height="11" rx="2" class="si-fill"/><path d="M7 4 V7 L9 8.5" class="si-tick"/>'
    elif state == "progress":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring"/><path d="M7 1.2 A5.8 5.8 0 0 1 7 12.8 Z" class="si-fill"/>'
    elif state == "planned":
        body = '<circle cx="7" cy="7" r="5.6" class="si-ring" stroke-dasharray="2.2 1.8"/>'
    elif state == "research":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring" stroke-dasharray="0.1 2.4" stroke-linecap="round"/><text x="7" y="10.2" text-anchor="middle" font-size="8.5" font-weight="700" class="si-text">?</text>'
    else:
        body = '<path d="M7 0.8 L13.2 7 L7 13.2 L0.8 7 Z" class="si-fill"/><rect x="6.2" y="3.6" width="1.6" height="4.4" class="si-tick-fill"/><rect x="6.2" y="9" width="1.6" height="1.6" class="si-tick-fill"/>'
    return '<svg %s width="%d" height="%d" viewBox="0 0 14 14" aria-hidden="true" focusable="false">%s</svg>' % (c, size, size, body)


def used_states(nodes):
    return [s for s in STATE_LABEL if any(n["state"] == s for n in nodes)]


def legend(states):
    return '<ul class="legend" aria-label="Legend">%s</ul>' % "".join("<li>%s%s</li>" % (state_icon(s), esc(STATE_LABEL[s])) for s in states)


# ---- the layer diagram ----------------------------------------------------------------------------------------------------------

# (row, x, w, kind, line1, line2); kind is built (merged, checked on the host or in an emulator), plan or ext (not ours).
LAYER_ROWS = ["Content", "Intent", "Baking", "Presenters", "Platform", "Machine"]
LAYER_BOXES = [
    (0, 0, 2, "built", "fizz-sim · fizz-phys", "tick, hash, input; Fizz 1's physics (#22)"),
    (0, 2, 2, "built", "fizz-ep2", "Ben, the chicken, the level, the replay (#27, #29)"),
    (1, 0, 4, "plan", "fizz-engine: scene, audio and event intent", "docs/SCENE-CONTRACT.md is a sketch (M3)"),
    (2, 0, 4, "plan", "fizz-bake", "Level Studio scripts, art and music lowered per target (M2)"),
    (3, 0, 2, "built", "fizz-present-web · fizz-slice-view", "wasm-bindgen App; the one slice drawing (#31, #32)"),
    (3, 2, 2, "built", "fizz-present-dos · fizz-slice-view", "320x200 frame, HUD, pixel captions (#31)"),
    (4, 0, 1, "built", "Web page, sfx.ts", "canvas, panel, Undertone cluck (#45)"),
    (4, 1, 1, "built", "fizz-hal · fizz-machine", "clock, input, display; probe and settings"),
    (4, 2, 1, "built", "fizz-sfx", "the cluck on OPL2, OPL3, the speaker"),
    (4, 3, 1, "built", "fizz-dos-rt · FIZZ2.EXE", "DPMI, 560 Hz timer, audio, mode 13h"),
    (5, 0, 2, "ext", "A browser, or Node and Bun", "only headless Chromium, Node and Bun have run it"),
    (5, 2, 2, "ext", "DOSBox Staging and CWSDPMI", "headless; a real 486 is untried"),
]


def layers_svg():
    X0, CW, RH, GAP, TOP = 110, 215, 50, 14, 34
    W, H = X0 + 4 * CW + 4, TOP + len(LAYER_ROWS) * (RH + GAP) + 26
    label = ("Layers of Fizz 2 from content to the machine, Web on the left and DOS on the right. Built and merged: the rules in fizz-sim, fizz-phys and fizz-ep2, "
             "the shared slice drawing with both presenters, the Web page, fizz-hal, fizz-machine, fizz-sfx and the DOS runtime. Planned: fizz-engine and fizz-bake. "
             "Not ours: the browser or JavaScript runtime, DOSBox and CWSDPMI.")
    o = ['<svg viewBox="0 0 %d %d" role="img" aria-label="%s" class="diagram">' % (W, H, esc(label)),
         '<text x="%d" y="20" text-anchor="middle" font-size="14" class="d-title">Web</text>' % (X0 + CW),
         '<text x="%d" y="20" text-anchor="middle" font-size="14" class="d-title">DOS</text>' % (X0 + 3 * CW)]
    for i, name in enumerate(LAYER_ROWS):
        o.append('<text x="0" y="%d" font-size="12" class="d-muted">%s</text>' % (TOP + i * (RH + GAP) + RH / 2 + 4, esc(name)))
    for row, col, span, kind, l1, l2 in LAYER_BOXES:
        x, y, w = X0 + col * CW, TOP + row * (RH + GAP), span * CW - 8
        dash = ' stroke-dasharray="6 4"' if kind == "plan" else ""
        o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (x, y, w, RH, kind, dash))
        o.append('<text x="%g" y="%d" text-anchor="middle" font-size="12.5" class="a-t1 a-t1-%s">%s</text>' % (x + w / 2, y + 21, kind, esc(l1)))
        o.append('<text x="%g" y="%d" text-anchor="middle" font-size="11" class="a-t2 a-t2-%s">%s</text>' % (x + w / 2, y + 37, kind, esc(l2)))
    o.append('<text x="%d" y="%d" font-size="12" class="d-text2">Solid: merged, checked on the host or in an emulator. Dashed: planned. Grey: not ours.</text>' % (X0, H - 6))
    o.append("</svg>")
    return "".join(o)


def layers(D):
    cap = ("The layers of <code>docs/ARCHITECTURE.md</code> as built at the end of Milestone 1, with the pull request that added each box. "
           "Intent flows up and lowering flows down: nothing in the content row depends on a presenter.")
    return chart(layers_svg(), cap, "Layer diagram", 880)


# ---- the three-platform parity picture -------------------------------------------------------------------------------------------

def parity(D):
    g = D["facts"]["golden"]
    m = D.get("measured") or {}
    final = g.get("final") or "unknown"
    if m.get("parityHash") and m["parityHash"] != final:
        raise ValueError("measured.json records PARITY %s but the golden transcript ends %s" % (m["parityHash"], final))
    cols = [
        ("Native x86_64", "cargo run -p fizz-ep2", "--example replay", "SSE2 f64"),
        ("Web (WASM)", "node tools/web-replay.mjs", "Node and Bun", "WebAssembly f64"),
        ("DOS (DOSBox)", "FIZZ2.EXE /REPLAY", "headless DOSBox Staging", "x87 at 53 bits, CW 027F"),
    ]
    W, BW, BH = 900, 260, 92
    o = ['<svg viewBox="0 0 %d 330" role="img" class="diagram" aria-label="%s"><defs><marker id="par-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>' % (
        W, esc("The same %d-tick slice script runs natively, as WebAssembly and in DOSBox. All three transcripts equal the golden file, which ends FINAL TICK %d HASH %s." % (g.get("finalTick") or 0, g.get("finalTick") or 0, final)))]
    o.append('<rect x="320" y="6" width="260" height="44" rx="6" class="a-ext"/><text x="450" y="25" text-anchor="middle" font-size="13" class="a-t1 a-t1-ext">SLICE_SCRIPT</text>'
             '<text x="450" y="41" text-anchor="middle" font-size="11" class="a-t2 a-t2-ext">%s ticks of input, in fizz-ep2</text>' % num(g.get("finalTick") or 0))
    for i, (name, cmd, where, fp) in enumerate(cols):
        x = 20 + i * (BW + 30)
        o.append('<line x1="450" y1="50" x2="%d" y2="86" class="d-line" stroke-width="1.3" marker-end="url(#par-arrow)"/>' % (x + BW / 2))
        o.append('<rect x="%d" y="88" width="%d" height="%d" rx="6" class="a-built" stroke-width="1.6"/>' % (x, BW, BH))
        o.append('<text x="%d" y="110" text-anchor="middle" font-size="13.5" class="a-t1 a-t1-built">%s</text>' % (x + BW / 2, esc(name)))
        for k, t in enumerate((cmd, where, fp)):
            o.append('<text x="%d" y="%d" text-anchor="middle" font-size="11.5" class="a-t2 a-t2-built">%s</text>' % (x + BW / 2, 130 + k * 16, esc(t)))
        o.append('<line x1="%d" y1="%d" x2="450" y2="226" class="d-line" stroke-width="1.3" marker-end="url(#par-arrow)"/>' % (x + BW / 2, 88 + BH))
    o.append('<rect x="250" y="228" width="400" height="58" rx="6" class="a-built" stroke-width="2.2"/>'
             '<text x="450" y="250" text-anchor="middle" font-size="12" class="a-t2 a-t2-built">TICK, FINAL and EVENT lines equal the slice golden file</text>'
             '<text x="450" y="273" text-anchor="middle" font-size="14" class="a-t1 a-t1-built num">FINAL TICK %d HASH %s</text>' % (g.get("finalTick") or 0, esc(final)))
    o.append('<text x="450" y="314" text-anchor="middle" font-size="12" class="d-text2">tools/check-parity.sh prints PARITY ok %s</text></svg>' % esc(final))
    cap = ("The parity gate on the slice. The final hash is read from <code>crates/fizz-ep2/slice-replay.txt</code> in the tree; <code>snapshots/measured.json</code> holds the run of "
           "<code>tools/check-parity.sh</code> that printed it (%s), and the build refuses a measured hash that differs from the golden file. Milestone 0's marker replay ended "
           "<code>%s</code> on the same three platforms.") % (esc(m.get("parityWhen", "date not recorded")), esc(D["facts"]["m0"]["golden"].get("final") or "?"))
    return chart("".join(o), cap, "Parity diagram", 820)


# ---- the merged stack -----------------------------------------------------------------------------------------------------------

def pr_stack(D):
    stacks = [s for s in D.get("stacks", []) if len(s["numbers"]) > 1]
    by = {p["number"]: p for p in D["prs"]}
    opens = [o for o in D["open"]["prs"] if not o["isReport"]]
    if not stacks:
        return '<figure class="chartfig"><p class="how">No stack of pull requests has been merged into main.</p></figure>'
    s = stacks[-1]
    nums = s["numbers"]
    BW, BH, GAP, X0 = 300, 46, 10, 150
    H = 30 + len(nums) * (BH + GAP) + 70 + (len(opens) * (BH + GAP))
    W = 900
    label = "The %d stacked pull requests %s, each based on the branch of the one below, merged into main by one merge commit, %s." % (
        len(nums), ", ".join("#%d" % n for n in nums), s["mergeCommit"])
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s"><defs><marker id="st-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>' % (W, H, esc(label))]
    top = 20
    ys = {}
    for i, n in enumerate(reversed(nums)):
        p = by[n]
        y = top + i * (BH + GAP)
        ys[n] = y
        title = p["title"] if len(p["title"]) <= 44 else p["title"][:43] + "…"
        o.append('<g><title>#%d %s: %d commits, +%s lines, CI on the head %s</title><rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-built" stroke-width="1.6"/>'
                 '<text x="%d" y="%d" font-size="13" class="a-t1 a-t1-built">#%d</text><text x="%d" y="%d" font-size="12" class="a-t2 a-t2-built">%s</text>'
                 '<text x="%d" y="%d" font-size="11.5" class="a-t2 a-t2-built">%s · %d commits · +%s</text></g>' % (
                     n, esc(p["title"]), p["commitCount"], num(p["added"]), p["headCi"], X0, y, BW, BH, X0 + 10, y + 19, n, X0 + 46, y + 19, esc(title),
                     X0 + 10, y + 36, esc(p["branch"] or ""), p["commitCount"], num(p["added"])))
        o.append('<text x="%d" y="%d" font-size="11.5" class="d-text2">head %s · CI %s</text>' % (X0 + BW + 12, y + 28, p["head"], "green" if p["headCi"] == "passing" else p["headCi"]))
    ybot = top + len(nums) * (BH + GAP)
    o.append('<rect x="0" y="%d" width="%d" height="40" rx="6" class="a-ext"/><text x="12" y="%d" font-size="13" class="a-t1 a-t1-ext">main</text>' % (ybot + 10, W - 4, ybot + 35))
    o.append('<circle cx="%d" cy="%d" r="8" class="m-merge"/><text x="%d" y="%d" font-size="12" class="d-text2">merge commit %s, %s %s</text>' % (
        X0 + BW / 2, ybot + 30, X0 + BW / 2 + 16, ybot + 35, s["mergeCommit"], day_label(s["landedAt"]), hhmm(s["landedAt"])))
    o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" class="d-line" stroke-width="1.5" marker-end="url(#st-arrow)"/>' % (X0 + BW / 2, ys[nums[0]] + BH, X0 + BW / 2, ybot + 20))
    for i, o_ in enumerate(opens):
        y = ybot + 64 + i * (BH + GAP)
        o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-review" stroke-width="1.6"/><text x="%d" y="%d" font-size="13" class="a-t1 a-t1-review">#%d · open</text>'
                 '<text x="%d" y="%d" font-size="11.5" class="a-t2 a-t2-review">%s · %s · CI %s</text>' % (
                     X0, y, BW, BH, X0 + 10, y + 19, o_["number"], X0 + 10, y + 36, esc(o_["title"][:40]), esc(o_["state"]), "green" if o_["ci"] == "passing" else o_["ci"]))
        o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" class="d-line" stroke-width="1.2" stroke-dasharray="4 3"/>' % (X0 + BW / 2, y, X0 + BW / 2, ybot + 50))
    o.append("</svg>")
    cap = ("The pull requests of the last stack merged into main, top of the stack first, read from <code>gh pr list</code> and <code>git</code>. "
           "Each box counts only the commits between its own head and the head of the pull request below it. Pull requests still open on main are drawn in blue below the line.")
    return chart("".join(o), cap, "Pull request stack", 820)


# ---- progress by milestone ------------------------------------------------------------------------------------------------------

def milestone_progress(D):
    road = D["roadmap"]
    order = [m["id"] for m in road["milestones"]]
    labels = {m["id"]: m["label"] for m in road["milestones"]}
    counts = {}
    for n in road["nodes"]:
        counts.setdefault(n["milestone"], {}).setdefault(n["state"], 0)
        counts[n["milestone"]][n["state"]] += 1
    L, R, bh = 290, 360, 22
    mx = max(sum(c.values()) for c in counts.values())
    H = 60 + len(order) * 34 + 10
    aria = " ".join("%s: %s." % (labels[m], ", ".join("%d %s" % (counts[m][s], STATE_SHORT[s]) for s in STATE_ORDER if s in counts.get(m, {}))) for m in order if m in counts)
    o = ['<svg viewBox="0 0 760 %d" role="img" class="diagram" aria-label="Steps of the plan by milestone and state. %s">' % (H, esc(aria))]
    x = 20
    for s in STATE_ORDER:
        if any(s in c for c in counts.values()):
            o.append('<rect x="%d" y="10" width="12" height="12" rx="2" class="pg pg-%s"/><text x="%d" y="21" font-size="11.5" class="d-text2">%s</text>' % (x, s, x + 17, STATE_SHORT[s]))
            x += 34 + 6.3 * len(STATE_SHORT[s])
    for i, m in enumerate(order):
        y = 50 + i * 34
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">%s</text>' % (L - 10, y + 16, esc(labels[m])))
        cx, unit = L, R / mx
        for s in STATE_ORDER:
            k = counts.get(m, {}).get(s, 0)
            if not k:
                continue
            w = k * unit
            o.append('<g><title>%s: %d %s</title><rect x="%.1f" y="%d" width="%.1f" height="%d" class="pg pg-%s"/><text x="%.1f" y="%d" text-anchor="middle" font-size="12" class="num pg-t">%d</text></g>' % (
                esc(labels[m]), k, STATE_SHORT[s], cx + 1, y, max(w - 2, 2), bh, s, cx + w / 2, y + 16, k))
            cx += w
        o.append('<text x="%.1f" y="%d" font-size="12" class="num d-text2">%d steps</text>' % (cx + 8, y + 16, sum(counts.get(m, {}).values())))
    o.append("</svg>")
    cap = ("Every step of <code>tools/progress-report/roadmap.json</code> by milestone and state. A step stays catalogued only while its pull requests are merged with CI green on main; "
           "the generator moves it down a shelf otherwise and prints a warning.")
    return chart("".join(o), cap, "Progress by milestone", 700)


def roadmap_lists(D):
    road = D["roadmap"]
    cards = []
    for m in road["milestones"]:
        ns = [n for n in road["nodes"] if n["milestone"] == m["id"]]
        lis = "".join('<li>%s<span>%s%s %s</span></li>' % (state_icon(n["state"], 16), esc(n["label"]), esc((" · " + ", ".join("#%d" % k for k in n["prs"])) if n["prs"] else ""),
                                                          '<span class="pill pill-%s">%s</span>' % ({"merged": "merged", "in review": "review", "in progress": "progress"}.get(n["tag"], "planned"), esc(n["tag"])))
                      for n in sorted(ns, key=lambda n: STATE_ORDER.index(n["state"])))
        cards.append('<section class="track"><h3>%s</h3><ul class="state-list">%s</ul><p class="next"><b>Next:</b> %s</p></section>' % (esc(m["label"]), lis, esc(m["next"])))
    return '<div class="glance">%s</div>%s' % ("".join(cards), legend(used_states(road["nodes"])))


def roadmap_table(D, ctx):
    rows = []
    for m in D["roadmap"]["milestones"]:
        for n in [n for n in D["roadmap"]["nodes"] if n["milestone"] == m["id"]]:
            rows.append('<tr><td class="nw">%s</td><td>%s<b>%s</b><div class="sub">%s</div></td><td class="nw">%s</td><td>%s</td></tr>' % (
                esc(m["label"].split(" · ")[0]), state_icon(n["state"], 14) + " ", esc(n["label"]), esc(n["what"]), esc(STATE_SHORT[n["state"]] + " · " + n["tag"]), esc(n["evidence"])))
    return ('<div class="tbl"><table><caption class="sr">Every step of the plan with its state and evidence</caption><thead><tr><th scope="col">Milestone</th><th scope="col">Step</th>'
            '<th scope="col">State</th><th scope="col">Evidence</th></tr></thead><tbody>%s</tbody></table></div>') % "".join(rows)


# ---- the size budget ------------------------------------------------------------------------------------------------------------

def size_budget(D):
    sc = D["facts"]["scraped"]
    m = D.get("measured") or {}
    exe, budget = sc.get("exeBytes"), sc.get("budget")
    if m.get("exeBytes") and exe and m["exeBytes"] != exe:
        raise ValueError("docs/PLATFORM.md says FIZZ2.EXE is %d bytes but the measured run printed SIZE %d" % (exe, m["exeBytes"]))
    rows = [("FIZZ2.EXE, unstripped", exe, "sz-1"), ("RTSMOKE.EXE", m.get("rtsmokeBytes"), "sz-3"), ("Budget in check-dos.sh", budget, "sz-4"),
            ("Plain floppy, room for data", 1390000, "sz-2"), ("1.44 MB floppy, data area", sc.get("floppyData"), "sz-5")]
    rows = [r for r in rows if r[1]]
    mx = max(r[1] for r in rows)
    L, R = 220, 440
    H = 20 + len(rows) * 32 + 30
    o = ['<svg viewBox="0 0 780 %d" role="img" class="diagram" aria-label="%s">' % (H, esc("Bytes: " + "; ".join("%s %s" % (r[0], num(r[1])) for r in rows) + "."))]
    for i, (name, v, c) in enumerate(rows):
        y = 14 + i * 32
        w = max(v / mx * R, 2)
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="12.5" class="a-t1">%s</text><rect x="%d" y="%d" width="%.1f" height="18" rx="2" class="%s"/>'
                 '<text x="%.1f" y="%d" font-size="12" class="num d-text2">%s</text>' % (L - 10, y + 14, esc(name), L, y, w, c, L + w + 6, y + 14, num(v)))
    o.append('<text x="%d" y="%d" font-size="12" class="d-muted">Linear scale. The plain-disk figure is approximate (about 1.39 MB).</text></svg>' % (L, H - 6))
    cap = ("The DOS size budget. The EXE and budget figures are read from <code>docs/PLATFORM.md</code> and <code>tools/check-dos.sh</code>; RTSMOKE.EXE is from the measured run of "
           "<code>tools/build-dos.sh</code> in <code>snapshots/measured.json</code>. FIZZ2.EXE uses %s of the budget." % (
               "%.1f%%" % (100.0 * exe / budget) if exe and budget else "an unknown share"))
    return chart("".join(o), cap, "Size budget", 700)


# ---- the golden replay, step by step --------------------------------------------------------------------------------------------

def held_between(runs, a, b):
    """What the script holds during ticks a+1 to b, as text: each run that overlaps, with its frames inside the window."""
    parts = []
    for r in runs:
        lo, hi = max(r["start"], a), min(r["start"] + r["frames"], b)
        if hi > lo:
            what = " + ".join(r["held"]) or "nothing"
            parts.append("%s for %d" % ("a dash" if r["frames"] == 1 and "Action" in r["held"] else what, hi - lo))
    return ", then ".join(parts) or "the script has no run here"


def replay(D, ctx):
    g = D["facts"]["m0"]["golden"]
    m = D["facts"]["m0"].get("measured") or {}
    text = g.get("text") or ""
    lines = [l for l in text.splitlines() if l.strip()]
    if not lines or lines[0] != "M0 REPLAY 1":
        raise ValueError("the golden transcript does not start with M0 REPLAY 1")
    dos = [l for l in (m.get("dosReplay") or [])]
    want = [l for l in lines if l.startswith(("TICK ", "FINAL "))]
    if dos and [l for l in dos if l.startswith(("TICK ", "FINAL "))] != want:
        raise ValueError("the measured DOS replay in snapshots/measured.json does not match the Milestone 0 transcript in snapshots/m0.json")
    runs = D["facts"]["m0"].get("script") or []
    if runs and sum(r["frames"] for r in runs) != g.get("finalTick"):
        raise ValueError("M0_SCRIPT covers %d ticks but the golden transcript ends at tick %s" % (sum(r["frames"] for r in runs), g.get("finalTick")))
    platform = [l for l in dos if l.startswith(("FPU ", "BIOS ", "VECTORS "))]
    shown = lines + platform
    steps, at = [], []
    steps.append(("Version line", "`M0 REPLAY 1` names the transcript format, version 1. The parity gate requires it from native and Web."))
    at.append(0)
    for l in lines[1:]:
        t = re.match(r"^(TICK|FINAL TICK) (\d+) HASH ([0-9a-f]{16})$", l)
        if not t:
            raise ValueError("unexpected line in the golden transcript: %r" % l)
        tick = int(t.group(2))
        if t.group(1) == "FINAL TICK":
            steps.append(("FINAL at tick %d" % tick, "The last line: `%s`. This is the hash `tools/check-parity.sh` prints after `PARITY ok`." % t.group(3)))
        elif tick == 0:
            steps.append(("Tick 0", "The state before any input: tick, `x`, `y`, `vx`, `vy` and `heading` as `f64` bits, then `dashes`, hashed with FNV-1a to `%s`." % t.group(3)))
        else:
            steps.append(("Tick %d" % tick, "After %d seconds at 60 Hz the state hashes to `%s`. Input in ticks %d to %d: %s." % (
                tick // 60, t.group(3), tick - 59, tick, held_between(runs, tick - 60, tick))))
        at.append(len(steps) - 1)
    for l in platform:
        word = l.split()[0]
        why = {"FPU": "The x87 control word after the replay: `027F` is 53-bit precision with every exception masked. `FNINIT` alone leaves `037F`.",
               "BIOS": "BIOS clock ticks counted while the replay ran. `tools/check-dos.sh` accepts %s; the timer chapter derives the figure." % (D["facts"]["m0"]["scraped"].get("biosRange") or "a narrow range"),
               "VECTORS": "IRQ 0 and IRQ 1 are back to the vectors found at start-up."}[word]
        steps.append(("DOS only: %s" % word, "`%s`. %s" % (l, why)))
        at.append(len(steps) - 1)
    stage = console(shown, at, "The golden transcript, followed by the DOS platform lines")
    cap = ("Every line comes from the Milestone 0 transcript in <code>snapshots/m0.json</code>, followed by the platform lines of Milestone 0's measured DOS run, also in <code>snapshots/m0.json</code>. "
           "The build refuses a measured DOS run whose <code>TICK</code> and <code>FINAL</code> lines differ from the transcript.")
    return stepper("replay", "The golden replay, line by line", stage, steps, cap, ctx)


# ---- the 60 Hz timer that keeps the BIOS clock ------------------------------------------------------------------------------------

PIT_HZ = 1193182


def timer(D, ctx):
    sc = D["facts"]["scraped"]
    m = D["facts"]["m0"].get("measured") or {}
    div = sc.get("pitDivisor")
    if not div:
        raise ValueError("no PIT divisor in docs/PLATFORM.md")
    ticks = D["facts"]["m0"]["golden"].get("finalTick") or 600
    derived = ticks * div // 65536
    seen = m.get("biosTicks")
    if seen is not None and abs(seen - derived) > 1:
        raise ValueError("measured BIOS TICKS %s is not %d, the count %d interrupts of divisor %d give" % (seen, derived, ticks, div))
    acc, rows = 0, []
    for i in range(1, 11):
        acc += div
        chained = acc >= 65536
        if chained:
            acc -= 65536
        rows.append((i, acc, chained))
    cells = "".join('<tr data-from="%d" data-at="%d"><td class="n">%d</td><td class="n">%s</td><td>%s</td></tr>' % (
        k + 1, k + 1, i, num(a), "chain to the BIOS handler" if c else "end of interrupt") for k, (i, a, c) in enumerate(rows))
    stage = ('<div class="px-box"><div class="px-h">IRQ 0 accumulator</div><div class="tbl"><table class="bus"><thead><tr><th scope="col" class="n">IRQ</th><th scope="col" class="n">Accumulator after</th>'
             '<th scope="col">Action</th></tr></thead><tbody>%s</tbody></table></div><p class="px-calc">%s Hz ÷ %s = <b>%.4f Hz</b>; BIOS rate %s ÷ 65,536 = <b>%.4f Hz</b></p></div>') % (
        cells, num(PIT_HZ), num(div), PIT_HZ / div, num(PIT_HZ), PIT_HZ / 65536)
    steps = [("The divisor", "The runtime programs the PIT with divisor %s, so IRQ 0 fires %.4f times a second. The BIOS expects one tick per 65,536 input clocks." % (num(div), PIT_HZ / div))]
    for i, a, c in rows:
        steps.append(("Interrupt %d" % i, "The handler adds %s to the 16-bit accumulator, which is now %s.%s" % (
            num(div), num(a), " It passed 65,536, so the handler wraps it and chains to the BIOS handler, which advances the time of day." if c else " Below 65,536: the handler sends the end of interrupt itself.")))
    steps.append(("Over the replay", "%d interrupts × %s ÷ 65,536 = %d BIOS ticks. The measured DOS run printed `BIOS TICKS %s`." % (ticks, num(div), derived, seen if seen is not None else "?")))
    cap = ("Derived from the divisor in <code>docs/PLATFORM.md</code> and the accumulator in <code>dos/rt/src/timer.rs</code>; the last step compares the arithmetic with the measured "
           "<code>BIOS TICKS</code> line, and the build refuses a measured count more than one tick away.")
    return stepper("timer", "The 60 Hz timer and the BIOS clock", stage, steps, cap, ctx)


# ---- the anatomy diagram --------------------------------------------------------------------------------------------------------

LAYER_H, GAP, LABEL_W, COL_W, TOP = 66, 18, 118, 71, 8
KIND = {"verified": "built", "unverified": "review", "planned": "plan", "external": "ext"}
WORD = {"verified": "built and verified (on the host or in an emulator)", "unverified": "built, not verified", "planned": "planned", "external": "not ours"}


def load_anatomy(name):
    with open(os.path.join(HERE, "anatomy-%s.json" % name), encoding="utf-8") as f:
        return json.load(f)


def validate_anatomy(data, root):
    """Every box has one of the file's statuses, a place on the twelve columns, and (unless it is planned or not ours) evidence."""
    problems = []
    ids = [b["id"] for b in data["boxes"]]
    if len(set(ids)) != len(ids):
        problems.append("duplicate box ids")
    for b in data["boxes"]:
        if b["status"] not in data["statuses"]:
            problems.append("%s: status %r is not one of %s" % (b["id"], b["status"], data["statuses"]))
        if b["status"] in ("verified", "unverified") and not b.get("evidence"):
            problems.append("%s: a built box needs evidence" % b["id"])
        for e in b.get("evidence", []):
            if not re.match(r"^(pr:\d+|commit:[0-9a-f]{7,40}|repo:\S+)$", e):
                problems.append("%s: evidence %r is not pr:N, commit:HASH or repo:PATH" % (b["id"], e))
            elif e.startswith("repo:") and not os.path.exists(os.path.join(root, e[5:])):
                problems.append("%s: evidence %s does not exist in the tree" % (b["id"], e))
        if not (0 <= b["layer"] < len(data["layers"]) and b["col"] >= 0 and b["col"] + b["span"] <= 12):
            problems.append("%s: does not fit the grid" % b["id"])
        for k in ("label", "sub", "text"):
            if not b.get(k):
                problems.append("%s: no %s" % (b["id"], k))
    by = {b["id"]: b for b in data["boxes"]}
    for a in data.get("arrows", []):
        if a["from"] not in by or a["to"] not in by:
            problems.append("arrow %s to %s names a box that does not exist" % (a["from"], a["to"]))
    for layer in range(len(data["layers"])):
        row = sorted((b for b in data["boxes"] if b["layer"] == layer), key=lambda b: b["col"])
        for x, y in zip(row, row[1:]):
            if x["col"] + x["span"] > y["col"]:
                problems.append("%s and %s overlap" % (x["id"], y["id"]))
    return problems


def wrap(s, n):
    words, lines, cur = s.split(), [], ""
    for w in words:
        if cur and len(cur) + 1 + len(w) > n:
            lines.append(cur)
            cur = w
        else:
            cur = (cur + " " + w).strip()
    return lines + ([cur] if cur else [])


def evidence_links(items, ctx):
    out = []
    for e in items:
        kind, _, rest = e.partition(":")
        out.append(inline("[%s](%s)" % ("#" + rest if kind == "pr" else (rest[:7] if kind == "commit" else rest), e), ctx))
    return ", ".join(out)


def anatomy(name, ctx, root, alt_lead):
    data = load_anatomy(name)
    problems = validate_anatomy(data, root)
    if problems:
        raise ValueError("anatomy-%s.json: %s" % (name, "; ".join(problems)))
    n = len(data["layers"])
    W, H = LABEL_W + 12 * COL_W + 6, TOP + n * (LAYER_H + GAP)
    pos = {}
    o = ['<svg viewBox="0 0 %d %d" role="img" aria-label="%s" class="diagram anatomy"><defs><marker id="an-%s" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>' % (
        W, H, esc(alt_lead), name)]
    for i, label in enumerate(data["layers"]):
        y = TOP + i * (LAYER_H + GAP)
        o.append('<text x="0" y="%d" font-size="12" class="d-muted">%s</text>' % (y + LAYER_H / 2 + 4, esc(label if len(label) < 17 else label[:16] + "…")))
    for b in data["boxes"]:
        x, y, w = LABEL_W + b["col"] * COL_W, TOP + b["layer"] * (LAYER_H + GAP), b["span"] * COL_W - 6
        pos[b["id"]] = (x, y, w)
        kind = KIND[b["status"]]
        dash = ' stroke-dasharray="6 4"' if b["status"] == "planned" else ""
        o.append('<g><title>%s: %s. %s</title><rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (esc(b["label"]), WORD[b["status"]], esc(b["sub"]), x, y, w, LAYER_H, kind, dash))
        lab = wrap(b["label"], int(w / 7.4))[:2]
        sub = wrap(b["sub"], max(8, int(w / 6.3)))[:2]
        lines = [(t, "a-t1 a-t1-%s" % kind, 12.5) for t in lab] + [(t, "a-t2 a-t2-%s" % kind, 11) for t in sub]
        y0 = y + LAYER_H / 2 - (len(lines) - 1) * 7.5 + 4
        for k, (t, cls, size) in enumerate(lines):
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="%g" class="%s">%s</text>' % (x + w / 2, y0 + k * 15, size, cls, esc(t)))
        o.append("</g>")
    status = {b["id"]: b["status"] for b in data["boxes"]}
    for a in data.get("arrows", []):
        (x1, y1, w1), (x2, y2, w2) = pos[a["from"]], pos[a["to"]]
        planned = "planned" in (status[a["from"]], status[a["to"]])
        if y1 == y2:
            xa, xb = (x1 + w1, x2) if x1 < x2 else (x1, x2 + w2)
            ya = yb = y1 + LAYER_H / 2
        else:
            xa, xb = x1 + w1 / 2, x2 + w2 / 2
            ya, yb = (y1 + LAYER_H, y2) if y1 < y2 else (y1, y2 + LAYER_H)
        o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.3"%s marker-end="url(#an-%s)"/>' % (xa, ya, xb, yb, ' stroke-dasharray="4 3"' if planned else "", name))
    o.append("</svg>")
    key = ('<ul class="m-key"><li><span class="sw a-sw-built"></span> built and verified on the host or in an emulator</li><li><span class="sw a-sw-review"></span> built, not verified</li>'
           '<li><span class="sw a-sw-plan"></span> planned (dashed)</li>%s</ul>' % ('<li><span class="sw a-sw-ext"></span> not ours</li>' if "external" in data["statuses"] else ""))
    rows = "".join('<tr><th scope="row">%s<div class="sub">%s</div></th><td><span class="an-st an-%s">%s</span></td><td>%s</td><td>%s</td></tr>' % (
        esc(b["label"]), esc(data["layers"][b["layer"]]), b["status"], esc(WORD[b["status"]]), inline(b["text"], ctx), evidence_links(b.get("evidence", []), ctx) or "none")
        for b in data["boxes"])
    table = ('<details class="an-text"><summary>The same diagram as text: every box, its status and its evidence</summary><div class="tbl"><table><caption class="sr">Boxes of the diagram with status and evidence</caption>'
             '<thead><tr><th scope="col">Box</th><th scope="col">Status</th><th scope="col">What it is</th><th scope="col">Evidence</th></tr></thead><tbody>%s</tbody></table></div></details>') % rows
    return ('<figure><div class="chart scroll" tabindex="0" aria-label="%s, scrolls sideways"><div style="min-width:820px">%s</div></div>%s<figcaption>%s</figcaption>%s</figure>') % (
        esc(data["title"]), titled("".join(o), data["title"]), key,
        "Drawn from <code>tools/progress-report/anatomy-%s.json</code>, updated %s. Each box is a file, a crate or a pull request; the table below lists the evidence for each." % (name, esc(data["updated"])), table)


# ---- the crate graph, read from the Cargo.toml files ----------------------------------------------------------------------------

CRATE_ROLE = {  # name: (fill class, what it is)
    "fizz-sim": ("e11", "tick, hash, input"), "fizz-phys": ("e11", "Fizz 1's physics"), "fizz-ep2": ("e11", "Ben, chicken, level"),
    "fizz-view": ("e10", "frame, camera, font"), "fizz-slice-view": ("e10", "the one slice drawing"),
    "fizz-present-dos": ("e10", "320x200 frame, HUD"), "fizz-present-web": ("e10", "wasm-bindgen App"),
    "fizz-hal": ("e14", "clock, input, display"), "fizz-machine": ("e14", "probe, settings, check"),
    "fizz-sfx": ("e12", "cluck on OPL and speaker"), "fizz-music": ("e12", "music OPL streams"),
    "fizz-dos-rt": ("e13", "DPMI, timer, audio, VGA"), "fizz2": ("e13", "FIZZ2.EXE"), "rtsmoke": ("e13", "runtime smoke test"),
}
CRATE_LEGEND = [("e11", "rules"), ("e10", "drawing and presenters"), ("e14", "HAL and machine model"), ("e12", "sound"), ("e13", "the dos/ workspace")]


def crate_deps(root):
    """{crate: (runtime deps, dev deps)} from the Cargo.toml of every package under crates/ and dos/, path dependencies on workspace crates only."""
    out = {}
    for sub in ("crates", "dos"):
        base = os.path.join(root, sub)
        for name in sorted(os.listdir(base)):
            path = os.path.join(base, name, "Cargo.toml")
            if not os.path.isfile(path):
                continue
            with open(path, encoding="utf-8") as f:
                text = f.read()
            pkg = re.search(r'^name\s*=\s*"([^"]+)"', text, re.M).group(1)
            deps, dev, section = [], [], ""
            for line in text.splitlines():
                m = re.match(r"^\[(.+)\]$", line.strip())
                if m:
                    section = m.group(1)
                    continue
                dm = re.match(r'^([A-Za-z0-9_-]+)\s*=\s*\{[^}]*path\s*=', line.strip())
                if dm:
                    (deps if section == "dependencies" else dev if section == "dev-dependencies" else []).append(dm.group(1))
            out[pkg] = (deps, dev)
    return out


def crate_graph(D, root):
    """The workspace's dependencies as a matrix: a row for each package, a mark in the column of each package it uses."""
    deps = crate_deps(root)
    for pkg in deps:
        if pkg not in CRATE_ROLE:
            raise ValueError("crate %s has no role in drawings.CRATE_ROLE" % pkg)
    depth = {}

    def level(p):
        if p not in depth:
            depth[p] = 1 + max([level(q) for q in deps[p][0]] or [-1])
        return depth[p]

    for p in deps:
        level(p)
    order = sorted(deps, key=lambda p: (depth[p], list(CRATE_ROLE).index(p)))
    N, CS, X0, Y0 = len(order), 30, 190, 130
    W, H = max(X0 + N * CS + 20, 760), Y0 + N * CS + 58
    count = sum(len(a) + len(b) for a, b in deps.values())
    label = ("Dependency matrix of the %d packages of the repository, read from their Cargo.toml files. Each row is a package and each mark is a package it uses: %s." % (
        len(deps), "; ".join("%s uses %s" % (p, ", ".join(deps[p][0] + [x + " (tests only)" for x in deps[p][1]]) or "nothing in the workspace") for p in order)))
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s">' % (W, H, esc(label))]
    for i, p in enumerate(order):
        fill, what = CRATE_ROLE[p]
        cx = X0 + i * CS + CS / 2
        o.append('<g transform="translate(%g,%d) rotate(-55)"><text x="0" y="0" font-size="12" class="a-t1">%s</text></g>' % (cx - 4, Y0 - 8, esc(p)))
        y = Y0 + i * CS
        o.append('<rect x="0" y="%d" width="12" height="%d" class="%s"/><text x="18" y="%d" font-size="12.5" class="a-t1">%s</text><text x="%d" y="%d" text-anchor="end" font-size="10.5" class="d-muted">depth %d</text>' % (
            y + 2, CS - 4, fill, y + CS / 2 + 4, esc(p), X0 - 8, y + CS / 2 + 4, depth[p]))
    for r in range(N):
        for c in range(N):
            x, y = X0 + c * CS, Y0 + r * CS
            o.append('<rect x="%d" y="%d" width="%d" height="%d" class="a-ext" opacity="%s"/>' % (x, y, CS, CS, "0.35" if r == c else "0.12"))
    for i, p in enumerate(order):
        run, dev = deps[p]
        for q in run:
            x, y = X0 + order.index(q) * CS, Y0 + i * CS
            o.append('<rect x="%d" y="%d" width="%d" height="%d" class="%s"/>' % (x + 4, y + 4, CS - 8, CS - 8, CRATE_ROLE[q][0]))
        for q in dev:
            x, y = X0 + order.index(q) * CS, Y0 + i * CS
            o.append('<rect x="%d" y="%d" width="%d" height="%d" fill="none" class="d-line" stroke-width="1.5" stroke-dasharray="3 2"/>' % (x + 4, y + 4, CS - 8, CS - 8))
    lx, ly = 0, Y0 + N * CS + 22
    for cls, name in CRATE_LEGEND:
        o.append('<rect x="%d" y="%d" width="12" height="12" class="%s"/><text x="%d" y="%d" font-size="11.5" class="d-text2">%s</text>' % (lx, ly, cls, lx + 17, ly + 11, esc(name)))
        lx += 24 + 7 * len(name) + 20
    o.append('<text x="0" y="%d" font-size="11.5" class="d-text2">A filled mark: the package in the row uses the one in the column. A dashed mark: used by tests only.</text></svg>' % (ly + 34))
    cap = ("The %d packages of the workspace and of <code>dos/</code> with the %d path dependencies in their <code>Cargo.toml</code> files, read when the site is built and sorted by depth; "
           "a package missing from the picture's roles fails the build.") % (len(deps), count)
    return chart("".join(o), cap, "Crate dependency matrix", 700)


# ---- the maze, read from level.rs ----------------------------------------------------------------------------------------------

TILE_CLASS = {".": "e0", "w": "e1", "#": "e8", "B": "e7", "=": "e10", "^": "e12", "D": "e6", "E": "e14"}
TILE_NAME = [("e0", "sky"), ("e1", "wall behind the rooms"), ("e8", "solid fill"), ("e7", "solid block"), ("e10", "one-way platform"), ("e12", "spikes"), ("e6", "red door"), ("e14", "exit")]


def level_facts(root):
    """The slice level's rows (top row first) and named places, read from crates/fizz-ep2/src/level.rs."""
    with open(os.path.join(root, "crates", "fizz-ep2", "src", "level.rs"), encoding="utf-8") as f:
        text = f.read()
    block = text[text.index("const ROWS"):]
    rows = re.findall(r'^\s*"([.w#B=^DE]{32})",', block, re.M)
    if len(rows) != 48:
        raise ValueError("level.rs has %d rows of 32 tiles, not 48" % len(rows))

    def pair(name):
        m = re.search(r"pub const %s: \(f64, f64(?:, f64)?\) = \(([-\d.]+), ([-\d.]+)" % name, text)
        if not m:
            raise ValueError("level.rs has no %s" % name)
        return float(m.group(1)), float(m.group(2))

    return {"rows": rows, "spawn": pair("SPAWN"), "key": pair("KEY"), "chicken": pair("CHICKEN")}


def maze(D, root):
    f = level_facts(root)
    rows, H, W, S = f["rows"], 48, 32, 11
    grid = {(x, H - 1 - i): c for i, row in enumerate(rows) for x, c in enumerate(row)}
    X0, Y0 = 56, 10

    def px(x, y):  # the centre of tile coordinates (x, y), y up
        return X0 + x * S, Y0 + (H - y) * S

    # Ledges: runs of one-way platform tiles, with the shaft they belong to.
    runs = []
    for y in range(H):
        x = 0
        while x < W:
            if grid[(x, y)] == "=":
                x0 = x
                while x < W and grid[(x, y)] == "=":
                    x += 1
                runs.append((y, x0, x - 1))
            else:
                x += 1
    left = sorted([r for r in runs if r[2] < 15], key=lambda r: r[0])
    right = sorted([r for r in runs if r[1] > 15], key=lambda r: r[0])
    door_x, door_ys = 15, [y for y in range(H) if grid[(15, y)] == "D"]
    exit_ys = [y for y in range(H) if grid[(20, y)] == "E"]
    if not door_ys or not exit_ys:
        raise ValueError("level.rs has no red door in column 15 or no exit in column 20")
    spawn, key, chick = f["spawn"], f["key"], f["chicken"]
    chick_row = int(chick[1]) - 1
    key_run = [r for r in left if r[0] == int(key[1]) - 1][0]
    route = [(spawn[0] + 0.35, spawn[1])]
    for y, a, b in left:
        if y == key_run[0]:
            route.append(((a + b) / 2 + 0.5, y + 1))
            route.append((key[0], y + 1))
        elif y < door_ys[0]:
            route.append(((a + b) / 2 + 0.5, y + 1))
    route.append((door_x + 0.5, door_ys[0] + 0.5))
    for y, a, b in right:
        route.append(((a + b) / 2 + 0.5, y + 1))
    route.append((20.5, exit_ys[0]))
    legend_x = X0 + W * S + 28
    W_SVG, H_SVG = legend_x + 300, Y0 + H * S + 24
    label = ("Map of the slice level, 32 tiles wide and 48 tall, read from crates/fizz-ep2/src/level.rs. Ben starts on the floor of the left shaft beside a run of spikes. "
             "One-way ledges climb the left shaft; the chicken walks the long ledge at the third level and the red key lies at its far left end. "
             "Above it the ledges lead to the red door in the wall between the shafts. Through the door the right shaft climbs to a gallery, where the exit stands.")
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s"><defs><marker id="mz-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M0,0 L10,5 L0,10 z" class="d-arrow-hot"/></marker></defs>' % (W_SVG, H_SVG, esc(label))]
    for y in range(H):
        for x in range(W):
            o.append('<rect x="%d" y="%d" width="%d" height="%d" class="%s"/>' % (X0 + x * S, Y0 + (H - 1 - y) * S, S, S, TILE_CLASS[grid[(x, y)]]))
    for y in range(0, H, 6):
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="10.5" class="d-muted num">y %d</text>' % (X0 - 6, Y0 + (H - y) * S - 2, y))
    pts = " ".join("%g,%g" % px(x, y) for x, y in route)
    o.append('<polyline points="%s" class="d-hot" stroke-width="2" stroke-dasharray="5 3" marker-end="url(#mz-arrow)"/>' % pts)
    marks = [("Start", spawn[0] + 0.35, spawn[1] + 0.7, 1), ("Key", key[0], key[1], 2), ("Chicken", chick[0] + 0.45, chick[1] + 0.7, 3), ("Door", door_x + 0.5, door_ys[1], 4), ("Exit", 20.5, exit_ys[0] + 1, 5)]
    for name, x, y, n in marks:
        cx, cy = px(x, y)
        o.append('<circle cx="%g" cy="%g" r="8" class="e15" stroke="#000" stroke-width="1"/><text x="%g" y="%g" text-anchor="middle" font-size="11" class="pg-t">%d</text>' % (cx, cy, cx, cy + 4, n))
    o.append('<text x="%d" y="%d" font-size="13" class="d-title">Key: tile colours</text>' % (legend_x, Y0 + 14))
    for i, (cls, name) in enumerate(TILE_NAME):
        y = Y0 + 26 + i * 20
        o.append('<rect x="%d" y="%d" width="14" height="14" class="%s" stroke="#888" stroke-width="0.5"/><text x="%d" y="%d" font-size="12" class="d-text2">%s</text>' % (legend_x, y, cls, legend_x + 22, y + 12, esc(name)))
    y = Y0 + 26 + len(TILE_NAME) * 20 + 14
    o.append('<text x="%d" y="%d" font-size="13" class="d-title">The route</text>' % (legend_x, y))
    steps = ["Start on the floor, beside the spikes.", "Climb the ledges to the long third one: the key is at its left end and the chicken walks it.",
             "Climb on to the red door, a wall of three tiles at x = 15.", "Walk into the door with the key: it opens.", "Climb the right shaft to the gallery and the exit."]
    yy = y + 20
    for lab, t in zip(["1", "2, 3", "4", "4", "5"], steps):
        words, line, wrapped = t.split(), "", []
        for w in words:
            if len(line) + len(w) > 36:
                wrapped.append(line)
                line = w
            else:
                line = (line + " " + w).strip()
        wrapped.append(line)
        o.append('<text x="%d" y="%d" font-size="12" class="a-t1">%s</text>' % (legend_x, yy, esc(lab)))
        for n, text in enumerate(wrapped):
            o.append('<text x="%d" y="%d" font-size="12" class="d-text2">%s</text>' % (legend_x + 34, yy + n * 14, esc(text)))
        yy += len(wrapped) * 14 + 8
    o.append("</svg>")
    cap = ("The level of <code>crates/fizz-ep2/src/level.rs</code>, drawn from the file's own rows when the site is built, with the places named there (<code>SPAWN</code>, <code>KEY</code>, "
           "<code>CHICKEN</code>, the door tiles and the exit tiles). The dashed route is schematic: straight lines through the middle of each ledge in the order the replay climbs them. "
           "The replay itself, which reaches the exit with the door open, is what <code>tools/check-dos.sh</code> and <code>tools/check-parity.sh</code> compare (<code>EXIT yes</code>).")
    return chart("".join(o), cap, "Map of the slice level", 760)
