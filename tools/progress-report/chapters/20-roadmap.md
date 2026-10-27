---
id: roadmap
n:
kicker: Reference
title: The roadmap
date: 2026-10-27
summary: Every step of Milestones 0 to 5, including Milestone 1.5, with its state, its tag from gh and its evidence.
prs: []
section: reference
---
This page lists the plan by milestone. Milestone 0 is merged and Milestone 1 is mostly merged. Milestone 1.5 sits between Milestones 1 and 2 so that the DOS machine, settings and audio work stays apart from the music of Milestone 4 and the budgets of Milestone 5. Milestones 2 to 5 follow the restart plan in `docs/POC-1-FINDINGS.md`. Each step's state and tag are recomputed from `gh` and `git` whenever the site is built.

<!-- figure milestones -->

<!-- figure nexts -->

## The milestones

- **M0, the platform skeleton.** A marker moved by input at 60 Hz on Web and DOS, hashed identically on native, Web and DOS (chapters 3 to 6).
- **M1, the first playable slice.** It reused Fizz 1's physics, proved by golden traces from Fizz 1's source, and covers the slice on Web and DOS: `fizz-phys`, the golden-trace tool, the key layouts, the sprite pipeline, Ben's port, the slice world and its replay, the view helpers and the slice drawing on both presenters, the retirement of `fizz-hello`, the chicken's cluck as a deterministic event, the cluck source with four variants, Web audio with Undertone, `/BENCH` and the drawing speed-up, the chicken's beak and captions, look and aim, and a documentation phase.
- **M1.5, machine, settings and DOS audio.** `fizz-machine` (the start-up check and the settings model) is merged; the DOS probes and `/PROBE`, the `/CHECK` screen, the sound settings in `FIZZ2.CFG` version 2, the `/?` help screen, DOS audio on the 560 Hz timer, OPL3 and the panned channel 14, the frame-rate policy, the live HUD and an Options screen are planned. The milestone is done when the cluck is audible on DOS, detection and settings work, and the frame-rate policy is measured.
- **M2, the content pipeline.** `fizz-bake`, the host tool that bakes authored assets for each target into Fizz Asset Packs (`.FAP`), starting with Fizz Level Studio scripts as the level format. DOS loads a pack with one read and parses nothing that the baker could prepare.
- **M3, scene intent and the camera.** `fizz-engine` with scene intent (tiles, sprites, lights, camera) and named events, following `docs/SCENE-CONTRACT.md`, and a camera with Fizz 1's pixel-locking: Fizz 1's `docs/ENGINE_SPEC.md` snaps the camera to the canvas-pixel grid every frame so sprites never shimmer.
- **M4, audio.** The music from the IMF lab's composition source, lowered to OPL2 and OPL3 arrangements for DOS and played on Web, built on the detection and routing of M1.5. The effects pipeline already exists.
- **M5, CI gates and the 486 budget.** The CI cycle gate and the size budgets, built on `/BENCH` and its calibration, against a 486DX model in CI, since DOSBox at `cycles=max` hides x87 cost, and a first run on real hardware.

<!-- figure roadmap-table -->

## The shelves

### Catalogued
- Every Milestone 0 step is merged with CI green on `main`, and so is each merged step of Milestones 1 and 1.5 that names its pull requests; the generator would move a step down a shelf, and print a warning, if that stopped being true.

### Uncatalogued
- The order and scope of Milestones 1 to 5, including 1.5: the working plan, not yet a document in `docs/`.

### Wish list
- Every planned step of Milestones 1 to 5.

```figures
id: milestones
kind: chart
track: shared
title: Progress by milestone
alt: Stacked bar chart of the steps in each milestone by state, in the order M0, M1, M1.5, M2, M3, M4 and M5: Milestone 0 has eight catalogued steps, Milestone 1 mostly catalogued steps with some in review, Milestone 1.5 one catalogued step and nine planned, and Milestones 2 to 5 planned steps and one idea.
caption: The steps of each milestone, by state.
at: figure milestones
thumb: svg
```
