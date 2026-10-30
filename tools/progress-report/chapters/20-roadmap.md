---
id: roadmap
n:
kicker: Reference
title: The roadmap
date: 2026-10-30
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
- **M1.5, machine, settings and DOS audio.** `fizz-machine`, the DOS probes and `/PROBE`, the `/CHECK` screen, the sound settings in `FIZZ2.CFG` version 2, the `/?` help screen, DOS audio on the 560 Hz timer with OPL2 and the PC speaker, and the OPL3 channel pair with positional sound (channel 5 for the left ear and channel 14 for the right, each at a level from Fizz 1's pan and distance formula, #74) are built and checked in DOSBox. The rest of the milestone landed on 28 October 2026: the frame-rate policy (`/FPS30`, `/FPS60` and `/FPSAUTO`, #84), the first HUD without `TICK` as a translucent pill on DOS and a panel on the Web (#83, #86), the Fizz colour, grape (#87), the grape can in the menus (#116), `FIZZ2.CFG` version 3 and its crash-safe save (#88, #89), live switching of the music and effects devices (#90), and the Paused and Options screens on both platforms with the Motion setting and the Classic or Enhanced choice for sound placement (#92 to #95). The Two-Row Stack HUD (#122) put the score, the deaths and a tray of key sockets on both screens, the BIOS tick count of a DOS run survives the midnight wrap (#117), and the music takes a screen mix under Paused and Options on the Web (#124) and, approximated with operator levels, on DOS (#125; the Web side is a 900 Hz low-pass at 0.7 gain). Still planned are the start-up check screen that runs before play (`/CHECK` is the opt-in version) and the HUD's toast line and level-name banner. Not verified: the cluck and the music on real sound hardware, the menu mix by ear, the floppy save time, the key repeat in a menu, the frame costs on a real 486 and a real browser; the owner's playtest on 28 October 2026 heard the Web cluck and music. It runs in parallel with the music of Milestone 4.
- **M2, the content pipeline, started ([chapter 11](chapter:pipeline)).** The Fizz Asset Pack format is specified in `docs/FAP.md` (#109), the `no_std` `fizz-pack` reader with its deflate decoder is merged (#111, #112), and OPX, the open register-stream format that audio sections hold, is specified in `docs/OPX.md` at version 0.1 with no reader or writer yet (#113). `fizz-bake` is planned, with level scripts as the level format; its next steps in order are the level source refactor, the level script dialect and `fizz-bake` itself, the pack loaders on both platforms, then the audio and sprite sections and the tools for floppy images. DOS loads a pack with one read and parses nothing that the baker could prepare. The enemy roster with its animation states, animated tiles and backdrop art belong here. Four animation ideas are new Episode 2 design and not ports from Fizz 1, each needing new sprites and a view-only implementation: idle blinks and orbiting dizzy stars, which the owner wants, and run dust and a landing squash on normal jumps, which are proposed and can be struck.
- **M3, scene intent and the camera.** `fizz-engine` with scene intent (tiles, sprites, lights, camera) and named events, following `docs/SCENE-CONTRACT.md`, and a camera with Fizz 1's pixel-locking: Fizz 1's `docs/ENGINE_SPEC.md` snaps the camera to the canvas-pixel grid every frame so sprites never shimmer. Fizz 1's camera constants and Tower mode, shake for the remaining events, point lights with a DOS 16-colour shade pass, night profiles and parallax layers are planned here. The first point light is Fizz 1's shot glow, a light on each shot (`render.rs` lines 1349 to 1356 of Fizz 1's Episode 1 game), followed by the exit and crystal glows and Ben's lantern; lights wait for this milestone. One constraint governs the engine: the viewport's size reaches the view and the camera code as view info and never reaches the rules, which wake and activate on fixed tile distances. It landed for the Web in #60.
- **M3.5, mechanics roster (placeholder).** Ladders and vines, mantle, wall-kick (first, because the lighthouse is vertical), lifts, switches and mirrors, cracked walls, the full enemy AI set and the boss. Score, snacks and lives wait for an Episode 2 decision. Nothing is started.
- **M4, audio, merged (#99 to #108).** Built on the detection and routing of M1.5. Undertone moved to 0.3.0 (#99), the rules raise sound and caption events (#100), the slice has a snack nook where each event happens (#102), Fizz 1's whole caption table shows on both platforms (#103), effects play by id (#104), eleven new effects are authored in Undertone with FM voices and played for every event, with a "Hear an effect" list on the Web (#105), the soda can's fsssht is the nineteenth effect (#127), the OPL3 pair prints as `FX ch5+14` in the `SOUND` line (#106), and the leftover audio ideas moved to M4.5 (#107, #108). The cluck is placed in the stereo field. The level music is "Crater Fields", Fizz 1's track used as a placeholder test song and not an official Fizz 2 track: the song source (#73, {{pr73.state}}), the Web player (#75, {{pr75.state}}), the lowering to OPL2 and OPL3 (#76, {{pr76.state}}), the `fizz-music` stepper (#79, {{pr79.state}}) and the DOS player (#81, {{pr81.state}}) exist, with the music documents. Separately authored songs stay as the later replacement. The owner played Milestone 4 on 29 October 2026: they listened to the new effects on the Web and found them good, and said it looks good in gameplay on the Web and in the DOS player. After listening on both platforms the owner chose Fizz 1's own jump, pogo and fizz shot (#126) ("the FM versions of these render weird in DOS and they sound great in Fizz 1 DOS"), so DOS plays Fizz 1 DOS's data for them and the FM versions stay as alternates. Nobody has reported listening to the menu mix or the can's sound, and hearing the effects on real hardware is still open.
- **M4.5, song sources and OPL tools.** The home of what M4 left out: separately authored songs, stereo-authored effects (the OPL3 pair already takes one record stream for each side), the proper FM-to-OPL downmix, and the review of the companion OPL package. It holds three ideas to measure, not commitments: one OPL3 stream whose second bank OPL2 skips (the OPL3 core-and-extras idea), compact-then-compress music streams that DOS decodes only for the detected device, OPX adoption with the `oplstream` crate, an audit of redundant register writes (measured on 29 October 2026 in a scratch experiment, report not committed: 26.3% of a converter's writes for the nine candidate songs change nothing and all 18 of its streams break the 16-writes-per-tick cap, against 1.1% and 0.2% for Crater Fields). and Fizz 1 DOS's data for the other eight effects (stun, clunk, crunch and bonk differ in character from Fizz 1 DOS's, hurt, win, key and plink are close; for the owner to decide). Nothing in it is started.
- **M5, CI gates and the 486 budget.** The CI cycle gate and the size budgets, built on `/BENCH` and its calibration, against a 486DX model in CI, since DOSBox at `cycles=max` hides x87 cost, and a first run on real hardware. The DOS check runs in named groups since #123: a pull request runs the groups its paths can affect and `main` and the nightly schedule run the full gate (chapter 11 has the durations); the runtime symbol warnings were cleared in #119.
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
alt: Stacked bar chart of the steps in each milestone by state, in the order M0, M1, M1.5, M2, M3, M3.5, M4, M5 and M6: Milestone 0 has eight steps done, Milestone 1 mostly steps done with the documentation phase in progress, Milestone 1.5 mostly steps done in an emulator with the start-up check screen planned, Milestone 4 steps done in an emulator, Milestone 4.5 planned steps and ideas, and the other later milestones planned steps.
caption: The steps of each milestone, by state.
at: figure milestones
thumb: svg
```
