---
id: rules
n: 8
title: The rules from Fizz 1
date: 2026-10-27
summary: Fizz 1's physics as an owned crate, twenty-two golden traces from Fizz 1's own simulation that Ben and the chicken reproduce bit for bit (four of them the wall-kick), the numeric model, the state hash and the places the rules depart from Fizz 1.
prs: [22, 23, 27, 29, 35, 36, 150, 168]
---
The rules of the slice are Fizz 1's. `fizz-phys` holds Fizz 1's tile map, body, platform and random-number code; `fizz-ep2` ports Ben's movement, the chicken (Fizz 1's Gloop), the Fizz Blaster shots, the red key and door, and the spikes on top of it. Fizz 1 itself is not a dependency: its simulation wrote a set of traces once, and Fizz 2's rules are compared with them on every `cargo test`. The state hash of chapter 5 then compares the platforms with each other.

<!-- figure crate-graph -->

## fizz-phys, an owned copy (#22)

`fizz-phys` ({{loc.fizz_phys}} lines, {{tests.fizz_phys}} test declarations) is `no_std` and forbids `unsafe`. The bodies of its `tilemap`, `body`, `platform` and `rng` modules are Fizz 1's, byte for byte, apart from the mechanical changes each file's Provenance comment lists: the `no_std` wiring and the functions `core` lacks, which come from `libm` through `fizz-phys`'s `fmath`. The `exp` family is left out on purpose (see [chapter 5](chapter:parity)). `tools/lf-sim-drift.sh` re-applies the recorded changes to Fizz 1's source at commit `4bce036` and diffs the result with `crates/fizz-phys/src`; it prints `drift: none` when they match. It needs a Fizz 1 checkout, so neither `tools/check.sh` nor CI runs it.

## Fizz 1's own traces (#23)

`tools/fizz1-trace` is a standalone Cargo project outside the workspace, with path dependencies on Fizz 1's `lf-sim` and `lf-episode-1`. It refuses to run unless Fizz 1 is at `4bce036` with a clean tree, builds a world the way Fizz 1's own `arena()` test helper does, plays a scenario file through it, and writes one row per tick: Ben's position, velocity, facing, squash, pogo, ground and dead flags, the key count, the switch word, the look timers and the aim, and, when the scenario has one, the first Gloop's body, direction, stun and dead flag. Every floating-point field is the 16 hexadecimal digits of its bit pattern, so a row has no rounding to argue about. Format 2 adds the look timers and the aim (#47); the sixteen older traces were regenerated and their old columns are byte for byte unchanged.

The scenario files, `crates/fizz-ep2/tests/fizz1/<name>.scn`, are shared: Fizz 1's tool and Fizz 2's test read the same map, start position and run-length input script. There are twenty-two:

- `fall`, `run`, `jump_full`, `jump_cut`: gravity and terminal speed, walking and friction, a held jump and a cut one.
- `pogo_idle`, `pogo_held`, `pogo_ceiling`: the short bounce, the held high bounce with its squash, and a bounce stopped by a low ceiling.
- `oneway`, `wall_step`, `slope`: one-way platforms, a wall stop at a ramp's lip with the 0.55 tile step-up, and a staircase of 45 degree slopes.
- `gloop_patrol`, `stomp`, `stomp_pogo`, `fizz_stun`: the chicken turning at a wall and a ledge, a stomp with and without the pogo, and a shot that stuns it while a second shot passes through.
- `key_door`, `route`: the red key and door, and a run through a small maze that ends on spikes.
- `look`, `aim`: the look timers (Up or Down held alone while idle, and the cases where no look starts) and the four shot directions.
- `wall_kick_chimney`, `wall_kick_single`, `wall_kick_pogo`, `wall_kick_oneway`: the wall-kick, added with Milestone 3.5's first mechanic: kicks chained between two walls, one kick per side until landing, no kick on the pogo stick, and a one-way platform that is not a wall.

`crates/fizz-ep2/tests/fizz1_traces.rs` parses each scenario, runs it through `fizz_ep2::World` and compares every row with the committed trace as text, so a floating-point value matches only if all 64 bits do. The tolerance is zero, and the first mismatch panics with the tick and the name of the field. The scenarios run through the whole world rather than Ben alone, so the chicken, the shots and the key and door are proved by the same files as the movement (#27, #29). One bit is masked: Fizz 1 rewrites the red door's tile when it opens and never touches the switch word, while Fizz 2 keeps the door as a switched tile on its own channel, so that bit is left out of the `switches` comparison.

<!-- figure trace-chart -->

## The numeric model (#27)

State is `f64` and the rules use basic IEEE operations: addition, subtraction, multiplication, division and comparison. `fsin` and `fsqrt` are never used, `libm` is pinned at `={{sc.libm}}` with default features off, and the `exp` family is banned in the rules crates. On the DOS build the program sets the x87 control word to `0x{{sc.controlWord}}` (53-bit precision, every exception masked) before the first tick. The numeric decisions are listed in `docs/PLATFORM.md`; chapter 5 explains why each one is there.

Fizz 1's idle pogo has a horizontal drag that decays for ever. The velocity reaches subnormal values, which the x87 at 53 bits, SSE2 and WebAssembly round differently, and the golden traces never get that far. The port snaps any velocity whose magnitude is below 2^-1000 (about 9.3e-302, the constant `SNAP` in `crates/fizz-ep2/src/ben.rs`) to zero at the end of Ben's move. Milestone 0's marker used a threshold of 1/1024; the slice's is far below any velocity the traces reach, so none of the traces changes. Unit tests pin the threshold and run an idle pogo for 30,000 ticks, checking that every velocity is zero or at least the threshold.

## The state hash

After every tick the rules hash, in a fixed order, the tick; Ben's `x`, `y`, `vx`, `vy`, `face`, `squash`, `dead`, `inv` and `shoot_t`, a flags byte, the look timers, the look offset, the aim and the wall-kick's `wall_t` and `kick_side`; the world flags, the ammo and the death count; the door word, one bit for each door binding; for each chicken its body, direction, stun, a flags byte, its cluck timer and its draw count; for each shot its body and life; a byte for each snack and can taken; and, in a level that declares states, the state block of `docs/RULES.md` (the state values, the door word, the rule memory and the life's start tick). The order is written out above `hash` in `crates/fizz-ep2/src/world.rs`. The function is 64-bit FNV-1a over the little-endian bytes of those fields, with each `f64` as its bit pattern, so a change of one unit in the last place changes the digest. Three tests in `crates/fizz-ep2/src/world.rs` lock it: an independent byte-by-byte FNV-1a over a hand-built sequence, a pinned digest of a world with every field set, and a check that changing any one field changes the digest.

The event list, the camera and the presentation fields (the chicken's `cluck_age` and `cluck_variant`) are not hashed, so a presenter's own state cannot split the platforms. The golden transcript's final hash has changed whenever a hashed field was added: `d1a225bf14ab4828` for the first slice script, `2145503c0d3b625f` once the chicken's cluck timer and draw count joined it, and later finals with the look timers, the look offset and the aim, with the wall-kick's fields and the doors indexed by binding (`eda8a62f036aeab6`), and with the state block (`{{golden.final}}`, [chapter 15](chapter:scripts)). The replay script itself changed too: it grew to 1,800 ticks for the exit's Up press (chapter 13), and it fires 20 ticks earlier since the chicken's body grew (`7f6a1a1248c4b0e8`, chapter 12). A hash that moves with no field added and no script changed is a bug until shown otherwise. The final hash after {{golden.finalTick}} ticks is the same on native x86_64, WebAssembly and DOS (`PARITY ok {{ms.parityHash}}`).

## Where the rules depart from Fizz 1

- The 2^-1000 snap above.
- The doors are switched tiles, solid while their door binding's bit is set, where Fizz 1 rewrites the tile. Opening follows Fizz 1's rule as a plain rule of the level (chapter 15): with the key, touching any cell of the door in front of Ben opens it and spends the key, on the tick he reaches it even if he took the key on that tick.
- A death respawns Ben after Fizz 1's 1.4 seconds and resets the level as starting it does, as Fizz 1's reload does: the doors shut, the keys, snacks and cans return and every state goes back to its start value (#140, #156).
- The wall-kick is a core move in every level, with Fizz 1's numbers; Fizz 1 could turn it off per level (`docs/FIZZ1-DIFFERENCES.md`).
- The chicken's body is 1.75 by 1.5 tiles, the size of its drawn sprite plus a 4 px margin, where Fizz 1's Gloop is 0.9 by 0.6 (#168, chapter 12). The traces build their chickens at Fizz 1's size, so they test the movement port unchanged.
- The Gloop's reset after three seconds more than four tiles off its floor is not ported; the slice's chicken cannot leave its floor, and the reset would need a hashed timer.
- The chicken's cluck timer and its stream of random numbers are new; Fizz 1's Gloop is silent. Each draw is Fizz 1's `hashf` over the tick, the chicken's index and the number of draws so far, a pure function of integers, so the traces, which compare only Fizz 1's fields, are untouched.
- The look offset is new: Fizz 1 keeps the same memory in its camera position, and Fizz 2's camera is a function of Ben's state.

## The shelves

### Catalogued
- #22, #23, #27, #29, #35 and #36 are merged into `main`; the last of them, #36, landed on `{{pr36.merge}}` with CI {{pr36.ci}}. The wall-kick traces came with #150 ({{pr150.state}}, CI {{pr150.ci}}) and the chicken's own size with #168 ({{pr168.state}}, CI {{pr168.ci}}).
- Ben, the chicken, the shots, the key and the door reproduce all twenty-two Fizz 1 traces bit for bit: `cargo test -p fizz-ep2 --test fizz1_traces` printed {{ms.fizz1Traces}} (`snapshots/measured.json`), and `tools/check.sh` runs the same tests through `cargo test --workspace`.
- `fizz-phys` equals Fizz 1's `lf-sim` at `4bce036` apart from its recorded mechanical changes: `tools/lf-sim-drift.sh` printed `{{ms.lfSimDrift}}` (`snapshots/measured.json`; a Fizz 1 checkout is needed).
- The hash's field order, its FNV-1a bytes and the fact that every field matters are locked by three tests in `crates/fizz-ep2/src/world.rs`; the snap threshold by two in `crates/fizz-ep2/src/ben.rs`.
- Native, Web and DOS print the golden transcript, which ends `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`: `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}` (`snapshots/measured.json`).

### Uncatalogued
- Whether Fizz 1 at `4bce036` still writes the committed traces. The four wall-kick traces were written from Fizz 1 at `9729c07` with the pinned-commit check bypassed; the pull request records that the directories the tool builds from are identical at the two commits. CI never runs `tools/fizz1-trace`, and regenerating or checking the files (`cargo run --release -- --check --all`) needs a Fizz 1 checkout.
- What the scenarios do not reach: ladders, vines (specified in `docs/CLIMBING.md`, not built), moving platforms, conveyors, the bridge switch and gates, rooms and automatic mantle. The tool fails loudly if a script triggers a mantle.
- The `f64` rules on a real 486 FPU, and what they cost there. DOSBox emulates the x87 and runs at a fixed cycle count that does not model its latency.
- WebAssembly in a browser other than Chromium: the module was replayed under Node and Bun and in headless Chromium.

### Wish list
- A scheduled CI job that regenerates the traces from a pinned Fizz 1 and fails on a difference.
- Scenarios for ladders and vines when Milestone 3.5 ports them.
- Fizz 1's other rules for the later levels (lights, switches, the full enemy set) proved the same way, one scenario file each.

> **From the workbench — FNV-1a.** The Fowler–Noll–Vo hash starts from an offset basis and, for each byte, exclusive-ors the byte into the hash and multiplies by a prime. The 64-bit version uses the offset basis `0xcbf29ce484222325` and the prime `0x100000001b3`, the two constants the state hash of `fizz-sim` uses.
> Source: [Fowler–Noll–Vo hash function, Wikipedia](https://en.wikipedia.org/wiki/Fowler%E2%80%93Noll%E2%80%93Vo_hash_function)

```figures
id: crate-graph
kind: diagram
track: shared
title: The crates and what they use
alt: Dependency matrix of the seventeen Rust packages, one row for each and a mark in the column of each package it uses. The rules crates fizz-sim and fizz-phys, the pack reader fizz-pack, the HAL, the machine model, fizz-view, fizz-sfx and fizz-music use nothing in the workspace; fizz-ep2 uses fizz-sim, fizz-phys and fizz-pack; the script parser fizz-script uses fizz-ep2, fizz-pack and fizz-phys; fizz-slice-view uses fizz-ep2, fizz-view and fizz-sfx; the two presenters build on those; fizz-dos-rt uses fizz-sfx, fizz-machine and fizz-music; and FIZZ2.EXE uses the runtime, the rules, the pack reader and the DOS presenter.
caption: Which package uses which, from the Cargo.toml files.
at: figure crate-graph
thumb: svg

id: trace-chart
kind: chart
track: shared
title: Fizz 1's traces and Fizz 2
alt: Four curves drawn from Fizz 1's golden traces (a fall, a run, a held pogo bounce and the look timer) with a bar chart of the rows in each of the twenty-two scenarios; every row is reproduced bit for bit by Fizz 2.
caption: The committed Fizz 1 traces, and the test that Fizz 2 passes against all of them.
at: figure trace-chart
thumb: svg
```
