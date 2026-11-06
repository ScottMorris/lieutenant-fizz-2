---
id: scripts
n: 15
title: States, rules and the level script
date: 2026-11-06
summary: The model behind a level's doors and the format an author writes it in. `LEVL` version 2 with states, bindings and 48-byte rules, the wall-kick made a core move, the script dialect `script fizz2 0` and its parser `fizz-script`, the rules specification, and the state table and rule phase in the simulation, where a lock is a plain rule. Three stacks of specification and code, with two re-pins of the golden; the climbing specification that follows them is specified and not built.
prs: [149, 150, 167, 152, 153, 155, 156, 174, 177]
---
Chapter 11 ended with the level as data: the slice's world built from a `LevelSource`, with the locks and the death reset as level data and a lock phase of its own. The next step on the plan was the level script dialect that `fizz-bake` reads. It came with a change of model. A level now declares named **states**; doors **bind** to a state and are passable while it holds one of a set of values; **rules** change states in answer to inputs; and a key-and-door lock is short for one state, one door binding and one rule. There are no switch channels. The model is the owner's, and it was written into three specifications before the code: the bytes in `docs/FAP.md`, the meaning in `docs/RULES.md` and the source in `docs/SCRIPT.md`.

The work landed as three stacks, each a specification with the code above it: `LEVL` version 2 and the wall-kick (#149, {{pr149.state}}; #150, {{pr150.state}}; #167, {{pr167.state}}), the script dialect and its parser (#152, {{pr152.state}}; #153, {{pr153.state}}), and the rules with the state table and rule phase (#155, {{pr155.state}}; #156, {{pr156.state}}). #174 ({{pr174.state}}) links the rules specification to the dialect.

## LEVL version 2 (#149, #167)

`docs/FAP.md` specifies the new layout and drops version 1 with no reader kept, since no pack has shipped. A 32-byte header carries the state, binding, rule and string counts; a state is an 8-byte record, a binding 16 bytes, an object stays at 32 bytes (its `channel` byte becomes a per-kind `aux`, with kinds 6, the switch, and 8, the light, added), and a rule is 48 bytes with two 4-byte conditions and two 12-byte actions. The limits are 32 states of up to 8 values each, 64 bindings of which 32 may be doors, and 32 rules. A binding holds a mask of the state's values that make it active, the same reading for doors, lights, platforms and panels. A door tile keeps no channel, and the touch that opens a lock covers any rectangle: in the replay, Ben opens the red door with his feet in its bottom cell and the blue door in its top cell.

#167 built it. `fizz-pack` gained public records for states, bindings, conditions, actions and rules, a reader whose errors name the offending record (`BadState(i)`, `BadBinding(i)`, `BadRule(i)` and so on) and a writer that refuses input over a limit. `LEVEL_HASH` is 0x46FA651A and `SCHEMA_HASH` 0xA1BB8460, pinned in Rust and recomputed independently by `crates/fizz-pack/tests/levl_hash.py`. The slice is two states of (closed, open), two door bindings over each door's whole run, two lock rules and eleven objects, 2,080 bytes, the figure of the specification's worked example. `tiles::pack_content()` shrank from 847 to 594 bytes with `RULES_SEMANTICS` 2, so the compatibility number of a world or shared pack is now 0x91615AAF and packs made for an earlier build are refused.

## The wall-kick (#149, #150)

`docs/FIZZ1-DIFFERENCES.md` reversed an earlier decision: the wall-kick is a core move of Fizz 2, the same in every level, with no per-level switch. It is Fizz 1's kick, ported from the same source: Jump pressed afresh while touching a wall in mid-air kicks Ben up at 19 tiles a second and away at 6, one kick per side until he lands, with a quarter-second cooldown and not on the pogo stick. A new `Event::Kick` prints as `kick` in the transcript; it has no sound or caption yet. Four new Fizz 1 traces cover it (`wall_kick_chimney`, `wall_kick_single`, `wall_kick_pogo` and `wall_kick_oneway`), so twenty-two traces now pass bit for bit ([chapter 8](chapter:rules)). They were written from a copy of Fizz 1 at its later head `9729c07` with the pinned-commit check bypassed; the pull request records that Fizz 1's `crates/` and `episodes/episode-1/game` are identical between that commit and `4bce036`. The slice never kicks, so no `EVENT` line moved.

## The golden, re-pinned twice

The kick's two fields, `wall_t` and `kick_side`, joined the state hash unconditionally in #167, and the doors' bits came to be indexed by binding, so the golden moved once, from `843736be6fdaeb76` to `eda8a62f036aeab6`, with all 83 `EVENT` lines unchanged (a test pins their digest). The wider chicken of #168 moved it again, to `7f6a1a1248c4b0e8` ([chapter 12](chapter:oblique)). The rule phase of #156 then moved it to `{{golden.final}}`, for two reasons the specification requires: the slice declares two states, so its hash carries the state block from tick 0, and a door that opens raises its event at the centre cell of its rectangle. Exactly two `EVENT` lines changed, `EVENT 889 dooropen 248 352` to `248 344` and `EVENT 1615 dooropen 360 682` to `360 632`. The owner decided the second change on 6 November 2026; `docs/RULES.md` records it as its first open question, now closed.

## The script dialect (#152)

`docs/SCRIPT.md` specifies `script fizz2 0`, the dialect that the level studio and the baker both read. It keeps the lexical habits of Fizz 1's level script language (one statement to a line, `key=value` fields, `//` comments, stable error codes, a lossless tree) and replaces its vocabulary with what a Fizz 2 level holds: the size and name, the start, the floor, the map rows, the objects with names, `state`, `door`, `when`, `lock` and the strings. A lock written as `lock red-door key=red at=15,21` expands to:

```text
state red-door values=closed,open
door x=15 y=20..22 state=red-door open=open
when touch x=15 y=20..22 if state red-door is closed and holding key-red then set red-door open, spend key-red
```

The specification also gives the catalogue of error codes and lints with stable numbers, a dump view that shows each expansion, the lossless-tree and canonical-printer requirements, and a conformance corpus. Lights, switches, platforms, `music`, `allow` and `world` are reserved words that give E102 until a later stage defines them. A Fizz 1 conversion writes each channel as a two-valued state named `channel-N`.

## fizz-script (#153)

`crates/fizz-script` is a host-only `std` library of about 5,700 lines: a lossless parser whose tree keeps every byte, a canonical printer, a validator, the expansion of `lock` and the lowering to `fizz_ep2::level::LevelSource`. Lowering builds the records through `fizz-pack`'s types, encodes them, reads them back with `LevelView::new` and passes them to `level::validate`. A valid level that the game cannot build yet lowers to `Unbuilt` with the line and the reason, not to a diagnostic. The corpus in `crates/fizz-script/corpus/` holds the slice as a script, whose lowering has the same `level::digest` as `SLICE_SOURCE` and builds a world with the same hash; two more valid levels (`tiny.fizz` and `two-doors.fizz`); 25 error files covering the 21 error codes the validator can produce, and a file for each of the 7 active lints; and a formatter pair. The tests check that parse then print is stable over the corpus and 400 generated inputs, that the lossless printer returns every input, and that every active code has a corpus file. The crate has no consumer in the game: it is the front end that `fizz-bake` and the studio will share.

## The rules and the rule phase (#155, #156)

`docs/RULES.md` specifies what the records mean: the state table, bindings, the inputs (switches, enter, take, touch, use, hit, stomp, stun, defeat, a group cleared, time, a state becoming a value or changing), the conditions and actions, the order inside a tick, the death reset, raised events, the hash block and a preview API for native hosts. The order is part of the parity contract. The rule phase is step 6 of a tick, after the shots; rows run in table order, a row sees the changes of the rows before it, and each fires at most once a tick. Bindings are recomputed in step 7, and a door that becomes passable raises its event there.

#156 built the part the slice needs. `fizz_ep2::states` holds the table, `rules.rs` reads a rule record into one input, two conditions and two actions, and `Doors::apply` works out each door binding's bit from its state and active set. Built: `touch` over a rectangle (Ben's front probe or his body, sampled in his contacts where Fizz 1 tried its locks), `enter`, the conditions "state in a set", "holds" and "does not hold", and the actions `set`, `toggle`, `next` and `spend`. Every other input, condition, action and any delay is refused at load (`RuleError::Unbuilt` and its siblings), never ignored. The lock is a plain `touch` row with no phase of its own, so a key Ben takes on the tick he reaches the door now counts on that tick. The death reset returns every state to its start value, clears the rule memory and recomputes the door bits with no event. `World::set_level` builds a world the way the level loader does. The preview API, delays, the other inputs and actions, and the light, platform and panel bindings are not built.

## Ladders and vines, specified (#177)

`docs/CLIMBING.md` ({{pr177.state}}) specifies ladders and vines for review, with no code: what Fizz 1 does at `4bce036`, the tiles `RUNG`, `RUNG_TOP` and a vine tile, the controls, the rules, the hash (the climb flags in free bits of Ben's flags byte, so a climbing build leaves the golden unchanged), how climbing meets the wall-kick, the pogo stick, shots and the chicken, and a staged build. Its twelve open questions, each with a recommendation, wait for the owner.

## The shelves

### Catalogued
- `LEVL` version 2 and the wall-kick (#149, #150, #167) landed as one merge commit, `{{pr167.merge}}`: #149 with CI {{pr149.ci}}, #150 with CI {{pr150.ci}}, #167 with CI {{pr167.ci}}. `cargo test -p fizz-pack` covers every reader check with its record index, the writer's refusals, the slice's 2,080 bytes and the pinned hashes, which `python3 -I crates/fizz-pack/tests/levl_hash.py` recomputes.
- The wall-kick: `cargo test -p fizz-ep2 --test fizz1_traces` runs all twenty-two Fizz 1 traces bit for bit, the four kick traces among them; `tools/check.sh` runs them through `cargo test --workspace`.
- `docs/SCRIPT.md` (#152, CI {{pr152.ci}}) and `fizz-script` (#153, `{{pr153.merge}}`, CI {{pr153.ci}}): `cargo test -p fizz-script` runs the corpus, the round trips and the syntax tests, including the slice script's digest equal to `SLICE_SOURCE`'s.
- `docs/RULES.md` (#155, CI {{pr155.ci}}) and the state table and rule phase (#156, `{{pr156.merge}}`, CI {{pr156.ci}}): `crates/fizz-ep2/tests/state_rules.rs` has 13 tests (active sets, lock-as-rule ordering, touch as a level, the death reset, the hash block and the refusals).
- The golden ends `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}` on native, Web and DOS: `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}` (`snapshots/measured.json`).

### Uncatalogued
- No `.FAP` file is written or loaded yet: the `LEVL` 2 bytes exist in tests and in `SLICE_SOURCE`, and the script's lowering reaches a `LevelSource` in memory only.
- The wall-kick played by a person, on any screen. The slice has no place that needs it, and the four traces are scripted.
- The kick traces were generated from Fizz 1 at `9729c07`, not at the pinned `4bce036`; the two are identical in the directories the tool builds from, as recorded in the pull request, and nothing in CI regenerates them.
- `docs/CLIMBING.md` describes a design; nothing in it has run.

### Wish list
- The rest of the rule phase: the other inputs, the actions `give`, `score`, `ammo`, `word` and `cue`, delays, the light, platform and panel bindings, and the preview API.
- `fizz-bake` reading `script fizz2 0`, and the pack loaders on both platforms.
- The owner's answers to the open questions of `docs/RULES.md` (world-scoped states, the world's sound library, `use` and Up, held key counts) and of `docs/CLIMBING.md`.
- A caption and a sound for the kick.

> **From the workbench — Lossless syntax trees.** A lossless (or "full-fidelity") syntax tree keeps every character of its source, whitespace and comments included, so printing the tree gives the file back byte for byte. Tools that rewrite source, such as formatters and refactoring engines, use one so that an edit changes only what it means to. Roslyn, the C# compiler platform, and rust-analyzer's `rowan` are two well-known designs.
> Source: [Roslyn syntax, Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/csharp/roslyn-sdk/work-with-syntax)
