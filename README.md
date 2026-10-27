# Lieutenant Fizz 2

<p align="center">
  <img src="assets/hero.svg" alt="Lieutenant Fizz 2 — Episode 2 in 16-colour EGA: a lighthouse sweeps a dithered beam over the sea while a magenta Zarg saucer lifts cocoa beans from a chocolate factory and Ben bounces on his pogo stick, with Web and DOS badges" width="100%">
</p>

<p align="center">
  <a href="https://github.com/ScottMorris/lieutenant-fizz-2/actions/workflows/ci.yml"><img src="https://github.com/ScottMorris/lieutenant-fizz-2/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <img src="https://img.shields.io/badge/status-skeleton-f97316" alt="Status: platform skeleton">
  <img src="https://img.shields.io/badge/platforms-Web%20%C2%B7%20DOS-1f6feb" alt="Platforms: Web and DOS">
  <img src="https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-3fb950" alt="Licence: Apache-2.0 OR MIT">
</p>

*Lieutenant Fizz 2* is the start of **Episode 2** of the Keen-style platformer series, and the simulation that Episode 3 will reuse. It is one game with two first-class presentations, **Web** and **DOS**, built in parallel from a deterministic Rust core. Fizz 1 stays where it is as the original and the reference; Fizz 2 learns from it.

> **Status:** platform skeleton. A marker moves with the arrow keys or WASD on Web and on DOS, and native, Web (WASM) and DOS (headless DOSBox) builds produce the same state-hash transcript, which a parity check compares. There is nothing to play beyond that marker. [`docs/PLATFORM.md`](docs/PLATFORM.md) describes the skeleton and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) has the plan.

## The story so far

Ben "Lieutenant Fizz" Blaze followed his cousin Billy to Planet Zargoth, fought through the Crater Fields, the Crystal Caves and Mildred's Citadel, and freed Billy, who left on a lead of his own. Now Ben is on his own. The Zargs are raiding Earth's chocolate, Mildred is back with a bigger plan to drain Earth's cocoa and fill Zargoth's rivers for good, and Billy is only a voice on the comms.

Episode 2 takes Ben through an overworld, an inner world and an underworld, with levels that climb and descend through mazes of keycards and doors, new gadgets beyond the pogo stick, and enemies in the style of the planet, such as a chicken that has been Zargified. One level takes place at a lighthouse. The full story and series plan live in the [main game's documents](https://github.com/liminal-hq/lieutenant-fizz/blob/main/docs/SERIES.md).

## What it is for

- **One game, two presentations.** The Web build is the rich one: real lighting, a zoom-out level map, an Undertone score. The DOS build is the bonus: a 486DX is the floor, and it may cut content but never limits the design.
- **A simulation both can trust.** Fixed 60 Hz steps, the Fizz 1 numeric model, and a per-tick state hash that native, Web and DOS builds must agree on.
- **Intent flows up, lowering flows down.** The game describes what is in a scene and what the music means. Each presentation projects that down to what its hardware can do, and offline tools do the lossy conversion.
- **Reusable beyond one episode.** The rules foundation is built so Episode 3 can reuse it.

## Platforms

| Platform | Role | Status |
| --- | --- | --- |
| Web (and Tauri on Linux, Windows, macOS and Android) | Rich presentation, authoring and playtest | Skeleton (hello tick) |
| DOS (486DX, VGA, AdLib) | Bonus presentation on a Rust-on-DOS runtime | Skeleton (hello tick) |

## Repository layout

| Path | What it holds |
| --- | --- |
| `crates/fizz-sim` | The deterministic rules foundation: the 60 Hz tick, the state hash and the input types |
| `crates/fizz-hello` | Throwaway Milestone 0 rules (a moving marker) and the golden replay transcript |
| `crates/fizz-hal` | Hardware abstraction traits mirrored from Desk: clock, input, display and the interrupt ring |
| `crates/fizz-present-dos` | The DOS frame, palette, font and key mapping, with no DOS calls |
| `crates/fizz-present-web` | The Web presenter, a `wasm-bindgen` app |
| `dos/` | The separate DOS workspace: the runtime, `RTSMOKE.EXE` and `FIZZ2.EXE` |
| `web/` | The static page for the Web presenter |
| `docs/ARCHITECTURE.md` | Layers, crate plan, parity policy, and what is mirrored from where |
| `docs/PLATFORM.md` | The platform skeleton: transcript format, DOS runtime, Web build, checks, toolchain and provenance |
| `docs/POC-1-FINDINGS.md` | What the earlier proof of concept settled and what it did not |
| `docs/SCENE-CONTRACT.md` | A sketch of the scene boundary between rules and presentations |
| `tools/` | The check, build, DOS and parity scripts |
| `assets/` | The README banner |

## Getting started

```bash
tools/check.sh
```

This runs formatting, lints, tests, the licence-header and the markdown style checks. The toolchain is pinned in `rust-toolchain.toml`. It needs nothing beyond that toolchain.

The Web and DOS checks need more:

```bash
cargo install wasm-bindgen-cli --version 0.2.125 --locked   # once, for tools/build-web.sh
DOS_TOOLCHAIN=/path/to/toolchain tools/setup-dos.sh         # once, links vendor/
tools/check-dos.sh                                          # builds FIZZ2.EXE and checks it in headless DOSBox
tools/build-web.sh                                          # builds dist/web
tools/check-parity.sh                                       # prints PARITY ok <hash> when native, Web and DOS agree
```

`DOS_TOOLCHAIN` is a directory holding `djgpp/`, `dosbox/` and `cwsdpmi/`, such as `/opt/dos-toolchain` in the pinned `ci-dos` image that CI uses. To try the Web page, serve `dist/web` with any static file server. [`docs/PLATFORM.md`](docs/PLATFORM.md) lists every command and what was not verified.

## Standing on other projects

Fizz 2 draws on several Liminal HQ projects and names each one where it uses it.

- [Lieutenant Fizz](https://github.com/liminal-hq/lieutenant-fizz) is the behaviour, story and level reference. Its `f64`, fixed-step simulation is the numeric model Fizz 2 follows.
- [Desk](https://github.com/liminal-hq/desk) is the Rust-on-DOS foundation. Its hardware abstraction ideas are mirrored here for now and swapped for the real crates later.
- [Undertone](https://github.com/liminal-hq/undertone) is the audio library for the Web score.
- Fizz Level Studio, an experiment, is the level and overworld authoring tool, with its own script language.
- [Waypoint](https://github.com/liminal-hq/waypoint) and [Spindle](https://github.com/liminal-hq/spindle) set the repository conventions: the README shape, CI, release notes and labels.

## Conventions

Lieutenant Fizz 2 follows the [Liminal HQ](https://liminalhq.ca) conventions and copyright, and shares Fizz 1's palette, sense of humour and appetite for chocolate.

## Licence

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the [MIT licence](LICENSE-MIT), at your option.
