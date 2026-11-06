# The Fizz Asset Pack (`.FAP`)

This document specifies the Fizz Asset Pack, the file format that `fizz-bake` writes and the Web and DOS builds read. It is the design for Milestone 2 and the source of truth for the format: the reader crate `fizz-pack` follows it, and where the two differ the crate is wrong. The baker and the loaders do not exist yet, so the parts about them are a plan with measured inputs, not a description of code. [`ARCHITECTURE.md`](ARCHITECTURE.md#assets-and-packs) says why packs exist and what stays in the executable; [`PLATFORM.md`](PLATFORM.md#size-budget) holds the size budget.

Sentences marked **Not yet specified** name a gap on purpose. Sentences marked **Open** are choices still to be made.

## Contents

- [Goals and non-goals](#goals-and-non-goals)
- [Files and names](#files-and-names)
- [Container layout](#container-layout)
- [Checksums](#checksums)
- [The compatibility number](#the-compatibility-number)
- [Codec](#codec)
- [Sections](#sections)
- [Target profiles](#target-profiles)
- [Loading on DOS](#loading-on-dos)
- [Memory limits](#memory-limits)
- [The baker](#the-baker)
- [Proof that a baked level equals the `const` level](#proof-that-a-baked-level-equals-the-const-level)
- [Packaging on a Fizz 1 floppy](#packaging-on-a-fizz-1-floppy)
- [Open points](#open-points)

## Goals and non-goals

- **DOS parses almost nothing.** There is no text and no variable-length decoding of level data. A level section is one read, one decode (or none, when stored), bounds checks against the directory, and fixed-offset `from_le_bytes`. The one variable-length form is the audio record stream, which is decoded by a stepper as it plays.
- **One container, many files.** A pack is a set of sections. Which sections share a file is a baking decision, so the split between files can change without a new reader.
- **Identical level bytes on both targets.** The `LEVL` bytes are the same in the Web and DOS profiles. Parity depends on it.
- **Cheap to check, hard to confuse.** Every file carries a magic, a format version, a compatibility number and a checksum, and every section a stored length, a decoded length and a checksum of its own.
- **Compression from day one.** The codec byte is live in version 1. A section may be stored when compression does not help.
- **Not a package manager.** No patching, no streaming, no encryption and no signing. A pack is read whole (or by section), checked and used.

Palette, tile properties (the rules' `TileProps`), fonts, trigonometry tables and dither patterns stay in the executable for good. They are small, they are read by the rules or by every frame, and moving them out would add a failure mode for no gain.

## Files and names

Names follow the DOS 8.3 rule, upper case on disk.

| File | Contents | Loaded |
| --- | --- | --- |
| `SHARED.FAP` | Ben, the HUD, the font extras, the sound effects (one `FXPK` section per device) and the song table (`MUSC`) | At start-up, after the executable |
| `<WORLD>.FAP`, for example `LIGHT.FAP` | The world's levels (`LEVL`), backdrops (`BDRP`) and world sprites (`SPRT`) | When a level of that world starts |
| `MUSIC.OP3` | One `OPL3` section per song | The one section for the song in play, when the song changes |
| `MUSIC.OP2` | One `OPL2` section per song | As above, on an OPL2 machine |

All files use the container below (the same magic, header and directory), whatever their extension. The extension is a hint for people and for the build scripts; the reader trusts the header.

- A **pack** is any such file. The reader asks for a section by tag and index and does not care which file holds it. The hybrid split above is the current plan (owner decision, 2026-10-29); moving a section to another file is a change to `fizz-bake`'s manifest, not to the format.
- Song `n` is the `n`th song of its tag in each of the two music files, and the `n`th record of `MUSC` in `SHARED.FAP`. A song held in one section is the `n`th section; a song held as segments (see [Audio sections](#audio-sections)) is a run of consecutive sections counted once. The baker refuses a music set where the two device files hold different song lists (a missing form is a placeholder, see [Target profiles](#target-profiles)). **There is no music on the PC speaker** (owner decision, 2026-10-29): the speaker plays sound effects only, so there is no `MUSIC.SPK`, and a machine with neither OPL chip plays no music.
- The Web build fetches the same names from its own `dist/web` directory, but has no `MUSIC.*` files.
- The file names above are decided (owner, 2026-10-29). The stem of every file is stored in its header (see [Header](#header)), so a renamed or misplaced file is caught at load.

## Container layout

Everything is little-endian. A pack starts with a 32-byte header, followed by the directory, followed by the section data. Section data starts on a 16-byte boundary, with zero padding between sections and after the last one, so the file length is a multiple of 16.

### Header

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Magic: bytes `46 41 50 1A` (`FAP` then 0x1A, which stops `TYPE` from printing the binary) |
| 4 | 2 | Format version, `u16`. This document is version 1 |
| 6 | 2 | Header size, `u16`: 32 in version 1. A reader that sees a larger value skips to that offset for the directory |
| 8 | 1 | Target, `u8`: 1 Web, 2 DOS |
| 9 | 1 | Flags, `u8`: 0 in version 1; a reader refuses a pack with any bit set |
| 10 | 2 | Section count, `u16` |
| 12 | 4 | Compatibility number, `u32` (see [The compatibility number](#the-compatibility-number)) |
| 16 | 4 | Total length of the file in bytes, `u32` |
| 20 | 4 | Checksum, `u32` (see [Checksums](#checksums)) |
| 24 | 8 | Pack stem: the file name without its extension, ASCII, padded to 8 bytes with spaces or zero bytes (for example `SHARED  `). The baker writes spaces |

The stem lets a reader tell that `LIGHT.FAP` was not renamed over `SHARED.FAP`, or that a file was copied from another place. A reader compares it with the stem it asked for, and a difference (including characters after the padding, or a space inside the stem) is refused with the stem message below, naming the file. The stem is upper case, 1 to 8 printable ASCII characters without spaces. For `MUSIC.OP3` and `MUSIC.OP2` the stem is `MUSIC`; the device is told apart by the compatibility number, which differs per device.

### Directory

The directory follows the header at the header size. Each entry is 24 bytes.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Tag: four ASCII bytes in file order (`L E V L`), not a number |
| 4 | 4 | Offset of the section's stored bytes from the start of the file, `u32`; a multiple of 16 |
| 8 | 4 | Stored length in bytes, `u32` |
| 12 | 4 | Decoded length in bytes, `u32` |
| 16 | 1 | Codec, `u8` |
| 17 | 1 | Section version, `u8`; each tag starts at 1 |
| 18 | 2 | Count, `u16`: the number of records the section holds (objects in a level, steps in an audio stream, sprites in a sprite section); 0 for a section without records |
| 20 | 4 | FNV-1a 32 of the section's stored bytes |

Rules the reader enforces, each with its own error:

- Entries are sorted by tag, comparing the four bytes in file order (so `BDRP` < `FXPK` < `LEVL` < `MUSC` < `OPL2` < `OPL3` < `SPRT`), and entries with the same tag are in the order the baker emitted them. The index of a section is its position among the entries with the same tag, counted from 0.
- Offsets are multiples of 16 and never fall in directory order: each section starts at or after the end of the one before it (so overlap is checked in one pass with no allocation) and after the end of the directory, and every `offset + stored length` is at most the total length. An empty section (a placeholder) may share its offset with the next one; no other two sections do.
- The total length in the header equals the file's real length, and is a multiple of 16 (the last section is followed by zero padding).
- Codec 0 (stored) needs stored length equal to decoded length. Any other codec needs the codec to be one the reader knows.
- A section whose tag the reader does not know is skipped, not an error, so an older reader can read a newer pack's known sections. The compatibility number still has to match, which in practice makes new tags arrive together with a new number.
- A section with a decoded length of 0 and a count of 0 is a **placeholder**: present in the directory and empty. The reader reports it as "absent" to its caller, which decides whether that is an error.

## Checksums

- **File checksum (header):** FNV-1a 32 over the whole file, with the four checksum bytes read as zero. It covers stored (compressed) bytes, the header, the directory and the padding. It catches a truncated or damaged file as a whole.
- **Section checksum (directory):** FNV-1a 32 over the stored bytes of one section. A reader that loads one section from a large file (a song from `MUSIC.OP3`) checks this one and does not read the whole file.
- **Decoded checksum (manifest only):** FNV-1a 32 over the decoded bytes of a section. It is not in the file, because the decoder is bounded by the decoded length and a bad stream is rejected on its own terms (see [Codec](#codec)). It is in the text manifest, so `fizz-bake check` proves that decoding gives back what the baker meant and a reviewer sees any change in decoded content.
- FNV-1a 32 uses the offset basis 0x811c9dc5 and the prime 0x01000193, as Fizz 1's level fingerprint does.

A DOS reader that loads a whole small pack (`SHARED.FAP`, a world pack) checks the file checksum. A reader that loads a section from a big file (a song from `MUSIC.*`) reads the header and directory, then the one section it decodes, and checks only that section's checksum. It does not read the whole `MUSIC.*` file at start-up, which would cost a full read from a floppy (owner decision, 2026-10-29). The file checksum of `MUSIC.*` is checked by the build (`fizz-bake check`) and by `tools/check-dos.sh`, not at play time.

In the reader crate the section checksum is checked whenever stored bytes are handed out or decoded, so no caller can skip it by accident, and `PackView::parse_head` checks the header and directory of a big file without its file checksum.

## The compatibility number

The header's `u32` says which schema the pack was baked for. The executable refuses a pack whose number differs from its own. It is not the build version: tying a pack to the git build would invalidate every pack on every commit and break `fizz-bake check` and the committed manifests.

There are two kinds of number, because music should not be rebuilt when a level changes (owner decision, 2026-10-29).

**World and shared packs** (`SHARED.FAP`, `<WORLD>.FAP`): FNV-1a 32 started from `SCHEMA_HASH` and continued over the **content bytes** the game crate supplies. `SCHEMA_HASH` is `fizz-pack`'s own hash of everything a reader must understand, built from five parts, each an FNV-1a 32 fed with little-endian `u32` values:

1. `CONTAINER_HASH`: the format version, the header, directory entry and alignment sizes, and the codec numbers.
2. `LEVEL_HASH`: the `LEVL` section version (2) and its record sizes, the tags, the limits, the object, binding, trigger, condition and action numbers, the caption and cue code bases and the string charset extras, in the order [the `LEVL` section](#levl) lists under "Compatibility numbers and hashes".
3. One audio hash per device (OPL3, OPL2, speaker): the OPX header size, the no-loop marker (a `loop_offset` of 0xFFFFFFFF), the device's song tag (none for the speaker), number and tick rate, and the `FXPK` tag, header and entry sizes. A pack that holds effects for all three devices depends on all three.

The content bytes are not known to `fizz-pack`, which cannot see the rules. `fizz-ep2` supplies them as `tiles::pack_content()`: the ASCII text `fizz-ep2-content`; the tile property table (256 flag bytes, then 256 slope bytes, indexed by tile id, from `tiles::props()`; a door tile keeps no channel, so there is no third table); the named tile ids (`EMPTY`, `FILL`, `BLOCK`, `PLAT`, `SPIKE`, `CHOC`, `R45`, `L45`, `R22A`, `R22B`, `L22A`, `L22B`, `DOOR_R`, `DOOR_B`, `EXIT`, `WALLBG`, in that order); the snack kind numbers (cheezie 0, chocolate 1, cookie 2, soda can 3, the provisional elixir 4); the key colour numbers (red 0, blue 1, green 2) and the state limits `MAX_STATES` (32), `MAX_STATE_VALUES` (8), `MAX_BINDINGS` (64) and `MAX_DOOR_BINDINGS` (32), as one byte each; the sound effect ids (the 19 named ones in order, then `EFFECT_COUNT`, 22) and the shared caption ids (the 15 in order, then `CAP_COUNT`, 15), all from `fizz_ep2::cues`; `STYLE_COUNT` (7); and the `RULES_SEMANTICS` byte (2), which is bumped when the same rule bytes come to mean something else (it was 1 for the version 1 lock). That is 594 bytes. The compatibility number of a world or shared pack is `fizz_pack::compatibility(&pack_content())`, 0x91615AAF for the slice's build (it was a different number, with 847 bytes of content, before `LEVL` version 2). The tile property flags are in it (owner decision), so moving a `SOLID` or `SWITCHED` flag or a slope invalidates the packs, which is wanted: the flag changes how a baked level plays without changing a byte of it. Every new enum, table or constant that a baked record refers to by number must be added to `pack_content()` in the same change that introduces it; a unit test in `fizz-ep2` pins the layout and length of the bytes.

**Music files** (`MUSIC.OP3`, `MUSIC.OP2`): `audio_compatibility(device)` (there is none for the speaker), FNV-1a 32 of `CONTAINER_HASH` continued over that device's audio hash. It does not include levels, tiles, content bytes or the other devices, so a level change does not invalidate music, and a change to the speaker effect layout does not invalidate the music files. (`SHARED.FAP` holds effects for all devices and so does depend on all three audio hashes, through `SCHEMA_HASH`.)

**When it changes.**

- It changes when any listed layout, tag, section version, codec number or enum value changes (and, for a world or shared pack, when a tile property or id changes), and so a file baked before the change is rejected.
- It does not change with the content of a level, sprite or song, with the git commit, with the build version, or with the target.
- The schema hashes are pinned in the `fizz-pack` tests, so an edit to the schema is a deliberate test change.

The baker and the executable use the same `const`s, so they cannot disagree.

## Codec

The codec byte names how a section's stored bytes become its decoded bytes.

| Codec | Name | State |
| --- | --- | --- |
| 0 | Stored: the decoded bytes, unchanged | Version 1 |
| 1 | Raw deflate (RFC 1951, no zlib or gzip wrapper), baked at level 9 | Version 1 |
| 2 | LZMA family | **Reserved only** (owner decision, 2026-10-29). Not specified and not planned; a reader rejects it as unknown |
| 3 to 255 | Unassigned | Rejected |

**Why deflate.** The experiment in the planning notes measured 9 early-experiment songs for both chips, 1,274,402 bytes raw. Records compacted and laid out in the earlier `I3` layout (which [OPX](OPX.md) replaced; see [Audio sections](#audio-sections)), one section per device and raw deflate level 9 came to 147,136 bytes (11.5%), and Fizz 2's own Crater Fields to 5,091 bytes of 31,984 (15.9%). The figures were measured with that earlier layout and before the compaction rule below that keeps all key-on writes, so they must be re-measured on OPX once the encoder exists. The decoder in `fizz-pack` is about 2.3 KB of machine code at opt-level `z` and 2.9 KB at `s` (measured 2026-10-29 on i586 with bounds checks, plus about 1.3 KB of stack); the 1 KB first assumed was for a decoder without Rust's bounds and overflow checks. LZSS and LZ4 decode in a few dozen bytes but are about 24% bigger. The LZMA family is about 17% smaller than deflate (22.9 KB over the 9 songs and both chips, 1.6% of a floppy) for a 3 to 5 KB decoder and a probability model of 3.7 to 14 KB, so codec 2 stays reserved and unimplemented. Cycle costs are guesses, not measurements: about 0.1 to 0.25 s to inflate the largest section at 66 MHz.

**Rules.**

- **Baked by `fizz-bake`.** The encoder runs on the host only. The DOS and Web builds contain a decoder and no encoder.
- **Decoded once, at load, into a caller buffer** whose length is the entry's decoded length. The decoder is `no_std`, has no `alloc` and no `unsafe`, and writes only into that buffer. Matches read back from the output, so there is no separate 32 KB window.
- **Never in an interrupt.** Decoding happens in the main loop, before the section is used. The interrupt handlers only step data that is already decoded.
- **Bounded and strict.** The decoder rejects, with an error and without panicking: output that would pass the decoded length; a stream that ends before the decoded length is reached; a back-reference before the start of the output; an invalid Huffman code or block type; and stored bytes left over after the final block. Working tables are fixed-size arrays on the stack or in the caller's scratch, sized for the worst case.
- **Stored is allowed for any section.** The baker picks per section and stores it when the deflate result is not smaller than the decoded bytes. A small level (the slice's is 2,080 bytes) may well be stored.
- **No dictionary shared between sections.** Each section is compressed on its own. Solid compression of the OPL2 and OPL3 streams of a song would save 4.6% with deflate and costs a double decode, so it is not done.

## Sections

A section is a payload plus its directory entry. The tags in version 1 (the `LEVL` payload is section version 2):

| Tag | Content | Version 1 payload |
| --- | --- | --- |
| `LEVL` | One level: states, bindings, tiles, objects, rules, strings, spawn | Fully specified (section version 2) |
| `FXPK` | The effect set of one device (OPL3, OPL2 or speaker): an index, then one OPX file per effect | Fully specified |
| `OPL3` | One OPX file for an OPL3 (a song, or one segment of one) | Fully specified |
| `OPL2` | The same for an OPL2 | Fully specified |
| `MUSC` | The song table: names and metadata | Outline; **Not yet specified** |
| `SPRT` | Sprites | Outline; **Not yet specified** |
| `BDRP` | Backdrops | Outline; **Not yet specified** |

All payloads are described as they are after decoding.

### `LEVL`

One section per level, **section version 2** (the directory's section-version byte). Version 1 is dropped: no pack has shipped, so there is nothing to stay compatible with, and a version 2 reader refuses a `LEVL` with any other section version. This subsection is the specification of the format, and `fizz-pack` (`level.rs`, `write.rs`) and `fizz-ep2` implement it: the reader is `LevelView::new`, or `LevelView::with_directory` when the directory's section version and object count are to be checked as well, and the writer is `encode_level`, which refuses input over a limit. Where the code and this page differ, the code is wrong.

All multi-byte fields are little-endian. All `f64` fields are the IEEE 754 binary64 bit pattern as eight bytes, so no text and no rounding sits between the source and the rules. The directory count is the object count. What the states, bindings and rules mean (when a rule runs, in what order, what a death restores) belongs to `docs/RULES.md`; this page fixes the bytes and the checks that need no game knowledge.

**Fixed part, 32 bytes.**

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `w`, `u16`: width in tiles, at least 1 |
| 2 | 2 | `h`, `u16`: height in tiles, at least 1 |
| 4 | 1 | `floor_solid`, `u8`: 0 or 1, the `TileMap` floor flag |
| 5 | 1 | `name_len`, `u8`: bytes in the name, at most 48 |
| 6 | 2 | `object_count`, `u16`; equals the directory count |
| 8 | 8 | `spawn_x`, `f64`: Ben's start, in tile units |
| 16 | 8 | `spawn_y`, `f64` |
| 24 | 1 | `state_count`, `u8`: state records, at most 32 |
| 25 | 1 | `binding_count`, `u8`: binding records, at most 64 |
| 26 | 1 | `rule_count`, `u8`: rule records, at most 32 |
| 27 | 1 | `string_count`, `u8`: string records, at most 32 |
| 28 | 4 | Reserved, zero |

The version 1 field `switches` (the initial switch bits) is gone: each state's start value is in its record.

**Name.** `name_len` bytes of printable ASCII (0x20 to 0x7E), zero-padded to a multiple of 8, so every later part starts on an 8-byte boundary relative to the section. The name is for banners and the manifest; the rules never read it.

**Order.** Header, name (padded), states, bindings, objects, rules, strings, tiles.

**Length.** The decoded length is `32 + pad8(name_len) + 8 * state_count + 16 * binding_count + 32 * (object_count + string_count) + 48 * rule_count + w * h`. The reader checks that the decoded length equals it exactly.

**States.** `state_count` records of 8 bytes, the level's state table. A state's index is its position; rules, bindings and the death reset name states by index. A state's name and its value names are source-level only (the manifest and the editor keep them); the record holds numbers.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `values`, `u8`: how many values the state has, 2 to 8 (a one-valued state could never change, so it is refused) |
| 1 | 1 | `start`, `u8`: the value the level begins with and a death restores, below `values` |
| 2 | 1 | `flags`, `u8`: bit 0 `WORLD` is reserved and must be zero (a state is level-scoped; see the open points); the other bits are zero |
| 3 | 5 | Reserved, zero |

Value 0 is the first name in the source's list. The `off,on` default is two values with `off` as 0 and `on` as 1, and the start defaults to value 0.

**Bindings.** `binding_count` records of 16 bytes. A binding attaches one thing to one state and a **set of values**, so the thing reads the state without a rule. Anything not bound is unaffected by states.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `kind`, `u8` (table below) |
| 1 | 1 | `state`, `u8`: index into the state table |
| 2 | 1 | `mask`, `u8`: the **active set**, one bit per value (bit `i` is value `i`); not zero, and no bit at or above that state's `values` |
| 3 | 1 | Reserved, zero |
| 4 | 2 | `target`, `u16`: an object index for a light binding; `0xFFFF` otherwise |
| 6 | 2 | Reserved, zero |
| 8 | 8 | The rect, `x0`, `y0`, `x1`, `y1` as four `u16` (both corners included) for a tile binding; zero otherwise |

| `kind` | Meaning | Active when |
| --- | --- | --- |
| 0 | Invalid; rejected | |
| 1 | Door tiles: every switched tile in the rect | The state's value is in `mask` (the tiles are passable) |
| 2 | Light: the light object `target` | The state's value is in `mask` (the light is on) |
| 3 | Platform | Reserved; rejected until the game has level platforms. Active would mean the platform runs |
| 4 | Parallax panel | Reserved; rejected until `BDRP` is specified (F19). Active would mean the panel is shown |
| 5 to 255 | Unassigned; rejected | |

Every binding kind reads the same way: the mask is the **active** set (passable, lit, shown), so a door that is open for two of three values is one binding and not two. The mask costs nothing, because it takes the byte that held a single value in the first draft of this layout.

`fizz-pack` checks that a tile binding's rect is inside the map and not inverted, that a light binding's `target` is an object of kind 8, that a binding's state exists, that its mask is not zero and holds no bit at or above the state's `values`, that there are at most 32 door bindings (kind 1), and that nothing is bound twice (two bindings on one light, or overlapping tile rects). `fizz-ep2`, which knows the tile table, checks that every cell in a tile rect is a switched tile and that no switched tile is left unbound (a door that nothing opens or shuts). The tile ids keep their art and their colour, but they keep **no channel**: a switched tile carries no state, a door binding says which state its tiles read, and `tiles::props` loses its switch-channel byte. The limit of 32 door bindings is there so that the simulation may keep one derived bit for each (see `docs/RULES.md`); it is inside the 64 bindings of all kinds.

**Objects.** `object_count` records of 32 bytes each, in construction order (see below).

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `kind`, `u8` |
| 1 | 1 | `sub`, `u8`: depends on the kind (table below); zero when the kind has none |
| 2 | 2 | `flags`, `u16`: depends on the kind; zero when the kind has none |
| 4 | 4 | `aux`, `u32`: depends on the kind; zero when the kind has none. It replaces version 1's `channel`, which no kind needs now that bindings name the state |
| 8 | 8 | `x`, `f64`: the centre, in tile units (for an attached light, an offset from what it follows) |
| 16 | 8 | `y`, `f64` |
| 24 | 8 | `p`, `f64`: a parameter, zero when the kind has none |

| `kind` | Meaning | `sub` | `flags` | `aux` | `p` |
| --- | --- | --- | --- | --- | --- |
| 0 | Invalid; rejected | | | | |
| 1 | Red key | 0 | 0 | 0 | 0.0 |
| 2 | Blue key | 0 | 0 | 0 | 0.0 |
| 3 | Snack | Snack kind (0 cheezie, 1 chocolate, 2 cookie, 3 soda can, 4 elixir) | 0 | 0 | 0.0 |
| 4 | Chicken | 0 | 0 | Byte 0: group (0 none, 1 to 16) | Direction, -1.0 or 1.0 |
| 5 | Gadget | Reserved; rejected until the gadget milestone | | | |
| 6 | Switch | Style: 0 lever, 1 button | Bit 0 `HIT_SHOT` (1), bit 1 `HIT_STOMP` (2), bit 2 `USE` (4): which inputs set it off; at least one | 0 | 0.0 |
| 7 | Built-in enemy | Reserved for the roster (F8); rejected | | | |
| 8 | Light | Colour: 0 to 15, the palette bank | Bit 0 `ATTACHED` (1) | Byte 0 falloff (0 soft, 1 hard), byte 1 flicker (0 none, 1 candle, 2 pulse), bytes 2 and 3 `attach`, a `u16` (below) | Radius in tiles, greater than 0 and at most 16 |
| 9 | Custom enemy | Reserved for `DEFS` (F16); rejected | | | |
| 10 | Custom item | Reserved for `DEFS` (F16); rejected | | | |
| 11 to 255 | Unassigned; rejected | | | | |

A light's `attach` is `0xFFFF` for none, `0xFFFE` for Ben, and otherwise the index of an object of kind 3 or 4 that it follows; `ATTACHED` is set exactly when `attach` is not `0xFFFF`. A group number is for the `cleared` input: the group is cleared when no object with that group number is left alive (chickens today, custom enemies later). Whether switches and lights enter the state hash is for `docs/RULES.md`; the construction order below keeps their positions stable either way. The numeric values of `kind`, `sub` and the flag bits come from the enums in `fizz-ep2` and the constants in `fizz-pack`, and they are in the compatibility number.

**Rules.** `rule_count` records of 48 bytes each, in rule order (the order the sim evaluates them). A rule says: when this input happens, if up to two conditions hold, after a delay, do up to two actions.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `trigger`, `u8`: the input (table below) |
| 1 | 1 | `flags`, `u8`: bit 0 `REPEAT` (1); the others are zero |
| 2 | 2 | `delay`, `u16`: ticks between the input and the actions |
| 4 | 2 | `subject`, `u16`: what the input names (table below); `0xFFFF` is "any" where the table allows it, and "none" elsewhere |
| 6 | 1 | `arg`, `u8`: for `becomes`, the value; zero otherwise |
| 7 | 1 | Reserved, zero |
| 8 | 8 | The union: a rect, `x0`, `y0`, `x1`, `y1` as four `u16` (both corners included) for the inputs that name an area; for `time`, `at` (`u32`) then `every` (`u32`), in ticks; zero for every other input |
| 16 | 4 | Condition 0 |
| 20 | 4 | Condition 1 |
| 24 | 12 | Action 0 |
| 36 | 12 | Action 1 |

The version 1 guard (one channel, with the flags `GUARD` and `GUARD_ON`) and the lock trigger are gone: a guard is a condition, and a lock is a rule built from them (the worked example below). Ben being hurt is not an input of this version: the `hit` of the first draft of `RULES.md` is reserved under the name `hurt`, with no trigger number.

| `trigger` | Input | `subject` | Union |
| --- | --- | --- | --- |
| 0, 1 | Invalid, rejected (1 was the version 1 lock) | | |
| 2 | Start of the level and of each life | `0xFFFF` | Zero |
| 3 | Ben enters the rect | `0xFFFF` | A rect |
| 4 | Ben takes an object | The index of a key or snack object | Zero |
| 5 | A shot lands in the rect | `0xFFFF` | A rect |
| 6 | A stomp | The index of a chicken (or built-in enemy), or `0xFFFF` for any | Zero |
| 7 | A stun | As stomp | Zero |
| 8 | Time | `0xFFFF` | `at`, `every` |
| 9 | Ben touches a rect: his front probe (the tile in front of him at `py + 0.2`) lands in it, or his body overlaps it. A level, not an edge | `0xFFFF` | A rect, any size; a cell has equal corners |
| 10 | Ben touches an object | The object index | Zero |
| 11 | Ben uses a switch (Up pressed beside it) | The index of a switch | Zero |
| 12 | A switch is hit (by a shot or a stomp, as its flags allow) | The index of a switch | Zero |
| 13 | A defeat | As stomp | Zero |
| 14 | A group is cleared | The group number, 1 to 16 | Zero |
| 15 | A state becomes a value, or changes (`arg` is the value, or `0xFF` for any change) | The state index | Zero |
| 16 to 255 | Rejected | | |

Triggers 2 to 8 keep their version 1 numbers. A `time` rule fires at tick `at` (at least 1) and, when `every` is not zero, again every `every` ticks; `REPEAT` is set exactly when `every` is not zero, and `every` is at least `MIN_PERIOD` (6) so a timer cannot run away. For the other triggers `REPEAT` lets the rule fire again after it has fired; without it a rule fires once per life. A touch is a level, so a `REPEAT` touch rule runs on every tick Ben is in contact, and the dialect warns when such a rule toggles or advances a state.

**Conditions**, 4 bytes each. A condition is empty (all four bytes zero) or one of the kinds below, and every non-empty condition must hold.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `kind`, `u8`: 0 none, 1 state is in a set, 2 Ben holds a key, 3 Ben does not hold a key; 4 to 6 are numbered and rejected (4 ammo at least, 5 score at least, 6 a group is cleared) |
| 1 | 1 | `a`, `u8`: the state index (kind 1) or the key colour (kinds 2 and 3) |
| 2 | 1 | `b`, `u8`: the set of values as a mask, bit `i` for value `i` (kind 1), not zero and with no bit at or above that state's `values`; zero otherwise |
| 3 | 1 | Reserved, zero |

"State is not" is the complement mask, which the source writes as `is not` and the baker lowers; there is no kind for it. A complement that leaves no value (every value excluded) is refused at the source.

**Actions**, 12 bytes each. An action is empty (all twelve bytes zero) or one of the ops below, run in slot order.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `op`, `u8` (table below) |
| 1 | 1 | `a`, `u8`: the state index, key colour or caption code, by `op` |
| 2 | 1 | `b`, `u8`: the value (`set`) or the style override (`word`), by `op` |
| 3 | 1 | Reserved, zero |
| 4 | 4 | `n`, `i32`: the amount (`score`, `ammo`) or the cue id (`cue`); zero otherwise |
| 8 | 2 | `x`, `u16`: the cell where a raised event is placed; zero for the other ops |
| 10 | 2 | `y`, `u16` |

| `op` | Action | Fields |
| --- | --- | --- |
| 0 | None | All zero |
| 1 | Set a state to a value | `a` state, `b` value |
| 2 | Toggle a state | `a` state, which must have exactly 2 values |
| 3 | Next: the state's next value, wrapping | `a` state |
| 4 | Spend a key: Ben no longer holds it; nothing, when he does not hold it | `a` key colour |
| 5 | Give a key | `a` key colour |
| 6 | Score | `n` the points, 1 to 9999 |
| 7 | Ammo | `n` the shots, 1 to 99 |
| 8 | Raise a word | `a` caption code, `b` style override or `0xFF` for the string's own, `x`, `y` the cell |
| 9 | Raise a cue | `n` the cue id, `x`, `y` the cell |
| 10 to 255 | Rejected | |

**Caption codes** (`op` 8, `a`): below `0x40`, a shared caption id; `0x40 + i`, the level's string `i`; `0x80 + i`, the world's string `i`; the rest are rejected. `CUE_NONE` (0xFF) is not valid in a word action, because an unused action is empty instead. **Cue ids** (`op` 9, `n`): below `0x100`, a shared sound effect id (`EFFECT_COUNT` is 20 today); `0x100 + i`, entry `i` of the world's sound library, which holds at most 256 entries (`CUE_ID_MAX`, 0x1FF); the rest are rejected. A word and a cue are two actions, so a rule that shows `bonk` and plays a cue uses both slots. The door's own sound and word are not actions: see the door event below. The world's strings and sound library live in the world pack: `fizz-pack` checks the level's own range (`0x40 + i` below the string count) and the shared and world ranges are checked by the game crate, which knows the tables.

**Strings.** `string_count` records of 32 bytes each, the level's own caption words; a word action names string `i` with caption code `0x40 + i`. The layout is unchanged from version 1.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `len`, `u8`: bytes of text, 1 to 30 |
| 1 | 1 | `style`, `u8`: below `STYLE_COUNT` (7): white, cyan, red, green, blue, yellow, grey |
| 2 | 30 | The text: UTF-8, then zero padding |

The text may use U+0020 to U+007E and the eight extras U+00B7 (middle dot), U+2013, U+2014, U+2018, U+2019, U+201C, U+201D and U+2026, which are the glyphs of the shared font. The widest string a caption can show on screen is a presenter limit, not checked by the reader.

**Tiles.** After the strings, `w * h` bytes, one tile id each, in `TileMap` order: row-major, row 0 (the bottom) first. A reader copies them into the map with `copy_from_slice`. The tile ids are those of `fizz-ep2::tiles`. A tile id outside the table is a corrupt level.

**Limits.** 32 states, 8 values per state, 64 bindings (at most 32 of them door bindings), 32 rules, 32 strings, 16 groups, 48 bytes of name, 30 bytes of string text, 2 conditions and 2 actions per rule. They are the level studio's proposed defaults, adopted here; each is a `const` in `fizz-pack` and in the compatibility number, so a later change moves the numbers and refuses older packs rather than misreading them. A level that needs more chains rules through a state: one rule sets a state and another reads it with `becomes`.

**Construction order.** The baker writes objects stably sorted by kind: red key, blue key, snacks in source order, the chicken, then the switches and the lights. The reader applies them in record order. Objects of the same kind keep their source order, because the state hash indexes snacks and chickens by position, and because a rule names an object by its index.

**Reader checks.** `LevelView::new` in `fizz-pack` refuses, each with its own error (`LevelView::with_directory` adds the first and the last, which need the directory entry): a section version other than 2 (`BadVersion`); a section shorter or longer than the length formula gives; `w` or `h` of 0; a `floor_solid` over 1; a name over 48 bytes, a name byte outside 0x20 to 0x7E or non-zero name padding; a count over its limit (checked before the length); non-zero reserved bytes anywhere; a state with fewer than 2 or more than 8 values, a start not below `values`, or a non-zero `flags` (the `WORLD` bit included) (`BadState`); a binding that breaks the binding paragraph above, including a zero or out-of-range mask and a 33rd door binding (`BadBinding`); an object `kind` of 0 or over 10, or a kind that is reserved; a non-zero `sub`, `flags`, `aux` or `p` where the object table says zero; a snack `sub` over 4; a chicken direction other than -1.0 or 1.0; a group over 16; a light with no radius or one over 16, or with an attach target that is not a snack, a chicken or Ben; a switch with no input flag; a spawn or object centre outside the map (`0 <= x < w`, `0 <= y < h`, which also refuses NaN and infinities; an attached light's `x` and `y` are offsets and need only be finite and within 16); a non-finite `p`; a rule that breaks the next paragraph (`BadRule`); a string that breaks its rules (`BadString`); and, through `LevelView::with_count`, an object count that differs from the directory count. Every error that names a record carries the record's index (`BadState(i)`, `BadBinding(i)`, `BadKind(i)`, `BadObject(i)`, `BadPosition(i)`, `BadRule(i)`, `BadString(i)`), so the baker can point at the source line; the header errors (`BadField`, `BadSize`, `BadName`, `BadSpawn`, `TooManyStates`, `TooManyBindings`, `TooManyRules`, `TooManyStrings`, `Truncated`, `TrailingBytes`) name none. Records are checked in this order, and the first bad one is named: states, objects, bindings, strings, rules.

**Rule checks.** A rule is refused when: the trigger is not 2 to 15 (so version 1's trigger 1 is refused); a flag bit other than `REPEAT` is set; the subject does not fit the trigger (a take that does not name a key or snack object, a stomp, stun or defeat that names neither a chicken or enemy object nor `0xFFFF`, a use or hit that does not name a switch, a touch that does not name an object, a group that no object carries, a trigger 15 that does not name a state, a subject other than `0xFFFF` on any other trigger); `arg` is neither a value of the named state nor `0xFF` on trigger 15, or is not zero elsewhere; a rect is inverted or reaches outside the map (a touch rect may be any size); the union is not zero on a trigger without one; a `time` rule has `at` of 0, an `every` between 1 and 5, or `REPEAT` that disagrees with `every`; a condition or action has a kind or op outside its table (the reserved condition kinds 4 to 6 included), a state index at or over `state_count`, a value at or over that state's `values`, a condition mask that is zero or has a bit at or over that state's `values`, a toggle of a state that does not have exactly 2 values, a key colour over 2, an amount outside its range, a raised cell outside the map, or a non-zero field that its kind says is zero; a word action's caption code is `CUE_NONE` or reaches `0x40 + i` with `i` at or over the string count; or the rule has no action at all. A string is refused when its length is 0 or over 30, its style is 7 or more, bytes after the text are not zero, the text is not valid UTF-8, or a character is outside the charset above. The map's maximum width in pixels is not checked here.

**Same bytes on both targets.** The Web and DOS packs hold byte-identical `LEVL` payloads. The stored bytes may differ when a target picks another codec for a section, but the decoded bytes, and their manifest FNV, do not. A test in the baker bakes both targets and compares the decoded `LEVL` bytes.

**What changed from version 1.** Version 1 is dropped, not read.

| Part | Version 1 | Version 2 |
| --- | --- | --- |
| Section version byte | 1 | 2 |
| Fixed part | 32 bytes, with `switches` at byte 4 and counts for rules and strings | 32 bytes, with `switches` gone, `floor_solid`, the name length and the object count moved up, and counts for states, bindings, rules and strings |
| Switch state | 32 on and off channel bits, one channel fixed by each door tile | A state table of up to 32 states with 2 to 8 values and a start value each. A door tile keeps no channel; `tiles::props` loses its switch-channel byte |
| Doors, lights | The tile's channel; lights are constants | Binding records: a rect or a light, a state and a mask of active values |
| Object kinds | 1 to 4 built, 5 to 7 reserved | 1 to 4, 6 and 8 built; 5, 7, 9 and 10 reserved |
| Object fields | `channel` (`u32`) | `aux` (`u32`), with a meaning per kind |
| Rule record | 32 bytes: one trigger, a guard channel, one channel action and one cue | 48 bytes: one trigger, two conditions, two actions |
| Lock | Trigger 1 | A rule: a touch input over the door's whole rect, the conditions "state is in a set" and "holds", the actions "set" and "spend" |
| Cue | Slot B holds an effect and a caption together | Two separate actions, a word and a cue |
| Caption codes | `0x40 + i` for level strings | The same, and `0x80 + i` for world strings |
| Trigger numbers | 1 to 8 | 2 to 8 unchanged, 9 to 15 added, 1 retired |
| Condition kinds | Guard flags on one channel | 1 state in a set, 2 holds, 3 does not hold; 4 to 6 reserved |
| Length | `32 + pad8(name) + 32 * (objects + rules + strings) + w * h` | The formula above |

**The slice as a worked example.** The slice (32 by 48, a 16-byte name, eleven objects (the two keys, eight snacks including the elixir, and the chicken), two states, two bindings, two rules, no strings) is 2,080 bytes: the header 32, the name 16, the states 16, the bindings 32, the objects 352, the rules 96 and the tiles 1,536. The record sizes are those of the first draft of this layout, so the length does not move; the bytes inside the bindings, the touch rects and the conditions do. The source of the two locks is `lock red-door key=red at=15,21` and `lock blue-door key=blue at=22,39`, each of which expands to a state, a door binding and a touch rule. The state table is `red-door` (values `closed`, `open`, start 0) and `blue-door` (the same); each binding makes its door's tiles passable when its state is in the set `{open}`. The red door is the column of tiles at x 15, y 20 to 22, the blue door the column at x 22, y 38 to 40. In bytes, every number little-endian:

```text
state 0 (red-door):   02 00 00 00 00 00 00 00      2 values, start 0 (closed), flags 0
state 1 (blue-door):  02 00 00 00 00 00 00 00

binding 0:  01 00 02 00 FF FF 00 00  0F 00 14 00 0F 00 16 00
            |  |  |     |           x0=15 y0=20 x1=15 y1=22
            |  |  |     target: none
            |  |  mask 0b10: value 1 (open) is active
            |  state 0
            kind 1: door tiles
binding 1:  01 01 02 00 FF FF 00 00  16 00 26 00 16 00 28 00      x 22, y 38 to 40

rule 0, the red lock: while Ben touches the door (15,20..22), the red door is closed and he
holds the red key, open the door and spend the key.
  header    09 00 00 00 FF FF 00 00         trigger 9, no repeat, no delay, subject none, arg 0
  union     0F 00 14 00 0F 00 16 00         the door's whole rect (15,20)..(15,22)
  cond 0    01 00 01 00                     state 0 is in the set 0b01 {closed}
  cond 1    02 00 00 00                     Ben holds key colour 0 (red)
  action 0  01 00 01 00 00 00 00 00 00 00 00 00      set state 0 to value 1 (open)
  action 1  04 00 00 00 00 00 00 00 00 00 00 00      spend key colour 0
rule 1, the blue lock: the same bytes with the rect (22,38)..(22,40) in the union
(16 00 26 00 16 00 28 00), cond 0 "01 01 01 00" (state 1 is in {closed}), cond 1 "02 01 00 00"
(key colour 1), action 0 "01 01 01 00 ..." and action 1 "04 01 00 00 ...".
```

Rule order is the order the sim evaluates them: the red lock first, then the blue (the order of the `lock` lines). The touch covers the door's whole rect, as version 1's lock probe did: version 1 tested the tile in front of Ben at `py + 0.2` against the door's channel, so any cell of the door's run accepted the key, and a touch of one cell at the door's middle would never fire for a Ben standing at the door's foot. The door's sound and word are not actions of the rule: any door binding that becomes passable raises the game's built-in door event (`Event::DoorOpen`, effect 19) at the centre cell of its rect, which keeps both action slots of the lock for `set` and `spend`. Version 1 raised that event at the touched column and Ben's top, which is a different position, so the two `dooropen` lines of the replay transcript changed when the rule phase landed and the lock became a plain touch rule in it (`EVENT 889 dooropen 248 344` and `EVENT 1615 dooropen 360 632`, from `248 352` and `360 682`; `docs/RULES.md`, section 3 and open question 1).

**Compatibility numbers and hashes.** `LEVEL_HASH` is rebuilt for version 2. It starts from the text `fizz-pack-level`, then feeds, as little-endian `u32` values and in this order: the `LEVL` section version (2); the sizes of the header, state, binding, object, rule, condition, action and string records (32, 8, 16, 32, 48, 4, 12, 32); the tags `LEVL`, `SPRT`, `MUSC` and `BDRP`; the limits `MAX_STATES`, `MAX_STATE_VALUES`, `MAX_BINDINGS`, `MAX_DOOR_BINDINGS`, `MAX_RULES`, `MAX_STRINGS`, `MAX_GROUPS`, `STRING_TEXT_MAX` and `MIN_PERIOD`; `STYLE_COUNT`; the object kind numbers 1 to 10 and the switch and light flag bits; the binding kind numbers 1 to 4; the trigger numbers 2 to 15; the condition kind numbers 1 to 6; the action op numbers 1 to 9; the `REPEAT` flag and the state `WORLD` flag bit; the caption code bases (`0x40`, `0x80`), the cue id base (`0x100`), the "any" subject (`0xFFFF`) and the "any change" argument (`0xFF`); and the eight string charset extras. The resulting values are `LEVEL_HASH` 0x46FA651A and, from it, `SCHEMA_HASH` 0xA1BB8460 (they were 0x76AA52BB and 0x25E81944 for version 1); the order is listed so a second implementation can reproduce them, and `crates/fizz-pack/tests/levl_hash.py` does, independently of the Rust constants. The tests in `fizz-pack` pin both. `SCHEMA_HASH` follows `LEVEL_HASH` (it feeds `CONTAINER_HASH`, `LEVEL_HASH` and the three audio hashes, so only `LEVEL_HASH` moves it). The content bytes of a world or shared pack change as well: `tiles::pack_content()` drops the 256 switch-channel bytes of the tile property table and `MAX_CHANNELS`, adds the state, value, binding and door binding limits, and its `RULES_SEMANTICS` byte becomes 2, so every world and shared pack's number moves, and the music numbers do not. The pack container's `FORMAT_VERSION` (1) and `CONTAINER_HASH` do not change, because the header and directory are untouched; only the `LEVL` section version does.

**Closed by this layout.** The door's sound and word are a built-in event of a door binding opening, not actions. A binding holds a set of values, not one. Channels are not in the bytes. A lock is a plain rule, and a touch is a level over a rect. States are level-scoped, with `WORLD` reserved.

**Open points for version 2.** Each has the default this page uses until the owner decides.

1. **A third key and more.** Kind numbers exist for the red and blue keys only (key colour 2 is numbered already). The default is one key kind with `sub` the colour (red 0, blue 1, green 2, gold 3), retiring kind 2, with a held count (0 to 3) for each colour in place of a flag. That changes the hash of every level, so it belongs in the same change as this layout; the layout above does not depend on it, because a rule names a key by colour.
2. **Light details.** A numeric falloff and a numeric flicker period do not fit in `aux` beside the attach target; they would take the `flags` high byte or a second word.
3. **The tile ids of doors.** The default keeps `DOOR_R` and `DOOR_B` as two switched tile ids, because the map letter and the art say the colour; a green door would be a third id.
4. **World-scoped states** that span levels are not in a level's table. The default is level-only; the `WORLD` bit of a state's `flags` is reserved so that a world table can give it meaning without a new section version.
5. **Objects that raise events** by themselves have no field yet; today a rule raises them.
6. **Whether the pack's `FORMAT_VERSION` should move** with the section version. This page keeps it at 1.
7. **The derived switch word and the golden (closed).** The simulation hashes one derived bit for each door binding in place of version 1's channel bits: door binding `i` owns bit `i`, so the slice's red door is bit 0 and its blue door bit 1 (they were the channels 1 and 2). That moved the slice's switch word (0b110 became 0b11), and the wall kick's `wall_t` and `kick_side` now enter the hash unconditionally, so the golden was re-pinned once, in the change that landed the layout: `FINAL TICK 1800 HASH 843736be6fdaeb76` became `7f6a1a1248c4b0e8`, and not one `EVENT` line moved. The rule phase re-pinned it again, to `ec9195c222b626f2`, because a world that declares a state hashes its state block (`docs/RULES.md`, section 11).

### Audio sections

`OPL3` and `OPL2` each hold one **complete [OPX](OPX.md) file** (its 32-byte header and its command stream) for one song of one OPL device, or for one segment of a song; there is no speaker music. `FXPK` holds the effects of one device, including the speaker, as OPX files too. A device section is chosen at load, so a machine decodes and keeps one device's form. The pack deflates the section as a whole, so the same bytes, taken out of the pack, are a standalone `.OPX` file that any OPX reader plays, and an `.OPX` file from a tool is packed unchanged. OPX is specified in [OPX.md](OPX.md), which is the only definition of the stream; this section says only what the pack adds.

**Grids.** OPL music and OPL effect files have `grid_hz` 560 and speaker effect files 140, as `fizz-sfx` and `fizz-music` do today. The reader rejects another value in a pack (OPX itself allows any grid). The `chip` field must match the section's device: OPL3 for `OPL3`, OPL2 for `OPL2`, and in an `FXPK` the device byte of the set.

**No separate audio header.** The earlier draft of this section had its own 16-byte audio header (loop start, prelude and loop ticks, rate, chunk and chunks) and a record layout named `I3`. Both are gone: the OPX header carries `grid_hz`, `total_ticks` and `loop_offset`, and the loop is the `LOOP` command. The prelude length is the clock at `LOOP`, which a stepper gets in one pass; the loop length is `total_ticks` minus it; `MUSC` holds a copy for a presenter that wants a song's length without decoding.

**Directory count.** The directory count of an audio section is the number of register writes (`W0`, `W1` and `SPK` commands) in the file, at most 65,535 (the `u16` limit is kept, owner decision). The largest early-experiment stream is 20,535 after compaction. A reader may check the count by one pass.

**Segment sections.** A song longer than a section, in writes or in decoded bytes (see [Memory limits](#memory-limits)), is stored as several OPX files in consecutive sections of its tag, **segments**, each a complete OPX file that begins with the `F1` segment extension (`index`, `count`, `loop_segment`; see [OPX.md](OPX.md#extension-opcodes)). Segment 0 is first. The sections are cut between commands and are played in order; the register state carries over, so no segment repeats the setup. A segment's own `loop_offset` is 0xFFFFFFFF and has no `LOOP`; `loop_segment` is the index of the first segment of the looping part (earlier segments are the prelude), or 0xFF for a song that plays once, and after the last segment the player returns to that segment. The player decodes the next segment into a second buffer from the main loop while the current one plays (double buffering), so the loop segments must each fit a buffer. Segments replace what the earlier draft called chunk sections; they are for very long songs and are not routine, because an ordinary song is one section. A reader of the container does not need to understand segments; the audio player does, and it is written in the audio pack package.

**Measured sizes, with the earlier layout.** The earlier `I3` layout was the records back to back as `reg` (`u8`), `val` (`u8`) and a varint of `wait << 1 | bank`, three to five bytes a record (3.02 bytes on average over the early-experiment songs). Measured with it: a decoded section of 568 KB over 9 early-experiment songs against 751 KB as 4-byte records, and the deflate figures under [Codec](#codec). **Those figures were measured with the `I3` layout and must be re-measured on OPX.** OPX spends a one-byte `WAIT` command where `I3` folded the wait into the record, so OPX is likely somewhat larger than `I3` before deflate; how much, and after deflate, is for the re-measurement. A stepper reads OPX directly and never expands it, so heap use equals the decoded length.
**Compaction (a separate, tested lowering step before the codec).** The baker drops a write that changes nothing and adds its wait to the previous kept record, so total ticks and the sequence of state-changing writes are unchanged.

- A write is redundant when the register (bank and register number) already holds that value, by last write wins. A register's first write is always kept, because the chip's state is unknown at the start.
- **The baker never drops a write to a register from 0xB0 to 0xB8 (in either bank) or to 0xBD** (owner decision, 2026-10-29), because rewriting a key-on or rhythm register can have side effects on the chip. A kept write still takes part in the last-write-wins state, so later writes to other registers are judged correctly. The experiment treated every register as last-write-wins and did not check this against an emulator, so its figures include 190 such drops over the early-experiment streams. **Every savings figure in this document and in the experiment (the 26.3% of records dropped, the 568 KB decoded, the 147,136-byte deflate total, the per-song budgets) must be re-measured under this rule once the encoder exists**; until then they are upper bounds on the saving.
- OPX has no wait limit below 2^32 ticks (a long gap is `LWAIT`s), so the baker has no 0x7FFF rule. A converter to the engine's 4-byte `OplStep` form splits waits over 0x7FFF itself (see [OPX.md](OPX.md#equivalence-with-the-runtime-oplstep-record)).
- The baker refuses a stream with more than 16 writes in one tick, the cap the DOS audio service relies on. the nine early-experiment songs all break it (34 to 129 writes in a tick), so they cannot be packed as they are; only Crater Fields complies. Lowering them is the job of the source tools, not of the baker.
- Dropping redundant writes matters more for decoded memory than for compressed size: it removes 26.3% of the early-experiment records but gains deflate only 3.9 to 4.7%.

**Speaker effects.** The speaker form of `FXPK` holds OPX files with `chip` set to PC speaker and `grid_hz` 140: each 140 Hz step of a divisor list is an `SPK` command and a one-tick wait (equal runs may be held), with 0 for gate off (see [OPX.md](OPX.md#the-fizz-profile)). This settles the layout that was open before; it still has to be measured against the old divisor-list form. The `FXPK` index and the directory, codec and checksum rules apply unchanged.

**Effects (`FXPK`).** The sound effects live in `SHARED.FAP` as one `FXPK` section per device holding every effect of that device, with an index. They are the 1st, 2nd and 3rd `FXPK` sections: OPL3, OPL2, speaker, in that order. Whole sets are used because the effects are tiny (about 100 bytes each) and one section per effect would spend a 24-byte directory entry, a 16-byte alignment and a checksum on each. A new tag was chosen over reusing `OPL3`, `OPL2` and `SPKR` with an index header because the tag then says what the payload is: a song reader can never be handed an effect set, a song's index among `OPL3` sections is not shifted by the effects, and the music files hold no `FXPK`. Decoded layout:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `device`, `u8`: 0 OPL3, 1 OPL2, 2 speaker; must match the section's position (1st, 2nd, 3rd `FXPK`) |
| 1 | 1 | Reserved, zero |
| 2 | 2 | `count`, `u16`: the number of effects; equals the directory count |
| 4 | 2 | `rate`, `u16`: 560 or 140 for the device |
| 6 | 2 | Reserved, zero |
| 8 | 16 * `count` | The index, one entry per effect in `fizz-sfx` order |
| after | | The streams, back to back, each starting on a 4-byte boundary |

An index entry is 16 bytes: `offset` (`u32`, from the start of the section to the effect's first record), `length` (`u32`, bytes of the OPX file), `records` (`u16`, the number of register writes in it), reserved (`u16`, zero) and `ticks` (`u32`, the effect's total ticks). Entries are in ascending `offset` order with no overlap, every `offset + length` is at most the decoded length, and each stream is a complete OPX file (header and commands) for the device, which is why the streams start on a 4-byte boundary and are 32 bytes longer each than their commands. An effect plays once: its OPX header has `loop_offset` 0xFFFFFFFF. Unused effect slots of a device with no form for an effect are an entry with `records` 0 and `length` 0. DOS decodes only the section for the detected device, once at start-up, and it stays resident (it is small), so the effect player indexes it directly. A device whose effects are not baked yet has a placeholder `FXPK` section. The reader for this layout is written in the audio pack package; the container, `fizz-pack`'s tag and size constants and the compatibility number already include it.

**Per-device sections.** The baker emits both OPL forms of every song, and all three device forms of the effects; DOS seeks to the section for the detected device (`MUSIC.OP3` or `MUSIC.OP2` for a song, the matching `FXPK` for effects), decodes it and ignores the rest. A machine with neither chip decodes the speaker `FXPK` and no song. The OPL2 and OPL3 forms of one song overlap in content, and that overlap costs floppy space only. If it matters, the planned single-stream OPL3 with a skippable second bank (M4.5 `opl3-core-extras`) removes the duplicate structurally, and would be a new section version.

### `MUSC`

The song table in `SHARED.FAP`: one record per song. **Not yet specified.** It is expected to hold the song's id, display name, the drum and card counts that `fizz-music` reports today, and the loop tick counts (a copy of what the audio headers hold, so a presenter can show a length without decoding a stream). The Web build keeps its song sources in the JavaScript bundle (Undertone) and may read `MUSC` for the same names.

### `SPRT`

Sprites for Ben, enemies, items, the HUD and the world. **Not yet specified.** Questions the layout has to settle, from [`SPRITES.md`](SPRITES.md) and [`ARCHITECTURE.md`](ARCHITECTURE.md#open-questions): bits per pixel (the `const` form is one byte a pixel, with 255 as transparent), a mask or a transparent colour, whether a sprite is stored as runs, an index with each sprite's size and frame count, and what a Web sprite section adds over a DOS one. The directory count is the number of sprites.

### `BDRP`

Backdrop art for the parallax layers of Milestone 3. **Not yet specified.** Backdrops differ most between the targets: the Web may carry richer art than the 16-colour DOS form.

## Target profiles

A pack is baked for one target, named in the header. The structure is the same; the content differs.

| | Web | DOS |
| --- | --- | --- |
| Header target | 1 | 2 |
| `LEVL` | Yes | Yes, byte-identical to Web |
| `SPRT`, `BDRP` | Yes, possibly richer (**not yet specified**) | Yes, DOS form |
| `MUSC` | Yes | Yes |
| `OPL3`, `OPL2` (music files), `FXPK` (effects, in `SHARED.FAP`) | **None.** The Web plays the Undertone sources bundled in the page | Yes: songs for the two OPL devices, effects for all three |
| Audio files `MUSIC.*` | None | Yes |
| Output directory | `dist/web` | `dist/dos` |

- **Missing forms.** If an asset has no form for a target, the baker writes a placeholder section (decoded length 0, count 0) and prints a warning, so Web iteration is never blocked. The DOS gate still reports the gap and the placeholder is never silently treated as content.
- **Web loading.** The page fetches the packs by name, so the browser cache and Tauri bundling work as for any file. Compression is decoded by the same Rust decoder that DOS uses, in WebAssembly, and not by HTTP content encoding, so the bytes the rules see are the same.
- **Parity.** The only content the rules read from a pack is `LEVL` (and, later, whatever gameplay data a section adds). It is identical on both targets, so the state hash is the same. Presentation sections may differ.

## Loading on DOS

The DOS build loads packs through the existing runtime calls (`File::open`, `len`, `read`, `seek_start`, `read_to_vec`), with `FailCritical` on for every read so a missing disk or an open drive door returns an error instead of printing "Abort, Retry, Fail?" over the screen.

**Where it looks.** For each pack name, the runtime tries the current directory first and the executable's directory second (from `env::program_path()`), and uses the first file it can open (owner decision, 2026-10-29). It does not search `PATH` and does not fall back to another name.

**When.** `SHARED.FAP` loads before mode 13h is set, so any failure prints in text mode. A world pack loads when a level of that world starts, and a `MUSIC.*` section when the song changes. The first level loads from a world pack at start-up for the same reason as `SHARED.FAP`. Packs are never all loaded at once; on a real floppy that costs time and memory.

**Order of checks for a pack.**

1. Open the file, read the header, check the magic, format version, header size and flags.
2. Check the target is DOS and the stem is the one asked for.
3. Check the compatibility number against the executable's own (the world-pack number for `SHARED.FAP` and `<WORLD>.FAP`, the device's audio number for `MUSIC.*`).
4. Check the total length against the file's length; read the rest.
5. Check the file checksum (a whole small pack) or, for a section of a large file (`MUSIC.*`), only the stored-bytes checksum of the section being decoded, from the directory. `MUSIC.*` is never read whole at start-up.
6. Check the directory rules above.
7. Decode the sections that are needed, each into a buffer of its decoded length, and check any per-section rule (for a level, the length and object checks).

**Failure is a hard exit.** There is no fallback to a compiled-in level (the slice level in the code is a test oracle, not a runtime fallback). The program prints a message in text mode, restores its vectors through the normal clean-up list, and exits with status 1. If the failure happens while the screen is in mode 13h, the program first restores text mode. The messages, with `<N>` for a file name and numbers in decimal or hexadecimal as marked:

| Cause | Message |
| --- | --- |
| File not found | `FIZZ2: cannot find <N>. Put it in the same directory as FIZZ2.EXE.` |
| Read error or disk not ready | `FIZZ2: cannot read <N> (DOS error <code>).` |
| File too short or length mismatch | `FIZZ2: <N> is incomplete. Copy it again.` |
| Bad magic, version, flags or header size | `FIZZ2: <N> is not a Fizz 2 data file.` |
| Wrong stem (a renamed or misplaced file) | `FIZZ2: <N> is the wrong file (it says it is <STEM>). Copy it again from the download.` |
| Wrong target | `FIZZ2: <N> was made for the Web version.` |
| Compatibility number differs | `FIZZ2: <N> does not match this FIZZ2.EXE (data <hex8>, program <hex8>). Use files from the same download.` |
| File checksum mismatch | `FIZZ2: <N> is damaged (checksum <hex8>, expected <hex8>). Copy it again.` |
| Section checksum mismatch | `FIZZ2: <N> is damaged (section <TAG> <index>, checksum <hex8>, expected <hex8>). Copy it again.` |
| Directory rule broken, or a section fails to decode | `FIZZ2: <N> is damaged (section <TAG> <index>). Copy it again.` |
| Required section missing, or a placeholder where content is needed | `FIZZ2: <N> has no <TAG> <index>.` |
| A section does not fit in memory | `FIZZ2: not enough memory to load <TAG> <index> from <N> (needs <bytes> bytes, <bytes> free).` |

The exact wording is final only once `tools/check-dos.sh` has a negative case for each row. After a successful load the platform prints a line for each pack, `PACK <name> <bytes> <checksum>`, which the DOS gate checks with the rest of the `SIZE` and `HEAP` lines; `SIZE` counts the executable and the packs together.

## Memory limits

- **Machine floor.** The stated minimum is a 4 MB machine with about 2 MB free (owner decision, 2026-10-29). Fizz 1 was a 4 MB-class game that needed 1 MB free and refused to start below it, because its sound handler faulted on pages the DPMI host had swapped out when DOSBox ran with `memsize` 2 and 1. 1 to 2 MB machines are not a design target; DOSBox's `memsize` 1 is only its smallest setting. Fizz 2 only warns when memory is below its floor, as the code does today, and carries on; nothing in the pack format adds a fallback for such machines.
- **Free memory.** The DOSBox ladder reads about 2,256 KB free at `memsize` 4 and about 208 KB at `memsize` 2, and after the load about 96 KB at `memsize` 2 and about 56 KB at `memsize` 1 (after the 256 KB heap and the executable). A decoded section, its stored bytes (while decoding) and the decoder's working tables must all fit what the floor machine has.
- **Levels.** A level is small (the slice's decoded section is 2,080 bytes; a Fizz 1-sized level of 192 by 28 tiles is about 6 KB). The tiles are copied into the `TileMap`, so the section buffer is freed once the world is built.
- **Audio, locked memory.** The decoded buffer that the sound interrupt reads must be **locked** (unpageable) memory, locked with the DPMI lock call before the first interrupt can read it and unlocked when the buffer is released. A paged-out page under an interrupt is the failure Fizz 1 hit. The stored bytes are read only by the main loop and need no lock, and can be freed as soon as the decode finishes, so what stays resident is the decoded length.
- **Limit.** The default per-section decoded limit is **96 KB** (owner decision, 2026-10-29), for audio sections. The baker takes the limit as a parameter, checks decoded plus stored length against free memory in its report, and a section over the limit is an error at bake time, not a surprise at play time. `fizz-pack` has the default as `SECTION_LIMIT` and `PackView::check_limit`. The largest early-experiment section, the earlier `I3` layout of sunrise-psy for OPL2 (measured with that layout, to be re-measured on OPX), is 61,720 bytes decoded and about 11.9 KB stored, so every early-experiment song fits one section.
- **Segment sections.** For a very long song, past the limit or the `u16` write count, the song is stored as OPX segments (see [Audio sections](#audio-sections)): a prelude and loop segments decoded in order, double buffered from the main loop. This is kept as an option and is not routine. Both buffers are locked.
- Packs are read with a single `read` into a buffer whenever the file is small, and by seek and read for sections of a big file. A pack is never memory-mapped.

## The baker

`fizz-bake` is a Rust host tool (library and binary). It depends on `fizz-pack` (the writer behind its `write` feature), `fizz-ep2` (tile ids, object enums and the bake-time world build) and `fizz-sim`. The existing Bun generators stay until their content moves. The commands, as planned:

```text
fizz-bake fmt [--check] assets/levels/*.fizz
fizz-bake bake --target dos|web -o <file> <sources...>
fizz-bake check --target dos|web
```

**Sources.** Levels are written in the Fizz 2 dialect of the Level Studio script, committed as `assets/levels/*.fizz`: a `map` grid of letters, `at=X,Y` for exact centres and `chicken` as an object kind. The dialect's grammar, legend, lowering and error codes are specified in [`SCRIPT.md`](SCRIPT.md); `fizz-script` parses and prints it, and `fizz-bake fmt` writes its canonical form.

**The manifest.** `check` re-bakes in memory and compares with a committed text manifest, `assets/packs/<name>.<target>.manifest`, so reviewers see changes to the bytes without binaries in git. One line per field, in this order:

```text
pack SHARED
target dos
format 1
abi 1a2b3c4d
section LEVL 0 stored 1052 decoded 1904 codec 1 fnv_stored 0a1b2c3d fnv_decoded 4e5f6071 count 10
file length 3472 checksum 89abcdef
```

The numbers above are an example of the shape, not real values. `check` also decodes every section and compares its decoded FNV with the manifest, which proves the encode-decode round trip.

**Determinism.** The same sources produce the same bytes on any machine, any day.

- No timestamps, git hashes, user names, paths or host byte order in the output.
- No `HashMap` iteration order: maps are sorted or are `BTreeMap`s. The directory is sorted by tag, then by emission order, and sources are processed in sorted path order.
- Numbers are parsed with Rust's `str::parse::<f64>`, which is correctly rounded, so `11.7` in a source gives the same bits as the `11.7` literal in the `const` level.
- Zero padding, fixed header fields, and reserved fields written as zero.
- The deflate encoder is pinned (the crate version is locked in `Cargo.lock`) with fixed parameters. The format does not require two encoder versions to agree, but `check` compares the stored FNV with the manifest, so a new encoder version shows up as a reviewed manifest change.
- Line endings in sources are normalised to LF before anything is hashed.
- Errors carry `file:line:col` and a stable code. A coordinate outside the map is refused. A source that is not in canonical form fails `fmt --check`.

**Builds.** `tools/build-dos.sh` writes the DOS files into `dist/dos` and `tools/build-web.sh` writes the Web files into `dist/web`; `tools/check.sh` runs `fizz-bake fmt --check` and `fizz-bake check` for both targets once the baker exists.

## Proof that a baked level equals the `const` level

The hand-written slice level in `crates/fizz-ep2/src/level.rs` stays in the code permanently as the test oracle (owner decision, 2026-10-29). Two things need proving, because the state hash of `fizz-ep2` covers the switches, the snack count and which snacks are taken, and not the tiles, the snack positions or the kinds. A baked level with one wrong tile could still match the golden hash.

1. **Same digest.** `fizz_ep2::level::digest(&LevelSource)` is FNV-1a 64 (offset basis 0xcbf29ce484222325, prime 0x100000001b3) over, in this order and little-endian: `w`, `h` (as `u16`), the tile bytes, `floor_solid` (`u8`), the bits of the spawn x and y, the state count (`u8`) and each state's 8-byte record, the binding count (`u8`) and each binding's 16-byte record, then for each object in order `kind`, `sub`, `flags`, `aux` and the bits of `x`, `y` and `p` (the object record's own bytes), then the rule count (`u8`) and each rule's 48-byte record (as `RuleRec::to_bytes` writes it), then the string count (`u8`) and each string's 32-byte record. The name is not included. The digest of the slice is 0x1AC799A4B63D632D; `tools/levl2-slice.py` writes the slice's `LEVL` bytes (2,048 of them, FNV-1a 32 0x5D520A47) and this digest from the description in this page, independently of the Rust code. The function is `fizz_sim::StateHash` (same basis and prime), fed the fields in that order; the digest of `SLICE_SOURCE` is pinned as a literal in a test of `fizz-ep2`. The digest of the baked script equals the digest of `const SLICE`.
2. **Same starting world.** `level::world_from_view(view)` and `level::world()` give an equal `hash()`, and equal tiles, switches, snacks, chickens, keys and spawn. `level::world()` is `level::world_from(&SLICE_SOURCE)`; a test keeps the old hand-written calls as an oracle and compares the two worlds field by field.
3. **Same transcript everywhere.** A replay from the pack matches `crates/fizz-ep2/slice-replay.txt` byte for byte on native, Web and DOS (`tools/check-parity.sh`).

Construction order (keys, then snacks, then the chicken, with same-kind objects in file order) is part of the digest and has its own test. The digest is computed from the decoded `LEVL` bytes as well as from the source, so the codec is inside the proof.

## Packaging on a Fizz 1 floppy

Fizz 2 follows the rules of Fizz 1's release scripts, which [`PLATFORM.md`](PLATFORM.md#size-budget) records. The release produces two 1.44 MB images:

| Image | Contents | Reserve |
| --- | --- | --- |
| Plain | `FIZZ2.EXE`, the packs and the documentation files | At least 8 KB free |
| Bootable | A FreeDOS boot floppy (about 153 KB) with the same game files | At least 64 KB free for saves |

- **Saves are not shipped.** The bootable image has no `FIZZ2.SAV` or `FIZZ2.CFG`; the game writes them. Fizz 1's real need is about 4 KB: the save and the configuration in whole 512-byte clusters plus one more for the temporary file the safe write uses. The 64 KB reserve is generous on purpose, and Fizz 2's own save and config reserve is still to be confirmed.
- **The packs count.** The reserve is checked against the free space left after the executable, every pack and every music file, counting each file in whole clusters.
- **The build fails if the reserve is missing.** The packaging script reads the free space of the finished image and exits with an error when it is below the reserve for that image; it never trims a pack or drops a song to make room. A pack that no longer fits is a design decision.
- The tool that builds the two images (plain and FreeDOS) and checks the 64 KB reserve is a separate package after the loaders (owner decision, 2026-10-29). Until then the layout above is the requirement, not an implementation.

## Open points

The open points of `LEVL` version 2 are listed at the end of [its section](#levl).

Decided by the owner on 2026-10-29 and no longer open: the file names, the DOS search order (current directory, then the executable's directory), the pack stem in the header, the file checksum of `MUSIC.*` at start-up (not checked), tile property flags in the compatibility number, a separate number for audio files, the key-on rule in compaction, the 96 KB section limit and segment sections (formerly chunk sections), the effects layout, the `u16` write count, OPX as the audio payload (formerly the `I3` layout; owner decision, 2026-10-30), LZMA as reserved only, and the floppy image tooling as a later package.

These remain, each with the package that settles it:

1. **Re-measure the compression figures** under the key-on rule, and check the rule against an OPL emulator (the audio pack package, with the baker's encoder).
2. **The speaker record layout** of the speaker `FXPK` section (effects only; the audio pack package).
3. **Segment playback.** The `F1` segment extension is specified; the double-buffered player, and whether any shipped song needs it, are for the audio pack package.
4. **Sections not yet specified.** `SPRT`, `BDRP` and `MUSC`.
5. **The level digest helper** (`fizz_ep2::level::digest`, FNV-1a 64) exists in `fizz-ep2`; `fizz-pack` does not carry it, and the baker will use it.
6. **Floppy image tooling**: a separate package after the loaders.
