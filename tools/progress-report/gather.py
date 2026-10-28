#!/usr/bin/env python3
# Gathers every number and record the Fizz 2 progress report shows, from the git history and tree of `main` and from `gh`.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/gather.py [--ref main] [--out data.json] [--offline] [--update-snapshot] [--dry-run]
#
# Standard library only, plus `git` and (optionally) `gh` on the PATH. Adapted from Desk's tools/progress-report/gather.py (same
# author, same licence). History comes from `git log`, counts from the tree of the ref, and pull request and CI data from `gh`.
# Fizz 2 merges a stack of pull requests with one merge commit on main, so a merged pull request is found by its head commit, not
# only by a `(#N)` subject: its commits are the range from the head of the pull request below it (or the merge commit's first
# parent) to its own head. When `gh` is missing or signed out the data falls back to snapshots/gh.json and says so under
# "sources". Nothing in the repository is written; --dry-run prints a summary, writes no file, and works in a shallow clone.
import datetime
import json
import os
import re
import subprocess
import sys

USAGE = "usage: gather.py [--ref REF] [--out FILE] [--offline] [--update-snapshot] [--dry-run]"
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
# The GitHub repository the gh queries name explicitly, so they work from a clone whose remote is a local path.
GH_REPO = os.environ.get("FIZZ2_GH_REPO", "ScottMorris/lieutenant-fizz-2")
REPO_URL = "https://github.com/" + GH_REPO
REPORT_BRANCH_PREFIX = "docs/progress-report"


class Repo:
    """Read-only access to one ref of a git repository."""

    def __init__(self, root, ref):
        self.root, self.ref = root, ref
        self._paths = None

    def git(self, *args, check=True):
        p = subprocess.run(["git", *args], cwd=self.root, capture_output=True, text=True, encoding="utf-8", errors="replace")
        if check and p.returncode != 0:
            raise RuntimeError("git %s: %s" % (" ".join(args), p.stderr.strip()))
        return p.stdout

    def show(self, path, ref=None):
        p = subprocess.run(["git", "show", "%s:%s" % (ref or self.ref, path)], cwd=self.root, capture_output=True)
        return p.stdout.decode("utf-8", errors="replace") if p.returncode == 0 else None

    def blobs(self, paths):
        """{path: bytes} for many files of the ref, read through one `git cat-file --batch` process."""
        out = {}
        if not paths:
            return out
        p = subprocess.run(["git", "cat-file", "--batch"], cwd=self.root, input="".join("%s:%s\n" % (self.ref, x) for x in paths).encode("utf-8"),
                           capture_output=True)
        buf, pos = p.stdout, 0
        for path in paths:
            nl = buf.index(b"\n", pos)
            head = buf[pos:nl].split()
            if len(head) == 3 and head[1] == b"blob":
                size = int(head[2])
                out[path] = buf[nl + 1:nl + 1 + size]
                pos = nl + 1 + size + 1
            else:
                out[path] = None
                pos = nl + 1
        return out

    def paths(self):
        if self._paths is None:
            self._paths = {}
            for line in self.git("ls-tree", "-r", "-l", self.ref).splitlines():
                meta, path = line.split("\t", 1)
                size = meta.split()[3]
                self._paths[path] = int(size) if size.isdigit() else 0
        return self._paths

    def matching(self, pattern):
        rx = re.compile(pattern)
        return sorted(p for p in self.paths() if rx.search(p))

    def lines(self, pattern):
        return sum((b or b"").count(b"\n") for b in self.blobs(self.matching(pattern)).values())

    def count(self, pattern, rx):
        """Occurrences of the bytes regex `rx` (multiline) in the files matching `pattern`."""
        return sum(len(re.findall(rx, b or b"", flags=re.M)) for b in self.blobs(self.matching(pattern)).values())

    def is_shallow(self):
        return self.git("rev-parse", "--is-shallow-repository", check=False).strip() == "true"


def has_commit(root, sha):
    return bool(sha) and subprocess.run(["git", "cat-file", "-e", sha + "^{commit}"], cwd=root, capture_output=True).returncode == 0


def is_ancestor(root, a, b):
    return subprocess.run(["git", "merge-base", "--is-ancestor", a, b], cwd=root, capture_output=True).returncode == 0


def branch_ref(root, name):
    """The remote-tracking ref of a branch if there is one, else the local branch, else None."""
    for cand in ("origin/" + name, name):
        if subprocess.run(["git", "rev-parse", "--verify", "-q", cand + "^{commit}"], cwd=root, capture_output=True).returncode == 0:
            return cand
    return None


# ---- gh ------------------------------------------------------------------------------------------------------------------------

def gh_json(*args):
    """Runs `gh` and parses its JSON output, or returns None when gh is missing, signed out or fails."""
    try:
        p = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if p.returncode != 0:
        return None
    try:
        return json.loads(p.stdout)
    except ValueError:
        return None


def gh_available():
    try:
        return subprocess.run(["gh", "auth", "status"], capture_output=True, timeout=30).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def slim_pr(p):
    """A pull request as gh returns it, with the check rollup reduced to what the report uses."""
    out = dict(p)
    out["checks"] = [{"name": c.get("name"), "workflow": c.get("workflowName"), "status": c.get("status"), "conclusion": c.get("conclusion")}
                     for c in out.pop("statusCheckRollup", None) or []]
    return out


def ci_of(checks):
    """passing, failing, pending or none, from the checks on a commit."""
    if not checks:
        return "none"
    if any(c.get("conclusion") in ("FAILURE", "CANCELLED", "TIMED_OUT") for c in checks):
        return "failing"
    if all(c.get("status") == "COMPLETED" and c.get("conclusion") in ("SUCCESS", "NEUTRAL", "SKIPPED") for c in checks):
        return "passing"
    return "pending"


def run_ci(run):
    """passing, failing, cancelled or pending, from one workflow run of `gh run list`."""
    if run.get("status") and run["status"] != "completed":
        return "pending"
    c = run.get("conclusion")
    return {"success": "passing", "cancelled": "cancelled", "": "pending", None: "pending"}.get(c, "failing")


def pr_state(p):
    """The state tag of a pull request from gh: merged, in review (open and ready), in progress (a draft) or closed."""
    if p.get("mergedAt"):
        return "merged"
    if p.get("state") == "OPEN":
        return "in progress" if p.get("isDraft") else "in review"
    return "closed"


def gh_data(offline):
    snap_path = os.path.join(HERE, "snapshots", "gh.json")
    if not offline and gh_available():
        prs = gh_json("pr", "list", "--repo", GH_REPO, "--state", "all", "--limit", "200", "--json",
                      "number,title,headRefName,baseRefName,headRefOid,mergeCommit,createdAt,mergedAt,state,isDraft,additions,deletions,changedFiles,labels,url,"
                      "statusCheckRollup")
        runs = gh_json("run", "list", "--repo", GH_REPO, "--branch", "main", "--limit", "200", "--json",
                       "status,conclusion,headSha,displayTitle,createdAt,url,workflowName,event")
        if prs is not None and runs is not None:
            return {"prs": sorted((slim_pr(p) for p in prs), key=lambda p: p["number"]), "runs": runs}, "gh", None
    if os.path.exists(snap_path):
        snap = read_json(snap_path)
        return snap["data"], "snapshot", snap.get("capturedAt")
    return {"prs": [], "runs": []}, "none", None


# ---- history -------------------------------------------------------------------------------------------------------------------

def kind_of(path):
    base = os.path.basename(path)
    if path.endswith(".md"):
        return "docs"
    if path.startswith("assets/"):
        return "assets"
    if base.startswith("check") or base.endswith(("-replay.txt", ".mjs")) or "/tests/" in path:
        return "tests"
    if path.startswith(("tools/", ".github/")) or path.endswith(".sh"):
        return "tools"
    return "source"


def first_parent(repo):
    out = []
    for line in repo.git("log", "--first-parent", "--format=%H%x09%h%x09%aI%x09%P%x09%s", repo.ref).splitlines():
        full, short, date, parents, subject = line.split("\t", 4)
        out.append({"hash": short, "full": full, "date": date, "subject": subject, "parents": parents.split()})
    return out


def commits_between(repo, base, head):
    out = []
    for line in repo.git("log", "--reverse", "--format=%h%x09%aI%x09%s%x09%P", head, "^" + base).splitlines():
        h, d, s, p = line.split("\t")
        out.append({"hash": h, "date": d, "subject": s, "merge": len(p.split()) > 1})
    return out


def sizes(repo, base, head):
    kinds, added, deleted = {}, 0, 0
    for line in repo.git("diff", "--numstat", base, head).splitlines():
        a, d, path = line.split("\t", 2)
        k = kinds.setdefault(kind_of(path), {"files": 0, "added": 0, "binary": 0})
        k["files"] += 1
        if a == "-":
            k["binary"] += 1
        else:
            k["added"] += int(a)
            added += int(a)
            deleted += int(d)
    return kinds, added, deleted


def history(repo, gh_prs):
    """The merged pull requests as git records them. A pull request counts as merged into the ref only when gh says it is merged and
    its head commit is reachable from the ref; with no gh data, a merge commit whose subject ends in (#N) stands for pull request N."""
    root = repo.root
    fp = first_parent(repo)
    oldest_first = list(reversed(fp))
    by_branch = {p["headRefName"]: p for p in gh_prs if p.get("mergedAt")}
    prs = []
    shallow = repo.is_shallow()
    for p in sorted((p for p in gh_prs if p.get("mergedAt")), key=lambda p: (p["mergedAt"], p["number"])):
        head = p.get("headRefOid")
        if shallow and not has_commit(root, head):
            # A shallow clone (CI) lacks the branches: take the pull request from gh alone, with no commits of its own, and say so.
            merge = ((p.get("mergeCommit") or {}).get("oid") or head or "")
            below = by_branch.get(p["baseRefName"])
            prs.append({"number": p["number"], "title": p["title"], "head": (head or "")[:7], "headFull": head, "base": p["baseRefName"], "branch": p["headRefName"],
                        "mergeCommit": merge[:7], "mergeFull": merge, "mergedAt": p["mergedAt"], "landedAt": p["mergedAt"], "commits": [], "commitCount": 0,
                        "kinds": {}, "added": p.get("additions") or 0, "deleted": p.get("deletions") or 0, "baseCommit": None,
                        "stackedOn": below["number"] if below else None, "fromGhOnly": True})
            continue
        if not has_commit(root, head) or not is_ancestor(root, head, repo.ref):
            continue
        landing = next((c for c in oldest_first if is_ancestor(root, head, c["full"])), None)
        if not landing:
            continue
        below = by_branch.get(p["baseRefName"])
        if below and has_commit(root, below.get("headRefOid")) and is_ancestor(root, below["headRefOid"], head):
            base = below["headRefOid"]
        else:
            base = landing["parents"][0] if landing["parents"] else head
        kinds, added, deleted = sizes(repo, base, head)
        commits = commits_between(repo, base, head)
        prs.append({"number": p["number"], "title": p["title"], "head": head[:7], "headFull": head, "base": p["baseRefName"], "branch": p["headRefName"],
                    "mergeCommit": landing["hash"], "mergeFull": landing["full"], "mergedAt": p["mergedAt"], "landedAt": landing["date"],
                    "commits": commits, "commitCount": len(commits), "kinds": kinds, "added": added, "deleted": deleted,
                    "baseCommit": base[:7], "stackedOn": below["number"] if below and base == below.get("headRefOid") else None})
    if not gh_prs:
        for c in oldest_first:
            m = re.search(r"\(#(\d+)\)$", c["subject"])
            if m and len(c["parents"]) == 2:
                base, head = c["parents"]
                kinds, added, deleted = sizes(repo, base, head)
                commits = commits_between(repo, base, head)
                prs.append({"number": int(m.group(1)), "title": re.sub(r"\s*\(#\d+\)$", "", c["subject"]), "head": head[:7], "headFull": head, "base": "main",
                            "branch": None, "mergeCommit": c["hash"], "mergeFull": c["full"], "mergedAt": c["date"], "landedAt": c["date"],
                            "commits": commits, "commitCount": len(commits), "kinds": kinds, "added": added, "deleted": deleted, "baseCommit": base[:7], "stackedOn": None})
    return fp, prs


def stacks(prs):
    """Groups the merged pull requests by the merge commit that brought them into the ref, bottom of the stack first."""
    out = {}
    for p in prs:
        out.setdefault(p["mergeCommit"], []).append(p)
    result = []
    for commit, group in out.items():
        order, nums = [], {p["number"] for p in group}
        bottom = [p for p in group if p["stackedOn"] not in nums]
        while bottom:
            cur = bottom.pop(0)
            order.append(cur)
            bottom += [p for p in group if p["stackedOn"] == cur["number"]]
        result.append({"mergeCommit": commit, "landedAt": group[0]["landedAt"], "numbers": [p["number"] for p in order]})
    return sorted(result, key=lambda s: s["landedAt"])


# ---- the tree ------------------------------------------------------------------------------------------------------------------

CRATES = [
    ("fizz-sim", r"^crates/fizz-sim/.*\.rs$"), ("fizz-phys", r"^crates/fizz-phys/.*\.rs$"), ("fizz-ep2", r"^crates/fizz-ep2/.*\.rs$"),
    ("fizz-hal", r"^crates/fizz-hal/.*\.rs$"), ("fizz-machine", r"^crates/fizz-machine/.*\.rs$"), ("fizz-menu", r"^crates/fizz-menu/.*\.rs$"), ("fizz-view", r"^crates/fizz-view/.*\.rs$"), ("fizz-slice-view", r"^crates/fizz-slice-view/.*\.rs$"),
    ("fizz-present-dos", r"^crates/fizz-present-dos/.*\.rs$"), ("fizz-present-web", r"^crates/fizz-present-web/.*\.rs$"), ("fizz-sfx", r"^crates/fizz-sfx/.*\.rs$"), ("fizz-music", r"^crates/fizz-music/.*\.rs$"),
    ("fizz-dos-rt", r"^dos/rt/.*\.rs$"), ("fizz2", r"^dos/fizz2/.*\.rs$"), ("rtsmoke", r"^dos/rtsmoke/.*\.rs$"),
]
# The slice's golden transcript: the one the parity and DOS gates compare. Milestone 0's transcript, script and BIOS bound belong to
# the deleted fizz-hello crate and are kept as history in snapshots/m0.json.
GOLDEN = "crates/fizz-ep2/slice-replay.txt"


def parse_int(text):
    return int(text.replace(",", ""))


def scrape(repo):
    """Numbers stated in the repository's own documents and scripts, read back so the pages cannot drift from them."""
    platform = repo.show("docs/PLATFORM.md") or ""
    check_dos = repo.show("tools/check-dos.sh") or ""
    ci = repo.show(".github/workflows/ci.yml") or ""
    out = {}
    for key, text, rx in [
        ("exeBytes", platform, r"`FIZZ2\.EXE` is ([\d,]+) bytes"),
        ("floppyRaw", platform, r"holds ([\d,]+) bytes raw"),
        ("floppyData", platform, r"about ([\d,]+) bytes of data area"),
        ("pitDivisor", platform, r"divisor (\d+), which is"),
        ("budget", check_dos, r"^BUDGET=(\d+)$"),
        ("freedosKB", platform, r"FreeDOS needs about (\d+) KB"),
        ("saveKB", platform, r"and (\d+) KB is kept free for saves"),
        ("plainKB", platform, r"\| Plain \| About [\d.]+ MB \| (\d+) KB kept free"),
    ]:
        m = re.search(rx, text, re.M)
        out[key] = parse_int(m.group(1)) if m else None
    m = re.search(r"control word is `0x([0-9A-F]{4})`", platform)
    out["controlWord"] = m.group(1) if m else None
    m = re.search(r"pinned at `=([\d.]+)`", platform)
    out["libm"] = m.group(1) if m else None
    m = re.search(r"wasm-bindgen-cli --version ([\d.]+)", ci)
    out["wasmBindgen"] = m.group(1) if m else None
    m = re.search(r"ci-dos@(sha256:[0-9a-f]{64})", ci)
    out["ciImage"] = m.group(1) if m else None
    m = re.search(r"BIOS TICKS .*?-ge (\d+).*?-le (\d+)", check_dos, re.S)
    out["biosRange"] = "%s to %s" % m.groups() if m else None
    return out


def m0_history():
    """Milestone 0's golden transcript, script and BIOS tick bound, frozen in snapshots/m0.json when `fizz-hello` was deleted."""
    return read_json(os.path.join(HERE, "snapshots", "m0.json"))


def golden(repo):
    text = repo.show(GOLDEN)
    if text is None:
        return {"text": None, "final": None, "ticks": 0}
    m = re.search(r"^FINAL TICK (\d+) HASH ([0-9a-f]{16})$", text, re.M)
    return {"text": text, "final": m.group(2) if m else None, "finalTick": int(m.group(1)) if m else None,
            "lines": len(re.findall(r"^TICK ", text, re.M)), "events": len(re.findall(r"^EVENT ", text, re.M))}


def tree_facts(repo):
    paths = repo.paths()
    loc = {name: repo.lines(rx) for name, rx in CRATES}
    loc["web"] = repo.lines(r"^web/.*\.(js|html|css)$")
    loc["tools"] = repo.lines(r"^tools/[^/]+\.(sh|py|mjs)$")
    loc["docs"] = repo.lines(r"^docs/.*\.md$")
    loc["rust"] = repo.lines(r"^(crates|dos)/.*\.rs$")
    tests = {name: repo.count(rx, rb"^\s*#\[test\]") for name, rx in CRATES}
    return {
        "files": len(paths), "docs": len(repo.matching(r"^docs/[^/]+\.md$")),
        "crates": len(repo.matching(r"^(crates/[^/]+|dos/[^/]+)/Cargo\.toml$")),
        "loc": loc, "tests": tests, "testsAll": sum(tests.values()), "scraped": scrape(repo), "golden": golden(repo), "m0": m0_history(),
        "checkScripts": repo.matching(r"^tools/check[^/]*$"),
    }


# ---- the whole record ----------------------------------------------------------------------------------------------------------

def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def resolve_roadmap(roadmap, prs, open_prs=()):
    """Checks every step against what is merged and what is open, and gives each a state tag from gh. A step that claims pull requests
    keeps a 'done' or 'done-emu' state only if every one is merged with CI passing on the commit that brought it into main. If one is
    open, it drops to 'review'; if one cannot be found, or CI did not pass, it drops to 'progress'. Returns per-milestone progress
    and any warnings."""
    by_num = {p["number"]: p for p in prs}
    open_by = {p["number"]: p for p in open_prs}
    warnings = []
    for n in roadmap["nodes"]:
        nums = n.get("prs") or []
        n["prs"] = nums
        tags = ["merged" if k in by_num else (open_by[k]["state"] if k in open_by else "unknown") for k in nums]
        n["merged"] = bool(nums) and all(t == "merged" for t in tags)
        if nums:
            n["ci"] = "passing" if all((by_num.get(k) or open_by.get(k) or {}).get("ci") == "passing" for k in nums) else "not all passing"
            if "unknown" in tags and n["state"] in ("done", "done-emu", "review"):
                warnings.append("step %s drops to progress: PR #%s was not found" % (n["id"], ", #".join(str(k) for k in nums if k not in by_num and k not in open_by)))
                n["state"] = "progress"
            elif n["merged"] and n["state"] in ("done", "done-emu") and n["ci"] != "passing":
                warnings.append("step %s drops to progress: its pull requests are merged but CI has not passed on the merge commit" % n["id"])
                n["state"] = "progress"
            elif not n["merged"] and "unknown" not in tags and n["state"] in ("done", "done-emu"):
                warnings.append("step %s drops to review: a pull request is open, not merged" % n["id"])
                n["state"] = "review"
        if "in progress" in tags:
            n["tag"] = "in progress"
        elif "in review" in tags:
            n["tag"] = "in review"
        elif nums and n["merged"]:
            n["tag"] = "merged"
        else:
            n["tag"] = "planned" if nums or n["state"] not in ("done", "done-emu") else "merged"
    progress = {}
    for n in roadmap["nodes"]:
        progress.setdefault(n["milestone"], {}).setdefault(n["state"], 0)
        progress[n["milestone"]][n["state"]] += 1
    return progress, warnings


def open_work(gh_prs, ci_by_sha):
    out = []
    for p in sorted((p for p in gh_prs if p.get("state") == "OPEN"), key=lambda p: p["number"]):
        out.append({"number": p["number"], "title": p["title"], "branch": p["headRefName"], "base": p["baseRefName"], "head": (p.get("headRefOid") or "")[:7],
                    "state": pr_state(p), "ci": ci_of(p.get("checks")), "createdAt": p.get("createdAt"), "url": p.get("url"),
                    "labels": [l["name"] for l in p.get("labels", [])], "ghAdditions": p.get("additions") or 0, "ghDeletions": p.get("deletions") or 0,
                    "isReport": p["headRefName"].startswith(REPORT_BRANCH_PREFIX)})
    return out


def metric(value, label, how):
    return {"value": value, "label": label, "how": how}


def gather(root=ROOT, ref="main", offline=False, dry_run=False):
    # `main` means the remote's main when there is one: the local branch of a worktree-based checkout can be behind what gh reports.
    repo = Repo(root, (branch_ref(root, "main") or "main") if ref == "main" else ref)
    if repo.git("rev-parse", "--verify", repo.ref, check=False).strip() == "":
        raise SystemExit("no such ref: %s" % ref)
    gh, source, captured = gh_data(offline or dry_run)
    fp, prs = history(repo, gh["prs"])
    facts = tree_facts(repo)
    ci_by_sha = {}
    for r in gh["runs"]:
        if r.get("workflowName") in ("CI", None) and r.get("event", "push") == "push":
            ci_by_sha.setdefault(r["headSha"], run_ci(r))
    gh_by_num = {p["number"]: p for p in gh["prs"]}
    warnings_gh = []
    for p in prs:
        g = gh_by_num.get(p["number"], {})
        p["url"] = g.get("url") or "%s/pull/%d" % (REPO_URL, p["number"])
        p["labels"] = [l["name"] for l in g.get("labels", [])]
        p["createdAt"] = g.get("createdAt")
        p["ghAdditions"], p["ghDeletions"] = g.get("additions"), g.get("deletions")
        p["ci"] = ci_by_sha.get(p["mergeFull"], "unknown")
        p["headCi"] = ci_of(g.get("checks"))
        p["checks"] = sorted({c["name"] for c in g.get("checks", []) if c.get("name")})
        p["ghMergeCommit"] = ((g.get("mergeCommit") or {}).get("oid") or "")[:7] or None
        if p["ghMergeCommit"] and p["ghMergeCommit"] != p["mergeCommit"]:
            warnings_gh.append("PR #%d: gh names merge commit %s but git finds its head first on main at %s" % (p["number"], p["ghMergeCommit"], p["mergeCommit"]))
    opens = open_work(gh["prs"], ci_by_sha)
    roadmap = read_json(os.path.join(HERE, "roadmap.json"))
    progress, warnings = resolve_roadmap(roadmap, prs, opens)
    warnings += warnings_gh
    thin = [p["number"] for p in prs if p.get("fromGhOnly")]
    if thin:
        warnings.append("shallow clone: pull requests %s are taken from gh without their commits" % ", ".join("#%d" % n for n in thin))
    mpath = os.path.join(HERE, "snapshots", "measured.json")
    measured = read_json(mpath) if os.path.exists(mpath) else {}
    loc, sc, gold = facts["loc"], facts["scraped"], facts["golden"]
    merged_ci_ok = sum(1 for p in prs if p["ci"] == "passing")
    head_ci_ok = sum(1 for p in prs if p["headCi"] == "passing")
    subject_open = [o for o in opens if not o["isReport"]]
    metrics = {
        "prs": metric(len(prs), "pull requests merged", "Pull requests that `gh` reports merged and whose head commit is reachable from %s (`git merge-base --is-ancestor`)." % ref),
        "ciGreen": metric(("%d of %d" % (merged_ci_ok, len(prs))) if source != "none" else "unknown", "merged with CI green on main",
                          "The CI workflow's conclusion on the commit that brought each pull request into main, from `gh run list --branch main` (source: %s)." % source),
        "headCiGreen": metric(("%d of %d" % (head_ci_ok, len(prs))) if source != "none" else "unknown", "CI green on the pull request head",
                              "Every check on the head commit of each merged pull request, from the `statusCheckRollup` of `gh pr list` (source: %s)." % source),
        "commitsFirst": metric(len(fp), "commits on the first-parent line", "`git rev-list --count --first-parent %s`." % ref),
        "commitsAll": metric(int(repo.git("rev-list", "--count", repo.ref).strip() or 0), "commits in all", "`git rev-list --count %s`, every commit inside the merged branches included." % ref),
        "files": metric(facts["files"], "files in the tree", "`git ls-tree -r %s | wc -l`." % ref),
        "crates": metric(facts["crates"], "Rust packages", "`Cargo.toml` files directly under `crates/*/` and `dos/*/` in the tree of %s." % ref),
        "rustLines": metric(loc["rust"], "lines of Rust", "Newlines in `crates/**/*.rs` and `dos/**/*.rs` in the tree of %s." % ref),
        "rtLines": metric(loc["fizz-dos-rt"], "lines in the DOS runtime", "Newlines in `dos/rt/**/*.rs`, the crate `fizz-dos-rt`."),
        "tests": metric(facts["testsAll"], "Rust test declarations", "Lines that are `#[test]` under `crates/` and `dos/`. A count of declarations, not a test runner's total."),
        "docLines": metric(loc["docs"], "lines of reference documents", "Newlines in `docs/*.md` in the tree of %s." % ref),
        "goldenTicks": metric(gold.get("finalTick"), "ticks in the golden replay", "The tick of the `FINAL` line of `%s`." % GOLDEN),
        "exeBytes": metric(sc["exeBytes"], "bytes in FIZZ2.EXE, unstripped", "Stated in `docs/PLATFORM.md` (Size budget) and printed as `SIZE` by `tools/check-dos.sh`; the measured run is in `snapshots/measured.json`."),
        "budget": metric(sc["budget"], "byte budget for FIZZ2.EXE", "`BUDGET=` in `tools/check-dos.sh`, which fails the check above it. Provisional."),
        "prsOpen": metric(len(subject_open), "pull requests open", "Open pull requests from `gh pr list`, not counting the ones that carry this report."),
        "openCi": metric("%d of %d" % (sum(1 for o in subject_open if o["ci"] == "passing"), len(subject_open)), "open pull requests with CI green",
                         "Every check on the head commit of each open pull request, from `gh pr list` (`statusCheckRollup`)."),
    }
    if sc["exeBytes"] and sc["budget"]:
        metrics["budgetUsed"] = metric("%.1f%%" % (100.0 * sc["exeBytes"] / sc["budget"]), "of the EXE budget used", "`FIZZ2.EXE` bytes divided by `BUDGET` in `tools/check-dos.sh`.")
    missing = [k for k, m in metrics.items() if m["value"] is None]
    missing += ["scraped." + k for k, v in sc.items() if v is None]
    if not gold.get("final"):
        missing.append("golden.final")
    if measured and measured.get("parityHash") != gold.get("final"):
        warnings.append("snapshots/measured.json records PARITY %s but %s ends %s" % (measured.get("parityHash"), GOLDEN, gold.get("final")))
    now = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
    return {
        "generated": now, "ref": ref, "head": fp[0]["hash"] if fp else None, "shallow": repo.is_shallow(),
        "sources": {"gh": source, "ghCapturedAt": captured, "offline": bool(offline or dry_run)},
        "repo": {"url": REPO_URL, "slug": GH_REPO}, "main": {"log": fp, "files": facts["files"]},
        "prs": prs, "stacks": stacks(prs), "open": {"prs": opens}, "facts": facts, "measured": measured,
        "roadmap": roadmap, "progress": progress, "warnings": warnings, "missing": sorted(set(missing)), "metrics": metrics,
    }


def summary(data):
    lines = ["ref %s at %s (%s)" % (data["ref"], data["head"], "shallow clone" if data["shallow"] else "full history"),
             "gh data: %s" % data["sources"]["gh"], "pull requests merged: %d" % len(data["prs"]),
             "files in tree: %d" % data["main"]["files"],
             "open pull requests: %s" % (", ".join("#%d" % o["number"] for o in data["open"]["prs"]) or "none")]
    for k, m in data["metrics"].items():
        lines.append("  %-13s %-10s %s" % (k, m["value"], m["label"]))
    lines += ["warning: " + w for w in data["warnings"]]
    lines += ["MISSING: " + m for m in data["missing"]]
    return "\n".join(lines)


def main(argv):
    args = argv[1:]
    opts = {"--ref": "main", "--out": None}
    flags = set()
    i = 0
    while i < len(args):
        if args[i] in opts and i + 1 < len(args):
            opts[args[i]] = args[i + 1]
            i += 2
        elif args[i] in ("--offline", "--update-snapshot", "--dry-run"):
            flags.add(args[i])
            i += 1
        else:
            print(USAGE, file=sys.stderr)
            return 2
    dry = "--dry-run" in flags
    ref = opts["--ref"]
    if dry and ref == "main":
        # The dry run reads whatever is checked out, so it also works in a CI clone that has no local `main`.
        ref = "HEAD"
    offline = "--offline" in flags
    if "--update-snapshot" in flags:
        if not gh_available():
            print("cannot update the snapshot: gh is not available or not signed in", file=sys.stderr)
            return 1
        fresh, source, _ = gh_data(False)
        if source != "gh":
            print("cannot update the snapshot: the gh queries failed", file=sys.stderr)
            return 1
        with open(os.path.join(HERE, "snapshots", "gh.json"), "w", encoding="utf-8") as f:
            json.dump({"capturedAt": datetime.datetime.now().astimezone().isoformat(timespec="seconds"), "data": fresh}, f, indent=1, sort_keys=True)
            f.write("\n")
        offline = True  # read back from the snapshot just written, so the two cannot differ
    data = gather(ROOT, ref, offline=offline, dry_run=dry)
    if dry:
        print(summary(data))
        return 1 if data["missing"] else 0
    text = json.dumps(data, indent=1, ensure_ascii=False)
    if opts["--out"]:
        os.makedirs(os.path.dirname(os.path.abspath(opts["--out"])), exist_ok=True)
        with open(opts["--out"], "w", encoding="utf-8") as f:
            f.write(text + "\n")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
