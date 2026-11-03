# Oblique ground and platforms

How Fizz 2 draws walkable surfaces, walls and blocks in oblique projection, so that ground reads as two parallel edges with depth between them rather than as a flat bar. This page is the specification the renderer will follow. It is a drawing rule only: the tile map, the collision, the state hash and the golden replay do not change.

Status: proposed, for review. The look was worked out on a design canvas (the demo level, the tile kit, Ben on the strip and a hill study) before any code. Nothing here is implemented yet.

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

1. Background: the interior back wall (`w`), or sky and sea in open air (`.`), drawn flat on the far plane.
2. Railings and other far-plane decoration.
3. Terrain, from the bottom row up and left to right within a row. This order makes upper-right neighbours correctly hide the far parts of lower-left faces with no depth buffer.
4. Spikes and the exit, which stand on the feet plane.
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
| `w` | Back wall | Vertical 8 px B boards with k seams, grain and D knots; a k ceiling shadow under stone, a D skirting where it meets a floor, the odd porthole |
| `.` | Open air | A k sky with W, c and y stars, a k/B horizon dither, a B sea with a C crest; the gallery gets an iron railing on the far edge of its planks |

## The exit

The exit doorway stands on the far plane: its sill sits on the far edge of the floor, and Ben walks in front of it. The exit tiles in the map do not move; the doorway is drawn over the far half of the cell above the floor and the cell above that.

This goes with a rules change, planned for the code phase: Ben goes through the exit when Up is pressed (a new press, not one held over) while his body overlaps an exit tile and he is standing on the ground. Touching the exit alone no longer ends the level. The press raises the exit event and freezes Ben as touching does today, and at the exit the door takes the press before the look-up and aim-up uses of Up. Because Fizz 1 exits on touch, the change gets its entry in `docs/FIZZ1-DIFFERENCES.md` when it lands. It moves the golden replay (which gains an Up press at the exit) and its hashes on every platform together, and the manual changes from touching the exit to pressing Up in the doorway.

## Ben

Ben is not redrawn here; his new sprites come from the pose workshop. The trial frames on `trial/lab-ben-sprites` are 33 px tall, one more than two tiles. The suggested trim is sprite row 26 (counting from 0 at the top of the cap), a shin row nearly identical to row 27, which keeps the cap outline, the face and the shoes intact. The run and fire frames share a 33 px box and trim on the same row.

## Hills

Not in the demo level, which has no room for one; recorded here for levels that do. The engine already has 1:2 slopes (`R22A` and `R22B`, two tiles across for one up) and 45° slopes (`R45`, `L45`).

- Hills never rotate. Each column of the strip is shifted straight up; a row at depth `r` takes the slope's height at its own world x, which is the screen x minus that row's depth offset.
- The feet line follows the collision slope, 16 px below the far edge everywhere.
- The near edge reaches a slope 4 px before the feet line and the far edge 6 px after it, 10 px in all. On a 1:2 slope the strip thins to 21 px going up and thickens to 31 px coming down; on 45° it is 16 and 36.
- A level may skip the lead on the way down and keep the strip at 26 (an art shortcut), where the thicker strip reads badly.

A hill 32 px (two tiles) high on a 1:2 slope is the reference case.

## Implementation notes

For the build after this design is approved:

- The rules crates do not change. Drawing reads the same tile map and actor positions it reads today, so native, Web and DOS hashes stay as they are and `tools/check-parity.sh` is unaffected.
- The oblique drawing belongs in `fizz-view`, which both presenters share. It keeps integer arithmetic (the row offsets are a 26-entry table) so it fits the float-free presenter rule.
- The Web view is 208 px tall and the DOS frame 200; both are drawing inputs only. The extra 16 px above a top row and 10 px to the right of a right column are drawn into the view like any other pixels.
- Tile pieces should be baked as sprite-like data under `assets/` rather than generated at run time on DOS, in keeping with "fidelity is lost at bake time".
- The manual gains a line once a build shows the new look.
