# fizz1-trace

Golden traces from Lieutenant Fizz 1's own simulation. Fizz 2's port of Ben's physics is proved against these files bit for bit, so CI never needs Fizz 1; only regenerating the traces does.

The tool is a standalone Cargo project (it has its own `[workspace]` table, outside the repository workspace) with path dependencies on Fizz 1's `lf-sim` and `lf-episode-1`. It only reads Fizz 1, never writes into its tree, and refuses to run unless `git -C $FIZZ1 rev-parse --short HEAD` is `4bce036` and `git -C $FIZZ1 status --porcelain` is empty. `FIZZ1` defaults to `/home/scott/source/liminal-hq/lieutenant-fizz`. The two path dependencies in `Cargo.toml` name the same checkout; edit them if it lives elsewhere.

## Regenerating

```bash
cd tools/fizz1-trace
CARGO_TARGET_DIR=../../target/fizz1-trace cargo run --release -- --all
```

Set `CARGO_TARGET_DIR` so no build product lands in Fizz 1's tree. Other forms:

- `cargo run --release -- fall run` regenerates the named scenarios only.
- `cargo run --release -- --check --all` regenerates in memory and fails if any committed `.trace` differs, writing nothing.
- `--dir <dir>` reads and writes another scenario directory (the default is `crates/fizz-ep2/tests/fizz1`).

Runs are headless and deterministic: two runs produce byte-identical files.

## Scenario format

One file per scenario, `crates/fizz-ep2/tests/fizz1/<name>.scn`, shared with Fizz 2's tests. It is line oriented. Blank lines and lines starting with `#` outside a block are comments.

```text
scenario <name>
map
<rows, top row first, all the same width>
end
start <x> <y>
invulnerable
gloop <x> <y> <dir>
key red <x> <y>
input
<ticks> <buttons>
end
```

- `scenario` is the name and must match the file name.
- The map is a grid of letters, one Fizz 1 tile id each. The bottom row is `y = 0`, `y` grows upwards, and each tile is one unit. The left and right edges of the map are walls; below row 0 is an open pit; above the top row is open sky. Ben dies below `y = -1.5`.
- `start` is the position of the lower-left corner of Ben's 0.7 by 1.4 body, in tiles, parsed as a decimal `f64`.
- `invulnerable` (optional) gives Ben the arena's invulnerability (`inv = 1e9`). Without it Ben is vulnerable.
- `gloop <x> <y> <dir>` adds a Gloop as Fizz 1's level builder spawns one at `x y` (the spawn point; its body is 0.9 by 0.6, centred on `x + 0.5`), walking in direction `dir`, `1` or `-1`.
- `key red <x> <y>` adds a red key item at the centre `x y`. A red door is the `D` tile.
- `input` is a run-length script. Each line is a count of ticks and the buttons held for all of them.

Tile letters:

| Letter | Fizz 1 tile |
| --- | --- |
| `.` | empty |
| `#` | `FILL`, solid |
| `B` | `BLOCK`, solid |
| `=` | `PLAT`, a one-way platform |
| `^` | `SPIKE`, lethal |
| `~` | `CHOC`, lethal chocolate |
| `/` | `R45`, a rising 45 degree slope |
| `\` | `L45`, a falling 45 degree slope |
| `a` `b` | `R22A`, `R22B`, the two halves of a rising 22.5 degree slope |
| `c` `d` | `L22A`, `L22B`, the two halves of a falling 22.5 degree slope |
| `D` | `DOOR_R`, the red door |

Buttons: `L` `R` `U` `D` for left, right, up and down, `J` jump, `P` pogo and `F` fire, in any order, or `-` for none. The letters are the held bits passed to `World::step`; Fizz 1 derives the edge bits itself from the previous tick. `P` toggles the pogo on its press edge only, so `1 P` followed by `JP` for many ticks turns the pogo on once and holds the high bounce.

## How the world is built

Like `arena()` in Fizz 1's `src/tests.rs`: `World::new()`, `game_new()`, `enter_level(CRATER)`, then `ents`, `items`, `plats` (and the rooms, hazards and static lights of the Crater level) are cleared, `map` is replaced with the scenario's, `p` is a fresh `Player::level(x, y)` and the entities are pushed directly. There are no settling ticks: tick 0 is the state before the first step. Fizz 1's `init_ent` is crate-private, so the Gloop is constructed with the same field values.

After every tick the tool asserts `p.mantle.is_none()`. Automatic mantle is out of scope for Fizz 2's port, and a script that triggers it fails loudly instead of producing a trace.

## Trace format

`crates/fizz-ep2/tests/fizz1/<name>.trace`, `\n` line endings, in a stable order:

```text
FIZZ1 TRACE 2
scenario <name>
commit 4bce036
<tick row>...
```

There is one row for tick 0 (before any step) and one after each step. Fields are separated by single spaces:

```text
<tick> <x> <y> <vx> <vy> <face> <squash> <pogo> <ground> <dead> <keys_red> <switches> <look_up> <look_down> <aim>[ | <gx> <gy> <gvx> <gvy> <gdir> <gstun> <gdead>]
```

- `x y vx vy` are Ben's body position and velocity, and `squash` is `Player::squash`: each is the `f64` bit pattern as 16 lowercase hexadecimal digits (`{:016x}` of `to_bits`). There is no rounding. `squash` is never clamped and goes slightly negative after a bounce.
- `face` is `-1` or `1`. `pogo`, `ground` (`on_ground`), `dead` (`p.dead > 0`) and `keys_red` are `0` or `1`.
- `switches` is the map's switch word as eight hexadecimal digits.
- `look_up` and `look_down` are `Player::look_up` and `Player::look_down`, the seconds Up or Down has been held alone while Ben is idle (on the ground, not on the pogo or a ladder, no horizontal input), as 16 hexadecimal digits of the `f64` bits. Both are zero otherwise. `aim` is `Player::aim`, the direction of Ben's last shot: `-1` down, `0` level, `1` up. It keeps its value until the next shot.
- After ` | `, the first Gloop in the world, if there is one: its body `x y vx vy` and `stun` as 16 hexadecimal digits, `dir` as `-1` or `1`, and `dead` as `0` or `1`.

## Scenarios

| Name | What it exercises |
| --- | --- |
| `fall` | A long drop to the terminal speed of -22, then landing. |
| `run` | Acceleration to the 7 tiles per second cap, then friction to a stop. |
| `jump_full` | A held jump to its apex. |
| `jump_cut` | A jump released after four ticks (the 0.45 cut). |
| `pogo_idle` | The short 14 tiles per second bounce. |
| `pogo_held` | The held high bounce, with squash. |
| `pogo_ceiling` | A rising pogo bounce stopped by a low ceiling through the taller head box. |
| `oneway` | Rising through a one-way platform, landing on it and walking off. |
| `wall_step` | A wall stop at a ramp lip and the 0.55 tile step-up onto a block. |
| `gloop_patrol` | A Gloop turning at a wall and at a ledge. |
| `stomp` | Landing on a Gloop: stun and bounce, repeated while it stays stunned. |
| `stomp_pogo` | The same with the pogo on. |
| `fizz_stun` | A fizz shot stuns a Gloop; a second shot passes through it. |
| `key_door` | Collecting the red key and opening the red door. |
| `slope` | A walk up and down a staircase of `R45` slopes. |
| `look` | Idle on the ground, Up held for one second and released, then Down; both held together, Up while running, in the air and on the pogo (no look starts); Down again at the end. |
| `aim` | A level shot, a shot straight up with Up held, Down held on the ground (still level), and Down held in the air (straight down). |
| `route` | A short run through a small vertical maze up to the first death on spikes. |

## Not covered

Ladders, vines, wall kicks, moving platforms, conveyors, the bridge switch and gates, rooms and automatic mantle. A new scenario is a new `.scn` file and a rerun; adding a tile letter or an entity directive means extending `src/main.rs`.
