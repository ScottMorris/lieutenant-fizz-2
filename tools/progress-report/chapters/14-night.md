---
id: night
n: 14
title: Night
date: 2026-11-04
summary: The Night setting and the light model behind it. A Night byte in FIZZ2.CFG and a pair of switches, a 256-entry palette of brightness banks, integer point lights in 8 by 8 cells, the slice's lanterns, lamps, beam and Ben's lantern, the Light and Full detail that decide whether the lights are cast, what each cost in DOSBox, and the PC speaker count that depended on how long start-up took. One pull request, stacked on the exit; it is in review.
prs: [146]
---
Chapter 12 drew the slice in oblique projection and gave it a night sky. This chapter is the other meaning of night: the level after dark, lit by lanterns, with Ben carrying one. It is one pull request, #146 ({{pr146.state}}), on the branch of the exit that takes an Up press (#145, {{pr145.state}}), so it is not on `main`. The setting is Off by default and the day picture is byte for byte what it was, so the state hash and the golden transcript are untouched. The design is `docs/LIGHTS.md`; the DOS side of it is the "Night on DOS" section of `docs/PLATFORM.md`; the player's side is the Night section of `docs/MANUAL.md`.

The pull request also carries a fix that is unrelated to light. The PC speaker's step count in the replay depended on how many ticks start-up took, and the longer start-up of the night build exposed it in CI. That is the last section.

## The setting

Night is `fizz_machine::Night` (Off or On) in the preferences. `FIZZ2.CFG` is now version 4, 14 bytes, and byte 13 is the night: 0 for Off, 1 for On, and `cfg-night` for any other value. Versions 1 to 3 still read, with the night Off, and saving writes version 4. DOS has `/NIGHT` and `/DAY`, the last one given wins, and both Options screens have a Night row before Back (`RowId::NIGHT`, 15). The DOS screen now has ten rows, so its rows start lower (`ROWS_Y` 40) and the Options and Paused digests were pinned again. The Web keeps `night` in its `fizz2-options` blob.

The `/?` screen was already 25 lines, so a switch may now name its opposite on its own line (`Switch::pair`): `/MODERN /KEEN` share a line, which made room for `/NIGHT /DAY`. The help tests check both ends of a pair.

## The palette

A frame byte is `bank * 16 + colour`. A level `L` from 1 (black) to 14 (full) shows `(L - 1) / 13` of the EGA colour, to the nearest 6-bit DAC step. Bank 0 is the picture as drawn: full brightness by day and the night's ambient, level 6, at night. Bank 1 is the dim the HUD box and the menus already used (level 5). Bank 2 is glow, full brightness by day and by night. Banks 3 to 15 are the shaded levels 13 down to 1, and bank 7, level 9, marks an actor. `light::palette` builds the 256 entries as a constant; the day's table and the night's differ in bank 0 only.

Because the palette carries the ambient, a cell that no light reaches is never written, and every day frame stays as it was. The Web widens each 6-bit value with `v * 255 / 63`, rounded, which gives exactly the EGA bytes 0x55, 0xAA and 0xFF, so a byte means the same colour on both presenters. Fizz 1's DOS port draws in a 256-colour mode with a palette of brightness levels and a pass over the frame; this is that technique with the ambient moved into the palette, and `docs/LIGHTS.md` lists each Fizz 1 source it reproduces and where it departs.

What stays full bright is decided by the bank the art is drawn in. Ben, the chicken, its beak and the stun stars are actors and never fall below level 9, about 0.62 of full, Fizz 1's floor. Pickups, the bubbles and their puffs, the door badges, the exit's opening and arrow, the glass of the lamps and lanterns and the captions are glow. The HUD box on DOS and the `LEVEL CLEAR` banner draw their own pixels in the glow bank, and the menus are shown in the day palette over a picture dimmed into bank 1.

## The lights

A light is a point in quarter pixels with a height `z`, a radius and a strength. What it adds to an 8 by 8 cell is Fizz 1's shader term for a sprite facing the screen, taken from a baked falloff table over the squared distance, so no square root runs and no floating point is used. A cell's level is the ambient plus its added light, rounded down, at most level 14, and within a cell a 2 by 2 ordered dither picks each pixel's level between two quarter levels. `light::Lights` selects the nearest 16 lights to the view's centre with the player's lantern pinned first; the static lights flicker as in Fizz 1, from a baked sine table.

| Light | Where | Character |
| --- | --- | --- |
| Lantern | over the red door and over the blue door | flickering, radius 8.5 |
| Lamp | in each of the two shafts | flickering, radius 7.5 |
| Exit glow | the middle of the doorway | green, radius 4 |
| Lighthouse beam | level with the gallery's railing | sweeps once every 512 ticks, dark while it faces the sea |
| Ben's lantern | 1.1 tiles over his feet | pinned first, radius 5.5 |
| Bubbles | each shot | cyan, radius 3 |

<!-- figure shots oblique-night.png -->

The lamps and lanterns are new baked pieces, drawn only at night, with their glowing parts split into their own pieces so nothing is read back from the frame. The slice never has more than 15 lights reaching the widest Web view, and a test sweeps the camera over the map to hold that.

## Detail and cost

The light pass belongs to the Full detail of the Sky setting (chapter 12). The Light detail, which is where DOS Auto sits on the floor, draws the night palette, the night fixtures and the markers and casts no light, so the level is evenly dim and the glowing art and the actors stand out against it. The first version cast the lights in every night frame and drew at an average of 32,594 us in DOSBox on the 486DX2-66 model (`BENCH NIGHT DRAW avg 32594`) because it worked a cell's level out again on every pixel row and Fizz 1's 8.5-tile lights reach most of the screen. `light::shade` now works a row of cells out once for its eight pixel rows and shades only the runs of lit cells. Shorter lights did not fit either. With Ben's lantern and the bubbles alone, at a radius of 4 tiles, the worst frame still added 2.7 ms because his lantern is in every frame, so the floor's night is the Light detail.

| Pass | Draw average | Draw largest | Total largest |
| --- | --- | --- | --- |
| Day, Light sky | 9,809 us | 13,376 us | 14,146 us |
| Night, Light | 10,244 us | 13,927 us | 14,698 us |
| Night, Full | 17,136 us | 24,071 us | 24,841 us |

These are DOSBox Staging at 26,800 cycles a millisecond over the 1,800-tick script, from local runs of `tools/check-dos.sh` with one job, in microseconds, against a step of 16,667. The Light night fits and `tools/check-dos.sh` holds its largest total under the step; the Full night does not, and its lines are logged without a bound. At the slice's worst frames 300 to 400 of the roughly 1,000 cells are lit, which is 4 to 6 ms against the 2.5 ms the worst day frame leaves. On the host build at the DOS build's optimisation level the day picture costs 68 instructions a frame more than before, and no day digest moved. `FIZZ2.EXE` is 374,272 bytes, 20,480 more than before.

Two pieces exist and nothing uses them. `faded` scales a palette towards black for a fade through the DAC, and `light::lower_span` lowers a shaded span by a number of levels for later shadows and fog; each has a test. The shade is also not fused into the copy to video memory. In DOSBox a `rep movsd` element costs what any instruction does, so fusing would save a few hundredths of a millisecond; on real hardware it would save a pass over the lit part of the frame, and it waits for a measurement on a 486.

<!-- figure shots dos-options.png web-options.png -->

## One speaker step fewer

The DOS gate of #146 failed one case, and only one. `AUDIO count (the PC speaker (1392 steps))` got 1391. Every OPL count matched and every line of the golden transcript held. The same branch gave 1392 on the machine that built it.

The clock that paces the game on DOS is the 560 Hz interrupt counter, and `Pacer::new` starts from whatever tick start-up ends on. The OPL players step every interrupt counted from their own start. The PC speaker stepped whenever the audio service's global tick count was a multiple of four (`self.ticks & 3 == 0` in `dos/rt/src/audio.rs`), so its 140 Hz frames sat at a phase of 0 to 3 interrupts against the simulation's steps, and that phase depended on how long start-up took. The replay is built so that its last sound finishes, which keeps most counts independent of phase, but a sound cut short by a later one played one step more or less depending on the phase. The night build starts up more slowly, and the build differs between machines, so the phase moved.

To prove it, the replay's start was held at each phase modulo four with a throwaway busy-wait before `Pacer::new`, not committed. The speaker made 1,393, 1,392, 1,391 and 1,390 steps while the OPL2 count stayed 7,375. The speaker now counts its 140 Hz steps from the tick its effect begins, as the OPL players do, and the total is 1,398 at all four phases and on repeated runs. The pinned count in `tools/check-dos.sh`, `docs/SOUND.md` and the manual's example moved with it.

This is the fourth cause of drifting counts in the replay; chapter 13 tells the first three.

| Step | Speaker steps |
| --- | --- |
| Start held at phase 0, 1, 2 and 3 before the fix | 1,393, 1,392, 1,391, 1,390 |
| After the fix, at each phase | 1,398 |

| Item | Value |
| --- | --- |
| Fix | `4c367e2`, the speaker counts from each effect's start |
| Pinned count in `tools/check-dos.sh` | 1,398 |

## The captures retaken

The DOSBox and Chromium captures in the manual and in this archive still showed the flat tiles. They were taken again from the head of the stack with `capture.py`, so they show the oblique ground, the exit doorway and the Sky and Night rows. The 1,800-tick replay moves every scene, so the replay grabs were picked again and each record in `media/captures.json` says what its grab shows. No grab caught the click of the empty blaster, so the empty-blaster picture is a nearby grab with a fzzt caption, and its record describes that instead of the click. The check screen is drawn from the text cells the program printed by a tool that needs a PSF2 VGA font, which the machine lacked until the capture run.

## The shelves

### Catalogued
- The Night setting (#146, {{pr146.state}}, CI {{pr146.ci}}): `fizz-machine` tests for the byte, the version 4 file with versions 1 to 3 read and the switches, the help tests for a pair, and the Options rows; `tools/check-dos.sh` checks the version 4 file, `/CFGTEST /NIGHT`, `/NIGHT /DAY`, a rejected `/NIGHT:ON` and the Night row on the Options screen.
- The palette and the light model: `light::tests` check every palette entry against the 6-bit rule, the falloff and sine tables against their formulas, the shade against byte-by-byte shading, the nearest 16 with the pinned light first, and that random lights and sizes never write outside the frame; `night_tests` pin seven night frames with each detail, hold actors at level 9 or above and glow unchanged across the replay, draw any view size from 1 by 1 to 2,000 by 900, and sweep the light budget.
- The day picture is unchanged: every pinned frame digest stands, and the golden transcript still ends `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`.
- The night frames on DOS equal the host's: `/BENCH` prints digests for the Light and Full nights and `tools/check-dos.sh` compares them with the `frames` example.
- The speaker count is 1,398 at every phase: the check in `tools/check-dos.sh` requires it, and the sweep that found the cause is recorded in the pull request's test plan.

### Uncatalogued
- Every timing is DOSBox at a fixed cycle count, from local runs of `tools/check-dos.sh` on a machine that ran other work; none is a CI figure and none is from a real 486.
- Nothing has run on real hardware, and the DAC's palette writes and the lights were not seen on a real VGA card.
- How the night reads to a player. The lit night in this chapter is a host render; no DOSBox or browser capture of it is recorded, and no playtest is.
- The four-phase sweep that proved the speaker's cause used a busy-wait that was not committed, so the sweep cannot be repeated from the repository.
- The Web night is the same integer code; it was not timed in a browser, and the Web tests that need `@liminal-hq/undertone` were left to CI because the package was unavailable locally.
- Whether anyone can hear what the speaker count describes; the counts are steps, not sound.

### Wish list
- A Full-detail night that fits the floor: fewer cells lit, a lighter shade, or the shade fused into the copy once a 486 can time it.
- Palette fades (`faded`) for a screen transition, and `lower_span` for shadow or fog.
- A theme format that carries a level's ambient and lights in place of the constants in `oblique.rs`.

> **From the workbench — The VGA DAC.** The digital-to-analogue converter of a VGA card holds 256 palette entries of 6 bits for each of red, green and blue, and a program changes what every pixel of one palette index looks like by writing those entries, without touching the frame. That is why a fade or a dimmer ambient costs 768 port writes, not a pass over 64,000 pixels.
> Source: [VGA hardware, OSDev Wiki](https://wiki.osdev.org/VGA_Hardware)

```figures
id: night-options
kind: screenshot
track: shared
title: The Night row on both Options screens
alt: Two Options screens over the dimmed slice. On the left, DOSBox: rows Music, Effects, Sound placement, Frame rate, Motion, Captions, Keys, Sky, Night and Back, the plate on Effects, with Night reading Off. On the right, headless Chromium: Sound, Music, Placement, Motion, Captions, Keys, Fullscreen, Sky, Night and Back, the plate on Sound, with Night reading Off.
caption: The DOS screen (FIZZ2.EXE in live play after Esc, Down, Enter and Down) and the Web screen (after Esc, Down and Enter), retaken from the stack's head. Both show Night Off; no capture of a lit night is recorded.
at: figure shots dos-options.png web-options.png
thumb: img:media/dos-options.png
```

```figures
id: night-lit
kind: screenshot
track: shared
title: The gallery at night, lit
alt: The open-air gallery at night at 320 by 200. The planks, bricks and railing are dim, brighter round a small lantern over the blue door and round Ben, who stands on the right firing a bubble; the green exit doorway glows, and the stars, clouds, grey headlands, sea and the pickups on the snack shelf stay bright.
caption: Replay tick 1225 drawn at night with the Full detail by the shared routine, as a host render; the light pass is the one DOS skips on its floor.
at: figure shots oblique-night.png
thumb: img:media/oblique-night.png
```
