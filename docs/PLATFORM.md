# Platform skeleton

Milestone 0 built the platform skeleton: a fixed 60 Hz loop that runs on Web and DOS from the same Rust rules, moves a marker with the keys of either layout, and hashes its state so a script can prove that native x86_64, Web (WASM) and DOS (headless DOSBox) agree. There is no game yet. This document describes the layers as built, the decisions behind them, how to run each check and what has not been verified. [`ARCHITECTURE.md`](ARCHITECTURE.md) holds the layer plan and the parity policy this skeleton implements.

## Contents

- [Layers as built](#layers-as-built)
- [The hash transcript](#the-hash-transcript)
- [Numeric model decisions](#numeric-model-decisions)
- [The hardware abstraction layer](#the-hardware-abstraction-layer)
- [The DOS runtime](#the-dos-runtime)
- [The Web build](#the-web-build)
- [Running the checks](#running-the-checks)
- [Toolchain](#toolchain)
- [Size budget](#size-budget)
- [Provenance](#provenance)
- [Not verified](#not-verified)

## Layers as built

| Crate or directory | Layer | Role |
| --- | --- | --- |
| `crates/fizz-sim` | Rules | The 60 Hz `Tick`, the FNV-1a `StateHash`, and the input types `Buttons`, `InputFrame` and `InputLatch`. `no_std`, no `unsafe`, no platform types |
| `crates/fizz-hello` | Rules | Throwaway rules for the marker; deleted when real rules arrive. Holds the golden transcript `m0-replay.txt` |
| `crates/fizz-hal` | Platform | Traits a platform implements, with host-side test doubles behind the `mock` feature |
| `crates/fizz-view` | Presenter base | The placement rule both presenters share: a position in game units to a logical pixel, rounding down. Neither presenter depends on the other |
| `crates/fizz-present-dos` | Presenter | The 320x200 indexed frame, palette, 8x8 font, HUD and key mapping; no DOS calls |
| `crates/fizz-present-web` | Presenter | The `wasm-bindgen` `App` and `WebClock` |
| `dos/rt` | Platform | `fizz-dos-rt`, the self-contained DOS runtime and the only DOS-aware code |
| `dos/fizz2` | App | `FIZZ2.EXE`, with the HAL adapters `DosClock`, `DosInput` and `DosDisplay` in `src/hal.rs` |
| `dos/rtsmoke` | Test | `RTSMOKE.EXE`, which exercises the runtime without any rules |
| `web/` | App | `index.html`, `main.js` and `style.css`, the static page that drives the `App`, with relative URLs only |

`InputFrame` carries `held` and `pressed` button sets. `InputLatch` turns key transitions into frames, and the rules act on `held | pressed`, so a tap shorter than one tick still counts. `Buttons` are `UP=1`, `DOWN=2`, `LEFT=4`, `RIGHT=8`, `JUMP=16`, `POGO=32` and `FIRE=64`.

`fizz-hello` is a marker with a position, a velocity, a heading and a dash count. The heading comes from `atan2` of the held direction, and a dash on the `JUMP` edge moves the marker along it. The trigonometry is deliberate: it makes the x87 precision setting an observable, tested part of the hash.

The DOS workspace is separate from the root workspace. It uses a custom target (`dos/dos-elf.json`), `-Zbuild-std` and `RUSTC_BOOTSTRAP=1`, so host checks never need `rust-src` or the nightly-only flags.

## The hash transcript

A replay runs the built-in 600-tick script (`M0_SCRIPT`) and prints a transcript. Version 1 is plain text, one record per line:

```text
M0 REPLAY 1
TICK 0 HASH <hex16>
TICK 60 HASH <hex16>
...
FINAL TICK 600 HASH <hex16>
```

`TICK` lines appear at tick 0 and after every 60 ticks. The hash covers, in this order: the tick as a `u64`, then `x`, `y`, `vx`, `vy` and `heading` as the bits of an `f64`, then `dashes` as a `u32`. A test locks that order. The parity gate compares only the `TICK` and `FINAL` lines. Platforms may add lines with other prefixes, and DOS prints `FPU CW 027F`, `BIOS TICKS <n>` and `VECTORS RESTORED yes`.

The golden file is `crates/fizz-hello/m0-replay.txt`. All three platforms produce it, ending `FINAL TICK 600 HASH efc699079baa6a88`: native x86_64 (`cargo run -p fizz-hello --example replay`), Web (WASM through `wasm-bindgen`, run under Node and Bun by `tools/web-replay.mjs`) and DOS (`FIZZ2.EXE /REPLAY` in headless DOSBox with the x87 at 53 bits).

## Numeric model decisions

The model is Fizz 1's, described in [`ARCHITECTURE.md`](ARCHITECTURE.md). The skeleton settled these specifics:

- State is `f64` and the rules use basic IEEE operations. `fsin` and `fsqrt` are never used.
- `libm` is pinned at `=0.2.16` with `default-features = false`. On x86 targets without SSE, its `exp`, `exp2` and `exp10` always run x87 assembly, so the exp family is banned in rules crates.
- Velocities below 1/1024 snap to zero. Without it, a decaying velocity reaches subnormal values, which x87 at 53 bits, SSE2 and WebAssembly round differently.
- The x87 control word is `0x027F` (53-bit precision, all exceptions masked). The program sets it before the first tick. The runtime's `_start` only runs `FNINIT`, which leaves `0x037F` (64-bit precision), so a program that skips the step would hash differently.
- Interrupt handlers never touch the x87.

## The hardware abstraction layer

`fizz-hal` mirrors the shapes of Desk's hardware abstraction layer. It is written in this repository, not vendored and not a path dependency, so the real Desk crates can replace it later.

| Area | Contents |
| --- | --- |
| Time | `Ticks`, the `Clock` trait, and `Pacer`, an integer accumulator that turns a clock reading into whole 60 Hz ticks with a catch-up cap |
| Input | `KeyCode`, `KeyEvent`, the `InputSource` trait and `Set1Decoder` for Set 1 scancodes (extended arrows, WASD, Z, X, C, Space, Esc, and Ctrl and Alt, where the extended right-hand keys report the same codes as the left) |
| Video | `Rgb6`, `ModeInfo`, `HalError` and the `Display` trait |
| Hand-off | `SpscRing`, a bounded single-producer, single-consumer queue between an interrupt handler and the game loop |

The crate is `no_std` with no dependencies, denies `clippy::float_arithmetic`, and denies `unsafe_code` everywhere except `ring.rs`.

- **Deliberate deviation from Desk.** `Display::present(pixels, pitch)` takes the finished frame and replaces Desk's frame-buffer and `MmioRegion` pair. The HAL therefore contains no pointer to video memory and no `unsafe`; the DOS adapter owns that boundary.
- **`SpscRing`** is the only `unsafe` in the HAL. It is adapted from Desk's `ring.rs` and uses only atomic load, store and swap. DOSBox with `cputype = 486` rejects `CMPXCHG`, so `clippy.toml` bans the compare-and-swap family of methods.
- **Second consumer.** `WebClock` and the DOS `DosClock` both implement `Clock` and share `Pacer`, which is what proves the trait. `WebClock` rounds `now_ms` up so a 60 Hz display never loses a tick to truncation.

## The DOS runtime

`dos/rt` (`fizz-dos-rt`) is self-contained and does not depend on `fizz-hal`. The HAL adapters live in `dos/fizz2/src/hal.rs`. The runtime keeps a tiny local byte ring in `rt/src/ring.rs` for keyboard bytes, to avoid the dependency.

- **Build.** Cargo links an ELF with `rust-lld` and `dos/link.ld`. `tools/elf2go32.py` packs it, with the DJGPP stub, into a go32 executable that loads under CWSDPMI. No C compiler or C library is involved.
- **Timer.** The PIT is programmed to divisor 19886, which is 60.0011 Hz. A 16-bit accumulator adds 19886 per interrupt and, when it wraps past 65536, chains to the BIOS handler. The BIOS clock therefore keeps 18.2065 Hz and the DOS time of day stays correct. The divisor is restored to 0 on exit.
- **Interrupts.** The IRQ handlers share one locked interrupt stack, so handlers never nest. Each returns `Action::Eoi` or `Action::Chain`. Handlers do bounded work, allocate nothing and never use the x87.
- **No `hlt`.** The instruction is privileged under CWSDPMI. Pacing is a volatile spin on the tick counter.
- **Keyboard.** The runtime owns IRQ1 and decodes Set 1 scancodes. It restores the BIOS keyboard flag lock bits at `0x417` and clears `0x418` on exit.
- **Video.** Mode 13h is set through DPMI function 0300h, a real-mode `INT 10h`.
- **Clean-up.** Every vector is restored on exit and on panic through the clean-up list. A panic exits with code 3.
- **File names** are 8.3 and upper case.

`FIZZ2.EXE` takes one argument. `/REPLAY` runs the 600-tick script and prints the transcript and the platform lines. `/LIVE` is keyboard play, ends on Esc, and is the default; it also ends by itself after 30 seconds so an emulator run cannot hang. `/MODERN` selects the modern key layout and combines with `/LIVE` or stands alone for it. `/PANIC` hooks IRQ 0 and panics, to show that the clean-up list runs.

### Key layouts

The two layouts are exclusive sets, and Keen is the default on both platforms. Keen is Fizz 1's classic layout: the arrows move, Ctrl is `JUMP`, Alt is `POGO` and Space is `FIRE`. Modern is the arrows or `W` `A` `S` `D` for movement, `Z` for `JUMP`, `X` for `POGO` and `C` or Space for `FIRE`. Arrows work and Space fires in both; a key of the other layout does nothing.

On DOS, `fizz_present_dos::key_to_buttons(KeyCode, Layout) -> Option<Buttons>` is the one mapping, with `Layout::Keen` (the default) and `Layout::Modern`. `FIZZ2.EXE /MODERN` picks the second. The decoder reports Ctrl and Alt for both the left and right keys. A Pause sequence is swallowed whole, including its `1D`, so it never looks like a Ctrl press.

On the Web, `web/main.js` holds the same two tables keyed on `event.code` and feeds button bits to `App::key`. A button on the page switches layouts and the choice is kept in `localStorage` when it is available. The page calls `preventDefault` for every key the active layout uses, which also keeps Alt from focusing the menu bar. A tab still cannot block every browser shortcut (Ctrl+W closes it) or the window and workspace shortcuts of some desktops, so the Modern layout avoids Ctrl and Alt.

In fullscreen (the `F` key or the page's button), the page calls the Keyboard Lock API, `navigator.keyboard.lock`, with the keys of both layouts, and releases it when fullscreen ends. Chromium supports it; where the API is missing or refuses, the page plays on without it. Esc is left out of the lock so the browser's way out of fullscreen keeps working.

## The Web build

`fizz-present-web` exports a `wasm-bindgen` `App`: `key(bits, down)` records a button change, `frame(now_ms)` runs the whole ticks that have elapsed and returns their count, and `tick`, `hash_hex`, `marker_x`, `marker_y` and `dashes` read the state. `replay_transcript()` returns the same transcript the other platforms print. The page's script feeds `performance.now()` and key events in; rules never see the browser clock. Time beyond five ticks per displayed frame is dropped, so a hidden tab does not cause a burst on return.

`wasm-bindgen` is pinned at `=0.2.125` in `Cargo.toml`, and the command-line tool must match. Install it at user level:

```bash
cargo install wasm-bindgen-cli --version 0.2.125 --locked
```

`tools/build-web.sh` fails, naming both versions, if the installed tool differs from the version in `Cargo.lock`. It writes a static bundle to `dist/web` with relative URLs only, so a launcher (such as Fizz 1's) can bundle it under any path later.

## Running the checks

| Command | What it does | Needs |
| --- | --- | --- |
| `tools/check.sh` | Formatting, lints, tests, licence headers and the markdown style check | The pinned Rust toolchain |
| `cargo run -p fizz-hello --example replay` | Prints the native transcript | The pinned Rust toolchain |
| `tools/setup-dos.sh` | Links `vendor/` to the DOS toolchain when it is absent | `DOS_TOOLCHAIN`, or a `vendor/` already in place |
| `tools/build-dos.sh` | Builds `RTSMOKE.EXE`, `FIZZ2.EXE` and `CWSDPMI.EXE` into `dist/dos` | The DOS toolchain |
| `tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY` | Runs a DOS program in headless DOSBox and passes back its output and exit status | The DOS toolchain |
| `tools/check-dos.sh` | Builds, then checks the replay against the golden file, the platform lines, a keyboard smoke (`MOVED yes`) and the panic exit status of 3, and prints `SIZE` | The DOS toolchain |
| `tools/build-web.sh` | Builds `dist/web` | The `wasm32-unknown-unknown` target and the pinned CLI |
| `node tools/web-replay.mjs` | Runs the replay inside the built WASM module (Node and Bun both work) | `dist/web` |
| `tools/check-parity.sh` | Compares the native, Web (Node and Bun) and DOS transcripts and prints `PARITY ok <hash>` | Everything above |

CI has two jobs in `.github/workflows/ci.yml`. `check` runs `tools/check.sh` on `ubuntu-latest`. `parity` runs in the pinned `ci-dos` image and runs the DOS, Web and parity checks.

## Toolchain

The DOS toolchain is the shared `ci-dos` image, pinned by digest:

```text
ghcr.io/liminal-hq/ci-dos@sha256:93d129b5c910b2a2d5de38ae7235e9dc8b311d302c1c57992f060b036a74dc4e
```

It provides Rust 1.98.0 with the `wasm32` and `i586` targets, Bun 1.3.14, DJGPP, DOSBox Staging 0.83.0 and CWSDPMI under `/opt/dos-toolchain`. It has no Node and no `wasm-bindgen`, so the parity job installs the pinned CLI. Changes to tool versions belong in the image repository, followed by a new digest here.

For local runs, `vendor/{djgpp,dosbox,cwsdpmi}` holds the same layout. The directory is git-ignored and never committed. `tools/setup-dos.sh` links `vendor` to `$DOS_TOOLCHAIN` when `vendor/djgpp` is absent, and does nothing when it is present:

```bash
DOS_TOOLCHAIN=/path/to/toolchain tools/setup-dos.sh
```

Only the DJGPP stub and CWSDPMI are used at run time; they are data, not a compiler. Do not install system packages to make a build pass.

## Size budget

A 1.44 MB floppy holds 1,474,560 bytes raw, about 1,457,664 bytes of data area. Fizz 1's release scripts set the reserves:

| Disk | Room for the EXE and data | Reserve |
| --- | --- | --- |
| FreeDOS-bootable | About 1.17 MB | FreeDOS needs about 153 KB and 64 KB is kept free for saves |
| Plain | About 1.39 MB | 8 KB kept free |

The budget covers the executable and the Fizz Asset Packs (`.FAP`) together (see [Assets and packs](ARCHITECTURE.md#assets-and-packs)). `tools/check-dos.sh` prints `SIZE <bytes>` for `FIZZ2.EXE` against a provisional `BUDGET 1100000` and fails if it is over; it counts the executable alone until packs exist, and then counts both. The figure is a tracked metric, not a design limit. Fizz 2's own save and config reserve is still to be confirmed. The Milestone 0 `FIZZ2.EXE` is 32,768 bytes, unstripped.

For scale, Fizz 1 embeds about 436 KB of assets in an 862 KB stripped executable: `sprites.bin` 102,252 bytes, `sound.bin` 279,348 bytes and `titles.bin` 54,488 bytes. The Episode 2 sprite design canvas holds 211 sprites, about 146 KB at one byte per pixel and about 73 KB at four bits per pixel. That count includes variants of Ben and three enemy rosters, so the sprites that ship will be fewer. Music and the larger backdrops are likely to take more room than sprites do.

## Provenance

Files mirrored or adapted from other work name their source in a header comment. Everything is dual-licensed Apache-2.0 OR MIT.

| Origin | Files |
| --- | --- |
| Mirrored from Desk's d0 spike and the earlier proof of concept (tag `poc-1`) | `dos/dos-elf.json`, `dos/link.ld`, `dos/rustfmt.toml`, and `dos/rt/src/` `start.rs`, `dpmi.rs`, `dos.rs`, `heap.rs`, `env.rs`, `panic.rs`, `cleanup.rs`, `io.rs` |
| Adapted | `dos/.cargo/config.toml`; `dos/rt/src/` `lib.rs`, `fpu.rs` (keeps only the control word), `irq.rs`, `timer.rs` (60 Hz in place of 560 Hz) and `vga.rs`; `tools/elf2go32.py`, `tools/dosrun.sh`, `tools/build-dos.sh`, `tools/setup-dos.sh`; `crates/fizz-hal/src/ring.rs` from Desk's `ring.rs` |
| Derived from Fizz 1 `crates/sim/src/` at commit `4bce036`, transformed as in fizz-dos `m2/patches/sim.patch` | `crates/fizz-phys/src/` `tilemap.rs`, `body.rs`, `platform.rs` and `rng.rs`; `fmath.rs` from fizz-dos `m2/sim/src/fmath.rs` without `exp`. `tools/lf-sim-drift.sh` compares them with a Fizz 1 checkout |
| Written for this repository | `dos/rt/src/` `ring.rs`, `port.rs` and `kbd.rs`; `dos/rtsmoke`; the rest of `crates/fizz-hal`, which follows Desk's shapes for time, input and video without port traits |

## Not verified

- Real 486 hardware. Every DOS result comes from DOSBox.
- x87 cycle cost. DOSBox with `cycles=max` hides it.
- The live Web keyboard and canvas on a real browser session. The WASM module is replayed under Node and Bun, and headless Chromium has received key events for both layouts over the DevTools protocol, but a person has not played it.
- Browser and desktop shortcut clashes (Ctrl+W, Alt combinations, workspace shortcuts) and Keyboard Lock on real desktops. A headless browser cannot show either.
- Audio. Nothing in the skeleton makes sound.
- Bundling the Web build into a launcher.
- The GitHub-hosted runner for the `parity` job.
