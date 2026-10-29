#!/usr/bin/env python3
# Builds the Fizz 2 progress report as a multi-page static site: an index, a page per chapter, a gallery, shared CSS and JS, a manifest.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/build.py [--data data.json] [--out dist/progress-report] [--chapters DIR] [--pages DIR]
#                                               [--offline] [--no-assets] [--check]
#
# Standard library only. Adapted from Desk's tools/progress-report/build.py (same author, same licence). Without --data the data is
# gathered first (see gather.py); --offline gathers from the committed gh snapshot. The chapters are markdown files with a small
# front matter (chapters/NN-id.md); the front page text is pages/front.md. --check validates what was written: balanced tags, unique
# ids, internal links and anchors that resolve, every chapter linked from the index.
import hashlib
import html.parser
import json
import os
import re
import shutil
import subprocess
import sys

USAGE = "usage: build.py [--data FILE] [--out DIR] [--chapters DIR] [--pages DIR] [--offline] [--no-assets] [--check]"
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.dont_write_bytecode = True  # the generator's modules sit in the tree; leave no __pycache__ beside them

import charts  # noqa: E402
import drawings as dr  # noqa: E402
import figures  # noqa: E402
import flows  # noqa: E402
import gather  # noqa: E402
import manual  # noqa: E402
import mdlite  # noqa: E402
from mdlite import Ctx, esc, inline  # noqa: E402

ARTIFACT_URL = "https://claude.ai/artifact/1K3wm9Su2dgs8ssGpK9Bbk"
TITLE = "Fizz 2 Progress Report"
DESCRIPTION = ("The archive of Lieutenant Fizz 2, chapter by chapter: what is catalogued with the check behind it, what is built and unverified, "
               "and what is planned, with pull request evidence and a state tag from gh for each. The first slice plays on Web and DOS; chapters 8 and 9 describe it.")
HEADLINE = ["prs", "ciGreen", "headCiGreen", "commitsAll", "files", "crates", "rustLines", "rtLines", "tests", "goldenTicks", "exeBytes", "budgetUsed"]
TAGS = {"merged": "merged", "in review": "review", "in progress": "progress", "planned": "planned", "closed": "closed"}
SHELF_TEXT = {
    "catalogued": ("Catalogued", "Each entry names its check: a merged pull request with CI green, a transcript compared line for line, a measured number with its command."),
    "uncatalogued": ("Uncatalogued", "Built, and not verified in the way that matters: no real hardware, no real browser session, or a run nobody has repeated."),
    "wish": ("Wish list", "Planned or an idea. Nothing here is built."),
}
NAV = [("index", "index.html", "Contents"), ("gallery", "gallery.html", "Gallery"), ("manual", "manual.html", "Manual"), ("roadmap", "roadmap.html", "Roadmap"),
       ("findings", "findings.html", "Findings"), ("archive", "archive.html", "How this is made")]


# ---- reading ------------------------------------------------------------------------------------------------------------------

def read_text(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def load_chapters(directory):
    chapters = []
    for name in sorted(os.listdir(directory)):
        if not name.endswith(".md"):
            continue
        meta, body = mdlite.front_matter(read_text(os.path.join(directory, name)), name)
        for key in ("id", "title", "summary"):
            if not meta.get(key):
                raise mdlite.MdError("%s: front matter needs %s" % (name, key))
        meta["file"] = name
        meta["blocks"], meta["figs"] = figures.take_blocks(mdlite.parse_blocks(body), meta["id"])
        meta.setdefault("n", "")
        meta.setdefault("section", "journey")
        meta.setdefault("date", "")
        try:
            meta["prs"] = [int(x) for x in meta.get("prs", [])]
        except ValueError:
            raise mdlite.MdError("%s: prs must be pull request numbers" % name)
        chapters.append(meta)
    ids = [c["id"] for c in chapters]
    if len(set(ids)) != len(ids):
        raise mdlite.MdError("duplicate chapter ids")
    return chapters


def num(v):
    return "{:,}".format(v) if isinstance(v, int) else str(v)


def tag(state):
    return '<span class="pill pill-%s">%s</span>' % (TAGS.get(state, "planned"), esc(state))


def coming_rows(pages_dir):
    path = os.path.join(pages_dir, "coming.json")
    return gather.read_json(path) if os.path.exists(path) else []


def values_of(data):
    """Every {{placeholder}} the chapters may use, from the gathered data. An unknown placeholder fails the build."""
    vals = {"m." + k: m["value"] for k, m in data["metrics"].items() if m["value"] is not None}
    for p in data["prs"]:
        n = p["number"]
        vals.update({"pr%d.added" % n: p["added"], "pr%d.commits" % n: p["commitCount"], "pr%d.merge" % n: p["mergeCommit"], "pr%d.head" % n: p["head"],
                     "pr%d.state" % n: "merged", "pr%d.ci" % n: p["ci"], "pr%d.headCi" % n: p["headCi"], "pr%d.time" % n: dr.hhmm(p["mergedAt"]),
                     "pr%d.title" % n: mdlite.neutral(p["title"])})
    for o in data["open"]["prs"]:
        n = o["number"]
        vals.update({"pr%d.added" % n: o["ghAdditions"], "pr%d.head" % n: o["head"], "pr%d.state" % n: o["state"], "pr%d.ci" % n: o["ci"],
                     "pr%d.headCi" % n: o["ci"], "pr%d.title" % n: mdlite.neutral(o["title"])})
    f = data["facts"]
    for k, v in f["scraped"].items():
        if v is not None:
            vals["sc." + k] = v
    for k, v in f["loc"].items():
        vals["loc." + k.replace("-", "_")] = v
    for k, v in f["tests"].items():
        vals["tests." + k.replace("-", "_")] = v
    g = f["golden"]
    vals["golden.final"] = g.get("final") or "unknown"
    vals["golden.finalTick"] = g.get("finalTick") or 0
    vals["golden.lines"] = g.get("lines") or 0
    # Milestone 0, frozen in snapshots/m0.json: the chapters that describe it read these.
    m0 = f["m0"]
    vals["m0.final"] = m0["golden"]["final"]
    vals["m0.finalTick"] = m0["golden"]["finalTick"]
    vals["m0.lines"] = m0["golden"]["lines"]
    vals["m0.biosRange"] = m0["scraped"]["biosRange"]
    for k, v in m0["tests"].items():
        vals["tests." + k.replace("-", "_")] = v
    for k, v in (m0.get("measured") or {}).items():
        if isinstance(v, (int, str)) and k not in ("how", "note"):
            vals["m0ms." + k] = v
    if m0.get("measured", {}).get("exeBytes") and f["scraped"].get("budget"):
        vals["m0.budgetUsed"] = "%.1f%%" % (100.0 * m0["measured"]["exeBytes"] / f["scraped"]["budget"])
    for k, v in (data.get("measured") or {}).items():
        if isinstance(v, (int, str)) and k not in ("how", "note"):
            vals["ms." + k] = v
    subject = [o for o in data["open"]["prs"] if not o["isReport"]]
    vals["open.numbers"] = ", ".join("#%d" % o["number"] for o in subject) or "none"
    vals["open.count"] = len(subject)
    big = [s for s in data.get("stacks", []) if len(s["numbers"]) > 1]
    if big:
        s = big[0]  # the earliest stack, Milestone 0's stack of eight, which chapter 6 and the front page describe (stacks are sorted by landing time)
        vals.update({"stack.count": len(s["numbers"]), "stack.merge": s["mergeCommit"], "stack.numbers": ", ".join("#%d" % n for n in s["numbers"]),
                     "stack.when": "%s at %s" % (dr.day_label(s["landedAt"]), dr.hhmm(s["landedAt"])),
                     "stack.commits": sum(p["commitCount"] for p in data["prs"] if p["mergeCommit"] == s["mergeCommit"]),
                     "stack.added": sum(p["added"] for p in data["prs"] if p["mergeCommit"] == s["mergeCommit"]),
                     "stack.ci": next((p["ci"] for p in data["prs"] if p["mergeCommit"] == s["mergeCommit"]), "unknown")})
    vals["head"] = data["head"] or ""
    vals["generated"] = data["generated"][:10]
    vals["gh.source"] = {"gh": "read from gh", "snapshot": "read from the committed snapshot", "none": "not available"}[data["sources"]["gh"]]
    return vals


# ---- page shell ---------------------------------------------------------------------------------------------------------------

def fragment(title, body, description=DESCRIPTION):
    """The published index: the Artifact skeleton adds the doctype, head and body, so this carries only title, metas and content."""
    return ('<title>%s</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n<meta name="description" content="%s">\n'
            '<link rel="stylesheet" href="site.css">\n%s\n<script src="site.js" defer></script>\n') % (esc(title), esc(description), body)


def document(title, body, description=DESCRIPTION):
    return ('<!doctype html>\n<html lang="en-CA">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'
            '<meta name="claude:viewport-fit-sides" content="cover">\n<title>%s</title>\n<meta name="description" content="%s">\n'
            '<link rel="stylesheet" href="site.css">\n</head>\n<body>\n%s\n<script src="site.js" defer></script>\n</body>\n</html>\n') % (esc(title), esc(description), body)


def site_header(current, ids):
    links = "".join('<a href="%s"%s>%s</a>' % (href, ' aria-current="page"' if current == i else "", text) for i, href, text in NAV if i == "index" or i in ids)
    return ('<a class="skip" href="#main">Skip to the content</a>\n<header class="site"><div class="wrap site-in"><a class="brand" href="index.html">%s</a>'
            '<nav aria-label="Site">%s</nav></div></header>') % (TITLE, links)


def label_of(c):
    return ("%s. %s" % (c["n"], c["title"])) if c.get("n") != "" else c["title"]


def site_footer(data, prev, nxt):
    nav = ""
    if prev or nxt:
        nav = '<nav class="pager" aria-label="Chapters">%s%s</nav>' % (
            ('<a class="prev" href="%s.html" rel="prev"><span>Previous</span><b>%s</b></a>' % (prev["id"], esc(label_of(prev)))) if prev else '<span></span>',
            ('<a class="next" href="%s.html" rel="next"><span>Next</span><b>%s</b></a>' % (nxt["id"], esc(label_of(nxt)))) if nxt else '<span></span>')
    src = data["sources"]
    ghs = {"gh": "read from <code>gh</code> at build time", "snapshot": "read from the committed snapshot of <code>gh</code>" + (" taken %s" % esc(src["ghCapturedAt"][:10]) if src.get("ghCapturedAt") else ""),
           "none": "not available"}[src["gh"]]
    return ('%s<footer class="site"><div class="wrap"><p>Generated %s from <code>%s</code> at <code>%s</code> by <code>tools/progress-report/</code>. Pull request and CI data: %s. '
            'Each number on these pages says how it was measured.</p></div></footer>') % (nav, esc(data["generated"][:10]), esc(data["ref"]), esc(data["head"] or "?"), ghs)


# ---- shared blocks ------------------------------------------------------------------------------------------------------------

def pr_records(data):
    out = {}
    for p in data["prs"]:
        out[p["number"]] = {"number": p["number"], "title": p["title"], "state": "merged", "commit": p["mergeCommit"], "head": p["head"],
                            "when": "%s %s" % (dr.day_label(p["mergedAt"]), dr.hhmm(p["mergedAt"])), "commits": p["commitCount"], "added": p["added"],
                            "ci": p["ci"], "headCi": p["headCi"], "url": p.get("url"), "base": p.get("base")}
    for o in data["open"]["prs"]:
        out[o["number"]] = {"number": o["number"], "title": o["title"], "state": o["state"], "commit": o["head"], "head": o["head"],
                            "when": "opened " + dr.day_label(o["createdAt"]) if o.get("createdAt") else "", "commits": None, "added": o["ghAdditions"],
                            "ci": o["ci"], "headCi": o["ci"], "url": o.get("url"), "base": o["base"]}
    return out


def chapter_state(data, numbers):
    """merged when every pull request is merged, in progress when one is a draft, in review when one is open, else planned."""
    recs = pr_records(data)
    states = [recs[n]["state"] for n in numbers if n in recs]
    if not numbers or len(states) != len(numbers):
        return "planned"
    if all(x == "merged" for x in states):
        return "merged"
    return "in progress" if "in progress" in states else "in review"


def ci_word(c):
    return {"passing": "green", "pending": "running", "unknown": "not found"}.get(c, c)


def pr_table(data, numbers, ctx):
    rows = []
    recs = pr_records(data)
    for n in numbers:
        p = recs.get(n)
        if not p:
            rows.append('<tr><td>#%d</td><td colspan="6">not found among the pull requests</td></tr>' % n)
            continue
        url = p.get("url") or ctx.href("pr:%d" % n)
        merged = p["state"] == "merged"
        checks = ("CI %s on the head; CI %s on main at <code>%s</code>" % (ci_word(p["headCi"]), ci_word(p["ci"]), p["commit"])) if merged else (
            "CI %s on the head commit; not merged" % ci_word(p["ci"]))
        rows.append('<tr><td><a href="%s" target="_blank" rel="noopener">#%d</a></td><td>%s<div class="sub">based on <code>%s</code></div></td><td>%s</td><td><code>%s</code><div class="sub">%s</div></td>'
                    '<td class="n">%s</td><td class="n">+%s</td><td>%s</td></tr>' % (
                        esc(url), n, esc(p["title"]), esc(p["base"] or ""), tag(p["state"]), p["head"], esc(p["when"]), num(p["commits"]) if p["commits"] is not None else "·",
                        num(p["added"] or 0), checks))
    note = ('<p class="how">Merged: the pull request\'s own commits run from the head of the pull request below it (or from main) to its head, from <code>git log</code> and '
            '<code>git diff --numstat</code>. Head CI is the check rollup of <code>gh pr list</code>; main CI is <code>gh run list --branch main</code> on the commit that brought it into main.</p>')
    return ('<div class="tbl"><table><caption class="sr">Pull requests behind this chapter</caption><thead><tr><th scope="col">PR</th><th scope="col">Title</th><th scope="col">State</th>'
            '<th scope="col">Head</th><th scope="col" class="n">Commits</th><th scope="col" class="n">Lines added</th><th scope="col">Checks</th></tr></thead><tbody>%s</tbody></table></div>%s') % ("".join(rows), note)


def facts_strip(data, keys):
    cells = []
    for k in keys:
        m = data["metrics"][k]
        cells.append('<div class="fact"><b class="num">%s</b><span>%s</span><details><summary>how measured</summary><p>%s</p></details></div>' % (
            esc(num(m["value"]) if m["value"] is not None else "not available"), esc(m["label"]), inline(m["how"], Ctx())))
    return '<div class="facts">%s</div>' % "".join(cells)


def measured_block(data):
    m = data.get("measured") or {}
    if not m:
        return '<p class="how">No measured run is recorded.</p>'
    lines = m.get("dosReplay", []) + m.get("dosLive", []) + ["PANIC exit %s" % m.get("panicExit"), "SIZE %s" % m.get("exeBytes"), "BUDGET %s" % m.get("budget")] + m.get("parityLines", [])
    return ('<figure><pre class="term" tabindex="0" aria-label="Output of the measured DOS and parity run"><code>%s</code></pre><figcaption>%s</figcaption></figure>' % (
        esc("\n".join(lines)), inline("From `snapshots/measured.json`: " + m.get("how", ""), Ctx())))


# ---- the site -----------------------------------------------------------------------------------------------------------------

class Site:
    def __init__(self, data, chapters, pages_dir, out, assets=True, manual_root=None, manual_evidence=None):
        self.data, self.chapters, self.pages_dir, self.out, self.assets = data, chapters, pages_dir, out, assets
        # docs/MANUAL.md from the working tree of the repository (or of manual_root in the tests); None skips the page
        self.manual_root = manual_root or gather.ROOT
        self.manual_text = manual.read(self.manual_root)
        self.manual_images = []
        self.manual_evidence = manual_evidence or os.path.join(HERE, "manual-evidence.json")
        self.ids = [c["id"] for c in chapters]
        self.ctx = Ctx(values_of(data), self.ids, data["repo"]["url"], data["prs"])
        self.shelf_items = []
        self.pages = {}
        self.registry = figures.validate([r for c in chapters for r in c["figs"]], chapters)
        claimed = {r.get("at") for r in self.registry}
        for c in chapters:
            for key in figures.unregistered(c["blocks"], [r for r in self.registry if r["chapter"] == c["id"]], c["id"]):
                if key not in claimed:
                    raise mdlite.MdError("%s: the directive %r draws something and no figures block claims it" % (c["id"], key))
        for r in self.registry:
            ch = [c for c in chapters if c["id"] == r["chapter"]][0]
            if r.get("at") and r["at"] not in [figures.directive_key(b["name"], b["args"]) for b in ch["blocks"] if b["type"] == "directive"]:
                raise mdlite.MdError("%s: figure %s names the directive %r, which the chapter does not contain" % (r["chapter"], r["id"], r["at"]))
        self.fig_at = {(r["chapter"], r["at"]): r for r in self.registry if r.get("at")}
        self.fig_html = {}

    def directive(self, chapter):
        d, ctx = self.data, self.ctx

        def run(name, args):
            html = inner(name, args)
            rec = self.fig_at.get((chapter["id"], figures.directive_key(name, args)))
            if rec:
                self.fig_html[rec["id"]] = html
                return '<div class="figwrap" id="%s">%s</div>' % (rec["anchor"], html)
            return html

        def inner(name, args):
            if name == "explainer":
                table = {"replay": dr.replay, "timer": dr.timer}
                if not args or args[0] not in table:
                    raise mdlite.MdError("unknown explainer %s in %s" % (args, chapter["id"]))
                return table[args[0]](d, ctx)
            if name == "figure" and args and args[0] == "shots":
                return flows.shots(d, args[1:])
            if name == "figure":
                fid = args[0] if args else ""
                table = {
                    "input-path": lambda: flows.input_path(d, gather.ROOT), "frame-path": lambda: flows.frame_path(d, gather.ROOT),
                    "dos-transcript": lambda: flows.dos_transcript(d), "script": lambda: flows.script_table(d),
                    "loc": lambda: charts.loc_by_crate(d), "tests": lambda: charts.tests_by_crate(d), "pr-sizes": lambda: charts.pr_sizes(d),
                    "ci": lambda: charts.ci_results(d), "floppy": lambda: charts.floppy(d),
                    "layers": lambda: dr.layers(d), "parity": lambda: dr.parity(d), "pr-stack": lambda: dr.pr_stack(d),
                    "milestones": lambda: dr.milestone_progress(d), "size-budget": lambda: dr.size_budget(d),
                    "anatomy-dos": lambda: dr.anatomy("dos", ctx, gather.ROOT, "Layers of FIZZ2.EXE from the program to the machine, each box marked built and verified, planned or not ours."),
                    "crate-graph": lambda: dr.crate_graph(d, gather.ROOT), "maze": lambda: dr.maze(d, gather.ROOT),
                    "trace-chart": lambda: charts.trace_chart(d, gather.ROOT), "cluck-path": lambda: flows.cluck_path(d, gather.ROOT),
                    "dos-help": lambda: flows.help_text(d, gather.ROOT), "dos-probe": lambda: flows.text_run(d, "dos-probe.txt", "FIZZ2.EXE /PROBE", "Output of FIZZ2.EXE /PROBE in DOSBox"),
                    "nexts": lambda: dr.roadmap_lists(d), "roadmap-table": lambda: dr.roadmap_table(d, ctx), "measured": lambda: measured_block(d),
                }
                if fid not in table:
                    raise mdlite.MdError("unknown figure %s in %s" % (fid, chapter["id"]))
                return table[fid]()
            if name == "facts":
                for k in args:
                    if k not in d["metrics"]:
                        raise mdlite.MdError("unknown metric %s in %s" % (k, chapter["id"]))
                return facts_strip(d, args)
            raise mdlite.MdError("unknown directive %s in %s" % (name, chapter["id"]))
        return run

    def render_chapter(self, i):
        c = self.chapters[i]
        body, shelves = mdlite.render(c["blocks"], self.ctx, self.directive(c))
        for k, items in shelves.items():
            for it in items:
                self.shelf_items.append((k, c, it))
        prev = self.chapters[i - 1] if i > 0 else None
        nxt = self.chapters[i + 1] if i + 1 < len(self.chapters) else None
        eyebrow = []
        if c.get("n") != "":
            eyebrow.append("Chapter %s" % c["n"])
        elif c.get("kicker"):
            eyebrow.append(c["kicker"])
        if c.get("date"):
            eyebrow.append(self.date_label(c["date"]))
        if c["prs"]:
            eyebrow.append(", ".join("PR #%d" % n for n in c["prs"]))
        evidence, staterow = "", ""
        if c["prs"]:
            evidence = '<section class="evidence"><h2 id="the-pull-requests">The pull requests</h2>%s</section>' % pr_table(self.data, c["prs"], self.ctx)
            recs = pr_records(self.data)
            parts = ["#%d %s" % (n, recs[n]["state"]) for n in c["prs"] if n in recs]
            staterow = '<p class="state-row">%s <span>%s</span></p>' % (tag(chapter_state(self.data, c["prs"])), esc("; ".join(parts)))
        main = ('<main id="main" class="wrap chapter"><article><header class="chead"><div class="eyebrow">%s</div><h1>%s</h1>%s<p class="lead">%s</p></header>'
                '<div class="prose">%s</div>%s</article></main>') % (esc(" · ".join(eyebrow)), esc(c["title"]), staterow, inline(c["summary"], self.ctx), body, evidence)
        current = c["id"] if c["id"] in [n[0] for n in NAV] else ""
        return c["id"] + ".html", document("%s · %s" % (TITLE, c["title"]), site_header(current, self.ids + self.extra_ids()) + main + site_footer(self.data, prev, nxt))

    @staticmethod
    def date_label(iso):
        import datetime
        d = datetime.date.fromisoformat(iso)
        return "%d %s %d" % (d.day, d.strftime("%B"), d.year)

    def extra_ids(self):
        return (["gallery"] if self.registry else []) + (["manual"] if self.manual_text is not None else [])

    def render_manual(self):
        """The Manual page: one line saying what the page is and where its text comes from, the manual itself in its own words,
        and the table of what is checked."""
        title, blocks = manual.parse(self.manual_text)
        mctx = manual.ManualCtx(self.manual_root, self.data["repo"]["url"], self.ids)
        body, _ = mdlite.render(blocks, mctx, lambda n, a: "")
        if mctx.problems:
            raise mdlite.MdError("; ".join(mctx.problems))
        self.manual_images = sorted(set(mctx.images))
        mapping = gather.read_json(self.manual_evidence)["sections"]
        table = manual.evidence_table(blocks, mapping, self.data, self.ctx, chapter_state, tag)
        intro = '<p class="lead">The player\'s manual of Lieutenant Fizz 2, %s.</p>' % inline(manual.source_of(self.manual_root), Ctx())
        main = ('<main id="main" class="wrap chapter"><article><header class="chead"><div class="eyebrow">Manual</div><h1>%s</h1>%s</header>'
                '<div class="prose manual">%s</div>%s</article></main>') % (esc(title), intro, body, table)
        self.pages["manual.html"] = document("%s · Manual" % TITLE, site_header("manual", self.ids + self.extra_ids()) + main + site_footer(self.data, None, None))

    def fig_state(self, rec):
        if rec.get("state"):
            return rec["state"]
        ch = [c for c in self.chapters if c["id"] == rec["chapter"]][0]
        return chapter_state(self.data, ch["prs"]) if ch["prs"] else "merged"

    def chapter_of(self, rec):
        return [c for c in self.chapters if c["id"] == rec["chapter"]][0]

    def tile(self, rec, i):
        ch = self.chapter_of(rec)
        state = self.fig_state(rec)
        chap = ("Chapter %s" % ch["n"]) if ch.get("n") != "" else ch["title"]
        pv = figures.preview(rec, self.fig_html.get(rec["id"], ""), "g%d-" % i, {})
        alt = "" if rec["thumb"].startswith("img:") else '<span class="sr">%s</span>' % esc(rec["alt"])
        date = rec.get("date") or ch["date"]
        return ('<li class="tile" data-kind="%s" data-track="%s" data-i="%d" data-date="%s"><a class="tile-link" href="%s.html#%s">'
                '<div class="tile-pv">%s</div><div class="tile-body"><b class="tile-title">%s</b>%s<span class="tile-cap">%s</span>'
                '<span class="tile-meta"><span class="kind">%s</span><span>%s</span>%s<span>%s</span></span></div></a></li>') % (
            rec["kind"], rec["track"], i, esc(date), rec["chapter"], esc(rec["anchor"]), pv, esc(rec["title"]), alt, esc(rec["caption"]),
            esc(rec["kind"]), esc(chap), tag(state) if state != "none" else '<span class="pill pill-overview">overview</span>', esc(self.date_label(date)) if date else "")

    def render_gallery(self):
        recs = self.registry
        kinds = [k for k in figures.KINDS if any(r["kind"] == k for r in recs)]
        tracks = [t for t in figures.TRACKS if any(r["track"] == t for r in recs)]

        def chips(group, values):
            return "".join('<a class="chip" id="%s-%s" href="#%s-%s" data-group="%s" data-value="%s" aria-pressed="false">%s <span class="muted">%d</span></a>' % (
                group, v, group, v, group, v, esc({"web": "Web", "dos": "DOS"}.get(v, v.title()) if group == "track" else v), sum(1 for r in recs if r[group] == v)) for v in values)
        tiles = "".join(self.tile(r, i) for i, r in enumerate(recs))
        rows = []
        for r in recs:
            ch = self.chapter_of(r)
            st = self.fig_state(r)
            rows.append('<tr data-kind="%s" data-track="%s"><td><a href="%s.html#%s">%s</a></td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>' % (
                r["kind"], r["track"], r["chapter"], esc(r["anchor"]), esc(r["title"]), esc(r["kind"]), esc(r["track"]), esc(ch["title"]), tag(st) if st != "none" else "overview"))
        body = ('<main id="main" class="wrap gallery"><header class="chead"><div class="eyebrow">Gallery · %d figures</div><h1>Gallery</h1>'
                '<p class="lead">Every diagram, chart and step-through in the archive. Each tile opens the chapter at the figure.</p></header>'
                '<form class="controls" id="galleryControls" aria-label="Filter and sort the gallery" onsubmit="return false">'
                '<div class="chiprow" role="group" aria-label="Kind"><span class="chip-h">Kind</span><a class="chip" id="kind-all" href="#kind-all" data-group="kind" data-value="" aria-pressed="true">All <span class="muted">%d</span></a>%s</div>'
                '<div class="chiprow" role="group" aria-label="Track"><span class="chip-h">Track</span><a class="chip" id="track-all" href="#track-all" data-group="track" data-value="" aria-pressed="true">All</a>%s</div>'
                '<div class="chiprow" role="group" aria-label="Sort and view"><span class="chip-h">Order</span>'
                '<button type="button" class="chip" data-sort="chapter" aria-pressed="true">Chapter order</button><button type="button" class="chip" data-sort="new" aria-pressed="false">Newest first</button>'
                '<span class="chip-h">View</span><button type="button" class="chip" data-view="grid" aria-pressed="true">Tiles</button><button type="button" class="chip" data-view="list" aria-pressed="false">List</button></div>'
                '<p class="count" id="galleryCount" aria-live="polite">Showing all %d figures.</p></form>'
                '<ul class="tiles" id="tiles">%s</ul>'
                '<details class="fig-listbox" id="figList"><summary>Every figure as a list</summary><div class="tbl"><table><caption class="sr">Every figure, with its kind, track, chapter and state</caption><thead><tr><th scope="col">Figure</th><th scope="col">Kind</th><th scope="col">Track</th><th scope="col">Chapter</th><th scope="col">State</th></tr></thead><tbody>%s</tbody></table></div></details>'
                '<p class="how">A figure takes the state tag of its chapter\'s pull requests, from <code>gh</code>. Step-through previews are static; play them in the chapter.</p>'
                '</main>') % (len(recs), len(recs), chips("kind", kinds), chips("track", tracks), len(recs), tiles, "".join(rows))
        self.pages["gallery.html"] = document("%s · Gallery" % TITLE, site_header("gallery", self.ids + self.extra_ids()) + body + site_footer(self.data, None, None))

    def recent_strip(self):
        recs = list(enumerate(self.registry))
        recs.sort(key=lambda t: (t[1].get("date") or self.chapter_of(t[1])["date"], t[0]), reverse=True)
        picked, per = [], {}
        for i, r in recs:
            if per.get(r["chapter"], 0) >= 2:
                continue
            per[r["chapter"]] = per.get(r["chapter"], 0) + 1
            picked.append((i, r))
            if len(picked) == 6:
                break
        return ('<ul class="tiles recent">%s</ul><p class="how"><a href="gallery.html">All %d figures in the gallery</a>, filtered by kind and track.</p>' % (
            "".join(self.tile(r, 100 + i) for i, r in picked), len(self.registry)))

    def status_panel(self):
        d = self.data
        last = max(d["prs"], key=lambda p: p["landedAt"]) if d["prs"] else None
        subject = [o for o in d["open"]["prs"] if not o["isReport"]]
        big = [s for s in d.get("stacks", []) if len(s["numbers"]) > 1]
        open_html = ("%d open: %s." % (len(subject), "; ".join("#%d %s, %s, CI %s" % (o["number"], esc(o["title"]), esc(o["state"]), ci_word(o["ci"])) for o in subject))) if subject else "None."
        last_html = ("<code>%s</code>, %s %s%s" % (last["mergeCommit"], dr.day_label(last["landedAt"]), dr.hhmm(last["landedAt"]),
                                                   (": the stack %s" % ", ".join("#%d" % n for n in big[-1]["numbers"])) if big and big[-1]["mergeCommit"] == last["mergeCommit"] else ": #%d" % last["number"])) if last else "none"
        main_ci = ci_word(last["ci"]) if last else "unknown"
        g, m0 = d["facts"]["golden"], d["facts"]["m0"]["golden"]
        return ('<dl class="status"><div><dt>Is there a game?</dt><dd><b>A first slice.</b> One small level with Ben, a chicken, a red key and a door plays on Web and DOS from the same rules (<code>fizz-ep2</code>). Chapters 8 and 9 describe the slice; chapters 3 to 6 describe Milestone 0, the marker that came before it.</dd></div>'
                '<div><dt>The parity gate</dt><dd>The same <code>FINAL TICK %s HASH %s</code> on native, Web (WASM under Node and Bun) and DOS (headless DOSBox), compared with <code>crates/fizz-ep2/slice-replay.txt</code> by <code>tools/check-parity.sh</code>.</dd></div>'
                '<div><dt>Milestone 0, the platform skeleton</dt><dd><b>%s.</b> Its marker replay ended <code>FINAL TICK %s HASH %s</code> on all three platforms.</dd></div>'
                '<div><dt>Last merge into main</dt><dd>%s<div class="sub">CI on that commit: %s.</div></dd></div>'
                '<div><dt>Open pull requests</dt><dd>%s</dd></div>'
                '<div><dt>On real hardware</dt><dd><b>Not tried.</b> No 486, no real display, no browser session. See Not verified below.</dd></div>'
                '<div><dt>Pull requests merged</dt><dd>%s, by merge commit, into <code>%s</code>.</dd></div></dl>') % (
            g.get("finalTick") or "?", esc(g.get("final") or "?"),
            "Merged" if all(n["tag"] == "merged" for n in d["roadmap"]["nodes"] if n["milestone"] == "m0") else "Not all merged",
            m0.get("finalTick") or "?", esc(m0.get("final") or "?"), last_html, esc(main_ci), open_html, num(len(d["prs"])), esc(d["ref"]))

    def render_index(self):
        d, ctx = self.data, self.ctx
        intro, _ = mdlite.render(mdlite.parse_blocks(read_text(os.path.join(self.pages_dir, "front.md"))), ctx, lambda n, a: "")
        for i in range(len(self.chapters)):
            self.pages.update([self.render_chapter(i)])
        if self.manual_text is not None:
            self.render_manual()
        if self.registry:
            self.render_gallery()
        sh = []
        for k in ("catalogued", "uncatalogued", "wish"):
            name, what = SHELF_TEXT[k]
            items = [(c, it) for kk, c, it in self.shelf_items if kk == k]
            sh.append('<section class="shelf shelf-%s"><h3>%s <span class="count">%d</span></h3><p>%s</p><details><summary>All %d entries, by chapter</summary><ul>%s</ul></details></section>' % (
                k, name, len(items), esc(what), len(items), "".join('<li><a href="%s.html">%s</a>: %s</li>' % (c["id"], esc(c["title"]), it) for c, it in items)))
        toc, placed = [], False

        def coming_html():
            return "".join('<tr class="coming"><td class="n">·</td><td><b>%s</b> <span class="pill pill-coming">coming</span><div class="sub">%s</div></td><td class="nw">not written</td><td class="nw">%s</td><td class="nw"></td></tr>' % (
                esc(r["title"]), inline(r["summary"], ctx), tag("planned")) for r in coming_rows(self.pages_dir))
        for c in self.chapters:
            if c.get("section") == "reference" and not placed:
                toc.append(coming_html())
                placed = True
            toc.append('<tr><td class="n">%s</td><td><a href="%s.html"><b>%s</b></a><div class="sub">%s</div></td><td class="nw">%s</td><td class="nw">%s</td><td class="nw">%s</td></tr>' % (
                esc(c["n"]), c["id"], esc(c["title"]), inline(c["summary"], ctx), esc(self.date_label(c["date"]) if c.get("date") else ""),
                tag(chapter_state(d, c["prs"])) if c["prs"] else "", esc(", ".join("#%d" % n for n in c["prs"]))))
        if not placed:
            toc.append(coming_html())
        if self.manual_text is not None:
            toc.append('<tr><td class="n">·</td><td><a href="manual.html"><b>The player\'s manual</b></a><div class="sub">The manual from <code>docs/MANUAL.md</code>, with what the archive has checked for each section.</div></td><td></td><td></td><td></td></tr>')
        else:
            toc.append('<tr class="coming"><td class="n">·</td><td><b>The player\'s manual</b> <span class="pill pill-coming">coming</span><div class="sub">Shown here once <code>docs/MANUAL.md</code> is in the repository.</div></td><td class="nw">not written</td><td class="nw">%s</td><td></td></tr>' % tag("planned"))
        if self.registry:
            toc.append('<tr><td class="n">·</td><td><a href="gallery.html"><b>Gallery</b></a><div class="sub">%d figures from %d chapters, each linked to where it lives.</div></td><td></td><td></td><td></td></tr>' % (
                len(self.registry), len({r["chapter"] for r in self.registry})))
        nv = gather.read_json(os.path.join(HERE, "not-verified.json"))
        notver = "".join("<li><b>%s</b> %s</li>" % (esc(x["what"]), inline(x["why"], ctx)) for x in nv)
        hero = '<figure class="banner"><img src="img/hero.svg" alt="The Lieutenant Fizz 2 banner in 16-colour EGA: a lighthouse beam over the sea, a Zarg saucer over a chocolate factory, and Ben on his pogo stick." width="1200" height="320"></figure>' if self.assets else ""
        strip = ('<section aria-labelledby="h-recent"><h2 id="h-recent">Recent figures</h2>%s</section>' % self.recent_strip()) if self.registry else ""
        warn = ""
        if d.get("warnings"):
            warn = '<section aria-labelledby="h-warn"><h2 id="h-warn">Warnings from this build</h2><ul class="notver">%s</ul></section>' % "".join("<li>%s</li>" % esc(w) for w in d["warnings"])
        return ('%s<main id="main" class="wrap front"><header class="hero">%s<div class="eyebrow">Liminal HQ · Lieutenant Fizz 2 · The archive · generated %s</div><h1>%s</h1>'
                '<div class="prose lead-block">%s</div></header>'
                '<section aria-labelledby="h-status"><h2 id="h-status">Status</h2>%s</section>%s%s'
                '<section aria-labelledby="h-shelves"><h2 id="h-shelves">The three shelves</h2><p class="sub col">Counts of entries on the chapter pages.</p><div class="shelves">%s</div></section>'
                '<section aria-labelledby="h-toc"><h2 id="h-toc">The chapters</h2><div class="tbl"><table class="toc"><caption class="sr">Chapters, with date, state and pull requests</caption><thead><tr><th scope="col" class="n">No.</th><th scope="col">Chapter</th><th scope="col">Date</th><th scope="col">State</th><th scope="col">Pull requests</th></tr></thead><tbody>%s</tbody></table></div></section>'
                '<section aria-labelledby="h-cols"><h2 id="h-cols">Status by milestone</h2>%s</section>'
                '<section aria-labelledby="h-nums"><h2 id="h-nums">Headline numbers</h2><p class="sub col">Computed from the tree of <code>%s</code> and from <code>gh</code>; open the note under each for the command.</p>%s</section>'
                '<section aria-labelledby="h-nv"><h2 id="h-nv">Not verified</h2><p class="sub col">Anything that depends on one of these stays on the uncatalogued shelf.</p><ul class="notver">%s</ul></section>'
                '</main>') % (site_header("index", self.ids + self.extra_ids()), hero, esc(d["generated"][:10]), TITLE, intro, self.status_panel(), warn, strip, "".join(sh),
                              "".join(toc), dr.roadmap_lists(d), esc(d["ref"]), facts_strip(d, [k for k in HEADLINE if k in d["metrics"]]), notver)

    def write(self):
        out = self.out
        if os.path.exists(out):
            # Only a directory this tool wrote (it holds our manifest) or an empty one is cleared.
            if os.listdir(out) and not os.path.exists(os.path.join(out, "manifest.json")):
                raise ValueError("refusing to clear %s: it is not empty and has no manifest.json from an earlier build" % out)
            shutil.rmtree(out)
        os.makedirs(os.path.join(out, "_publish"))
        copied = (self.copy_assets() + self.copy_media()) if self.assets else []
        index_body = self.render_index()
        for rel in self.manual_images:
            dst = os.path.join(out, "manual", rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(os.path.join(self.manual_root, rel), dst)
            copied.append("manual/" + rel.replace(os.sep, "/"))
        footer = site_footer(self.data, None, self.chapters[0] if self.chapters else None)
        files = {"index.html": document(TITLE, index_body + footer), "_publish/index.html": fragment(TITLE, index_body + footer)}
        files.update(self.pages)
        files["site.css"] = read_text(os.path.join(HERE, "templates", "site.css"))
        files["site.js"] = read_text(os.path.join(HERE, "templates", "site.js"))
        for path, text in files.items():
            with open(os.path.join(out, path), "w", encoding="utf-8") as f:
                f.write(text)
        self.write_manifest(sorted(files) + copied)
        return files

    def copy_media(self):
        """The captured screenshots, each listed in media/captures.json."""
        copied = []
        for c in flows.load_captures():
            if c["file"].endswith((".png", ".jpg")):
                os.makedirs(os.path.join(self.out, "media"), exist_ok=True)
                shutil.copyfile(os.path.join(flows.MEDIA, c["file"]), os.path.join(self.out, "media", c["file"]))
                copied.append("media/" + c["file"])
        return copied

    def copy_assets(self):
        """The README banner, from the tree of the commit the data was gathered at (the working tree when that commit is not here)."""
        blob = None
        if self.data.get("head"):
            p = subprocess.run(["git", "show", "%s:assets/hero.svg" % self.data["head"]], cwd=gather.ROOT, capture_output=True)
            blob = p.stdout if p.returncode == 0 else None
        src = os.path.join(gather.ROOT, "assets", "hero.svg")
        if blob is None and os.path.exists(src):
            with open(src, "rb") as f:
                blob = f.read()
        if blob is None:
            print("warning: missing asset assets/hero.svg", file=sys.stderr)
            return []
        os.makedirs(os.path.join(self.out, "img"), exist_ok=True)
        with open(os.path.join(self.out, "img", "hero.svg"), "wb") as f:
            f.write(blob)
        return ["img/hero.svg"]

    def write_manifest(self, paths):
        out = self.out
        entries = []
        for p in paths:
            with open(os.path.join(out, p), "rb") as f:
                blob = f.read()
            entries.append({"path": p, "bytes": len(blob), "sha256": hashlib.sha256(blob).hexdigest()})
        published = {p: p for p in paths if p not in ("index.html", "_publish/index.html", "manifest.json")}
        try:
            inside = os.path.commonpath([os.path.abspath(out), gather.ROOT]) == gather.ROOT
        except ValueError:
            inside = False
        rel = os.path.relpath(out, gather.ROOT) if inside else os.path.abspath(out)
        manifest = {
            "title": TITLE, "artifact": ARTIFACT_URL, "description": DESCRIPTION,
            "publish": {"action": "publish", "url": ARTIFACT_URL, "file_path": rel + "/_publish/index.html", "root": rel, "files": published, "description": DESCRIPTION,
                        "notes": "A first publish omits url and passes icon and title; a republish passes url and omits icon and title. file_path is the index as a fragment (the Artifact skeleton supplies the doctype and head); every other page is a full document."},
            "limits": {"perFileBytes": 16 * 1024 * 1024, "filesPerPublish": 255, "totalBytesPerPublish": 64 * 1024 * 1024},
            "totals": {"files": len(entries), "bytes": sum(e["bytes"] for e in entries), "note": "every file written except manifest.json itself"},
            "files": entries,
        }
        with open(os.path.join(out, "manifest.json"), "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=1)
            f.write("\n")


# ---- checking -----------------------------------------------------------------------------------------------------------------

VOID = {"meta", "link", "br", "hr", "img", "input", "col", "area", "base", "source", "track", "wbr", "path", "rect", "circle", "line", "polygon", "polyline", "ellipse", "stop", "use"}


class Scan(html.parser.HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.stack, self.ids, self.hrefs, self.problems, self.dupes = [], [], [], [], []

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            if a["id"] in self.ids:
                self.dupes.append(a["id"])
            self.ids.append(a["id"])
        if tag in ("a", "link", "script", "img") and (a.get("href") or a.get("src")):
            self.hrefs.append((tag, a.get("href") or a.get("src")))
        if tag == "img" and not (a.get("alt") or "").strip():
            self.problems.append("an image without alt text: %s" % a.get("src"))
        if tag not in VOID:
            self.stack.append(tag)

    def handle_startendtag(self, tag, attrs):
        a = dict(attrs)
        if tag == "img" and not (a.get("alt") or "").strip():
            self.problems.append("an image without alt text: %s" % a.get("src"))
        if "id" in a:
            self.ids.append(a["id"])
        if tag in ("img", "link") and (a.get("href") or a.get("src")):
            self.hrefs.append((tag, a.get("href") or a.get("src")))

    def handle_endtag(self, tag):
        if tag in VOID:
            return
        if not self.stack or self.stack[-1] != tag:
            self.problems.append("unexpected </%s> (open: %s)" % (tag, "/".join(self.stack[-4:])))
            if tag in self.stack:
                while self.stack and self.stack.pop() != tag:
                    pass
        else:
            self.stack.pop()


def check_site(out, expect_chapters=()):
    problems = []
    pages = sorted(f for f in os.listdir(out) if f.endswith(".html"))
    scans = {}
    for name in pages:
        text = read_text(os.path.join(out, name))
        s = Scan()
        s.feed(text)
        scans[name] = s
        problems += ["%s: %s" % (name, p) for p in s.problems]
        if s.stack:
            problems.append("%s: unclosed tags %s" % (name, "/".join(s.stack)))
        problems += ["%s: duplicate id %s" % (name, x) for x in s.dupes]
        if not text.lstrip().lower().startswith("<!doctype html>"):
            problems.append("%s: no doctype" % name)
        if "<title>" not in text or 'name="viewport"' not in text:
            problems.append("%s: missing title or viewport meta" % name)
        if text.count("<h1") != 1:
            problems.append("%s: expected exactly one h1, found %d" % (name, text.count("<h1")))
    for name, s in scans.items():
        for _, href in s.hrefs:
            if re.match(r"^(https?:|mailto:|data:|javascript:)", href):
                continue
            path, _, frag = href.partition("#")
            page = path or name
            if not os.path.exists(os.path.join(out, page)):
                problems.append("%s: broken link %s" % (name, href))
            elif frag and page in scans and frag not in scans[page].ids:
                problems.append("%s: anchor #%s not found in %s" % (name, frag, page))
    if "index.html" in scans:
        linked = {h.partition("#")[0] for _, h in scans["index.html"].hrefs}
        problems += ["index.html: chapter %s is not linked" % c for c in expect_chapters if c + ".html" not in linked]
    problems += ["missing %s" % m for m in ("site.css", "site.js", "manifest.json") if not os.path.exists(os.path.join(out, m))]
    used = {h.partition("#")[0] for s in scans.values() for _, h in s.hrefs}
    if os.path.isdir(os.path.join(out, "media")):
        problems += ["media/%s is published and no page shows it" % f for f in sorted(os.listdir(os.path.join(out, "media"))) if "media/" + f not in used]
    problems += flows.check_captures()
    return problems


# ---- command line -------------------------------------------------------------------------------------------------------------

def main(argv):
    opts = {"--data": None, "--out": os.path.join(gather.ROOT, "dist", "progress-report"), "--chapters": os.path.join(HERE, "chapters"), "--pages": os.path.join(HERE, "pages")}
    flags, i = set(), 1
    while i < len(argv):
        if argv[i] in opts and i + 1 < len(argv):
            opts[argv[i]] = argv[i + 1]
            i += 2
        elif argv[i] in ("--no-assets", "--check", "--offline"):
            flags.add(argv[i])
            i += 1
        else:
            print(USAGE, file=sys.stderr)
            return 2
    try:
        data = gather.read_json(opts["--data"]) if opts["--data"] else gather.gather(offline="--offline" in flags)
        chapters = load_chapters(opts["--chapters"])
        files = Site(data, chapters, opts["--pages"], opts["--out"], assets="--no-assets" not in flags).write()
    except (mdlite.MdError, ValueError, KeyError, OSError) as e:
        print("build: %s: %s" % (type(e).__name__, e), file=sys.stderr)
        return 1
    print("wrote %d pages to %s (gh data: %s)" % (len([f for f in files if f.endswith(".html")]), opts["--out"], data["sources"]["gh"]))
    if "--check" in flags:
        problems = check_site(opts["--out"], [c["id"] for c in chapters])
        for p in problems:
            print("check: " + p, file=sys.stderr)
        if problems:
            return 1
        print("check: ok")
    for w in list(data.get("warnings", [])) + ["missing: " + m for m in data.get("missing", [])]:
        print("warning: " + w, file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
