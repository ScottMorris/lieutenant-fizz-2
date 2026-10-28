---
id: roadmap
n:
kicker: Reference
title: The roadmap
date: 2026-10-27
summary: Every step of Milestones 0 to 6, including Milestones 1.5 and 3.5, with its state, its tag from gh and its evidence.
prs: []
section: reference
---
This page lists the plan by milestone: Milestone 0 is the platform skeleton, Milestone 1 the first playable slice. Milestone 1.5 sits between Milestones 1 and 2 so that the DOS machine, settings and audio work stays apart from the music of Milestone 4 and the budgets of Milestone 5. Milestones 2 to 5 follow the restart plan in `docs/POC-1-FINDINGS.md`. Milestones 3.5 and 6 are placeholders with no start date. Each step's state and tag are recomputed from `gh` and `git` whenever the site is built.

<!-- figure milestones -->

<!-- figure nexts -->

## The milestones

- **M0, the platform skeleton.** A marker moved by input at 60 Hz on Web and DOS, hashed identically on native, Web and DOS (chapters 3 to 6). Its rules, `fizz-hello`, are deleted.
- **M1, the first playable slice.** Fizz 1's physics as `fizz-phys`, proved by eighteen golden traces from Fizz 1's source; Ben, the chicken, the key and the door in `fizz-ep2`; the slice level and its replay; one slice drawing for both presenters, with the Web view fitted to the window; the retirement of `fizz-hello`; the chicken's cluck as a deterministic event with four variants, its source, Web audio with Undertone, its beak and captions; `/BENCH` and the drawing speed-up; look and aim; and the view-only effects of Fizz 1's survey (stun stars, the shot-end puff, the death shake and the death spin). The Web view follows the window (#58, #60). The documentation phase, which includes this archive's refresh, closes it ([chapters 8 and 9](chapter:slice)).
- **M1.5, machine, settings and DOS audio.** `fizz-machine`, the DOS probes and `/PROBE`, the `/CHECK` screen, the sound settings in `FIZZ2.CFG` version 2, the `/?` help screen, DOS audio on the 560 Hz timer with OPL2 and the PC speaker, and the OPL3 channel pair with positional sound (channel 5 for the left ear and channel 14 for the right, each at a level from Fizz 1's pan and distance formula, #74) are built and checked in DOSBox. The rest of the milestone landed on 28 October 2026: the frame-rate policy (`/FPS30`, `/FPS60` and `/FPSAUTO`, #84), the HUD without `TICK` as a translucent pill on DOS and a panel on the Web (#83, #86), the Fizz colour, grape (#87), `FIZZ2.CFG` version 3 and its crash-safe save (#88, #89), live switching of the music and effects devices (#90), and the Paused and Options screens on both platforms with the Motion setting and the Classic or Enhanced choice for sound placement (#92 to #95). Still planned are the start-up check screen that runs before play (`/CHECK` is the opt-in version) and the HUD's toast line and level-name banner. Not verified: the cluck and the music on real sound hardware, the floppy save time, the key repeat in a menu, the frame costs on a real 486 and a real browser; the owner's playtest on 28 October 2026 heard the Web cluck and music. It runs in parallel with the music of Milestone 4.
- **M2, the content pipeline.** `fizz-bake`, the host tool that bakes authored assets for each target into Fizz Asset Packs (`.FAP`), starting with Fizz Level Studio scripts as the level format. DOS loads a pack with one read and parses nothing that the baker could prepare. The enemy roster with its animation states, animated tiles and backdrop art belong here. Four animation ideas are new Episode 2 design and not ports from Fizz 1, each needing new sprites and a view-only implementation: idle blinks and orbiting dizzy stars, which the owner wants, and run dust and a landing squash on normal jumps, which are proposed and can be struck.
- **M3, scene intent and the camera.** `fizz-engine` with scene intent (tiles, sprites, lights, camera) and named events, following `docs/SCENE-CONTRACT.md`, and a camera with Fizz 1's pixel-locking: Fizz 1's `docs/ENGINE_SPEC.md` snaps the camera to the canvas-pixel grid every frame so sprites never shimmer. Fizz 1's camera constants and Tower mode, shake for the remaining events, point lights with a DOS 16-colour shade pass, night profiles and parallax layers are planned here. The first point light is Fizz 1's shot glow, a light on each shot (`render.rs` lines 1349 to 1356 of Fizz 1's Episode 1 game), followed by the exit and crystal glows and Ben's lantern; lights wait for this milestone. One constraint governs the engine: the viewport's size reaches the view and the camera code as view info and never reaches the rules, which wake and activate on fixed tile distances. It landed for the Web in #60.
- **M3.5, mechanics roster (placeholder).** Ladders and vines, mantle, wall-kick (first, because the lighthouse is vertical), lifts, switches and mirrors, cracked walls, the full enemy AI set and the boss. Score, snacks and lives wait for an Episode 2 decision. Nothing is started.
- **M4, audio.** Built on the detection and routing of M1.5, and running in parallel with the end of it. The effects pipeline exists, with the cluck placed in the stereo field. The level music is "Crater Fields", Fizz 1's track used as a placeholder test song and not an official Fizz 2 track: the song source (#73, {{pr73.state}}), the Web player (#75, {{pr75.state}}), the lowering to OPL2 and OPL3 (#76, {{pr76.state}}), the `fizz-music` stepper (#79, {{pr79.state}}) and the DOS player (#81, {{pr81.state}}) exist, with the music documents. The IMF lab's songs stay as the later replacement. Also planned: Fizz 1's other sound effects with the `whoa!` caption and the rest of the caption table, stereo-authored effects (the OPL3 pair already takes one record stream for each side). Undertone FM: pending the owner's Undertone FM work.
- **M5, CI gates and the 486 budget.** The CI cycle gate and the size budgets, built on `/BENCH` and its calibration, against a 486DX model in CI, since DOSBox at `cycles=max` hides x87 cost, and a first run on real hardware.
- **M6, shell (placeholder).** The title, pause, options, four save slots, result cards and the opening cinematic; haptics and a debug level select come later. Nothing is started.

Open owner questions, not decided: whether Ben gets invulnerability after a hit (Fizz 1 never sets it), a lower-case 8x8 font for toasts, random sparks for the boss, and a night version of the lighthouse.

<!-- figure roadmap-table -->

## The shelves

### Catalogued
- A step marked done keeps that state only while every pull request it names is merged with CI green on `main`; the generator moves a step down a shelf, and prints a warning, if that stops being true. Every state on this page comes from that check and the tags from `gh`.

### Uncatalogued
- The order and scope of Milestones 1 to 6, including 1.5 and 3.5: the working plan, not yet a document in `docs/`.

### Wish list
- Every planned step of Milestones 1 to 6.

```figures
id: milestones
kind: chart
track: shared
title: Progress by milestone
alt: Stacked bar chart of the steps in each milestone by state, in the order M0, M1, M1.5, M2, M3, M3.5, M4, M5 and M6: Milestone 0 has eight steps done, Milestone 1 mostly steps done with the documentation phase in progress, Milestone 1.5 mostly steps done in an emulator with the Options screen and the HUD work planned, and the later milestones planned steps and one idea.
caption: The steps of each milestone, by state.
at: figure milestones
thumb: svg
```
