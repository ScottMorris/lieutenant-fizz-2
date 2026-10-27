# Platform skeleton

Milestone 0 built the platform skeleton: a fixed 60 Hz loop that runs on Web and DOS from the same Rust rules and hashes the state so a script can prove that native x86_64, Web (WASM) and DOS (headless DOSBox) agree. Milestone 1 replaced the skeleton's throwaway marker rules with the first playable slice, and the slice is now the product the gates compare. Milestone 0's marker crate, `fizz-hello`, is deleted; its numbers are kept below as history. This document describes the layers as built, the decisions behind them, how to run each check and what has not been verified. [`ARCHITECTURE.md`](ARCHITECTURE.md) holds the layer plan and the parity policy this skeleton implements.

## Contents

- [Layers as built](#layers-as-built)
- [The hash transcript](#the-hash-transcript)
- [Numeric model decisions](#numeric-model-decisions)
- [Ben against Fizz 1](#ben-against-fizz-1)
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
| `crates/fizz-phys` | Rules | Fizz 1's tile map, body, moving platform and rng code as an owned `no_std` copy |
| `crates/fizz-ep2` | Rules | The slice's rules (Ben, the chicken, the shots, the red key and door, the level), the hash, the scripted replay and its golden transcript `slice-replay.txt` |
| `crates/fizz-hal` | Platform | Traits a platform implements, with host-side test doubles behind the `mock` feature |
| `crates/fizz-sfx` | Platform data | Generated OPL2 and PC speaker data for the cluck, and the `OplPlayer` (plain OPL2, or OPL3-panned on reserved channel 14) and `SpeakerPlayer` steppers the DOS audio service drives; see [`SFX.md`](SFX.md) |
| `crates/fizz-machine` | Platform | The pure model of the DOS start-up check: probe data, sound settings and resolver, the OPL3 write router and the check screen's cells. See [`DOS-CHECK.md`](DOS-CHECK.md) |
| `crates/fizz-view` | Presenter base | The placement rule both presenters share: a position in game units to a logical pixel, rounding down. Neither presenter depends on the other |
| `crates/fizz-slice-view` | Presenter base | `draw_slice`, the one routine that draws the Episode 2 slice into a palette-indexed buffer; both presenters call it |
| `crates/fizz-present-dos` | Presenter | The 320x200 indexed frame, palette, 8x8 font, HUD (the slice's ammo, key, deaths and tick strip in `render_slice`) and key mapping; no DOS calls |
| `crates/fizz-present-web` | Presenter | The `wasm-bindgen` `App` (the slice, drawn with `fizz_slice_view::draw_slice`) and `WebClock` |
| `dos/rt` | Platform | `fizz-dos-rt`, the self-contained DOS runtime and the only DOS-aware code |
| `dos/fizz2` | App | `FIZZ2.EXE`, with the HAL adapters `DosClock`, `DosInput` and `DosDisplay` in `src/hal.rs` |
| `dos/rtsmoke` | Test | `RTSMOKE.EXE`, which exercises the runtime without any rules |
| `web/` | App | `index.html`, `main.js` and `style.css`, the static page that drives the `App`, with relative URLs only |

`InputFrame` carries `held` and `pressed` button sets. `InputLatch` turns key transitions into frames, and the rules act on `held | pressed`, so a tap shorter than one tick still counts. `Buttons` are `UP=1`, `DOWN=2`, `LEFT=4`, `RIGHT=8`, `JUMP=16`, `POGO=32` and `FIRE=64`.

The rules are `fizz-ep2`, built on `fizz-phys`. Milestone 0's `fizz-hello` was a marker with a position, a velocity, a heading from `atan2` of the held direction, and a dash count; the trigonometry was deliberate, to make the x87 precision setting an observable part of the hash. It ran a 600-tick script (`M0_SCRIPT`) whose transcript, header `M0 REPLAY 1`, ended `FINAL TICK 600 HASH efc699079baa6a88` on native, Web and DOS. Nothing compares that hash any more.

The DOS workspace is separate from the root workspace. It uses a custom target (`dos/dos-elf.json`), `-Zbuild-std` and `RUSTC_BOOTSTRAP=1`, so host checks never need `rust-src` or the nightly-only flags.

## The hash transcript

A replay runs the slice's built-in 1,268-tick script (`SLICE_SCRIPT`) from the start of the level and prints a transcript. Version 1 is plain text, one record per line:

```text
SLICE REPLAY 1
TICK 0 HASH <hex16>
TICK 60 HASH <hex16>
...
FINAL TICK 1268 HASH <hex16>
```

`TICK` lines appear at tick 0 and after every 60 ticks. The parity gate compares the lines that start with `TICK `, `FINAL ` or `EVENT ` (an `EVENT <tick> <name> ...` line records a semantic event the rules emitted; the transcript has none yet, and a golden without them is compared by the same rule), and requires the golden's first line, the header, from every platform. Platforms may add lines with other prefixes, and DOS prints `EXIT yes`, `FPU CW 027F`, `BIOS TICKS <n>` and `VECTORS RESTORED yes`.

The golden file is `crates/fizz-ep2/slice-replay.txt`. All three platforms produce it: native x86_64 (`cargo run -p fizz-ep2 --example replay`, which calls `fizz_ep2::replay`), Web (WASM through `wasm-bindgen`, run under Node or Bun by `tools/web-replay.mjs`) and DOS (`FIZZ2.EXE /REPLAY` in headless DOSBox with the x87 at 53 bits, collected in an 8,192-byte buffer). `tools/check-parity.sh` compares native, Web and DOS with it and prints `PARITY ok <hash>` with the hash read from the golden's `FINAL` line; `tools/check-dos.sh` makes the same comparison for DOS alone, and the host tests of `fizz-present-web` compare the Web transcript with it.

### History: the Milestone 0 transcript

Milestone 0's replay ran 600 ticks of `fizz-hello` with the header `M0 REPLAY 1` and hashed the tick, then `x`, `y`, `vx`, `vy` and `heading` as `f64` bits, then `dashes` as a `u32`. All three platforms ended `FINAL TICK 600 HASH efc699079baa6a88`, and its `BIOS TICKS` bound was 180 to 184. The crate, its golden `m0-replay.txt` and that gate were deleted when the slice became the gated product; the progress archive keeps the transcript in `tools/progress-report/snapshots/m0.json`.

### The slice transcript

The slice replay (`fizz_ep2::replay`, golden `crates/fizz-ep2/slice-replay.txt`) has the header `SLICE REPLAY 1`: a `TICK` line at tick 0 and after every 60 ticks, then `FINAL TICK 1268 HASH <hex16>`, with `EVENT <tick> cluck <variant>` lines between them (see [Events and clucks](#events-and-clucks)). It takes an `InputFrame` stream through `fizz_ep2::step` and `fizz_ep2::hash`. Native prints it with `cargo run -p fizz-ep2 --example replay`, DOS runs it as `FIZZ2.EXE /REPLAY`, and the Web runs it through `tools/web-replay.mjs`; `tools/check-parity.sh` and `tools/check-dos.sh` compare each with the golden file.

The slice hash covers, in this order, with floating-point fields as `f64` bits and counts as `u32`:

1. the tick (`u64`);
2. Ben: `x`, `y`, `vx`, `vy`, `face`, `squash`, `dead`, `inv`, `shoot_t`, then a flags byte (bit 0 pogo, bit 1 cut, bit 2 on the ground, bit 3 bumped);
3. the `WorldFlags` word (bit 0 red key carried, bit 1 red door open, bit 2 exit reached), `ammo` and `deaths`;
4. `map.switches`, which holds the red door (a closed door is a set bit on channel 1);
5. the chicken count, then per chicken `x`, `y`, `vx`, `vy`, `dir`, `stun`, a flags byte (bit 0 on the ground, bit 1 touching Ben, bit 2 dead), the cluck timer (`u32`) and the cluck stream's draw count (`u32`);
6. the shot count, then per shot `x`, `y`, `vx`, `vy` and `life`.

The camera and the event list are not hashed. Tests in `crates/fizz-ep2/src/world.rs` lock the order three ways: an independent byte-by-byte FNV-1a over a hand-built sequence, a pinned digest of a world with every field set, and a check that changing any one field changes the digest.

## Numeric model decisions

The model is Fizz 1's, described in [`ARCHITECTURE.md`](ARCHITECTURE.md). The skeleton settled these specifics:

- State is `f64` and the rules use basic IEEE operations. `fsin` and `fsqrt` are never used.
- `libm` is pinned at `=0.2.16` with `default-features = false`. On x86 targets without SSE, its `exp`, `exp2` and `exp10` always run x87 assembly, so the exp family is banned in rules crates.
- In Milestone 0's rules, velocities below 1/1024 snapped to zero. Without it, a decaying velocity reaches subnormal values, which x87 at 53 bits, SSE2 and WebAssembly round differently.
- The x87 control word is `0x027F` (53-bit precision, all exceptions masked). The program sets it before the first tick. The runtime's `_start` only runs `FNINIT`, which leaves `0x037F` (64-bit precision), so a program that skips the step would hash differently.
- Interrupt handlers never touch the x87.

## Ben against Fizz 1

`fizz-ep2` holds Ben's movement, ported line for line from Fizz 1's `tick_ben_move` (with the alive and dead branches of `tick_ben`, `tick_head_bump` and `fire`) on top of `fizz-phys`; the rest of Fizz 1's `World::step` for the slice lives in `crates/fizz-ep2/src/world.rs`. It is proved against golden traces that Fizz 1's own simulation wrote, committed in `crates/fizz-ep2/tests/fizz1/`: `crates/fizz-ep2/tests/fizz1_traces.rs` parses each shared `.scn` scenario, runs it and compares every tick with the `.trace` as `f64` bit patterns, with no tolerance. The traces are regenerated with `tools/fizz1-trace`, which needs Fizz 1 at its pinned commit; the test needs only the committed files.

Proven bit for bit, all sixteen scenarios:

| Scenario | What it proves |
| --- | --- |
| `fall`, `run`, `jump_full`, `jump_cut` | gravity and terminal speed, walking and friction, the jump and its cut |
| `pogo_idle`, `pogo_held`, `pogo_ceiling` | the pogo and its held high bounce, the squash (slightly negative, never clamped), the taller head box |
| `oneway`, `wall_step`, `slope` | one-way platforms, the wall stop, the 0.55 step-up, slopes |
| `gloop_patrol` | the chicken's patrol, turning at a wall and at a ledge |
| `stomp`, `stomp_pogo` | landing on the chicken: 3 s stun, bounce 12 (20 with the pogo), repeated bounces while it stays stunned |
| `fizz_stun` | a shot stuns the chicken for 6 s, and a second shot passes through the stunned one; ammo and the shot's flight |
| `key_door` | picking up the red key and opening the red door by walking into it |
| `route` | a run through a small maze that ends on spikes: one-way ledges, jumps and the spike death |

The harness runs every scenario through the whole `World` and compares every field a trace carries, Ben's and the first Gloop's. One bit is masked in the `switches` field: Fizz 1 rewrites the red door's tile when it opens and never touches its switch word, while Fizz 2 keeps the door as a switched tile on channel 1 (a closed door is a set bit), so the door's bit is left out of that comparison. The facts the traces do not show (the door staying shut without the key, what a death keeps and drops) are covered by unit tests in `crates/fizz-ep2/src/replay.rs`.

The slice level is `crates/fizz-ep2/src/level.rs`: 32 by 48 tiles, written as text and baked at compile time, with the tile alphabet documented in the module. The replay proves it can be finished with the real physics: the script reaches the exit in 1,268 ticks with the door open and the key spent, after one death on the spikes carrying the key (the key drops and lies where it began; the world respawns Ben, resets the chicken and the ammo). It shoots the chicken in the first life and stomps it in the second.

### Events and clucks

`World::events()` returns the `Event`s the last `World::step` emitted, in the order they happened; the next step clears them, and every one happened on `World::tick`. The only event so far is `Event::Cluck { variant, chicken }` with `variant` below `CLUCK_VARIANTS` (4) and `chicken` the chicken's index in `World::chickens`. The rules never touch audio: a presenter plays the events. A presenter that runs several steps between frames reads the list after each step.

Each chicken has a cluck timer and its own stream of random numbers: Fizz 1's `fizz_phys::rng::hashf` over the world tick, the chicken's index and the number of draws the chicken has made, with a fixed salt. It is a pure function of integers, so there is no platform random source; the only floating-point work is `hashf`'s own division by 2^32, a multiply by a small integer and a truncation, all exact or correctly rounded in IEEE 754 and so the same on native, Web and DOS. The chicken's walk draws nothing, which is why Fizz 1's traces, which compare only Fizz 1's fields, stay bit for bit identical. The first draw at spawn (or respawn) sets the timer, uniform over 90 to 360 ticks (1.5 to 6 s). The timer counts down one per tick while the chicken is alive and not stunned; at zero the next draw is the variant and the one after is the next timer, and the world emits `Cluck`. Stunned chickens do not cluck and their timer holds still while they are stunned, so a stun delays the next cluck by its length and the chicken never clucks while stunned or the instant it wakes; a dead chicken (which Fizz 1 never produces) does not cluck either. The timer and the draw count are in the hash, so a platform whose stream drifts shows up in the parity gate.

The transcript prints `EVENT <tick> cluck <variant> <chicken>` for each cluck, in the order of `World::events()`, before the `TICK` or `FINAL` line of the same tick: the line reads as what happened during that tick, and the hash after it as the state the tick left. The header stays `SLICE REPLAY 1` because no existing line changed shape. `fizz_ep2::replay::{write_start, write_step, write_final}` write the pieces, so the DOS build, which paces the script itself, prints the same bytes `replay` does. The slice replay has three clucks, at ticks 197 (variant 2), 853 (variant 1) and 1094 (variant 0), and ends `FINAL TICK 1268 HASH 2145503c0d3b625f`. The transcript is 798 bytes against the DOS buffer's 8,192. `tools/check-dos.sh` and `tools/check-web-slice.sh` compare `TICK`, `FINAL` and `EVENT` lines with the golden file, so the number, tick, variant and chicken of every cluck match on native, Web and DOS.

Each chicken also carries two pieces of presentation state, `cluck_age` and `cluck_variant`. `World::step` adds one to every chicken's `cluck_age` (saturating) at the start of its turn and, when the chicken clucks, sets `cluck_age` to 0 and `cluck_variant` to the cluck's variant; `cluck_age` is `CLUCK_NEVER` (`u32::MAX`) until the first cluck, and a respawn resets both. They are not part of the state hash and no rule reads them: they only tell a presenter how far into a cluck a chicken is, so it can draw an open beak and a floating caption from the world and the tick alone. The hashed cluck timer and draw count are what set them, so any divergence shows in the hash first. `World::chicken_views()` is the read-only accessor for presenters: each chicken's position, facing, `cluck_age` and `cluck_variant`. The Web presenter's `take_cluck` queue still returns the variant only; the chicken index is available from `World::events()` for a presenter that needs it, such as one that pans the sound.

Departures from Fizz 1:

- Any velocity with a magnitude below 2^-1000 snaps to zero at the end of Ben's move. Fizz 1 lets an idle pogo's horizontal drag decay for ever, which reaches subnormal values where x87 at 53 bits and SSE2 differ. No golden trace changes with the snap, and a unit test runs an idle pogo for 30,000 ticks.
- The red door is a switched tile on channel 1 (solid while the bit is set) rather than a solid tile that Fizz 1 rewrites to empty space. Opening follows Fizz 1's rule exactly: with the key, touching the door's tile in front of Ben at body height opens it and spends the key. The open door is `WorldFlags::DOOR_RED_OPEN`.
- A death respawns Ben in place after Fizz 1's 1.4 s instead of reloading the level, so the open door survives it. The key does not: it is dropped and lies where it began unless the door is already open. The chickens, the shots and the ammo (5) start over, and `deaths` counts the respawns.
- The Gloop's "stray" reset (sent home after three seconds more than four tiles off its floor) is not ported; the slice's chicken cannot leave its floor. It would need a hashed timer.
- The chicken's cluck timer and stream are new in Fizz 2; Fizz 1's Gloop is silent. They are described under Events and clucks above.
- Mantle is not ported, so nothing starts one automatically. The trace tool refuses scenarios that would trigger it, and the port documents the omission where Fizz 1 calls `tick_mantle`. Ladders, vines, wall kicks, lifts and conveyors under Ben are also not ported.

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
- **Video.** Mode 13h is set through DPMI function 0300h, a real-mode `INT 10h`. In `/LIVE`, `FIZZ2.EXE` calls `Display::wait_vretrace` once per drawn frame, just before `present`, so the 64,000-byte copy begins at the start of the vertical retrace and the picture does not tear (the method Fizz 1's C port uses, `vga_vsync`). The wait polls bit 3 of port 3DAh, first until any retrace in progress ends and then until one begins, each phase bounded at 1,000,000 reads, so a machine or emulator whose status port never toggles costs a frame's time and not a hang. It only delays the copy: the simulation is still paced by the timer through `Pacer`, and the retrace adds no ticks. `/SLICE` and `/REPLAY` do not wait. Tearing cannot be seen headlessly, so the effect is unverified on real hardware.
- **Clean-up.** Every vector is restored on exit and on panic through the clean-up list. A panic exits with code 3.
- **File names** are 8.3 and upper case.

`FIZZ2.EXE` takes one mode argument. `/LIVE` plays the slice from the keyboard, ends on Esc, and is the default; it also ends by itself after 30 seconds so an emulator run cannot hang, and prints `LIVE TICKS`, `MOVED` (Ben is off his spawn) and `VECTORS RESTORED`. `/REPLAY` plays the slice script (1,268 ticks, about 21 seconds), drawing it as it goes, and prints the `SLICE REPLAY 1` transcript, `EXIT yes` when Ben reached the exit, `FPU CW 027F`, `BIOS TICKS <n>` (385 for the script) and `VECTORS RESTORED yes`; it is the DOS leg of the parity gate. The transcript is collected in an 8,192-byte buffer and printed only after the screen is back in text mode. `/MODERN` selects the modern key layout and combines with `/LIVE` or stands alone for it. `/NOCAPTIONS` turns off the in-world cluck captions and `/CAPTIONS` turns them on again (the default); the last of the two on the command line wins, and either combines with any mode. They are switches because the settings package has not landed: a field in the settings file replaces them then. The Web page has a Captions button and the V key (neither layout uses V), on by default and remembered in `localStorage` under `fizz2-captions`. Captions gate only the text; the chicken's beak moves regardless. `fizz_slice_view::draw_slice_with(.., &DrawOptions { captions })` is the one place the choice is applied, and `draw_slice` means captions on, so the parity gate's hashes (which never see a picture) are unaffected. `/PANIC` hooks IRQ 0 and panics, to show that the clean-up list runs.

### Key layouts

The two layouts are exclusive sets, and Keen is the default on both platforms. Keen is Fizz 1's classic layout: the arrows move, Ctrl is `JUMP`, Alt is `POGO` and Space is `FIRE`. Modern is the arrows or `W` `A` `S` `D` for movement, `Z` for `JUMP`, `X` for `POGO` and `C` or Space for `FIRE`. Arrows work and Space fires in both; a key of the other layout does nothing.

On DOS, `fizz_present_dos::key_to_buttons(KeyCode, Layout) -> Option<Buttons>` is the one mapping, with `Layout::Keen` (the default) and `Layout::Modern`. `FIZZ2.EXE /MODERN` picks the second. The decoder reports Ctrl and Alt for both the left and right keys. A Pause sequence is swallowed whole, including its `1D`, so it never looks like a Ctrl press.

On the Web, `web/main.js` holds the same two tables keyed on `event.code` and feeds button bits to `App::key`. A button on the page switches layouts and the choice is kept in `localStorage` when it is available. The page calls `preventDefault` for every key the active layout uses, which also keeps Alt from focusing the menu bar. A tab still cannot block every browser shortcut (Ctrl+W closes it) or the window and workspace shortcuts of some desktops, so the Modern layout avoids Ctrl and Alt.

In fullscreen (the `F` key or the page's button), the page calls the Keyboard Lock API, `navigator.keyboard.lock`, with the keys of both layouts, and releases it when fullscreen ends. Chromium supports it; where the API is missing or refuses, the page plays on without it. Esc is left out of the lock so the browser's way out of fullscreen keeps working.

## The Web build

`fizz-present-web` exports one `wasm-bindgen` app, `App`, over the slice's world: it holds `fizz_ep2::level::world()`. `key(bits, down)` records a button change, `frame(now_ms)` runs the whole ticks that have elapsed and returns their count, and `reset()` starts the level again without touching the pacer. `render()` draws the world into a 320 by 200 palette-indexed buffer with `fizz_slice_view::draw_slice`, the routine the DOS presenter also calls, and expands it to RGBA with the sixteen EGA colours; `rgba_ptr()`, `rgba_len()`, `width()` and `height()` let the page view that buffer in the module's memory (`new Uint8ClampedArray(memory.buffer, ptr, len)`, made again every frame because the memory can move) and `putImageData` it, so JavaScript draws nothing itself. `tick`, `hash_hex`, `ammo`, `has_key`, `deaths`, `door_open` and `exit_reached` feed the page's text HUD. `slice_replay_transcript()` returns the transcript `fizz_ep2::replay` writes, which the parity gate runs under Node or Bun. The page's script feeds `performance.now()` and key events in; rules never see the browser clock. Time beyond five ticks per displayed frame is dropped, so a hidden tab does not cause a burst on return. The first tick of each `frame` call takes the key latch's edges and later ticks in the same call see only the held buttons, so a press and release between two frames still reaches the rules once.

The page scales the 320 by 200 canvas with `image-rendering: pixelated` to the width the window allows, which is not a whole-number scale; whole-pixel locking and the larger viewport are Milestone 3. The HUD is page text under the canvas.

`wasm-bindgen` is pinned at `=0.2.125` in `Cargo.toml`, and the command-line tool must match. Install it at user level:

```bash
cargo install wasm-bindgen-cli --version 0.2.125 --locked
```

`App` also queues the chicken's cluck for the page. `frame` reads `World::events()` after each step it runs (the rules clear the list on the next step, and a frame can run up to five), and `take_cluck()` hands out the oldest waiting variant, or -1 when none is left; the page calls it until it returns -1 after each frame. The queue holds 32 and drops the rest, `reset()` empties it, and nothing in it reaches the rules, the hash or the transcript.

The page's script is bundled with Bun. `package.json` pins `@liminal-hq/undertone` at exactly 0.2.0, the version Fizz 1 resolves from the public npm registry, and `bun.lock` is committed. `tools/build-web.sh` runs `bun install --frozen-lockfile`, which is the build's only use of the network and only on a cold cache, and then `bun build web/main.js`, which writes one minified `dist/web/main.js` holding `web/sfx.ts`, `assets/sfx/cluck.ts` and Undertone. The wasm bindings stay a relative external import, so the bundle works from any base path and runs with no network. Bun is therefore a build requirement, as it already is for the sprite and sound tools; the `ci-dos` image has it and the `parity` job needs no extra step. `bun test web/sfx.test.ts` runs in that job. `tools/check-web-sfx.py` needs Chromium and runs locally.

`tools/build-web.sh` fails, naming both versions, if the installed tool differs from the version in `Cargo.lock`. It writes a static bundle to `dist/web` with relative URLs only, so a launcher (such as Fizz 1's) can bundle it under any path later.

## Running the checks

| Command | What it does | Needs |
| --- | --- | --- |
| `tools/check.sh` | Formatting, lints, tests, licence headers and the markdown style check | The pinned Rust toolchain |
| `cargo run -p fizz-ep2 --example replay` | Prints the native slice transcript | The pinned Rust toolchain |
| `tools/setup-dos.sh` | Links `vendor/` to the DOS toolchain when it is absent | `DOS_TOOLCHAIN`, or a `vendor/` already in place |
| `tools/build-dos.sh` | Builds `RTSMOKE.EXE`, `FIZZ2.EXE` and `CWSDPMI.EXE` into `dist/dos` | The DOS toolchain |
| `tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY` | Runs a DOS program in headless DOSBox and passes back its output and exit status | The DOS toolchain |
| `tools/check-dos.sh` | Builds, then checks that the `/REPLAY` transcript equals the golden file and the platform lines (`EXIT yes`, `FPU CW 027F`, `VECTORS RESTORED yes`, a BIOS tick count of 383 to 387), a keyboard smoke of both layouts (`MOVED yes`), the panic exit status of 3 and the `/BENCH` frame digests against the host's, and prints `SIZE` | The DOS toolchain |
| `tools/build-web.sh` | Builds `dist/web` | The `wasm32-unknown-unknown` target and the pinned CLI |
| `node tools/web-replay.mjs` | Runs the slice replay inside the built WASM module (Node and Bun both work) and prints `slice_replay_transcript()` | `dist/web` |
| `tools/check-parity.sh` | Compares the native, Web (Node or Bun) and DOS transcripts with `crates/fizz-ep2/slice-replay.txt` and prints `PARITY ok <hash>`, the final hash read from that file | Everything above |

CI has two jobs in `.github/workflows/ci.yml`. `check` runs `tools/check.sh` on `ubuntu-latest`. `parity` runs in the pinned `ci-dos` image and runs the DOS, Web and parity checks, the Web sound player's unit test and the generated-data checks. Its Web build installs Undertone with `bun install --frozen-lockfile`, so the runner needs access to the npm registry and nothing else.

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

## Performance

`FIZZ2.EXE /BENCH` and `cargo run --release -p fizz-ep2 --example bench` measure what the slice costs per tick. The budget is one tick at 60 Hz, 16,667 us, shared by the simulation step, the drawing and the copy to the screen.

### How `/BENCH` measures

The run plays the slice script once from a fresh level (1,268 ticks), unpaced, and times three sections of every tick: the step (`fizz_ep2::step`), the drawing (`render_slice`: the picture from `draw_slice` into the 320 by 200 buffer plus the HUD text) and the copy of the buffer to video memory (`present`). It prints their average and largest cost in microseconds, a total, and the final state hash, which must equal the golden's (`BENCH HASH`), so a bench run also proves the world it timed was the slice. It also prints a `BENCH FRAME <tick> <digest>` line for each of seven ticks: an FNV-1a digest of the 320 by 200 frame below the HUD strip, taken outside the timed sections, which `tools/check-dos.sh` compares with the digests the host computes from the same shared routines (`cargo run -p fizz-present-dos --example frames`), so a drawing routine that exists only in the DOS build is proved to draw the same pixels. It then prints `EXIT`, `FPU CW` and `VECTORS RESTORED` like the other modes. `FIZZ2.EXE /BENCH 120` times only the first 120 ticks (about a second at the settings below) for quick measurements while changing the drawing; a limited run ends on an earlier hash and is for timing only.

A 486 has no `rdtsc`, so the stopwatch is PIT channel 0, which the 60 Hz interrupt already programs: `timer::clocks()` is the interrupt count times the divisor (19,886) plus how far the counter has counted down in the current period, read with the latch command. One count is 0.838 us (1,193,182 Hz). That is fine enough for sections of tens of microseconds or more, and each section is timed per tick rather than in a loop, so the maxima are real. A reading costs two port reads and a port write, about 5 counts (4 us) in DOSBox at 26,800 cycles; `/BENCH` measures that first (`reading cost` on the `BENCH TIMER` line) and subtracts it from each section. Two edge cases are handled: an interrupt between the counter read and the interrupt count (the pair is read again), and the counter having reloaded before IRQ 0 has run, which makes a reading one period low (the stopwatch adds a period when a reading goes backwards). The 60 Hz handler itself still runs during the bench and appears in the maxima. Interrupts, vectors and pacing are unchanged, and the vectors are restored on exit and on panic like the other modes.

### DOSBox settings

`tools/check-dos.sh` runs the bench with `core=normal` and `cpu_cycles = 26800`, about a 486DX2-66 (26.8 million cycles a second), through `DOS_CYCLES=26800 tools/dosrun.sh`. The setting is the plain number: DOSBox Staging 0.83 rejects the form `fixed 26800`, logs `CPU: Invalid 'cpu_cycles' setting: 'fixed 26800', using '3000'` and carries on at 3,000 cycles a millisecond. `tools/dosrun.sh` now exits with status 125 and prints that log line whenever DOSBox reports an `Invalid '` setting, and `tools/check-dos.sh` tests the guard with the rejected form. Other modes keep `cpu_cycles = max`. The whole script takes about five seconds to emulate at 26,800 cycles, so the bench timeout is 120 seconds.

`/BENCH` also times a loop of 200,000 `dec`/`jnz` pairs (400,000 instructions, in 100 chunks, with the same stopwatch) and prints it as `BENCH CALIBRATE <loops> loops in <us> us (2 instructions per loop)`. DOSBox's normal core charges one cycle to each simple instruction, so the instructions per millisecond, `2 * loops * 1000 / us`, equal the configured cycles per millisecond. Measured: 2,968 at a setting of 3,000, 11,987 at 12,000, 26,739 at 26,800 and 80,128 at 80,000 (within 1% each time). `tools/check-dos.sh` asserts the implied rate is within 10% of the configured cycles, so a run that did not get the speed it asked for fails. The check also asserts the line shapes, the hash, the seven frame digests against the host's, the platform lines, positive averages for every section and a step average under 16,667 us, and prints the figures. It sets no tight bound on any of the timings.

### Numbers

An earlier version of these figures was taken at 3,000 cycles a millisecond, not at 26,800, because the setting was written in a form DOSBox rejects (see above). It was found when two runs at different `fixed` settings printed identical figures while plain numbers scaled. The cost then appeared to be about nine cycles an instruction and the drawing to be over the budget; neither conclusion holds, and every figure below was taken again.

The programs are `FIZZ2.EXE` built from the `feat/slice-bench` branch (before: 66,560 bytes) and with the drawing changes below (after: 69,632 bytes), run in DOSBox Staging 0.83.0 at the stated cycles, one play of the whole script (1,268 ticks). The share is of the 16,667 us tick.

At 26,800 cycles a millisecond, about a 486DX2-66 (the `tools/check-dos.sh` setting):

| Section | Before: average (us) | Before: largest (us) | Before: share | After: average (us) | After: largest (us) | After: share |
| --- | --- | --- | --- | --- | --- | --- |
| Step | 103 | 139 | 0.6% | 103 | 139 | 0.6% |
| Drawing | 17,828 | 21,665 | 107% | 2,728 | 3,198 | 16% |
| Copy to video memory | 3,589 | 3,601 | 22% | 599 | 611 | 3.6% |
| Total | 21,521 | 25,381 | 129% | 3,432 | 3,893 | 21% |

At 12,000 cycles a millisecond, a guess at a slower 486DX (the mapping from cycles to a named model is not something DOSBox can confirm):

| Section | Before: average (us) | Before: largest (us) | Before: share | After: average (us) | After: largest (us) | After: share |
| --- | --- | --- | --- | --- | --- | --- |
| Step | 231 | 310 | 1.4% | 231 | 310 | 1.4% |
| Drawing | 39,888 | 48,471 | 239% | 6,105 | 7,139 | 37% |
| Copy to video memory | 8,030 | 8,052 | 48% | 1,340 | 1,366 | 8.0% |
| Total | 48,150 | 56,785 | 289% | 7,677 | 8,694 | 46% |

The largest total tick after the changes is 23% of the budget at 26,800 cycles and 52% at 12,000. The figures scale with the cycle setting (the after total is 31,012 us at the 3,000 cycles the earlier version actually ran), as a cost counted in instructions should. Nothing here shows that a 486DX2-66 retires 26.8 million simple instructions a second, or that a 486DX-33 retires 12 million.

Natively (`cargo run --release -p fizz-ep2 --example bench`, 200 repetitions of the script, stepping only) the step costs about 88 ns a tick on the development machine, wall clock. `valgrind` was not available there, so the instruction count the example is written for (`valgrind --tool=callgrind`, with the command in the example's header) has not been taken; wall-clock time on a modern machine says little about a 486 and is an extra.

### Drawing changes

Each change was measured with `FIZZ2.EXE /BENCH 120` (the first 120 ticks) at 26,800 cycles a millisecond and kept only for a clear gain. The figures are the drawing average in microseconds; the step did not move, and the copy moved only where stated.

| Change | Drawing before | Drawing after |
| --- | --- | --- |
| `memset` on `rep stosd` in `dos/rt/src/mem.rs` (the sky fill and every tile row ended in the generic loop) | 19,035 | 11,434 |
| `memcpy` on `rep movsd` in the same file (the copy to video memory ends in it; the copy went from 3,589 to 599) | 11,434 | 11,429 |
| `draw_tilemap_on_sky`: sky and tiles as one layer of merged runs, each pixel written once | 11,429 | 5,145 |
| A tile band's runs filled into its first row, the other rows copied from it | 5,145 | 4,274 |
| `blit_sprite` clips once and blits rows as slices | 4,274 | 3,963 |
| Closed doors noted during the tile pass in place of a second scan of the view | 3,963 | 3,456 |
| HUD glyphs wholly inside the frame drawn without a clip test per pixel | 3,456 | 3,318 |
| A band's rows copied in doubling blocks (four copies, not fifteen) | 3,318 | 3,087 |
| Tile colours from a 256-entry table and `map.data` in place of `get` and a `match` | 3,087 | 2,814 |

The first 120 ticks end at 2,814 us and the whole script averages 2,728 us, because the later ticks show different things.

Tried and not kept, because the gain was a few percent at most: a generic parameter in place of the `&mut dyn FnMut` for the tile colours (under 1%), and drawing glyphs four pixels at a time through a nibble mask (2.7%, for more code than the gain is worth).

What stayed identical: the pinned frame digests in `fizz-slice-view` (ticks 0, 150, 270, 400, 602, 700, 865, 1000 and 1268), every `fizz-view` test, the replay golden, the state hash (`PARITY ok 2145503c0d3b625f`) and the seven DOS frame digests. The simulation, `fizz-phys` and `fizz-ep2` were not touched. The portable changes (the tile layer, the sprite blit, the door list) are in the code the Web build also runs; each has a test against the older definition of the picture, and the Web build was not timed.

How the drawing's last 2,800 us splits (first 120 ticks): the figures come from leaving pieces out, measured at 3,000 cycles before the setting was corrected and scaled to this one, so they are approximate. The sky and tile layer is about 600 us (writing the 64,000 bytes once is about 600 us on its own; the rest is the per-tile colour lookups and the run bookkeeping), the HUD text about 530 us, the sprites and the key, chicken and shots after the tiles about 575 us, and the camera and the black HUD strip the remainder.

### The cost unit in DOSBox

The calibration loop (see above) shows what a unit of cost is: one cycle for a simple instruction, so at 26,800 cycles a millisecond a simple instruction costs 37 ns. The `rep stosd` and `rep movsd` of a 64,000-byte frame (16,000 dwords) take about 600 us, one cycle a dword, as the `BENCH PRESENT` figure of 599 us shows. The cost of the drawing is the number of instructions and string-instruction elements it runs. That is why the fixes that mattered run fewer of them (one long `rep` for many short loops, one clip for many, one table read for a `match`).

It is also a caution: a count of one cycle an instruction is not a 486's. Many simple instructions take one clock there, but loads, stores, branches and string instructions take more, so the absolute figures are a guide to the order of magnitude, and they say nothing about the x87.

### What the numbers say

The step is cheap: about 2,800 emulated cycles a tick (103 us at 26,800), 0.6% of the budget. The drawing was the problem: 107% of the budget on its own at 26,800 cycles and 239% at 12,000, so the slice did not hold 60 Hz at either setting. After the changes above the drawing is 2,728 us (16% of a tick) at 26,800 and 6,105 us (37%) at 12,000, and a whole tick costs 21% and 46% of the budget, the largest tick 23% and 52%. In the emulator, then, the slice holds 60 Hz at both settings with room to spare. The target of the drawing under 30% of a tick is met at 26,800 cycles and missed at 12,000.

Writing the 64,000-byte frame once costs about 600 us and copying it to the screen the same again, so a full-frame redraw plus the copy is about 7% of a tick before any tile, sprite or text is drawn. What remains is small pieces (glyphs 2.7%, looking sprites up once instead of by string compare, not drawing the rows under the black HUD strip, formatting the HUD numbers without `core::fmt`), each a few percent, and a change of design (keeping the scene between ticks and drawing again only what moved) that these figures do not call for.

The copy figure is DOSBox's cost for 16,000 dword writes to its emulated video memory and does not stand for a real card: a real VGA's bus speed decides it, and a real ISA or PCI card takes several milliseconds for 64,000 bytes whatever the instruction. Treat it as a lower-confidence figure than the other two; a slow ISA card alone could take a large share of the budget.

These numbers do not say how a real 486DX behaves. DOSBox cycles are an approximation: it does not model cache, memory or bus timing, or a slow ISA video card, and the cost of x87 instructions, which the step uses for every `f64` operation and the drawing uses for each sprite position (`tile_px` takes a floor through `f64`), is not measured. DOSBox charges them the same as other instructions, so the step and the position arithmetic are floors, not estimates. The mapping from a cycle count to a named 486 model is a guess. No i486 cycle weighting of the instruction stream (as Fizz 1's `cyc486.py` does) was run for these changes, and `valgrind` is not installed here, so there is no instruction count either. That the changes help a real 486 rests on their running fewer instructions (a `rep stosd` stores a dword in about a clock after a few clocks of start-up, against five or more instructions for the generic loop), not on a measurement. The 486 floor claim stands on the order of magnitude until the build is run on hardware or a cycle-accurate model.

## Size budget

A 1.44 MB floppy holds 1,474,560 bytes raw, about 1,457,664 bytes of data area. Fizz 1's release scripts set the reserves:

| Disk | Room for the EXE and data | Reserve |
| --- | --- | --- |
| FreeDOS-bootable | About 1.17 MB | FreeDOS needs about 153 KB and 64 KB is kept free for saves |
| Plain | About 1.39 MB | 8 KB kept free |

The budget covers the executable and the Fizz Asset Packs (`.FAP`) together (see [Assets and packs](ARCHITECTURE.md#assets-and-packs)). `tools/check-dos.sh` prints `SIZE <bytes>` for `FIZZ2.EXE` against a provisional `BUDGET 1100000` and fails if it is over; it counts the executable alone until packs exist, and then counts both. The figure is a tracked metric, not a design limit. Fizz 2's own save and config reserve is still to be confirmed. The Milestone 0 `FIZZ2.EXE` is 32,768 bytes, unstripped; with the slice (the rules, the level, the sprite data and the drawing) it was 72,192 bytes, and deleting the marker's rules, drawing and replay took it to 61,440 bytes, about 5.6% of the budget. The `/BENCH` mode adds about 3.6 KB (62,976 to 66,560 bytes), and the drawing changes with the frame digests add about 3 KB more (69,632 bytes).

For scale, Fizz 1 embeds about 436 KB of assets in an 862 KB stripped executable: `sprites.bin` 102,252 bytes, `sound.bin` 279,348 bytes and `titles.bin` 54,488 bytes. The Episode 2 sprite design canvas holds 211 sprites, about 146 KB at one byte per pixel and about 73 KB at four bits per pixel. That count includes variants of Ben and three enemy rosters, so the sprites that ship will be fewer. Music and the larger backdrops are likely to take more room than sprites do.

## Provenance

Files mirrored or adapted from other work name their source in a header comment. Everything is dual-licensed Apache-2.0 OR MIT.

| Origin | Files |
| --- | --- |
| Mirrored from Desk's d0 spike and the earlier proof of concept (tag `poc-1`) | `dos/dos-elf.json`, `dos/link.ld`, `dos/rustfmt.toml`, and `dos/rt/src/` `start.rs`, `dpmi.rs`, `dos.rs`, `heap.rs`, `env.rs`, `panic.rs`, `cleanup.rs`, `io.rs` |
| Adapted | `dos/.cargo/config.toml`; `dos/rt/src/` `lib.rs`, `fpu.rs` (keeps only the control word), `irq.rs`, `timer.rs` (60 Hz in place of 560 Hz) and `vga.rs`; `tools/elf2go32.py`, `tools/dosrun.sh`, `tools/build-dos.sh`, `tools/setup-dos.sh`; `crates/fizz-hal/src/ring.rs` from Desk's `ring.rs` |
| Derived from Fizz 1 `crates/sim/src/` at commit `4bce036`, transformed as in fizz-dos `m2/patches/sim.patch` | `crates/fizz-phys/src/` `tilemap.rs`, `body.rs`, `platform.rs` and `rng.rs`; `fmath.rs` from fizz-dos `m2/sim/src/fmath.rs` without `exp`. `tools/lf-sim-drift.sh` compares them with a Fizz 1 checkout |
| Ported from Fizz 1 `episodes/episode-1/game/src/` (`world.rs`, `ents.rs`, `tiles.rs`) at commit `4bce036` | `crates/fizz-ep2/src/` `ben.rs`, `chicken.rs`, `shot.rs`, `tiles.rs` and `world.rs`, proved by the traces in `crates/fizz-ep2/tests/fizz1/` |
| Adapted from fizz-dos (`lieutenant-fizz-dos`, commit `221dd6c`, the same author's code, MIT there) | `tools/sfx/convert.ts` from `m4/sound/fizz-dos/sfx.ts`, `tools/sfx/opl.ts` from `opl.ts` and `tools/sfx/patches.ts` from `patches.ts`; see [`SFX.md`](SFX.md) |
| Ported from Fizz 1's DOS port, fizz-dos `m2/dos-rt/src/preview.rs` at commit `221dd6c` (`Cells`, `plate`, `rows`, the box drawing and the four-panel layout), with the changes listed in the file's header | `crates/fizz-machine/src/screen.rs`; the golden `tests/check-cells.txt` follows the format of fizz-dos `m5/bench/expected-startup-cells.txt` |
| Written for this repository | `dos/rt/src/` `ring.rs`, `port.rs` and `kbd.rs`; `dos/rtsmoke`; the rest of `crates/fizz-hal`, which follows Desk's shapes for time, input and video without port traits |

## Not verified

- Real 486 hardware. Every DOS result comes from DOSBox.
- x87 cycle cost. DOSBox with `cycles=max` hides it, and a fixed cycle count charges every simple instruction alike; the `/BENCH` figures are DOSBox's, not a 486's (see [Performance](#performance)).
- The live Web keyboard and canvas on a real browser session. The WASM module is replayed under Node and Bun, and headless Chromium has received key events for both layouts over the DevTools protocol, but a person has not played it.
- Browser and desktop shortcut clashes (Ctrl+W, Alt combinations, workspace shortcuts) and Keyboard Lock on real desktops. A headless browser cannot show either.
- Audio. The cluck events are proved by the transcript, and the Web page's request for each cluck is checked against a recording stand-in for `AudioContext` (`web/sfx.test.ts`, `tools/check-web-sfx.py`). Nobody has listened to a cluck, on the Web or anywhere else, and no result here says how it sounds. DOS makes no sound yet. Autoplay behaviour, a hidden tab and overlapping clucks have not been tried in a real browser.
- The slice replay on real platforms. Native x86_64 (debug and release builds, which agree), DOS (headless DOSBox, with the x87 at 53 bits) and the Web build (WASM under Node or Bun) each reproduce `slice-replay.txt`, compared by `tools/check-parity.sh`; no 486 and no browser session has produced it.
- The Web slice in a browser played by a person. Headless Chromium loaded the page, moved Ben on Keen-layout key events, fired a shot and used Restart, but it did not play the level to the exit; the level's drawing is the first one and not judged.
- That the slice level is pleasant to play. The replay proves it can be finished, not that it is fun. The DOS build's `/LIVE` has been played only by DOSBox AUTOTYPE key presses, so the feel of the camera and the keys, and the speed of the drawing on a real 486, are unverified. The DOS screenshot in the manual comes from DOSBox.
- Bundling the Web build into a launcher.
- The GitHub-hosted runner for the `parity` job.
