---
id: scaffold
n: 2
title: The house rules and the scaffold
date: 2026-10-27
summary: Seven commits straight onto main set the rules, the fizz-sim tick and hash, the checks, the README banner and CI, before the first pull request.
prs: []
---
The repository began on 27 October 2026 with seven commits on `main` and no pull request. Each one is on the first-parent line of `main`, so the table lists them as data.

| Commit | Time (Eastern) | What it added |
|---|---|---|
| [`7955155`](commit:7955155) | 03:00 | `Initial Commit`: `AGENTS.md`, `CLAUDE.md`, the two licences, `NOTICE`, `rust-toolchain.toml` |
| [`f2f6e66`](commit:f2f6e66) | 03:00 | `fizz-sim` with the 60 Hz `Tick` and the FNV-1a `StateHash`; `tools/check.sh`, `tools/check-docs.py`, `tools/check-headers.sh` |
| [`c5bb1d5`](commit:c5bb1d5) | 03:00 | `docs/ARCHITECTURE.md`, `docs/POC-1-FINDINGS.md`, `docs/SCENE-CONTRACT.md` |
| [`b229933`](commit:b229933) | 03:00 | The README and the animated EGA banner, `assets/hero.svg` |
| [`14680d1`](commit:14680d1) | 03:00 | `.github/workflows/ci.yml` with the `check` job, templates, release notes configuration, `CHANGELOG.md` |
| [`1e960a5`](commit:1e960a5) | 03:05 | README badges pointed at `ScottMorris/lieutenant-fizz-2` |
| [`3737b39`](commit:3737b39) | 03:09 | The README's conventions note restored |

## The rules

`AGENTS.md` is the authority for everything after it. The rules that shape the code:

- **The rules layer is pure.** `fizz-sim` and the game logic on it are `no_std` plus `alloc`, forbid `unsafe`, and have no clocks, randomness or platform types. A frame is a function of the steps that led to it.
- **Fizz 1's numeric model.** `f64` state under a determinism discipline: basic IEEE operations, no reordering, a software `libm` for transcendental functions, and the x87 at 53-bit precision on DOS before the first tick.
- **The state hash is the arbiter.** A change that makes the native, Web and DOS hashes disagree is a bug until proven otherwise.
- **Interrupt handlers do bounded work**, allocate nothing, hand work to the game loop through a bounded single-producer, single-consumer queue, and every vector is restored on exit and on panic.
- **No speculative scaffolding.** A crate, trait or driver arrives with the consumer and the test that need it, and a piece is not done until a second consumer exercises it.

And the rules that shape the prose and history: Canadian English, markdown with one paragraph per line, Conventional Commits, merge commits only, explicit staging, and an SPDX header on every source file.

## The checks

`tools/check.sh` is the gate before every pull request. It runs `cargo fmt --check`, `cargo clippy` with warnings as errors, `cargo test --workspace`, `cargo fmt --check` in the `dos/` workspace, the licence-header check and the markdown check. The `check` job in CI runs it on `ubuntu-latest` for every pull request and every push to `main`; the `parity` job joined it in Milestone 0 (chapter 4).

## The shelves

### Catalogued
- The seven scaffold commits are on the first-parent line of `main`, from `git log --first-parent`.
- `tools/check.sh` runs in the `check` job of `.github/workflows/ci.yml`, and CI concluded "success" on the pushes of `1e960a5` and `3737b39` (`gh run list --branch main`).
- `tools/check-headers.sh` and `python3 -I tools/check-docs.py` pass on the tree of `main`.

### Uncatalogued
- The CI result of the first five scaffold commits: they were pushed together, and `gh run list` shows runs only for `1e960a5` and `3737b39`.

### Wish list
- Release notes grouped by label, configured in `.github/release.yml` and unused until the first release.

> **From the workbench — Merge commits.** Git's `--first-parent` option follows only the first parent of each merge commit, which turns a history of merged branches into one line per merge. With merge commits only, that line holds one commit per landed branch and the branches keep their own commits.
> Source: [git-log documentation](https://git-scm.com/docs/git-log)
