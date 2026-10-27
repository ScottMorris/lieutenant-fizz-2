# CLAUDE.md

## Project

Lieutenant Fizz 2 is the start of Episode 2 (and the simulation Episode 3 reuses), with Web and DOS developed in parallel. `fizz-sim` is the deterministic rules foundation: `no_std`, no `unsafe`, no platform types. Presenters, a cooker and a DOS platform layer arrive with their first consumers.

`AGENTS.md` is authoritative. Its essential rules are Canadian English everywhere, unwrapped markdown, author-facing prose, Conventional Commits but no prefixes in PR titles, merge commits only, no push without instruction, explicit staging, and copyright/SPDX headers on source files.

The first commit is titled `Initial Commit`; later commits use Conventional Commits. Do not install system packages to make a build pass. `tools/check.sh` is the pre-PR gate.

## Where things come from

- Fizz 1 (`~/source/liminal-hq/lieutenant-fizz`, docs in `docs/GAME_DESIGN.md`, `STORY.md`, `SERIES.md`) is the behaviour reference. Its DOS port lives in `~/source/experiments/fizz-dos`.
- Desk (`~/source/liminal-hq/desk`) is the Rust-on-DOS reference. Mirror its ideas here for now and swap in the real crates later.
- `docs/POC-1-FINDINGS.md` records what the earlier proof of concept (`~/source/experiments/fizz-2-dos-poc1`, tag `poc-1`) settled and what it did not.
- Fizz Level Studio (`~/source/experiments/fizz-level-studio`) is the level and overworld authoring tool and its script language.

## Layout

- `crates/fizz-sim`: the deterministic rules foundation (fixed tick, state hash)
- `docs/`: architecture, the proof-of-concept findings, the scene contract sketch
- `assets/`: the animated README hero, hand-authored in the 16-colour EGA palette
- `tools/check.sh`: the pre-PR gate

## Commands

```bash
tools/check.sh
```

DOS support is not scaffolded before its first real consumer. When it arrives, mirror Desk's hardware abstraction layer, retain CWSDPMI as the host, pin the shared `ci-dos` image by digest, and add DOSBox transcript and state-hash checks. Do not add C or DJGPP glue by default.
