# CLAUDE.md

## Project

Lieutenant Fizz 2 is the start of Episode 2 (and the simulation Episode 3 reuses), with Web and DOS developed in parallel. `fizz-sim` is the deterministic rules foundation: `no_std`, no `unsafe`, no platform types. The Milestone 0 platform skeleton is built: a moving marker hashed identically on native, Web (WASM) and DOS (headless DOSBox). A baker and the real rules and scene arrive with their first consumers.

`AGENTS.md` is authoritative. Its essential rules are Canadian English everywhere, unwrapped markdown, author-facing prose, Conventional Commits but no prefixes in PR titles, merge commits only, no push without instruction, explicit staging, and copyright/SPDX headers on source files.

The first commit is titled `Initial Commit`; later commits use Conventional Commits. Do not install system packages to make a build pass. `tools/check.sh` is the pre-PR gate.

## Where things come from

- Fizz 1 (`~/source/liminal-hq/lieutenant-fizz`, docs in `docs/GAME_DESIGN.md`, `STORY.md`, `SERIES.md`) is the behaviour reference. Its DOS port lives in `~/source/experiments/fizz-dos`.
- Desk (`~/source/liminal-hq/desk`) is the Rust-on-DOS reference. Mirror its ideas here for now and swap in the real crates later.
- `docs/POC-1-FINDINGS.md` records what the earlier proof of concept (`~/source/experiments/fizz-2-dos-poc1`, tag `poc-1`) settled and what it did not.
- Fizz Level Studio (`~/source/experiments/fizz-level-studio`) is the level and overworld authoring tool and its script language.

## Layout

- `crates/fizz-sim`: the deterministic rules foundation (fixed tick, state hash, input types)
- `crates/fizz-hello`: throwaway Milestone 0 rules and the golden transcript `m0-replay.txt`; deleted when real rules arrive
- `crates/fizz-hal`: Desk-shaped hardware abstraction traits (clock, input, display, interrupt ring)
- `crates/fizz-present-dos`: the DOS frame, palette, font and key mapping
- `crates/fizz-present-web`: the `wasm-bindgen` Web presenter
- `dos/`: its own workspace (custom target, `-Zbuild-std`): `rt` (the runtime), `rtsmoke`, `fizz2` (FIZZ2.EXE)
- `web/`: the static page for the Web presenter
- `docs/`: architecture, the platform skeleton reference, the proof-of-concept findings, the scene contract sketch
- `assets/`: the animated README hero, hand-authored in the 16-colour EGA palette
- `tools/`: `check.sh` (the pre-PR gate), the DOS and Web build scripts, `check-dos.sh` and `check-parity.sh`

## Commands

```bash
tools/check.sh                                   # host gate, run before every PR
cargo run -p fizz-hello --example replay         # native transcript
tools/setup-dos.sh                               # links vendor/ from $DOS_TOOLCHAIN when absent
tools/check-dos.sh                               # build FIZZ2.EXE and check it in headless DOSBox
tools/build-web.sh                               # build dist/web (needs the pinned wasm-bindgen CLI)
tools/check-parity.sh                            # native, Web and DOS transcripts must match: PARITY ok <hash>
```

The DOS toolchain is the shared `ci-dos` image pinned by digest; locally `vendor/` is a git-ignored copy of its toolchain layout. Rules crates keep the numeric model in `docs/PLATFORM.md`: `f64`, basic IEEE operations, no `exp` family, the x87 control word set by the program. `fizz-hal`, `fizz-present-dos` and `fizz-dos-rt` deny float arithmetic. Document every new build or emulator check in `docs/PLATFORM.md`. Do not add C or DJGPP glue by default.
