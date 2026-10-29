# Scene contract sketch

## Status

This is a proposal, not a decision. It records why the current `Scene` boundary loses fidelity and sketches a richer contract that DOS can still honour. Nothing here is implemented. The music equivalent is a separate note, because the same principle applies to scores.

## Problem

`fizz2_engine::Scene` is a Mode 13h frame description in a trait. It carries a 256-entry `Rgb6` palette, `u8` palette indices and three commands: `Clear`, `Rect` and `Stipple`. The core therefore decides *how a scene is drawn on the weakest target* before any presenter sees it.

The rotating beam shows the cost. `Beacon::rebuild_scene` approximates a cone by stacking `Rect` slices whose `y` offsets are nudged each tick, then picks two palette indices (`BEAM_DIM`, `BEAM_BRIGHT`) for the glow. The intent, "a lighthouse beam sweeps across the sky", exists only as that pile of rectangles. A Web or GPU presenter cannot draw a smooth gradient cone, bloom or additive blend, because by the time it receives the scene the intent is gone. It can only draw the same rectangles with more pixels.

Music has the identical shape. The Web presenter receives OPL2 `fnum`/`block` words, converts them back to hertz and guesses an Undertone waveform per channel. The richer presenter is built from the reduced form.

## Principle

The core emits semantic intent at full fidelity. Each presenter projects it downward, and the projection runs toward the weaker target.

```text
core: what is in the scene, in fixed-point world units
        │
        ├──► Web / GPU presenter: draws it richly
        │
        └──► Mode 13h presenter: quantizes to 256 colours, snaps to pixels, dithers
```

Platform limits (six-bit DAC colour, palette indices, 320 by 200, slot counts) live in the DOS-side backend. They do not appear in types the core or the engine contract exports.

## Proposed contract

The core describes a scene as layered entities in fixed-point world coordinates with true colour and named intent. All numbers are integers so determinism holds on every target.

```rust
/// 16.16 fixed-point world units. One pixel on the DOS target is 1.0.
pub struct Fixed(i32);

pub struct Colour { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

pub enum Shape<'a> {
    Rect { size: Vec2 },
    Polygon { points: &'a [Vec2] },
    Cone { length: Fixed, spread: Angle },     // lighthouse beam
    Sprite { sheet: SheetId, frame: u16 },
}

pub enum Fill {
    Solid(Colour),
    Gradient { from: Colour, to: Colour },     // along the shape's main axis
    Stipple { colour: Colour, density: u8 },   // authored dither, kept as intent
}

pub enum Blend { Opaque, Alpha, Additive }

pub struct Node<'a> {
    pub shape: Shape<'a>,
    pub position: Vec2,
    pub rotation: Angle,
    pub fill: Fill,
    pub blend: Blend,
    pub layer: u8,
    pub role: Role,                            // semantic tag, see below
}

pub struct Scene<'a> {
    pub camera: Camera,
    pub nodes: &'a [Node<'a>],
}
```

`Role` is a small enum of things the game means (`Sky`, `Sea`, `Tower`, `Lantern`, `Beam`, `Player`). Presenters use it for authored behaviour such as bloom on `Beam` or palette pinning for `Player`, so the core never encodes presentation tricks as geometry.

The lighthouse beam becomes one node: a `Cone` with a gradient fill, additive blend and a rotation driven by the same `lighthouse_sweep` function. The twenty-odd rectangles disappear from the core.

## Semantic events

Sound and captions follow the same rule as the scene: the core says what happened and never how it is shown. `fizz_ep2::Event` is the slice's version of that. The rules emit `Fire`, `Dud`, `Stomp`, `Stun`, `Death`, `Key`, `DoorOpen`, `Exit`, `Bounce`, `HeadBump`, `Jump` and `Snack` (and the older `Cluck` and `ShotEnd`), each with a position in sixteenths of a tile, and every presenter picks its own sound, caption and stereo placement. Events are output only and not hashed. The parity transcript prints them as `EVENT` lines, so all platforms agree on when each fired. A richer scene would carry them beside the nodes. Which event gets which sound id and which caption is a table in `fizz-slice-view` (`presentation`), not part of the rules; `EventCaptions` keeps the live words (view state, never hashed).

## Backend responsibilities

| Presenter | Responsibility |
| --- | --- |
| Web / GPU | Draw shapes with true colour, anti-aliasing, additive blending and optional bloom for `Role::Beam`. Interpolate between ticks for smooth motion above 60 Hz. |
| Mode 13h | Allocate a 256-entry palette from the colours the scene uses (or a pinned authored palette), quantize fills, snap positions to integer pixels, rasterize cones as scanline spans, convert gradients and alpha to ordered dither or `Stipple`. |
| Test / host PPM | Use the Mode 13h path so visual regression stays deterministic. |

Palette and dither choices become DOS-backend decisions, with an optional per-scene authored palette hint for art direction. The `Rgb6` type moves into the DOS backend. `fizz2-runtime` stops re-exporting it from the engine.

## Constraints to honour

- **Determinism.** Fixed-point maths only. Cone rasterization and quantization are pure functions of the scene and an integer tick.
- **Budget.** A 486-class target fills the frame in a bounded time. The Mode 13h backend must keep a node-count cap and a fast path for opaque rectangles; the cap lives in that backend, not in the contract. The existing 486 performance notes in Fizz 1 are the reference.
- **`no_std`.** Scene storage stays borrowed slices over core-owned fixed buffers, as it is today.
- **Allocation.** No allocation in the per-tick path on DOS. A Web presenter may allocate.

## Costs and risks

- The Mode 13h backend becomes a real software rasterizer for cones, polygons and sprites, instead of three fill loops. That is the cost of moving the intelligence out of the core, and it is the part to prototype first.
- Dithering and palette choice may look worse than hand-picked indices. An authored palette hint and `Role` tags are the mitigation, and the prototype should judge whether they are enough.
- A richer contract is a larger surface. Keep it to what the vertical slice below needs, and add shapes only when a real scene asks.

## Vertical slice to validate it

Build one scene, the lighthouse beam, through both paths:

1. Replace the stacked rectangles with a single `Cone` node and the sea and tower with simple shapes.
2. Add a Web presenter that draws gradient, additive blend and bloom, interpolating between ticks.
3. Add a Mode 13h projection that rasterizes the cone with dither into the existing 320 by 200 frame.
4. Compare the two against today's frame in the host PPM output.

The slice answers three questions. Does the DOS projection read as the same beam? Does the Web version look noticeably richer? And does adding a third presenter, such as a headless one, require any core change? If the answer to the third is no, the boundary is in the right place.

## Open questions

- Should interpolation between ticks be a presenter feature (previous and current scene) or expressed by the core as velocities?
- Does sprite data belong in the contract, or does `SheetId` stay an opaque handle owned by each presenter's asset loader?
- Which of the existing `DrawCommand` tests should become projection tests in the Mode 13h backend?
