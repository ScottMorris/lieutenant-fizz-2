# The report's charts, drawn as inline SVG in the EGA palette from the gathered data: lines and tests per crate, pull request sizes,
# CI results and the floppy budget.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Each chart makes one claim, labels its bars directly and repeats its numbers in text, so nothing depends on colour alone. The bar
# style follows Desk's tools/progress-report/charts.py (same author, same licence).
import os
import struct

from drawings import chart, num
from mdlite import esc

CRATE_ORDER = ["fizz-sim", "fizz-phys", "fizz-ep2", "fizz-hal", "fizz-machine", "fizz-menu", "fizz-view", "fizz-slice-view", "fizz-present-dos", "fizz-present-web", "fizz-sfx", "fizz-music", "fizz-dos-rt", "fizz2", "rtsmoke"]
WHERE = {"fizz-sim": "crates/fizz-sim", "fizz-phys": "crates/fizz-phys", "fizz-ep2": "crates/fizz-ep2", "fizz-hal": "crates/fizz-hal",
         "fizz-view": "crates/fizz-view", "fizz-slice-view": "crates/fizz-slice-view", "fizz-present-dos": "crates/fizz-present-dos",
         "fizz-present-web": "crates/fizz-present-web", "fizz-machine": "crates/fizz-machine", "fizz-menu": "crates/fizz-menu", "fizz-sfx": "crates/fizz-sfx", "fizz-music": "crates/fizz-music", "fizz-dos-rt": "dos/rt", "fizz2": "dos/fizz2", "rtsmoke": "dos/rtsmoke"}


def bars(rows, title, label_w=170, bar_w=420, unit=""):
    """rows: [(label, value, css class)]. A horizontal bar chart with the value at the end of each bar."""
    mx = max([v for _, v, _ in rows] + [1])
    H = 14 + len(rows) * 28 + 10
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s">' % (label_w + bar_w + 120, H, esc("%s: %s." % (title, "; ".join("%s %s%s" % (l, num(v), unit) for l, v, _ in rows))))]
    for i, (lab, v, c) in enumerate(rows):
        y = 10 + i * 28
        w = max(v / mx * bar_w, 1.5)
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="12.5" class="a-t1">%s</text><rect x="%d" y="%d" width="%.1f" height="18" class="%s"/>'
                 '<text x="%.1f" y="%d" font-size="12" class="num d-text2">%s%s</text>' % (label_w - 10, y + 14, esc(lab), label_w, y, w, c, label_w + w + 6, y + 14, num(v), unit))
    o.append("</svg>")
    return "".join(o)


def loc_by_crate(D):
    loc = D["facts"]["loc"]
    rows = [(c, loc.get(c, 0), "e11" if c.startswith(("fizz-sim", "fizz-phys", "fizz-ep2")) else ("e10" if c.startswith("fizz-present") else ("e14" if c == "fizz-hal" else "e13"))) for c in CRATE_ORDER]
    rows += [("web/ (HTML, JS, CSS)", loc.get("web", 0), "e7"), ("tools/ scripts", loc.get("tools", 0), "e7"), ("docs/", loc.get("docs", 0), "e7")]
    cap = ("Newlines per Rust package (cyan: rules; green: presenters; yellow: the HAL; magenta: the DOS workspace) and in the page, scripts and documents, "
           "counted in the tree of <code>main</code> by <code>gather.py</code>. Paths: %s.") % ", ".join("<code>%s</code>" % WHERE[c] for c in CRATE_ORDER)
    return chart(bars(rows, "Lines per package"), cap, "Lines of source per package", 640)


def tests_by_crate(D):
    t = D["facts"]["tests"]
    rows = [(c, t.get(c, 0), "e10") for c in CRATE_ORDER]
    cap = ("Lines that are <code>#[test]</code> in each package's Rust files, %s in all. The DOS workspace (<code>fizz-dos-rt</code>, <code>fizz2</code>, <code>rtsmoke</code>) has none: "
           "it is checked by running <code>FIZZ2.EXE</code> and <code>RTSMOKE.EXE</code> in DOSBox through <code>tools/check-dos.sh</code>.") % num(sum(t.values()))
    return chart(bars(rows, "Test declarations per package"), cap, "Test declarations per package", 640)


def pr_sizes(D):
    prs = sorted(D["prs"], key=lambda p: p["number"])
    rows = [("#%d %s" % (p["number"], p["title"][:30] + ("…" if len(p["title"]) > 30 else "")), p["added"], "e11" if p["mergeCommit"] == (D["stacks"][0]["mergeCommit"] if D.get("stacks") else "") else "e14") for p in prs]
    cap = ("Lines added by each merged pull request's own commits (<code>git diff --numstat</code> from the head of the pull request below it, or from <code>main</code>, to its head). "
           "Cyan bars landed in the first merge commit of a stack; yellow bars landed later.")
    return chart(bars(rows, "Lines added per pull request", label_w=280, bar_w=360), cap, "Pull request sizes", 680)


def ci_results(D):
    prs = sorted(D["prs"], key=lambda p: p["number"])
    W, RH = 760, 26
    H = 40 + len(prs) * RH + 10
    word = {"passing": "green", "failing": "failed", "pending": "running", "unknown": "not found", "none": "no checks", "cancelled": "cancelled"}
    cls = {"passing": "e10", "failing": "e12", "pending": "e14", "cancelled": "e7"}
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s">' % (W, H, esc("CI per merged pull request: " + "; ".join("#%d head %s, main %s" % (p["number"], word.get(p["headCi"], p["headCi"]), word.get(p["ci"], p["ci"])) for p in prs) + "."))]
    o.append('<text x="300" y="20" text-anchor="middle" font-size="12.5" class="a-t1">Checks on the pull request head</text><text x="560" y="20" text-anchor="middle" font-size="12.5" class="a-t1">CI on main</text>')
    for i, p in enumerate(prs):
        y = 32 + i * RH
        o.append('<text x="0" y="%d" font-size="12.5" class="a-t1">#%d</text><text x="40" y="%d" font-size="12" class="d-text2">%s</text>' % (y + 14, p["number"], y + 14, esc(", ".join(p["checks"]) or "")))
        for x, state, extra in ((240, p["headCi"], ""), (500, p["ci"], " on " + p["mergeCommit"])):
            o.append('<rect x="%d" y="%d" width="120" height="18" class="%s"/><text x="%d" y="%d" text-anchor="middle" font-size="12" class="pg-t">%s</text><text x="%d" y="%d" font-size="11.5" class="d-text2">%s</text>' % (
                x, y, cls.get(state, "e8"), x + 60, y + 13, esc(word.get(state, state)), x + 128, y + 13, esc(extra)))
    o.append("</svg>")
    cap = ("Left: every check on each merged pull request's head commit, from the <code>statusCheckRollup</code> of <code>gh pr list</code>. Right: the CI workflow's conclusion on the merge commit that "
           "brought it into <code>main</code>, from <code>gh run list --branch main</code>. Source: %s.") % esc(D["sources"]["gh"])
    return chart("".join(o), cap, "CI results", 700)


def floppy(D):
    """The 1.44 MB data area as one stacked bar: the FreeDOS system, the save reserve, CWSDPMI, FIZZ2.EXE and what is left."""
    sc, m = D["facts"]["scraped"], D.get("measured") or {}
    total = sc.get("floppyData")
    parts = [("FreeDOS system, about %d KB" % sc["freedosKB"], sc["freedosKB"] * 1024, "e8", False) if sc.get("freedosKB") else None,
             ("Save and config reserve, %d KB" % sc["saveKB"], sc["saveKB"] * 1024, "e5", True) if sc.get("saveKB") else None,
             ("CWSDPMI.EXE", m.get("cwsdpmiBytes"), "e6", False) if m.get("cwsdpmiBytes") else None,
             ("FIZZ2.EXE, unstripped", sc.get("exeBytes"), "e14", False) if sc.get("exeBytes") else None]
    parts = [p for p in parts if p]
    if not total or not parts:
        raise ValueError("the floppy figures are missing from docs/PLATFORM.md or snapshots/measured.json")
    used = sum(p[1] for p in parts)
    free = total - used
    if free < 0:
        raise ValueError("the parts add up to more than the floppy's data area")
    W, X0, BW, Y = 880, 10, 860, 40
    o = ['<svg viewBox="0 0 %d 230" role="img" class="diagram" aria-label="%s">' % (W, esc("The %s-byte data area of a 1.44 MB floppy: %s, and %s bytes free. The provisional budget for FIZZ2.EXE is %s bytes." % (
        num(total), "; ".join("%s %s bytes" % (p[0], num(p[1])) for p in parts), num(free), num(sc.get("budget") or 0))))]
    o.append('<defs><pattern id="fl-dither" width="2" height="2" patternUnits="userSpaceOnUse"><rect width="1" height="1" class="e13"/><rect x="1" y="1" width="1" height="1" class="e13"/></pattern>'
             '<pattern id="fl-free" width="2" height="2" patternUnits="userSpaceOnUse"><rect width="1" height="1" class="e8"/><rect x="1" y="1" width="1" height="1" class="e8"/></pattern></defs>')
    x = X0
    o.append('<text x="%d" y="16" font-size="13" class="a-t1">1.44 MB floppy, %s bytes of data area</text>' % (X0, num(total)))
    for i, (lab, v, c, dither) in enumerate(parts):
        w = v / total * BW
        o.append('<rect x="%.1f" y="%d" width="%.1f" height="40" %s/>' % (x, Y, max(w, 1.5), 'fill="url(#fl-dither)"' if dither else 'class="%s"' % c))
        ly = 112 + i * 20
        o.append('<line x1="%.1f" y1="%d" x2="%.1f" y2="%d" class="d-line"/><text x="%.1f" y="%d" font-size="12" class="d-text2">%s: %s bytes</text>' % (x + w / 2, Y + 40, x + w / 2, ly - 4, x + w / 2 + 4, ly, esc(lab), num(v)))
        x += w
    o.append('<rect x="%.1f" y="%d" width="%.1f" height="40" fill="url(#fl-free)"/>' % (x, Y, X0 + BW - x))
    o.append('<text x="%.1f" y="%d" text-anchor="middle" font-size="12" class="a-t1">free: %s bytes</text>' % ((x + X0 + BW) / 2, Y + 60, num(free)))
    if sc.get("budget"):
        bx = X0 + sc["budget"] / total * BW
        o.append('<line x1="%.1f" y1="%d" x2="%.1f" y2="%d" class="d-hot" stroke-width="2" stroke-dasharray="4 3"/><text x="%.1f" y="%d" text-anchor="end" font-size="12" class="a-t1">BUDGET %s for FIZZ2.EXE</text>' % (
            bx, Y - 8, bx, Y + 48, bx - 4, Y - 10, num(sc["budget"])))
    o.append("</svg>")
    cap = ("The FreeDOS-bootable disk of <code>docs/PLATFORM.md</code>: the data area, the FreeDOS and save reserves are read from its Size budget table, the budget from <code>tools/check-dos.sh</code>, "
           "and the sizes of CWSDPMI.EXE and FIZZ2.EXE from the measured <code>tools/build-dos.sh</code> run in <code>snapshots/measured.json</code>. Dithered segments are reserves, not files. "
           "No such disk has been built.")
    return chart("".join(o), cap, "Floppy budget", 820)


# ---- Fizz 1's golden traces ----------------------------------------------------------------------------------------------------

TRACE_DIR = os.path.join("crates", "fizz-ep2", "tests", "fizz1")
# (scenario, column, title, unit): the curves drawn from the committed traces. Columns are the trace format's field order.
TRACE_PANELS = [("fall", 2, "fall: Ben's y", "tiles"), ("run", 3, "run: Ben's vx", "tiles/s"), ("pogo_held", 2, "pogo_held: Ben's y", "tiles"), ("look", 13, "look: look_up", "s")]


def read_trace(root, name):
    """The commit named in a committed trace's header, and its rows as lists of fields."""
    with open(os.path.join(root, TRACE_DIR, name + ".trace"), encoding="utf-8") as f:
        lines = f.read().splitlines()
    if lines[0] != "FIZZ1 TRACE 2" or lines[1] != "scenario " + name:
        raise ValueError("%s.trace does not start with FIZZ1 TRACE 2 and its own name" % name)
    return lines[2].split()[1], [l.split(" ") for l in lines[3:]]


def f64(h):
    return struct.unpack(">d", bytes.fromhex(h))[0]


def trace_stats(root):
    names = sorted(n[:-6] for n in os.listdir(os.path.join(root, TRACE_DIR)) if n.endswith(".trace"))
    stats, commit = [], None
    for n in names:
        commit, rows = read_trace(root, n)
        chicken = "|" in rows[0]
        fields = len(rows[0]) - (1 if chicken else 0)
        stats.append((n, len(rows), fields, 7 + (5 if chicken else 0)))
    return stats, commit


def trace_chart(D, root):
    stats, commit = trace_stats(root)
    total_rows = sum(s[1] for s in stats)
    total_fields = sum(s[1] * s[2] for s in stats)
    total_f64 = sum(s[1] * s[3] for s in stats)
    PW, PH, X0, Y0, GAP = 205, 110, 14, 34, 14
    W = X0 * 2 + 4 * PW + 3 * GAP
    BAR_Y = Y0 + PH + 78
    H = BAR_Y + len(stats) * 18 + 12
    label = ("Four curves from Fizz 1's golden traces and a bar for each of the %d scenarios. Fizz 2's rules reproduce all %s rows of the %d traces bit for bit: %s values, of which %s are 64-bit floating-point numbers, with a tolerance of zero." % (
        len(stats), num(total_rows), len(stats), num(total_fields), num(total_f64)))
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s">' % (W, H, esc(label))]
    o.append('<text x="%d" y="18" font-size="13" class="a-t1">Fizz 1 wrote these curves (commit %s); the Fizz 2 test finds no differing bit.</text>' % (X0, esc(commit)))
    for i, (name, col, title, unit) in enumerate(TRACE_PANELS):
        _, rows = read_trace(root, name)
        ys = [f64(r[col]) for r in rows]
        lo, hi = min(ys), max(ys)
        if hi == lo:
            hi = lo + 1
        x0 = X0 + i * (PW + GAP)
        pts = " ".join("%.1f,%.1f" % (x0 + k / (len(ys) - 1) * PW, Y0 + PH - (v - lo) / (hi - lo) * PH) for k, v in enumerate(ys))
        o.append('<rect x="%d" y="%d" width="%d" height="%d" class="a-ext"/>' % (x0, Y0, PW, PH))
        o.append('<polyline points="%s" fill="none" class="d-hot" stroke-width="1.6"/>' % pts)
        o.append('<text x="%d" y="%d" font-size="12" class="a-t1">%s</text>' % (x0, Y0 + PH + 16, esc(title)))
        o.append('<text x="%d" y="%d" font-size="11" class="d-text2 num">%s to %s %s over %d ticks</text>' % (x0, Y0 + PH + 31, "%.3g" % lo, "%.3g" % hi, unit, len(ys) - 1))
        o.append('<text x="%d" y="%d" font-size="11" class="d-text2 num">%d rows, equal in the test</text>' % (x0, Y0 + PH + 45, len(ys)))
    o.append('<text x="%d" y="%d" font-size="13" class="a-t1">Rows compared in each of the %d scenarios</text>' % (X0, BAR_Y - 6, len(stats)))
    mx = max(s[1] for s in stats)
    for i, (n, rows, fields, f64s) in enumerate(stats):
        y = BAR_Y + i * 18
        w = rows / mx * 420
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="11.5" class="a-t1">%s</text><rect x="%d" y="%d" width="%.1f" height="12" class="e11"/>'
                 '<text x="%.1f" y="%d" font-size="11" class="num d-text2">%d rows, %d fields each</text>' % (X0 + 110, y + 10, esc(n), X0 + 120, y, w, X0 + 126 + w, y + 10, rows, fields))
    o.append("</svg>")
    cap = ("The traces are the committed files in <code>%s</code>, read when the site is built; each curve is a column of one trace, its 64-bit values decoded from the hexadecimal bit patterns. "
           "Fizz 2 is not recomputed here: <code>crates/fizz-ep2/tests/fizz1_traces.rs</code> runs each scenario through <code>fizz_ep2::World</code> and fails on the first row whose text differs from the trace, "
           "and <code>cargo test --workspace</code> runs it in <code>tools/check.sh</code>. Together the %d traces hold %s rows and %s values, %s of them 64-bit floating-point numbers."
           ) % (TRACE_DIR.replace(os.sep, "/"), len(stats), num(total_rows), num(total_fields), num(total_f64))
    return chart("".join(o), cap, "Fizz 1 and Fizz 2 traces", 880)
