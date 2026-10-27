# Proof of concept 1 findings

## Status

The `feat/adlib-music` work is a proof of concept for structure. It shows that one deterministic `no_std` simulation can drive several presenters, with a fixed 60 Hz tick, press-edge input, named sound effects, build-time score precompilation and an IRQ-driven OPL2 service on DOS. It is not yet a game engine. This repository is the rebuild: it starts clean and brings pieces in against the principles below. The proof of concept lives in `fizz-2-dos-poc1` at tag `poc-1`.

## What the proof of concept settled

- The crate split holds: `fizz2-core` (rules), `fizz2-engine` (contract), a renderer, a DOS runtime, and thin presenters.
- A fixed tick with explicit press edges fixes the dropped-sound and diagonal-input bugs that variable timing caused.
- Precompiling data at build time keeps DOS free of parsing.
- Effects are semantic names; each presenter plays them in its own engine. DOS is last-wins on channel 5; Web is simultaneous.

## What it did not settle

- The shared `Scene` is Mode 13h in a trait: a `Rgb6` palette, `u8` palette indices and `Clear`, `Rect` and `Stipple`. The lighthouse beam is stacked rectangles, so a richer presenter has no intent to work from. The Web preview is a canvas blit of the same indexed frame, so there is no second renderer yet. See [SCENE-CONTRACT.md](SCENE-CONTRACT.md).
- The score library starts from OPL2 limits and has the Web presenter read them back: `fnum` and `block` words become hertz, and Undertone waveforms are chosen per channel. It uses a fixed 60-row, 560-tick grid, three source formats, and a 1,935-line crate holding instruments, grammar, compiler and IMF reader. It cannot express tempo changes, long phrases or layered parts. Its reusable parts are the instrument definitions, `pitch_to_fnum_block`, the IMF reader and exporter, and the DOS music service.
- Nothing has been tested with real content: no tilemap, sprites, collision, levels or saves.

## Principles

1. **Intent flows up, lowering flows down.** The core emits semantic intent at full fidelity. Each presenter projects it down. The shared contract is never shaped like the weakest target.
2. **Fidelity is lost at cook time.** Art and music are authored for the richest target. Offline host tools produce the DOS forms (indexed sprites with shade levels, IMF or OPL2 patches). DOS plays pre-cooked data and parses nothing.
3. **Platform limits live in the platform.** `Rgb6`, palette indices, OPL patch ids, slot counts and `MAX_*` capacities belong to the presenter or backend that needs them, not to the engine contract.
4. **`no_std` plus `alloc` for the rules and engine; `std` elsewhere.** Only interrupt handlers need a no-allocation discipline. Fixed-capacity arrays are an IRQ concern, not an engine rule. `forbid(unsafe_code)` stays on the engine.
5. **A presenter may add what the core does not know about.** The Web zoom-out level map and the multi-voice Undertone mix are presenter features. DOS can omit them.
6. **DOS is a bonus, not a ceiling.** The floor is a 486DX. DOS may cut content or features. It does not limit the design.
7. **Second-consumer rule.** A piece is not done until a second consumer exercises it.

## Numeric model

Fizz 1's smooth pogo physics are continuous and are the product, so the rules keep Fizz 1's numeric model: floating point, with the determinism discipline Fizz 1 already uses (basic IEEE operations only, no reordering, a software `libm` for trig, x87 at 53-bit precision on DOS).

Fixed-point rules with a floating-point presentation layer were considered and set aside, because continuous physics would need re-tuning. Fixed-point stays the alternative if a later episode drops continuous physics.

- Determinism matters for state that feeds back into the rules. It does not matter for display-only motion (particles, lighting, interpolation, camera easing).
- The x87 FPU is on the 486DX. A 486SX has none, so soft-float is out of scope.
- Measure before deciding: Fizz 1 provides `m5/bench` and `cyc486.py` for DOS cycle estimates. DOSBox at fixed cycles hides x87 cost.

## Layering

```text
CONTENT     episode data and rules, no platform types
              step(InputFrame) at a fixed 60 Hz
INTENT      scene: tilemap, sprites, lights, camera
            audio: composition source, named cues
            events: ExplorerStep, BuoyBell, ...
COOKING     offline host tools: assets lowered per target
PRESENTERS  Web (canvas or WebGL, Undertone), DOS (320x200, OPL2)
PLATFORM    clock, input, storage, display, audio services
              Web: JS shim   DOS: Desk's HAL and dos-rt
```

- The DOS platform layer is Desk's HAL and `dos-rt`, not a separate `fizz2-dos-rt`. Fizz 2 is its first game consumer. The IRQ music service and the headless DOSBox transcript checks are candidates to move down into it.
- Fizz 1's frame model is the right intent level: sprites and lights, lowered on DOS by a palette trick (16 colours by 14 brightness levels) and an 8x8 light shade pass. On Web the same intent can use real lighting.
- Tauri wraps a webview, so Linux, Windows, macOS and Android can share the Web presenter. There are two presenters in practice, Web and DOS. Whether the engine runs as WASM in the webview or as native Rust in the Tauri backend is open.

## Episode 2 content ideas

- **Worlds:** an overworld, an inner world and an underworld. The world is a graph of data-driven levels with transitions.
- **Vertical level design:** mazes and puzzles that climb and descend, with chained key and door progression. Fizz 1 already has this in miniature (Zarg Lookout, a nine-floor watchtower with a red key and a cookie door), plus overworld regions, teleporter pairs, cracked walls, conveyors and presses.
- **Gadgets:** helpers beyond the pogo stick, as a data-driven tool model with a small state machine, rules and sound cues for each. This shapes `InputFrame` (more than four actions) and the entity model.
- **Sprite detail:** richer art, authored above the DOS palette limit and quantized by the cooker. The base art resolution needs to be settled early.
- **A chicken enemy** in the style of Zargoth's characters (crystals, chocolate, cake, mechanical hover platforms). Fizz 1's bestiary mixes mechanical and confectionery designs. Episode 2 is the Zarg raid on Earth's chocolate, so a Zargified Earth animal fits. Behaviour belongs in the sim's enemy terms (patroller, chaser, teleporter, gunner).
- **Camera:** follow, look-ahead and look up and down are one deterministic shared module, so both platforms frame the same scene. The Web zoom-out whole-level view is a presenter feature.

## Progression model

- Keys, doors, switches and teleporters are persistent world flags in the rules, saved with position.
- A generic lock-and-key model (colour or id pairs) covers chained puzzles without per-puzzle code.
- Pogo jump height and fall speed set what a maze can demand.
- A deterministic sim allows solvability checks in CI: a recorded or scripted input run proves a level can be completed against the cooked data.

## Level tooling

The editor is Fizz Level Studio, in `~/source/experiments/fizz-level-studio/` (specification in `SPEC.md`, with `docs/` for goals, architecture, editor, MCP, overworld, engine changes, testing, roadmap, decisions and the language reference). It has a builder language, a Rust exporter for Fizz 1's `Builder` calls, a nine-tool MCP server and an interactive design prototype. The studio application itself is not built.

- Every level and overworld is a short text script. Painting, dragging, typing and accepted Claude proposals all edit the script.
- The scripts for all 16 Episode 1 levels and the overworld rebuild the game's data exactly, including each level's fingerprint (`scripts/prove-all.sh`).
- Known gaps in the Fizz 1 engine that the studio audit found: the progress mask `MapPoint::req` is a `u16` with every bit used, the game builds one overworld, the exporter does not register levels in the `LEVELS` table, and `OverworldBuilder` exists only in the proof copy.
- For Fizz 2, the script is the authoring format and the cooker is the single place that turns it into engine data. A data export from the same language, with Rust statics generated for DOS, avoids a compile step in the editing loop. The Fizz 1 Rust export stays for Fizz 1. Whether Fizz 2 adopts the language as is or extends it is open.

## Music direction

- The source of truth is an Undertone-style composition that is not limited by AdLib.
- A build step lowers it to IMF or OPL2 for DOS through a per-song instrument mapping. IMF is the portable DOS target and the IMF lab is an authoring and comparison tool.
- Web plays the source directly. Presentation extras (gain, pan, extra lanes) are a Web-only sidecar until a second consumer exists.
- Channel 5 stays reserved for game effects on DOS.

## Restart plan

1. Tag the proof of concept (`poc-1`) and start the rebuild from a clean tree.
2. A DOS skeleton on Desk's HAL with a ticking loop.
3. The simulation with the numeric model from above and a per-tick state hash that CI compares across targets.
4. Scene intent with two real presenters, starting with one vertical slice: a small vertical maze with a keycard and door, Fizz 1's pogo physics, a chicken, and two-axis scrolling, shown in Web first and then DOS.
5. The cooker and the audio path.
6. Tauri integration.

## Open questions

- Is Fizz 1's pogo physics code reused directly, or ported with the same constants?
- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later? (Fizz 1 stays as it is.)
- Which language does the cooker use? Rust is preferred so it shares types.
- Do the Level Studio scripts need extensions for the Fizz 2 content (vertical mazes, gadgets, depth)?
- What base art resolution does Episode 2 author for?

## References

- Codex session `01a1765b-cb7b-7f03-95e3-c62b7c77a6c9`: design and most of the proof of concept.
- Antigravity session `61849e15-1516-47bf-9a1d-2453ce5053c3`: the OPL2 music service and the IMF lab channel naming.
- Claude session `66cb2a84-3d9f-4325-86a7-130f63f99346` in the `lieutenant-fizz` project: Fizz Level Studio, its design canvas (`https://claude.ai/artifact/E43evy173R6mC7bAeN5FAF`) and the spec.
- Fizz 1: `~/source/liminal-hq/lieutenant-fizz` (`docs/GAME_DESIGN.md`, `docs/STORY.md`, `docs/SERIES.md`) and `~/source/experiments/fizz-dos`.
- Desk: `~/source/liminal-hq/desk`.
- [SCENE-CONTRACT.md](SCENE-CONTRACT.md) here, and `docs/SCORE-EXTENSIONS.md` in the proof of concept.
