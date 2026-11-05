# Differences from Lieutenant Fizz 1

Where Lieutenant Fizz 2 plays differently from Lieutenant Fizz 1 on purpose, kept in one place so each can be reviewed again when level making starts. Fizz 1 is read at commit `4bce036`. A difference here is a decision, not a bug: change it only by changing this page too.

Each entry says what Fizz 1 does, what Fizz 2 does, why, and what to look at in the review.

## Mechanics

### Wall-kick: core, always on

- **Fizz 1:** pressing Jump while touching a solid wall in mid-air kicks off it, up at 19 tiles a second and away at 6 (`WALL_KICK_UP`, `WALL_KICK_AWAY` in `episodes/episode-1/game/src/world.rs`). Each side allows one kick until Ben lands, climbs, or kicks off the opposite wall, so one wall cannot be scaled but two facing walls at least three tiles apart can. A kick is always full height, has a quarter-second cooldown and the caption `Kick`. Fizz 1's Whisper Hollow is built around it (a wall-kick chimney), and it has a joke toast for twelve kicks.
- **Fizz 2:** the wall-kick is a core mechanic, the same in every level, with no per-level switch. The kick is Fizz 1's own: the numbers above, the one-kick-per-side rule, the cooldown and the full-height jump, ported from the same source and proved against it by a trace test. Level authors can rely on it, and a Fizz 1 level that was made with the kick works in Fizz 2 as it did.
- **Why:** owner decision, reversing the decision of 2026-10-30 that left it out. A kick gives a narrow shaft a way up without ledges, and every Fizz 2 level, not only ported ones, can use it. A per-level switch would make the same shaft climbable in one level and not in the next, and it would put a flag in every level and in the compatibility number for no gain.
- **Review at level making:**
  - The slice level never kicks: it is two shafts climbed by ledges, so its replay and hash are unchanged by the kick.
  - A level whose walls are within three tiles of each other, or a wall with a ledge a kick can reach, now has routes the author may not intend. The authoring tools should say so; the check that a Fizz 1 export does not need the kick belongs to the Fizz 1 export, which targets levels made with the kick turned off.
  - The `kick` caption (white, at Ben's chest, `CAP_KICK` 15), its sound (a new effect, `EFFECT_KICK` 22: a short upward chirp with a scuff of noise and a thump, authored for Fizz 2) and the twelve-kick joke are built. The caption and the sound come from the kick event through the same table as every other event, on the Web and on DOS. The joke is presenter view state, described under "The four small jokes" below. The two ids moved the compatibility number of baked packs (0x9161_5AAF to 0x6365_509C).
  - Ladders, vines and the mantle are not in Fizz 2 yet. Fizz 1 does not kick while Ben climbs or on the tick he lets go of a ladder or vine (`was_climbing`: he hops instead), and it clears the one-kick-per-side memory while he climbs. The hooks are in `Ben`: `climb` and `was_climbing` fields, wired into the kick refusal and the reset exactly as Fizz 1 has them, and tested, but nothing sets `climb` until ladders and vines exist (`tick_climb` goes where `tick_move` takes `was_climbing`). They are in the state hash's flags byte (bits 4 and 5), which does not move any state while they are false. Ladders and vines are specified for the owner's review in [`CLIMBING.md`](CLIMBING.md), which also says how a grab refills the kick.

### The four small jokes

- **Fizz 1:** a few rare actions earn a one-off toast per level visit (`self.fun`, `joke(..)` and `Toast::Joke*` in `episodes/episode-1/game/src/world.rs` and `text.rs`): thirty pogo bounces in a row, eight stomps on marshmallows, twelve wall-kicks and twenty seconds of standing still. A pogo run is broken only by switching the stick off. The stomp and kick counts never reset within a visit. Standing still adds one tick when no button is held and Ben begins the tick on the ground, and it tells when the sum is above 20 seconds. Each is told once, and the counters start again whenever the level is loaded, which includes after a death.
- **Fizz 2:** `fizz_slice_view::JokeWatch` keeps the same counters as presenter view state, read after every step on the Web (`App::collect_events`) and on DOS (`play_events`), so the rules, the transcript and the state hash do not know it. It produces a `Joke` (a kind and Fizz 1's text, from the table `JOKE_TEXTS`) once per level visit. There is no toast line yet, so nothing is shown: the Web `App` reports the last joke (`last_joke`, `last_joke_kind`) and `/REPLAY` on DOS prints a `JOKE` line for each one told (none in the golden replay).
- **The departure:** Fizz 2 has no marshmallows, so the eight stomps are stomps on chickens. Fizz 1 counts every stomp on a marshmallow, which always bounces. Fizz 2 counts `Event::Stomp`, which a stomp raises only when it stuns a chicken that was not stunned; a bounce off an already stunned chicken raises no event and is not counted. The text keeps Fizz 1's marshmallow line verbatim for now, so it will want a Fizz 2 line when a toast line shows it. A respawn is a new visit (Fizz 1 reloads the level on a death), and so is a restart.
- **The golden replay** tells no joke: a test pins that. It never kicks, never stands for twenty seconds, ends with a pogo run of nothing (the stick is switched off) and stomps once.

### The exit takes an Up press

- **Fizz 1:** touching the exit tile ends the level. `tick_ben_contacts` in `episodes/episode-1/game/src/world.rs` sets the level complete the moment Ben's body overlaps an exit tile, however he got there (walking, falling or bouncing).
- **Fizz 2:** Ben goes through the exit when Up is newly pressed while his body overlaps an exit tile and he is standing on the ground. Touching the exit alone does nothing, an Up press in mid-air does nothing, and an Up held from before he arrived does not count until it is released and pressed again. The press raises the same `Exit` event and freezes Ben as touching did, and the look-up timer is cleared so the camera does not start to look. The door takes the press before the look-up and aim-up uses of Up. The slice's golden replay presses Up at the exit (see `docs/PLATFORM.md`).
- **Why:** owner decision, 2026-11-03: the exit is now a doorway on the far edge of the oblique floor (see `docs/OBLIQUE.md`), and Up walks into it, like a door.
- **Review at level making:**
  - An exit must be somewhere Ben can stand: a level cannot put an exit where he only passes through it in the air, on the pogo stick (which is never still on the ground), or while falling; the replay steps off the pogo stick before it presses Up.
  - Levels ported from Fizz 1 whose exit is reached by a jump or a bounce need a place to land beside it.
  - Whether Up should also work for the doors that open with a key (they open on touch, as in Fizz 1) is open.

### The chicken's body fits its sprite

- **Fizz 1:** the Gloop's body is 0.9 tiles wide and 0.6 tall, centred on its spawn tile (`info(Kind::Gloop)` in `episodes/episode-1/game/src/ents.rs`, `init_ent` in `world.rs`).
- **Fizz 2:** the chicken's body is 1.75 tiles wide and 1.5 tall (`CHICKEN_W`, `CHICKEN_H` in `crates/fizz-ep2/src/chicken.rs`). The placeholder art is 24 by 24 pixels, opaque across all 24 columns and up to 24 rows above the feet, so the body is the sprite's 24 by 24 plus 4 pixels on its right, where a wall's front face is drawn 4 pixels left of its collision edge in the oblique projection (`docs/OBLIQUE.md`). The body's left edge is the sprite's left edge, and the view draws the sprite with its anchor a constant 12 pixels right of it. The spawn centres the sprite, not the body, on the spawn tile (`x + 0.5`). Everything else about the chicken is Fizz 1's line for line: the speed, the patrol turn at a wall or at a ledge's end, the stun times and the stomp rule.
- **Why:** owner decision, 2026-11-05: the chicken itself stops earlier, and the drawing is not corrected to fit the collision. The rule is that an enemy's collision body contains its sprite's opaque pixels, plus 4 pixels on its right.
- **Review at level making:**
  - Measured from its middle (the sprite's anchor), the chicken turns 0.55 tiles earlier at a wall on its right than a Fizz 1 Gloop does, and 0.3 tiles earlier at a wall on its left and at a ledge's end (the ledge probe is 0.05 tiles beyond the body's front edge, as in Fizz 1). A ledge must be longer than the body (1.75 tiles) for the chicken to walk on it.
  - The body is 1.5 tiles tall, so it needs 1.5 tiles of headroom. Ben jumping a chicken must clear 0.825 tiles of it to stomp, and a hop lower than that kills him, where a Fizz 1 Gloop was 0.33 tiles.
  - `tests/fizz1_traces.rs` still proves the port bit for bit: it builds its chickens with `Shape::FIZZ1`, Fizz 1's size, through `World::add_chicken_shaped`. The proof is of the movement code, not of the size.
  - A second kind of enemy gets its own body by the same rule from its own art.

## Small departures that are already in the game

### Doors, keys and death

- **Fizz 1:** a key and its door are hard-wired in the rules (a red door tile that a red key rewrites to empty space), and dying reloads the level, so every door is shut again and every key is back.
- **Fizz 2:** the door is a switched tile on a channel, and which key opens which channel is a lock rule in the level data (`crates/fizz-ep2/src/rules.rs`). Dying resets the level exactly as starting it would: the channels go back to the level's start state, the keys come back and leave Ben's hands, the snacks come back. An earlier build of Fizz 2 kept an opened door open through a death; it does not any more, which is closer to Fizz 1's reload.
- **Why:** owner decision, 2026-10-30: a death always resets the level, so a puzzle that closes a door behind the player cannot be left unwinnable by dying.
- **Review:** checkpoints, if levels get them, would need to say which of this they keep.

### Ammo on a respawn

- **Fizz 1:** dying does not refill the Fizz Blaster.
- **Fizz 2:** the slice restarts Ben with five shots after a death. Changing it would move the replay golden's first life.
- **Review:** decide when levels have checkpoints.

### The menu mix on DOS

- **Fizz 1:** the Web version quietens and dulls the music behind the Paused and Options screens; its DOS version does not.
- **Fizz 2:** both do (DOS by a small change to the operator levels, see [`SOUND.md`](SOUND.md#the-screen-mix)).
- **Review:** whether the Classic way of playing should skip it.
