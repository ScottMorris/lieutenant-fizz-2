---
id: design
n: 7
title: Design explorations
date: 2026-10-27
summary: The Episode 2 sprite design canvas, where Ben variant A is the provisional Ben and the enemy rosters are options, the level script language from an earlier experiment, and the music direction. None of it is built into the game.
prs: [10]
---
Three pieces of design work sit outside the finished game. Nothing in `main` depends on any of them yet. The sprite canvas is a private artifact of the project owner, so its link works for the owner only.

## The Episode 2 sprite canvas

[The Episode 2 sprite design canvas](https://claude.ai/artifact/Q2jqaTS6yd19Gf8Wv521Ys) lays out variants of Ben for Episode 2 and three candidate enemy rosters.

Ben variant A is the provisional Episode 2 Ben. It is the variant faithful to Fizz 1's Ben, with more shading, at the recommended size of 24 by 32 pixels. The choice is provisional: the final size is undecided, and the close-up face (the bust) is not accepted and is to be redrawn. The README banner still uses Fizz 1's own pogo sprites: #10 redrew the banner's Ben from them ({{pr10.commits}} commits, +{{pr10.added}} lines, {{pr10.state}}).

The enemy rosters are options, and none is chosen. `docs/POC-1-FINDINGS.md` sets the direction for the enemies: the style of Zargoth's characters (crystals, chocolate, cake, mechanical hover platforms) and a Zargified Earth animal such as a chicken. No level design is chosen either.

The base art resolution for Episode 2 is an open question in `docs/ARCHITECTURE.md`, because it decides what the baker must quantise for the DOS palette.

## The level script language

An earlier experiment designed a level script language for levels and the overworld. `docs/POC-1-FINDINGS.md` (Level tooling) records what that experiment held: a builder language in which every level and the overworld is a short text script, a Rust exporter to Fizz 1's `Builder` calls, a nine-tool MCP server and an interactive prototype. The scripts for all 16 Episode 1 levels and the overworld rebuild Fizz 1's data exactly, fingerprints included. The editor application itself was not built. For Fizz 2 the script is meant to be the authoring format and `fizz-bake` the one place that turns it into engine data (Milestone 2).

## Music from separately authored songs

Milestone 4 takes the music from its source: a composition that is not limited by AdLib, played directly on Web and lowered to IMF or OPL2 for DOS by a build step (`docs/POC-1-FINDINGS.md`, Music direction). Songs will come from separately authored material, and OPL register streams made by other tools can be imported through the stream spec. Channels 5 and 14 stay reserved for effects, which is a rule of the game's audio service that any score-writing tool must respect (`docs/SOUND.md`, Scores from other tools).

## The shelves

### Catalogued
- #10 is merged with CI {{pr10.ci}} on `{{pr10.merge}}`: the banner's Ben is drawn from Fizz 1's pogo sprites, and the archive's front page shows that banner from the tree of `main`.

### Uncatalogued
- The contents of the sprite canvas, including Ben variant A: it is linked, not copied, and no check in this repository reads it.
- The level script results (the 16 levels rebuilt exactly): recorded in `docs/POC-1-FINDINGS.md` from an earlier experiment and not re-run here.
- Songs from other tools: nothing in this repository imports them yet.

### Wish list
- A final Ben: the size, a redrawn bust, and the sprites authored above the DOS palette and quantised by the baker.
- A chosen enemy roster and the level designs.
- Level scripts baked into Fizz 2 data, with a solvability run in CI (`docs/POC-1-FINDINGS.md`, Progression model).

> **From the workbench — IMF.** The IMF format stores AdLib music as a list of OPL2 register writes with delays between them. id Software used it for the music of Wolfenstein 3D and other games of the early 1990s, which is why an IMF file can be played back without an instrument library.
> Source: [AdLib music formats in the ModdingWiki](https://moddingwiki.shikadi.net/wiki/IMF_Format)
