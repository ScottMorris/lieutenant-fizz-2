# Tests for the Fizz 2 progress report generator: the markdown reader, the gatherer on throwaway repositories (a stacked merge and a
# shallow clone), the roadmap's state checks, a fixture build with link checking, the figure registry, the drawings against the
# repository's own files, and the voice patterns of VOICE.md.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/progress-report/tests/test_report.py [TestClass]
#
# Adapted from Desk's tools/progress-report/tests/test_report.py (same author, same licence).
import copy
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOL = os.path.normpath(os.path.join(HERE, ".."))
sys.path.insert(0, TOOL)
sys.dont_write_bytecode = True

import build  # noqa: E402
import charts  # noqa: E402
import drawings as dr  # noqa: E402
import figures  # noqa: E402
import flows  # noqa: E402
import gather  # noqa: E402
import manual  # noqa: E402
import mdlite  # noqa: E402

ENV = dict(os.environ, GIT_AUTHOR_NAME="T", GIT_AUTHOR_EMAIL="t@example.com", GIT_COMMITTER_NAME="T", GIT_COMMITTER_EMAIL="t@example.com",
           GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null")
REAL = os.path.join(TOOL, "chapters")
NO_MANUAL = tempfile.mkdtemp(prefix="fizz2-nomanual.")  # a root without docs/MANUAL.md, for the fixture chapters


def rd(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def wr(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def git(cwd, *args):
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, check=True, capture_output=True, text=True).stdout


def commit_file(d, path, text, msg):
    wr(os.path.join(d, path), text)
    git(d, "add", path)
    git(d, "commit", "-q", "-m", msg)


def make_repo(d):
    """main with an initial commit, then a stack of two branches (feat/a on main, feat/b on feat/a) merged by one merge commit
    titled after the top one, then a third branch merged on its own."""
    git(d, "init", "-q", "-b", "main")
    commit_file(d, "a.txt", "one\n", "Initial Commit")
    git(d, "checkout", "-q", "-b", "feat/a")
    commit_file(d, "crates/fizz-sim/src/lib.rs", "#[test]\nfn a() {}\n", "feat: a")
    git(d, "checkout", "-q", "-b", "feat/b")
    commit_file(d, "b.txt", "b\nb\n", "feat: b1")
    commit_file(d, "c.txt", "c\n", "feat: b2")
    git(d, "checkout", "-q", "main")
    git(d, "merge", "-q", "--no-ff", "-m", "Add b (#2)", "feat/b")
    git(d, "checkout", "-q", "-b", "feat/c")
    commit_file(d, "d.txt", "d\n", "feat: c")
    git(d, "checkout", "-q", "main")
    git(d, "merge", "-q", "--no-ff", "-m", "Add c (#3)", "feat/c")


def gh_prs(d):
    def pr(n, head, base):
        return {"number": n, "title": "T%d" % n, "headRefName": head, "baseRefName": base, "headRefOid": git(d, "rev-parse", head).strip(),
                "state": "MERGED", "mergedAt": "2026-10-27T14:54:%02dZ" % n, "isDraft": False, "labels": [], "url": "u%d" % n,
                "checks": [{"name": "check", "status": "COMPLETED", "conclusion": "SUCCESS"}]}
    return [pr(1, "feat/a", "main"), pr(2, "feat/b", "feat/a"), pr(3, "feat/c", "main")]


class MdTests(unittest.TestCase):
    def test_front_matter(self):
        meta, body = mdlite.front_matter("---\nid: a\nprs: [1, 2]\ntitle: Has: a colon\n---\nbody\n")
        self.assertEqual((meta["id"], meta["prs"], meta["title"], body.strip()), ("a", ["1", "2"], "Has: a colon", "body"))
        with self.assertRaises(mdlite.MdError):
            mdlite.front_matter("no front matter")

    def test_inline_and_links(self):
        ctx = mdlite.Ctx({"m.x": 1234}, ["one"], "https://example.com/r", [])
        out = mdlite.inline("**b** *i* `c<d>` [t](chapter:one) [f](repo:docs/f.md) {{m.x}}", ctx)
        for want in ("<strong>b</strong>", "<em>i</em>", "<code>c&lt;d&gt;</code>", 'href="one.html"', 'href="https://example.com/r/blob/main/docs/f.md"', "1,234"):
            self.assertIn(want, out)

    def test_unknown_placeholder_and_chapter_fail(self):
        ctx = mdlite.Ctx({}, ["one"])
        with self.assertRaises(mdlite.MdError):
            mdlite.inline("{{nope}}", ctx)
        with self.assertRaises(mdlite.MdError):
            mdlite.inline("[x](chapter:missing)", ctx)

    def test_shelves_and_sidebar(self):
        blocks = mdlite.parse_blocks("### Catalogued\n- a\n\n### Uncatalogued\n- b\n\n### Wish list\n- c\n\n> **From the workbench — T.** Text.\n> Source: [s](https://example.com)\n")
        html, shelves = mdlite.render(blocks, mdlite.Ctx(), lambda n, a: "")
        self.assertEqual(sorted(shelves), ["catalogued", "uncatalogued", "wish"])
        self.assertIn("not evidence about Fizz 2", html)


class GatherTests(unittest.TestCase):
    def test_a_stacked_merge_credits_each_pull_request_with_its_own_commits(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            _, prs = gather.history(gather.Repo(d, "main"), gh_prs(d))
            by = {p["number"]: p for p in prs}
            self.assertEqual(sorted(by), [1, 2, 3])
            self.assertEqual(by[1]["mergeCommit"], by[2]["mergeCommit"], "the stack lands in one merge commit")
            self.assertNotEqual(by[2]["mergeCommit"], by[3]["mergeCommit"])
            self.assertEqual((by[1]["commitCount"], by[2]["commitCount"], by[3]["commitCount"]), (1, 2, 1))
            self.assertEqual(by[2]["added"], 3)
            self.assertEqual(by[2]["stackedOn"], 1)
            stacks = gather.stacks(prs)
            self.assertEqual(stacks[0]["numbers"], [1, 2])

    def test_without_gh_the_merge_subjects_stand_in(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            _, prs = gather.history(gather.Repo(d, "main"), [])
            self.assertEqual([p["number"] for p in prs], [2, 3])
            self.assertEqual(prs[0]["commitCount"], 3)

    def test_a_pull_request_whose_head_is_not_on_main_is_not_merged(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            git(d, "checkout", "-q", "-b", "feat/x")
            commit_file(d, "x.txt", "x\n", "feat: x")
            git(d, "checkout", "-q", "main")
            prs = gh_prs(d) + [dict(gh_prs(d)[0], number=9, headRefName="feat/x", headRefOid=git(d, "rev-parse", "feat/x").strip())]
            _, merged = gather.history(gather.Repo(d, "main"), prs)
            self.assertNotIn(9, [p["number"] for p in merged])

    def test_gather_offline_on_a_throwaway_repository(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            data = gather.gather(d, "main", offline=True)
            self.assertEqual(data["main"]["files"], 5)
            self.assertEqual(data["facts"]["tests"]["fizz-sim"], 1)
            for m in data["metrics"].values():
                self.assertTrue(m["how"])
            self.assertIn("exeBytes", data["missing"], "a number missing from the documents is reported")

    def test_dry_run_survives_a_shallow_clone(self):
        with tempfile.TemporaryDirectory() as d:
            make_repo(d)
            clone = os.path.join(d, "clone")
            subprocess.run(["git", "clone", "-q", "--depth", "1", "file://" + d, clone], check=True, capture_output=True, env=ENV)
            data = gather.gather(clone, "HEAD", offline=True, dry_run=True)
            self.assertTrue(data["shallow"])
            self.assertIn("pull requests merged", gather.summary(data))

    def test_a_shallow_clone_of_this_repository_builds_the_real_chapters(self):
        # CI's check job has a depth-1 clone: the pull request heads are missing, so their records come from the gh snapshot alone.
        with tempfile.TemporaryDirectory() as d:
            clone = os.path.join(d, "clone")
            subprocess.run(["git", "clone", "-q", "--depth", "1", "file://" + gather.ROOT, clone], check=True, capture_output=True, env=ENV)
            data = gather.gather(clone, "HEAD", offline=True, dry_run=True)
            self.assertTrue(data["shallow"])
            self.assertGreaterEqual(len(data["prs"]), 8)
            build.Site(data, build.load_chapters(REAL), os.path.join(TOOL, "pages"), os.path.join(d, "site"), assets=False).write()

    def test_the_real_tree_yields_every_scraped_number(self):
        data = gather.gather(gather.ROOT, "HEAD", offline=True, dry_run=True)
        self.assertEqual(data["missing"], [])
        sc = data["facts"]["scraped"]
        self.assertEqual(sc["controlWord"], "027F")
        self.assertEqual(sum(r["frames"] for r in data["facts"]["script"]), data["facts"]["golden"]["finalTick"])

    def test_ci_and_state(self):
        self.assertEqual(gather.ci_of([]), "none")
        self.assertEqual(gather.ci_of([{"status": "IN_PROGRESS"}]), "pending")
        self.assertEqual(gather.run_ci({"status": "in_progress", "conclusion": ""}), "pending")
        self.assertEqual(gather.run_ci({"status": "completed", "conclusion": "success"}), "passing")
        self.assertEqual(gather.pr_state({"state": "OPEN", "isDraft": True}), "in progress")


class RoadmapTests(unittest.TestCase):
    def road(self):
        return {"milestones": [{"id": "m0"}], "nodes": [
            {"id": "a", "milestone": "m0", "state": "done-emu", "prs": [1]},
            {"id": "b", "milestone": "m0", "state": "done", "prs": [9]},
            {"id": "c", "milestone": "m0", "state": "done", "prs": [5]},
            {"id": "d", "milestone": "m0", "state": "done-emu", "prs": [2]},
            {"id": "e", "milestone": "m0", "state": "planned"}]}

    def test_a_done_step_keeps_its_state_only_when_merged_with_ci_green(self):
        road = self.road()
        _, warnings = gather.resolve_roadmap(road, [{"number": 1, "ci": "passing"}, {"number": 2, "ci": "pending"}], [{"number": 5, "state": "in review", "ci": "passing"}])
        n = {x["id"]: x for x in road["nodes"]}
        self.assertEqual((n["a"]["state"], n["a"]["tag"]), ("done-emu", "merged"))
        self.assertEqual(n["b"]["state"], "progress", "a pull request that cannot be found")
        self.assertEqual((n["c"]["state"], n["c"]["tag"]), ("review", "in review"), "an open pull request")
        self.assertEqual(n["d"]["state"], "progress", "merged, but CI on main has not passed")
        self.assertEqual(n["e"]["tag"], "planned")
        self.assertEqual(len(warnings), 3)

    def test_the_real_roadmap_is_well_formed(self):
        road = gather.read_json(os.path.join(TOOL, "roadmap.json"))
        ms = {m["id"] for m in road["milestones"]}
        for n in road["nodes"]:
            self.assertIn(n["milestone"], ms, n["id"])
            self.assertIn(n["state"], dr.STATE_LABEL, n["id"])
            self.assertTrue(n["what"] and n["evidence"], n["id"])
            if n["milestone"] != "m0":
                self.assertNotIn(n["state"], ("done", "done-emu"), "%s: a later milestone cannot be catalogued yet" % n["id"])


def fixture_data():
    with tempfile.TemporaryDirectory() as d:
        make_repo(d)
        return gather.gather(d, "main", offline=True)


class BuildTests(unittest.TestCase):
    def build_fixture(self, out, chapters_dir=None):
        data = fixture_data()
        chapters = build.load_chapters(chapters_dir or os.path.join(HERE, "fixture", "chapters"))
        build.Site(data, chapters, os.path.join(HERE, "fixture", "pages"), out, assets=False, manual_root=NO_MANUAL).write()
        return chapters

    def test_fixture_site_is_valid_and_links_resolve(self):
        with tempfile.TemporaryDirectory() as out:
            site = os.path.join(out, "site")
            chapters = self.build_fixture(site)
            self.assertEqual([p for p in build.check_site(site, [c["id"] for c in chapters]) if "captures.json" not in p and "media/" not in p], [])
            for f in ("index.html", "one.html", "two.html", "site.css", "site.js", "manifest.json", "_publish/index.html"):
                self.assertTrue(os.path.exists(os.path.join(site, f)), f)
            index = rd(os.path.join(site, "index.html"))
            self.assertIn("how measured", index)
            frag = rd(os.path.join(site, "_publish", "index.html"))
            self.assertNotIn("<!doctype", frag.lower())
            self.assertIn("<title>Fizz 2 Progress Report</title>", frag)
            manifest = json.loads(rd(os.path.join(site, "manifest.json")))
            self.assertNotIn("index.html", manifest["publish"]["files"])
            self.assertEqual(manifest["publish"]["url"], build.ARTIFACT_URL)

    def test_the_checker_catches_broken_links_anchors_and_missing_alt_text(self):
        with tempfile.TemporaryDirectory() as out:
            site = os.path.join(out, "site")
            chapters = self.build_fixture(site)
            p = os.path.join(site, "two.html")
            wr(p, rd(p).replace("</main>", '<a href="gone.html">x</a><a href="one.html#nope">y</a><img src="site.css" alt=""></main>'))
            problems = build.check_site(site, [c["id"] for c in chapters])
            self.assertTrue(any("broken link gone.html" in x for x in problems))
            self.assertTrue(any("anchor #nope" in x for x in problems))
            self.assertTrue(any("without alt text" in x for x in problems))

    def test_build_does_not_clear_a_directory_it_did_not_make(self):
        with tempfile.TemporaryDirectory() as d:
            wr(os.path.join(d, "precious.txt"), "keep me\n")
            site = build.Site(fixture_data(), build.load_chapters(os.path.join(HERE, "fixture", "chapters")), os.path.join(HERE, "fixture", "pages"), d, assets=False, manual_root=NO_MANUAL)
            with self.assertRaises(ValueError):
                site.write()
            self.assertTrue(os.path.exists(os.path.join(d, "precious.txt")))

    def test_command_lines_fail_cleanly(self):
        self.assertEqual(build.main(["build.py", "--out"]), 2)
        self.assertEqual(gather.main(["gather.py", "--ref"]), 2)


GOOD = ("id: layers-test\nkind: diagram\ntrack: shared\ntitle: The layers\nalt: Layer diagram from content to the machine with Web and DOS columns.\n"
        "caption: Layers.\nat: figure layers\nthumb: svg")


class FigureTests(unittest.TestCase):
    def build(self, figs, extra=""):
        out = tempfile.mkdtemp()
        ch = os.path.join(out, "chapters")
        wr(os.path.join(ch, "00-one.md"), "---\nid: one\nn: 1\ntitle: One\ndate: 2026-10-27\nsummary: S\nprs: []\n---\nText.\n\n<!-- figure layers -->\n%s\n### Catalogued\n- a\n\n### Uncatalogued\n- b\n\n### Wish list\n- c\n\n```figures\n%s\n```\n" % (extra, figs))
        site = os.path.join(out, "site")
        chapters = build.load_chapters(ch)
        build.Site(fixture_data(), chapters, os.path.join(HERE, "fixture", "pages"), site, assets=False, manual_root=NO_MANUAL).write()
        return site, [p for p in build.check_site(site, ["one"]) if "captures.json" not in p and "media/" not in p]

    def test_a_registered_figure_gets_an_anchor_and_a_gallery_tile(self):
        site, problems = self.build(GOOD)
        self.assertEqual(problems, [])
        self.assertIn('href="one.html#fig-layers-test"', rd(os.path.join(site, "gallery.html")))
        self.assertIn("<title>Layer diagram</title>", rd(os.path.join(site, "one.html")))

    def test_bad_records_fail_the_build(self):
        for bad in (GOOD.replace("kind: diagram", "kind: poster"), GOOD.replace("track: shared", "track: moon"), GOOD.replace("thumb: svg", "thumb: jpeg"),
                    GOOD.replace("Layer diagram from content to the machine with Web and DOS columns.", "layers.png"), GOOD.replace("at: figure layers", "at: figure parity"),
                    GOOD + "\n\n" + GOOD):
            with self.assertRaises(mdlite.MdError, msg=bad):
                self.build(bad)

    def test_a_drawing_that_no_record_claims_fails(self):
        with self.assertRaises(mdlite.MdError):
            self.build(GOOD, extra="\n<!-- figure parity -->\n")

    def test_the_real_registry_is_complete(self):
        chapters = build.load_chapters(REAL)
        reg = figures.validate([r for c in chapters for r in c["figs"]], chapters)
        self.assertGreaterEqual(len(reg), 15)
        kinds = {r["kind"] for r in reg}
        self.assertTrue({"screenshot", "transcript", "diagram", "chart", "explainer"} <= kinds, kinds)
        caps = {c["file"] for c in flows.load_captures()}
        for r in reg:
            if r["thumb"].startswith("img:"):
                self.assertIn(r["thumb"][len("img:media/"):], caps, r["id"])


class ContentTests(unittest.TestCase):
    def test_real_chapters_are_well_formed(self):
        chapters = build.load_chapters(REAL)
        ids = [c["id"] for c in chapters]
        for need in ("prologue", "poc", "scaffold", "m0-core", "m0-presenters", "parity", "stack", "design", "roadmap", "findings", "archive"):
            self.assertIn(need, ids)
        for c in chapters:
            self.assertRegex(c["date"], r"^\d{4}-\d{2}-\d{2}$", c["id"])
            shelves = {mdlite.shelf_of(b) for b in c["blocks"]} - {None}
            self.assertEqual(shelves, {"catalogued", "uncatalogued", "wish"}, c["id"])
            if c["section"] != "reference":
                self.assertTrue(any(b["type"] == "quote" and b["lines"][0].startswith("**From the workbench") for b in c["blocks"]), c["id"])
            for b in c["blocks"]:
                if b["type"] == "quote" and b["lines"][0].startswith("**From the workbench"):
                    self.assertTrue(any(l.startswith("Source") for l in b["lines"][1:]), c["id"])

    def test_the_anatomy_file_is_valid_and_refuses_bad_boxes(self):
        data = dr.load_anatomy("dos")
        self.assertEqual(dr.validate_anatomy(data, gather.ROOT), [])
        bad = copy.deepcopy(data)
        bad["boxes"][0]["evidence"] = []
        self.assertTrue(any("evidence" in p for p in dr.validate_anatomy(bad, gather.ROOT)))
        bad = copy.deepcopy(data)
        bad["boxes"][0]["evidence"] = ["repo:no/such/file.rs"]
        self.assertTrue(any("does not exist" in p for p in dr.validate_anatomy(bad, gather.ROOT)))
        bad = copy.deepcopy(data)
        bad["boxes"][1]["col"] = 0
        self.assertTrue(any("overlap" in p for p in dr.validate_anatomy(bad, gather.ROOT)))
        self.assertEqual([b["status"] for b in data["boxes"] if b["id"] in ("game", "real")], ["planned", "planned"])

    def test_the_path_diagrams_name_code_that_exists(self):
        self.assertEqual(flows.check_paths(gather.ROOT), [])

    def test_the_captures_are_recorded_and_small(self):
        self.assertEqual(flows.check_captures(), [])
        total = sum(os.path.getsize(os.path.join(flows.MEDIA, f)) for f in os.listdir(flows.MEDIA))
        self.assertLess(total, 1024 * 1024)


class DrawingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.D = gather.gather(gather.ROOT, "HEAD", offline=True, dry_run=True)

    def test_every_drawing_renders_with_a_title(self):
        D, ctx = self.D, mdlite.Ctx()
        for fn in (dr.layers, dr.parity, dr.milestone_progress, charts.loc_by_crate, charts.tests_by_crate, charts.ci_results, charts.floppy):
            html = fn(D)
            self.assertIn("<svg", html, fn.__name__)
            self.assertIn("<title>", html, fn.__name__)
        self.assertIn("<svg", flows.input_path(D, gather.ROOT))
        self.assertIn("<svg", flows.frame_path(D, gather.ROOT))

    def test_the_replay_explainer_has_a_step_per_line(self):
        html = dr.replay(self.D, mdlite.Ctx())
        self.assertEqual(html.count('class="x-step"'), 1 + self.D["facts"]["golden"]["lines"] + 1 + 3)
        self.assertIn("a dash", html)

    def test_the_replay_refuses_a_measured_run_that_differs(self):
        D = copy.deepcopy(self.D)
        D["measured"]["dosReplay"] = [l.replace("efc699079baa6a88", "0" * 16) for l in D["measured"]["dosReplay"]]
        with self.assertRaises(ValueError):
            dr.replay(D, mdlite.Ctx())
        D = copy.deepcopy(self.D)
        D["measured"]["parityHash"] = "0" * 16
        with self.assertRaises(ValueError):
            dr.parity(D)

    def test_the_timer_explainer_agrees_with_the_measured_bios_ticks(self):
        html = dr.timer(self.D, mdlite.Ctx())
        self.assertIn("182 BIOS ticks", html)
        D = copy.deepcopy(self.D)
        D["measured"]["biosTicks"] = 170
        with self.assertRaises(ValueError):
            dr.timer(D, mdlite.Ctx())

    def test_the_dos_transcript_matches_the_golden_file(self):
        self.assertIn("FPU CW 027F", flows.dos_transcript(self.D))
        D = copy.deepcopy(self.D)
        D["facts"]["golden"]["text"] = D["facts"]["golden"]["text"].replace("efc699079baa6a88", "0" * 16)
        with self.assertRaises(ValueError):
            flows.dos_transcript(D)


FIXTURE_MANUAL = os.path.join(HERE, "fixture", "manual")


class ManualTests(unittest.TestCase):
    """The Manual page renders docs/MANUAL.md from a given root, copies its images, checks its links, and is left out when the file
    does not exist."""

    def site(self, root, evidence=None):
        out = tempfile.mkdtemp()
        site = os.path.join(out, "site")
        chapters = build.load_chapters(os.path.join(HERE, "fixture", "chapters"))
        build.Site(fixture_data(), chapters, os.path.join(HERE, "fixture", "pages"), site, assets=False, manual_root=root,
                   manual_evidence=evidence or os.path.join(FIXTURE_MANUAL, "evidence.json")).write()
        return site, chapters

    def copy_fixture(self):
        d = tempfile.mkdtemp()
        import shutil
        shutil.copytree(os.path.join(FIXTURE_MANUAL, "docs"), os.path.join(d, "docs"))
        return d

    def test_the_page_renders_the_manual_with_its_images_and_links(self):
        site, chapters = self.site(FIXTURE_MANUAL)
        page = rd(os.path.join(site, "manual.html"))
        self.assertIn("<h1>Fixture Game — Player&#x27;s Manual</h1>", page)
        self.assertIn("rendered from", page)
        self.assertIn('<img src="manual/docs/img/start.png" alt="The fixture game', page)
        self.assertTrue(os.path.exists(os.path.join(site, "manual", "docs", "img", "start.png")))
        self.assertIn('href="https://github.com/ScottMorris/lieutenant-fizz-2/blob/main/docs/OTHER.md#part"', page)
        self.assertIn("{\u200b{braces}}", page, "braces in the manual are text")
        self.assertIn('id="what-is-checked"', page)
        self.assertIn("Not covered: a fixture gap.", page)
        self.assertRegex(page, r"<th scope=\"row\">Unmapped</th><td><span class=\"an-st an-unverified\">uncatalogued")
        self.assertIn('href="manual.html"', rd(os.path.join(site, "index.html")))
        self.assertEqual([p for p in build.check_site(site, [c["id"] for c in chapters]) if "captures.json" not in p and "media/" not in p], [])
        manifest = json.loads(rd(os.path.join(site, "manifest.json")))
        self.assertIn("manual.html", manifest["publish"]["files"])
        self.assertIn("manual/docs/img/start.png", manifest["publish"]["files"])

    def test_a_missing_manual_leaves_out_the_page_and_shows_a_coming_row(self):
        site, _ = self.site(tempfile.mkdtemp())
        self.assertFalse(os.path.exists(os.path.join(site, "manual.html")))
        index = rd(os.path.join(site, "index.html"))
        self.assertNotIn('href="manual.html"', index)
        self.assertIn("The player's manual</b> <span class=\"pill pill-coming\">coming", index)

    def test_a_missing_image_or_a_broken_link_fails_the_build(self):
        for old, new in (("img/start.png", "img/gone.png"), ("OTHER.md#part", "GONE.md"), ("![The fixture game's start screen with a yellow square]", "![]")):
            d = self.copy_fixture()
            p = os.path.join(d, "docs", "MANUAL.md")
            wr(p, rd(p).replace(old, new))
            with self.assertRaises(mdlite.MdError, msg=new):
                self.site(d)

    def test_a_mapped_heading_the_manual_lacks_fails_the_build(self):
        d = tempfile.mkdtemp()
        wr(os.path.join(d, "evidence.json"), json.dumps({"sections": [{"section": "No such heading", "shelf": "none", "evidence": "x"}]}))
        with self.assertRaises(mdlite.MdError):
            self.site(FIXTURE_MANUAL, os.path.join(d, "evidence.json"))

    def test_the_real_evidence_map_matches_the_real_manual(self):
        text = manual.read(gather.ROOT)
        if text is None:
            self.skipTest("docs/MANUAL.md is not in this tree")
        hs = manual.headings(manual.parse(text)[1])
        for m in gather.read_json(os.path.join(TOOL, "manual-evidence.json"))["sections"]:
            self.assertIn(m["section"], hs)
            self.assertIn(m["shelf"], ("catalogued", "uncatalogued", "none"))


def prose_strings():
    """(where, text) for every piece of prose the archive publishes: the chapter and page sources, the JSON data that becomes text,
    the README, the templates, the string literals in the generator's Python, and a fresh build of the site."""
    out = []
    for sub in ("chapters", "pages", "templates"):
        for name in sorted(os.listdir(os.path.join(TOOL, sub))):
            out.append(("%s/%s" % (sub, name), rd(os.path.join(TOOL, sub, name))))
    if os.path.exists(os.path.join(gather.ROOT, manual.PATH)):
        out.append((manual.PATH, rd(os.path.join(gather.ROOT, manual.PATH))))  # the manual is the player's voice, and the patterns still apply
    for name in ("README.md", "roadmap.json", "not-verified.json", "anatomy-dos.json", "manual-evidence.json", os.path.join("media", "captures.json")):
        if os.path.exists(os.path.join(TOOL, name)):
            out.append((name, rd(os.path.join(TOOL, name))))
    for name in sorted(os.listdir(TOOL)):
        if name.endswith(".py"):
            text = rd(os.path.join(TOOL, name))
            out.append((name, "\n".join(re.findall(r'"([^"\n]{12,})"', text) + re.findall(r"#\s(.+)", text))))
    return out


class VoiceTests(unittest.TestCase):
    """VOICE.md's patterns: phrases that comment on the archive instead of describing Fizz 2 (grading its own order or method, contrasts
    with unnamed alternatives, truisms, telling the reader what to think). VOICE.md itself is exempt because it quotes them."""

    @classmethod
    def setUpClass(cls):
        cls.patterns = gather.read_json(os.path.join(TOOL, "voice-patterns.json"))["patterns"]

    def scan(self, items):
        hits = []
        for where, text in items:
            for p in self.patterns:
                for m in re.finditer(p["pattern"], text, flags=re.I):
                    hits.append("%s: %r (%s)" % (where, m.group(0), p["reason"]))
        return hits

    def test_the_sources_follow_voice_md(self):
        self.assertEqual(self.scan(prose_strings()), [])

    def test_a_fresh_build_follows_voice_md(self):
        with tempfile.TemporaryDirectory() as out:
            site = os.path.join(out, "site")
            build.Site(self.data(), build.load_chapters(REAL), os.path.join(TOOL, "pages"), site, assets=False).write()
            pages = [(f, re.sub(r"<[^>]+>", " ", rd(os.path.join(site, f)))) for f in sorted(os.listdir(site)) if f.endswith(".html")]
            self.assertEqual(self.scan(pages), [])

    def data(self):
        return gather.gather(gather.ROOT, "HEAD", offline=True, dry_run=True)

    def test_every_pattern_is_listed_in_voice_md(self):
        voice = rd(os.path.join(TOOL, "VOICE.md"))
        for p in self.patterns:
            self.assertIn("`%s`" % p["pattern"], voice, "VOICE.md does not list %s" % p["pattern"])
            self.assertTrue(re.search(p["pattern"], p["example"], flags=re.I), "the example of %s does not match it" % p["pattern"])

    def test_the_patterns_catch_the_desk_examples(self):
        bad = ["That is the right order for an archive and a poor one for learning.",
               "Most diagrams of how a program runs are drawn from memory. This one is drawn from the repository.",
               "I keep the archive, so I have only edited this part.", "The same honesty rules as the log apply, in a stricter form."]
        for s in bad:
            self.assertTrue(self.scan([("example", s)]), s)


if __name__ == "__main__":
    unittest.main()
