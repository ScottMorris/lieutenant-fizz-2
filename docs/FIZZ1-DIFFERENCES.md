# Differences from Lieutenant Fizz 1

Where Lieutenant Fizz 2 plays differently from Lieutenant Fizz 1 on purpose, kept in one place so each can be reviewed again when level making starts. Fizz 1 is read at commit `4bce036`. A difference here is a decision, not a bug: change it only by changing this page too.

Each entry says what Fizz 1 does, what Fizz 2 does, why, and what to look at in the review.

## Mechanics

### Wall-kick: left out

- **Fizz 1:** pressing Jump while touching a solid wall in mid-air kicks off it, up at 19 tiles a second and away at 6 (`WALL_KICK_UP`, `WALL_KICK_AWAY` in `episodes/episode-1/game/src/world.rs`). Each side allows one kick until Ben lands, climbs, or kicks off the opposite wall, so one wall cannot be scaled but two facing walls at least three tiles apart can. A kick is always full height, has a quarter-second cooldown and the caption `Kick`. Fizz 1's Whisper Hollow is built around it (a wall-kick chimney), and it has a joke toast for twelve kicks.
- **Fizz 2:** there is no wall-kick. Jump does nothing in mid-air against a wall, and `fizz-phys` has no wall contact state for it.
- **Why:** owner decision, 2026-10-30: wall jumping is not in the new game.
- **Review at level making:**
  - The slice level is two shafts climbed by ledges, so it does not need a kick. A level that is a tall shaft with no ledges cannot be climbed in Fizz 2.
  - Levels ported or converted from Fizz 1 (see the Fizz 1 export in the level script plan) must not rely on a kick; the baker or the editor should refuse a shaft that only a kick can climb, or the author adds ledges.
  - Whether something takes its place (a different way up a narrow shaft, such as vines, ladders or a pogo bounce) is open.
  - The `Kick` caption, its sound and the twelve-kick joke are not built, and stay out.

## Small departures that are already in the game

### Ammo on a respawn

- **Fizz 1:** dying does not refill the Fizz Blaster.
- **Fizz 2:** the slice restarts Ben with five shots after a death. Changing it would move the replay golden's first life.
- **Review:** decide when levels have checkpoints.

### The menu mix on DOS

- **Fizz 1:** the Web version quietens and dulls the music behind the Paused and Options screens; its DOS version does not.
- **Fizz 2:** both do (DOS by a small change to the operator levels, see [`SOUND.md`](SOUND.md#the-screen-mix)).
- **Review:** whether the Classic way of playing should skip it.
