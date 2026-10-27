---
id: poc
n: 1
title: The proof of concept and what it left
date: 2026-10-27
summary: An earlier repository, tagged poc-1, proved the crate split and an OPL2 music service, and showed where its scene and score contracts were shaped like the weakest target.
prs: []
---
Fizz 2 was seeded from the findings of an earlier repository, `fizz-2-dos-poc1`, tagged `poc-1`. No code was carried over; the findings were, and they are recorded in [`docs/POC-1-FINDINGS.md`](repo:docs/POC-1-FINDINGS.md) and [`docs/SCENE-CONTRACT.md`](repo:docs/SCENE-CONTRACT.md). I list here what those two documents say, with the section each statement comes from. The proof of concept's own code and history are not part of this repository, so nothing below is checked against it.

## What it settled

- **The crate split.** Rules, a contract, a renderer, a DOS runtime and thin presenters, with one deterministic `no_std` simulation driving several presenters ("What the proof of concept settled").
- **A fixed tick with press edges.** A 60 Hz tick with explicit press edges fixed the dropped sounds and diagonal input that variable timing caused.
- **Build-time preparation.** Precompiling data at build time kept DOS free of parsing, which became the rule "fidelity is lost at bake time".
- **Semantic sound events.** Effects are names; each presenter plays them in its own engine. On DOS the last effect wins on channel 5; on Web they play together.

## What it did not settle

The shared `Scene` was Mode 13h in a trait: an `Rgb6` palette, `u8` palette indices and three commands, `Clear`, `Rect` and `Stipple`. A lighthouse beam was a pile of stacked rectangles whose offsets moved each tick, so a richer presenter received no intent to draw from, only the same rectangles. The Web preview blitted the same indexed frame onto a canvas.

The score library had the same shape. It started from OPL2 limits and the Web side read them back: `fnum` and `block` words converted to hertz, Undertone waveforms guessed per channel. It used a fixed 60-row, 560-tick grid and three source formats in a 1,935-line crate, and it could not express tempo changes, long phrases or layered parts. The findings name its reusable parts: the instrument definitions, `pitch_to_fnum_block`, the IMF reader and exporter, and the DOS music service.

`docs/SCENE-CONTRACT.md` sketches the alternative: a scene of layered nodes in fixed-point world units, true colour, and a `Role` tag such as `Beam` or `Player`, which each presenter projects down. The lighthouse beam becomes one `Cone` node with a gradient fill and an additive blend. The document is marked as a proposal, and nothing in it is implemented.

## The principles it left

The seven principles of `docs/POC-1-FINDINGS.md` became the house rules of the next chapter: intent flows up and lowering flows down; fidelity is lost at bake time; platform limits live in the platform; `no_std` plus `alloc` for the rules; a presenter may add what the core does not know; DOS is a bonus, not a ceiling; and a piece is not done until a second consumer exercises it.

## The shelves

### Catalogued
- The findings and the scene sketch are committed in `docs/` on `main` (commit [`c5bb1d5`](commit:c5bb1d5) added them with the architecture), and the markdown check passes on them.

### Uncatalogued
- Every statement about the proof of concept's code (the 1,935-line score crate, the stacked-rectangle beam, the channel 5 rule): they come from the findings document, and this archive has not read the `poc-1` tree.
- Whether the reusable parts of the score library (`pitch_to_fnum_block`, the IMF reader, the DOS music service) still build: they are not in this repository.

### Wish list
- The scene contract's vertical slice: one lighthouse beam drawn as a `Cone` on Web and rasterised with dither on DOS (`docs/SCENE-CONTRACT.md`, Vertical slice).
- A composition source for music that is not limited by AdLib, lowered to IMF or OPL2 by a build step (Milestone 4).

> **From the workbench — Mode 13h.** IBM introduced the VGA in 1987 with its PS/2 computers, and BIOS mode 13h, 320 by 200 pixels in 256 colours with one byte per pixel at segment A000h, became the usual mode for DOS games. The proof of concept's scene was shaped by that mode; Fizz 2 keeps it in the DOS presenter only.
> Source: [Mode 13h, Wikipedia](https://en.wikipedia.org/wiki/Mode_13h)
