---
id: frames
n: 13
title: Fitting the oblique ground into a frame
date: 2026-11-03
summary: What the oblique ground cost on DOS and how it was brought under the 60 Hz step. The first gate run (draw 29,279 us against 3,299), the callgrind profile, a pixel-identical rewrite, the memory floor raised to 4 MB, the dear-frames recalibration, three separate causes of drifting audio counts, the HUD and text fixes found by their worst frames, the Sky setting with Auto, the golden replay that grew to 1,800 ticks when the exit took an Up press, then the fixed CPU speed for the sound counts, an estimate of the drawing in 486 clocks and the measured case against copying only what changed. All merged.
prs: [143, 144, 145, 169, 170, 172]
---
The budget is one tick at 60 Hz, 16,667 us, shared by the simulation step, the drawing and the copy to the screen. DOS leads on budgets and the floor is a 486DX2-66, modelled by `tools/check-dos.sh` as DOSBox Staging 0.83.0 at a fixed 26,800 cycles a millisecond (`docs/PLATFORM.md` has the method and says what it cannot show: no cache, no bus, no x87 cost, no slow video card). Chapter 12 is the look. This chapter is what that look cost when the DOS gate first measured it, the work that brought it back inside a step, and the failures along the way, including the ones that were the checks' own. Every number below has its source named; none comes from a real 486.

## The first gate run

The stack's DOS gate failed on its first run. At 26,800 cycles `BENCH DRAW` averaged 29,279 us against 3,299 us on `main`, and the six-captions picture at tick 1190 took 40,798 us, over the 16,667 us that `caps_check` allows, so DOS could not hold 60 Hz. The renderer alone, before the parallax was stacked on it (#143), failed the same check: the picture at tick 1190 took 25,018 us with no captions and 28,192 us with six.

The estimate in #143 had been 3 to 4 ms for the ground, from about 2,170 row copies a frame at some 40 cycles each. It was written as an estimate and left to CI to replace, and CI's figure was about seven times higher at the picture it measured. The copies were not the cost.

## Where the instructions went

Host time says little about a 486, so the profile counts instructions: `valgrind --tool=callgrind` over the 48 `bench` cameras of the `frame` example, built at `opt-level = "s"` as the DOS build is. The oblique ground with its sky cost about 524,000 instructions a frame on average (indoor cameras 439,000, gallery cameras 710,000, the worst frame 894,000), against about 7,800 for the flat tile pass it replaced, about 67 times as many. The breakdown was bookkeeping, not pixels:

- About 45% was the same questions asked again. The tile at a cell and at each of its neighbours was looked up, with clamping, for the cell's look, for its background and for every neighbour of those, and a stone cell's depth was a loop over a mask.
- About 18% was the parallax set-up, repeated for every run of open-air cells: clearing the rectangle, hashing every star and cloud cell it touched, drawing cloud rows through a per-pixel test, and for each band looping over every pixel row and every column with a modulo for the column's height.
- About 20% was per-row copy overhead: the clip, the period and the table row worked out again for each row of a run.

`tools/bench-draw.sh` was added as the figure to compare between changes. It builds `frame` at `opt-level = "s"` with debug information, runs one round under callgrind collecting only while `frame::draw` runs, and prints the average and worst frame for the indoor cameras, the gallery cameras and all of them.

## A rewrite that draws the same pixels

The optimisation was held to a rule: every frame stays pixel-identical, so the pinned frame digests do not change and the tests that compare the new path with the old one decide whether it is right.

- **The cell window.** A draw reads the map once into `Window`, which holds the tile of every cell the view reaches and three cells round them, and for each cell its look and background packed in one byte, in two arrays on the stack (768 cells for the 320 by 200 picture, 3,840 for the widest Web view). The backgrounds, dressing, terrain and spikes read it and none asks the map for a neighbour. A view too large for the arrays is drawn asking the map, as before. A class table makes the test of a tile one load and a mask.
- **Stone depth** is worked out for the whole view from one bit mask of open cells for each column, so a cell's depth is a shift and a mask. The old cache read too few rows when the whole view lay below the map, and a test now compares the window with the map for doors shut and open.
- **Fewer, longer copies.** A run's clip, period and table row are worked out once and each row is copied with the length class chosen once, a background run takes in the cells that four solid cells cover, and a ledge's three rows of shadow are drawn for the whole run of planks. Pieces are baked with their opaque runs, so drawing a piece copies runs and tests no pixel.
- **The sky prepared once for the frame.** `Sky::new` works out each layer's shift, the stars that can lie in the frame, the clouds and the columns of each band in groups of equal height, and `Sky::draw` composes a rectangle from those lists. Rows from which a band is fill in every column hide every layer before them, so the sea below the horizon is one fill. A stack the fixed lists cannot hold falls back to the old path, and new tests compare the two over random cameras, ticks, stacks, orders and clips.

Measured on the host in instructions a frame: the ground alone went from 460,000 to 149,000 with the window and the longer copies, and the whole frame from 524,000 to 173,000 once the sky was prepared once, about a third. The tool's own figures for that build are 173,000 on average, 230,000 for the gallery cameras and 278,000 for the worst frame. CI's DOS gate then measured, at 26,800 cycles, `BENCH DRAW avg 10180`, the six-captions picture at 13,642 us and `FRAMES 1755 TICKS 1757 RATE 60`: the speed checks passed.

## The memory the tables took

The job then failed later, on memory: `/PROBE (memsize 2) free_kb '0' is outside 64-1024`. On `main`, `memsize = 2` leaves 96 KB free; with the stack it left none, and at `memsize = 4` it read 2,008 KB where `docs/DOS-CHECK.md` had 2,256. The baked tables are about 100 KB (three depths of stone front at 16 KB each, the back wall at 24 KB, brick 8 KB, plank and stone tops 6.5 KB each) and the drawing code is about 45 KB more, which is more than a 2 MB machine has spare.

The options were to shrink the tables, give DOS a reduced art path, or raise the floor. The owner chose to raise the floor so that Web and DOS keep the same pictures. The DOS slice now needs 4 MB: `tools/check-dos.sh` keeps the `/PROBE` ladder at 16 and 4 MB, drops the 2 and 1 MB cases and repeats `/REPLAY` at `memsize = 4`; `MEMORY_FLOOR_KB` is unchanged, so a 2 MB machine still starts and the check screen warns that less than 1 MB is free; and the manual names 4 MB. `docs/DOS-CHECK.md` records that the tables could shrink later, by a shorter period or by shades derived at draw time, if a smaller machine matters again.

## The dear-frames case

`frames_replay 'Auto goes to 30 when frames are dear'` slows DOSBox to `FRAMES_SLOW` cycles so that a drawn frame costs 100 to 170% of a sixtieth of a second and Auto settles at 30 Hz. It had been calibrated on the flat tiles (draw plus copy about 3.5 ms at 26,800 cycles, so 4,500 cycles). With the oblique ground a frame costs about 10.8 ms at 26,800 cycles, so at 4,500 cycles it took about 64 ms and the run drew 425 frames, below the 800 to 950 the case expects. 14,000 cycles puts a frame at about 21 ms, 124%, the same place in the governor's 30 Hz band as before. This was a calibration that went stale with the cost, not a fault in the governor; the cases at 26,800 cycles, where Auto stays at 60, were not touched and CI drew 1,758 frames there.

## Audio counts that would not hold still

The next failures were in `AUDIO WRITES`, the count of OPL register writes that the replay's sounds make, which `tools/check-dos.sh` pins exactly because the replay is deterministic and DOSBox runs at fixed cycles. `/REPLAY (/SB, sb16)` counted 14,620 where 14,622 was expected, and an earlier run gave 7,368 against 7,366. There turned out to be three causes, found one after another.

1. **Frames longer than a step.** The DOS audio service has one effect slot that the 560 Hz interrupt picks up, and the last request wins. While every drawn frame fits in a 16.7 ms step, each step's sound gets its own audio tick. A frame longer than a step makes the `Pacer` release the steps that fell behind back to back, and two steps' sounds can land in one audio tick, so one is lost. `BENCH DRAW` peaked at 19,262 us against about 12 ms on `main`. The fix is to bring the worst frame under a step on the floor; loosening the audio checks would have hidden the cause.
2. **The length of a cut sound.** With the exit on an Up press (below), the replay's `ta-da!` started at tick 1710 and the chicken's cluck at tick 1720 cut it short. How many 140 Hz frames a cut sound plays depends on the phase of the 560 Hz audio clock at the two requests, and ten 60 Hz ticks are 93.3 audio ticks, so the phase differs between sound paths. Channel 5 with music and without it counted 7,136 and 7,135 writes, which broke the relations `tools/check-dos.sh` requires between them. The replay now waits in the doorway for the cluck to finish and lets its last sound play out, as the next section describes.
3. **The check's own count file.** CI failed `/REPLAY (/MUSIC:OFF, sb16 (the pair, no music))` with `AUDIO WRITES 14622` against an expectation worked out from an empty channel-5 count (the message read `expected -68 writes ... channel 5's -20`): the run had written the right number. Four `opl2` cases run in parallel and each wrote channel 5's count with `printf > "$parroot/ch5-writes"`, which truncates the file before it writes; a reader that came between another case's truncation and its write read an empty string and the arithmetic took it as 0. It depends only on how the parallel cases interleave and not on the drawing or the audio timing. `ch5_put` now writes a file of the case's own and renames it into place, which is atomic, and `ch5_get` keeps waiting until it reads a number. A stress run of four writers and 2,000 reads gave 71 bad reads the old way and none the new way.

## What the worst frames were

With the average near 10 ms the remaining risk was the worst frame, and the worst frames of the replay were not the ground's. They were the ticks that render the HUD again (a Fizz shot, a pickup, each step of the score counting up), at about 19.3 ms of drawing against a 10.3 ms average at 26,800 cycles. These are the figures from callgrind on the DOS build's own code: the renderer built for the `dos-elf.json` CPU (`i486`, no SSE) at `opt-level = "s"` with `dos/rt/src/mem.rs`'s `rep movsd` and `rep stosd`, run as a static 32-bit program under 32-bit callgrind, which counts each element of a `rep` string instruction as DOSBox charges a cycle for each. It agreed with CI's DOSBox figures to within 1 percent before any change (draw average 11,811 us against 11,705, largest 23,200 against 23,121, the six-captions picture 17,589 against 17,490), so it stands in for the gate between pushes. Three changes, each the same picture:

- **A baked HUD pill.** A change of a HUD number rendered the whole 120 by 27 strip, working out the rounded outline pixel by pixel (`depth` and `in_pill` for 3,240 pixels) and rebuilt all 810 words of the composite masks. The outline never changes, so it is now `PILL`, a 3,240-byte table built at compile time with the same functions made `const`, and `Hud::update` renders into a scratch strip and works out the masks only for the words that differ from the strip shown. The worst frame of the replay went from 517,700 to 331,700 instructions, 19.3 to 12.4 ms.
- **Text by its set bits.** `pfont::draw_text` tested every column of every glyph row against the frame's edges with 64-bit arithmetic and searched the font (seven steps) for every character, twice for a caption. A row is now masked to the frame once and its set bits visited lowest first, and code points under 128 go through a 128-byte table built at compile time. A test compares the new loop with the old over 1,200 random positions, sizes and strings, and another checks the table against the search. Six captions at tick 1190 went from 86,000 to 42,000 instructions.
- **One call for a long run on 32-bit x86.** The renderer moved a run of 33 to 128 pixels as two overlapping fixed-size moves, which on x86-64 and WebAssembly are vector moves. On the DOS build (i486, no vector registers, `opt-level = "s"`) the compiler inlines a fixed move of up to 16 bytes and makes anything larger a call to `memcpy` or `memset`, so the trick cost two calls and their set-up. Under `ONE_CALL_FROM_32` (`cfg!(target_arch = "x86")`) a run of 32 or more is one `copy_from_slice` or `fill`. It saves about 16,000 instructions a frame, 5% of the draw, and the other targets' code does not change.

The sky also drew once for each row of cells across gaps of up to four cells (`OPEN_GAP`) instead of once for each run, and prepared its stars only above the row from which a later band is fill. On the Full sky the six-captions frame at tick 1190 went from 471,400 instructions to 445,900 with the first and 395,100 after the second and a caption fix. CI then measured `BENCH DRAW avg 9282 max 12333` for the renderer without the parallax (it had been 10,180 and 19,262), the six-captions picture at 11,610 us (13,642 before) and 12,829 us under `/BENCH /AUDIO`, with Auto holding 60 Hz. The worst frame plus the 600 us copy was 12.9 ms, under a step.

## The sky does not fit the floor

The renderer now fit, and the Full sky on top of it did not. CI measured the parallax branch at `BENCH DRAW avg 11705 max 23121` and the six-captions picture at 17,490 us, 823 us over the 16,667 us limit, and the worst frames ran past a step again, which put the audio counts back in the first cause above. The renderer fit; the sky on top of it was 0.8 ms too much at the point the gate looks.

The answer was a detail setting, as in Fizz 1's DOS port, which has Auto, High and Low for its backdrop layers in `FIZZ.CFG` byte 8: Low drops the backdrops, and Auto is judged in the frame governor's window, only while the frame rate is Auto, going Low above 45 ms a frame and back to High under 17 ms. Fizz 2's Sky setting (chapter 12) is Auto, Full or Light in `FIZZ2.CFG` byte 8, a byte that had been reserved for a detail level and written as 0. Light keeps the stars and the sea and drops the clouds and the headlands, which are its only layers with a transparent sprite over many columns or a per-pixel checker. Auto starts Light, and `FrameGovernor::with_auto_sky` chooses Full when a 32-frame window of Light frames averages under `SKY_FULL_BELOW` (45% of a 60 Hz frame) and Light again when a window of Full frames averages over `SKY_LIGHT_ABOVE` (75%), only while the frame rate is Auto and drawing at 60 Hz; a window that changes the sky does not also change the rate. The thresholds are this game's. Full's worst frame costs about 1.5 times Light's average on the floor, so 45% keeps Full's worst near 70% of a step, and the floor's Light windows (about 63% in `/REPLAY`, more in `/LIVE`, which counts the retrace wait) stay Light. On the floor, then, DOS Auto resolves to Light; the Web draws Full.

`/BENCH` is not governed: it measures the sky Auto starts with (Light) and says which (`BENCH SKY LIGHT`), and then plays the script again with the Full sky and prints `BENCH FULL SKY` lines that `tools/check-dos.sh` logs without a bound, so later work can follow what Full costs at the floor. The tick of the worst frame is printed too. `BENCH FRAME 1190` digests a gallery frame, so the DOS sky is compared with the host's pixel for pixel.

## The exit and the golden replay (#145)

Fizz 1 ends the level the moment Ben touches an exit tile. The exit is now a doorway on the far edge of the floor, and the owner decided that Ben walks into it with Up, like a door. In `tick_ben_contacts`, `Event::Exit` is raised only when Up is newly pressed (`input.pressed` contains `Buttons::UP`), Ben's body overlaps an `EXIT` tile and `ben.b.on_ground` is set. Touching alone, a press in mid-air and an Up held from before Ben arrived all do nothing, and on the exit tick the look-up timer is cleared so the camera does not start to look. The entry in `docs/FIZZ1-DIFFERENCES.md` records the consequences: an exit must be somewhere Ben can stand, never somewhere he only passes through in the air or on the pogo stick, and levels ported from Fizz 1 whose exit is reached by a jump or a bounce need a place to land beside it.

The golden replay moved with it. The script steps off the pogo stick in the doorway, lands and presses Up, which moved the exit event from tick 1664 to 1710 and the final hash from `a08501a57d509e8a` to `cee58f32f465df2a`. Then the second cause of drifting counts appeared (above), and the script was changed again: it waits in the doorway for the chicken's cluck at tick 1720 to finish, presses Up at tick 1745 (the exit event is at 1746) and runs to 1,800 ticks so that the `ta-da!`, 771 ms long, ends before the run does. Nothing before tick 1666 changed. The golden then ended `FINAL TICK 1800 HASH 843736be6fdaeb76` (it ends `{{golden.final}}` today, after the re-pins that chapters 12 and 15 describe), and the pinned copies moved with it: the replay tests, the `ta-da!` caption tick, the Web options check, and `tools/check-dos.sh` (the BIOS tick bound 544 to 548, `BENCH TICKS 1800`, the 60 Hz frame windows 1,800 to 1,802 and the 30 Hz windows 890 to 910).

Each sound path now agrees. Measured one case at a time, the totals went from 7,345, 7,366, 14,622 and 1,388 to the figures below.

| Path | Before | After |
| --- | --- | --- |
| Channel 5 on an OPL2 | 7,345 | 7,375 |
| An OPL3 on the AdLib role | 7,366 | 7,396 |
| The OPL3 pair | 14,622 | 14,682 |
| PC speaker steps | 1,388 | 1,392 |

The pair is 41 start and stop writes plus twice the 7,355 records, less 69 enables. `docs/SOUND.md` says why the run lets its last sound finish on purpose.

## Frame budgets by stage

All figures are DOSBox Staging 0.83.0 at 26,800 cycles a millisecond over the replay, in microseconds, from the `BENCH` lines of CI's DOS gate unless the last column says otherwise. The six-captions column is the picture at tick 1190.

| Stage | Draw average | Worst frame | Six captions | Source |
| --- | --- | --- | --- | --- |
| `main`, flat tiles | 3,299 | about 12,000 | not recorded | CI, `main` |
| Renderer and parallax as pushed | 29,279 | not recorded | 40,798 | CI, #144 |
| Renderer alone as pushed | not recorded | not recorded | 28,192 | CI, #143 |
| After the cell window, longer copies and the prepared sky | 10,180 | 19,262 | 13,642 | CI, #143 |
| Full sky, before the HUD, text and copy fixes reached #144 | 11,705 | 23,121 | 17,490 | CI, #144 |
| After the HUD, text and copy fixes, no parallax | 9,282 | 12,333 | 11,610 | CI, #143 |
| With Auto, which is Light at the floor | 9,801 | 13,377 | 12,656 | CI, #144 |
| The Full sky, logged and not bounded | 10,588 | 15,375 | 14,661 | CI, #144 |

The same work counted in instructions a frame (host callgrind at `opt-level = "s"`, the figure to compare between changes):

| Stage | Instructions a frame |
| --- | --- |
| Flat tile pass | about 7,800 |
| Ground and sky as pushed | 524,000 average, 894,000 worst |
| Ground alone, before and after the cell window | 460,000, then 149,000 |
| Ground and sky after the prepared sky | 173,000 average, 230,000 gallery, 278,000 worst |

The local run of the same gate in `snapshots/measured.json` gives `BENCH DRAW` {{ms.benchDrawUs}} us and `BENCH TOTAL` {{ms.benchTickUs}} us on average against the 16,667 us tick, and `FIZZ2.EXE` at {{ms.exeBytes}} bytes. `tools/check-dos.sh` holds no tight bound on the drawing's average; its bounds are the six-captions picture under 16,667 us, the step average, the digests and the audio and frame counts above.

## Where the checks were run

The DOS gate ran in CI in the pinned `ci-dos` image, and for this stack also on a machine with Docker, in that same image: `tools/check-dos.sh` built `FIZZ2.EXE` there, with `vendor/` linked to the toolchain copied out of the image, and the replay, `/BENCH` and the parity script ran on the host against that build. The audio totals above come from `tools/check-dos.sh --groups base,audio,config-sound` with `CHECK_DOS_JOBS=1`: DOSBox ran one case at a time, because four DOSBox processes in parallel on a loaded host dropped frames and moved the counts. `tools/check-parity.sh` printed native, Web and DOS matches and `PARITY ok {{ms.parityHash}}` for the 1,800-tick golden. The `BENCH` numbers on a loaded host are high-side, and the figures in the table are CI's.

## Sound counts at a fixed CPU speed (#170)

One cause of drifting counts outlived the stack. The `/REPLAY` sound cases and the `AUDIO count` cases ran at DOSBox's `max` cycles, where emulated time follows the host's clock; when the host stalls, the game releases two steps inside one 560 Hz audio tick and one of their sounds is lost, which is the first cause above arriving from outside the program. Two idle local runs on `main` each failed a different case, by two writes (14,680 for 14,682 in the corrupt-file case, 7,398 for 7,396 through the OR rule), and the `parity` jobs of #153 and #156 failed on the same flake, by two to seven writes, until they were rebased onto the fix. The full gate on `main` failed the same way on the merge commit of #163 and #164 (`AUDIO WRITES 15140` against 15,142 for the pair without music, run 42095073445), and no run on that commit passed, so the roadmap holds their steps at in progress. #170 ({{pr170.state}}) runs both helpers at `DOS_CYCLES="$CYCLES"`, the 26,800 cycles the bench groups already use. Three runs of each of the OR-rule, pair and speaker cases then gave the same counts every time, and `CHECK_DOS_JOBS=1 tools/check-dos.sh --groups base,audio,config-sound` passed twice where it had failed twice. A later measurement for #168 found eight of eight pair runs at 15,533 at 26,800 cycles, against 15,511 to 15,533 at `max`. `docs/SOUND.md` says why.

## An estimate in 486 clocks (#169)

DOSBox charges about one cycle for an instruction, so it under-counts x87 work and string moves against a real 486. #169 ({{pr169.state}}) ported Fizz 1 DOS's estimator as `tools/cyc486.py`, which weighs each instruction of a callgrind profile by the Intel i486 datasheet's cache-hit timings, and `tools/bench486/`, a `no_std` harness built for `dos/dos-elf.json` at the DOS release profile with the runtime's `memcpy` and `memset`, linked as a static 32-bit program; its frame digests equal `FIZZ2.EXE`'s. `tools/bench-draw.sh --486 [--full] [--night]` runs it under 32-bit callgrind. The weight table was checked against the datasheet's tables 10.1 and 10.3 and differs from Fizz 1's in conditional jumps (3 taken, 1 not), `rep movs` (12 + 3 an element) and `rep stos` (7 + 4), x87 costs by operand size, a register `pop` at 1 and address-generation penalties; Fizz 1's table gives 474,000 cycles a frame on the same profile. `tools/check.sh` runs `python3 -I tools/cyc486.py --self-test`.

| What is drawn | Estimated 486 clocks a frame | At 66 MHz | DOSBox `/BENCH` at 26,800 |
| --- | --- | --- | --- |
| Replay, Light sky, by day | 515,000 | 7.8 ms | 9.8 ms |
| Replay, Full sky | 552,000 | 8.4 ms | 10.6 ms |
| Replay, night, Light | 535,000 | 8.1 ms | 10.2 ms |
| Replay, night, Full | 879,000 | 13.3 ms | 17.1 ms |

The x87 is about 1% of the drawing's clocks, and `draw_cells`, `memcpy`, `table_rows` and `piece` account for 76%. All float work in the draw is about 0.76%, around 23 `tile_px` conversions a frame; an integer `tile_px` that left every frame unchanged saved about 0.3% and was not kept. The estimate leaves out cache misses, the write buffer, bus and VGA wait states, misaligned accesses and x87 overlap (`docs/PLATFORM.md`, "An estimate in 486 clocks"), and no real 486 was used.

## Copying only what changed (#172)

Two more of Fizz 1 DOS's ideas were measured and not built; `docs/PLATFORM.md`, "Copying only what changed", has the figures. Every tick of the replay was drawn on the host and compared with the tick before. The camera stands still in 1,075 of 1,799 pairs, and then about 185 bytes differ by day (1,791 on the Full night, with its beam and lanterns); a moving frame changes every row, so a copy of each row's changed span still covers about 57,000 of the 64,000 bytes. A trial `present` that compared each row with a shadow of the screen by `repe cmpsd` and copied only the span between took `BENCH PRESENT` from 599 to 1,364 us on average at 26,800 cycles, with every digest unchanged, because DOSBox charges the compare as it charges the copy. A kept ground would roughly halve the replay's average drawing but cut its worst frame by only about 6%, since the dearest frames are still frames that render the HUD again, and cheap still frames would push Auto to the Full sky. Neither was kept: the floor is judged by its worst frame. The saving on a real ISA card is an estimate, 5 to 10 ms on a still frame, and was not measured.

## Tried and not kept, and what comes next

Leaving the sky out under open-air cells that the top face of the row below covers gave the same picture over the replay at four sizes, with both skies and nudged cameras. On the DOS build's code it cost about 10,000 instructions a frame more than it saved, because the test runs in the background loop for every open-air cell and the cells it skips are under the horizon, where the sky is the sea's plain fill, about 1,700 instructions at tick 1190. It was dropped.

The next step on DOS is to keep the sky between frames, written up in `docs/OBLIQUE.md` as persistent extents. The presenter would keep, for each scanline, a short list of spans saying what the last picture showed there (foreground, or which open-air layer was on top, with its offset), a few hundred bytes of view state that the rules never read. A new frame compares the camera and each layer's offset with the last frame's, draws only the spans whose layer moved by a whole pixel or that were newly revealed or covered, and marks as force-rerender extents everything the mask cannot keep exact (sprites and where they were, captions, puffs, the HUD, a door that opened, the first frame, a camera jump, a change of the Sky setting, anything uncertain). When in doubt the span is drawn fresh with the stateless path. The rule is that a frame built with the mask equals the stateless frame pixel for pixel, proved over the whole replay and random cameras, ticks and sizes. The idea of persistent extents came from a friend of the owner. It would also give the skipped-cover test a place where it pays, as part of the extents, and the layers at factors of 10 to 15 out of 256 move a screen pixel only for every 17 to 26 pixels of camera. None of it is built.

A review of Fizz 1's DOS port for performance ideas sorted them into those taken and those not. Taken: the backdrop detail setting with Auto judged in the frame governor's window, and the 486 cycle estimator (#169, above). Measured and not kept: integer sprite placement (#169), a kept scene between frames and a copy of only the spans that changed (#172). Not yet tried: DAC fades with banked translucency. `docs/OBLIQUE.md` records two of Fizz 1's decisions that bear on these: its DOS port keeps the last frame when nothing stepped and lists the per-row coverage mask as its own next step, and it rejected Mode X because its half-speed parallax backdrop and palette lighting change every pixel when the camera moves.

## The shelves

### Catalogued
- The frame figures in the first table come from the `BENCH` lines of the DOS gate in CI (`tools/check-dos.sh`, run by the `parity` job) on the pull requests' heads: #143 ({{pr143.state}}, CI {{pr143.ci}}), #144 ({{pr144.state}}, CI {{pr144.ci}}) and #145 ({{pr145.state}}, CI {{pr145.ci}}). The instruction counts come from `tools/bench-draw.sh`.
- The pictures are unchanged by every optimisation: the pinned frame digests in `fizz-slice-view` and the DOS frame digests (`BENCH FRAME`, compared with the host's by `tools/check-dos.sh`) stand, and the new tests compare the window, the prepared sky, the text loop and the HUD cache with the paths they replace.
- The DOS memory floor is 4 MB: `tools/check-dos.sh` runs `/PROBE` at 16 and 4 MB and `/REPLAY` at `memsize = 4`, and `docs/DOS-CHECK.md` has the free-memory figures (2,008 KB at 4 MB).
- The audio totals of the 1,800-tick replay (7,375, 7,396, 14,682 and 1,392) were pinned at #145's head; #146 counts the speaker's steps from each effect's start and pins 1,398 (chapter 14). After the walk-off whistle (#163) and the wider chicken (#168) the pins are 7,802, 7,823, 15,533 and 1,484, measured at the fixed 26,800 cycles that #170 ({{pr170.state}}, CI {{pr170.ci}}) set for every sound case. The totals are pinned by `tools/check-dos.sh` and were measured one case at a time in the pinned image; the parity hash is `{{ms.parityHash}}` on native, Web and DOS (`tools/check-parity.sh`).
- The channel-5 count file is handed between parallel cases atomically; the stress run of four writers and 2,000 reads is recorded in the commit that changed it.
- The 486 estimate (#169, CI {{pr169.ci}}): `tools/cyc486.py --self-test` runs in `tools/check.sh`, and `tools/bench486`'s frame digests equal `FIZZ2.EXE`'s `BENCH FRAME` lines; the table's figures are one run of `tools/bench-draw.sh --486` recorded in the pull request.
- The changed-span trial (#172, CI {{pr172.ci}}): the frame comparison and the trial `present` are recorded in `docs/PLATFORM.md`; the trial build was not kept, so its `BENCH PRESENT` figures cannot be repeated from the repository.
- The Sky setting is covered by `fizz-machine` tests for the byte, the switches and the Options row, a test that Full and Light differ only under open air, and the governor's window thresholds; the `BENCH SKY` and `BENCH FULL SKY` lines are in the DOS gate.

### Uncatalogued
- Everything about speed on a real 486. The figures are DOSBox's at a fixed cycle count, one cycle for a simple instruction, with no cache, bus, x87 or slow video card; the callgrind figures are instruction counts that agreed with DOSBox to within 1 percent on one comparison; the 486 clocks of #169 are an estimate from the datasheet's cache-hit timings, not a measurement.
- The Auto thresholds (45% and 75%) were chosen from arithmetic on the floor model's figures; no run has watched Auto change the sky on a faster modelled machine.
- The local DOS runs, which used the pinned image on a machine that ran other work; the `BENCH` maxima there are high-side, and the CI figures stand in the table.
- The Fizz 1 DOS performance review is described here from the work's notes; no document in the repository records it beyond the two Fizz 1 decisions in `docs/OBLIQUE.md`.
- Whether the Web draws the Full sky inside its frame on a slow phone or browser: the Web path is the same integer code and was not timed.
- Whether anyone can hear the audio the counts describe; they are register writes, not sound.

### Wish list
- Persistent extents with force-rerender spans for the sky, and the coverage mask that skips sky under terrain inside them.
- A cycle budget gated in CI on the 486 estimate (Milestone 5), and DAC fades with banked translucency from Fizz 1's DOS port.
- A light pass for DOS (palette banks and integer lights, shaded during the copy to video memory) that `docs/OBLIQUE.md` sketches; night mode with palette banks and integer lights is #146 ({{pr146.state}}) and chapter 14.
- Shrinking the baked tables if a machine under 4 MB matters again.

> **From the workbench — Callgrind.** Callgrind is a Valgrind tool that runs a program on a simulated CPU and records how many instructions each function and call executes, with the call graph. Counting instructions gives the same number on every run, which a timer on a shared machine cannot, so a change of a few percent can be seen. The count says nothing about cache or bus behaviour.
> Source: [Callgrind manual, Valgrind](https://valgrind.org/docs/manual/cl-manual.html)
