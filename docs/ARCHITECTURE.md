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
- CI gates both from the start. Host checks run everywhere; the DOS build and a headless DOSBox transcript join when the DOS platform layer arrives.
- Cooked assets have per-target profiles. A missing DOS cook falls back to a placeholder so Web iteration is never blocked, and the DOS gate still reports the gap.

The cost of parallel development is the DOS toolchain and its slower feedback. Keep a quick, host-only `tools/check.sh` for the inner loop and run the full DOS gate in CI and before a PR.

## Numeric model

Fizz 1's physics are continuous and tuned by feel, so Fizz 2 keeps Fizz 1's numeric model: `f64` state stepped at a fixed 60 Hz, under a determinism discipline.

- Basic IEEE operations only, and no reordering of floating-point operations for speed.
- A software `libm` for transcendental functions, so results do not depend on the platform's maths library.
- x87 set to 53-bit precision on DOS before the first tick, so 80-bit intermediates do not leak into results.
- Display-only motion (particles, lighting, interpolation, camera easing) may use any numeric form because it never feeds back into the rules.

Fixed-point rules remain the alternative if a later episode drops continuous physics. The 486DX has an on-chip FPU. A 486SX would need software floating point and is out of scope. Measure before changing the model: Fizz 1's `m5/bench` and its 486 cycle estimator are the reference tools, and DOSBox at fixed cycles hides x87 cost.

## Crate plan

Crates arrive with their first consumer and test. Only `fizz-sim` exists today.

| Crate | Layer | Purpose | State |
| --- | --- | --- | --- |
| `fizz-sim` | Rules | Fixed tick, state hash, and later the shared rules foundation | Exists |
| `fizz-hal` | Platform | Hardware abstraction traits mirrored from Desk (clock, input, storage, display, audio) | Planned |
| `fizz-engine` | Intent | `Game`, `InputFrame`, scene intent and events | Planned |
| `fizz-cook` | Cooking | Host tool: scripts to level data, art and music to per-target assets | Planned |
| `fizz-present-web` | Presenter | Web presentation, WASM exports and the canvas or WebGL layer | Planned |
| `fizz-present-dos` | Presenter | DOS presentation: mode 13h, IRQ input, OPL2 | Planned |

Names are provisional. Rules that depend on a presenter are a bug.

## What is mirrored from where

- **Fizz 1** (`liminal-hq/lieutenant-fizz`) is the behaviour reference: the pogo physics, the camera (including look up and down and the Web zoom-out map), the enemy kinds, locks and keys, the overworld. When Fizz 2 reproduces a Fizz 1 behaviour, it names the Fizz 1 source and proves the match with a trace test. Fizz 1 stays as it is.
- **Fizz 1's DOS port** (`ScottMorris/lieutenant-fizz-dos`) supplies the 486 performance model and its palette frame technique: 16 colours by 14 brightness levels with a shade pass for lights.
- **Desk** (`liminal-hq/desk`) supplies the hardware abstraction layer shape, the rule that shared crates forbid `unsafe` and never call DOS directly, the no-float-in-graphics rule for presenters, the toolchain and the check scripts. Its ideas are mirrored here now, kept behind `fizz-hal`, so the real Desk crates can be swapped in later.
- **Waypoint and Spindle** supply repository conventions: the README shape, CI, release notes grouped by labels, and the pull request and issue templates.
- **Fizz Level Studio** supplies the level script language and the proof that scripts rebuild Fizz 1's levels exactly. Fizz 2 uses the script as its authoring format and the cooker as the single place that turns it into engine data.

## Open questions

- Is Fizz 1's pogo physics reused directly or ported with the same constants and proved by golden traces?
- Does the engine crate stay Fizz 2's own, or become something Fizz 1 could adopt later?
- Which language does the cooker use? Rust is preferred so it shares types.
- Do the level script and the studio need extensions for vertical mazes, gadgets and the underworld?
- What base art resolution does Episode 2 author for, given the DOS palette limits?
