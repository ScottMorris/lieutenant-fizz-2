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
  - The `Kick` caption, its sound and the twelve-kick joke are not built, and stay out until the sound and caption work takes them. The kick raises an event in the transcript, so presenters can add them later without a rules change.
  - Ladders, vines and the mantle are not in Fizz 2 yet. Fizz 1 does not kick on the tick Ben leaves a ladder or vine (he hops instead); that rule has nothing to apply to until they exist.

### The exit takes an Up press

- **Fizz 1:** touching the exit tile ends the level. `tick_ben_contacts` in `episodes/episode-1/game/src/world.rs` sets the level complete the moment Ben's body overlaps an exit tile, however he got there (walking, falling or bouncing).
- **Fizz 2:** Ben goes through the exit when Up is newly pressed while his body overlaps an exit tile and he is standing on the ground. Touching the exit alone does nothing, an Up press in mid-air does nothing, and an Up held from before he arrived does not count until it is released and pressed again. The press raises the same `Exit` event and freezes Ben as touching did, and the look-up timer is cleared so the camera does not start to look. The door takes the press before the look-up and aim-up uses of Up. The slice's golden replay presses Up at the exit (see `docs/PLATFORM.md`).
- **Why:** owner decision, 2026-11-03: the exit is now a doorway on the far edge of the oblique floor (see `docs/OBLIQUE.md`), and Up walks into it, like a door.
- **Review at level making:**
  - An exit must be somewhere Ben can stand: a level cannot put an exit where he only passes through it in the air, on the pogo stick (which is never still on the ground), or while falling; the replay steps off the pogo stick before it presses Up.
  - Levels ported from Fizz 1 whose exit is reached by a jump or a bounce need a place to land beside it.
  - Whether Up should also work for the doors that open with a key (they open on touch, as in Fizz 1) is open.

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
