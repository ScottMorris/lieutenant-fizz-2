---
id: design
n: 7
title: Design explorations
date: 2026-10-27
summary: A sprite design canvas with options for Ben and three enemy rosters, Fizz Level Studio for levels and scripts, and the private IMF lab that the music will come from. None of it is chosen or built into the game.
prs: [10]
---
Three pieces of design work sit outside the repository. They are options and tools; nothing in `main` depends on any of them yet. The two canvases are private artifacts of the project owner, so the links work for the owner only.

## The Episode 2 sprite canvas

[The Episode 2 sprite design canvas](https://claude.ai/artifact/Q2jqaTS6yd19Gf8Wv521Ys) lays out variants of Ben for Episode 2 and three candidate enemy rosters. They are options, and none is chosen. `docs/POC-1-FINDINGS.md` sets the direction for the enemies: the style of Zargoth's characters (crystals, chocolate, cake, mechanical hover platforms) and a Zargified Earth animal such as a chicken. The Ben the project uses is still Fizz 1's: #10 redrew the README banner's Ben from Fizz 1's own pogo sprites ({{pr10.commits}} commits, +{{pr10.added}} lines, {{pr10.state}}).

The base art resolution for Episode 2 is an open question in `docs/ARCHITECTURE.md`, because it decides what the baker must quantise for the DOS palette.

## Fizz Level Studio

[Fizz Level Studio](https://claude.ai/artifact/E43evy173R6mC7bAeN5FAF) is the design canvas of the level and overworld editor; its specification lives in the `fizz-level-studio` experiment. `docs/POC-1-FINDINGS.md` (Level tooling) records what that experiment holds: a builder language in which every level and the overworld is a short text script, a Rust exporter to Fizz 1's `Builder` calls, a nine-tool MCP server and an interactive prototype. The scripts for all 16 Episode 1 levels and the overworld rebuild Fizz 1's data exactly, fingerprints included, by `scripts/prove-all.sh`. The studio application itself is not built. For Fizz 2 the script is meant to be the authoring format and `fizz-bake` the one place that turns it into engine data (Milestone 2).

## The IMF lab

The IMF lab is a private authoring and comparison tool for IMF music (`docs/POC-1-FINDINGS.md`, Music direction); it is not published. Milestone 4 takes the music from its source: a composition that is not limited by AdLib, played directly on Web and lowered to IMF or OPL2 for DOS by a build step, with channel 5 kept for effects.

## The shelves

### Catalogued
- #10 is merged with CI {{pr10.ci}} on `{{pr10.merge}}`: the banner's Ben is drawn from Fizz 1's pogo sprites, and the archive's front page shows that banner from the tree of `main`.

### Uncatalogued
- The contents of the two canvases: they are linked, not copied, and no check in this repository reads them.
- The Level Studio results (the 16 levels rebuilt exactly): recorded in `docs/POC-1-FINDINGS.md` and run in the experiment's own repository, not here.
- The IMF lab: private, and nothing in this repository uses it.

### Wish list
- A chosen Episode 2 Ben and enemy roster, authored above the DOS palette and quantised by the baker.
- Level Studio scripts baked into Fizz 2 data, with a solvability run in CI (`docs/POC-1-FINDINGS.md`, Progression model).

> **From the workbench — IMF.** The IMF format stores AdLib music as a list of OPL2 register writes with delays between them. id Software used it for the music of Wolfenstein 3D and other games of the early 1990s, which is why an IMF file can be played back without an instrument library.
> Source: [AdLib music formats in the ModdingWiki](https://moddingwiki.shikadi.net/wiki/IMF_Format)
