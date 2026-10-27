---
id: roadmap
n:
kicker: Reference
title: The roadmap
date: 2026-10-27
summary: Every step of Milestones 0 to 5 with its state, its tag from gh and its evidence.
prs: []
section: reference
---
This page lists the plan by milestone. Milestone 0 is merged; Milestones 1 to 5 are the working plan, in the order of the restart plan in `docs/POC-1-FINDINGS.md`, and none of their steps has a pull request yet. Each step's state and tag are recomputed from `gh` and `git` whenever the site is built.

<!-- figure milestones -->

<!-- figure nexts -->

## The milestones

- **M0, the platform skeleton.** A marker moved by input at 60 Hz on Web and DOS, hashed identically on native, Web and DOS (chapters 3 to 6).
- **M1, reuse `lf-sim`.** Fizz 1's engine core, `lf-sim` in `crates/sim`, is in its own words a "deterministic fixed-step tile/AABB simulation, instance buffer and lights". It is either reused directly or ported with the same constants, and a golden trace from Fizz 1's source proves the match on all three platforms. `fizz-hello` is deleted when the real rules hash identically.
- **M2, the content pipeline.** `fizz-bake`, the host tool that bakes authored assets for each target into Fizz Asset Packs (`.FAP`), starting with Fizz Level Studio scripts as the level format. DOS loads a pack with one read and parses nothing that the baker could prepare.
- **M3, scene intent and the camera.** `fizz-engine` with scene intent (tiles, sprites, lights, camera) and named events, following `docs/SCENE-CONTRACT.md`, and a camera with Fizz 1's pixel-locking: Fizz 1's `docs/ENGINE_SPEC.md` snaps the camera to the canvas-pixel grid every frame so sprites never shimmer.
- **M4, audio.** The music from the IMF lab's composition source, played on Web and lowered to IMF or OPL2 for DOS, with channel 5 kept for effects.
- **M5, CI gates and the 486 budget.** Cycle and size budgets against a 486DX model in CI, since DOSBox at `cycles=max` hides x87 cost, and a first run on real hardware.

<!-- figure roadmap-table -->

## The shelves

### Catalogued
- Every Milestone 0 step is merged with CI green on `main`; the generator would move a step down a shelf, and print a warning, if that stopped being true.

### Uncatalogued
- The order and scope of Milestones 1 to 5: the working plan, not yet a document in `docs/`.

### Wish list
- Every step of Milestones 1 to 5.

```figures
id: milestones
kind: chart
track: shared
title: Progress by milestone
alt: Stacked bar chart of the steps in each milestone by state: Milestone 0 has eight catalogued steps, and Milestones 1 to 5 have planned steps, one open decision and one idea.
caption: The steps of each milestone, by state.
at: figure milestones
thumb: svg
```
