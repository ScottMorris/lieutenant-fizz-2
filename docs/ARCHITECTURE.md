# Architecture

Lieutenant Fizz 2 is one game with two first-class presentations, Web and DOS, built in parallel from one deterministic simulation. This document defines the layers, the crate plan, the parity policy and the references the project mirrors. [`POC-1-FINDINGS.md`](POC-1-FINDINGS.md) records the proof of concept these decisions come from.

## Layers

```text
CONTENT     episode data and rules, no platform types
              step(InputFrame) at a fixed 60 Hz
INTENT      scene: tilemap, sprites, lights, camera
            audio: composition source, named cues
            events: semantic names, never sound or pixel data
COOKING     offline host tools lower authored assets per target
PRESENTERS  Web (canvas or WebGL, Undertone)  ·  DOS (320x200, OPL2)
PLATFORM    clock, input, storage, display, audio services
              Web: JS shim   DOS: Desk-style hardware abstraction layer
```

- **Intent flows up, lowering flows down.** The simulation never emits platform-shaped data such as palette indices, OPL patches or capacity-limited tables. Presenters and cookers project intent down to their hardware. The shared contract is never shaped like the weakest target.
- **Fidelity is lost at cook time.** Art and music are authored for the richest presentation. Offline tools produce the DOS forms. DOS parses nothing at start-up that a tool could have prepared.
- **A presenter may add what the rules do not know about.** The Web zoom-out level map and the multi-voice Undertone mix are presenter features. DOS can omit them.

## Web and DOS parity policy

Both presentations are developed together, so neither is a late port.

- A feature is done when it has a Web path and a DOS path. The DOS path may be reduced (flatter lighting, fewer effects, simpler audio), but it runs.
- Web leads on feel and fidelity; DOS leads on budgets. A 486DX is the DOS floor. DOS is a bonus platform and never limits the design.
- The state hash from `fizz-sim` is the arbiter. The native, Web and DOS builds hash the same fields in the same order after each tick. A mismatch is a bug until proven otherwise.
- CI gates both from the start. `check` runs the host checks everywhere. The `parity` job runs in the pinned `ci-dos` image: it builds FIZZ2.EXE and the Web bundle, runs the replay natively, in WASM under Node and Bun, and in headless DOSBox, and fails unless all transcripts are identical. `tools/check-parity.sh` runs the same comparison locally and prints `PARITY ok <hash>`.
- The parity gate compares the `TICK` and `FINAL` lines of a versioned replay transcript, which carry the state hash every 60 ticks. Platform-specific lines (`FPU`, `BIOS`, `VECTORS`, `SIZE`) use other prefixes and are checked by the platform's own gate. [`PLATFORM.md`](PLATFORM.md) defines the transcript and the commands.
- Cooked assets have per-target profiles. A missing DOS cook falls back to a placeholder so Web iteration is never blocked, and the DOS gate still reports the gap.

The cost of parallel development is the DOS toolchain and its slower feedback. Keep a quick, host-only `tools/check.sh` for the inner loop and run the full DOS gate in CI and before a PR.

## Numeric model

Fizz 1's physics are continuous and tuned by feel, so Fizz 2 keeps Fizz 1's numeric model: `f64` state stepped at a fixed 60 Hz, under a determinism discipline.

- Basic IEEE operations only, and no reordering of floating-point operations for speed.
- A software `libm` for transcendental functions, so results do not depend on the platform's maths library.
- x87 set to 53-bit precision on DOS before the first tick, so 80-bit intermediates do not leak into results.
- Display-only motion (particles, lighting, interpolation, camera easing) may use any numeric form because it never feeds back into the rules.

Where floating point is allowed follows from that model:

- Rules crates (`fizz-sim`, `fizz-hello` and the game logic that replaces it) use `f64` under the discipline above, and may not use the `exp` family (`exp`, `exp2`, `exp10`) because `libm` runs x87 assembly for those on x86 targets without SSE.
- The Web presenter may use floats freely for display, because WebAssembly floating point is IEEE and the rules already agree across platforms.
- `fizz-hal`, `fizz-present-dos` and `fizz-dos-rt` deny `clippy::float_arithmetic`. The DOS presenter works in integers, and interrupt handlers never touch the x87, so a handler cannot disturb the control word or the register stack of the interrupted game loop.

[`PLATFORM.md`](PLATFORM.md) records the specific decisions (the `libm` pin, snap-to-zero of tiny velocities, the control word the program sets) and why each exists.

Fixed-point rules remain the alternative if a later episode drops continuous physics. The 486DX has an on-chip FPU. A 486SX would need software floating point and is out of scope. Measure before changing the model: Fizz 1's `m5/bench` and its 486 cycle estimator are the reference tools, and DOSBox at fixed cycles hides x87 cost.

## Assets and packs

The executable holds code and small constant tables. Sprites, tile sets, backdrops, levels and music live in cooked packs: files that the cooker writes and each platform loads. This is the plan for Milestone 2; no pack exists yet.

- **In the executable:** the palette, trigonometry tables and dither patterns. They are small, they need no parsing and a table in read-only data costs nothing at start-up.
- **In packs:** everything authored. Pixel data is an opaque byte array to the compiler, so embedding it gives the optimiser nothing to specialise on.
- **Memory:** the DOS stub loads the whole executable image at start-up, so embedded art stays resident for the whole run. A pack loads for the level in play and is freed afterwards, which matters on a 486 with 4 to 8 MB.
- **Iteration:** a change to art or a level is a re-cook of one file, with no Rust rebuild or relink.
- **One layout, two platforms:** the Web build fetches the same pack layout (cached by the browser, or bundled by Tauri) and does not carry the assets inside the WASM module. A pack is cooked per target, so the DOS and Web packs differ in content but not in structure.
- **Pack format:** a header with a format version, the build version and a checksum, then an offset table, then data in the layout the loader uses as read. DOS reads a pack with one file read into memory and parses nothing. Pack names follow the 8.3 rule, for example `FIZZ.DAT` and one pack per world. The executable refuses a pack whose build version differs from its own.
- **Costs:** file-reading code in the DOS runtime (DOS function calls it already wraps), an error path for a missing or mismatched pack, and slower loads from a real floppy, so a level's pack loads when the level starts and not all packs at once.
- **Saves and configuration** stay in separate files, as in Fizz 1.

Fizz 1 embeds `sprites.bin` (102,252 bytes), `sound.bin` (279,348 bytes) and `titles.bin` (54,488 bytes) in its DOS executable. That is about 436 KB of its 862 KB stripped executable, and sound is the largest part. The size budget in [`PLATFORM.md`](PLATFORM.md#size-budget) counts the executable and the packs together.

## Crate plan

Crates arrive with their first consumer and test. Milestone 0 built the platform skeleton, a marker moved by input and hashed on every platform, so the crates below marked Exists are real but carry only that much behaviour. [`PLATFORM.md`](PLATFORM.md) describes how they fit together.

| Crate | Layer | Purpose | State |
| --- | --- | --- | --- |
| `fizz-sim` | Rules | Fixed tick, state hash, and the input types (`Buttons`, `InputFrame`, `InputLatch`) | Exists |
| `fizz-hello` | Rules | Throwaway Milestone 0 rules: a marker moved by input, used to prove parity. Deleted when real rules arrive | Exists, Milestone 0 only |
| `fizz-hal` | Platform | Hardware abstraction traits mirrored from Desk: clock and pacer, input, display, and the interrupt hand-off ring. Storage and audio traits join with their consumers | Exists (clock, input, display) |
| `fizz-present-dos` | Presenter | DOS presentation: the indexed 320x200 frame, palette, font and key mapping, with no DOS calls | Exists |
| `fizz-present-web` | Presenter | Web presentation: a `wasm-bindgen` `App` that paces the rules from browser time | Exists |
| `fizz-dos-rt` (`dos/rt`) | Platform | The DOS runtime: start-up, DPMI, heap, interrupts, 60 Hz timer, keyboard, VGA and clean-up. Lives in the separate `dos/` workspace | Exists |
| `fizz-engine` | Intent | `Game`, scene intent and events | Planned |
| `fizz-cook` | Cooking | Host tool: scripts to level data, art and music to per-target packs | Planned |

`InputFrame` first appeared in this plan under `fizz-engine`. It now lives in `fizz-sim` next to `Tick`, because both presenters and the rules need it before any scene intent exists, and it has no dependency on scenes or events. `fizz-engine` keeps `Game` and the scene when they arrive.

The DOS programs (`RTSMOKE.EXE`, `FIZZ2.EXE`) are crates of the `dos/` workspace, which has its own target and build flags and is excluded from the root workspace so host checks never need them.

Names are provisional. Rules that depend on a presenter are a bug.

## What is mirrored from where

- **Fizz 1** (`liminal-hq/lieutenant-fizz`) is the behaviour reference: the pogo physics, the camera (including look up and down and the Web zoom-out map), the enemy kinds, locks and keys, the overworld. When Fizz 2 reproduces a Fizz 1 behaviour, it names the Fizz 1 source and proves the match with a trace test. Fizz 1 stays as it is.
- **Fizz 1's DOS port** (`ScottMorris/lieutenant-fizz-dos`) supplies the 486 performance model and its palette frame technique: 16 colours by 14 brightness levels with a shade pass for lights.
- **Desk** (`liminal-hq/desk`) supplies the hardware abstraction layer shape, the rule that shared crates forbid `unsafe` and never call DOS directly, the no-float-in-graphics rule for presenters, the toolchain and the check scripts. Its ideas are mirrored here now, kept behind `fizz-hal` and `fizz-dos-rt`, so the real Desk crates can be swapped in later.
- **Waypoint and Spindle** supply repository conventions: the README shape, CI, release notes grouped by labels, and the pull request and issue templates.
- **Fizz Level Studio** supplies the level script language and the proof that scripts rebuild Fizz 1's levels exactly. Fizz 2 uses the script as its authoring format and the cooker as the single place that turns it into engine data.

## Open questions

- Is Fizz 1's pogo physics reused directly or ported with the same constants and proved by golden traces?
- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later?
- Which language does the cooker use? Rust is preferred so it shares types.
- Do the level script and the studio need extensions for vertical mazes, gadgets and the underworld?
- What base art resolution does Episode 2 author for, given the DOS palette limits?
- What pack granularity and compression does Episode 2 use: one pack per world or per level, and whether packs are compressed, which trades load time against floppy space?
- How are sprites laid out in a pack: bits per pixel, a mask or a transparent colour, and whether a sprite is stored as runs?
