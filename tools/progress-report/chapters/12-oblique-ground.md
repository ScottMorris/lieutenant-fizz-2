---
id: oblique
n: 12
title: The oblique ground and the night sky
date: 2026-11-03
summary: The demo level redrawn in oblique projection, from the specification (a 26 px strip, the feet line 16 px below the far edge, a 69 degree slant, colour roles from the palette study, the exit as a doorway) through the baked EGA texture tables and the integer renderer to the parallax layers of Lighthouse Point behind the open air, with the Sky setting. Four stacked pull requests, merged into `main` together with the night (#146), then the whitewashed back wall (#148), and Ben and the chicken fitted to the wall's front edge (#166, #168); the frame budget they had to fit is chapter 13.
prs: [142, 143, 144, 145, 148, 166, 168]
---
Until now the slice drew every tile as a flat 16 by 16 square, as Fizz 1 does, so a ledge was a coloured bar and nothing said where along it Ben was standing. This stack gives each walkable surface a far edge, a near edge and a feet line between them, puts a back wall or a night sky behind every cell, and turns the exit into a doorway. It is four pull requests, each on the branch below it: the specification (#142, {{pr142.state}}), the renderer (#143, {{pr143.state}}), the parallax layers with the Sky setting (#144, {{pr144.state}}) and the exit that takes an Up press (#145, {{pr145.state}}). All of them, with the night on top (#146), were merged into `main` together after a rebase onto it; the CI results are the ones the pull requests' heads show, and the pull request table at the end of this page gives the state as `gh` reports it. The rules did not change for the drawing: only pixels moved, and the state hash is the golden's. The exit rule is the one change to the rules, and it moved the golden; chapter 13 tells that part with the audio counts it disturbed.

Three later pull requests changed the look after the merge: the back wall was whitewashed (#148, {{pr148.state}}), Ben is drawn stopping at a wall's front edge (#166, {{pr166.state}}), and the chicken's body was made to fit its sprite (#168, {{pr168.state}}), the one of the three that changed the rules. They are the last two sections before the shelves.

This chapter is the look and how it is drawn. Chapter 13 is what the drawing cost on DOS, how that was brought down, and what broke on the way.

## The design (#142)

The specification came before any code. `docs/OBLIQUE.md` was written against a design canvas (a private page: the demo level, the tile kit, Ben standing on the strip and a hill study) that is at [the design canvas](https://claude.ai/artifact/PNns5aXLVMDegrcue2yAvv), and against the owner's level-design and palette research canvas at [the research canvas](https://claude.ai/artifact/DxttXzJE3WxzzRaGvvPjhV). The first commit stated the rule and held the code until the look was approved; a second commit brought in the palette study's roles and the exit.

| Rule | Value |
| --- | --- |
| Tile | 16 px, already `TILE_PX` in `fizz-view/src/camera.rs`, so no tile resize |
| Ground strip | 26 px, from the far edge down to the near edge of a walkable surface |
| Feet line | 16 px below the far edge, which is the collision top, so rules and hashes do not move; the near edge is 10 px below it |
| Depth across | 10 px; ends and side faces slant 10 across for 26 up, which is 69.0 degrees |
| Plank fascia | 6 px, so a one-way ledge fills its own tile under the near edge and measures 32 px from far edge to fascia |
| Palette | The 16 EGA colours; no new colours |

The registration ties the picture to the rules. Row `r` of a strip (0 at the far edge, 25 at the near) is shifted `round((25 - r) * 10 / 25)` pixels right of the near edge, which is 4 px at the feet line, so a ledge's slanted end crosses the feet line exactly on the tile edge: where the picture ends is where Ben can stand. Faces follow one rule for every edge that meets a surface: the top face is the strip, the front face is flat at true size on the near plane, only a right side face shows (10 px wide, slanted the same way), and doors are prisms like blocks. Drawing goes background, far-plane decoration, terrain from the bottom row up and left to right, spikes, then items and actors, so upper-right neighbours hide the far parts of lower-left faces with no depth buffer. Actors draw over terrain even where a wall's front face would strictly hide 4 px of Ben.

The colour roles come from the owner's palette study. Light falls from the upper left, so tops and left rims are lit and undersides and right faces are shaded. The world is built from the dark colours and light grey (`k`, `B`, `C`, `R`, `N`, `D`, `L`), and the bright colours (`y`, `r`, `b`, `g`, `c`, `m`) and large areas of `W` are kept for what Ben can touch; the exceptions are the lit edge that marks every walkable surface (`y` on planks, `W` on stone) and one or two pixels of stars. Doors carry a white shape badge as well as their colour, a triangle on the red door and a circle on the blue, so colour is never the only cue. The texture set is chosen by tile letter: bracketed plank ledges with grain and a yellow nosing, ashlar that turns to deep stone three tiles from any opening, lit brick, vertical board back walls with a ceiling shadow, a night sky with a dithered horizon.

The exit is a two-tile stone-arched doorway standing on the far edge, lit green inside with a white up arrow, and Ben walks into it with Up. The decision and its consequences for the golden replay are in chapter 13; the entry in `docs/FIZZ1-DIFFERENCES.md` says that Fizz 1 ends the level on touch and Fizz 2 takes a fresh Up press while Ben stands on the ground.

The hill rule (the strip's columns shift straight up on the 1:2 and 45 degree slope tiles, the feet line follows the collision slope) is specified and not built, since the demo level has no hill.

## Baking the textures (#143)

Fidelity is lost at bake time, and the textures are baked. `tools/tiles/emit-oblique.ts` ports the design prototype's hash and rules and writes `crates/fizz-slice-view/src/oblique_data.rs`, which holds each texture as a function of world position tabulated over a 256 px period, so seams line up across cells and between runs, and a row of texture is a copy from a table at the world column of the run's left edge. The block, board and grain sizes divide the period so each pattern closes on itself.

| Table | Size | What it is |
| --- | --- | --- |
| Plank top | 256 by 26, 6.5 KB | Four boards across the strip, grain, joints every 32 px |
| Plank fascia | 256 by 6 | The 6 px under the near edge |
| Stone at three depths | 256 by 64, 16 KB each | Ashlar, ashlar darkening, deep rock, by the distance to the nearest opening (one tile, two, three or more) |
| Brick | 256 by 32, 8 KB | Red bands with a lit top and left on each brick |
| Back wall | 256 by 96, 24 KB | Vertical 8 px boards with seams, grain and knots |

Next to the tables are the pieces: the doors with their white badges, the exit doorway, spikes, railing, brackets, portholes, the side faces and the `o(r)` row offsets. The generated file is 213 KB of source and about 100 KB of data in the executable, against the 1.1 MB budget. CI checks that the committed file equals what the generator writes (`bun tools/tiles/emit-oblique.ts --check`), and the `view` check group covers `tools/tiles/*`.

The renderer, `crates/fizz-slice-view/src/oblique.rs`, is integer-only, allocates nothing, clips every write and replaces the flat tile pass and the door sprite pass in `draw_slice_captions`. It draws a back wall or open sky behind every cell, then ceiling and plank shadows, brackets, portholes, railing and the exit, then terrain from the bottom row up (tops, fronts, right side faces), then spikes. It merges neighbouring cells that look alike into one run and skips a background that four solid cells cover. Web and DOS call the same `draw_slice_*`, so they draw the same pixels, and `tools/check-dos.sh` compares the host's frame digests with DOS's.

The first version wrote 1.42 pixels for each picture pixel over the replay (at most 1.49 in a frame) in about 2,170 row copies a frame, and a test bounds the average at 2.5. With the open air counted as two writes a pixel the figures are 1.68 on average and 2.18 in the worst frame, a gallery frame; `cargo test -p fizz-slice-view overdraws -- --nocapture` prints them. The `frame` example writes a replay tick, or the bare ground at a camera, as a PPM, and the two pictures below were made with it from the shared drawing routine. They are host renders at 320 by 200 with the full sky, not captures of DOSBox or a browser.

<!-- figure shots oblique-indoor.png oblique-gallery.png -->

The tests added with the renderer cover the `o(r)` offsets, the slant crossing the feet line on the tile edge, the feet 16 px below the far edge, every pixel being one of the sixteen EGA colours, no out-of-bounds write at any camera or view size from 96 by 64 to 1280 by 416, the overdraw bound and the generated data. The frame digests in `fizz-slice-view` were re-pinned after the frames were looked at; the door, nook and colour tests were rewritten against the new art, and the replay transcript did not change.

## Backgrounds that belong to the level's theme (#144)

`docs/OBLIQUE.md` says that each level's theme chooses its background. Interior cells (`w`) take the theme's back-wall material, drawn flat on the far plane so it scrolls with the tiles, and open-air cells (`.`) take the theme's parallax stack, which sits behind the back wall, scrolls at its own factors and shows only where a cell is open air. In code the slice's theme is a set of constants in `oblique.rs` (the `lighthouse` stack, its factors, the sprites `MTN_TOP` and `SEA_TOP`, `HORIZON_ROW` and `HORIZON_CAM_Y`) and the baked tables; a later level and theme format will carry them, and nothing yet defines one.

`fizz_view::parallax` is the layer system: platform-neutral, integer, `no_std`, after the vocabulary of Fizz 1's `Layer`. A layer at factor `f` out of 256 is offset by `camera * (256 - f) / 256`, rounded down, with a factor for each axis. There are three kinds, the ones the slice uses.

- **`Stars`** are W, c and y points on a hashed 16 px grid, 2 by 2 pixels one time in five and one pixel otherwise, drawn only where the frame is still black.
- **`Clouds`** are Fizz 1's 24 by 8 cloud, a blue checker inside an ellipse with no blending, in cells 3.5 tiles wide, drifting right 0.2 tiles a second with the animation tick.
- **`Band`** is 16 px columns of stepped heights from a baked table, a 16 by 16 sprite on top and a fill below: the far headlands and the sea.

The stack is the one on the Lighthouse Point parallax board, back to front, over a black clear.

| Layer | Factor x | Factor y |
| --- | --- | --- |
| Stars | 0.04 | 0.04 |
| Clouds | 0.06 | 0.06 |
| Far headlands | 0.12 | 0.06 |
| Sea | 0.18 | 0.06 |

The board's near-rocks layer is left out because the slice has no shore. The headland heights are `round(0.5 + 1.5 sin(ix 0.19 + 1.08) + sin(ix 0.067 + 2.4))` tiles, baked by the generator so no sine runs at draw time, and a test checks the table against the `f64` formula. The horizon is calibrated at the camera Ben has when he stands on the gallery floor (48 px, `HORIZON_CAM_Y`), where the sea's crest lies on the top of map row 39; calibrating at the top of the map would put the horizon behind the gallery floor, since a vertical factor of 0.06 barely moves a layer on the screen. Layers are clipped to the runs of open-air cells, so a frame with no open air pays nothing for them. The tests cover the offsets of each factor, the cloud and sprite shapes, that the layers change only the pixels under open-air cells, that frames repeat exactly, that nothing is written outside the clip, and the horizon's position.

## The Sky setting (#144)

The full night did not fit the 486DX2-66 floor's frame (chapter 13 has the numbers), so the sky has a detail setting after the one Fizz 1's DOS port has for its backdrops. **Full** is the four layers. **Light** is the stars and the sea over the black clear, which keeps the night and the horizon and drops the two layers that cost the most for each pixel they cover; it does not drift. **Auto**, the DOS default, starts Light and lets the frame governor choose Full when a window of frames leaves room. The Web draws Full unless the player picks Light. The setting is `DrawOptions::full_sky` and `fizz_machine::SkyDetail`, saved in `FIZZ2.CFG` byte 8 (0 Auto, 1 Full, 2 Light, so every earlier file reads as Auto), shown as a Sky row on the Options screen of both platforms (Auto, Full and Light on DOS, with `Auto (Light)` or `Auto (Full)` naming the governor's choice; Full and Light on the Web), set at start-up on DOS by `/SKY:AUTO`, `/SKY:FULL` and `/SKY:LIGHT`, and stored in the Web's `fizz2-options` blob. A test checks that only open-air pixels differ between Full and Light.

## The whitewashed wall (#148)

The back wall's boards were bright blue, which fought the night sky, the sea and the actors. Eight colourways were played side by side in lockstep in one page (blue, whitewash, whitewash with black grain, dark grey, black, teal, green and red), and the wall is now whitewashed: light grey boards with dark grey seams, board ends and grain, and black knots. Only the colour letters of `backWall` in `tools/tiles/emit-oblique.ts` changed, and the baked `oblique_data.rs` was generated again. It is a view change: the rules, the hash and the golden did not move, and 17 pinned picture digests in seven `fizz-slice-view` tests were updated from the new output. The host renders on this page were made again from the current tree and show the whitewash.

## Bodies that stop at the wall (#166, #168)

A wall's front face is drawn on the near plane, 4 px left of its collision edge, and the specification let actors draw over it. Ben's sprite is 4.8 px wider than his body, so at a wall on his right about 6 px of his back and hand showed inside the brick. #166 leaves his physics alone and moves only his picture: `oblique::wall_gap` finds a solid cell less than 12 px right of his body, and `oblique::wall_offset` turns the gap into a draw offset that puts his current frame's opaque right edge exactly on the front line at contact, for every pose and both facings, growing a whole pixel at a time over the last 12 px so there is no pop. It is integer code shared by Web and DOS. A wall on his left needs nothing, and the HUD's dodge box includes the offset. Three pinned digests at tick 1450, where Ben rides the pogo stick at the gallery's right wall, moved; nothing else did.

The same document now holds enemies to the opposite rule: an enemy's collision body contains its sprite's opaque pixels plus the 4 px margin on its right, so an enemy stops where it is drawn and needs no correction. The chicken did not meet it. Its body was Fizz 1's 0.9 by 0.6 tiles under a 24 by 24 px sprite, so about 9 px of the bird went into a wall, and a hop could pass through its drawn head unhurt. #168 made its body 28 px (1.75 tiles) wide and 1.5 tiles tall, with the sprite drawn at a constant 12 px anchor for every frame and both facings; the derivation from the pixel data is in the comments of `crates/fizz-ep2/src/chicken.rs` and the departure in `docs/FIZZ1-DIFFERENCES.md`. The Fizz 1 traces still build their chickens at Fizz 1's size (`Shape::FIZZ1`), so the proof of the movement port is unchanged. In the slice the wider body reaches Ben before his shot, so the replay fires 20 ticks earlier; no `EVENT` line appeared or vanished, the shot now stuns at tick 267 and the stomp is at 652, and the golden moved from `eda8a62f036aeab6` to `7f6a1a1248c4b0e8` (chapter 15 has the re-pins either side of it). With the fire at tick 261 and the stun at 267 the replay starts two sounds where it started one, and the DOS audio totals, measured in DOSBox at the fixed 26,800 cycles, rose from 7,606, 7,627 and 15,142 writes and 1,465 speaker steps to 7,802, 7,823 and 15,533 writes and 1,484 steps.

## What is built and where it stands

The four pull requests are {{pr142.state}}, {{pr143.state}}, {{pr144.state}} and {{pr145.state}}, and landed together with the night (#146) as the merge commit `{{pr146.merge}}`. Each showed both CI jobs (`check` and `parity`, the latter running the DOS gate) green at its head. Nothing in this stack has run on real hardware. The DOS and Web screenshots in this archive were taken again from `main` after the rule phase (#156), and show the oblique ground, the whitewashed wall and the wider chicken. #148, #166 and #168 each showed `check` and `parity` green at its head.

## The shelves

### Catalogued
- The specification `docs/OBLIQUE.md` (#142, {{pr142.state}}, CI {{pr142.ci}}): its numbers are the ones the renderer's tests assert (the feet line 16 px below the far edge, the slant crossing the feet line on the tile edge, every pixel an EGA colour).
- The renderer (#143, {{pr143.state}}, CI {{pr143.ci}}): `cargo test --workspace` through `tools/check.sh` runs the frame digests, the out-of-bounds test at every size, the overdraw bound and the generated-data test, and `bun tools/tiles/emit-oblique.ts --check` keeps `oblique_data.rs` equal to its generator.
- The parallax layers and the Sky setting (#144, {{pr144.state}}, CI {{pr144.ci}}): tests for each layer's offsets, the clip, the repeat, the horizon and the heights against the float formula; the Full and Light skies differ only under open air.
- The whitewashed wall (#148, CI {{pr148.ci}}) and Ben's draw offset at a wall (#166, CI {{pr166.ci}}): `bun tools/tiles/emit-oblique.ts --check` and the pinned picture digests; `crates/fizz-slice-view/src/wall_tests.rs` checks that Ben's opaque edge lands on the front line at contact for every pose, both facings and each kind of wall, and that the ramp never moves him backwards.
- The chicken's body (#168, CI {{pr168.ci}}): tests of its size, its spawn, its turns at walls on either side, a 3,000-tick patrol of the slice's ledge, and the drawn sprite inside the body for every frame and facing; the Fizz 1 traces pass unchanged at Fizz 1's size.
- The golden transcript is unchanged by the drawing, and it ends `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}` on native, Web and DOS: `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}` with the figures in `snapshots/measured.json`.

### Uncatalogued
- How the new look reads to the owner or to a player on a screen. The whitewash was chosen from a side-by-side page of eight colourways and played on the Web with the trial Ben sprites; the pictures above are host renders, the frame digests were re-pinned after looking at frames, and no playtest of the stack is recorded.
- The Full and Light skies in a browser or on DOS hardware: the Web path is the same integer code and was not timed, and DOS figures are DOSBox's.
- The design canvases are private pages that this archive links and cannot check.
- The cost of Ben's wall offset on its own: the ground bench draws no actors, so `/BENCH` times it only inside the whole replay's drawing.

### Wish list
- Hills, as specified in `docs/OBLIQUE.md` for levels that have room for one.
- A theme format that carries a level's back wall and open-air stack, in place of the constants in `oblique.rs`.
- The near-rocks layer, for a level with a shore, and Ben's trial frames trimmed to 32 px (sprite row 26).
- Dynamic lights: night mode with palette banks and integer lights is #146 ({{pr146.state}}), stacked on #145 and told in chapter 14.

> **From the workbench — Oblique projection.** In an oblique projection the front face of an object is drawn at true size and the receding edges run off at an angle, in cabinet projection at half their length. It is the projection of most side-view technical sketches and of many early games, which is why a flat front face and a slanted top read at once. The specification keeps the front faces flat and gives the depth edges their own slant.
> Source: [Oblique projection, Wikipedia](https://en.wikipedia.org/wiki/Oblique_projection)

```figures
id: oblique-shots
kind: screenshot
track: shared
title: The oblique ground, indoors and in the gallery
alt: Two 320 by 200 pictures of the slice in oblique projection. On the left, plank ledges with yellow nosings and iron brackets over a whitewashed board wall, a small magenta bottle on the second ledge, spikes on a grey floor and a red brick pier. On the right, the open-air gallery under a starry sky with clouds, grey headlands and a sea behind an iron railing, a green exit doorway with a white arrow and Ben in front of it.
caption: Replay ticks 500 and 1700 drawn by the shared routine at 320 by 200 with the full sky, as host renders.
at: figure shots oblique-indoor.png oblique-gallery.png
thumb: img:media/oblique-gallery.png
```
