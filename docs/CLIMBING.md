# Climbing: ladders and vines

How Ben climbs in Fizz 2: what Fizz 1 does, what Fizz 2 should do, how a ladder or a vine is authored and stored, how a climb starts, moves, ends and hops off, what it raises and hashes, how it meets the wall-kick, the pogo, shots and the chicken, how it is drawn, what a level author must know, and the tests and stages of the build. It owns the climbing semantics. The byte layout of the level section is [`FAP.md`](FAP.md), the order of a tick and the events are [`RULES.md`](RULES.md), the source dialect is [`SCRIPT.md`](SCRIPT.md) and the drawing rule is [`OBLIQUE.md`](OBLIQUE.md).

Status: **specified, not built.** Nothing here is in the code. `fizz-phys` already carries the tile flags and the two tile tests (`LADDER`, `CLIMB`, `TileMap::has_ladder`, `TileMap::has_climb`) from Fizz 1's tile map, and `fizz-ep2`'s `Ben` leaves out Fizz 1's `tick_climb` on purpose (the header of `crates/fizz-ep2/src/ben.rs` says so). The page leads the code, as `RULES.md` does for the rule phase: where it recommends, it says so, and section 14 lists the choices the owner makes before a line is written, each with the answer this page assumes.

Fizz 1 is read at commit `4bce036`, the commit Fizz 2's traces are pinned to. Line numbers below are in `episodes/episode-1/game/src/world.rs` at that commit unless a file is named.

## 1. What Fizz 1 does

### Tiles

Three tile ids carry climbing, set in `episodes/episode-1/game/src/tiles.rs` (`props`) and read through `crates/sim/src/tilemap.rs`:

| Tile | Id | Flags | Meaning |
| --- | --- | --- | --- |
| `RUNG` | 28 | `LADDER` | A ladder rung. Not solid |
| `RUNG_TOP` | 29 | `LADDER` and `ONEWAY` | The top rung. Not solid from the side or below; Ben stands on it as on a plank |
| `VINE` | 36 | `CLIMB` | A crystal vine. Not solid, and climbed freely in every direction |

A ladder is a column of `RUNG` tiles with a `RUNG_TOP` at the upper floor's level. `has_ladder` is true for both `RUNG` and `RUNG_TOP`; `has_climb` is true for `VINE` only. Ladders and vines are map tiles. Nothing else in Fizz 1 describes one.

### The state

`Player` (lines 248 to 259) holds:

- `climb: bool`: Ben is holding a ladder or vine. Gravity and running are off and Up and Down move him.
- `wall: bool`: the hold is a vine (free climbing), not a ladder. The name is Fizz 1's; it has nothing to do with the wall-kick.
- `no_grab: bool`: set by a hop, so Ben cannot grab again until he lets go of Up and Down or leaves the ladder.
- `kick_side` and `wall_t`: the wall-kick's memory (section 8).

Only `climb`, `wall` and `no_grab` are climbing's own. `was_climbing` is not a field in Fizz 1: it is a local in `tick_ben_move` (line 1177), a copy of `climb` taken before `tick_climb` runs, so that the kick test can tell that Ben let go on this very tick.

### The constants

| Name | Value | Where |
| --- | --- | --- |
| `CLIMB_SPEED` | 4.5 tiles a second | Up and Down on a ladder (line 94) |
| `VINE_SPEED` | 3.5 tiles a second | Up and Down on a vine (line 62) |
| `VINE_SIDE_SPEED` | 3.0 tiles a second | Left and Right on a vine (line 63) |
| Hop | `vy = 12.0`, `vx = 5.0 * ax`, `cut = true` | `tick_climb`, the Jump branch (lines 1791 to 1800) |

With `GRAVITY` 60 the hop rises 1.2 tiles if Jump stays held and 0.24 tile if Jump is let go on the next tick (the usual 0.45 cut of an upward speed, lines 1263 to 1266; the jump's own 20.5 is 3.5 tiles). On a ladder Ben moves 0.075 tile a tick, on a vine 0.058 a tick up or down and 0.05 across.

### How a climb starts

`tick_climb` (lines 1748 to 1822) runs first in Ben's movement, from the held and pressed buttons (`h`, `e`). The test cell is the column of Ben's centre, `floor(centre_x)`, and the row `floor(y + 0.5)`, half a tile above his feet. Fizz 1's comment calls it chest height. Not climbing:

1. If `no_grab` is set, it clears when neither Up nor Down is held or when the test cell holds no ladder or vine; while it holds, nothing else in this function runs.
2. Up and Down both held, or neither, does nothing.
3. A grab needs Up with a ladder or vine in the test cell, or Down with Ben standing on the ground over a ladder (`from_top`: the cell under his feet, `floor(y - 0.1)`, is a ladder tile). There is no condition on being airborne, so Ben holding Up as he jumps past a ladder is caught by it.
4. `wall` is set only for a vine and never from the top. Fizz 1's comment says the ladder wins where both are present; a cell holds one tile id and no Fizz 1 tile has both flags, so the case arises only on a grab from the top, where a vine in the test cell does not make the hold a vine.
5. A ladder snaps Ben's centre to the column (`x = column + 0.5 - w/2`). A vine does not move him.
6. A grab from the top also lowers Ben to 0.45 tile above the bottom of the top rung's tile, so he hangs inside the ladder.
7. Speeds are zeroed, `on_ground` is cleared, the pogo is switched off, `climb` is set, and the caption `clink` is raised at Ben's x and 1.0 above his feet (`Cap::Clink`, line 1788). It is the only caption or sound that climbing raises in Fizz 1.

### How a climb moves

After `tick_climb`, the general movement code still runs (acceleration, gravity), and then, if `climb` is set, overwrites the velocity (lines 1275 to 1285): on a ladder `vx = 0` and `vy = CLIMB_SPEED * dir`, on a vine `vx = VINE_SIDE_SPEED * ax` and `vy = VINE_SPEED * dir`, where `dir` is Up minus Down. Collision still runs: a ceiling, a wall or a floor stops Ben like any body, and a one-way tile that is not a ladder top still lets him through from below. Facing still follows Left and Right (`face = ax`), even on a ladder, where it changes nothing else.

### How a climb ends

Inside `tick_climb`, in this order, once Ben is already climbing:

1. **Jump pressed** (the edge `e & JUMP`): `climb` and `wall` clear, `no_grab` is set, and Ben hops (`vy = 12`, `vx = 5 * ax`, `cut = true`, `on_ground` false). Nothing is raised.
2. **The test cell no longer holds what he holds.** A ladder hold needs a ladder tile there; a vine hold accepts a ladder or a vine. If not, `climb` and `wall` clear. For a ladder, if Ben is still moving up (`vy > 0`), the tile below the test cell is a ladder tile and that tile is one-way, he is set standing on top of it (`y = row + 1`, `vy = 0`, `on_ground`): the top rung is a ledge he settles on instead of dropping past. Anywhere else, including the top of a vine and a ladder with no top rung, he simply stops climbing and keeps the speed he had, so gravity takes him from there.
3. **On the ground and pressing Down**: `climb` and `wall` clear (the foot of the ladder).

Death does not clear `climb` in Fizz 1 (`kill_by` clears the mantle and the pogo); a dead Ben runs the death branch, which never reads it, and the respawn builds a new `Player`.

### The hop, the wall-kick and the other guards

Every read of `climb` and `wall` outside `tick_climb`:

| Read | Line | Effect |
| --- | --- | --- |
| `tick_ben` | 1141 | `kick_side` is reset while Ben is on the ground **or climbing**, at the start of the tick, from last tick's `climb` |
| `tick_ben_move` | 1161 | The pogo toggle is ignored while climbing |
| | 1184 | The look timers count only when Ben is idle: on the ground, not on the pogo, **not climbing**, no horizontal input |
| | 1238 to 1243 | The wall-kick needs `!was_climbing && !self.p.climb`: a kick is refused while climbing, and on the tick he lets go (the hop) |
| | 1275 to 1285 | The climb's velocity override |
| | 1302 | The animation counter advances by `|vy| + |vx|` while climbing, by `|vx|` otherwise |
| `tick_head_bump` | 1325 | A climb into a ceiling raises no clunk |
| `tick_mantle` | 1670 | No mantle starts while climbing |
| `fire` | 1839 | Down aims the shot down while airborne **or climbing** (a climber at the foot, still `climb`, aims down too) |

No line of `tick_ben_contacts` (from line 1339) reads `climb`: hazards, locks, keys, snacks and enemies treat a climber like any body at its position. I read this and did not run it. In particular the front probe of a lock reads `face`, so a climber facing a door with the key should open it.

The wall-kick guard has two parts that matter. While climbing, Jump is the hop, and the kick branch is never reached because the kick sits in the `else` of `on_ground` and needs `!was_climbing`. On the tick Ben lets go with Jump, `tick_climb` has already set `climb` false, so only `was_climbing` stops the kick: that is why Fizz 1 keeps the local. A hop towards a wall beside a vine therefore hops (Fizz 1 test `jumping_off_a_vine_beside_a_wall_hops_instead_of_kicking`, `tests.rs` line 3010), and the next Jump press, with the wall still against him, kicks.

### Pogo, shots, mantle and the chicken

- **Pogo.** Grabbing switches the pogo off, and the Pogo key is ignored while climbing (test `climbing_ignores_the_pogo_toggle`, `tests.rs` line 1236). After a let-go the pogo stays off until pressed again.
- **Shots.** Fire works on a ladder or vine. Fire alone shoots level in the facing direction, Up with Fire up, Down with Fire down, and aim and the shooting pose follow (`GAME_DESIGN.md`, "Ladders"). Test `firing_on_a_ladder_goes_where_up_or_down_points` (`tests.rs` line 3060). A shot does not let go.
- **Mantle.** `tick_mantle` runs at the end of the move and starts only when Ben is airborne, off the pogo, not climbing, pressing towards a wall (`ax != 0`) and blocked by it (`hit_x`), with a lip within 0.95 tile (`MANTLE_REACH`) and room above it. Because it reads `climb` after the physics, a hop towards a ledge from a ladder can start a mantle on the next tick, and a Ben holding a vine against a wall cannot.
- **The chicken** (the enemies that touch Ben are all in `tick_ben_contacts`). A contact stomps when Ben's `vy` is below zero and his previous bottom was at least 0.55 of the enemy's height above its feet. A climber moving down has `vy = -4.5`, so he stomps what is under him; the stomp sets `vy = 12`, but the next tick's climb override sets it back, so the bounce is lost and a stunned chicken is stomped again every tick he stays on it. A chicken touched from the side, or from below, hurts him as ever. I read this and did not run it.

### Where Fizz 1 uses it

`GAME_DESIGN.md` lists the levels. Ladders: Zarg Lookout (nine floors by ladders, stairs and a lift), Frosting Spire (twelve floors, ladders alternating sides), the Saucer's bridge. Vines: Crystal Caves and Fudge Bog (each a vine up to a hidden shelf and a cracked wall), Mirror Shafts (a vine inside the wall past gate two), and Whisper Hollow (a vine rope over a fudge pool, and a vine to finish the wall-kick chimney). `playtest.rs` proves Whisper Hollow finishable with climbing and not without it (`whisper_hollow_can_be_finished_by_climbing`, `whisper_hollow_cannot_be_finished_without_climbing`).

Fizz 1's art uses two Ben frames, `ben_climb1` and `ben_climb2` (`sprites.rs` line 23), chosen by `floor(anim * 1.6) & 1` in `render.rs` (line 1443), so a frame lasts 0.625 tile of travel; firing up or down shows `ben_aimUp` or `ben_aimDown` instead. A vine uses the same two frames as a ladder.

## 2. What Fizz 2 should do

**Fizz 2 plays climbing as Fizz 1 does.** The first build is a port, not a redesign: the same tiles with the same flags, the same constants, the same order of tests and the same state, proved against Fizz 1's own simulation by trace tests (`AGENTS.md`: a Fizz 2 behaviour that reproduces Fizz 1 names the source and proves the match). The reasons are those of the wall-kick (`FIZZ1-DIFFERENCES.md`): Fizz 1's levels were built with it and the Fizz 1 conversion needs it; Whisper Hollow is finishable only by climbing (`playtest.rs`).

The differences are on purpose and all outside the rules:

| Difference | Fizz 1 | Fizz 2 | Why |
| --- | --- | --- | --- |
| Events | One caption, `clink`, at the grab | A grab event and a release event in the transcript, and a presenter watch for the climb steps | Presenters get the sounds and captions without a rules change; see section 6 |
| Hash | Not applicable | Climbing's three flags are hashed | The hash must carry everything that shapes later ticks; see section 7 |
| Drawing | Flat 16-pixel tiles and two frames | Oblique far-plane art and, on the Web, extra life in it | See section 9 |
| Authoring | A tile id in a Rust level | A legend letter in `script fizz2 0`, and lints | See sections 3 and 11 |

Three owner rules apply:

- **The Web build is richer than DOS and DOS never limits the design** (`AGENTS.md`; the owner's standing decision). There is one set of rules for both. The Web may draw swaying vines, rung glints, dust and a smoother climb animation; DOS draws the plain tiles and the two climb frames. No rule, speed or reach depends on the presenter, and no limit of the 486 (a sound without a speaker form, a palette entry) shortens what a level may do with ladders.
- **A character's hit box matches its sprite.** Climbing does not add a box. Ben's body stays the 0.7 by 1.4 tile body of every other pose, and the climbing frames must sit inside the same footprint Ben's other frames use, so that what a player sees touching a rung is what the rules test (sections 9 and 11). The ladder and vine tiles are full-tile art, so a grab test on a tile is a grab test on what is drawn.
- **The state hash stays deterministic**: `no_std`, `f64` and basic IEEE operations only, no `exp` family, nothing reordered. Climbing adds no new arithmetic: it sets two velocities to constants times a sign, compares a few tiles and clears flags.

## 3. The tile model

**A ladder rung, a ladder top and a vine are tile kinds, not object kinds.** The reasons:

- They are terrain: they sit in the map's cells, a level is drawn as a grid of them, and the author sees and edits them in the map rows beside walls and planks.
- Fizz 1 stores them as tiles, so a conversion maps tile ids to legend letters one for one and no geometry changes.
- The physics already reads them as tiles (`TileMap::has_ladder`, `has_climb`), through a property table that exists.
- The oblique renderer draws the map by tile.
- An object kind would be a second description of the same cells (32 bytes a placement for a centre and a parameter, where a ladder is a run of cells), would need a rectangle form the object record does not have, and would still need tile art under it.

Three tile ids, using Fizz 1's numbers so that the Fizz 1 tile table and Fizz 2's agree (Fizz 2's other ids already follow Fizz 1's):

| Letter (proposed) | Tile | Id | `fizz-phys` flags |
| --- | --- | --- | --- |
| `H` | `RUNG` | 28 | `LADDER` |
| `T` | `RUNG_TOP` | 29 | `LADDER` and `ONEWAY` |
| `v` | `VINE` | 36 | `CLIMB` |

The letters are the owner's to choose (section 14, question 1). `H` for a rung looks like one, `T` for the top, `v` for a vine; none is in use (`.`, `#`, `B`, `=`, `^`, `D`, `b`, `E` and `w` are in the Fizz 2 legend, and Fizz 1's trace tool uses `.`, `#`, `B`, `=`, `^`, `~`, `/`, `\`, `a` to `d` and `D`).

**What it costs.** In the `LEVL` section: nothing. The tiles part is one byte a cell and a tile id is a byte, so a ladder ten tall costs the ten bytes the cells already cost. There is no record, no object, no binding, no state and no new limit. The `LEVL` section version stays 2 and `LEVL_HASH` does not change. `RULES_SEMANTICS` stays 2: no rule byte comes to mean something else.

**What it does to the compatibility number.** A world or shared pack's number includes `tiles::pack_content()`, which holds the tile property table (flag bytes by tile id) and the named tile ids (`FAP.md`, "The compatibility number"). Giving three ids flags changes three flag bytes, and naming them in the id list grows that list (the 594 bytes become 597 if all three are named). So **adding the tiles changes the compatibility number of every world and shared pack, once**, and the pins in the `fizz-pack` tests move with it. No pack has shipped, so nothing is invalidated. The music files' numbers do not move (they hold no content bytes), and neither does any `LEVL` byte of an existing level.

**What it does not do.** The tiles are never bound to a state: a door binding covers switched tiles only. A ladder that appears or a vine that grows on a rule is out of scope; it would be a new binding kind, and `FAP.md` has the numbers reserved for that kind of work (section 14, question 10).

**Rules of the cells.** The ladder column and its top follow the shape Fizz 1's test asserts (`a_ladder_top_is_a_ledge_and_down_goes_back_onto_the_ladder`):

- A ladder's lowest `RUNG` sits in the first row above the floor it starts from. Ben standing on that floor has his test cell (half a tile above his feet) in that row, so Up grabs.
- A ladder's `RUNG_TOP` replaces the floor tile of the upper level in the ladder's column, so its top face is the floor Ben stands on. The tiles between are `RUNG`.
- A ladder that ends without a top rung has no ledge: a climber who runs out of it drops, as from a vine.
- A vine is any run or patch of `VINE` tiles, of any shape. It has no top and no foot.
- A ladder or vine is climbed by the column of Ben's centre, so a one-tile-wide ladder is climbed from the middle of the tile, and a vine one tile wide is held by the middle of the body.

## 4. Controls

No new button. Climbing uses Up, Down, Left, Right, Jump and Fire as in Fizz 1:

| Input | Not climbing | Climbing a ladder | Climbing a vine |
| --- | --- | --- | --- |
| Up | Grabs a ladder or vine at the test cell (anywhere, standing or airborne) | Up the ladder, 4.5 tiles a second | Up the vine, 3.5 |
| Down | Grabs a ladder from its top, standing on the ground | Down, 4.5 | Down, 3.5 |
| Left and Right | Walk or steer; set the facing | No movement; set the facing | Across the vine, 3.0 |
| Jump | A jump, or a kick if airborne against a wall | The hop: lets go, up 12 and 5 sideways | The same hop |
| Pogo | Toggles the pogo | Ignored | Ignored |
| Fire | A shot (level, up with Up, down with Down while airborne) | The same, and Down aims down also at the foot | The same |

Up and Down together do nothing: not a grab, and not movement. Up is also the exit's button (`FIZZ1-DIFFERENCES.md`), `use`'s (`RULES.md`, planned) and the look-up's; section 11 says how they stay apart.

## 5. The rules, as the code reads them

This section is the specification of the port. It keeps Fizz 1's order, because the state hash after each tick depends on it, and it names `fizz-ep2`'s places where Fizz 1's code goes.

**The state.** `Ben` gains three flags beside `kick_side` and `wall_t`, with Fizz 1's names (the port keeps `Player`'s names, `ben.rs` header): `climb`, `wall` and `no_grab`, all `false` at the start of a life. `was_climbing` is a local of the tick, taken before the climb step and read only by the kick guard; it is not state and not hashed. If a change that adds the kick hooks keeps it as a field, it is scratch: written at the top of the tick from `climb`, never read by anything on a later tick.

**The tick**, in Ben's own movement (step 2 of `RULES.md` section 7; climbing needs no new phase):

1. As today: the start-of-tick decrements, then `kick_side` is cleared when Ben is on the ground **or `climb`** (Fizz 1 line 1141, from last tick's `climb`).
2. The pogo toggle is ignored if `climb`.
3. `was_climbing = climb`; then the climb step, which can grab, hop, or let go, in the order of section 1.
4. `face` follows Left and Right; the look timers count only when Ben is on the ground, off the pogo and not `climb`, with no horizontal input.
5. The ordinary horizontal acceleration, the ground jump and pogo, the kick (refused when `was_climbing` or `climb`), the jump cut and gravity run unchanged.
6. If `climb`: the velocity override of section 1.
7. Fire; the head box; the physics; the head bump (silent while `climb`); the mantle test (refused while `climb`, when a mantle exists).
8. Contacts as today.

**What the step may change**, once: `climb`, `wall`, `no_grab`, Ben's `x` (a ladder grab), his `y` (a grab from the top, or settling on a top rung), his velocities, `on_ground`, `pogo` and `cut`. Nothing else.

**Where Fizz 2's code differs from Fizz 1's.** The tile functions are Fizz 1's (`fizz-phys` has them). `Body::on_ladder` and `Body::ladder_below` in `fizz-phys` are not what `tick_climb` uses (it tests the single cell at `floor(centre_x), floor(y + 0.5)` and, from the top, `floor(y - 0.1)`), and the port does not use them either. The look-timer `idle` and the kick and fire guards each take the `climb` test of the table in section 1.

**Departure to decide.** The Fizz 2 `Ben` has no `anim` counter (the presenter derives frames), so the climbing animation is the presenter's, from Ben's position (section 9). Nothing in the rules reads `anim`.

## 6. Events

Events are output only and are not hashed (`RULES.md` section 10 and 11). Climbing raises two, in step 2 of the tick with Ben's other events, and the transcript prints them as `EVENT` lines:

| Event | Raised when | Where | Caption and sound |
| --- | --- | --- | --- |
| `Grab` | A grab succeeds, ladder or vine, from the ground, the top or the air | Fizz 1's point: Ben's x and 1.0 above his feet (after any snap) | Fizz 1's `clink` caption; a short metallic tick on the Web and a speaker click on DOS |
| `Release` | A climb ends for any reason: hop, running out of ladder or vine, the foot of a ladder, or death | Ben's position at the end of the step | None in the first build; carries `hop: bool` so a presenter can tell the hop from the rest |

`Grab` is Fizz 1's one climbing caption, kept. A shared caption and a sound mean a new shared caption id and a new effect id: `CAP_COUNT` and `EFFECT_COUNT` (`crates/fizz-ep2/src/cues.rs`) each grow by one, and both enter `pack_content()`, so the compatibility number moves with the tiles, in the same change. If the owner would rather a silent first build, the events can still be raised and the id left out (section 14, question 4).

**The climb steps are not an event.** A step sound or a rung tick while Ben climbs is derived by the presenter from his position, like the long fall's whistle (`fizz_slice_view::FallWatch`: the rules, the transcript and the hash do not know it). A `ClimbWatch` in the view crate counts Ben's travel while `climb` is set and plays one step each time the climb frame changes, which is every 0.625 tile of travel (Fizz 1's `anim * 1.6` rule, section 1), so a vine's slower speed gives a slower rhythm for free. It needs only `climb`, `x` and `y` from the world's read-only view of Ben. Putting steps in the rules would add an event every eight ticks to the transcript for no rule's sake.

The hop raises the ordinary jump sound on the Web through the `Release { hop: true }` event, as the kick's presenters will; a later change may add a caption.

Rules cannot read these events as inputs. A rule that needs to know Ben is on a ladder uses an `enter` rectangle over the ladder. A `climb` input is not proposed.

## 7. The hash and the golden

**What enters the hash.** `climb`, `wall` and `no_grab` shape every later tick (a hop's lock, a climb's speed), so they are hashed. The recommended place is **the Ben flags byte** (`hash` in `crates/fizz-ep2/src/world.rs`: bit 0 pogo, bit 1 cut, bit 2 on the ground, bit 3 bumped): bit 4 `climb`, bit 5 `wall`, bit 6 `no_grab`. The field count and the byte count do not change, and the byte equals today's byte in every state where all three flags are false.

**The golden.** The slice replay never climbs and its level has no ladder or vine tile, so the three bits are zero for all 1,800 ticks: `FINAL TICK 1800 HASH eda8a62f036aeab6` and `TICK 0 HASH af2259b26b871ca4` are unchanged, and so is every other line of `slice-replay.txt`. **Recommended: keep it so.** The replay is a parity artefact, and every platform change to it costs an owner-visible re-pin; climbing does not need it. A build that does move the golden (by putting a ladder in the slice level, or by hashing a new byte) re-pins it in its own change, with the reason, as `PLATFORM.md` records for each earlier move.

**When the golden moves.** Only if the replay climbs, or the hash layout changes:

- Hashing the flags in a byte of their own (rather than in free bits of the existing byte) would move the golden at once, for no gain. Do not.
- Adding a ladder to the slice's playground and a climb to the script would move it, and is a choice for the owner (section 14, question 3). The recommended path gives climbing its own replay (section 12) and leaves the slice's script alone.

The tile ids are not hashed: tiles are level data, and the hash covers the switches and what moves, not the map. Presentation (the climb frames, the `ClimbWatch`, the events) is not hashed.

A test pins the new bits and extends `PLATFORM.md`'s "changing any one field changes the digest" check to the three flags.

## 8. The wall-kick

`kick_side` and `wall_t` exist (the wall-kick change, `ben.rs`) and are hashed. Climbing adds the hooks the kick's comments name:

- **A kick is refused while climbing** and **on the tick of letting go.** The kick branch needs `!was_climbing && !climb`. While Ben climbs, Jump is the hop; the tick he lets go, the hop has already cleared `climb`, so only `was_climbing` stops the kick.
- **`kick_side` resets while climbing**: the start-of-tick clear becomes "on the ground or `climb`". The `Ben::kick_side` comment in `ben.rs` already names it: "Fizz 1 also does so when he grabs a ladder or vine". The reset happens at the start of the next tick, so a grab clears it one tick later; since no kick can happen while climbing, the delay is unobservable.
- **The hop does not touch `wall_t` or `kick_side`.** A kick after a hop still obeys the quarter-second cooldown of the last kick, and the side memory the last grab cleared.

Consequences a level author must know (section 11):

- A grab refills the kick: one kick off each side, again, after any vine or ladder. By the code, a single wall with vine patches in it can be climbed (kick, grab the next vine, hop, kick the same wall again); I did not find a Fizz 1 level or test that does it (Whisper Hollow's chimney is two facing walls with a vine to finish on). The shaft rule (walls three tiles apart) is the rule for kicks alone, not for walls with vines.
- A Jump press never kicks and hops in the same tick; a press while climbing always hops.
- A hop towards a wall beside a vine hops (does not kick), and the next press kicks.

## 9. Presentation

**Sprites.** Ben needs, for the shipping set: `climb1` and `climb2` (the two hand-over-hand frames), and the climbing shot poses already exist as `aimUp` and `aimDown`, which Fizz 1 shows instead of the climb frames while a vertical shot's pose runs; a level shot while climbing uses the climb frames (Fizz 1's choice). The frames come from Fizz 1's `ben_climb1` and `ben_climb2` through the sprite exporter (`tools/sprites/export-fizz1.ts`, with the Fizz recolour), 16 by 24 like `ben_stand`. Ben variant A (`ep2-ben-a.json`) has no climbing frames; if variant A ships they are drawn with it. Three tiles need art: `RUNG`, `RUNG_TOP` and `VINE`.

**The hit box and the art.** The body does not change. The climbing frames must not reach wider than the body plus the margin the other Ben frames already use (`OBLIQUE.md`: Ben alone has a body narrower than his sprite, and the view offsets him at a wall). A climber's hands and feet are on the rungs, so the frames stay inside the 16-pixel tile. Whether the view's wall offset for a wall on Ben's right applies to a climber who has a wall beside him (a vine against a wall) is a drawing question for the build; it is view-only and never moves the rules.

**Oblique.** A ladder or a vine is wall furniture, so it is far-plane decoration in the draw order of `OBLIQUE.md` ("Draw order", step 2), like the portholes and the exit doorway: drawn after the background and before the terrain, on the far plane, so Ben, an actor drawn over terrain and never hidden by it, climbs in front of it. The rungs and rails fill the 16-pixel tile cell, so the cell reads as climbable wherever the rules test it. A `RUNG_TOP` is drawn as a plank (the 6-pixel fascia) with the rails continuing below it, because Ben stands on its top face like any one-way plank, and the plank's lit edge says so. A vine is drawn as strands across the full cell. Neither adds a side face, a top strip or a shadow of its own beyond the oblique tile kit's. They are not solid, so no neighbour's face is hidden by them.

**The Web is richer.** The Web may add motion and light that DOS skips: vines that sway a pixel with the tick, a glint on a rung Ben has just passed, a puff of dust at a grab. All of it is a function of the world's tick and Ben's position, so it does not affect the hash and parity holds. The Web's night lighting treats a ladder like any far-plane tile. DOS draws the tile art as baked and Ben's two frames.

**Captions and sounds.** The `clink` caption and its effect (section 6). A caption on DOS draws in the 8-by-8 font; a sound without a speaker form plays nothing there, which is the reduction DOS may take (`RULES.md` section 10). The climb steps are the presenter's `ClimbWatch`. The effect's three forms are specified in `SOUND.md`'s style when it is built.

**Camera.** Climbing has no look-up or look-down (the timers need Ben idle), and the camera follows him as it follows any body. A climb at 4.5 tiles a second is slower than the fall's 22, so no new camera rule is needed.

**Manual.** A change a player can see or do updates `MANUAL.md` in its PR (`AGENTS.md`, "Game Manual"). The manual describes shipped builds only, so the section on climbing arrives with the first shipped level that has a ladder or a vine and not with the first code; the PRs before that say why no manual change is needed.

## 10. The other mechanics

- **Mantle.** Not built in Fizz 2, and Fizz 1's guard is `!climb`. Mantle is specified after climbing and reads `climb` where Fizz 1 does (section 1). The two meet at a hop towards a ledge: Ben's `climb` is false after the hop, so a mantle may start on the next tick; and a climber pressing into a wall cannot start one. The port's `ben.rs` header and `PLATFORM.md` ("Mantle is not ported") say that the trace tool refuses scenarios that trigger a mantle: climbing scenarios must keep clear of lips within 0.95 tile.
- **Pogo.** As section 1: grab switches it off, the key is ignored while climbing, letting go leaves it off.
- **Shots.** Fire is untouched by a climb, and aims down at the foot of a ladder too (`fire` reads `climb`). Shots fly from the climber's body.
- **The chicken.** Fizz 1's contacts, unchanged: a climber stomps what he is above while moving down and the bounce is lost to the next tick's override; a side touch hurts him. Section 14, question 5.
- **Doors and keys.** A climber facing a door with the key opens it (a lock reads `face` and Ben's box, never `climb`). A switched tile that closes on a climber is physics' business and is not specified by Fizz 1; the port does what the body does and a level author keeps a gate from closing on a ladder.
- **The exit.** The exit takes Up pressed while standing on the ground, and a grab takes the same Up before the contacts run, so a ladder and an exit in the same column would lose the exit. Section 11 forbids the pair.
- **Moving platforms.** Not in Fizz 2 yet. Fizz 1 carries a body on a lift; whether a climber's `on_plat` is carried is a question for the platform work, which reads this page then.
- **Death.** The respawn builds a fresh Ben, so the three flags are false. The kill clears `climb` and `wall` itself so the dying frame is the tumble.

## 11. For the level author

**The legend.** Three letters (section 3) go in the legend table of `SCRIPT.md` when climbing is built; the dialect needs **no new statement and no new object kind**. A ladder is `H` rows with a `T` on top; a vine is `v` cells. The canonical printer, the validator and the lowering treat them as any other tile.

**A Fizz 1 conversion** maps `RUNG`, `RUNG_TOP` and `VINE` to `H`, `T` and `v` cell for cell. Nothing about a ladder or a vine is level data beyond the tile.

**Reach checks.** A tool that says whether a route exists must know climbing. What a climber can reach:

- A ladder from any floor that meets its lowest rung, and from the ground to its top rung; Ben at the foot grabs it with Up and stands on the top with the settle.
- Off a ladder: the hop, 5 tiles a second sideways and up to 1.2 tiles, or the top's ledge, or the foot.
- A vine from any cell of the patch, and across it at 3 tiles a second. Off the top of a vine Ben keeps the climb's speed (3.5 up) and rises about 0.1 tile more (3.5 squared over 2 times 60) before gravity takes him; off a ladder with no top rung, about 0.17. A shelf at a vine's top should be within that, or reached by the sideways speed.
- The wall-kick budget refills at every grab (section 8).

**The shaft rule** is the kick's (`FIZZ1-DIFFERENCES.md`): two facing walls at least three tiles apart can be climbed by kicks alone. A reach check that asks whether a shaft is climbable must also count a ladder or a vine in it, and the kick budget a grab refills (section 8). The last lint below is that check.

**Lints, proposed.** Codes assigned in `SCRIPT.md` when built; they are warnings, not errors:

- A ladder whose top is not a `T`, and a `T` with no `H` under it: the first leaves no ledge, the second a floating plank.
- A ladder whose lowest `H` is not one row above a floor or a `T`, so Up from the floor does not grab.
- An exit tile in the column of a ladder or a vine, where the grab would take the Up press (section 10).
- A vine or ladder under a ceiling within the climb's reach, where a climber stops with nothing to stand on.
- A switched tile that can close on a ladder or vine.
- A shaft (two facing walls) too narrow to kick and holding no ladder or vine, a reach check for the author's tool.

**An exit, a switch or a key beside a ladder.** Up grabs a ladder at the cell it is in; an exit or a `use` object in the same column would never fire, so the lint forbids the pair. The planned `use` input shares Up with the grab (`RULES.md`, open question 4).

**A death on a ladder** loses nothing but the climb: the level resets as always.

## 12. Tests and proofs

A build must have all of these, and no weaker substitute.

**Unit tests in `fizz-ep2`**, ported from Fizz 1 with the same inputs and the same assertions:

- `ben_climbs_a_ladder_with_up_and_hops_off_with_jump`, `a_ladder_top_is_a_ledge_and_down_goes_back_onto_the_ladder`, `up_and_down_do_nothing_away_from_a_ladder`, `climbing_ignores_the_pogo_toggle`, `jumping_off_a_ladder_with_up_held_does_not_grab_it_again`
- `ben_free_climbs_a_vine_in_every_direction_and_lets_go_when_it_runs_out`, `a_ladder_still_snaps_to_its_column_where_a_vine_does_not`, `a_jump_off_a_vine_hops_clear_and_does_not_regrab_with_up_held`
- `jumping_off_a_vine_beside_a_wall_hops_instead_of_kicking`, `firing_on_a_ladder_goes_where_up_or_down_points`
- A kick-reset test: kick, grab a vine, hop, kick the same wall again (the second kick is allowed only because of the grab).
- A kick-refusal test for the let-go tick on a ladder as well as a vine, and for running out of the ladder with Jump not pressed (the next press kicks).
- A look-timer test: Ben on the ground with `climb` set does not count a look.

**Trace tests against Fizz 1.** `tools/fizz1-trace` runs Fizz 1's own `World`, writes a trace and the tests compare every tick bit for bit. Climbing needs:

- Tile letters in the `.scn` legend for the three tiles (Fizz 1's `RUNG`, `RUNG_TOP`, `VINE`), added to its table.
- A **trace format 3** that adds `climb`, `wall` and `no_grab` as one column of three bits, after `aim`. The precedent is format 2: the older traces are regenerated with their old columns byte for byte unchanged and the new column appended, and `--check --all` proves it.
- Scenarios that keep clear of the mantle (the tool fails loudly otherwise): `ladder_up` (grab with Up, climb, settle on the top), `ladder_down_from_top`, `ladder_hop` (hop, the `no_grab` lock, release Up and Down, grab again), `ladder_pogo` (the pogo toggle ignored; a grab clears it), `ladder_fire` (Fire up, down and level while climbing, the `aim` column), `vine_free` (all four directions and out the top), `vine_hop_wall` (a hop beside a wall, then a kick), `climb_kick_reset` (kick, grab, hop, kick again), `ladder_stomp` (a descending climber over a stunned chicken, the lost bounce). Each is also a Fizz 2 test that runs the scenario and compares the trace.

**The hash.** The flags byte pinned for each of the three bits; the golden `slice-replay.txt` unchanged (a check that the climbing build leaves `FINAL TICK 1800 HASH eda8a62f036aeab6`); the extended "any one field changes the digest" test; and a test that two worlds with the same Ben and different `no_grab` hash differently.

**Parity.** A replay that climbs, the same on native, Web and DOS: **recommended as a second transcript**, `climb-replay.txt`, from its own small level (a ladder up to a ledge, a vine patch, a hop, a kick after a grab, a shot from a ladder), so the slice's golden stays. `tools/check-parity.sh` and the DOS check each run it, and DOS runs it in a check group of its own so the base gate is not slower (`tools/check-dos.sh --list` names the groups; a new check is documented in `PLATFORM.md`). The owner may instead put the climb in the slice's playground and re-pin (question 3).

**The pack and the dialect.** A conformance script with each of the three letters round-trips through the printer and lowers to the same `LevelSource`; an encode and decode of a level holding the tiles is byte-identical; the pinned compatibility numbers and `pack_content()` length change deliberately, with the reason in the PR; the lints each have a case.

**The presenter.** A frame digest or picture test of the three tiles and the two Ben frames in the slice view (`oblique_data.rs` is regenerated by `tools/tiles/emit-oblique.ts`, and its `--check` must pass); a `ClimbWatch` test that steps are counted from position and ignore the tick; a headless DOS check that the DOS picture of a level with a ladder equals the host's.

A trace test proves Fizz 1's behaviour. It proves nothing about the presenter, the Web or DOS behaviour of the events, and no result implies a sound was heard (`AGENTS.md`, "Testing").

## 13. The staged build

Each stage is a PR with the host gate passing and, from stage 2, the parity checks. The smallest first slice is the ladder alone.

1. **Ladder core.** The tiles `RUNG` and `RUNG_TOP` (ids, flags in `tiles::props`, `pack_content()`, the compatibility pins), the three flags on `Ben`, the climb step for ladders (grab, move, hop, settle, foot, let-go), the kick and fire and look guards, the flags in the hash, the ported unit tests, the trace format 3 and the ladder scenarios. No presenter change beyond drawing the two tiles and the two climb frames, no event, no level uses them. The golden is unchanged by construction.
2. **Vines.** `VINE`, the `wall` branch, the vine tests and scenarios.
3. **Events and the sound.** `Grab` and `Release`, the `clink` caption and its effect, `ClimbWatch`, the Web and DOS forms.
4. **The dialect.** Legend letters, the lints, the conformance corpus, the Fizz 1 export's tile mapping.
5. **A level that climbs, and the replay.** A small climb level, `climb-replay.txt`, the parity and DOS checks, the manual's section.
6. **Mantle** (its own spec), which reads `climb`.

Stages 1 and 2 are one PR if the owner prefers; 3 and 4 are independent.

## 14. Open questions

Each carries the answer this page assumes until the owner decides.

1. **Letters.** `H` rung, `T` rung top, `v` vine. Any other three unused characters work; the tile ids stay 28, 29 and 36.
2. **Where the flags go in the hash.** Bits 4 to 6 of the Ben flags byte, so the golden is untouched. Alternative: a byte of their own, which moves the golden for no gain.
3. **Does the slice show a ladder.** No: the slice level stays as it is, and a separate small climb level and replay prove climbing on all three platforms. Alternative: add a ladder to the playground and re-pin the golden in that PR.
4. **Which events and captions.** `Grab` (with `clink`) and `Release { hop }`, with the caption and effect ids added to the shared tables in stage 3. Alternative: raise both and give neither a caption or sound yet, which avoids the compatibility-number change until the sound work takes it.
5. **A stomp from a ladder.** Keep Fizz 1: the bounce is lost and a stunned chicken is stomped every tick. Alternative: a stomp lets go and bounces. It would break the trace proof unless the scenario avoids the case.
6. **A climb ends with its speed.** Keep Fizz 1: running out of a vine or a ladder with no top leaves Ben the climb's last vertical speed and he rises 0.1 to 0.17 tile before he falls. Alternative: zero it.
7. **A single wall with vines.** Keep Fizz 1: a grab refills the kick, so a wall with vine patches in it can be scaled. The lint says so; no rule stops it.
8. **A ladder next to an exit.** The lint forbids the pair; the precedence is not changed (the grab runs before the contacts).
9. **Sprites.** Fizz 1's two climb frames, recoloured, for both ladders and vines, and a vine pose of its own on the Web later. Alternative: draw a vine pose in the first build.
10. **Switched ladders and vines.** Out of scope for the first build: a binding kind for them waits for a level that needs one.
11. **Mantle.** After climbing, in its own spec, with the hop-to-ledge case as one of its tests.
12. **Moving platforms.** Decided with the platform work; this page says what Fizz 1 reads.
