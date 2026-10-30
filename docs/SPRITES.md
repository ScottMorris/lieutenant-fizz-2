# Sprites

Sprites are authored and stored as text grids. The committed grids under `assets/sprites/` are the source of truth that every tool and presenter reads. Fizz 1's TypeScript sprite DSL remains an authoring tool for the sprites it generates; its output is reproducible and is committed as grids.

Until the Fizz Asset Pack format exists (see `ARCHITECTURE.md`, Assets and packs), the grids are lowered to `const` data in `fizz-view`. That data is the Milestone 1 form and is replaced by a pack bake.

## File format

One JSON file per sprite set, `assets/sprites/<set>.json`, in canonical serialisation: fixed key order, 2-space indentation, one row per line, the anchor on one line, no trailing spaces and a final newline. A parse followed by a canonical write gives the same bytes.

Set fields, in this order:

- `set`: the set name, equal to the file name without `.json`.
- `note`: optional text; sets of provisional art say so here.
- `sprites`: the sprites, in drawing-table order.

Sprite fields, in this order:

- `name`: `[A-Za-z0-9_]+`, unique across every file. Generated Fizz 1 sprites keep their Fizz 1 catalog names.
- `w`, `h`: integers in 1 to 255.
- `anchor`: `[x, y]` integers in pixels from the top left, the point that is placed at the entity's position. The default is the horizontal centre and the bottom edge, `[w / 2, h]` (feet and centre).
- `source`: `generated:fizz1`, `generated:ep2-canvas` or `drawn`.
- `rows`: `h` strings of exactly `w` characters, top row first. Each character is a palette letter or `.` for transparent.

Palette letters are Fizz 1's `EGA` table (`packages/engine/src/palette.ts`); the index is the position in this table and the EGA colour number.

| Index | Letter | Hex |
| --- | --- | --- |
| 0 | `k` | `#000000` |
| 1 | `B` | `#0000aa` |
| 2 | `G` | `#00aa00` |
| 3 | `C` | `#00aaaa` |
| 4 | `R` | `#aa0000` |
| 5 | `M` | `#aa00aa` |
| 6 | `N` | `#aa5500` |
| 7 | `L` | `#aaaaaa` |
| 8 | `D` | `#555555` |
| 9 | `b` | `#5555ff` |
| 10 | `g` | `#55ff55` |
| 11 | `c` | `#55ffff` |
| 12 | `r` | `#ff5555` |
| 13 | `m` | `#ff55ff` |
| 14 | `y` | `#ffff55` |
| 15 | `W` | `#ffffff` |

Fizz 1's grids use `null` for a transparent cell; the exporter writes `.`.

## Sources and editing

- `generated:fizz1` sprites are produced by `tools/sprites/export-fizz1.ts` from Fizz 1's `defineSprites()`. An editor shows them read-only; changing one means changing the generator and re-exporting.
- `generated:ep2-canvas` sprites were imported once from the Episode 2 sprite design canvas by `tools/sprites/import-canvas.ts`. They are provisional and carry a `note` saying so. An editor shows them read-only until a set is re-labelled `drawn`.
- `drawn` sprites are edited by hand and are editable in an editor.

A grid is integers and letters, so an editor shows it directly as a `w` by `h` array of palette cells and writes it back deterministically: take the cell letters row by row, emit the sprite object with the fields above in order, and serialise with the canonical rules. Opening and saving a file without changing a cell leaves its bytes unchanged. `tools/sprites/lib.ts` is the reference reader and writer.

## Sets in the repository

- `fizz1-slice.json`: Fizz 1's 16-pixel-wide `ben_stand`, `ben_run1`, `ben_run2`, `ben_jump`, `ben_shoot`, `ben_aimUp`, `ben_aimDown`, `ben_lookUp` and `ben_lookDown` (all 16 by 24, the box of `ben_stand`), `ben_pogo` and `ben_pogo2` (16 by 32; `ben_pogo2` is the squash frame, drawn two rows lower), `keyRed` and `doorRed` (16 by 16), then the snack nook's `keyBlue`, `doorBlue`, `cheezie`, `choc` and `cookie` (16 by 16, anchored at the feet and centre like the red pair; the export script lists them in `SLICE_NAMES` and they are Fizz 1's own `gumdrop('b', 'B')`, `doorTile('b')`, `cheezie()`, `choc()` and `cookie()`, with no Fizz recolour) and `bubble` (8 by 8, the fizz bubble shot) and `stars0` and `stars1` (16 by 8, anchor `[8, 8]`: Fizz 1's two-frame stun stars, `stars(f)` in `episodes/episode-1/src/sprites/enemies.ts`, three yellow plus-shaped stars with white centres that shift 3 pixels a frame, exported from Fizz 1 pinned at `4bce036`) and `puff0`, `puff1`, `puff2` and `puff3` (Fizz 1's shot-end puff, baked: see below). Fizz 1 has no hurt or dead pose for Ben.
- `ep2-ben-a.json`: Ben variant A, 24 by 32 (`benA_pogoUp` and `benA_pogoDown` are 24 by 40): `benA_idle`, `benA_run1` to `benA_run4`, `benA_jump`, `benA_fall`, `benA_shoot`, `benA_hurt`, `benA_pogoUp`, `benA_pogoDown`. Provisional. It is drawn against Fizz 1's physics body, anchored at the feet and centre; the body does not change.
- `ep2-cluckzarg.json`: `cluckzarg_idle`, `cluckzarg_walk`, `cluckzarg_alert`, 24 by 24. Provisional. In `idle` the legs are together; in `walk` the right leg is out to the right.
- `ep2-hud-keys.json`: the key tray of the HUD, four 7 by 7 mini keys anchored at their top left corner, `drawn` by hand: `hudkey_red` (a light red gumdrop with a black dot), `hudkey_blue` (light blue, a bar), `hudkey_green` (light green, a plus) and `hudkey_gold` (a yellow USB stick with a grey plug, a different silhouette). The mark is a second way to tell the keys apart besides colour. The DOS tray draws a held key as its pixels and an empty socket as the sprite's edge pixels in dark grey; the Web tray draws the same grids as SVG (`KEY_ROWS` in `web/hud.ts`, which `web/hud.test.ts` compares with this file). The slice's HUD shows the red and blue ones; the green and gold are in the file for the levels that gain them (`fizz_slice_view::hud::KeyKind`). Provisional.
- `ep2-cluckzarg-extra.json`: `cluckzarg_walk2` and `cluckzarg_beak`, `drawn` by hand. `cluckzarg_walk2` is 24 by 24: `walk` with the opposite stride (the left leg out to the left, the right leg planted), so the slice's cycle of `walk`, `idle`, `walk2`, `idle` steps with both legs. It lives in its own set because the imported `ep2-cluckzarg.json` stays as the canvas produced it. Provisional.

  `cluckzarg_beak` is a 4 by 4 overlay, not a pose: it is drawn over the head of `cluckzarg_idle`, `cluckzarg_walk` or `cluckzarg_walk2` (which share one head) to show the beak open. It covers body columns 20 to 23 and rows 9 to 12 of the 24 by 24 frame: it redraws the lower outline of the closed beak and adds a red mouth, a lower bill and a closing outline, so the bill is five rows tall instead of four. Its anchor is `[-8, 15]`, so drawing it with the same position and flip as the body puts it in place (the anchor is the body's `[12, 24]` less the overlay's offset `(20, 9)` in the body), and a chicken drawn flipped flips the overlay about the same line. The `alert` frame has its own open-mouthed head and takes no overlay. Provisional.

Every sprite in `fizz1-slice.json` and `ep2-cluckzarg*.json` is drawn by the slice. The snack nook's items are drawn by `draw_slice` where the rules put them (`World::snacks`, `World::key_blue_position`): an item's centre is half its sprite above the anchor, and it bobs one pixel every 16 ticks, as the red key does. The blue door takes the red door's brown tile colour under its art. `ep2-ben-a.json` is data only: the slice shows Fizz 1's Ben, and no code reads the `benA_` sprites yet. `cluckzarg_alert` is drawn when the chicken is stunned or touching Ben. While it is stunned `stars0` and `stars1` are also drawn above its head, as Fizz 1 draws them above a stunned enemy (centred 0.3 tiles above the head, so the 8-row sprite ends one pixel above the sprite's top), swapping every 10 ticks: Fizz 1 swaps when `floor(t * 6)` changes, six swaps a second, and 60 Hz / 6 Hz is 10 ticks. They hide with the body during the wake-up flash. This is drawing only: no rule or hash reads it.

## The shot-end puff

Where a shot ends Fizz 1 pushes a 16 by 16 `puff` (`puff()` in `enemies.ts`: a dithered white ring round a dithered light-grey core) that grows from 0.5x by 1.5x a second and fades for 0.3 s, tinted cyan `0x55ffff` for Ben's shots and red for an enemy's in Fizz 1 (Fizz 2 recolours Ben's to its accent, see "The Fizz recolour"). DOS has no alpha or scaling, so the exporter bakes four frames into `fizz1-slice.json`, anchored at their centres and already tinted (white ring to light cyan `c`, grey core to cyan `C`, then recoloured to light magenta `m` and magenta `M` by the exporter's Fizz recolour): `puff0`, `puff1` and `puff2` are 8, 12 and 15 pixels square (0.5x, 0.75x and 0.95x, the last being Fizz 1's size at the end of its life), each drawn with Fizz 1's own `Pen` and the same two ellipses and dither conditions scaled to the size (resampling the 16 by 16 grid would turn its checkerboard solid or empty), and `puff3` is `puff2` thinned to a sparse dither, the stand-in for the fade. The slice has no enemy shots, so there is no red set.

`fizz_slice_view::Puffs` is the view's own fixed list of eight, with no allocation; a presenter calls `observe` after every step, which takes the `ShotEnd` events, and passes the list to `draw_slice_puffs`. A puff lives 18 ticks (0.3 s): `puff0` for ticks 0 to 5, `puff1` for 6 to 11, `puff2` for 12 to 14 and `puff3` for 15 to 17. It stays where the shot ended in the world. It is drawn after the shots and before Ben. Web and DOS draw it into the picture itself, so they show the same puffs.

## The Fizz recolour

Fizz 2 draws everything called Fizz in one accent colour, light magenta (EGA 13, `#ff55ff`, "grape"), defined once in `crates/fizz-view/src/theme.rs`; Fizz 1 draws them in cyan. The recolour is a step of `tools/sprites/export-fizz1.ts`, not a hand edit of the grids: `FIZZ_RECOLOUR` lists, by sprite name, the letters to swap after the Fizz 1 grid is read and before it is written (light cyan `c` to light magenta `m`; cyan `C` and light blue `b`, the puff's core and the shot's body, to magenta `M`), so `fizz1-slice.json` stays a pure function of Fizz 1 and the table, and two runs give identical bytes. It covers `bubble` (Ben's shot) and `puff0` to `puff3`. It also covers `soda`, Fizz 1's soda can (the menus' bullet), recoloured as a whole since the can became grape: its light red body `r` becomes light magenta `m`, and its two light cyan bubble pixels `c` become white `W` (light magenta bubbles would vanish on a light magenta body); the grey rim, the white band and highlight and the black outline are kept. The Web Options overlay's `SODA_ROWS` is the same grid, and `web/options.test.ts` compares it to `fizz1-slice.json`. After changing the table, run the exporter, then `bun tools/sprites/emit-rust.ts`; `--check` stays green only when both are done.

## Which Ben frame the slice shows

`fizz_slice_view::draw_slice` chooses Ben's frame by the rule in Fizz 1's `design/melting/engine.js`. Dead is `ben_jump`, turned about its centre as he falls under Full Motion (Fizz 1's `rot`; the sprite itself is unchanged). On the pogo stick it is `ben_pogo2` during the squash and `ben_pogo` otherwise; the pogo wins over the shooting pose, as it does in Fizz 1. Then the shooting pose while it runs: `ben_aimUp` or `ben_aimDown` when the shot went straight up or down, otherwise `ben_shoot`. Then `ben_jump` in the air, a run frame while his horizontal speed is above 0.5 tiles a second, `ben_lookUp` or `ben_lookDown` while a look timer runs (Fizz 1's `render.rs` `ben_sprite` puts them after the run frames), and `ben_stand` at rest. All are flipped by facing. Fizz 1 adds `|vx| * dt` to a distance counter `anim` every step and shows `ben_run2` when `floor(anim * 1.4)` is odd and `ben_run1` otherwise, so the frame changes every 1/1.4 tiles run. Fizz 1 has no turn or idle animation. Fizz 2's rules keep no such counter and drawing may not add one, so the view uses `floor(|x| * 1.4)` of Ben's position instead: the same cadence, and a different phase only after he turns round. The screenshots in `docs/img` were taken before this change and may show the earlier run frame.

## Build data

`tools/sprites/emit-rust.ts` reads every `assets/sprites/*.json` (sets in file-name order, sprites in file order) and writes `crates/fizz-view/src/sprites_data.rs`: `no_std`, no heap, `const`-initialised.

```rust
pub struct Sprite { pub name: &'static str, pub w: u8, pub h: u8, pub ax: i8, pub ay: i8, pub pixels: &'static [u8] }
pub static SPRITES: &[Sprite];
pub fn sprite(name: &str) -> Option<&'static Sprite>;
```

`pixels` holds `w * h` bytes, top row first: the EGA index 0 to 15, or 255 where the sprite is transparent, as Fizz 1's DOS sprite export. The generated file is committed and begins with a header naming the generator and the files it was made from.

## Commands

Bun runs all of them. The first three need no Fizz 1 checkout and run in the `parity` CI job.

- `bun tools/sprites/emit-rust.ts` regenerates `sprites_data.rs`.
- `bun tools/sprites/emit-rust.ts --check` regenerates in memory and exits non-zero with a diff if the committed file differs.
- `bun tools/sprites/roundtrip.ts` checks that every grid file is canonical; with `FIZZ1` set it also checks that a fresh Fizz 1 export equals `fizz1-slice.json`.
- `FIZZ1=<path> bun tools/sprites/export-fizz1.ts` rewrites `fizz1-slice.json` from a Fizz 1 checkout (default `~/source/liminal-hq/lieutenant-fizz`). It only reads Fizz 1, and two runs give identical bytes.
- `bun tools/sprites/import-canvas.ts <ep2-sprites.json>` documents the one-off canvas import.

The drawing routines in `fizz-view` treat only 255 as transparent. Index 0 is black and is drawn, because the grids use it for outlines. A sprite is placed with its anchor at the entity's screen position, and a horizontal flip mirrors about the vertical line at the left edge of the anchor's pixel column, so an even-width sprite with a centred anchor covers the same box either way.
