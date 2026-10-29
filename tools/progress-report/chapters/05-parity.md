---
id: parity
n: 5
title: The parity gate
date: 2026-10-27
summary: The same final hash on native, Web and DOS, first for Milestone 0's 600-tick marker script and now for the slice's 1,268 ticks, the three numeric rules that make it possible, and the script and CI job that enforce it.
prs: [7]
---
The state hash is the arbiter between the platforms. After every tick the rules hash their state in a fixed order with FNV-1a, and a test locks the order. A replay prints transcript version 1: a header line, a `TICK` line at tick 0 and after every 60 ticks, a `FINAL` line and, since the cluck, `EVENT` lines (the cluck, the shot end and, since Milestone 4, the sound and caption events such as `fire`, `jump`, `stomp` and `exit`; adding them changed no `TICK` or `FINAL` line). Milestone 0 hashed a marker's tick, `x`, `y`, `vx`, `vy` and `heading` as the bits of an `f64` and its `dashes` as a `u32`; its replay of `M0_SCRIPT` printed `M0 REPLAY 1` and ended `FINAL TICK {{m0.finalTick}} HASH {{m0.final}}` on all three platforms. The slice replaced it, and the gate now compares the slice's golden file, `crates/fizz-ep2/slice-replay.txt`, which begins `SLICE REPLAY 1` and ends `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`; [chapter 8](chapter:rules) lists the fields it hashes.

<!-- figure parity -->

## Three rules behind one hash

Bit-identical `f64` results on SSE2, WebAssembly and the x87 rest on three decisions recorded in `docs/PLATFORM.md`:

- **The x87 at 53 bits.** The x87 computes in 80 bits unless told otherwise. `FIZZ2.EXE` sets the control word to `0x{{sc.controlWord}}`, 53-bit precision with every exception masked, before the first tick. The runtime's `_start` only runs `FNINIT`, which leaves `0x037F` (64-bit precision), so a program that skipped the step would hash differently. The DOS run prints `FPU CW {{sc.controlWord}}` after the replay to show the word held.
- **No exp family.** `libm` is pinned at `={{sc.libm}}` with default features off. On x86 targets without SSE its `exp`, `exp2` and `exp10` always run x87 assembly, so the rules crates may not use them. `atan2`, `sin` and `cos` run in software, and Milestone 0's replay, which used all three, agreed on every platform.
- **Snap to zero.** Milestone 0 turned velocities below 1/1024 into zero; the slice's threshold is 2^-1000 ([chapter 8](chapter:rules)). Without it a decaying velocity reaches subnormal values, which the x87 at 53 bits, SSE2 and WebAssembly round differently.

Interrupt handlers never touch the x87, and the HAL, the DOS presenter and the runtime deny float arithmetic, so nothing else can disturb the control word.

## The replay, tick by tick

Milestone 0's script ran for {{m0.finalTick}} ticks: idle, a run right, a diagonal, dashes while moving and while still, a long run left into the clamp, a run up, and an idle tail that lets the velocity decay to the snap.

<!-- explainer replay -->

<!-- figure script -->

## The script and the CI job (#7, #36)

At Milestone 0, `tools/check-parity.sh` ran the replay natively with `cargo run -p fizz-hello --example replay` (kept here as history: the crate is deleted), in the built WebAssembly module under Node or Bun with `tools/web-replay.mjs`, and in headless DOSBox with `FIZZ2.EXE /REPLAY`. It compared only the `TICK` and `FINAL` lines of each with the golden file (carriage returns removed), required the version line from native and Web, and printed `PARITY ok <hash>`. `fizz-hello` is deleted; the script now runs `fizz-ep2`'s replay example and compares the slice's golden file, `crates/fizz-ep2/slice-replay.txt`, with `EVENT` lines included. Platform lines such as `FPU`, `BIOS` and `VECTORS` use other prefixes and are checked by `tools/check-dos.sh`.

The `parity` job in `.github/workflows/ci.yml` runs in the shared `ci-dos` image pinned by digest (`{{sc.ciImage}}`), which brings Rust, the `wasm32` and `i586` targets, Bun, DJGPP, DOSBox Staging and CWSDPMI. It installs the pinned `wasm-bindgen` command-line tool (cached), links the toolchain, and runs `tools/check-dos.sh`, `tools/build-web.sh` and `tools/check-parity.sh`. This is the output of the same three scripts run on the tree of `main`:

<!-- figure measured -->

## The shelves

### Catalogued
- #7 is merged with CI {{pr7.ci}} on `{{pr7.merge}}`; the `check` and `parity` jobs both passed on its head on the GitHub-hosted runner, and again on #8 and #10.
- `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}` with native, Web and DOS compared, on the tree of `main` (`snapshots/measured.json`); the build refuses a measured hash that differs from the golden file.
- `FPU CW {{sc.controlWord}}` after the DOS replay, checked by `tools/check-dos.sh`.

### Uncatalogued
- The x87 path on a real 486DX FPU: DOSBox emulates the x87, and an emulator bug and a hardware difference would look the same.
- WebAssembly in a browser engine: the module was run under Node and Bun, and the one screenshot that carries a hash (chapter 9, tick 300) is from headless Chromium.

### Wish list
- Parity of whole scenes, not only the rules' state, once scene intent exists (Milestone 3).

> **From the workbench — Extended precision.** The 8087 and its successors keep intermediate results in an 80-bit format with a 64-bit significand. The precision-control field of the control word rounds results to 24 or 53 bits instead, which makes basic operations round like IEEE single or double on the x87.
> Source: [x87, Wikipedia](https://en.wikipedia.org/wiki/X87)

```figures
id: parity
kind: diagram
track: shared
title: One script, three platforms, one hash
alt: Diagram of the 1,268-tick slice script running natively, as WebAssembly under Node and Bun, and as FIZZ2.EXE in DOSBox, all three producing transcripts equal to the golden file that ends with hash 0edebb797e63edff.
caption: Native, Web and DOS replay the same slice script to the same final hash.
at: figure parity
thumb: svg

id: replay
kind: explainer
track: shared
title: Milestone 0's golden replay, line by line
alt: Step-through of Milestone 0's golden transcript: the version line, the eleven TICK lines with the scripted input of each second, the FINAL line and the three DOS platform lines.
caption: Each hashed tick of the Milestone 0 marker script, with the input that led to it.
at: explainer replay
thumb: steps
```
