# Night and lights

How Fizz 2 draws a level at night: a palette of brightness banks shared by the Web and DOS, markers carried in the art, a small integer point-light model and a shade pass over the finished picture. It is a drawing rule only: the rules, the state hash and the golden replay do not see it, and by day none of it runs.

Status: built in `fizz-slice-view` for the slice, on the Web and on DOS, behind the Night setting (Off by default). The model is `crates/fizz-slice-view/src/light.rs`; the slice's night (its ambient, lamps, lanterns, beam and their art) is in `crates/fizz-slice-view/src/oblique.rs` beside the Sky stack's constants; the tables and pieces are baked by `tools/tiles/emit-oblique.ts`.

## Where it comes from

Fizz 1 has a Night option. Its Web build lights each sprite pixel in a fragment shader; its DOS port draws in a 256-colour mode with a palette of brightness levels and a pass over the frame. Fizz 2 ports the DOS technique, so both presenters draw the same bytes, and takes the light model from the Web build and the simulation:

| Fizz 1 source | What is reproduced |
| --- | --- |
| fizz-dos `m2/dos-rt/src/video.rs` (`light_levels`, `shade`, `falloff`, `BAYER`, `ACTOR_FLOOR`, `MARK_ACTOR`, `MARK_GLOW`) | 8 by 8 cells, quarter levels, the 2 by 2 ordered dither, the falloff table over the squared distance with positions in quarter pixels, markers in the high nibble, the actor floor of level 9 |
| fizz-dos `docs/DOS_ARCHITECTURE.md` (the palette table) | `(L - 1) / 13` of the colour for level `L`, 6-bit DAC values; level 1 black, 14 full |
| fizz-dos `docs/PERFORMANCE.md` | integer lighting (the float version was unstable through x87 spills); fades through the palette; fusing the shade into the copy as a later step |
| `packages/engine/src/renderer.ts` (`FRAG`) | `att = clamp(1 - |d| / r, 0, 1)^2` with `d` including the light's `z` lift, `n = (0, 0, 1)` (no normal maps), actors floored at 0.62 (`vAct`), emissive sprites skip lighting |
| `crates/sim/src/lights.rs` (`LightPool`, `MAX_ACTIVE`, `pin_first`) | the nearest 16 lights to the camera, the player's lantern pinned first |
| `episodes/episode-1/game/src/render.rs` (`update_lights`, `LevelTheme::night`) | Ben's lantern (centre, 1.1 tiles up, `z` 1.2, radius 5.5, warm), static lights' flicker `1.6 + 0.12 sin(9 t + x) + 0.08 sin(23 t + 3 x)` (`z` 1.4, radius 8.5), a cyan light on every Fizz Blaster bubble (`z` 0.6, radius 3), a per-theme night ambient |
| `episodes/episode-1/game/src/levels.rs` (`the_map_lights_fit_the_budget_wherever_the_camera_goes`) | the camera-sweep test of the light budget |

Where Fizz 2 differs, and why:

- **The palette carries the ambient.** Fizz 1's DOS port draws the scene in level 0, a marker, and shades every pixel to its level. Fizz 2 keeps the picture as drawn (bank 0) and gives bank 0 the night's ambient in the palette, so a cell no light reaches is never written. That keeps every day frame byte for byte as it was and makes the pass cost what the lights reach, not the whole screen.
- **Markers are real banks.** The actor marker is the level 9 bank and the glow marker the level 14 bank, so a pixel the pass skips already shows right.
- **One colour, brightness only.** As in fizz-dos, a light's colour is folded into one brightness (`0.3 r + 0.59 g + 0.11 b`): the palette can dim a colour, not tint it. Glows keep their colours (the green doorway, the cyan bubbles).
- **No float at all.** The falloff and the flicker's sines are baked; positions are integers from the start.

## The palette

A frame byte is `bank * 16 + colour`. A level `L` from 1 (black) to 14 (full) is `(L - 1) / 13` of the EGA colour, to the nearest 6-bit DAC step: `(v * (L - 1) + 6) / 13`.

| Bank | Bytes | Level | Use |
| --- | --- | --- | --- |
| 0 | 0–15 | 14 by day, the night's ambient at night | the picture as drawn; every byte drawn before the night exists |
| 1 | 16–31 | 5 | the HUD box's and the menus' dim (`DIM_BASE`), unchanged |
| 2 | 32–47 | 14 | glow: full brightness by day and by night |
| 3–15 | 48–255 | 13 down to 1 (bank `16 - L`) | the shaded levels; bank 7 (level 9) is the actor marker |

`light::palette(base)` builds the 256 entries as a `const`; `light::DAY` is `palette(14)` and the night's is `palette(oblique::NIGHT_AMBIENT)`. DOS writes them to the DAC (`dos_palette`, `dos_palette_for`); the Web widens each value with `v * 255 / 63` rounded (`light::rgba`), which gives exactly the EGA bytes 0x55, 0xAA and 0xFF, so a byte means the same colour on both. The two palettes differ in bank 0 only.

## Markers

The art drawn at night carries its marker in its bank (`fizz_view::blit_sprite_banked`, `Canvas::piece_banked`):

- **Lit** (bank 0): the ground, the back wall, the sky. Takes its cell's light.
- **Actor** (bank 7): Ben, the chicken, its beak and stun stars. Takes its cell's level or 9, whichever is higher, so the characters never go below about 0.62 of full (8/13), Fizz 1's floor.
- **Glow** (bank 2): the keys, snacks and cans, the bubbles and their puffs, the door badges, the exit's opening and arrow, the lamps' and lanterns' glass and flames, and the captions. Never changed. The HUD box (DOS) and the `LEVEL CLEAR` banner draw their own pixels in it too, so they stay as by day; the menus draw over a picture dimmed into bank 1 and are shown in the day palette.

The glowing parts of baked pieces (`LANTERN_GLOW`, `LAMP_GLOW`, `BADGE_RED`, `BADGE_BLUE`, `EXIT_GLOW`) are split from their pieces by the baker and drawn over them, so nothing is read back from the frame.

## The light model

A light is a point in map pixels times 4 (quarter pixels), with a height `z` over the picture's plane, a radius `r` and a strength in units of 1,024 for full brightness. What it adds to an 8 by 8 cell is Fizz 1's shader term for a sprite facing the screen, `strength * z / |d| * (1 - |d| / r)^2`, with `|d|` from the light to the cell's centre including `z`: in fizz-dos's form, `k = strength * z / r` times `FALLOFF[|d|^2 / r^2 * 1024]`, where `FALLOFF` is `(1 - u)^2 / u * 256` at the middle of each of 1,024 steps of `u^2`, so no square root runs. Along a row of cells the squared distance grows by additions.

A cell's quarter level is `(ambient - 1) * 4` plus its added light times 52 / 1,024, rounded down, at most 52 (level 14). Rounding down leaves a light's faintest ring out of the pass. Within a cell a 2 by 2 ordered dither (`[[0, 2], [3, 1]]`) picks each pixel's level between the two quarter levels, so a gradient has four steps between two levels.

`light::Lights` takes up to 32 candidates and selects the nearest 16 to the view's centre, a pinned light first. The slice never has more than 15 reaching the widest Web view, with Ben's lantern and five bubbles out (a test sweeps the camera over the map).

## The slice's night

All view-side constants in `oblique.rs`, documented like the portholes; a later level and theme format will carry them.

| What | Where | Light |
| --- | --- | --- |
| Ambient | `NIGHT_AMBIENT` | level 6 of 14 (Fizz 1's themes have a `night` ambient of 0.14 to 0.36) |
| Lantern over the red door | pillar cell (15, 23), on its front face | `z` 1.4, radius 8.5, 0.75, flickering |
| Lantern over the blue door | pillar cell (22, 41) | the same |
| Lamp in the left shaft | back wall (4, 13) | `z` 1.4, radius 7.5, 0.69, flickering |
| Lamp in the right shaft | back wall (27, 30) | the same |
| The exit's glow | the doorway's middle | green, `z` 1, radius 4, 0.83 |
| The lighthouse beam | level with the gallery's railing, row 8 from the top | `z` 2.5, radius 7, 1.07, sweeping |
| Ben's lantern | his middle, 1.1 tiles over his feet, pinned first | `z` 1.2, radius 5.5, 0.75 |
| Each bubble | its centre | `z` 0.6, radius 3, 0.87 |

The flicker is Fizz 1's from the baked `SINE` table (a whole turn in 256 steps): 6 and 16 steps a tick, the nearest whole steps to its 9 and 23 radians a second, with the light's x as its phase. The beam turns once every 512 ticks: its x is the gallery's middle plus 22 tiles times the sine of its turn and its strength the cosine, so it crosses the view facing the player and is dark while it faces the sea. The sky behind the gallery is black, which no light brightens, so the beam is seen on the railing, the floor, the sea and the headlands it passes. Ben's lantern is Fizz 1's for a theme with a lantern: without it he walks through the dark between the lamps, lit only by his floor of level 9.

The lamps and lanterns are new baked pieces (`LAMP`, a caged bulkhead lamp; `LANTERN`, a lantern on a hook), drawn only at night, so the day picture is unchanged; each is drawn only where the map still has the cell it hangs on.

## Detail and cost

The night has the Sky setting's two detail levels (`DrawOptions::full_sky`), which is Fizz 2's detail level as fizz-dos's detail setting is Fizz 1's:

- **Full** draws the light pass: the lights above, added up and shaded.
- **Light** draws the night palette, the night dressing and the markers, and no light pass: the level is evenly dim and the glowing art and the actors stand out against it.

On DOS the Sky's Auto starts at Light and the frame governor chooses Full on a machine that draws Light in under 45% of a frame, and back if Full goes over 75% (`FrameGovernor::with_auto_sky`), so the same decision covers the light pass. The Web's Sky is Full unless the player picks Light.

What it costs, in instructions of the host build at the DOS build's `opt-level = "s"` over the whole replay (callgrind, read at 26.8 instructions a microsecond; `tools/bench-draw.sh`'s method, and the `frame` example's `script` mode with `--night` and `--light`):

| Picture | Average | Worst frame |
| --- | --- | --- |
| Day, Light sky | 7,423 us | 9,471 us |
| Night, Light | 7,675 us (+252) | 9,856 us (+422 at its worst) |
| Day, Full sky | 8,206 us | 11,409 us |
| Night, Full | 14,201 us (+5,995) | 19,909 us (+8,765 at its worst) |

The DOS figures are in [`PLATFORM.md`](PLATFORM.md#night-on-dos). A shaded cell row is two words read, tested for a marker, ORed and written: about 11 instructions for eight pixels, plus the cell's setup once for its eight rows. The light pass costs what its lights reach, and Fizz 1's lights reach far: at the slice's worst frames 300 to 400 of the 1,000 cells are lit, most of them the faint rings at the pools' edges, which is 4 to 6 ms on the 486DX2-66 floor against the 2.5 ms the worst day frame leaves (14,146 us of a 16,667 us step). Shorter lights do not fit either: with Ben's lantern and the bubbles alone, at a radius of 4 tiles, the worst frame still adds 2.7 ms, because his lantern is in every frame. So on the floor the night is Light, which keeps 60 Hz.

## Pipeline

1. The ground (`oblique::draw_ground_sky`), as by day.
2. The night dressing (`oblique::draw_night`): the lamps, the lanterns, and their glowing parts, the door badges and the exit's opening in the glow bank.
3. The items, the chicken, the bubbles, the puffs and Ben, in their banks.
4. With the Full detail, the light pass: `night_lights` (Ben's lantern, the bubbles, `oblique::night_lights`), `light::add_lights` into a grid on the stack (1,024 cells for a DOS-sized view, 8,320 for the largest Web view), `light::shade` over the frame in place. A pure function, the same on both presenters. A view larger than the largest grid (wider or taller than any the Web asks for) is drawn without the light pass.
5. The captions, in the glow bank. Then the presenter's own: the DOS HUD box (`Hud::draw_glowing`) and the banner.

The shade is not fused into the copy to video memory. In DOSBox a `rep movsd` element costs what any instruction does, so shading on the way to video memory would save about one instruction for each lit word, a few hundredths of a millisecond; on real hardware it would save a pass over the lit part of the frame. It stays a pure pass over the frame, which the tests compare byte for byte; fusing it is left for when it is measured on a real 486.

## Translucency and fades

`light::lower_span(span, by, base)` lowers a shaded span by `by` levels, keeping each pixel's colour: the shadow or fog of a later level. Nothing uses it yet; its test checks it.

`fizz_present_dos::faded(palette, step, steps)` scales the 256 entries towards black: a full-screen fade or dim is 768 DAC writes, not a pass over 64,000 pixels, and the frame keeps the undimmed picture. Nothing uses it yet: the slice has no screen transition, and the menus dim the frame itself so the Web, which has no DAC, and the frame digests see the same thing.

## Tests

- `light::tests`: every palette entry against the 6-bit rule; the day palette keeps the EGA colours and the dim bank; the Web table widens to the EGA bytes; `FALLOFF` and `SINE` against their formulas; the shade table (lit bytes take the level, actors the level or 9, every other byte stays); the word path against byte by byte at many sizes; a cell at the ambient is not written; the dither's four steps; the nearest 16 with the pinned first; a light's term against the shader's formula; random lights and sizes never write outside the grid or the frame; `lower_span`.
- `night_tests`: seven night frames pinned with each detail; day pictures hold only bytes 0 to 15 and a night picture keeps the day's colours; across the replay at two sizes, every actor pixel ends at level 9 or above and every glowing one unchanged; any view size, 1 by 1 to 2,000 by 900, is drawn whole; the light budget over a camera sweep; the flicker's range.
- `fizz-present-dos`: the night table differs from the day's in bank 0 only and the day table is `light::DAY`; the fade.
- `tools/check-dos.sh`: `/BENCH`'s night frames (Light and Full) against the host's digests, and the Light night's worst tick under the 16,667 us step.
- Every pinned day frame and digest stands.
