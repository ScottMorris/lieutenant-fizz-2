# The Manual page: docs/MANUAL.md rendered through the markdown reader, with its images copied into the site and its relative links
# pointed at the repository, followed by a table of what the archive has checked for each section.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# docs/MANUAL.md is the single source of the manual's text; nothing here copies or rewrites it. A relative link or image source is
# resolved against docs/, and the build refuses one whose file does not exist. Images are copied to manual/<path in the repository>
# in the site; other relative links go to the file on GitHub. The evidence table comes from manual-evidence.json, which maps a heading
# of the manual to the chapters, checks and pull requests that back it; a heading the file does not map is listed as uncatalogued.
import os
import re
import subprocess

import mdlite
from mdlite import esc, inline

HERE = os.path.dirname(os.path.abspath(__file__))
PATH = "docs/MANUAL.md"
IMAGE_EXT = (".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp")


class ManualCtx(mdlite.Ctx):
    """Resolves the manual's own relative links and images, and records what it could not resolve."""

    def __init__(self, root, repo_url, chapters):
        super().__init__({}, chapters, repo_url, ())
        self.root, self.images, self.problems = root, [], []

    def href(self, target):
        if re.match(r"^([a-z][a-z0-9+.-]*:|#)", target):
            return target
        path, _, frag = target.partition("#")
        rel = os.path.normpath(os.path.join(os.path.dirname(PATH), path))
        if rel.startswith("..") or not os.path.exists(os.path.join(self.root, rel)):
            self.problems.append("%s links to %s, which does not exist" % (PATH, target))
            return target
        if rel.lower().endswith(IMAGE_EXT):
            self.images.append(rel)
            return "manual/" + rel.replace(os.sep, "/")
        return "%s/blob/main/%s%s" % (self.repo_url, rel.replace(os.sep, "/"), ("#" + frag) if frag else "")


def read(root):
    """The manual's text from the tree, or None when docs/MANUAL.md does not exist."""
    p = os.path.join(root, PATH)
    if not os.path.exists(p):
        return None
    with open(p, encoding="utf-8") as f:
        return f.read()


def source_of(root):
    """Where the rendered text came from, as data: the last commit that changed the file when it is committed and unchanged,
    otherwise the working tree."""
    def git(*args):
        return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True).stdout.strip()
    tracked = git("ls-files", "--", PATH)
    dirty = git("status", "--porcelain", "--", PATH)
    if tracked and not dirty:
        return "rendered from `%s` at commit `%s`" % (PATH, git("log", "-1", "--format=%h", "--", PATH))
    return "rendered from an uncommitted `%s` in the working tree on `%s`" % (PATH, git("rev-parse", "--short", "HEAD") or "?")


def parse(text):
    """(title, blocks): the first `# ` line is the page title; the rest goes through the markdown reader. Placeholder braces in the
    manual are text, never placeholders."""
    lines = text.replace("{{", "{​{").splitlines()
    title = "Player's manual"
    if lines and lines[0].startswith("# "):
        title, lines = lines[0][2:].strip(), lines[1:]
    return title, mdlite.parse_blocks("\n".join(lines))


def headings(blocks):
    return [b["text"] for b in blocks if b["type"] == "h"]


def evidence_table(blocks, mapping, data, ctx, chapter_state, tag):
    """One row per heading of the manual: the evidence the archive holds for it, or uncatalogued."""
    hs = headings(blocks)
    unknown = [m["section"] for m in mapping if m["section"] not in hs]
    if unknown:
        raise mdlite.MdError("manual-evidence.json maps headings that %s does not have: %s" % (PATH, ", ".join(unknown)))
    by = {m["section"]: m for m in mapping}
    rows = []
    for h in hs:
        m = by.get(h)
        if not m:
            rows.append('<tr><th scope="row">%s</th><td><span class="an-st an-unverified">uncatalogued</span></td><td>No check in the archive is mapped to this section.</td><td></td></tr>' % esc(h))
            continue
        shelf = m["shelf"]
        if shelf not in ("catalogued", "uncatalogued", "none"):
            raise mdlite.MdError("manual-evidence.json: %s has an unknown shelf %r" % (h, shelf))
        state = tag(chapter_state(data, m.get("prs", []))) if m.get("prs") else ""
        word = {"catalogued": '<span class="an-st an-verified">catalogued</span>', "uncatalogued": '<span class="an-st an-unverified">uncatalogued</span>',
                "none": '<span class="an-st an-planned">nothing to check</span>'}[shelf]
        rows.append('<tr><th scope="row">%s</th><td>%s</td><td>%s%s</td><td class="nw">%s</td></tr>' % (
            esc(h), word, inline(m["evidence"], ctx), ('<div class="sub">Not covered: %s</div>' % inline(m["gaps"], ctx)) if m.get("gaps") else "", state))
    return ('<section class="evidence"><h2 id="what-is-checked">What is checked</h2><p class="how">Each section of the manual with the check in this archive that backs it, '
            'from <code>tools/progress-report/manual-evidence.json</code>. The state tag is that of the pull requests named, from <code>gh</code>.</p>'
            '<div class="tbl"><table><caption class="sr">Manual sections and their evidence</caption><thead><tr><th scope="col">Section</th><th scope="col">Shelf</th>'
            '<th scope="col">Evidence</th><th scope="col">State</th></tr></thead><tbody>%s</tbody></table></div></section>') % "".join(rows)
