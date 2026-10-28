---
id: m0-core
n: 3
title: Milestone 0: rules, the HAL and the DOS runtime
date: 2026-10-27
summary: Input frames and throwaway rules, a Desk-shaped hardware abstraction layer, and a Rust DOS runtime whose 60 Hz timer still keeps the BIOS clock.
prs: [1, 2, 3]
---
Milestone 0 is the platform skeleton: the smallest program that reads input, steps rules at 60 Hz, draws a frame and hashes its state on every platform. [`docs/PLATFORM.md`](repo:docs/PLATFORM.md) is its reference. This chapter covers the three bottom pull requests of the stack; chapter 4 covers the presenters and chapter 5 the parity gate.

## Input frames and the hello-tick rules (#1)

`fizz-sim` gained the input types: `Buttons` (`UP=1`, `DOWN=2`, `LEFT=4`, `RIGHT=8`, `ACTION=16`), `InputFrame` with `held` and `pressed` sets, and `InputLatch`, which turns key transitions into one frame per tick. The rules act on `held | pressed`, so a tap shorter than a tick still counts. `fizz-hello` is the throwaway rule set: a marker with a position, a velocity, a heading and a dash count. The heading is `libm::atan2` of the held direction and a dash on the `ACTION` edge moves the marker along it, which puts trigonometry, and so the x87 precision, into the hash on purpose. The 600-tick `M0_SCRIPT` and its golden transcript `crates/fizz-hello/m0-replay.txt` arrived in the same pull request ({{pr1.commits}} commits, +{{pr1.added}} lines).

<!-- figure input-path -->

## The hardware abstraction layer (#2)

`fizz-hal` mirrors the shapes of Desk's layer without depending on it: `Clock` and an integer `Pacer` that turns clock readings into whole 60 Hz ticks with a catch-up cap; `KeyCode`, `KeyEvent`, `InputSource` and a `Set1Decoder` for scancodes; `Rgb6`, `ModeInfo` and `Display`; and `SpscRing`, the bounded queue between an interrupt handler and the game loop. The crate is `no_std` with no dependencies and denies `clippy::float_arithmetic`. `SpscRing` is its only `unsafe` code, and `clippy.toml` bans the compare-and-swap methods because DOSBox with `cputype = 486` rejects `CMPXCHG`. Desk's frame-buffer and memory-mapped region pair became `Display::present(pixels, pitch)`, so the HAL holds no pointer to video memory. `WebClock` and `DosClock` are the two consumers of `Clock` and `Pacer` that the second-consumer rule asks for.

## The DOS runtime (#3)

`dos/` is a separate Cargo workspace with its own target (`dos/dos-elf.json`), `-Zbuild-std` and `RUSTC_BOOTSTRAP=1`, so the host checks never need them. `fizz-dos-rt` ({{loc.fizz_dos_rt}} lines in `dos/rt/src/`) is the only DOS-aware code: start-up, DPMI calls, the heap, a shared interrupt registry on one locked stack, the timer, the keyboard, mode 13h through DPMI function 0300h, and the clean-up list that restores every vector on exit and on panic. Cargo links an ELF with `rust-lld`, and `tools/elf2go32.py` packs it with the DJGPP stub into a go32 executable that loads under CWSDPMI. No C compiler or C library is involved. Most of the runtime is mirrored from Desk's d0 spike and the proof of concept, and `docs/PLATFORM.md` lists the provenance file by file.

<!-- figure anatomy-dos -->

### The 60 Hz timer that keeps the BIOS clock

The game needs 60 interrupts a second and DOS needs its clock to keep running at 18.2 Hz. The runtime programs the PIT with divisor {{sc.pitDivisor}} and keeps a 16-bit accumulator: each interrupt adds the divisor, and when the sum passes 65,536 the handler wraps it and chains to the BIOS handler instead of sending the end of interrupt itself. Over time the BIOS handler runs once per 65,536 input clocks, as it would with its own divisor, and the divisor goes back to 0 (65,536) on exit.

<!-- explainer timer -->

Under CWSDPMI `hlt` is privileged, so the game loop paces itself with a volatile spin on the tick counter. The keyboard handler owns IRQ 1, decodes Set 1 scancodes into a local byte ring, and on exit restores the BIOS keyboard flag bits at `0x417` and clears `0x418`.

<!-- facts rtLines tests -->

<!-- figure tests -->

## The shelves

### Catalogued
- #1, #2 and #3 are merged into `main` with CI {{pr3.ci}} on the merge commit `{{pr3.merge}}` and every check green on each head ({{m.headCiGreen}}).
- `cargo test --workspace` runs the {{tests.fizz_sim}} `fizz-sim`, {{tests.fizz_hello}} `fizz-hello` and {{tests.fizz_hal}} `fizz-hal` test declarations in the `check` job.
- The timer keeps the BIOS clock in DOSBox: the Milestone 0 replay printed `BIOS TICKS {{m0ms.biosTicks}}` over 600 ticks, which is 600 × {{sc.pitDivisor}} ÷ 65,536 rounded down, and `tools/check-dos.sh` requires {{m0.biosRange}}.
- Every vector is restored in DOSBox: `VECTORS RESTORED yes` after `/REPLAY` and `/LIVE`, and `/PANIC` exits with status {{m0ms.panicExit}}.

### Uncatalogued
- The timer, the keyboard handler and the clean-up on a real 486 and a real 8042 keyboard controller.
- The DOS time of day after a long session: the arithmetic says it holds, and no run has compared the clock with a wall clock.
- `SpscRing` under a real interrupt load: it is tested on the host, and the DOS runtime uses its own byte ring for the keyboard.

### Wish list
- The real Desk crates in place of `fizz-hal` and `fizz-dos-rt` (`docs/ARCHITECTURE.md`, What is mirrored from where).
- Storage and audio traits, which join the HAL with their first consumers.

> **From the workbench — 18.2 Hz.** The PC's programmable interval timer runs from a 1,193,182 Hz clock, and the BIOS programs channel 0 with the largest divisor, 65,536, which gives about 18.2 interrupts a second. DOS's time of day counts those ticks at 0040:006C.
> Source: [Intel 8253, Wikipedia](https://en.wikipedia.org/wiki/Intel_8253)

```figures
id: input-path
kind: diagram
track: shared
title: The input path on DOS and Web
alt: Flow diagram: on DOS a key goes through the IRQ 1 handler, the byte ring, the Set 1 decoder and key_to_buttons; on Web a keydown goes through main.js and App.key; both reach InputLatch, InputFrame and step.
caption: From a key press to one InputFrame per tick, with the file behind each box.
at: figure input-path
thumb: svg

id: anatomy-dos
kind: diagram
track: dos
title: Anatomy of FIZZ2.EXE
alt: Layered box diagram of FIZZ2.EXE from the program through the rules, the presenter, the HAL adapters, the runtime files, start-up, packaging and the host, each box marked verified, planned or not ours.
caption: Every layer of the DOS program with its status and evidence.
at: figure anatomy-dos
thumb: svg

id: timer
kind: explainer
track: dos
title: The 60 Hz timer and the BIOS clock
alt: Step-through of ten timer interrupts: the accumulator grows by 19,886 each time and chains to the BIOS handler whenever it passes 65,536, ending with the 182 BIOS ticks of the replay.
caption: Ten interrupts of the accumulator, and the BIOS ticks of a 600-tick replay.
at: explainer timer
thumb: steps

id: tests-chart
kind: chart
track: shared
title: Test declarations per package
alt: Bar chart of the number of test declarations in each Rust package; the DOS workspace packages have none.
caption: Where the Rust tests are, by package.
at: figure tests
thumb: svg
```
