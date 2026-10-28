---
id: prologue
n: 0
title: Prologue: one game, two presentations
date: 2026-10-27
summary: What Lieutenant Fizz 2 is for, why Web and DOS are built together, and what exists today: a first slice that plays on both, built on Fizz 1's rules.
prs: []
---
Lieutenant Fizz 2 is the start of Episode 2 of the Fizz games and the simulation that Episode 3 is meant to reuse. Ben "Lieutenant Fizz" Blaze is back on Earth, the Zargs are raiding its chocolate, and the episode is planned around an overworld, an inner world and an underworld, vertical mazes of keycards and doors, and gadgets beyond the pogo stick. None of that exists yet; what plays today is one small slice (below). The story lives in Fizz 1's `docs/SERIES.md`, and the plan in [`docs/ARCHITECTURE.md`](repo:docs/ARCHITECTURE.md).

The repository's rule for every milestone is that it ships on both presentations. The Web build leads on feel and fidelity: real lighting, a zoom-out level map, an Undertone score. The DOS build leads on budgets: a 486DX with VGA and AdLib is the floor, DOS may cut content, and it never limits the design. Both are driven by one deterministic Rust simulation, `fizz-sim`, stepped at a fixed 60 Hz, and the state hash it produces after each tick is the arbiter between them.

<!-- figure layers -->

What exists on `main` at `{{head}}` is the first slice: one 32 by 48 tile level with Ben, a chicken, a red key, a red door and an exit, played on the Web in a window that sets its own width and on DOS in 320 by 200. Ben's rules are Fizz 1's, proved against Fizz 1's own traces ([chapter 8](chapter:rules)); the chicken clucks, with a caption on both screens and a sound on both, and `FIZZ2.EXE` also has the machine probe, the start-up check and a help screen ([chapter 9](chapter:slice)). The three builds print the same transcript, ending `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`. Milestone 0, the skeleton of a marker, a clock and a hash that came before it, is chapters 3 to 6. The rest of this archive is how the slice came to be, how it is checked, and what comes next.

<!-- facts prs commitsAll files crates rustLines tests -->

Fizz 1 (`liminal-hq/lieutenant-fizz`) stays as it is: a working multiplatform prototype and the reference for behaviour, physics and levels. Desk (`liminal-hq/desk`) is the Rust-on-DOS foundation; its hardware abstraction layer and runtime ideas are mirrored in Fizz 2 for now, and the real Desk crates are meant to replace them later.

## The shelves

### Catalogued
- The parity rule and the layers are written down in `docs/ARCHITECTURE.md` and `AGENTS.md` on `main`, and `python3 -I tools/check-docs.py` passes on them in CI.
- The slice's transcript is the same on native, Web and DOS: `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}` (`snapshots/measured.json`; chapters 5 and 8). Milestone 0's marker transcript ended `{{m0.final}}` on the same three platforms and is kept in `snapshots/m0.json`.

### Uncatalogued
- The game beyond the slice: there is one level, placeholder art for the chicken, one sound effect and one placeholder song on the Web, and no story, menu or save.
- Anything on a real 486, a real display or a real browser window (the Not verified list on the front page).

### Wish list
- Episode 2's worlds, gadgets and enemies (`docs/POC-1-FINDINGS.md`, Episode 2 content ideas).
- Tauri builds for Linux, Windows, macOS and Android around the Web presenter.

> **From the workbench — The floor.** Intel's 486DX, introduced in 1989, was the first x86 processor with the floating-point unit on the same chip; the cheaper 486SX that followed had it disabled. That is why Fizz 2 names the DX as its DOS floor and leaves the SX out of scope.
> Source: [i486, Wikipedia](https://en.wikipedia.org/wiki/I486)

```figures
id: layers
kind: diagram
track: shared
title: The layers, as built
alt: Layer diagram with Web on the left and DOS on the right: the rules crates, the shared slice drawing, both presenters, the Web page, fizz-hal, fizz-machine, fizz-sfx and the DOS runtime are built; fizz-engine and fizz-bake are planned.
caption: Content, intent, baking, presenters and platform, each box marked built or planned (as of Milestone 1).
at: figure layers
thumb: svg
```
