---
id: findings
n:
kicker: Reference
title: Findings, decisions and risks
date: 2026-10-27
summary: What Milestones 0 and 1 established, the decisions they recorded, the questions still open and the risks to the plan.
prs: []
section: reference
---
This page collects the results of the chapters in one place, each with the chapter or document that holds its evidence.

## Findings

- **One `f64` rule set hashes identically on SSE2, WebAssembly and the x87.** It needs the x87 at 53 bits, no exp family, and snap-to-zero for tiny velocities ([chapter 5](chapter:parity)). The slice's rules, Fizz 1's physics, do the same for {{golden.finalTick}} ticks ([chapter 8](chapter:rules)).
- **Fizz 1's physics can be owned and proved.** `fizz-phys` is a `no_std` copy of Fizz 1's physics, and Ben, the chicken, the shots, the key and the door reproduce eighteen traces from Fizz 1's own simulation bit for bit ([chapter 8](chapter:rules)).
- **One drawing routine serves both screens.** `fizz_slice_view::draw_slice` draws the slice for the Web and DOS presenters, with frame digests pinned in its tests; the Web view follows the window's shape and DOS keeps 320 by 200 ([chapter 9](chapter:slice)).
- **Sound can follow the events.** The rules emit the cluck as an `EVENT` line that all three platforms print; each presenter plays it its own way, and the DOS paths cannot change the hash ([chapter 9](chapter:slice)).
- **A Rust DOS program needs no C.** `FIZZ2.EXE` is linked by `rust-lld` and packed by `tools/elf2go32.py` with the DJGPP stub, and loads under CWSDPMI ([chapter 3](chapter:m0-core)).
- **60 Hz and the DOS clock can coexist.** Divisor {{sc.pitDivisor}} with a chaining accumulator gave `BIOS TICKS {{m0ms.biosTicks}}` over Milestone 0's 600-tick replay, the count the arithmetic predicts ([chapter 3](chapter:m0-core#the-60-hz-timer-that-keeps-the-bios-clock)).
- **The HAL holds no video pointer.** `Display::present(pixels, pitch)` keeps all `unsafe` video access in the DOS adapter and the runtime ([chapter 3](chapter:m0-core)).
- **The DOS program is small.** {{sc.exeBytes}} bytes unstripped with the slice, {{m.budgetUsed}} of the provisional {{sc.budget}}-byte budget; {{m0ms.exeBytes}} bytes at Milestone 0 ([chapter 4](chapter:m0-presenters)).
- **The parity job runs on GitHub's runner.** `check` and `parity` passed on the heads of #7, #8 and #10 ([chapter 6](chapter:stack)).

## Decisions recorded

| Decision | Where | Why |
|---|---|---|
| Keep Fizz 1's `f64` numeric model | `docs/ARCHITECTURE.md`, Numeric model | Fizz 1's physics are continuous and tuned by feel; fixed point would need re-tuning |
| Control word `0x{{sc.controlWord}}` set by the program | `docs/PLATFORM.md`, Numeric model decisions | `FNINIT` leaves 64-bit precision |
| `libm` pinned at `={{sc.libm}}`, exp family banned in rules | `docs/PLATFORM.md` | x87 assembly on targets without SSE |
| Snap velocities below 2^-1000 to zero (1/1024 at Milestone 0) | `docs/PLATFORM.md` | Subnormals round differently across targets |
| Own a copy of Fizz 1's physics as `fizz-phys` | `docs/ARCHITECTURE.md`, Open questions | Fizz 1's `lf-sim` is `std`, and Fizz 2 will diverge from it |
| The door is a switched tile and a death respawns in place | `docs/PLATFORM.md`, Departures from Fizz 1 | The door stays open and no level reload is needed; traces mask one switch bit |
| `InputFrame` lives in `fizz-sim`, not `fizz-engine` | `docs/ARCHITECTURE.md`, Crate plan | Both presenters and the rules need it before any scene exists |
| `Display::present` replaces Desk's frame buffer pair | `docs/PLATFORM.md`, The hardware abstraction layer | No video pointer and no `unsafe` in the HAL |
| Compare-and-swap banned in `fizz-hal` | `crates/fizz-hal/clippy.toml` | DOSBox with `cputype = 486` rejects `CMPXCHG` |
| The `dos/` workspace is separate | `docs/PLATFORM.md`, Layers as built | Host checks never need `-Zbuild-std` |

## Open questions

- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later?
- Which language does the baker use? Rust is preferred, so it shares types.
- Do the level script and the studio need extensions for vertical mazes, gadgets and the underworld?
- What base art resolution does Episode 2 author for, given the DOS palette limits? The slice uses Fizz 1's sizes and the provisional Ben A is not shown yet.
- Is the `f64` model affordable on a 486? In DOSBox the slice's step costs a small part of the tick, and DOSBox does not model the x87's latency.
- How much level does the Web view show on a large screen by default?

These are the open questions of `docs/ARCHITECTURE.md`; the one about Fizz 1's physics is answered there and in chapter 8.

## Risks

- **x87 cost on a 486DX.** The rules are `f64`, and the 560 Hz audio interrupt adds work. DOSBox at `cycles=max` hides what they cost; until a cycle estimate exists, the 486 budget is unknown.
- **DOSBox as the only DOS.** Every DOS result comes from one emulator, one DPMI host and its built-in DOS.
- **The Web page played only by scripts.** Parity is proved on the module under Node and Bun, and the page's key handling has been driven by headless Chromium through the DevTools protocol; no person has played it in a browser.
- **Sound nobody has heard on DOS or on hardware.** The cluck and the eleven effects after it exist on Web and DOS and have been checked by what is scheduled or written; the owner has listened on the Web and not on real sound hardware.
- **Mirrored code.** `fizz-hal` and `fizz-dos-rt` mirror Desk; swapping in the real crates later may move behaviour the parity gate does not cover, such as timing and clean-up.
- **The skeleton's rules were throwaway.** Milestone 0's parity result covered `fizz-hello`, which is deleted; the slice's rules start over with Fizz 1's traces, which exercise one slice of Fizz 1's rules and leave out ladders, vines, wall kicks and moving platforms.

## The shelves

### Catalogued
- Each finding above links the chapter whose shelves give its check.

### Uncatalogued
- Every risk above: each names something nobody has measured.

### Wish list
- An answer to each open question, recorded in `docs/` with the decision.
