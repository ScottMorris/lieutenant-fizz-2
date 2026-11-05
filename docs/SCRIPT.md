# The Fizz 2 script dialect

This document specifies `script fizz2 0`, the text format in which Fizz 2 levels are authored, and the tool behaviour around it: the lossless syntax tree, the canonical printer, the validator and the lowering to `fizz_ep2::level::LevelSource`, which the baker (`fizz-bake`) writes as a `LEVL` section. It is the source of truth for the grammar: the crate `fizz-script` follows it, and where the two differ the crate is wrong. The byte layout of `LEVL` belongs to [`FAP.md`](FAP.md) and the meaning of rules and states to `RULES.md`; this document does not restate either. [`ARCHITECTURE.md`](ARCHITECTURE.md#assets-and-packs) says why levels are baked and not parsed at run time.

## Contents

- [Scope and stages](#scope-and-stages)
- [Lexical rules](#lexical-rules)
- [Statements](#statements)
- [Inputs, conditions and actions](#inputs-conditions-and-actions)
- [`lock`, the sugar](#lock-the-sugar)
- [The dump view](#the-dump-view)
- [The legend](#the-legend)
- [Lowering to `LevelSource`](#lowering-to-levelsource)
- [Errors and lints](#errors-and-lints)
- [The lossless tree and the printer](#the-lossless-tree-and-the-printer)
- [The conformance corpus](#the-conformance-corpus)
- [Extension points](#extension-points)

## Scope and stages

The dialect descends from Fizz 1's level script language, which rebuilt all sixteen Fizz 1 levels exactly. It keeps that language's lexical habits (one statement to a line, `key=value` fields, `//` comments, stable error codes, a lossless tree) and replaces its Fizz 1 builder vocabulary with what a Fizz 2 level holds. A Fizz 2 file starts with `script fizz2 0` and the version never changes under a file; a Fizz 1 script has no `script` line and is not read by this parser (it gets E007, which says so).

Version 0 covers what `LEVL` version 2 carries: the level's size and name, the start, the floor, the tile map, the objects (keys, snacks, soda cans and the chicken, each with a name), **states**, the **door bindings** that make tiles passable by a state's value, the **rules** that change states (`when`), `lock` as a short way to write the key-and-door case, and the level's strings. It lowers to a `LevelSource` and so to the `LEVL` bytes, and the slice level written in it has the same `level::digest` as `level::SLICE_SOURCE`.

One model is written three ways. `state` declares values by name; `door` binds switched tiles to a set of a state's values; `when` changes states in answer to inputs; and `lock` is sugar that expands to exactly one state, one door binding and one rule (see [`lock`, the sugar](#lock-the-sugar)). There are **no channels** in the dialect: a Fizz 1 channel is a two-valued state with a door wired to it, and every case it covers is written with a state.

Version 0 does not write lights, switches, platforms or custom objects. Their words are reserved (see [Extension points](#extension-points)) and give E102 until a later stage defines them. Where a statement below refers to such an object (a `use` of a switch), the statement parses and the name does not resolve (E002) until the object does.

The crate arrives in stages: the parser and printer for these statements, then the expansion and lowering, then the rule phase in the simulation. Until the crate implements a statement, this document leads and the crate is the one that is wrong.

Stage owners: the byte layout of `LEVL` is `FAP.md`'s, and the semantics of rules and states are `RULES.md`'s. A lowering rule in this document that restates one of them is a mistake in this document.

## Lexical rules

- A file is UTF-8 text. A line ends with `\n` or `\r\n`; the lossless tree keeps whichever each line used, and the canonical printer writes `\n`. A lone `\r`, or any other control character except a tab, is E004.
- One line holds at most one statement. Blank lines and comment lines are lines.
- `//` starts a comment that runs to the end of the line. It is not a comment inside a quoted string or between the bars of a map row.
- Indentation is ignored. The canonical form indents map rows and objects by two spaces and everything else by none.
- Tokens are separated by spaces or tabs. A token that ends in a comma joins the next token (`values=closed, open` reads as `values=closed,open`).
- A **word** is lower-case: a letter, then letters, digits and hyphens (`key-red`, `floor`). A **name** (an `id`, a state name, a lock name, a string name) is a word-like token that may start with a digit: `[a-z0-9][a-z0-9-]*`, at most 32 bytes (E109 beyond that; E004 if it has other characters). A state's **value names** are words.
- A **number** is a plain decimal: an optional `-`, digits, and an optional `.` with digits. No exponent, no leading `+`, no bare `.5`. It is read with Rust's correctly rounded `str::parse::<f64>`, so `11.7` gives the same bits as the `11.7` literal in `level.rs`. An **integer** is a number with no fraction.
- A **field** is `key=value` with no spaces around `=`. After a statement's leading words, fields may come in any order. A field given twice, or a field the statement does not take, is E009.
- A **string** is text between double quotes. There is no escape: a string cannot hold `"`, and one that is not closed on its line is E004.
- A **pair** is two numbers joined by a comma (`2.5,11.7`); a **size** is two integers joined by `x` (`32x48`). A **range** is an integer, or two integers joined by `..` (`20..22`), the low one first. A **list** is words or integers joined by commas (`closed,open`).
- A field holds one of three things, each with one spelling: a **point** (`at=X,Y` exact, or `x=I y=J` for a cell centre, on objects and `start`), a **cell** (`at=I,J` with integers, on `lock`, `touch`, `word` and `cue`) or a **rectangle** (`x=RANGE y=RANGE`, on `door`, `enter`, `shoot` and `touch`). An `at=` that names a cell and has a fraction is E009.

## Statements

```text
file      = script-line { line } ;
script    = "script" "fizz2" "0" ;
level     = "level" STRING SIZE "id=" NAME [ "ground=" INT ] ;
start     = "start" PAIR ;
floor     = "floor" ( "open" | "solid" ) ;
state     = "state" NAME [ "values=" WORDLIST ] [ "start=" WORD ] ;
door      = "door" "x=" RANGE "y=" RANGE "state=" NAME "open=" WORDLIST ;
when      = "when" INPUT [ "if" COND [ "and" COND ] ] [ "after=" INT ] "then" ACTION { "," ACTION } [ "repeat" ] ;
lock      = "lock" NAME "key=" ( "red" | "blue" ) "at=" INT "," INT ;
string    = "string" NAME STRING [ "style=" STYLE ] ;
theme     = "theme" WORD ;
map       = "map" ;                    (* then rows *)
row       = "|" TILES "|" [ INT ] ;
objects   = "objects" ;                (* then object lines *)
object    = OBJWORD "id=" NAME ( "at=" PAIR | "x=" INT "y=" INT ) [ "dir=" ( "1" | "-1" ) ] ;
OBJWORD   = "key-red" | "key-blue" | "cheezie" | "choc" | "cookie" | "soda" | "chicken" ;
STYLE     = "white" | "cyan" | "red" | "green" | "blue" | "yellow" | "grey" ;
INPUT     = "start" | "enter" RECT | "take" ID | "shoot" RECT | ( "stomp" | "stun" | "defeat" ) ( ID | "any" )
          | "time" "at=" INT [ "every=" INT ] | "touch" ( RECT | CELL | ID ) | "use" ID | "hit" ID
          | "cleared" "group=" NAME | "state" NAME "becomes" WORD | "state" NAME "changes" ;
COND      = "state" NAME "is" [ "not" ] WORDLIST | [ "not" ] "holding" KEYWORD ;
ACTION    = "set" NAME WORD | "toggle" NAME | "next" NAME | "spend" KEYWORD | "give" KEYWORD | "score" INT | "ammo" INT
          | "word" ( NAME | STRING ) [ "style=" STYLE ] CELL | "cue" NAME CELL ;
RECT      = "x=" RANGE "y=" RANGE ;
CELL      = "at=" INT "," INT ;
ID        = "id=" NAME ;
KEYWORD   = "key-red" | "key-blue" ;
```

| Statement | Meaning | Lowers to |
| --- | --- | --- |
| `script fizz2 0` | The dialect and its version. It must be the first statement | Nothing |
| `level "Name" WxH id=NAME` | The level's name (up to 48 printable ASCII bytes, E109 otherwise), its size in tiles (each 1 to 65535) and its file-name slug. `ground=` is a Fizz 1 field, accepted and ignored with W112 | `name`, `w`, `h`. The slug is for file names and the manifest; the `LEVL` section stores none |
| `start X,Y` | Ben's start: the lower-left corner of his body, in tiles. It must lie in the map (E108). Required (E010) | `spawn` |
| `floor open` or `floor solid` | Whether the bottom of the map is solid. Default `open`, as in the slice | `floor_solid` |
| `state NAME [values=V0,V1,...] [start=V]` | A state: a small number with named values. Values are words, unique within the state (E105), 2 to 8 of them (E109); value `i` is the `i`th word, which is the number the record and every rule hold. The default set is `off,on`. `start` names the value the level begins with and a death restores; it defaults to the first value. Every state counts toward the 32 of a level (E111) | One state record, numbered in file order, counting the states that `lock` lines declare at the lock's line |
| `door x=RANGE y=RANGE state=NAME open=V[,V...]` | Binds every switched tile in the rectangle to a state: the tiles are passable while the state's value is one of the listed values, and solid otherwise. A binding holds a **set** of values, so a door that is open for two of three values is one line. Every switched tile in the map must be covered by exactly one door binding, and a rectangle may hold only switched tiles (E110); the rectangle must be inside the map (E117). A level has at most 32 door bindings (E111). Anything not bound is unaffected by states | One binding record of kind 1, whose mask has a bit for each listed value |
| `when INPUT [if COND [and COND]] [after=TICKS] then ACTION[, ACTION] [repeat]` | A rule: when the input happens, if up to two conditions hold, after the delay, run up to two actions. Without `repeat` a rule fires once per life. See [Inputs, conditions and actions](#inputs-conditions-and-actions) | One rule record, in file order |
| `lock NAME key=K at=I,J` | A key of colour `K` spent on a closed door. The cell `I,J` is any cell of the door's run and must hold a switched door tile (E110) inside the map (E117). It is sugar for one state, one door binding and one rule; see [`lock`, the sugar](#lock-the-sugar) | What its expansion lowers to, at the lock's place in file order |
| `string NAME "text" [style=S]` | A caption the level carries. The text is 1 to 30 bytes of printable ASCII plus the middle dot, en and em dashes, curly quotes and the ellipsis (E112 otherwise); the style defaults to `white` | One `StringRec`, in file order |
| `theme NAME` | Reserved: accepted and printed, not baked (W109) | Nothing |
| `map` | Starts the tile grid: the rows that follow it, top row first | `tiles` |
| `objects` | Starts the object list: the object lines that follow it | `objects` |

A **map row** sits between `|` bars, holds exactly `w` letters of the [legend](#the-legend) (E103 on a wrong count, E104 on an unknown letter), and may end with a row label that gives its `y`. `y` counts up from the bottom: the first row is `h - 1` and the last is 0. A label that is not the row's `y` is E103; the canonical printer always writes the label. There must be exactly `h` rows (E103).

An **object line** names a kind, a required `id=`, a position and, for a chicken, a required `dir=` of `1` or `-1` (E109 for another value, E003 when missing, E009 on any other kind). Positions are in tile units (cells), never pixels, fractions allowed, and `y` counts up from the bottom row (only the `map` rows are written top first). The position is `at=X,Y`, an exact point in tiles for anything off a cell centre, or `x=I y=J`, which names a cell and is the centre of cell `I,J` and means `at=I+0.5,J+0.5`. The centre must lie in the map (`0 <= x < w`, `0 <= y < h`, E108). What the point means depends on the kind exactly as in `level.rs`: the centre of a key or snack, the spawn of a chicken (its feet and centre), while Ben's `start` is the lower-left corner of his body.

- **Order.** Objects are lowered stably sorted by kind (red key, blue key, snacks, chickens), same-kind objects keeping file order, because the state hash indexes snacks and chickens by position. The file may list them in any order; the canonical printer groups them the same way. A second key of one colour is E107.
- **Names.** Every object, state (a lock's name is its state's name) and string has a name, and one name may be used once in a file (E105). The `LEVL` section holds no name; the baker resolves names to indices.
- **Order.** States are numbered in file order, counting the state a `lock` declares at the lock's line. Door bindings, rules and strings are numbered in file order too. `when` and `lock` share one order, which is the order the sim evaluates rules; the order of strings is the order of `string` statements.

Statements may appear in any order after `level`, with these constraints. `script` is first, then `level` (E007 for a statement before either). `script`, `level`, `start`, `floor`, `theme`, `map` and `objects` appear at most once (E006). Rows belong to the `map` block and object lines to the `objects` block, which run until the next statement that is not a row or an object line; a row or object line elsewhere is E008. `state`, `door`, `when`, `lock` and `string` may repeat.

## Inputs, conditions and actions

A rule reads `when INPUT [if COND [and COND]] [after=TICKS] then ACTION[, ACTION] [repeat]`. What each part means in the simulation is `RULES.md`'s; this table is the spelling and what it lowers to, with the numbers in [`FAP.md`](FAP.md).

| Part | Dialect | Lowers to |
| --- | --- | --- |
| Inputs | `start`; `enter x=R y=R`; `take id=ID`; `shoot x=R y=R`; `stomp`, `stun` or `defeat` with `id=ID` or `any`; `time at=T [every=T]`; `touch x=R y=R`, `touch at=I,J` (a cell) or `touch id=ID`; `use id=SWITCH`; `hit id=SWITCH`; `cleared group=NAME`; `state NAME becomes V`; `state NAME changes` | Triggers 2 to 15. `touch` over a rectangle is trigger 9 and over an object trigger 10; `changes` is trigger 15 with `arg` 0xFF |
| Conditions | `state NAME is V[,V...]`, `state NAME is not V[,V...]`, `holding KEY`, `not holding KEY` | A state-in-set condition with a mask (`is not` is the complement within the state's values), and the two key conditions |
| Delay | `after=TICKS` | The record's `delay` |
| Actions | `set NAME V`, `toggle NAME`, `next NAME`, `spend KEY`, `give KEY`, `score N`, `ammo N`, `word NAME-or-"text" [style=S] at=I,J`, `cue NAME at=I,J` | Ops 1 to 9 |
| Repeat | `repeat` | `REPEAT`; without it a rule fires once per life |

- `KEY` is the key object's word (`key-red`); `lock` takes the colour (`key=red`), because there it is a field of the door.
- `holding` is a condition, never a modifier of `touch`. A rule that spends a key it does not test for is legal: a `spend` of a key Ben does not hold is simply nothing, and the loader adds no condition of its own.
- **`touch` takes a rectangle.** Ben touches it when his front probe lands in it or his body overlaps it, which is exactly Fizz 1's lock test (the tile in front of him at `py + 0.2`) taken over the door's run. A cell is a rectangle with equal corners. The rectangle must be inside the map (E117).
- **`touch` is a level, not an edge**: it holds on every tick Ben is in contact. A `repeat` rule on a `touch` whose action is `toggle` or `next` would flip every tick, so it is W114.
- **`hit` is a switch struck** by a shot or a stomp, as its flags allow. Ben being hurt is `hurt`, reserved. `kind=KIND` on `stomp`, `stun` and `defeat` is reserved until custom enemies.
- A **group** name resolves to a number from 1 to 16 (E109 beyond), carried by the chicken (`group=NAME` on an object line is reserved with the other object fields, see [Extension points](#extension-points)); `cleared` is an input only for now.
- **Reserved and not lowered** (E102): `hurt`, `use at=I,J` (a cell that is used), and the conditions ammo at least, score at least and group cleared.
- A rule has at least one action (E003), and `toggle` needs a state with exactly two values (E109). A value word that is not one of the named state's values is E002.

## `lock`, the sugar

```text
lock red-door key=red at=15,21
```

expands to exactly these three lines, at the lock's place in file order:

```text
state red-door values=closed,open
door x=15 y=20..22 state=red-door open=open
when touch x=15 y=20..22 if state red-door is closed and holding key-red then set red-door open, spend key-red
```

- The door's run is the 4-connected set of cells holding the same switched tile id as the cell `at=` names. It must be a rectangle; a run that is not is E110, with the fix "write `door` lines and a `when` rule".
- The rule's touch rectangle is the run's rectangle, so any cell of the door accepts the key, as in Fizz 1.
- The `holding` condition is written by the expansion, not added by the loader. It is visible in the dump view and counts toward the rule's two conditions. The loader adds nothing hidden.
- The door's sound and word are not actions. Any door binding that becomes passable raises the game's built-in door event (`Event::DoorOpen`, effect 19) at the centre cell of its rectangle.
- The expansion has no `repeat`, so a lock opens its door once per life; a death re-arms it. A door that closes again and takes another key is written by hand with `when ... repeat`.
- Options are reserved with the object model: `keep` (do not spend the key), `id=SWITCH` in place of `at=` (a keyhole lock on a switch), and `key=red,blue` (two keys). Until they are defined they are E009.
- The printer keeps the `lock` line as written. A tool that edits the expansion by hand replaces the `lock` with its three lines as one explicit step.

## The dump view

The dump view is the resolved, expanded level: every name numbered, every sugar line shown as its expansion with the source line it came from, and the counts against the limits. The `expand` function of `fizz-script` produces the lines with their source spans, and the dump prints them. For the slice:

```text
// dump: slice.fizz — states 2/32, bindings 2/64 (2 door), rules 2/32, strings 0/32
state 0   red-door   values=closed:0,open:1 start=closed       // lock, line 14
state 1   blue-door  values=closed:0,open:1 start=closed       // lock, line 15
door  0   x=15 y=20..22 state=0 open={1}                       // lock, line 14
door  1   x=22 y=38..40 state=1 open={1}                       // lock, line 15
rule  0   touch x=15 y=20..22 if s0 in {0} and holding red then set s0 1, spend red   // lock, line 14
rule  1   touch x=22 y=38..40 if s1 in {0} and holding blue then set s1 1, spend blue // lock, line 15
```

A toggle shows each record's bytes in the layout of the worked example in [`FAP.md`](FAP.md).

## The legend

The legend maps a letter to a tile id. It is `fizz_ep2::tiles`'s, written here from `level.rs`; the baker and the printer use one table in the crate, built from those constants, and a test fails if it drifts. The content export (`pack_content()` as JSON) will generate it for the studio.

| Letter | Tile | Id | Meaning |
| --- | --- | --- | --- |
| `.` | `EMPTY` | 0 | Sky, or open air |
| `#` | `FILL` | 1 | Solid fill |
| `B` | `BLOCK` | 2 | A solid block |
| `=` | `PLAT` | 3 | A one-way platform |
| `^` | `SPIKE` | 4 | Spikes: lethal |
| `D` | `DOOR_R` | 12 | The red door: a switched tile, bound by a `door` line or a `lock` |
| `b` | `DOOR_B` | 13 | The blue door: a switched tile, bound by a `door` line or a `lock` |
| `E` | `EXIT` | 17 | The exit |
| `w` | `WALLBG` | 30 | Interior wall behind the rooms; not solid |

`CHOC` (5) and the six slopes (6 to 11) have tile ids and properties but no letter yet: using any other character is E104 until the Fizz 2 rules assign letters. Letters are case-sensitive and ASCII.

## Lowering to `LevelSource`

Lowering takes a tree with no errors and produces the content of a `LevelSource`, owned (a `Lowered` value whose `source()` borrows as a `LevelSource`):

1. `tiles`: the rows reversed into row-major, bottom-row-first order, each letter replaced by its tile id.
2. `states`: each `state` (and each lock's state) as a state record, in file order, its `start` the index of the named value.
3. `floor_solid`, `spawn`, `name`, `w` and `h`: from `floor`, `start` and `level`.
4. `objects`: the object lines as `level::Object` values, positions as `f64` (a cell form adds 0.5), stably sorted by kind.
5. `bindings`: each `door` (and each lock's door) as a binding of kind 1, with the rectangle and the mask of the listed values, in file order.
6. `rules`: each `when` (and each lock's rule) as a rule record in file order, names resolved to indices, `is not` to the complement mask, a cell to a rectangle with equal corners. This crate builds records through the types `fizz-pack` publishes and never lays out a rule record itself.
7. `strings`: each `string` as `StringRec::new(style, text)` in file order.

The result is then passed to `level::validate`, which must agree with the tree's own checks; a disagreement is E199. A lowered level is the same level as the `const` one when `level::digest` agrees, and the slice's script is the proof (see below).

## Errors and lints

Every diagnostic has a stable code, a line and a column (both from 1, the column in characters), a message and, where one exists, a fix. They print as `path:line:col: error[E003]: message (fix: ...)` and `warning[W101]`. A bad line stays in the tree, so one error does not hide the others; at most one syntax error is reported for a line. Codes are never reused (E119, an earlier error for two locks on one door, was withdrawn when the simulation was found to allow it, and the finding is W123). The numbering follows the Level Studio's plan, where the meaning is the same; where `fizz-script` numbers a code differently, these numbers win.

| Code | Meaning | Example |
| --- | --- | --- |
| E001 | Unknown statement word | `plot 1,2` |
| E002 | Unknown name: a floor word other than `open` or `solid`, a key colour other than `red` or `blue`, a state, value, object or key that is not declared | `lock a key=green at=2,1`; `set lift up` with no `lift` |
| E003 | Missing field or word | an object with no `id=`; `level` with no size |
| E004 | Malformed value: a number, list, size, name, string, map row without its closing bar, or a control character | `start 2` |
| E006 | A statement that may appear once, appears again | a second `start` |
| E007 | The file does not start with `script fizz2 0`, or a statement comes before `level`, or there is no `level` (a Fizz 1 script gives this) | no `script` line |
| E008 | A statement in the wrong place | a map row before `map`; `cheezie` outside `objects` |
| E009 | Unknown field, stray word, a field given twice, or a cell with a fractional `at=` | `start 1,1 extra`; `lock a key=red at=15.5,21` |
| E010 | No `start` statement | |
| E101 | The `script` line names a dialect or version this parser does not read | `script fizz2 7` |
| E102 | A word reserved for a later stage, or a Fizz 1 word the dialect does not have (`channel`, `channel=N`); the fix for a channel is "declare a `state` and bind it with `door`" | `light`; `channel 1 on` |
| E103 | The map's rows or widths differ from `h` and `w`, or a row label is not the row's `y` | row 3 has 31 letters |
| E104 | A map letter the legend does not hold | `Q` |
| E105 | A name used twice | two objects with `id=a` |
| E107 | A second key of a colour | two `key-red` lines |
| E108 | The start or an object's centre is outside the map | `at=32.5,4` in a 32-wide level |
| E109 | A value the record cannot hold | a size of 0; a name over 48 bytes; a name or slug over 32 bytes; a state with fewer than 2 or more than 8 values or a `start` that is not one of them; a `toggle` of a state without two values; a group over 16; `dir=2` |
| E110 | A door meaning that does not hold: a lock whose cell is not a switched door tile, a lock whose run is not a rectangle, a `door` rectangle with a tile that is not switched, a switched tile that no `door` binding covers, or one that two cover | `lock a key=red at=3,4` on a block |
| E111 | A table over its limit, counting what `lock` expands to: more than 32 states, 64 bindings, 32 door bindings or 32 rules | the 33rd state |
| E112 | A string problem: more than 32 strings, a length outside 1 to 30 bytes, a character the game cannot draw, an unknown style | a 31-byte string |
| E117 | A cell or rectangle outside the map: a lock's cell, a `door`, `enter`, `shoot` or `touch` rectangle, or the cell of a `word` or `cue` | `at=40,1` in a 32-wide level |
| E199 | Internal: `level::validate` refused a level this validator accepted. Report it as a bug | |

| Code | Lint | Why |
| --- | --- | --- |
| W101 | A door's bound state starts at one of the door's open values, so the door starts open | Usually a mistake; sometimes a puzzle |
| W108 | A state that nothing binds and no rule reads or sets | Dead state |
| W109 | A reserved word that is accepted but not baked | `theme` |
| W112 | A Fizz 1 field ignored in a Fizz 2 file | `ground=` |
| W114 | A `repeat` rule on a `touch` whose action is `toggle` or `next` | A touch holds on every tick Ben is in contact, so the state flips every tick |
| W120 | A lock for a key colour that no object in the level provides | The door can never open |
| W121 | Retired: a switched tile that no `door` binding covers is E110 | |
| W122 | Retired with channels, and never reused | |
| W123 | Two rules that open one door: two locks on one run, or a lock and a rule that sets its state to an open value. The simulation allows it, so it is a warning that flags a likely duplicate | two `lock` lines at `D` cells |

The fix, where present, is a short instruction. Codes E005 and E011 to E016 of the Fizz 1 language are not used here: ranges, the ground height and enemy fields do not exist in this dialect. The studio's E105 (an object without an id) is E003 here, because a missing field is a missing field. Every code, except E199, has a corpus file that produces it, and a test fails if a code is added without one.

## The lossless tree and the printer

**The tree is lossless.** `parse(text)` never fails and returns a tree and its syntax diagnostics. The tree holds every line: its text as read, its line ending, and what it parsed to (a blank, a comment, a statement or a bad line). Printing the tree in lossless mode writes back exactly the bytes read, for every input, including bad lines, mixed line endings and a missing final newline. Statements hold typed values (numbers as `f64`, positions as a point or a cell) so the validator and lowering never re-read text.

**The canonical printer** rewrites a tree in the one form `fizz-bake fmt` writes and `fmt --check` demands. The form is:

- Line endings `\n`, one final newline, no trailing white space, no tabs; no blank line at the start or end of the file and no run of two blank lines.
- Tokens separated by single spaces; two spaces before a trailing comment, written `// text` with the text trimmed; a comment line takes the indent of the statement after it.
- Statement fields in the order of the grammar above (`id=` then the position then `dir=`); a `state` omits `values=off,on` and a `start=` that names its first value, so the common state is one short line; a value list keeps its order (the order is the numbering) and a `door`'s `open=` list is written in value order without repeats; a `lock` stays one line and is never printed as its expansion; numbers in the shortest form that reads back to the same `f64`, with no `+` and no trailing `.0`.
- **The position rule.** A position is written `x=I y=J` exactly when both coordinates are cell centres (`I + 0.5` and `J + 0.5` with `I` and `J` non-negative integers), and `at=X,Y` for everything else, so one position has one spelling.
- Map rows and objects indented two spaces; every map row inside the level's height carries its label, counted down from `h - 1`, right-aligned to the width of the top row's label and separated from the closing bar by two spaces. A row below the bottom of the level has no `y` and no label.
- Objects grouped by kind in lowering order, each with the comment lines directly above it; blank lines between objects are dropped.
- A bad line is written as its text with the outer white space trimmed and is otherwise not changed. A file with syntax errors is not formatted: `fmt` refuses it, and also a file with E110, because dropping a `channel=` alias that disagrees with its door would hide the error. Any other file that parses is formatted, whatever the validator says.

Two properties are required and tested: **stable** (printing the canonical text again gives the same text, and parsing canonical text and printing it canonically gives it back) and **meaning-preserving** (the canonical text lowers to the same digest as the input, and gives the same set of diagnostic codes, apart from positions, W122 (the alias is dropped) and a wrong row label, which formatting corrects).

## The conformance corpus

The corpus is a directory of plain files, `crates/fizz-script/corpus/`, that any consumer of the dialect (the baker, and the studio through `fizz-script`) can run the same way:

| Directory | Holds | A consumer checks |
| --- | --- | --- |
| `levels/` | Valid levels, in canonical form. `slice.fizz` is the slice level | They parse with no diagnostics, print canonically to themselves and lower; `slice.fizz` lowers to a `LevelSource` with the same `level::digest` as `SLICE_SOURCE` in its `LEVL` 2 form, equal field for field, and `world_from` builds a world with the same hash |
| `errors/` | One file per error code, named `E###-description.fizz` | The diagnostics have exactly that error code and no other |
| `lints/` | One file per lint, named `W###-description.fizz` | The diagnostics are exactly that warning and no error |
| `fmt/` | Pairs `name.in.fizz` and `name.out.fizz` | Printing the first canonically gives the second; the pair lowers to one digest |

The slice file is written by hand to the rules above (rows from `ROWS`, the keys, snacks and chicken at the positions of `SLICE_OBJECTS`, the two `lock` lines of the slice's red and blue doors), not generated, so that a change to either side is a reviewed change. The baker's later `check` also bakes it and compares the manifest.

## Extension points

These are reserved and are not part of version 0. Each gives E102 today. A later stage defines them, with `FAP.md` and `RULES.md` as the owners of bytes and meaning.

| Word | Reserved for |
| --- | --- |
| `light`, `switch`, `platform`, `key-green`, `key-gold`, `usb`, custom enemies and items in `objects` | Objects the pack reserves (kinds 5, 7, 9 and 10, and the generic key kind) and the world's definitions. A light, panel or platform binds to a state with `state=NAME on=V[,V...]` on the object, lowering to a binding with the same mask meaning as `door`; a switch takes `by=use,shot,stomp` |
| `group=NAME` on an object | The group number a `cleared` input counts |
| `keep`, `id=` and `key=A,B` on a `lock` | A kept key, a keyhole lock on a switch and a two-key lock |
| `hurt`, `kind=KIND`, `use at=I,J` and the conditions ammo at least, score at least and group cleared | Ben hurt, custom enemy kinds, a used cell and the reserved conditions |
| `music NAME`, `allow CODE`, `world` | Per-level music, a lint allowance, the world project file. A state of the world's `defs/states.fizz` is named and not redeclared, and is copied into the level's table in the order the level first names it (E130 when a level redeclares it) |

A Fizz 1 conversion writes a new level and a report and does not change the Fizz 1 file. Each Fizz 1 channel that a tile, prop or `channel` line uses becomes `state channel-N` (with `start=on` when the Fizz 1 level turns it on), red and blue doors become `lock` lines named `red-door` and `blue-door` (with `-2` and `-3` for more of a colour), and props that drive channels become rules where Fizz 2 has the input.

A version bump (`script fizz2 1`) is for a change that makes a version 0 file mean something else. Adding statements that version 0 reserves does not bump the version, because those files are errors today.
