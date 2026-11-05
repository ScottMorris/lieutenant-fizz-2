# Rules: states, bindings and events

This document specifies what a level's rules mean to the simulation: the state table, the bindings that make states visible in the world, the inputs rules react to, their conditions and actions, the order everything runs in each tick, the death reset, raised events and the preview API that native hosts use. It owns the semantics only. The byte layout of the records is [`FAP.md`](FAP.md) (the `LEVL` section), and the source dialect that authors write is `script fizz2 0`, specified in [`SCRIPT.md`](SCRIPT.md).

The design is the owner's: locks are states with rules. A level declares named states, doors and other things bind to a state, and rules change states in answer to inputs. A key is one input among many, and `lock` is short sugar for the key-and-door case: one state, one door binding and one plain rule. There are no switch channels. The sim today runs the state table, door bindings, the rule phase with `touch` and `enter` rows (a lock is a plain `touch` row in it) and the death reset; this document says where the rest goes and the order it runs in, so that every platform computes the same state hash.

Status words used below: **built** means the code runs it today, and **planned** means specified here and not yet built. The code arrives in steps (the `LEVL` 2 bytes, the state table and the first rule inputs, then the dialect and its lowering, then the rest of the rule phase and the preview API), and until a step lands this document leads. A record that names an input, condition, action or delay that is not built is refused at load (`RuleError::Unbuilt`, `UnbuiltCondition`, `UnbuiltAction` or `Delay`), never ignored. Two points are built in an interim form that keeps the slice's golden, each awaiting the owner's decision to re-pin: the hash block of section 11 and the place of the door event of section 3 (open question 1).

## 1. Contract with the other work

These limits are shared with the `LEVL` 2 layout and the script dialect, and are the studio's proposed defaults for its questions Q23 and Q24.

- A level declares up to **32 states**. Each has **2 to 8 values** (numbered from 0) and a **start value**, which defaults to value 0.
- A level has up to **64 bindings**, of which at most **32 are door bindings**. A binding holds a **set** of values: an 8-bit mask over the state's values.
- A rule record is fixed-size and holds **2 conditions and 2 actions**. A rule that needs more is split into several rules chained through a state.
- A level has up to 32 rules (one bit per rule in a `u32` of rule memory, as today).
- The **death reset** restores every state to its start value.
- **`lock`** is sugar for the key-and-door case: one state, one door binding and one rule (section 9).
- Names are source-level only. A state, an object and a string are numbered in the baked level, in declaration order, and the baker resolves every name to a number. The sim never sees a name.

## 2. The state table

A state is a small integer with a fixed number of values. The table is a list of at most 32 entries: `values` (2 to 8) and `start` (below `values`). The current value of each state is world state. A state is changed only by rule actions (section 6) and by the death reset (section 8); nothing else writes it.

There are **no switch channels**. A switched tile carries no state of its own; a door binding says which state its tiles read (section 3). Fizz 1's channel was a two-valued state with a door wired to it, and a Fizz 1 conversion writes the ordinary two-valued states `channel-N`.

Arithmetic on a state is closed: `set` takes a value below `values`, `next` wraps from the last value to 0, and a record that names a value out of range is refused at load, not clamped at run time.

## 3. Bindings

A binding attaches something in the world to a state, so that the state is what the world shows. Each binding holds an **active set**: the values at which the bound thing is passable, lit or shown. Every kind reads the same way, and a door that is open for two of three values is one binding.

| Binding | What it does | Status |
| --- | --- | --- |
| Door | The switched tiles in a rectangle are passable exactly when the state's value is in the active set, and solid otherwise | built |
| Light | A placed light is lit when the state's value is in the active set | planned (F12) |
| Platform | A moving platform runs, or is held at an end, according to the state's value | planned |
| Panel | A parallax panel layer is drawn when the state's value is in the active set | planned (F19) |

Every switched tile in the map is covered by exactly one door binding, and a door rectangle holds only switched tiles; the loader refuses a level that breaks either. Anything not bound is unaffected by states.

A binding is **derived, not stored**. For doors the sim keeps a **private derived bit** for each door binding: binding `i` of kind 1 owns bit `i` of a `u32` (set while the door is closed, that is, while the state's value is not in the active set), and a per-cell `u8` overlay built at load from the rectangles (0xFF for none) says which bit a switched cell reads. The overlay is `w * h` bytes, about 6 KB for a Fizz 1-sized level, and the solidity test reads the bit. The bits are recomputed at two points: once after the rule phase of every tick in which any state changed, and once when the world is built or reset. The word of bits is hashed (section 11), so two worlds with the same state values have the same bits. Neither the source nor the bytes name a bit, and a simulation that read the state directly in the solidity test would behave the same.

A rule never writes a door: it writes a state, and the binding follows. The cost of a binding is one mask test, only when a state changed.

**The door event.** When the recomputation makes a door binding passable (its state has just entered the active set), the sim raises the game's built-in door event, `Event::DoorOpen` (effect 19), at the centre cell of the binding's rectangle (the cell at the integer midpoint of its corners, drawn at that cell's centre). Events from several bindings follow binding order. Building and resetting the world raise no event. The door's sound and word are therefore not actions, and a lock keeps both of its action slots.

*Built, with one interim exception:* when Ben's front probe lay inside the door's rectangle on that tick (a lock opened it), the event keeps the place Fizz 1 and the old lock phase gave it, above the probed column at Ben's feet plus two tiles, so the two `dooropen` lines of the slice's transcript do not move. Raising every door event at the centre cell moves them from `248 352` and `360 682` to `248 344` and `360 632` (the state hash does not change); that re-pin is the owner's call (open question 1).

## 4. Inputs

An input is something that happened this tick. Phases that already exist raise inputs as they run; the rule phase (section 7) reads them. A rule has exactly one input, in its `trigger`.

| Input | Raised when | Where it is raised | Status |
| --- | --- | --- | --- |
| `start` | The first rule phase of a life: tick 1 of the level and the first tick after each death | The rule phase | planned |
| `enter` | Ben's body comes into a rectangle of cells (an edge: not inside last tick, inside this tick); not raised while Ben is dead | The rule phase | built |
| `take` | Ben takes a key or snack object this tick | Ben's contacts | planned |
| `touch` | Ben's **front probe** lands in a rectangle of cells, or his body overlaps it, or his body overlaps a named object this tick (a level, not an edge; a rule may repeat) | Ben's contacts | built for a rectangle (trigger 9); the object form (trigger 10) is planned |
| `use` | Ben presses Up while overlapping a switch object (a cell is reserved) | Ben's contacts | planned |
| `hit` | A switch object is struck by a shot or a stomp, as its flags allow | The shots, Ben's contacts | planned |
| `stomp` | Ben stomps a named chicken or enemy, or any | Ben's contacts | planned |
| `stun` | A shot stuns a named chicken or enemy, or any | The shots | planned |
| `defeat` | A named enemy is defeated (stunned for good or killed, per its definition) | The shots, contacts | planned |
| `cleared` | Every member of a group of objects has been taken or defeated, on the tick the last one is | The phase that finished the last one | planned |
| `shoot` | A shot enters a rectangle of cells | The shots | planned |
| `time` | The life's tick count reaches `at`, and every `every` ticks after (at least 6) | The rule phase | planned |
| `becomes` | A named state has changed to a given value since this rule last ran | The rule phase | planned |
| `changes` | A named state has changed value since this rule last ran | The rule phase | planned |

`hurt` (Ben hurt by a hazard or an enemy) is reserved and not lowered; `hit` is about switches. `touch` on a cell and the conditions beyond the three kinds in section 5 are the other reserved parts.

**The front probe.** It is Fizz 1's lock test: the tile in front of Ben at `py + 0.2`, that is, the cell at column `floor(px + w + 0.1)` when he faces right or `floor(px - 0.1)` when he faces left, and row `floor(py + 0.2)`. A rectangle is touched when that cell is inside it or when Ben's body box overlaps its cells. A lock's rectangle is its door's whole run, so a key works from any cell of the door, including the bottom one that Ben meets when he stands at the door's foot. A touch rectangle of one cell in the middle of a tall door would not be reached from there, which is why a touch takes any rectangle.

`touch` is a level, so a rule that repeats on it runs on every tick of the contact; a repeating touch rule whose action is `toggle` or `next` flips every tick, and the dialect warns about it.

Inputs are sampled, not queued. A phase that raises an input records it in a per-tick record (a bit per rule, plus the index of the object involved) and the record is cleared at the start of the next tick. Two raisings of the same input on one tick are one input for rule purposes; a rule fires at most once per tick.

`enter`, `shoot` and `touch` take the rectangle from the record, both corners included, in tile cells with row 0 at the bottom. Ben is inside a rectangle when his body box overlaps the box of the cells (the half-open region from the low corner to one past the high corner). The previous tick's insideness is rule memory (section 8).

## 5. Conditions

A rule holds up to 2 conditions, and the rule fires only when all that are present hold. They are read before any action of the rule runs, in the state the rule phase has reached (section 7). Two conditions are always an AND; an OR is two rules.

| Condition | True when |
| --- | --- |
| State in | A state's value is in a set of values (a mask). `is` names the set, and `is not` is the complement within the state's values |
| Key held | Ben holds the key of a colour |
| Key not held | Ben does not hold the key of a colour |

Three more kinds are numbered and rejected until they are built: ammo at least, score at least and group cleared (a level, not an edge). A rule with no condition always passes. The three kinds in the table are built.

## 6. Actions

A rule holds up to 2 actions, run in slot order after its conditions pass. Each takes effect at once, so the second action and every later rule see the first.

| Action | Effect |
| --- | --- |
| `set` | Set a state to a value |
| `toggle` | Flip a two-valued state (a state with more than two values is refused for toggle: use `next`) |
| `next` | Advance a state to its next value, wrapping |
| `spend` | Take a key of a colour from Ben. A `spend` of a key Ben does not hold does nothing; the loader adds no condition, and a rule that must not run without the key says so with `holding` (a lock does) |
| `give` | Give Ben a key of a colour |
| `score` | Add points to the score |
| `ammo` | Add shots to the blaster |
| `word` | Raise a floating caption at a cell (section 10) |
| `cue` | Raise a sound cue at a cell (section 10) |

Built: `set`, `toggle`, `next` and `spend`. `give`, `score`, `ammo`, `word` and `cue` are refused at load until they are built, and so is a delay.

`set`, `toggle` and `next` that leave the value unchanged do not count as a change. A change marks the state as changed for `becomes` and `changes` rules and for bindings.

**Delay.** A rule with `after=TICKS` tests its conditions when its input arrives and runs its actions that many ticks later, in the rule phase. A row with a delay pending **ignores its input** until the delay has run, so an input cannot queue a second copy. A death drops every pending delay (section 8). `score` and `ammo` additions are recorded as world state that the hash already covers, through the snack and ammo fields (extended when a rule can give score of its own: open question 6).

## 7. Order inside a tick

This order is part of the parity contract: native, Web and DOS must agree on it, because the state hash after each tick depends on it. Existing phases keep the order they have today (see `World::step`), except the locks, which leave Ben's contacts and become rows of the rule phase; the rule phase is added after the other phases, never between them.

1. The tick counter advances and last tick's events and input record are cleared.
2. Ben's movement: the jump or bounce, the shot, the head bump.
3. Ben's contacts, in Fizz 1's order without the locks: hazards and the exit, the keys, the snacks, the chickens (stomp raises its input here). `touch` and `use` are **sampled** here, from Ben's final position of the tick. If Ben dies, the contacts end.
4. Chickens and enemies move, in index order.
5. Shots move, from the last to the first (stun, defeat and a switch hit raise their inputs here).
6. **The rule phase.** Rows run in table order, 0 to 31, which is the file order of `when` and `lock` lines:
   1. For each row whose input was raised this tick (or, for `becomes` and `changes`, whose watched state changed since the row last ran), whose once-per-life memory allows it, which has no delay pending and which has not fired this tick: evaluate the conditions.
   2. If they all hold, run the actions in slot order (or start the delay), set the row's fired memory and queue its cue.
   3. Move to the next row.
   A row sees every change made by an earlier row this tick, so an earlier row can arm a later one in the same tick. A change made by a later row reaches an earlier `changes` row on the next tick. Each row fires at most once per tick, so a chain of `changes` rows cannot loop and the phase costs at most 32 evaluations.
7. **Bindings** are recomputed if any state changed in this tick (section 3), and the door event is raised for each door binding that became passable.
8. The death check and the respawn, as today. A respawn runs the death reset (section 8).

**The lock is a plain rule.** It runs in step 6 like any other row. Its `touch` is sampled in step 3, where Ben's contacts are read, and the row runs once the world has fully moved, so the door opens on the tick Ben presses against it (and the key he picked up on that same tick counts, which Fizz 1's earlier lock phase did not see until the next tick). All rule inputs wait for step 6 so a rule reads the world after it has fully moved. The events a rule queues are appended to the world's event list after the events of steps 2 to 5, in row order, and the door events of step 7 follow them.

The input record is what makes the order independent of presentation: no rule reads a presenter, a clock or a random draw.

Built as listed, for the inputs that exist: the `touch` sample sits where Fizz 1's lock test was, after the hazards and the exit and before a death ends the contacts, so a lock still fires on the tick Ben dies against the door; the rule phase runs every tick, even while Ben is dead, though no `enter` is raised then.

## 8. The death reset

A death reloads the level in effect (as Fizz 1 reloads). The reset restores, in this order:

1. Ben, ammo, shots, snacks and chickens (as today), and the keys Ben holds.
2. **Every declared state to its start value.**
3. Rule memory: each row's fired bit and each edge row's inside bit are cleared, **pending delays are dropped**, and the life's start tick becomes the current tick, so `time` rules count from the respawn.
4. Bindings are recomputed from the state table, and the derived door bits follow, with no door event.

The `start` rows then fire in the first rule phase after the respawn, as they do on tick 1. A world with no declared states and no rules resets exactly as it does today. States are **level-scoped**: a world-scoped state is open question 2.

## 9. `lock` as sugar

`lock NAME key=K at=I,J` is a short way to write one state, one door binding and one rule, and the sim sees only those:

```text
state red-door values=closed,open
door x=15 y=20..22 state=red-door open=open
when touch x=15 y=20..22 if state red-door is closed and holding key-red then set red-door open, spend key-red
```

The door's run is the 4-connected set of cells holding the same switched tile as the cell the lock names, and it must be a rectangle. The rule's `touch` rectangle is the whole run, so any cell of the door accepts the key, as in Fizz 1, and the `holding` condition is written by the expansion and not added by the loader. The rule is an ordinary row of the rule phase (section 7) and has no `repeat`, so a lock opens its door once per life and a death re-arms it. The door's sound is the built-in door event of the binding (section 3), not an action.

The slice's two locks are two such triples. The state hash they produce is a function of the states and the derived door word (section 11).

## 10. Raised events and the sound library

A rule's `word` or `cue` action, and later an object, raises an event a presenter shows or plays: a **word** (a floating caption) or a **cue** (a sound). The record names a shared caption id, a level string (`0x40 + i`), a shared effect id or a world sound. The sim emits the event with its cell; each presenter draws and plays it, and the transcript records it as an `EVENT` line so Web and DOS can be compared. Events are output only and are not hashed.

**Where the world's sound library lives (open question 3, proposed answer for the owner).** The world's own sounds are `FXPK` sections in the world pack, `<WORLD>.FAP`, three of them in the OPL3, OPL2 and speaker order that `SHARED.FAP` uses. The `FXPK` layout is already fully specified per device in [`FAP.md`](FAP.md), a world pack already holds the per-device audio hash in its compatibility number, and a world pack is loaded in full when a level of the world starts, so the library needs no new section tag and no new compatibility number. An effect code below `0x40` is a `SHARED.FAP` effect, as today; `0x40 + i` is the world's sound `i`, the same split the caption codes use. `SHARED.FAP` keeps the built-in effects. The alternative is a separate `SNDS` section; it would duplicate the index and the device sections for no gain. A world with no sounds has no world `FXPK`, and a rule that names a world sound beyond the library is refused at load. This is a proposal: the owner decides it.

On DOS a world sound with no speaker form plays nothing there, and a caption draws in the 8-by-8 font at the same place; this is the reduction DOS is allowed.

## 11. The hash

State enters the world hash only when it exists. A world with at least one declared state appends one block to the end of the hash after the snacks: the state count (`u32`), each state's current value (`u8`) in table order, the derived door word (`u32`, one bit per door binding), the fired memory (`u32`), the inside memory (`u32`) and the life's start tick (`u64`), and, when any rule has a delay, a `u16` countdown per rule in table order (0 when none is pending). A world with no state hashes as before. The move to `LEVL` 2 re-pinned the slice's golden once, because the derived door word replaced the channel bits ([`FAP.md`](FAP.md), open point 7); it is now `FINAL TICK 1800 HASH 7f6a1a1248c4b0e8`. The input record and the queued events are not hashed: both are cleared every tick.

*Built, with one interim exception:* a level whose rules are locks alone, each state opened by exactly one lock of the section 9 shape (the slice), writes no block. In such a level the state values and the fired bits equal the door word, which the hash already carries, and the inside memory and the start tick are never read, so the block adds no information. Appending it for those levels too moves the slice's golden from `FINAL TICK 1800 HASH 7f6a1a1248c4b0e8` to `ec9195c222b626f2`; that re-pin is the owner's call. No rule can have a delay yet, so no countdowns are written.

## 12. Mapping onto the existing records

`fizz-pack`'s `RuleRec` (version 1 of `LEVL`) is the starting point, and `LEVL` 2 replaces it; the version 2 sim does not read a version 1 record. The mapping from today's constants:

| Record | Meaning here |
| --- | --- |
| `TRIGGER_LOCK` (1) | Retired: a `touch` rule (trigger 9) over the door's rectangle, with a state condition and a key condition |
| `TRIGGER_START` (2) | `start` |
| `TRIGGER_ENTER` (3) | `enter`, the rect from the union (built) |
| `TRIGGER_TAKE` (4) | `take`, `subject` is the object index |
| `TRIGGER_SHOOT` (5) | `shoot` |
| `TRIGGER_STOMP` (6) | `stomp` |
| `TRIGGER_STUN` (7) | `stun` |
| `TRIGGER_TIME` (8) | `time`, `at` and `every` from the union |
| 9 to 15 | `touch` (rect), `touch` (object), `use`, `hit`, `defeat`, `cleared`, and `becomes` or `changes` (`arg` the value or `0xFF`) |
| `OP_ON`, `OP_OFF`, `OP_TOGGLE` on `channel` | `set` a two-valued state to 1 or 0, `toggle` it |
| `FLAG_REPEAT` | The row may fire again after Ben leaves and re-enters (an edge row), on every tick of a contact (`touch`), or on every period (`time`); without it a row fires once per life |
| `FLAG_GUARD`, `FLAG_GUARD_ON`, `guard_ch` | One state-in-set condition |
| `delay` | Ticks between the input and the actions (section 6) |
| Slot A event cell | The built-in door event of a binding, or a `word` or `cue` action |
| Slot B effect, caption and cue cell | A `word` action and a `cue` action |

## 13. The preview API (F6)

A native host (the level studio) needs to build a world from baked bytes, step it by inputs and read what happened, without a presenter. The API lives in `fizz-ep2`, is `no_std` plus `alloc` like the rest, and wraps what exists:

- `Preview::from_level(bytes: &[u8]) -> Result<Preview, PreviewError>` reads a `LEVL` section with `fizz_pack::LevelView::new` and builds the world with `world_from_view`. Its errors keep the pack's `LevelError` and the sim's `SourceError` apart so a host can say which failed.
- `Preview::step(&mut self, input: InputFrame)` advances one tick; `run(&mut self, inputs: &[InputFrame])` advances many and returns the hash after each.
- `Preview::events(&self)` is the last tick's `Event` list. `Preview::hash(&self)` is the state hash. `Preview::states(&self)` is the state table's current values, and `Preview::fired(&self)` is the rule indices that fired on the last tick, in order, for the debugger.
- `Preview::reset(&mut self)` rebuilds from the same bytes, and a preview is `Clone`, so a host can snapshot and rewind.

A preview of a level is the same code path as the game, so a rule preview that agrees with the studio's lowering also agrees with the Web and DOS builds. The hash it reports is the arbiter, as everywhere else.

## 14. Cost on the DOS floor

The state table is at most 32 bytes of values; the rule memory is 8 bytes plus the life's start tick; the door overlay is one byte a tile. The rule phase touches at most 32 rows per tick, each one an input-bit test, up to two integer comparisons, and up to two small writes; `enter` adds one box overlap in `f64` per row (a handful of compares, no `exp`, nothing reordered). That is in the low hundreds of integer operations and at most 32 box tests a tick, against a step that the platform document measures at about 103 microseconds of a 16,667 microsecond tick on a 486DX2-66 in DOSBox. Bindings run only on a tick where a state changed. Nothing in the phase allocates: the tables are read once at load and the memory is fixed-size. The DOS limits the design must not carry back to the Web are cosmetic ones: lights and panels bound to a state step between banks without fading, a world sound without a speaker or OPL2 form is silent on that device, and captions use the 8-by-8 font.

## 15. Open questions

Decided by this document: channels are not in `LEVL` 2 (a door binding names a state and a mask, and the sim may keep a private derived bit for each of at most 32 door bindings); two conditions are an AND, and an OR is two rules; a delayed row drops its pending actions on death and ignores its input while pending; a group is a number from 1 to 16 in a chicken's `aux`, named in the source, and an input only for now; `RULES_SEMANTICS` becomes 2 with the `LEVL` 2 change. Each question below carries the default this page uses until the owner decides.

1. **Lock timing.** Moving the lock from Ben's contacts (step 3) to the rule phase (step 6) can reorder the slice's events within a tick (the door event after the snack events) and moves the door event's position from the touched column and Ben's top to the centre cell of the door. Default: move it, prove the state hash unchanged, and re-pin the event transcript in the `LEVL` 2 change if it moves.
2. **World-scoped states** (the studio's Q27). Default: level-only for now, so a death reset stays exact, with the state record's `WORLD` flag reserved. A world state would need saving with progress and a rule for what a death resets.
3. **The world's sound library** (Q29). Proposed in section 10: `FXPK` sections in the world pack, code `0x40 + i`. The owner decides.
4. **`use` and Up.** Up is also the look-up control. Which cells accept `use` and how a press is told from a look is the controls' question (Q25 in the studio's list).
5. **Score and ammo of a rule's own.** The score is worked out from what is taken, not stored. A `score` action needs a stored bonus in the hash, which would change the hash of any level that uses it, and is therefore a `LEVL` 2 matter.
6. **Held key counts.** A count of 0 to 3 for each key colour in place of a flag would change the hash of every level. Default: take it in the `LEVL` 2 change, when the hash moves anyway.
7. **The switch object's inputs.** `by=use,shot,stomp` maps to the switch flags of the object record. Default: that field name, with `use` the default.
8. **The conditions the records do not carry yet.** Ammo at least, score at least and group cleared are numbered and rejected until a level needs them.
