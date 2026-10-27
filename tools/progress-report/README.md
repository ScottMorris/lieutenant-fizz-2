# progress-report

The generator of the Fizz 2 Progress Report, a multi-page archive of the project's history and status narrated by Gregory, the archivist. It reads the history and tree of `main` with `git` and the pull request and CI data with `gh`, and writes a static site to `dist/progress-report/` (git-ignored). Python standard library only, plus `git` and, optionally, `gh`. The prose follows [`VOICE.md`](VOICE.md).

It is adapted from Desk's `tools/progress-report/` (`liminal-hq/desk`), which has the same author and the same licence, Apache-2.0 OR MIT. `mdlite.py`, `figures.py` and the templates are copies with small changes; `gather.py`, `build.py`, `drawings.py` and the tests are adapted; the chapters, data files, captures, `charts.py`, `flows.py` and the capture tools are written for Fizz 2. Each file's header says which.

## Contents

- [Quick start](#quick-start)
- [Layout](#layout)
- [How the data is gathered](#how-the-data-is-gathered)
- [Chapters and figures](#chapters-and-figures)
- [Captures](#captures)
- [Checks](#checks)
- [Publishing](#publishing)

## Quick start

```bash
python3 tools/progress-report/build.py --check               # gathers from git and gh, builds dist/progress-report, checks it
python3 tools/progress-report/build.py --offline --check     # the same from snapshots/gh.json, with no network
python3 -m http.server -d dist/progress-report 9000          # then open http://127.0.0.1:9000/
```

`gather.py --out FILE` writes the gathered record, and `build.py --data FILE` builds from it. `gather.py --update-snapshot` refreshes `snapshots/gh.json` from a signed-in `gh`. `gather.py --dry-run` prints a summary and writes nothing; it reads whatever is checked out and works in a shallow clone, which is what `tools/check.sh` runs.

The site is living: after new pull requests merge, rebuilding refreshes the counts, the evidence tables, the CI results and every state tag, and the roadmap moves a step down a shelf, with a warning, if its pull requests stop being merged with CI green on `main`.

## Layout

| Path | What it is |
|---|---|
| `gather.py` | Reads `git` and `gh`, counts from the tree, and returns one record |
| `build.py` | Renders the pages, copies the banner and the captures, writes `manifest.json`, and checks the result with `--check` |
| `mdlite.py` | The small markdown reader for chapters |
| `figures.py` | The figure registry: parsing, validation and the gallery's previews |
| `drawings.py` | The layer, parity, stack and anatomy diagrams, the milestone chart, and the replay and timer step-throughs |
| `charts.py` | Lines and tests per package, pull request sizes, CI results and the floppy budget |
| `flows.py` | The input and frame path diagrams, the screenshots, the DOSBox transcript and the replay script table |
| `manual.py`, `manual-evidence.json` | The Manual page from `docs/MANUAL.md`, and the map from its sections to the archive's evidence |
| `capture.py`, `capture-dos.sh`, `capture-web.py` | The tools that make `media/` |
| `chapters/NN-id.md` | One chapter per stretch of the work, in reading order; reference pages at the end |
| `pages/front.md`, `pages/coming.json` | The front page's text, and the chapters not yet written |
| `roadmap.json` | The steps of Milestones 0 to 5 with state and evidence |
| `anatomy-dos.json` | The boxes, arrows, statuses and evidence of the FIZZ2.EXE anatomy |
| `not-verified.json` | The "Not verified" panel |
| `snapshots/gh.json`, `snapshots/measured.json` | `gh` data for offline builds, and numbers that need a build to measure, with the commands |
| `media/` | Captured screenshots and the DOSBox transcript, each recorded in `captures.json` |
| `VOICE.md`, `voice-patterns.json` | The style guide and the phrases the tests refuse |
| `templates/` | `site.css` and `site.js` |
| `tests/` | `unittest` tests and a fixture |

## How the data is gathered

- **History.** `git log --first-parent main` gives the merge commits. Fizz 2 lands a stack of pull requests with one merge commit, so a pull request counts as merged when `gh` reports it merged and its head is reachable from `main`. Its own commits run from the head of the pull request below it (the one whose branch is its base), or from the merge commit's first parent, to its own head; its size is `git diff --numstat` over that range. The merge commit `gh` names is compared with the one `git` finds, and a difference is a warning. Without `gh` data, a merge commit whose subject ends in `(#N)` stands for pull request N.
- **Counts.** Files, lines per package, `#[test]` declarations, the golden transcript, `M0_SCRIPT`, and the figures stated in `docs/PLATFORM.md`, `tools/check-dos.sh` and `.github/workflows/ci.yml`, all from the tree of `main`. A number that a document no longer yields is reported as missing, and `--dry-run` then exits with status 1.
- **Pull requests and CI.** `gh pr list --state all` and `gh run list --branch main`, named explicitly for `ScottMorris/lieutenant-fizz-2` (`FIZZ2_GH_REPO` overrides it). Every page footer says whether the data came from `gh` or the snapshot.
- **Measured numbers.** `snapshots/measured.json` holds the output of `tools/check-dos.sh`, `tools/build-dos.sh` sizes and `tools/check-parity.sh` on the tree of `main`, with the commit. Update it by hand when the build changes.
- **State checks.** A roadmap step marked done keeps that state only while its pull requests are merged with CI green on the commit that brought them into `main`; otherwise it drops to "in review" or "partly done". The build also refuses a measured transcript or parity hash that differs from the golden file and a measured BIOS tick count that disagrees with the timer arithmetic.

## Chapters and figures

A chapter is `chapters/NN-id.md`: a front matter (`id`, `n`, `title`, `date`, `summary`, `prs`, and `section: reference` with a `kicker` for reference pages), the narrative, a `figures` block and three shelves. Inline markup is `**bold**`, `*em*`, `` `code` ``, links with the schemes `chapter:ID#anchor`, `pr:N`, `commit:HASH` and `repo:PATH`, and placeholders such as `{{m.prs}}`, `{{pr7.merge}}`, `{{sc.pitDivisor}}` or `{{ms.biosTicks}}`, which come from the data and fail the build if unknown. Inline images are markdown images, a link with `!` in front and the alt text in the brackets. Directives are HTML comments on their own line: `<!-- explainer replay -->` (also `timer`), `<!-- figure NAME -->` (`layers`, `parity`, `pr-stack`, `anatomy-dos`, `input-path`, `frame-path`, `milestones`, `loc`, `tests`, `pr-sizes`, `ci`, `floppy`, `dos-transcript`, `shots FILE...`, and the tables `nexts`, `roadmap-table`, `script`, `measured`) and `<!-- facts KEY ... -->`.

The shelves are `### Catalogued`, `### Uncatalogued` and `### Wish list`, each followed by a list. A "From the workbench" sidebar is a block quote starting `> **From the workbench — Title.**` with a `> Source:` line.

Every drawn or captured figure is declared once in its chapter's `figures` block: `id`, `kind` (`screenshot`, `transcript`, `diagram`, `chart`, `explainer` or `history`), `track` (`web`, `dos`, `shared` or `design`), `title`, `alt`, `caption`, `at` (the directive) and `thumb` (`svg`, `svgs`, `steps`, `term` or `img:PATH`). The gallery and the front page's recent figures are built from those records. Diagrams use the 16-colour EGA palette of the README banner, and every SVG carries a `<title>` and `<desc>`.

To update the anatomy diagram, edit `anatomy-dos.json`: each box has a `status` (`verified`, `unverified`, `planned` or `external`), a `text` and `evidence` (`pr:N`, `commit:HASH` or `repo:PATH`, which must exist).

## The Manual page

`docs/MANUAL.md` is the player's manual, kept in step with the game under the "Game Manual" rule of `AGENTS.md`, and it is the only source of the page's text. `manual.py` reads it from the working tree, takes its first `# ` line as the page title and renders the rest through `mdlite.py` (headings, tables, lists, inline code, links and markdown images, written as a link with `!` in front; `{{` stays text). One line above it gives the source as data: the last commit that changed the file, or an uncommitted file in the working tree on the current commit. Relative links and image sources are resolved against `docs/`: images are copied to `manual/<path>` in the site with their alt text, other files are linked on GitHub, and a missing target, an image without alt text, or a link without text fails the build.

The page ends with "What is checked", built from `manual-evidence.json`: for each heading of the manual, the shelf (`catalogued`, `uncatalogued` or `none` for a section that describes no behaviour), the evidence with links into the chapters, what the section says that no check covers (`gaps`), and the state tag of the pull requests named. A heading the file does not map shows as uncatalogued, and a mapped heading the manual no longer has fails the build, so the map has to follow the manual's edits. Without `docs/MANUAL.md` the page and its navigation item are left out and the contents show a "coming" row.

## Captures

```bash
tools/check-dos.sh && tools/build-web.sh                     # build dist/dos and dist/web first
python3 tools/progress-report/capture.py                     # needs xvfb-run, xdotool, ImageMagick, Chromium; optipng if installed
```

`capture-dos.sh` runs `FIZZ2.EXE /REPLAY` and `/LIVE` through `tools/dosrun.sh` with `DOS_X11=1` under `xvfb-run`, types keys with DOSBox's AUTOTYPE, and grabs DOSBox's window with ImageMagick's `import`. `capture-web.py` serves `dist/web`, starts headless Chromium with a debugging port, sends real key events through the DevTools protocol and saves `Page.captureScreenshot`. `capture.py` reduces each grab to a 32-colour PNG in `media/` and writes `media/captures.json` with what each shows, its source, the command, the commit and the date. Keep `media/` under 1 MB.

## Checks

```bash
python3 -I tools/progress-report/tests/test_report.py      # the unit tests (tools/check.sh runs these)
python3 -I tools/progress-report/gather.py --dry-run       # the dry run (tools/check.sh runs this too)
python3 tools/progress-report/build.py --check             # a full build and site check
```

`--check` fails on unbalanced tags, duplicate ids, a page without exactly one `h1`, a broken internal link or anchor, a chapter the index does not link, an image without alt text, a published image no page shows, and a file in `media/` without a full record in `captures.json`. The tests cover the gatherer on a throwaway repository with a stacked merge and on a shallow clone, the roadmap's state checks, the figure registry, the drawings against the repository's files, the captures, and `VOICE.md`'s patterns over the sources and a fresh build.

## Publishing

The site is published by hand as a private multi-file Artifact titled "Fizz 2 Progress Report", at `https://claude.ai/artifact/1K3wm9Su2dgs8ssGpK9Bbk` (`ARTIFACT_URL` in `build.py`). The build writes `dist/progress-report/manifest.json` with the call:

1. Build: `python3 tools/progress-report/build.py --check`.
2. Read the live artifact once with the Artifact tool (`action: "read"`, `url`), so a republish is allowed.
3. Publish to the same `url` with `file_path` set to `dist/progress-report/_publish/index.html` (the front page as a fragment; the Artifact skeleton supplies the doctype, head and body), `root` set to `dist/progress-report`, and `files` set to the `publish.files` map of `manifest.json`. Pass the manifest's `description`, and omit `icon` and `title` on a republish.
4. Read a few published paths back with `action: "read"` and `path` to confirm they exist.

Use absolute paths in the call when the tool runs from another checkout. Every page other than the index is a full HTML document. Limits: 16 MB per file, 255 files and 64 MB per publish; the manifest lists the size and SHA-256 of every file.
