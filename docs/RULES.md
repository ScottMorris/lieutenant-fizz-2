# Rules: states, bindings and events

This document specifies what a level's rules mean to the simulation: the state table, the bindings that make states visible in the world, the inputs rules react to, their conditions and actions, the order everything runs in each tick, the death reset, raised events and the preview API that native hosts use. It owns the semantics only. The byte layout of the records is [`FAP.md`](FAP.md) (the `LEVL` section), and the source dialect that authors write is the level script language's.

The design is the owner's: locks are states with rules. A level declares named states, doors and other things bind to a state, and rules change states in answer to inputs. A key is one input among many, and `lock` stays as short sugar for the key-and-door case. The sim today runs only locks; this document says where the rest goes and the order it runs in, so that every platform computes the same state hash.

Status words used below: **built** means the code runs it today, **next** means it is the first thing the state table package builds, and **planned** means specified here and not yet built.

## 1. Contract with the other work

These limits are shared with the `LEVL` 2 layout and the script dialect, and are the studio's proposed defaults for its questions Q23 and Q24.

- A level declares up to **32 states**. Each has **2 to 8 values** (numbered from 0) and a **start value**.
- A rule record is fixed-size and holds **2 conditions and 2 actions**. A rule that needs more is split into several rules chained through a state.
- A level has up to 32 rules (one bit per rule in a `u32` of rule memory, as today).
- The **death reset** restores every state to its start value.
- **`lock`** stays as sugar for the key-and-door case (section 9).
- Names are source-level only. A state, an object and a string are numbered in the baked level, in declaration order, and the baker resolves every name to a number. The sim never sees a name.

## 2. The state table

A state is a small integer with a fixed number of values. The table is a list of at most 32 entries: `values` (2 to 8) and `start` (below `values`). The current value of each state is world state. A state is changed only by rule actions (section 6) and by the death reset (section 8); nothing else writes it.

A **switch channel** (the 32 bits of `map.switches` that switched tiles read) is a built-in two-valued state: value 0 is off and value 1 is on, and a switched tile is solid while its channel is on. The channels exist today and the lock phase uses them. They are not part of the declared table, and a declared state does not share a number with a channel. Whether `LEVL` 2 folds channels into the table (a switched tile naming a state and the values at which it is closed) is open question 1; until then both exist, joined by bindings.

Arithmetic on a state is closed: `set` takes a value below `values`, `next` wraps from the last value to 0, and a record that names a value out of range is refused at load, not clamped at run time.

## 3. Bindings

A binding attaches something in the world to a state, so that the state is what the world shows.

| Binding | What it does | Status |
| --- | --- | --- |
| Door | A switched-tile channel is on (the door is closed) exactly when the state's value is in a **closed set** (a mask over the 8 values) | next |
| Light | A placed light is lit when the state's value is in a lit set | planned (F12) |
| Platform | A moving platform runs, or is held at an end, according to the state's value | planned |
| Panel | A parallax panel layer is drawn when the state's value is in a shown set | planned (F19) |

A binding is **derived, not stored**. The sim recomputes a bound channel from the state at two points: once after the rule phase of every tick in which any state changed, and once when the world is built or reset. Nothing hashes a bound channel's effect twice: `map.switches` is already hashed, and a bound channel's bit follows the state, so two worlds with the same state values have the same bits.

A channel that has a binding is owned by it: a lock or a legacy channel action must not also write that channel, and the loader refuses a level where both do. The cost of a binding is one mask test per bound channel, only when a state changed.

## 4. Inputs

An input is something that happened this tick. Phases that already exist raise inputs as they run; the rule phase (section 7) reads them. A rule has exactly one input, in its `trigger`.

| Input | Raised when | Where it is raised | Status |
| --- | --- | --- | --- |
| `lock` | Ben holds the key and the tile in front of him is the lock's closed door | Ben's contacts | built |
| `start` | The first rule phase of a life: tick 1 of the level and the first tick after each death | The rule phase | planned |
| `enter` | Ben's body comes into a rectangle of cells (an edge: not inside last tick, inside this tick) | After the shots | **next** |
| `take` | Ben takes a key or snack object this tick | Ben's contacts | planned |
| `touch` | Ben's body overlaps a switch object or a cell this tick (a level, not an edge; a rule may repeat) | Ben's contacts | planned |
| `use` | Ben presses Up while overlapping a cell or switch object | Ben's contacts | planned |
| `hit` | Ben is hurt by a hazard or an enemy (before the death check) | Ben's contacts | planned |
| `stomp` | Ben stomps a named chicken or enemy, or any | Ben's contacts | planned |
| `stun` | A shot stuns a named chicken or enemy, or any | The shots | planned |
| `defeat` | A named enemy is defeated (stunned for good or killed, per its definition) | The shots, contacts | planned |
| `cleared` | Every member of a group of objects has been taken or defeated, on the tick the last one is | The phase that finished the last one | planned |
| `shoot` | A shot enters a rectangle of cells | The shots | planned |
| `time` | The life's tick count reaches `at`, and every `every` ticks after (at least 6) | The rule phase | planned |
| `change` | A named state has changed value since this rule last ran | The rule phase | planned |

Inputs are sampled, not queued. A phase that raises an input records it in a per-tick record (a bit per rule, plus the index of the object involved) and the record is cleared at the start of the next tick. Two raisings of the same input on one tick are one input for rule purposes; a rule fires at most once per tick.

`enter` and `shoot` take the rectangle from the record, both corners included, in tile cells with row 0 at the bottom. Ben is inside a rectangle when his body box overlaps the box of the cells (the half-open region from the low corner to one past the high corner). The previous tick's insideness is rule memory (section 8).

## 5. Conditions

A rule holds up to 2 conditions, and the rule fires only when all that are present hold. They are read before any action of the rule runs, in the state the rule phase has reached (section 7).

| Condition | True when |
| --- | --- |
| State is | A state's value equals a value (or a channel is on, or off) |
| State in | A state's value is in a mask of values |
| Key held | Ben holds the key of a colour |
| Ammo at least | The Fizz Blaster has at least that many shots |
| Score at least | The score is at least that many points |
| Group cleared | Every member of a group has been taken or defeated (a level, not an edge) |

The record's existing **guard** (`GUARD` with a channel, true when the channel is on, or off with `GUARD_ON` clear) is a state-is condition on a channel and fills one condition slot. A rule with no condition always passes.

## 6. Actions

A rule holds up to 2 actions, run in slot order after its conditions pass. Each takes effect at once, so the second action and every later rule see the first.

| Action | Effect |
| --- | --- |
| `set` | Set a state (or channel) to a value |
| `toggle` | Flip a two-valued state or a channel (a state with more than two values is refused for toggle: use `next`) |
| `next` | Advance a state to its next value, wrapping |
| `spend` | Take a key of a colour from Ben, or an amount of ammo (a rule that spends what Ben does not have does not run: it is a condition too, and the loader adds the matching key-held or ammo condition itself) |
| `give` | Give Ben a key of a colour |
| `score` | Add points to the score |
| `ammo` | Add shots to the blaster |
| `raise` | Raise an event: a floating word or a cue (section 10) |

`set`, `toggle` and `next` that leave the value unchanged do not count as a change. A change marks the state as changed for `change` rules and for bindings. `score` and `ammo` additions are recorded as world state that the hash already covers, through the snack and ammo fields (extended when a rule can give score of its own: open question 6).

## 7. Order inside a tick

This order is part of the parity contract: native, Web and DOS must agree on it, because the state hash after each tick depends on it. Existing phases keep the order they have today (see `World::step`); the rule phase is added after them, never between them, so that nothing the sim already does moves.

1. The tick counter advances and last tick's events and input record are cleared.
2. Ben's movement: the jump or bounce, the shot, the head bump.
3. Ben's contacts, in Fizz 1's order: hazards and the exit, **the locks in rule order**, the keys, the snacks, the chickens (stomp and hit raise their inputs here). If Ben dies, the contacts end.
4. Chickens and enemies move, in index order.
5. Shots move, from the last to the first (stun and defeat raise their inputs here).
6. **The rule phase.** Rows run in table order, 0 to 31:
   1. For each row whose input was raised this tick (or, for `change`, whose watched state changed since the row last ran), whose once-per-life memory allows it, and which has not fired this tick: evaluate the conditions.
   2. If they all hold, run the actions in slot order, set the row's fired memory and queue its cue.
   3. Move to the next row.
   A row sees every change made by an earlier row this tick, so an earlier row can arm a later one in the same tick. A change made by a later row reaches an earlier `change` row on the next tick. Each row fires at most once per tick, so a chain of `change` rows cannot loop and the phase costs at most 32 evaluations.
7. **Bindings** are recomputed if any state changed in this tick (section 3).
8. The death check and the respawn, as today. A respawn runs the death reset (section 8).

Lock rules stay in step 3 because the door has to open on the tick Ben presses against it, before the keys and snacks are read, as in Fizz 1. All other rule inputs wait for step 6 so a rule reads the world after it has fully moved. The events a rule queues are appended to the world's event list after the events of steps 2 to 5, in row order.

The input record is what makes the order independent of presentation: no rule reads a presenter, a clock or a random draw.

## 8. The death reset

A death reloads the level in effect (as Fizz 1 reloads). The reset restores, in this order:

1. Ben, ammo, shots, snacks and chickens (as today).
2. `map.switches` to the level's start channels (as today).
3. **Every declared state to its start value.**
4. Rule memory: each row's fired bit and each edge row's inside bit are cleared, and the life's start tick becomes the current tick, so `time` rules count from the respawn.
5. Bindings are recomputed from the state table.

The `start` rows then fire in the first rule phase after the respawn, as they do on tick 1. A world with no declared states, no non-lock rules and no rule memory resets exactly as it does today. States are **level-scoped**: a world-scoped state is open question 2.

## 9. `lock` as sugar

A `lock` record says "a key of this colour, spent on a closed door whose tile reads this channel, opens the door". It means a rule with the input `lock`, a condition (the channel is on), the actions `spend key` and `set channel off`, and a raised `door` event with the existing `Event::DoorOpen`. The sim keeps its dedicated lock phase (step 3) because the probe of the tile in front of Ben is geometry, not a rectangle. `LEVL` 2's script dialect may write `lock` as one line; the baker emits the lock record. Nothing about the slice's two locks changes.

## 10. Raised events and the sound library

A rule's `raise` action, and later an object, raises an event a presenter shows or plays: a **word** (a floating caption) or a **cue** (a sound). The record names a shared caption id, a level string (`0x40 + i`), a shared effect id or a world sound. The sim emits the event with its cell; each presenter draws and plays it, and the transcript records it as an `EVENT` line so Web and DOS can be compared. Events are output only and are not hashed.

**Where the world's sound library lives (open question 3, proposed answer for the owner).** The world's own sounds are `FXPK` sections in the world pack, `<WORLD>.FAP`, three of them in the OPL3, OPL2 and speaker order that `SHARED.FAP` uses. The `FXPK` layout is already fully specified per device in [`FAP.md`](FAP.md), a world pack already holds the per-device audio hash in its compatibility number, and a world pack is loaded in full when a level of the world starts, so the library needs no new section tag and no new compatibility number. An effect code below `0x40` is a `SHARED.FAP` effect, as today; `0x40 + i` is the world's sound `i`, the same split the caption codes use. `SHARED.FAP` keeps the built-in effects. The alternative is a separate `SNDS` section; it would duplicate the index and the device sections for no gain. A world with no sounds has no world `FXPK`, and a rule that names a world sound beyond the library is refused at load. This is a proposal: the owner decides it.

On DOS a world sound with no speaker form plays nothing there, and a caption draws in the 8-by-8 font at the same place; this is the reduction DOS is allowed.

## 11. The hash

State enters the world hash only when it exists. A world with at least one declared state, or at least one rule that is not a lock, appends one block to the end of the hash after the snacks: the state count (`u32`), each state's current value (`u8`) in table order, the fired memory (`u32`), the inside memory (`u32`) and the life's start tick (`u64`). A world with neither hashes exactly as before, so the slice's golden (`FINAL TICK 1757 HASH a08501a57d509e8a`) and the `LEVL` digest are unchanged until `LEVL` 2 changes the slice's level on purpose. The input record, the queued events and the bound channel bits are not separately hashed: the first two are cleared every tick and the third is part of `map.switches`.

## 12. Mapping onto the existing records

`fizz-pack`'s `RuleRec` (version 1 of `LEVL`) is the starting point, and `LEVL` 2 replaces it. The mapping from today's constants:

| Record | Meaning here |
| --- | --- |
| `TRIGGER_LOCK` (1) | `lock` (built) |
| `TRIGGER_START` (2) | `start` (planned) |
| `TRIGGER_ENTER` (3) | `enter` (**next**), the rect from the union |
| `TRIGGER_TAKE` (4) | `take` (planned), `subject` is the object index |
| `TRIGGER_SHOOT` (5) | `shoot` (planned) |
| `TRIGGER_STOMP` (6) | `stomp` (planned) |
| `TRIGGER_STUN` (7) | `stun` (planned) |
| `TRIGGER_TIME` (8) | `time` (planned), `at` and `every` from the union |
| 9, 10 | Reserved for `touch` and `hit` (the gadget milestone); `LEVL` 2 gives all the inputs above their own numbers |
| `OP_ON`, `OP_OFF`, `OP_TOGGLE` on `channel` | `set` the channel to 1, `set` it to 0, `toggle` it |
| `FLAG_REPEAT` | The row may fire again after Ben leaves and re-enters (an edge row), or on every period (`time`); without it a row fires once per life |
| `FLAG_GUARD`, `FLAG_GUARD_ON`, `guard_ch` | One state-is condition on a channel |
| `delay` | Ticks between the trigger and the actions (planned; a row with a delay is refused until it is built) |
| Slot A event cell | Where the switch's event is reported |
| Slot B effect, caption and cue cell | A `raise` action (planned) |

Version 1 records have only a channel to act on, no state number, no second condition and no second action. A level that declares states, uses `next`, `spend`, `give`, `score`, `ammo` or two conditions needs `LEVL` 2, which the other work lays out. The sim's `Rule` form is built so that both records decode into it.

## 13. The preview API (F6)

A native host (the level studio) needs to build a world from baked bytes, step it by inputs and read what happened, without a presenter. The API lives in `fizz-ep2`, is `no_std` plus `alloc` like the rest, and wraps what exists:

- `Preview::from_level(bytes: &[u8]) -> Result<Preview, PreviewError>` reads a `LEVL` section with `fizz_pack::LevelView::new` and builds the world with `world_from_view`. Its errors keep the pack's `LevelError` and the sim's `SourceError` apart so a host can say which failed.
- `Preview::step(&mut self, input: InputFrame)` advances one tick; `run(&mut self, inputs: &[InputFrame])` advances many and returns the hash after each.
- `Preview::events(&self)` is the last tick's `Event` list. `Preview::hash(&self)` is the state hash. `Preview::states(&self)` is the state table's current values, and `Preview::fired(&self)` is the rule indices that fired on the last tick, in order, for the debugger.
- `Preview::reset(&mut self)` rebuilds from the same bytes, and a preview is `Clone`, so a host can snapshot and rewind.

A preview of a level is the same code path as the game, so a rule preview that agrees with the studio's lowering also agrees with the Web and DOS builds. The hash it reports is the arbiter, as everywhere else.

## 14. Cost on the DOS floor

The state table is at most 32 bytes of values; the rule memory is 8 bytes plus the life's start tick. The rule phase touches at most 32 rows per tick, each one an input-bit test, up to two integer comparisons, and up to two small writes; `enter` adds one box overlap in `f64` per row (a handful of compares, no `exp`, nothing reordered). That is in the low hundreds of integer operations and at most 32 box tests a tick, against a step that the platform document measures at about 103 microseconds of a 16,667 microsecond tick on a 486DX2-66 in DOSBox. Bindings run only on a tick where a state changed. Nothing in the phase allocates: the tables are read once at load and the memory is fixed-size. The DOS limits the design must not carry back to the Web are cosmetic ones: lights and panels bound to a state step between banks without fading, a world sound without a speaker or OPL2 form is silent on that device, and captions use the 8-by-8 font.

## 15. Open questions

1. **Do channels survive `LEVL` 2?** One answer folds them in: a switched tile names a state and the values at which it is closed, and channel numbers disappear. The other keeps 32 channels as built-in two-valued states beside the declared ones. The first is simpler for authors; the second is what the slice's two doors are today. Stack A owns the layout, and this document works with either.
2. **World-scoped states** (the studio's Q27). Proposed: level-only for now, so a death reset stays exact. A world state would need saving with progress and a rule for what a death resets.
3. **The world's sound library** (Q29). Proposed in section 10: `FXPK` sections in the world pack, code `0x40 + i`. The owner decides.
4. **Delay.** Records have a delay in ticks between trigger and actions. Does a delayed action survive a death (no: the reset drops it), and may it be re-triggered while pending (proposed: no, a pending row ignores its trigger)?
5. **`use` and Up.** Up is also the look-up control. Which cells accept `use` and how a press is told from a look is the controls' question (Q25 in the studio's list).
6. **Score and ammo of a rule's own.** The score is worked out from what is taken, not stored. A `score` action needs a stored bonus in the hash, which would change the hash of any level that uses it, and is therefore a `LEVL` 2 matter.
7. **Group definition.** Is a group a list of object indices in the record, a tag on each object, or a rectangle? The record has room for neither today.
8. **Conditions the records may not carry.** Are two conditions always an AND? Proposed: yes; an OR is two rules.
9. **The pack compatibility number.** Accepting `enter` rules does not change the bytes of any record, so `RULES_SEMANTICS` stays 1; the first change to what an existing record means (and `LEVL` 2) bumps it.
