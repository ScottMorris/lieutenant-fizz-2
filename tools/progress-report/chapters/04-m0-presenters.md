---
id: m0-presenters
n: 4
title: Milestone 0: two presenters and FIZZ2.EXE
date: 2026-10-27
summary: A mode 13h presenter in integers, the FIZZ2.EXE hello-tick program checked in DOSBox, a wasm-bindgen Web presenter, and the platform reference with its size budget.
prs: [4, 5, 6, 8]
---
The rules of chapter 3 draw through two presenters. `fizz-present-dos` turns the world into a 320 by 200 indexed frame with no DOS calls; `FIZZ2.EXE` puts it on the screen. `fizz-present-web` exports the same rules to JavaScript, and a static page draws them on a canvas. Both presenters read the same `World`; neither feeds anything back into the rules.

<!-- figure frame-path -->

## The mode 13h presenter (#4)

`fizz-present-dos` holds the frame (64,000 bytes, one palette index per pixel), the 16-colour EGA palette as `Rgb6`, an 8x8 font, the HUD and `key_to_buttons`, which maps arrows, WASD and Space to buttons. It works in integers only: the crate denies `clippy::float_arithmetic`, and the marker's `f64` position is cast once in `render`. Golden checksums of rendered frames and of the HUD text are host tests, so a change to a pixel fails `cargo test`.

## FIZZ2.EXE (#5)

`FIZZ2.EXE` takes one argument. `/REPLAY` runs the 600-tick script in real time and prints the transcript and three platform lines; `/LIVE` is keyboard play that ends on Esc or after 30 seconds; `/PANIC` hooks IRQ 0 and panics, to show the clean-up list runs. The program sets the x87 control word to `0x{{sc.controlWord}}` before the first tick. `tools/check-dos.sh` builds it, compares the replay with the golden file, checks the platform lines, types keys with DOSBox's AUTOTYPE until the program prints `MOVED yes`, expects exit status 3 from `/PANIC`, and fails if the executable is over budget.

These are DOSBox Staging's window during a `/REPLAY` run and a `/LIVE` run, grabbed from a virtual X server by `tools/progress-report/capture.py`:

<!-- figure shots dos-replay-1.png dos-replay-2.png dos-replay-3.png dos-live.png -->

<!-- figure dos-transcript -->

## The Web presenter (#6)

`fizz-present-web` exports a `wasm-bindgen` `App`: `key(bits, down)` records a button change, `frame(now_ms)` runs the whole ticks that have elapsed, and getters read the tick, the hash, the marker and the dash count. `replay_transcript()` returns the same transcript as the other platforms. The page feeds `performance.now()` and key events in; the rules never see the browser clock, and time beyond five ticks per displayed frame is dropped. `wasm-bindgen` is pinned at `={{sc.wasmBindgen}}`, and `tools/build-web.sh` fails, naming both versions, when the installed command-line tool differs. The bundle in `dist/web` uses relative URLs only.

These are headless Chromium screenshots of the page, with real key events sent through the DevTools protocol by `tools/progress-report/capture-web.py`:

<!-- figure shots web-start.png web-moved.png web-dash.png -->

## The platform reference and the size budget (#8)

`docs/PLATFORM.md` records the layers as built, the transcript format, the numeric decisions, the HAL, the runtime, the Web build, every check and the toolchain. Its size budget starts from a 1.44 MB floppy: {{sc.floppyRaw}} bytes raw and about {{sc.floppyData}} bytes of data area. A FreeDOS-bootable disk keeps about {{sc.freedosKB}} KB for FreeDOS and {{sc.saveKB}} KB for saves; a plain disk keeps {{sc.plainKB}} KB. `tools/check-dos.sh` prints `SIZE` and fails above the provisional `BUDGET {{sc.budget}}`. The Milestone 0 `FIZZ2.EXE` is {{sc.exeBytes}} bytes unstripped, {{m.budgetUsed}} of the budget.

<!-- figure floppy -->

<!-- figure loc -->

## The shelves

### Catalogued
- #4, #5, #6 and #8 are merged into `main` with CI {{pr8.ci}} on `{{pr8.merge}}`.
- `FIZZ2.EXE` runs in headless DOSBox Staging: `tools/check-dos.sh` printed `DOS CHECK ok` on the tree of `main`, with `MOVED yes`, `PANIC exit {{ms.panicExit}}` and `SIZE {{ms.exeBytes}}` (`snapshots/measured.json`).
- The DOS screenshots are DOSBox's own window, grabbed by `capture-dos.sh` under Xvfb; the transcript shown with them is that run's output and matches the golden file.
- The Web page loads the module and responds to key events in headless Chromium: the HUD's tick and hash change, and the dash count goes to 1 after Space (`media/captures.json`).

### Uncatalogued
- The mode 13h picture on a real VGA card and monitor: the grabs are DOSBox's scaled window.
- The Web page with a real keyboard, in Firefox or Safari, on a phone, or at a refresh rate other than headless Chromium's.
- The DOS build at a 486's speed: DOSBox ran at `cycles=max`.

### Wish list
- A FreeDOS-bootable floppy image built in CI, with its free space as a tracked number.
- A launcher that bundles `dist/web` under its own path.

> **From the workbench — DPMI.** The DOS Protected Mode Interface, published in 1990, lets a DOS program run in 32-bit protected mode and still call DOS and the BIOS through the host. CWSDPMI is a free DPMI host written for DJGPP, and `FIZZ2.EXE` loads under it with DJGPP's stub and no DJGPP library.
> Source: [DOS Protected Mode Interface, Wikipedia](https://en.wikipedia.org/wiki/DOS_Protected_Mode_Interface)

```figures
id: frame-path
kind: diagram
track: shared
title: The frame path on DOS and Web
alt: Flow diagram: on DOS the World goes through render, the 320 by 200 Frame with the EGA palette, present, DosDisplay and vga.rs; on Web it goes through the App getters and draw in main.js to the canvas.
caption: From the World to the screen, with the file behind each box.
at: figure frame-path
thumb: svg

id: dos-shots
kind: screenshot
track: dos
title: FIZZ2.EXE in DOSBox
alt: DOSBox screenshots of FIZZ2.EXE: a blue mode 13h screen with a white HUD showing the tick, the state hash and the dash count, and a yellow marker at different places.
caption: Three moments of the replay and one of live play, grabbed from DOSBox's window.
at: figure shots dos-replay-1.png dos-replay-2.png dos-replay-3.png dos-live.png
thumb: img:media/dos-live.png

id: dos-transcript
kind: transcript
track: dos
title: FIZZ2.EXE /REPLAY output
alt: Terminal output of FIZZ2.EXE /REPLAY: the version line, eleven TICK lines, the FINAL line and the FPU, BIOS and VECTORS lines.
caption: The transcript of the grabbed DOSBox run, line for line.
at: figure dos-transcript
thumb: term

id: web-shots
kind: screenshot
track: web
title: The Web page in headless Chromium
alt: Screenshots of the Web page: an EGA blue field with a light-grey border, a yellow marker and a HUD with the tick, the hash and the dash count below it, before and after key presses.
caption: The page at start, after Right and Down, and after a dash.
at: figure shots web-start.png web-moved.png web-dash.png
thumb: img:media/web-moved.png

id: floppy
kind: chart
track: dos
title: The floppy budget
alt: Stacked bar of a 1.44 MB floppy's data area: FreeDOS, the save reserve, CWSDPMI and FIZZ2.EXE take a small part, most is free, and a dashed line marks the 1.1 MB budget.
caption: Where FIZZ2.EXE sits on a FreeDOS-bootable floppy.
at: figure floppy
thumb: svg

id: loc
kind: chart
track: shared
title: Lines of source per package
alt: Bar chart of lines of source in each Rust package, the Web page, the tools and the documents; the DOS runtime is the largest package.
caption: Where the code is, by package.
at: figure loc
thumb: svg
```
