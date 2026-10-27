---
id: findings
n:
kicker: Reference
title: Findings, decisions and risks
date: 2026-10-27
summary: What Milestone 0 established, the decisions it recorded, the questions still open and the risks to the plan.
prs: []
section: reference
---
This page collects the results of the chapters in one place, each with the chapter or document that holds its evidence.

## Findings

- **One `f64` rule set hashes identically on SSE2, WebAssembly and the x87.** It needs the x87 at 53 bits, no exp family, and snap-to-zero for tiny velocities ([chapter 5](chapter:parity)).
- **A Rust DOS program needs no C.** `FIZZ2.EXE` is linked by `rust-lld` and packed by `tools/elf2go32.py` with the DJGPP stub, and loads under CWSDPMI ([chapter 3](chapter:m0-core)).
- **60 Hz and the DOS clock can coexist.** Divisor {{sc.pitDivisor}} with a chaining accumulator gave `BIOS TICKS {{ms.biosTicks}}` over a 600-tick replay, the count the arithmetic predicts ([chapter 3](chapter:m0-core#the-60-hz-timer-that-keeps-the-bios-clock)).
- **The HAL holds no video pointer.** `Display::present(pixels, pitch)` keeps all `unsafe` video access in the DOS adapter and the runtime ([chapter 3](chapter:m0-core)).
- **The DOS program is small.** {{sc.exeBytes}} bytes unstripped, {{m.budgetUsed}} of the provisional {{sc.budget}}-byte budget ([chapter 4](chapter:m0-presenters)).
- **The parity job runs on GitHub's runner.** `check` and `parity` passed on the heads of #7, #8 and #10 ([chapter 6](chapter:stack)).

## Decisions recorded

| Decision | Where | Why |
|---|---|---|
| Keep Fizz 1's `f64` numeric model | `docs/ARCHITECTURE.md`, Numeric model | Fizz 1's physics are continuous and tuned by feel; fixed point would need re-tuning |
| Control word `0x{{sc.controlWord}}` set by the program | `docs/PLATFORM.md`, Numeric model decisions | `FNINIT` leaves 64-bit precision |
| `libm` pinned at `={{sc.libm}}`, exp family banned in rules | `docs/PLATFORM.md` | x87 assembly on targets without SSE |
| Snap velocities below 1/1024 to zero | `docs/PLATFORM.md` | Subnormals round differently across targets |
| `InputFrame` lives in `fizz-sim`, not `fizz-engine` | `docs/ARCHITECTURE.md`, Crate plan | Both presenters and the rules need it before any scene exists |
| `Display::present` replaces Desk's frame buffer pair | `docs/PLATFORM.md`, The hardware abstraction layer | No video pointer and no `unsafe` in the HAL |
| Compare-and-swap banned in `fizz-hal` | `crates/fizz-hal/clippy.toml` | DOSBox with `cputype = 486` rejects `CMPXCHG` |
| The `dos/` workspace is separate | `docs/PLATFORM.md`, Layers as built | Host checks never need `-Zbuild-std` |

## Open questions

- Is Fizz 1's pogo physics reused directly, or ported with the same constants and proved by golden traces?
- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later?
- Which language does the baker use? Rust is preferred, so it shares types.
- Do the level script and the studio need extensions for vertical mazes, gadgets and the underworld?
- What base art resolution does Episode 2 author for, given the DOS palette limits?

These are the open questions of `docs/ARCHITECTURE.md`, as written there.

## Risks

- **x87 cost on a 486DX.** The rules are `f64` and use `atan2`, `sin` and `cos` in software. DOSBox at `cycles=max` hides what they cost; until a cycle estimate exists, the 486 budget is unknown.
- **DOSBox as the only DOS.** Every DOS result comes from one emulator, one DPMI host and its built-in DOS.
- **The Web page without a browser test.** Parity is proved on the module under Node and Bun; the page's key handling has been driven only by the headless captures of chapter 4.
- **Mirrored code.** `fizz-hal` and `fizz-dos-rt` mirror Desk; swapping in the real crates later may move behaviour the parity gate does not cover, such as timing and clean-up.
- **The skeleton's rules are throwaway.** The parity result covered `fizz-hello`, which is deleted; the real rules start over with their own golden traces.

## The shelves

### Catalogued
- Each finding above links the chapter whose shelves give its check.

### Uncatalogued
- Every risk above: each names something nobody has measured.

### Wish list
- An answer to each open question, recorded in `docs/` with the decision.
