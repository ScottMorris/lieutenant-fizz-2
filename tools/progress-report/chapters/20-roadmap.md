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
This page lists the plan by milestone. Milestone 0 is merged and Milestone 1 is mostly merged. Milestone 1.5 sits between Milestones 1 and 2 so that the DOS machine, settings and audio work stays apart from the music of Milestone 4 and the budgets of Milestone 5. Milestones 2 to 5 follow the restart plan in `docs/POC-1-FINDINGS.md`. Milestones 3.5 and 6 are placeholders with no start date. Each step's state and tag are recomputed from `gh` and `git` whenever the site is built.

<!-- figure milestones -->

<!-- figure nexts -->

## The milestones

- **M0, the platform skeleton.** A marker moved by input at 60 Hz on Web and DOS, hashed identically on native, Web and DOS (chapters 3 to 6).
- **M1, the first playable slice.** The polish still to come is view only: stun stars, the shot-end puff, the death shake, a Web canvas that fills the page, and the Web caption overlay (#54) and follow-source fix. It reused Fizz 1's physics, proved by golden traces from Fizz 1's source, and covers the slice on Web and DOS: `fizz-phys`, the golden-trace tool, the key layouts, the sprite pipeline, Ben's port, the slice world and its replay, the view helpers and the slice drawing on both presenters, the retirement of `fizz-hello`, the chicken's cluck as a deterministic event, the cluck source with four variants, Web audio with Undertone, `/BENCH` and the drawing speed-up, the chicken's beak and captions, look and aim, and a documentation phase.
- **M1.5, machine, settings and DOS audio.** `fizz-machine` (the start-up check and the settings model) is merged; the DOS probes and `/PROBE`, the `/CHECK` screen, the sound settings in `FIZZ2.CFG` version 2, the `/?` help screen, DOS audio on the 560 Hz timer, OPL3 and the panned channel 14, the frame-rate policy, the live HUD, the HUD extras (ammo in red at zero, a key chip, a toast line and a level-name banner) and an Options screen with a Motion setting (Reduced Motion, which turns screen shake off) are planned. The milestone is done when the cluck is audible on DOS, detection and settings work, and the frame-rate policy is measured.
- **M2, the content pipeline.** `fizz-bake`, the host tool that bakes authored assets for each target into Fizz Asset Packs (`.FAP`), starting with Fizz Level Studio scripts as the level format. DOS loads a pack with one read and parses nothing that the baker could prepare. The enemy roster with its animation states, animated tiles and backdrop art belong here. Four animation ideas are new Episode 2 design and not ports from Fizz 1, each needing new sprites and a view-only implementation: idle blinks and orbiting dizzy stars, which the owner wants, and run dust and a landing squash on normal jumps, which are proposed and can be struck.
- **M3, scene intent and the camera.** `fizz-engine` with scene intent (tiles, sprites, lights, camera) and named events, following `docs/SCENE-CONTRACT.md`, and a camera with Fizz 1's pixel-locking: Fizz 1's `docs/ENGINE_SPEC.md` snaps the camera to the canvas-pixel grid every frame so sprites never shimmer. Fizz 1's camera constants and Tower mode, shake for the remaining events, point lights with a DOS 16-colour shade pass, night profiles and parallax layers are planned here.
- **M3.5, mechanics roster (placeholder).** Ladders and vines, mantle, wall-kick (first, because the lighthouse is vertical), lifts, switches and mirrors, cracked walls, the full enemy AI set and the boss. Score, snacks and lives wait for an Episode 2 decision. Nothing is started.
- **M4, audio.** The music from the IMF lab's composition source, lowered to OPL2 and OPL3 arrangements for DOS and played on Web, built on the detection and routing of M1.5. The effects pipeline already exists.
- **M5, CI gates and the 486 budget.** The CI cycle gate and the size budgets, built on `/BENCH` and its calibration, against a 486DX model in CI, since DOSBox at `cycles=max` hides x87 cost, and a first run on real hardware.
- **M6, shell (placeholder).** The title, pause, options, four save slots, result cards and the opening cinematic; haptics and a debug level select come later. Nothing is started.

Open owner questions, not decided: whether Ben gets invulnerability after a hit (Fizz 1 never sets it), a lower-case 8x8 font for toasts, random sparks for the boss, and a night version of the lighthouse.

<!-- figure roadmap-table -->

## The shelves

### Catalogued
- Every Milestone 0 step is merged with CI green on `main`, and so is each merged step of Milestones 1 and 1.5 that names its pull requests; the generator would move a step down a shelf, and print a warning, if that stopped being true.

### Uncatalogued
- The order and scope of Milestones 1 to 6, including 1.5 and 3.5: the working plan, not yet a document in `docs/`.

### Wish list
- Every planned step of Milestones 1 to 6.

```figures
id: milestones
kind: chart
track: shared
title: Progress by milestone
alt: Stacked bar chart of the steps in each milestone by state, in the order M0, M1, M1.5, M2, M3, M3.5, M4, M5 and M6: Milestone 0 has eight catalogued steps, Milestone 1 mostly catalogued steps with some in review, Milestone 1.5 one catalogued step and nine planned, and the later milestones planned steps and one idea.
caption: The steps of each milestone, by state.
at: figure milestones
thumb: svg
```
