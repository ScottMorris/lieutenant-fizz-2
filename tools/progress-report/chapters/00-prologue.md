---
id: prologue
n: 0
title: Prologue: one game, two presentations
date: 2026-10-27
summary: What Lieutenant Fizz 2 is for, why Web and DOS are built together, and what exists today: a marker, a clock and a hash.
prs: []
---
Lieutenant Fizz 2 is the start of Episode 2 of the Fizz games and the simulation that Episode 3 is meant to reuse. Ben "Lieutenant Fizz" Blaze is back on Earth, the Zargs are raiding its chocolate, and the episode is planned around an overworld, an inner world and an underworld, vertical mazes of keycards and doors, and gadgets beyond the pogo stick. None of that exists yet. The story lives in Fizz 1's `docs/SERIES.md`, and the plan in [`docs/ARCHITECTURE.md`](repo:docs/ARCHITECTURE.md).

The repository's rule for every milestone is that it ships on both presentations. The Web build leads on feel and fidelity: real lighting, a zoom-out level map, an Undertone score. The DOS build leads on budgets: a 486DX with VGA and AdLib is the floor, DOS may cut content, and it never limits the design. Both are driven by one deterministic Rust simulation, `fizz-sim`, stepped at a fixed 60 Hz, and the state hash it produces after each tick is the arbiter between them.

<!-- figure layers -->

What exists on `main` at `{{head}}` is the Milestone 0 skeleton: a yellow marker that moves with the arrow keys or WASD, dashes with Space, and hashes its state every tick, built natively, as WebAssembly and as a DOS executable. The three builds print the same transcript, ending `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`. The rest of this archive is how that came to be, how it is checked, and what comes next.

<!-- facts prs commitsAll files crates rustLines tests -->

Fizz 1 (`liminal-hq/lieutenant-fizz`) stays as it is: a working multiplatform prototype and the reference for behaviour, physics and levels. Desk (`liminal-hq/desk`) is the Rust-on-DOS foundation; its hardware abstraction layer and runtime ideas are mirrored in Fizz 2 for now, and the real Desk crates are meant to replace them later.

## The shelves

### Catalogued
- The parity rule and the layers are written down in `docs/ARCHITECTURE.md` and `AGENTS.md` on `main`, and `python3 -I tools/check-docs.py` passes on them in CI.
- The skeleton's transcript is the same on native, Web and DOS: `tools/check-parity.sh` printed `PARITY ok {{golden.final}}` (chapter 4).

### Uncatalogued
- The game itself: no level, sprite, enemy, sound or save exists.
- Anything on a real 486, a real display or a real browser window (the Not verified list on the front page).

### Wish list
- Episode 2's worlds, gadgets and enemies (`docs/POC-1-FINDINGS.md`, Episode 2 content ideas).
- Tauri builds for Linux, Windows, macOS and Android around the Web presenter.

> **From the stacks — The floor.** Intel's 486DX, introduced in 1989, was the first x86 processor with the floating-point unit on the same chip; the cheaper 486SX that followed had it disabled. That is why Fizz 2 names the DX as its DOS floor and leaves the SX out of scope.
> Source: [i486, Wikipedia](https://en.wikipedia.org/wiki/I486)

```figures
id: layers
kind: diagram
track: shared
title: The layers, as built
alt: Layer diagram with Web on the left and DOS on the right: the rules crates, both presenters, the Web page, fizz-hal and the DOS runtime are built; fizz-engine, fizz-bake and the Episode 2 rules are planned.
caption: Content, intent, baking, presenters and platform, each box marked built or planned.
at: figure layers
thumb: svg
```
