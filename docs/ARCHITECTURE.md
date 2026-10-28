# Architecture

Lieutenant Fizz 2 is one game with two first-class presentations, Web and DOS, built in parallel from one deterministic simulation. This document defines the layers, the crate plan, the parity policy and the references the project mirrors. [`POC-1-FINDINGS.md`](POC-1-FINDINGS.md) records the proof of concept these decisions come from.

## Layers

```text
CONTENT     episode data and rules, no platform types
              step(InputFrame) at a fixed 60 Hz
INTENT      scene: tilemap, sprites, lights, camera
            audio: composition source, named cues
            events: semantic names, never sound or pixel data
BAKING     offline host tools lower authored assets per target
PRESENTERS  Web (canvas or WebGL, Undertone)  ·  DOS (320x200, OPL2)
PLATFORM    clock, input, storage, display, audio services
              Web: JS shim   DOS: Desk-style hardware abstraction layer
```

- **Intent flows up, lowering flows down.** The simulation never emits platform-shaped data such as palette indices, OPL patches or capacity-limited tables. Presenters and cookers project intent down to their hardware. The shared contract is never shaped like the weakest target.
- **Fidelity is lost at bake time.** Art and music are authored for the richest presentation. Offline tools produce the DOS forms. DOS parses nothing at start-up that a tool could have prepared.
- **A presenter may add what the rules do not know about.** The Web zoom-out level map and the multi-voice Undertone mix are presenter features. DOS can omit them.

## Web and DOS parity policy

Both presentations are developed together, so neither is a late port.

- A feature is done when it has a Web path and a DOS path. The DOS path may be reduced (flatter lighting, fewer effects, simpler audio), but it runs.
- Web leads on feel and fidelity; DOS leads on budgets. A 486DX is the DOS floor. DOS is a bonus platform and never limits the design.
- The state hash from `fizz-sim` is the arbiter. The native, Web and DOS builds hash the same fields in the same order after each tick. A mismatch is a bug until proven otherwise.
- CI gates both from the start. `check` runs the host checks everywhere. The `parity` job runs in the pinned `ci-dos` image: it builds FIZZ2.EXE and the Web bundle, runs the slice replay natively, in WASM under Node or Bun, and in headless DOSBox, and fails unless all transcripts equal the golden file, `crates/fizz-ep2/slice-replay.txt`. `tools/check-parity.sh` runs the same comparison locally and prints `PARITY ok <hash>`.
- The parity gate compares the `TICK`, `FINAL` and `EVENT` lines of a versioned replay transcript, which carry the state hash every 60 ticks and the semantic events the rules emit. Platform-specific lines (`FPU`, `BIOS`, `VECTORS`, `SIZE`) use other prefixes and are checked by the platform's own gate. [`PLATFORM.md`](PLATFORM.md) defines the transcript and the commands.
- Baked assets have per-target profiles. A missing DOS bake falls back to a placeholder so Web iteration is never blocked, and the DOS gate still reports the gap.

The cost of parallel development is the DOS toolchain and its slower feedback. Keep a quick, host-only `tools/check.sh` for the inner loop and run the full DOS gate in CI and before a PR.

## Numeric model

Fizz 1's physics are continuous and tuned by feel, so Fizz 2 keeps Fizz 1's numeric model: `f64` state stepped at a fixed 60 Hz, under a determinism discipline.

- Basic IEEE operations only, and no reordering of floating-point operations for speed.
- A software `libm` for transcendental functions, so results do not depend on the platform's maths library.
- x87 set to 53-bit precision on DOS before the first tick, so 80-bit intermediates do not leak into results.
- Display-only motion (particles, lighting, interpolation, camera easing) may use any numeric form because it never feeds back into the rules.

Where floating point is allowed follows from that model:

- Rules crates (`fizz-sim`, `fizz-phys`, `fizz-ep2`) use `f64` under the discipline above, and may not use the `exp` family (`exp`, `exp2`, `exp10`) because `libm` runs x87 assembly for those on x86 targets without SSE.
- The Web presenter may use floats freely for display, because WebAssembly floating point is IEEE and the rules already agree across platforms.
- `fizz-hal`, `fizz-present-dos` and `fizz-dos-rt` deny `clippy::float_arithmetic`. The DOS presenter works in integers, and interrupt handlers never touch the x87, so a handler cannot disturb the control word or the register stack of the interrupted game loop.

[`PLATFORM.md`](PLATFORM.md) records the specific decisions (the `libm` pin, snap-to-zero of tiny velocities, the control word the program sets) and why each exists.

Fixed-point rules remain the alternative if a later episode drops continuous physics. The 486DX has an on-chip FPU. A 486SX would need software floating point and is out of scope. Measure before changing the model: Fizz 1's `m5/bench` and its 486 cycle estimator are the reference tools, and DOSBox at fixed cycles hides x87 cost.

## Assets and packs

The executable holds code and small constant tables. Sprites, tile sets, backdrops, levels and music live in Fizz Asset Packs (`.FAP`): files that the baker, `fizz-bake`, writes and each platform loads. This is the plan for Milestone 2; no pack exists yet.

- **In the executable:** the palette, trigonometry tables and dither patterns. They are small, they need no parsing and a table in read-only data costs nothing at start-up.
- **In packs:** everything authored. Pixel data is an opaque byte array to the compiler, so embedding it gives the optimiser nothing to specialise on.
- **Memory:** the DOS stub loads the whole executable image at start-up, so embedded art stays resident for the whole run. A pack loads for the level in play and is freed afterwards, which matters on a 486 with 4 to 8 MB.
- **Iteration:** a change to art or a level is a re-bake of one file, with no Rust rebuild or relink.
- **One layout, two platforms:** the Web build fetches the same pack layout (cached by the browser, or bundled by Tauri) and does not carry the assets inside the WASM module. A pack is baked per target, so the DOS and Web packs differ in content but not in structure.
- **Pack format:** a header with a format version, the build version and a checksum, then an offset table, then data in the layout the loader uses as read. DOS reads a pack with one file read into memory and parses nothing. Pack names follow the 8.3 rule, for example `FIZZ.FAP` and one pack per world, such as `LIGHT.FAP`. The executable refuses a pack whose build version differs from its own.
- **Costs:** file-reading code in the DOS runtime (DOS function calls it already wraps), an error path for a missing or mismatched pack, and slower loads from a real floppy, so a level's pack loads when the level starts and not all packs at once.
- **Saves and configuration** stay in separate files, as in Fizz 1.
- **Sprites before packs:** sprites are committed text grids under `assets/sprites/`, lowered to `const` data in `fizz-view` until the pack format exists. `SPRITES.md` specifies the format and the tools.
- **Sound before packs:** the chicken's cluck is a rich Undertone effect under `assets/sfx/`, the source of truth. `tools/sfx/downmix.ts` derives an OPL2 chunk (register writes timed at 560 Hz) and a PC speaker divisor list (140 Hz) from it into `const` data in `fizz-sfx`, and `--check` keeps the generated file current. The Web plays the rich source through Undertone, bundled into the page by Bun (`web/sfx.ts`); DOS steps the downmixes from its timer interrupt. `SOUND.md` describes the pipeline and what each step loses.

Fizz 1 embeds `sprites.bin` (102,252 bytes), `sound.bin` (279,348 bytes) and `titles.bin` (54,488 bytes) in its DOS executable. That is about 436 KB of its 862 KB stripped executable, and sound is the largest part. The size budget in [`PLATFORM.md`](PLATFORM.md#size-budget) counts the executable and the packs together.

## Web view and pixel-locking

This is the plan for Milestone 3; the view itself is not built. The Web page today plays the slice in a canvas 208 pixels (13 tiles, as Fizz 1) tall and 96 to 1280 pixels wide, as wide as the window's aspect gives, drawn by the same routine as the DOS frame at that size (`App::set_view`), and scales it by the window height over 208 with `image-rendering: pixelated`, so it fills the window. The view size is a drawing input only: the rules never read it, so hashes and events are the same at every size and on DOS. A small HUD (the Fizz count, the deaths and the key) is DOM over the picture in `#stage`, in Fizz 1's pixel font and scaled by the stage's height (`web/hud.ts`, `docs/PLATFORM.md`). The controls, notes and the debug lines (`TICK`, `HASH` and the rest) sit in an overlay panel that `H` toggles.

- **The Web view is not a projection of a fixed frame.** It fills the window and shows more of the level on larger screens, as Fizz 1's does. DOS keeps its fixed 320 by 200 mode 13h frame.
- **Pixels are square.** The Web view does not stretch to the 4:3 shape of a CRT.
- **The HUD stays page text on the Web.** DOS draws its HUD inside the frame.
- **Fizz 1 is the model.** Its camera works from a target tile count (13 tiles tall for levels, 12 for the map, 14 for cinematics). Its Sharp mode picks a whole-number scale, `max(2, floor(viewport height / (16 x target tiles)))`, and the leftover height shows more of the level. Its Fast mode draws one canvas pixel per sprite pixel and sizes the canvas box so the browser's upscale is an exact whole number. Its camera snaps to the canvas-pixel grid, so scenes move by whole pixels and nothing shimmers. A zoom, from a close-up of 3 times to a whole-level overview, changes how many tiles are visible. Fizz 1's desktop default is its Soft mode, which uses a fractional scale on a device-pixel canvas; its whole-number modes are Sharp and Fast. Fizz 2's Web view uses the whole-number behaviour so the image stays crisp.
- **Fizz 1 sets no maximum scale.** The scale grows with the window, so a larger screen shows the same number of tiles at a larger size unless the player zooms out. A pixel budget of 3840 by 2160 limits the canvas backing, not the scale.
- **Rounding is decided once.** The Web marker is drawn with `Math.round` and DOS truncates with `as i32`, so the two can differ by a pixel. One shared rule, written in Rust and tested, replaces both.
- **What exists in `fizz-view`.** `pixel` is the floor rule. `tile_px(v)` is `pixel(v * 16.0)`, and `screen_y(world_y, map_h_tiles)` flips the rules' y-up heights to screen rows, flooring first. `follow(target_px, map_px, view_px)` is a stateless, clamped camera origin for one axis (called once per axis each frame, with no easing; the eased camera stays Milestone 3), and it centres a map that is smaller than the view. The slice's look up and look down fit that shape without hidden camera state: the rules keep a small `look_offset` in Ben's state (a ramp of his look timers, hashed with them), the view adds it to the target, and `follow` clamps the result (see `PLATFORM.md`, Looking up and down). `blit_sprite`, `fill_rect`, `draw_tilemap` and `draw_tilemap_on_sky` (the tiles and the sky as one layer of merged runs, each pixel written once) draw into a plain palette-indexed `&mut [u8]` of any width and height, clipped for every position. The slice's DOS and Web presenters consume these so neither repeats the pixel rule: they reach them through `fizz-slice-view`, which uses `f64` for positions as `fizz-view` does (the DOS runtime sets the x87 to 53 bits before the first tick), while the HUD and everything else in `fizz-present-dos` stays in integers.

The open question is whether Fizz 2 shows more level on large screens by default, with a larger target tile count or a capped scale, or leaves that to a zoom control as Fizz 1 does. It is decided at Milestone 3 with real levels, because the size of the view changes how much of a maze the player can see.

## Crate plan

Crates arrive with their first consumer and test. Milestone 0 built the platform skeleton and Milestone 1 the first playable slice on it; the crates below marked Exists carry that much behaviour. [`PLATFORM.md`](PLATFORM.md) describes how they fit together.

| Crate | Layer | Purpose | State |
| --- | --- | --- | --- |
| `fizz-sim` | Rules | Fixed tick, state hash, and the input types (`Buttons`, `InputFrame`, `InputLatch`) | Exists |
| `fizz-hal` | Platform | Hardware abstraction traits mirrored from Desk: clock and pacer, input, display, and the interrupt hand-off ring. Storage and audio traits join with their consumers | Exists (clock, input, display) |
| `fizz-view` | Presenter base | Platform-neutral placement: turns the rules' positions into logical pixels that each presenter places in its own environment, the shared 8x8 bitmap font with clipped `draw_text`, the `theme` module that defines Fizz's accent colour (light magenta, EGA 13) once for every presenter and the stylesheet, and, for captions, Fizz 1's proportional mixed-case font (`pfont`) (the 8x8 moved here from `fizz-present-dos` so the slice view can draw in-world captions; the DOS HUD draws through it too). The eased camera and the scene-to-screen step of Milestone 3 join it | Exists (the pixel rule, tile and y-flip helpers, the follow camera, clipped sprite, rectangle and tile drawing, generated sprite data) |
| `fizz-slice-view` | Presenter base | The one scene-drawing routine for the Episode 2 slice: `draw_slice(frame, w, h, &World, tick)` paints sky, the tile window, the key, the door, the chicken, the shots and Ben into a palette-indexed buffer, with the follow camera. Both presenters call it, so Web and DOS draw the same pixels; the HUD text stays presenter-side. `no_std`, no `unsafe`, depends on `fizz-ep2` and `fizz-view`. It reads the world and never changes it, and its `tick` argument drives animation only. `draw_slice_with` takes `DrawOptions` (captions on or off, and the camera shake after Ben dies on or off) and draws the chicken's open beak and, as pixel text, its floating caption from the chicken's unhashed `cluck_age` and `cluck_variant`; `caption_view` gives the same caption as data (word, colours, age, feet position, the one-tile reach rule, with no clamping) for a presenter that draws it another way, as the Web page does, as DOM text | Exists |
| `fizz-present-dos` | Presenter | DOS presentation: the indexed 320x200 frame, palette and key mapping, with no DOS calls; its HUD box uses Fizz 1's pixel font in `fizz-view` | Exists |
| `fizz-present-web` | Presenter | Web presentation: a `wasm-bindgen` `App` that paces the slice's rules from browser time and renders them through `fizz-slice-view` into an RGBA frame the page copies to its canvas | Exists |
| `fizz-machine` | Platform | The pure model of the DOS start-up check: the `Probe` of what a machine has, the sound `Settings` with the `FIZZ2.CFG` layout and command-line flags, `resolve` (choices and probe to a sound plan with three-state device rows), the OPL3 write router `route`, and `cells`, the 80 by 25 text screen with its golden file. `no_std`, no `unsafe`, no dependencies; it also prints and parses the `PROBE` lines and hashes a screen. See [`DOS-CHECK.md`](DOS-CHECK.md) | Exists (`FIZZ2.EXE /PROBE` uses the probe model; the screen is not drawn yet) |
| `fizz-menu` | Platform | The menu model shared by the Web page and the DOS screens: a generic list with a cursor (`Menu`, `MenuKey`, `Outcome`), the Options screen (`Values` for every setting with conversion to and from `Prefs`, `Caps` for the platform and the devices this machine has, `OptionsScreen`, and `RowView`s carrying every label and value word) and the Paused menu. A value the machine cannot use is skipped when cycling, a saved choice whose device is missing is shown as "(not found)" and kept, and a row set by a switch shows `*` until it is changed. `no_std`, no allocation, no `unsafe`, no floating point; depends only on `fizz-machine` | Exists (the Web page draws it as `web/options.ts`'s overlay through `fizz-present-web`'s `menus.rs`; the DOS crate links it to prove it builds for the DOS target and its screens are not drawn yet) |
| `fizz-dos-rt` (`dos/rt`) | Platform | The DOS runtime: start-up, DPMI, heap, interrupts, the 60 Hz timer and its PIT stopwatch, keyboard, VGA, `memset` and `memcpy` on string instructions, a PIT delay, the machine probes, the text screen and clean-up. Lives in the separate `dos/` workspace | Exists |
| `fizz-phys` | Rules | Fizz 1's tile map, body, moving platform and rng code as an owned `no_std` copy, with `libm` giving the maths functions method syntax (no `exp`). Function bodies stay identical to Fizz 1's; `tools/lf-sim-drift.sh` reports any drift | Exists |
| `fizz-ep2` | Rules | Episode 2's rules on `fizz-sim` and `fizz-phys`. It holds Ben's movement, the chicken (Fizz 1's Gloop), the Fizz Blaster shots, the red key and door as `WorldFlags`, hazards, death and respawn and the exit, all ported line for line from Fizz 1 and proved against its golden traces; the slice level as `const` data in the shape `fizz-bake` will fill; the state hash; the chicken's seeded cluck stream and the `Event`s the world emits for presenters; and the scripted slice replay with its golden transcript, `EVENT` lines included. Replaced the Milestone 0 `fizz-hello`, which is deleted | Exists |
| `fizz-sfx` | Platform data | Generated OPL2 and PC speaker data for the chicken's cluck (four variants), downmixed from the Undertone source in `assets/sfx/` by `tools/sfx/downmix.ts`, the pure `place` function (a sound's place on screen to a level for each ear, shared by Web and DOS) and allocation-free steppers `OplPlayer` and `OplPair` (560 Hz; the pair is channels 5 and 14 of an OPL3, left and right) and `SpeakerPlayer` (140 Hz). `no_std`, no `unsafe`, no dependencies; hardware access stays with the DOS audio service. See [`SOUND.md`](SOUND.md) | Exists (data and steppers; the Web plays the rich source, and DOS plays nothing yet) |
| `fizz-music` | Platform data | Generated OPL2 and OPL3 register streams (4-byte records in `fizz-sfx`'s `OplStep` layout, with a loop point) for the placeholder level music "Crater Fields", lowered from the Undertone song source in `assets/music/` by `tools/music/generate.ts`. `no_std`, no `unsafe`, no dependencies; the tables, and `MusicPlayer`, the 560 Hz stepper that loops them and keys off only the music channels. See [`MUSIC.md`](MUSIC.md) | Exists (data and stepper, host-tested; the DOS audio service runs it: `MUSIC` line, `/MUSICTEST`, `/BENCH /AUDIO`) |
| `fizz-engine` | Intent | `Game`, scene intent and events | Planned |
| `fizz-bake` | Baking | Host tool: bakes scripts, art and music into per-target Fizz Asset Packs (`.FAP`) | Planned |
| `fizz-fap` | Baking | The Fizz Asset Pack format and loader that `fizz-bake` writes and both platforms read. The name is provisional | Planned |

`InputFrame` first appeared in this plan under `fizz-engine`. It now lives in `fizz-sim` next to `Tick`, because both presenters and the rules need it before any scene intent exists, and it has no dependency on scenes or events. `fizz-engine` keeps `Game` and the scene when they arrive.

The DOS programs (`RTSMOKE.EXE`, `FIZZ2.EXE`) are crates of the `dos/` workspace, which has its own target and build flags and is excluded from the root workspace so host checks never need them.

Names are provisional. Rules that depend on a presenter are a bug.

## What is mirrored from where

- **Fizz 1** (`liminal-hq/lieutenant-fizz`) is the behaviour reference: the pogo physics, the camera (including look up and down and the Web zoom-out map), the enemy kinds, locks and keys, the overworld. When Fizz 2 reproduces a Fizz 1 behaviour, it names the Fizz 1 source and proves the match with a trace test. Fizz 1 stays as it is.
- **Fizz 1's DOS port** (`ScottMorris/lieutenant-fizz-dos`) supplies the 486 performance model and its palette frame technique: 16 colours by 14 brightness levels with a shade pass for lights.
- **Desk** (`liminal-hq/desk`) supplies the hardware abstraction layer shape, the rule that shared crates forbid `unsafe` and never call DOS directly, the no-float-in-graphics rule for presenters, the toolchain and the check scripts. Its ideas are mirrored here now, kept behind `fizz-hal` and `fizz-dos-rt`, so the real Desk crates can be swapped in later.
- **Waypoint and Spindle** supply repository conventions: the README shape, CI, release notes grouped by labels, and the pull request and issue templates.
- **Fizz Level Studio** supplies the level script language and the proof that scripts rebuild Fizz 1's levels exactly. Fizz 2 uses the script as its authoring format and the baker as the single place that turns it into engine data.

## Open questions

- Fizz 1's physics (answered): `fizz-phys` is an owned `no_std` copy of Fizz 1's tile map, body, platform and rng code, proved by golden traces from Fizz 1. The reasons:
  - Fizz 1's `lf-sim` is `std`, so it cannot be a path dependency of the rules crates that also build for DOS and Web.
  - Fizz 2 will diverge from Fizz 1 (new tiles, Episode 2 mechanics), so a vendored copy kept in step by a sync script would fight the changes it exists to allow.
  - An upstream change to make `lf-sim` `no_std` would break the sync check that Fizz 1's DOS port runs against it.
- Can `f64` rules agree on x87, SSE2 and WebAssembly (answered): yes, under the discipline in Numeric model. Native, Web and DOS reproduce the slice golden, `FINAL TICK 1268 HASH 0edebb797e63edff`, and Ben and the chicken match Fizz 1's traces bit for bit. The costs were a pinned software `libm` without the `exp` family, the 53-bit control word and a snap of velocities below 2^-1000 to zero (see [`PLATFORM.md`](PLATFORM.md#numeric-model-decisions)).
- Is the `f64` model affordable on a 486 (partly answered): in DOSBox at about a 486DX2-66 the step costs 103 us of a 16,667 us tick and a whole tick, drawing and copy included, 3,432 us. DOSBox charges the x87 like any simple instruction, so the real cost on a 486 is still unmeasured (see [Performance](PLATFORM.md#performance)).
- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later?
- Which language does the baker use? Rust is preferred so it shares types.
- Do the level script and the studio need extensions for vertical mazes, gadgets and the underworld?
- What base art resolution does Episode 2 author for, given the DOS palette limits? (partly answered) The slice uses Fizz 1's 16 by 24 Ben and 16-pixel tiles in the 16 EGA colours, with the provisional 24 by 32 Ben A drawn against the same physics body but not yet shown. Which Ben ships is still open.
- How much level does the Web view show on a large screen by default: a larger target tile count, a capped scale, or only a zoom control?
- What pack granularity and compression does Episode 2 use: one pack per world or per level, and whether packs are compressed, which trades load time against floppy space?
- How are sprites laid out in a pack: bits per pixel, a mask or a transparent colour, and whether a sprite is stored as runs? (The slice's `const` form is one byte a pixel with 255 as transparent, which a pack may replace; see `SPRITES.md`.)
