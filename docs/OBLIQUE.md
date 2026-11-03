# Oblique ground and platforms

How Fizz 2 draws walkable surfaces, walls and blocks in oblique projection, so that ground reads as two parallel edges with depth between them rather than as a flat bar. This page is the specification the renderer follows. It is a drawing rule only: the tile map, the collision, the state hash and the golden replay do not change.

Status: implemented in `fizz-slice-view` for the slice level, on the Web and on DOS. The look was worked out on a design canvas (the demo level, the tile kit, Ben on the strip and a hill study) before any code. Hills are specified below and not built, because the slice has none.

Where things are: the renderer is `crates/fizz-slice-view/src/oblique.rs`, the textures and pieces it copies are the generated `oblique_data.rs`, which `tools/tiles/emit-oblique.ts` writes (`bun tools/tiles/emit-oblique.ts --check` fails when the committed file is out of date), and `cargo run -p fizz-slice-view --example frame` writes a picture to look at. See [Implementation notes](#implementation-notes).

## Why

Fizz 1 draws every tile as a flat 16 px square, so a ledge is a coloured bar and nothing says where along it Ben is standing. An oblique strip gives each walkable surface a far edge, a near edge and a feet line between them, which reads as a floor without touching the rules.

## The numbers

| Rule | Value | Notes |
| --- | --- | --- |
| Tile | 16 px | Already the case: `TILE_PX = 16` in `fizz-view/src/camera.rs`; the rules count in tiles |
| Ground strip | 26 px | From the far edge down to the near edge of every walkable surface |
| Feet line | 16 px below the far edge | One tile; the near edge is 10 px below it |
| Depth across | 10 px | The far edge is 10 px right of the near edge |
| End slant | 10 across for 26 up | 69.0°; rows step 2 or 3 px a column |
| Feet plane shift | 4 px | 10 × 10 / 26, rounded: the near plane is 4 px left of the feet plane |
| Plank fascia | 6 px | 16 − 10: a one-way ledge fills its own tile under the near edge |
| Wall, block and door faces | the same slant | One rule for every edge that meets a surface |
| Palette | EGA 16 | No new colours; DOS stays in its current palette |

Camera equivalent, for reference: with a half-depth (cabinet) assumption the look-down angle is about 28° (asin(0.5 × sin 69°)).

## Registration

The feet line is the collision top. Everything else is placed relative to the collision geometry of a tile:

- The **feet plane** is the collision geometry itself. Ben's anchor (the bottom of his soles) sits on it.
- The **near plane** is drawn at (−4, +10): 4 px left and 10 px down.
- The **far plane** is drawn at (+6, −16): 6 px right and 16 px up.

Row `r` of a strip (0 at the far edge, 25 at the near edge) is shifted `round((25 − r) × 10 / 25)` px right of the near edge. At the feet line (row 16) that is 4 px, so a ledge's slanted end crosses the feet line exactly on the tile edge: what the player sees is where Ben can stand.

A strip therefore occupies the far 16 px of the tile above its collision tile and the top 10 px of its own tile. A one-way plank uses the rest of its tile for its 6 px fascia, so it looks 32 px thick from far edge to fascia, which is two tiles and Ben's height.

## Faces

- **Top face:** the 26 px strip, drawn where the cell above is not solid. Its left and right ends slant up and to the right by one full depth.
- **Front face:** drawn flat at true size on the near plane. Solid tiles have a 16 px front; one-way planks have the 6 px fascia.
- **Right side face:** 10 px wide, slanted the same way, drawn where the cell to the right is not solid. Left side faces are never visible.
- **Doors** are prisms like blocks: the door art is their front face, and they have a side face. An open door is not drawn.

## Draw order

1. Background, chosen by the level's theme (see [Backgrounds and parallax](#backgrounds-and-parallax)): the back wall for interior cells (`w`), drawn flat on the far plane so it scrolls with the tiles, and for open-air cells (`.`) the theme's parallax stack, which sits behind it, scrolls at its own factors and shows only where a cell is open air.
2. Far-plane decoration: portholes, the shadows and brackets under ledges, railings, and the exit doorway, whose sill is on the far edge of the floor, so the strip drawn next covers its foot.
3. Terrain, from the bottom row up and left to right within a row. This order makes upper-right neighbours correctly hide the far parts of lower-left faces with no depth buffer.
4. Spikes, which stand on the feet plane.
5. Items, then actors.

Actors always draw over terrain. Strictly, a wall's front face would hide the 4 px of Ben nearest to it when he stands against it; readability wins.

## Light and colour

Light comes from the upper left: tops and left rims are lit, undersides and right side faces are shaded. The colour roles follow the Episode 2 palette study: the world is built from the dark colours and light grey (k, B, C, R, N, D, L), and the bright colours (y, r, b, g, c, m) and large areas of W are kept for things Ben can touch. The exceptions are the lit edge that marks every walkable surface (y on planks, W on stone) and 1 or 2 px stars. Doors carry a white shape badge as well as their colour (▲ red, ● blue), so colour is never the only cue.

## Textures

Textures are functions of world position, so seams line up across tiles and the board joints of a top face follow the depth slant without special cases. Masonry courses take block widths of 12 to 28 px over a 256 px period, so large areas don't repeat. The demo level's set, all in the EGA palette:

| Letter | Piece | Look |
| --- | --- | --- |
| `=` | Plank ledge | Four N boards across the strip with R grain and k joints every 32 px, a y nosing on the near edge (the one-way cue), a 6 px fascia, iron brackets and a contact shadow on the wall below |
| `#` | Stone mass | D ashlar of mixed widths with L lit rims and k mortar; three or more tiles from any opening it becomes deep stone (k with D joints and thin B veins), so the playable edge carries the light; L flagstone tops with a W lit edge |
| `B` | Brick block | Red bands: r lit top and left of each brick, R body, a few darker bricks, stone coping on a walkable top |
| `^` | Spikes | A shaded back row and a lit front row standing on the floor strip, W tips the brightest line in the room |
| `D`, `b` | Doors | One piece across the door's tiles: boards, iron bands, a yellow lock, the shape badge on the top tile |
| `E` | Exit | A two-tile stone-arched doorway on the far edge, lit g inside, a W up arrow (see [The exit](#the-exit)) |
| `w` | Back wall | The theme's interior material; the slice's is vertical 8 px B boards with k seams, grain and D knots; a k ceiling shadow under stone, the odd porthole |
| `.` | Open air | The theme's parallax stack; the slice's is the Lighthouse night (stars, clouds, headlands and sea, see [Backgrounds and parallax](#backgrounds-and-parallax)); the gallery gets an iron railing on the far edge of its planks |

## The exit

The exit doorway stands on the far plane: its sill sits on the far edge of the floor, and Ben walks in front of it. The exit tiles in the map do not move; the doorway is drawn over the far half of the cell above the floor and the cell above that.

This goes with a rules change, planned for the code phase: Ben goes through the exit when Up is pressed (a new press, not one held over) while his body overlaps an exit tile and he is standing on the ground. Touching the exit alone no longer ends the level. The press raises the exit event and freezes Ben as touching does today, and at the exit the door takes the press before the look-up and aim-up uses of Up. Because Fizz 1 exits on touch, the change gets its entry in `docs/FIZZ1-DIFFERENCES.md` when it lands. It moves the golden replay (which gains an Up press at the exit) and its hashes on every platform together, and the manual changes from touching the exit to pressing Up in the doorway.

## Ben

Ben is not redrawn here; his new sprites come from the pose workshop. The trial frames on `trial/lab-ben-sprites` are 33 px tall, one more than two tiles. The suggested trim is sprite row 26 (counting from 0 at the top of the cap), a shin row nearly identical to row 27, which keeps the cap outline, the face and the shoes intact. The run and fire frames share a 33 px box and trim on the same row.

## Backgrounds and parallax

Every level's theme chooses its background. Interior cells (`w`) take the theme's back-wall material, drawn on the far plane, so it scrolls with the tiles. Open-air cells (`.`) take the theme's parallax stack, which sits behind the back wall and scrolls at its own factors, and it shows only where a cell is open air.

In code the slice's theme is a set of constants in `crates/fizz-slice-view/src/oblique.rs` (the `lighthouse` stack, its factors, the sprites `MTN_TOP` and `SEA_TOP`, `HORIZON_ROW` and `HORIZON_CAM_Y`) and the baked tables in `oblique_data.rs`. A later level and theme format will carry them; nothing here defines one.

The layer system is `fizz_view::parallax`, platform-neutral, integer and `no_std`, after Fizz 1's `Layer` vocabulary: a layer at factor `f` is offset by `camera * (1 - f)`, so `f = 0` stays fixed on the screen and `f = 1` moves with the tiles. A factor is a number out of 256, each axis has its own, and the offset is `camera * (256 - f) / 256` rounded down (`offset`), so a screen pixel shows the layer's position plus `camera - offset` (`shift`). A draw clears its clip to black and draws the layers back to front; the slice clips it to each run of open-air cells. Three kinds exist, the ones the slice uses:

| Layer | What it draws |
| --- | --- |
| `Stars` | W, c and y points, 2 by 2 pixels one time in five and 1 pixel otherwise, one in three 16 by 16 cells of a hashed grid, only where the frame is still black |
| `Clouds` | The 24 by 8 cloud (a B checker inside an ellipse, no blending) in cells 3.5 tiles wide, 65% of them kept, spread up to 2 tiles along the cell and drifting right 0.2 tiles a second with the animation tick |
| `Band` | 16 px columns of stepped heights from a baked table, a 16 by 16 sprite on top and a fill below: the far headlands (D, the `mtnTop` peak) and the sea (a C crest, a B and k checker, B) |

The Lighthouse Point stack, back to front, from the parallax board (clear colour k):

| Layer | Factor x | Factor y | Notes |
| --- | --- | --- | --- |
| Stars | 0.04 | 0.04 | W, c, y |
| Clouds | 0.06 | 0.06 | two rows of cells |
| Far headlands | 0.12 | 0.06 | D; heights `round(0.5 + 1.5 sin(ix 0.19 + 1.08) + sin(ix 0.067 + 2.4))` tiles, baked in `tools/tiles/emit-oblique.ts` as `HEADLAND_HEIGHTS`; no sine runs at draw time |
| Sea | 0.18 | 0.06 | flat band, B below a C crest |

The board's near-rocks layer is left out: the slice has no shore. Colours follow the palette roles: dark colours throughout, and bright ones only for the stars.

The horizon is calibrated at a camera: at `HORIZON_CAM_Y` (48 px, the view's top when Ben stands on the gallery floor) the sea's crest lies on the top of map row 39. Calibrating at the top of the map would put the horizon behind the gallery floor when Ben is on it, because a layer with a vertical factor of 0.06 barely moves on the screen. From lower in the level the layers have scrolled by their factors and the horizon is higher on the screen, but there the back wall and terrain cover it.

## Hills

Not in the demo level, which has no room for one; recorded here for levels that do. The engine already has 1:2 slopes (`R22A` and `R22B`, two tiles across for one up) and 45° slopes (`R45`, `L45`).

- Hills never rotate. Each column of the strip is shifted straight up; a row at depth `r` takes the slope's height at its own world x, which is the screen x minus that row's depth offset.
- The feet line follows the collision slope, 16 px below the far edge everywhere.
- The near edge reaches a slope 4 px before the feet line and the far edge 6 px after it, 10 px in all. On a 1:2 slope the strip thins to 21 px going up and thickens to 31 px coming down; on 45° it is 16 and 36.
- A level may skip the lead on the way down and keep the strip at 26 (an art shortcut), where the thicker strip reads badly.

A hill 32 px (two tiles) high on a 1:2 slope is the reference case.

## Implementation notes

- The rules crates did not change. Drawing reads the same tile map and actor positions it read before, so native, Web and DOS state hashes and `tools/check-parity.sh` are unaffected; only the pictures (and the pinned frame digests of `fizz-slice-view`) moved.
- The renderer lives in `fizz-slice-view` and not in `fizz-view`, because it reads the slice's tile ids from `fizz-ep2`; `fizz-view`'s `draw_tilemap` and `draw_tilemap_on_sky` stay as they were for other callers. It is integer-only and `no_std` with no allocation, so Web and DOS draw the same pixels (`tools/check-dos.sh` compares frame digests with the host's).
- The textures are baked, not drawn at run time ("fidelity is lost at bake time"). Each is a function of world position tabulated over its period: 256 px across, and 26 rows for a strip, 6 for the plank fascia, 64 for stone, 32 for brick and 96 for the back wall. A row is copied as a run from the table at the world column of the run's left edge, so seams line up across cells and between runs. The generator ports the design prototype's hash and rules, with block, board and grain sizes chosen to divide the period so the pattern closes on itself.
- Stone has three tables, by the Chebyshev distance to the nearest opening (one tile, two tiles, three or more): ashlar, ashlar darkening, and deep rock. The distance is worked out for the whole view at once from a bit mask of the open cells for each column of the view, so a cell's distance is a shift and a mask and not 49 reads.
- Cells outside the map repeat the nearest edge cell, so a view larger than the map (the Web at 1280 pixels wide against a 512 pixel map) is filled like any other part of the picture.
- A cell's background is the back wall for a wall tile, the open sky for open air and the exit, and for anything else (a ledge, spikes, a door, the exposed edge of a solid) the background of the cell above if that is open air and the back wall otherwise. The ceiling shadow falls under a solid cell onto a wall cell; the plank shadow and brackets fall on the wall under a ledge that has a room above it, and a ledge open to the sky gets a railing instead. The portholes are slice dressing, at cells (4, 19) and (19, 33), drawn where the map still has back wall.
- Open air is one function, `draw_open_air`, which draws the theme's parallax stack clipped to each run of open-air cells, so a layer's pixel appears only where a cell is open air and indoor frames never ask a layer to draw. The stack and the calibration are in [Backgrounds and parallax](#backgrounds-and-parallax).
- Cost: every pixel is written once or twice (the replay averages 1.7 writes a pixel, at most 2.2 in any frame, counting an open-air rectangle as two), as about 2,100 copies of a table row or piece row a picture; `docs/PLATFORM.md` has the host figures and says what DOS does not measure here. A cell is skipped as a background when four solid cells cover it.
- The Web view is 208 px tall and 96 to 1280 px wide, and the DOS frame is 320 by 200; both are only drawing inputs. The extra 16 px above a top row and 10 px to the right of a right column are drawn into the view like any other pixels.
- The manual describes what a player now sees.
- A draw reads the map once. The cell window (`Window` in `oblique.rs`) holds the tile of every cell the view reaches and of three cells round them, and for each cell the view reaches its look (kind, whether it has a top face) and its background, packed in one byte, in two arrays on the stack (768 cells for the DOS picture and the Web's default view, 3,840 for the widest, which is 89 by 34 cells with the margin). The backgrounds, the dressing, the terrain and the spikes read it, and none of them asks the map for a neighbour. A view too large for the larger array is drawn without a window, asking the map for each cell as the first renderer did, with the same picture. A cell whose tile and neighbours match those of the cell before it takes its look and background without working them out again.
- Fewer, longer copies. A background run takes in the cells between it and the next of the same background that four solid cells cover, since what is drawn there is never seen. The faces of a run of cells are one copy for each row, with the clip, the table row and the period worked out once for the run and not for each row; a ledge's three rows of shadow are drawn for the whole run of planks, since they reach no pixel a bracket covers. A copy of up to 128 pixels is two moves of fixed size, because a call to `memcpy` costs more than the bytes it moves; the sky and sea are copied from small patterns and not tested pixel by pixel. Pieces carry their opaque runs, baked with them (`rows` and `runs`), so drawing a piece copies runs and tests no pixel, and a side face knows its width, so a face inside the frame is not clipped.
- The numbers are in [`PLATFORM.md`](PLATFORM.md#the-oblique-ground). `cargo run --release -p fizz-slice-view --example frame -- bench` prints host time for the indoor and gallery cameras separately, and `tools/bench-draw.sh` counts the instructions of each of those frames under callgrind and prints the average and the worst for each class, which is the figure to compare between changes.
