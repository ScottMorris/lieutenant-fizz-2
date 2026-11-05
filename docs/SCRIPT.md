# The Fizz 2 script dialect

This document specifies `script fizz2 0`, the text format in which Fizz 2 levels are authored, and the tool behaviour around it: the lossless syntax tree, the canonical printer, the validator and the lowering to `fizz_ep2::level::LevelSource`, which the baker (`fizz-bake`) writes as a `LEVL` section. It is the source of truth for the grammar: the crate `fizz-script` follows it, and where the two differ the crate is wrong. The byte layout of `LEVL` belongs to [`FAP.md`](FAP.md) and the meaning of rules and states to `RULES.md`; this document does not restate either. [`ARCHITECTURE.md`](ARCHITECTURE.md#assets-and-packs) says why levels are baked and not parsed at run time.

## Contents

- [Scope and stages](#scope-and-stages)
- [Lexical rules](#lexical-rules)
- [Statements](#statements)
- [The legend](#the-legend)
- [Lowering to `LevelSource`](#lowering-to-levelsource)
- [Errors and lints](#errors-and-lints)
- [The lossless tree and the printer](#the-lossless-tree-and-the-printer)
- [The conformance corpus](#the-conformance-corpus)
- [Extension points](#extension-points)

## Scope and stages

The dialect descends from Fizz 1's level script language, which rebuilt all sixteen Fizz 1 levels exactly. It keeps that language's lexical habits (one statement to a line, `key=value` fields, `//` comments, stable error codes, a lossless tree) and replaces its Fizz 1 builder vocabulary with what a Fizz 2 level holds. A Fizz 2 file starts with `script fizz2 0` and the version never changes under a file; a Fizz 1 script has no `script` line and is not read by this parser (it gets E007, which says so).

Version 0 covers what `LEVL` version 1 carries today: the level's size and name, the start, the floor, the switch channels that start on, the tile map, the objects (keys, snacks, soda cans and the chicken, each with a name), the lock rules and the level's strings. It lowers to a `LevelSource` and so to the `LEVL` bytes, and the slice level written in it has the same `level::digest` as `level::SLICE_SOURCE`.

Version 0 does not write states, general rules, lights, switches or custom objects. Their words are reserved (see [Extension points](#extension-points)) and give E102 until a later stage defines them. The limits proposed for that stage, up to 32 states with up to 8 values and rules of 2 conditions and 2 actions, are recorded there and are not enforced here.

Stage owners: the byte layout of `LEVL` (including any version 2) is `FAP.md`'s, and the semantics of rules and states are `RULES.md`'s. A lowering rule in this document that restates one of them is a mistake in this document.

## Lexical rules

- A file is UTF-8 text. A line ends with `\n` or `\r\n`; the lossless tree keeps whichever each line used, and the canonical printer writes `\n`. A lone `\r`, or any other control character except a tab, is E004.
- One line holds at most one statement. Blank lines and comment lines are lines.
- `//` starts a comment that runs to the end of the line. It is not a comment inside a quoted string or between the bars of a map row.
- Indentation is ignored. The canonical form indents map rows and objects by two spaces and everything else by none.
- Tokens are separated by spaces or tabs. A token that ends in a comma joins the next token (`channel 1, 2 on` reads as `channel 1,2 on`).
- A **word** is lower-case: a letter, then letters, digits and hyphens (`key-red`, `floor`). A **name** (an `id`, a lock name, a string name) is a word-like token that may start with a digit: `[a-z0-9][a-z0-9-]*`, at most 32 bytes (E109 beyond that; E004 if it has other characters).
- A **number** is a plain decimal: an optional `-`, digits, and an optional `.` with digits. No exponent, no leading `+`, no bare `.5`. It is read with Rust's correctly rounded `str::parse::<f64>`, so `11.7` gives the same bits as the `11.7` literal in `level.rs`. An **integer** is a number with no fraction.
- A **field** is `key=value` with no spaces around `=`. After a statement's leading words, fields may come in any order. A field given twice, or a field the statement does not take, is E009.
- A **string** is text between double quotes. There is no escape: a string cannot hold `"`, and one that is not closed on its line is E004.
- A **pair** is two numbers joined by a comma (`2.5,11.7`); a **size** is two integers joined by `x` (`32x48`).

## Statements

```text
file      = script-line { line } ;
script    = "script" "fizz2" "0" ;
level     = "level" STRING SIZE "id=" NAME [ "ground=" INT ] ;
start     = "start" PAIR ;
floor     = "floor" ( "open" | "solid" ) ;
channel   = "channel" INTLIST ( "on" | "off" ) ;
lock      = "lock" NAME "key=" ( "red" | "blue" ) "at=" INT "," INT [ "channel=" INT ] ;
string    = "string" NAME STRING [ "style=" STYLE ] ;
theme     = "theme" WORD ;
map       = "map" ;                    (* then rows *)
row       = "|" TILES "|" [ INT ] ;
objects   = "objects" ;                (* then object lines *)
object    = OBJWORD "id=" NAME ( "at=" PAIR | "x=" INT "y=" INT ) [ "dir=" ( "1" | "-1" ) ] ;
OBJWORD   = "key-red" | "key-blue" | "cheezie" | "choc" | "cookie" | "soda" | "chicken" ;
STYLE     = "white" | "cyan" | "red" | "green" | "blue" | "yellow" | "grey" ;
```

| Statement | Meaning | Lowers to |
| --- | --- | --- |
| `script fizz2 0` | The dialect and its version. It must be the first statement | Nothing |
| `level "Name" WxH id=NAME` | The level's name (up to 48 printable ASCII bytes, E109 otherwise), its size in tiles (each 1 to 65535) and its file-name slug. `ground=` is a Fizz 1 field, accepted and ignored with W112 | `name`, `w`, `h`. The slug is for file names and the manifest; the `LEVL` section stores none |
| `start X,Y` | Ben's start: the lower-left corner of his body, in tiles. It must lie in the map (E108). Required (E010) | `spawn` |
| `floor open` or `floor solid` | Whether the bottom of the map is solid. Default `open`, as in the slice | `floor_solid` |
| `channel LIST on` or `off` | Sets switch channels (0 to 31, E109 beyond) at the start of the level, in file order, a later line winning. Channels are a Fizz 1 idea; this statement is sugar for the two-valued states `channel-N` that a later stage defines | `switches` |
| `lock NAME key=K at=X,Y` | A key of colour `K` spent on a closed door. The cell `X,Y` is any cell of the door's run of tiles and must hold a switched door tile (E110) inside the map (E117); the door's switch channel is the tile's (`fizz_ep2::tiles::props`), so no channel is written. `NAME` is the name the door's state will take when states arrive. `channel=N` is a **provisional alias** the parser reads and the printer never writes: it must equal the tile's channel (E110 otherwise) and is W122 | One `RuleRec` from `fizz_ep2::rules::Rule::lock(colour, channel, (X, Y))`, the channel taken from the tile and the event cell the one written, in file order |
| `string NAME "text" [style=S]` | A caption the level carries. The text is 1 to 30 bytes of printable ASCII plus the middle dot, en and em dashes, curly quotes and the ellipsis (E112 otherwise); the style defaults to `white` | One `StringRec`, in file order |
| `theme NAME` | Reserved: accepted and printed, not baked (W109) | Nothing |
| `map` | Starts the tile grid: the rows that follow it, top row first | `tiles` |
| `objects` | Starts the object list: the object lines that follow it | `objects` |

A **map row** sits between `|` bars, holds exactly `w` letters of the [legend](#the-legend) (E103 on a wrong count, E104 on an unknown letter), and may end with a row label that gives its `y`. `y` counts up from the bottom: the first row is `h - 1` and the last is 0. A label that is not the row's `y` is E103; the canonical printer always writes the label. There must be exactly `h` rows (E103).

An **object line** names a kind, a required `id=`, a position and, for a chicken, a required `dir=` of `1` or `-1` (E109 for another value, E003 when missing, E009 on any other kind). Positions are in tile units (cells), never pixels, fractions allowed, and `y` counts up from the bottom row (only the `map` rows are written top first). The position is `at=X,Y`, an exact point in tiles for anything off a cell centre, or `x=I y=J`, which names a cell and is the centre of cell `I,J` and means `at=I+0.5,J+0.5`. The centre must lie in the map (`0 <= x < w`, `0 <= y < h`, E108). What the point means depends on the kind exactly as in `level.rs`: the centre of a key or snack, the spawn of a chicken (its feet and centre), while Ben's `start` is the lower-left corner of his body.

- **Order.** Objects are lowered stably sorted by kind (red key, blue key, snacks, chickens), same-kind objects keeping file order, because the state hash indexes snacks and chickens by position. The file may list them in any order; the canonical printer groups them the same way. A second key of one colour is E107.
- **Names.** Every object, lock and string has a name, and one name may be used once in a file (E105). The `LEVL` section holds no name; the baker resolves names to indices where a later stage needs them.
- **Rule order.** Locks lower in file order, which is the order the sim evaluates them; the order of strings is the order of `string` statements.

Statements may appear in any order after `level`, with these constraints. `script` is first, then `level` (E007 for a statement before either). `script`, `level`, `start`, `floor`, `theme`, `map` and `objects` appear at most once (E006). Rows belong to the `map` block and object lines to the `objects` block, which run until the next statement that is not a row or an object line; a row or object line elsewhere is E008. `channel`, `lock` and `string` may repeat.

## The legend

The legend maps a letter to a tile id. It is `fizz_ep2::tiles`'s, written here from `level.rs`; the baker and the printer use one table in the crate, built from those constants, and a test fails if it drifts. The content export (`pack_content()` as JSON) will generate it for the studio.

| Letter | Tile | Id | Meaning |
| --- | --- | --- | --- |
| `.` | `EMPTY` | 0 | Sky, or open air |
| `#` | `FILL` | 1 | Solid fill |
| `B` | `BLOCK` | 2 | A solid block |
| `=` | `PLAT` | 3 | A one-way platform |
| `^` | `SPIKE` | 4 | Spikes: lethal |
| `D` | `DOOR_R` | 12 | The red door: switched on channel 1 |
| `b` | `DOOR_B` | 13 | The blue door: switched on channel 2 |
| `E` | `EXIT` | 17 | The exit |
| `w` | `WALLBG` | 30 | Interior wall behind the rooms; not solid |

`CHOC` (5) and the six slopes (6 to 11) have tile ids and properties but no letter yet: using any other character is E104 until the Fizz 2 rules assign letters. Letters are case-sensitive and ASCII.

## Lowering to `LevelSource`

Lowering takes a tree with no errors and produces the content of a `LevelSource`, owned (a `Lowered` value whose `source()` borrows as a `LevelSource`):

1. `tiles`: the rows reversed into row-major, bottom-row-first order, each letter replaced by its tile id.
2. `switches`: a `u32` with a bit for each channel the `channel` statements leave on, applied in file order from all off.
3. `floor_solid`, `spawn`, `name`, `w` and `h`: from `floor`, `start` and `level`.
4. `objects`: the object lines as `level::Object` values, positions as `f64` (a cell form adds 0.5), stably sorted by kind.
5. `rules`: each `lock` as `Rule::lock(colour, channel, cell).to_rec()` in file order, with `channel` read from the door tile at `cell`, so this crate never lays out a rule record.
6. `strings`: each `string` as `StringRec::new(style, text)` in file order.

The result is then passed to `level::validate`, which must agree with the tree's own checks; a disagreement is E199. A lowered level is the same level as the `const` one when `level::digest` agrees, and the slice's script is the proof (see below).

## Errors and lints

Every diagnostic has a stable code, a line and a column (both from 1, the column in characters), a message and, where one exists, a fix. They print as `path:line:col: error[E003]: message (fix: ...)` and `warning[W101]`. A bad line stays in the tree, so one error does not hide the others; at most one syntax error is reported for a line. Codes are never reused (E119, an earlier error for two locks on one channel, was withdrawn when the simulation was found to allow it, and the finding is W123). The numbering follows the Level Studio's plan, where the meaning is the same; where `fizz-script` numbers a code differently, these numbers win.

| Code | Meaning | Example |
| --- | --- | --- |
| E001 | Unknown statement word | `plot 1,2` |
| E002 | Unknown name: a floor word other than `open` or `solid`, a key colour other than `red` or `blue`, a word other than `on` or `off` | `lock a key=green at=2,1` |
| E003 | Missing field or word | an object with no `id=`; `level` with no size |
| E004 | Malformed value: a number, list, size, name, string, map row without its closing bar, or a control character | `start 2` |
| E006 | A statement that may appear once, appears again | a second `start` |
| E007 | The file does not start with `script fizz2 0`, or a statement comes before `level`, or there is no `level` (a Fizz 1 script gives this) | no `script` line |
| E008 | A statement in the wrong place | a map row before `map`; `cheezie` outside `objects` |
| E009 | Unknown field, stray word, or a field given twice | `start 1,1 extra` |
| E010 | No `start` statement | |
| E101 | The `script` line names a dialect or version this parser does not read | `script fizz2 7` |
| E102 | A word reserved for a later stage | `state alarm`; `light` |
| E103 | The map's rows or widths differ from `h` and `w`, or a row label is not the row's `y` | row 3 has 31 letters |
| E104 | A map letter the legend does not hold | `Q` |
| E105 | A name used twice | two objects with `id=a` |
| E107 | A second key of a colour | two `key-red` lines |
| E108 | The start or an object's centre is outside the map | `at=32.5,4` in a 32-wide level |
| E109 | A value the record cannot hold | a size of 0; a name over 48 bytes; a name or slug over 32 bytes; a channel of 32 or more in `channel`; `dir=2` |
| E110 | A lock whose cell is not a switched door tile, or whose `channel=` alias differs from the tile's | `lock a key=red at=3,4` on a block |
| E111 | More than 32 locks | the 33rd |
| E112 | A string problem: more than 32 strings, a length outside 1 to 30 bytes, a character the game cannot draw, an unknown style | a 31-byte string |
| E117 | A lock's cell is outside the map | `at=40,1` in a 32-wide level |
| E199 | Internal: `level::validate` refused a level this validator accepted. Report it as a bug | |

| Code | Lint | Why |
| --- | --- | --- |
| W101 | A lock's channel starts off, so its door starts open | Usually a mistake; sometimes a puzzle |
| W108 | A `channel` statement names a channel that no lock and no door tile in the map uses | Dead state |
| W109 | A reserved word that is accepted but not baked | `theme` |
| W112 | A Fizz 1 field ignored in a Fizz 2 file | `ground=` |
| W122 | A `channel=` alias on a `lock`, which the printer drops | Channels are a Fizz 1 idea; name the door's cell and let the tile say |
| W123 | Two locks on one door channel (two cells of one door, or two doors of one colour). The simulation allows it, so it is a warning that flags a likely duplicate | two `lock` lines at `D` cells |
| W120 | A lock for a key colour that no object in the level provides | The door can never open |
| W121 | A door tile whose channel no lock opens | The door is a wall |

The fix, where present, is a short instruction. Codes E005 and E011 to E016 of the Fizz 1 language are not used here: ranges, the ground height and enemy fields do not exist in this dialect. The studio's E105 (an object without an id) is E003 here, because a missing field is a missing field. Every code, except E199, has a corpus file that produces it, and a test fails if a code is added without one.

## The lossless tree and the printer

**The tree is lossless.** `parse(text)` never fails and returns a tree and its syntax diagnostics. The tree holds every line: its text as read, its line ending, and what it parsed to (a blank, a comment, a statement or a bad line). Printing the tree in lossless mode writes back exactly the bytes read, for every input, including bad lines, mixed line endings and a missing final newline. Statements hold typed values (numbers as `f64`, positions as a point or a cell) so the validator and lowering never re-read text.

**The canonical printer** rewrites a tree in the one form `fizz-bake fmt` writes and `fmt --check` demands. The form is:

- Line endings `\n`, one final newline, no trailing white space, no tabs; no blank line at the start or end of the file and no run of two blank lines.
- Tokens separated by single spaces; two spaces before a trailing comment, written `// text` with the text trimmed; a comment line takes the indent of the statement after it.
- Statement fields in the order of the grammar above (`id=` then the position then `dir=`); the `channel=` alias of a `lock` is dropped; `channel` lists sorted and without repeats; numbers in the shortest form that reads back to the same `f64`, with no `+` and no trailing `.0`.
- **The position rule.** A position is written `x=I y=J` exactly when both coordinates are cell centres (`I + 0.5` and `J + 0.5` with `I` and `J` non-negative integers), and `at=X,Y` for everything else, so one position has one spelling.
- Map rows and objects indented two spaces; every map row carries its label, right-aligned to the width of the top row's label and separated from the closing bar by two spaces.
- Objects grouped by kind in lowering order, each with the comment lines directly above it; blank lines between objects are dropped.
- A bad line is written as its text with the outer white space trimmed and is otherwise not changed. A file with errors is not formatted by `fmt`, which refuses it.

Two properties are required and tested: **stable** (printing the canonical text again gives the same text, and parsing canonical text and printing it canonically gives it back) and **meaning-preserving** (the canonical text lowers to the same digest as the input, and gives the same diagnostics apart from their positions).

## The conformance corpus

The corpus is a directory of plain files, `crates/fizz-script/corpus/`, that any consumer of the dialect (the baker, and the studio through `fizz-script`) can run the same way:

| Directory | Holds | A consumer checks |
| --- | --- | --- |
| `levels/` | Valid levels, in canonical form. `slice.fizz` is the slice level | They parse with no diagnostics, print canonically to themselves and lower; `slice.fizz` lowers to a `LevelSource` with the same `level::digest` as `SLICE_SOURCE`, equal field for field, and `world_from` builds a world with the same hash |
| `errors/` | One file per error code, named `E###-description.fizz` | The diagnostics have exactly that error code and no other |
| `lints/` | One file per lint, named `W###-description.fizz` | The diagnostics are exactly that warning and no error |
| `fmt/` | Pairs `name.in.fizz` and `name.out.fizz` | Printing the first canonically gives the second; the pair lowers to one digest |

The slice file is written by hand to the rules above (rows from `ROWS`, the keys, snacks and chicken at the positions of `SLICE_OBJECTS`, the locks of `SLICE_RULES`, channels 1 and 2 on), not generated, so that a change to either side is a reviewed change. The baker's later `check` also bakes it and compares the manifest.

## Extension points

These are reserved and are not part of version 0. Each gives E102 today. A later stage defines them, with `FAP.md` and `RULES.md` as the owners of bytes and meaning.

| Word | Reserved for |
| --- | --- |
| `state NAME START [values=...]` | Named states. Proposed limits: up to 32 states with up to 8 values. A `lock`'s `NAME` is the name its state will take, and the `channel=N` alias and the `channel` statement become states |
| `door x=R y=R state=NAME open=VALUE` | Binding door tiles to a state |
| `when INPUT [if COND [and COND]] then ACTION[, ACTION] [repeat]` | General rules. Proposed limits: 2 conditions and 2 actions per rule, 32 rules a level |
| `light`, `switch`, `key-green`, `usb`, custom enemies and items in `objects` | Objects the pack reserves (kinds 5 to 7) and the world's definitions |
| `music NAME`, `allow CODE`, `world` | Per-level music, a lint allowance, the world project file |

A version bump (`script fizz2 1`) is for a change that makes a version 0 file mean something else. Adding statements that version 0 reserves does not bump the version, because those files are errors today.
