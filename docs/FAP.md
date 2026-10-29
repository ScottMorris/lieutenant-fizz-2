# The Fizz Asset Pack (`.FAP`)

This document specifies the Fizz Asset Pack, the file format that `fizz-bake` writes and the Web and DOS builds read. It is the design for Milestone 2. No pack, reader or baker exists yet, so everything here is a plan with measured inputs, not a description of code. [`ARCHITECTURE.md`](ARCHITECTURE.md#assets-and-packs) says why packs exist and what stays in the executable; [`PLATFORM.md`](PLATFORM.md#size-budget) holds the size budget.

Sentences marked **Not yet specified** name a gap on purpose. Sentences marked **Open** are choices the owner has not made.

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
| `SHARED.FAP` | Ben, the HUD, the font extras, the sound effects (for all three devices) and the song table (`MUSC`) | At start-up, after the executable |
| `<WORLD>.FAP`, for example `LIGHT.FAP` | The world's levels (`LEVL`), backdrops (`BDRP`) and world sprites (`SPRT`) | When a level of that world starts |
| `MUSIC.OP3` | One `OPL3` section per song | The one section for the song in play, when the song changes |
| `MUSIC.OP2` | One `OPL2` section per song | As above, on an OPL2 machine |
| `MUSIC.SPK` | One `SPKR` section per song | As above, on a PC speaker machine |

All files use the container below (the same magic, header and directory), whatever their extension. The extension is a hint for people and for the build scripts; the reader trusts the header.

- A **pack** is any such file. The reader asks for a section by tag and index and does not care which file holds it. The hybrid split above is the current plan (owner decision, 2026-10-29); moving a section to another file is a change to `fizz-bake`'s manifest, not to the format.
- Song `n` is the `n`th section of its tag in each of the three music files, and the `n`th record of `MUSC` in `SHARED.FAP`. The baker refuses a music set where the three device files hold different song lists (a missing form is a placeholder, see [Target profiles](#target-profiles)).
- The Web build fetches the same names from its own `dist/web` directory, but has no `MUSIC.*` files.
- **Open:** the music file names (`MUSIC.OP3`, `MUSIC.OP2`, `MUSIC.SPK`) and the world file stems. They are proposals.

## Container layout

Everything is little-endian. A pack starts with a 32-byte header, followed by the directory, followed by the section data. Section data starts on a 16-byte boundary, with zero padding between sections and after the last one, so the file length is a multiple of 16.

### Header

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Magic: bytes `46 41 50 1A` (`FAP` then 0x1A, which stops `TYPE` from printing the binary) |
| 4 | 2 | Format version, `u16`. This document is version 1 |
| 6 | 2 | Header size, `u16`: 32 in version 1. A reader that sees a larger value skips to that offset for the directory |
| 8 | 1 | Target, `u8`: 1 Web, 2 DOS |
| 9 | 1 | Flags, `u8`: 0 in version 1; a reader rejects a pack with an unknown bit set |
| 10 | 2 | Section count, `u16` |
| 12 | 4 | Compatibility number, `u32` (see [The compatibility number](#the-compatibility-number)) |
| 16 | 4 | Total length of the file in bytes, `u32` |
| 20 | 4 | Checksum, `u32` (see [Checksums](#checksums)) |
| 24 | 8 | Pack stem: the file name without its extension, ASCII, space-padded to 8 bytes (for example `SHARED  `) |

The stem lets a reader tell that `LIGHT.FAP` was not renamed over `SHARED.FAP`. A reader compares it with the stem it asked for and treats a difference as a corrupt pack. For `MUSIC.OP3` the stem is `MUSIC   `. **Open:** whether the stem is worth a field; the alternative is 8 reserved zero bytes.

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

- Entries are sorted by tag (bytewise), and entries with the same tag are in the order the baker emitted them. The index of a section is its position among the entries with the same tag, counted from 0.
- Offsets rise strictly, no section overlaps another or the directory, and every `offset + stored length` is at most the total length.
- The total length in the header equals the file's real length.
- Codec 0 (stored) needs stored length equal to decoded length. Any other codec needs the codec to be one the reader knows.
- A section whose tag the reader does not know is skipped, not an error, so an older reader can read a newer pack's known sections. The compatibility number still has to match, which in practice makes new tags arrive together with a new number.
- A section with a decoded length of 0 and a count of 0 is a **placeholder**: present in the directory and empty. The reader reports it as "absent" to its caller, which decides whether that is an error.

## Checksums

- **File checksum (header):** FNV-1a 32 over the whole file, with the four checksum bytes read as zero. It covers stored (compressed) bytes, the header, the directory and the padding. It catches a truncated or damaged file as a whole.
- **Section checksum (directory):** FNV-1a 32 over the stored bytes of one section. A reader that loads one section from a large file (a song from `MUSIC.OP3`) checks this one and does not read the whole file.
- **Decoded checksum (manifest only):** FNV-1a 32 over the decoded bytes of a section. It is not in the file, because the decoder is bounded by the decoded length and a bad stream is rejected on its own terms (see [Codec](#codec)). It is in the text manifest, so `fizz-bake check` proves that decoding gives back what the baker meant and a reviewer sees any change in decoded content.
- FNV-1a 32 uses the offset basis 0x811c9dc5 and the prime 0x01000193, as Fizz 1's level fingerprint does.

A DOS reader that loads a whole small pack (a world pack of a few kilobytes) checks the file checksum. A reader that loads a section from a big file checks the section checksum only; the file checksum is then checked by the build (`fizz-bake check`) and by `tools/check-dos.sh`, not at play time. **Open:** whether to check the file checksum of `MUSIC.*` at start-up, which costs a read of the whole file from a floppy.

## The compatibility number

The header's `u32` says which schema the pack was baked for. The executable refuses a pack whose number differs from its own. It is not the build version: tying a pack to the git build would invalidate every pack on every commit and break `fizz-bake check` and the committed manifests.

**What it is computed from.** FNV-1a 32 over a canonical schema text, which lists in a fixed order:

1. The format version, the header size and the directory entry size.
2. For each section tag the format knows: the tag, its section version and its payload layout (field names, widths and order), written out as text next to the code that reads it.
3. The codec ids.
4. The numeric values of the identifiers that baked data refers to: every tile id by name and value, every object kind and every snack kind by name and value, and any other enum that a record stores as a number.

**Where it lives.** `fizz-pack` holds the text for items 1 to 3 and a `const` hash of it. `fizz-ep2` holds the text for item 4 (it owns the tiles and objects) and a `const` hash. The compatibility number is a `const fn` in `fizz-ep2` that combines both hashes (FNV over the two little-endian `u32` values), because `fizz-ep2` is the one crate that sees both. The baker and the executable use the same `const`, so they cannot disagree. A unit test walks the real enum values and fails if the schema text does not list them, so changing a tile id without changing the text is a test failure, not a silent change.

**When it changes.**

- It changes when any listed layout, tag, section version, codec id or enum value changes, and so a pack baked before the change is rejected.
- It does not change with the content of a level, sprite or song, with the git commit, with the build version, or with the target. Adding an enum member means adding it to the text, which changes the number on purpose; the test above makes forgetting that a failure.
- **Open:** whether tile properties (the `SOLID`, `SWITCHED` and other flags in `tiles::props()`) belong in the schema text. They live in the executable, not in the pack, and a changed flag changes how a baked level plays without changing any byte of it. Including them costs a pack rebuild whenever a flag moves; leaving them out relies on `check-parity.sh` and the level digest to catch the change.

The audio files carry the same number as the packs, even though they use little of the schema. One number is simpler to check than several, and the cost is a rebuild of the music when a tile id moves. **Open:** whether audio files should have a number of their own.

## Codec

The codec byte names how a section's stored bytes become its decoded bytes.

| Codec | Name | State |
| --- | --- | --- |
| 0 | Stored: the decoded bytes, unchanged | Version 1 |
| 1 | Raw deflate (RFC 1951, no zlib or gzip wrapper), baked at level 9 | Version 1 |
| 2 | LZMA family | **Reserved.** Not specified; a reader rejects it as unknown |
| 3 to 255 | Unassigned | Rejected |

**Why deflate.** The experiment in the planning notes measured 9 lab songs for both chips, 1,274,402 bytes raw. Records compacted and laid out as `I3` (see [Audio sections](#audio-sections)), one section per device and raw deflate level 9 came to 147,136 bytes (11.5%), and Fizz 2's own Crater Fields to 5,091 bytes of 31,984 (15.9%). A decoder is about 1 KB of code. LZSS and LZ4 decode in a few dozen bytes but are about 24% bigger. The LZMA family is about 17% smaller than deflate (22.9 KB over the 9 songs and both chips, 1.6% of a floppy) for a 3 to 5 KB decoder and a probability model of 3.7 to 14 KB, so it stays a later option. Cycle costs are guesses, not measurements: about 0.1 to 0.25 s to inflate the largest section at 66 MHz.

**Rules.**

- **Baked by `fizz-bake`.** The encoder runs on the host only. The DOS and Web builds contain a decoder and no encoder.
- **Decoded once, at load, into a caller buffer** whose length is the entry's decoded length. The decoder is `no_std`, has no `alloc` and no `unsafe`, and writes only into that buffer. Matches read back from the output, so there is no separate 32 KB window.
- **Never in an interrupt.** Decoding happens in the main loop, before the section is used. The interrupt handlers only step data that is already decoded.
- **Bounded and strict.** The decoder rejects, with an error and without panicking: output that would pass the decoded length; a stream that ends before the decoded length is reached; a back-reference before the start of the output; an invalid Huffman code or block type; and stored bytes left over after the final block. Working tables are fixed-size arrays on the stack or in the caller's scratch, sized for the worst case.
- **Stored is allowed for any section.** The baker picks per section and stores it when the deflate result is not smaller than the decoded bytes. A small level (the slice's is about 1.9 KB) may well be stored.
- **No dictionary shared between sections.** Each section is compressed on its own. Solid compression of the OPL2 and OPL3 streams of a song would save 4.6% with deflate and costs a double decode, so it is not done.

## Sections

A section is a payload plus its directory entry. The tags in version 1:

| Tag | Content | Version 1 payload |
| --- | --- | --- |
| `LEVL` | One level: tiles, objects, spawn | Fully specified |
| `OPL3` | One OPL3 register stream (a song or the effect set) | Fully specified |
| `OPL2` | The same for an OPL2 | Fully specified |
| `SPKR` | The same for the PC speaker | Outline; the record layout is not yet specified |
| `MUSC` | The song table: names and metadata | Outline; **Not yet specified** |
| `SPRT` | Sprites | Outline; **Not yet specified** |
| `BDRP` | Backdrops | Outline; **Not yet specified** |

All payloads are described as they are after decoding.

### `LEVL`

One section per level. All multi-byte fields are little-endian. All `f64` fields are the IEEE 754 binary64 bit pattern as eight bytes, so no text and no rounding sits between the source and the rules. The directory count is the object count.

**Fixed part, 32 bytes.**

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 2 | `w`, `u16`: width in tiles, at least 1 |
| 2 | 2 | `h`, `u16`: height in tiles, at least 1 |
| 4 | 4 | `switches`, `u32`: the initial switch bits of the `TileMap` (a set bit means that channel is on at the start) |
| 8 | 1 | `floor_solid`, `u8`: 0 or 1, the `TileMap` floor flag |
| 9 | 1 | `name_len`, `u8`: bytes in the name, at most 48 |
| 10 | 2 | `object_count`, `u16`; equals the directory count |
| 12 | 8 | `spawn_x`, `f64`: Ben's start, in tile units |
| 20 | 8 | `spawn_y`, `f64` |
| 28 | 4 | Reserved, zero |

**Name.** `name_len` bytes of printable ASCII (0x20 to 0x7E), zero-padded to a multiple of 8, so the objects start on an 8-byte boundary relative to the section. The name is for banners and the manifest; the rules never read it.

**Objects.** `object_count` records of 32 bytes each, in construction order (see below).

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 1 | `kind`, `u8` |
| 1 | 1 | `sub`, `u8`: the snack kind for a snack; reserved (zero) for other kinds until an enemy or gadget needs it |
| 2 | 2 | `flags`, `u16`: zero in version 1 |
| 4 | 4 | `channel`, `u32`: the switch channel for a switched object; zero otherwise |
| 8 | 8 | `x`, `f64`: the centre, in tile units |
| 16 | 8 | `y`, `f64` |
| 24 | 8 | `p`, `f64`: a parameter; for the chicken its direction (-1.0 or 1.0), otherwise 0.0 |

| `kind` | Meaning |
| --- | --- |
| 0 | Invalid; rejected |
| 1 | Red key |
| 2 | Blue key |
| 3 | Snack |
| 4 | Chicken |
| 5 to 7 | Reserved for a gadget, a switch and a generic enemy |
| 8 to 255 | Unassigned; rejected |

The numeric values of `kind` and `sub` come from the enums in `fizz-ep2`, and they are in the compatibility number.

**Tiles.** After the objects, `w * h` bytes, one tile id each, in `TileMap` order: row-major, row 0 (the bottom) first. A reader copies them into the map with `copy_from_slice`. The tile ids are those of `fizz-ep2::tiles`. A tile id outside the table is a corrupt level.

**Length.** The decoded length is `32 + pad8(name_len) + 32 * object_count + w * h`. The reader checks that the decoded length equals it exactly. For the slice (32 by 48, eight objects, a 16-byte name) that is 1,840 bytes.

**Construction order.** The baker writes objects in the order the world is built: red key, blue key, snacks in source order, then the chicken. The reader applies them in record order. Objects of the same kind keep their source order, because the state hash indexes snacks and chickens by position.

**Reader checks.** Every `f64` is finite; spawn and object centres lie inside the map (`0 <= x < w`, `0 <= y < h`); `w * h` does not overflow and matches the remaining bytes; the reserved bytes and `flags` are zero; `kind` and `sub` are known.

**Same bytes on both targets.** The Web and DOS packs hold byte-identical `LEVL` payloads. The stored bytes may differ when a target picks another codec for a section, but the decoded bytes, and their manifest FNV, do not. A test in the baker bakes both targets and compares the decoded `LEVL` bytes.

### Audio sections

`OPL3`, `OPL2` and `SPKR` hold timed register writes for one song or for the effect set of one device. A device section is chosen at load, so a machine decodes and keeps one of them.

**Tick rates.** OPL streams run at 560 Hz and the speaker stream at 140 Hz, as `fizz-sfx` and `fizz-music` do today.

**Header, 16 bytes (all three tags).**

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | `loop_start`, `u32`: the index of the first record of the loop, or 0xFFFFFFFF for a stream that plays once (an effect) |
| 4 | 4 | `prelude_ticks`, `u32`: ticks from the first record to the record at `loop_start`; the whole stream's ticks for a stream that plays once |
| 8 | 4 | `loop_ticks`, `u32`: ticks from the record at `loop_start` round to it again; 0 for a stream that plays once |
| 12 | 2 | `rate`, `u16`: 560 or 140; the reader rejects another value for the tag |
| 14 | 2 | Reserved, zero |

The directory count is the number of records. The prelude (the one-time setup, instrument loads and the first notes) runs once; the player then jumps back to the record at `loop_start` and never replays the prelude. A song with no prelude has `loop_start` 0. The record count is a `u16`, so a stream has at most 65,535 records (the largest lab stream is 20,535 after compaction); the baker refuses a longer one, which then has to be split into several sections.

**Records, layout `I3`.** After the header, the records back to back: `reg` (`u8`), `val` (`u8`) and then `varint(wait << 1 | bank)`.

- The varint is little-endian base 128: seven bits per byte, low group first, the high bit set on every byte but the last. The value is below 2^16 (`wait` is at most 0x7FFF, which is `MAX_WAIT` in `fizz-sfx`), so it takes one to three bytes and a record is three to five bytes (3.02 bytes on average over the lab songs).
- `bank` is 0 or 1. On an `OPL2` section it is always 0; on `SPKR` there is no bank (see below).
- `wait` is the number of ticks to wait after this write, as `OplStep` does. The encoding must be the shortest one: a longer encoding of the same value is a corrupt stream.
- The decoded `I3` section is 568 KB over 9 lab songs against 751 KB as 4-byte records. A stepper reads `I3` directly and never expands it, so heap use equals the decoded length.

**Compaction (a separate, tested lowering step before the codec).** The baker drops a write that changes nothing and adds its wait to the previous kept record, so total ticks and the sequence of state-changing writes are unchanged.

- A write is redundant when the register (bank and register number) already holds that value, by last write wins. A register's first write is always kept, because the chip's state is unknown at the start.
- Writes to the key-on and rhythm registers (0xB0 to 0xB8 in both banks, and 0xBD) are never dropped, because rewriting them can have side effects on the chip. The experiment treated every register as last-write-wins and did not check this against an emulator, so its figures include 190 such drops over the lab streams; the rule here is the safer one and the numbers need re-measuring once the encoder exists.
- The baker refuses a stream where a summed wait would exceed 0x7FFF; the source tool must put a silent step in the middle.
- The baker refuses a stream with more than 16 writes in one tick, the cap the DOS audio service relies on. The nine lab songs all break it (34 to 129 writes in a tick), so they cannot be packed as they are; only Crater Fields complies. Lowering them is the job of the source tools, not of the baker.
- Dropping redundant writes matters more for decoded memory than for compressed size: it removes 26.3% of the lab records but gains deflate only 3.9 to 4.7%.

**`SPKR`.** The tick rate is 140 Hz and the payload is a list of speaker divisors. **Not yet specified:** the record layout (one `u16` divisor per record with a varint wait, or a divisor per tick), and whether the same header is used. The directory, codec and loop rules above apply unchanged.

**Effects.** The sound effects of `SHARED.FAP` use the same tags, one section per effect set and device, with `loop_start` 0xFFFFFFFF. **Not yet specified:** whether each effect is its own section or whether one section holds all fifteen with an index table. The first is simpler for the reader and wasteful for 100-byte streams; the answer needs a measurement.

**Per-device sections.** The baker emits all three device forms of every song; DOS seeks to the section for the detected device, decodes it and ignores the rest. The OPL2 and OPL3 forms of one song overlap in content, and that overlap costs floppy space only. If it matters, the planned single-stream OPL3 with a skippable second bank (M4.5 `opl3-core-extras`) removes the duplicate structurally, and would be a new section version.

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
| `OPL3`, `OPL2`, `SPKR` | **None.** The Web plays the Undertone sources bundled in the page | Yes, one section per device |
| Audio files `MUSIC.*` | None | Yes |
| Output directory | `dist/web` | `dist/dos` |

- **Missing forms.** If an asset has no form for a target, the baker writes a placeholder section (decoded length 0, count 0) and prints a warning, so Web iteration is never blocked. The DOS gate still reports the gap and the placeholder is never silently treated as content.
- **Web loading.** The page fetches the packs by name, so the browser cache and Tauri bundling work as for any file. Compression is decoded by the same Rust decoder that DOS uses, in WebAssembly, and not by HTTP content encoding, so the bytes the rules see are the same.
- **Parity.** The only content the rules read from a pack is `LEVL` (and, later, whatever gameplay data a section adds). It is identical on both targets, so the state hash is the same. Presentation sections may differ.

## Loading on DOS

The DOS build loads packs through the existing runtime calls (`File::open`, `len`, `read`, `seek_start`, `read_to_vec`), with `FailCritical` on for every read so a missing disk or an open drive door returns an error instead of printing "Abort, Retry, Fail?" over the screen.

**Where it looks.** For each pack name, the runtime tries the current directory first and the executable's directory second (from `env::program_path()`), and uses the first file it can open. It does not search `PATH` and does not fall back to another name.

**When.** `SHARED.FAP` loads before mode 13h is set, so any failure prints in text mode. A world pack loads when a level of that world starts, and a `MUSIC.*` section when the song changes. The first level loads from a world pack at start-up for the same reason as `SHARED.FAP`. Packs are never all loaded at once; on a real floppy that costs time and memory.

**Order of checks for a pack.**

1. Open the file, read the header, check the magic, format version, header size and flags.
2. Check the target is DOS and the stem is the one asked for.
3. Check the compatibility number against the executable's own.
4. Check the total length against the file's length; read the rest.
5. Check the file checksum (a whole pack) or the section checksum (one section of a large file).
6. Check the directory rules above.
7. Decode the sections that are needed, each into a buffer of its decoded length, and check any per-section rule (for a level, the length and object checks).

**Failure is a hard exit.** There is no fallback to a compiled-in level (the slice level in the code is a test oracle, not a runtime fallback). The program prints a message in text mode, restores its vectors through the normal clean-up list, and exits with status 1. If the failure happens while the screen is in mode 13h, the program first restores text mode. The messages, with `<N>` for a file name and numbers in decimal or hexadecimal as marked:

| Cause | Message |
| --- | --- |
| File not found | `FIZZ2: cannot find <N>. Put it in the same directory as FIZZ2.EXE.` |
| Read error or disk not ready | `FIZZ2: cannot read <N> (DOS error <code>).` |
| File too short or length mismatch | `FIZZ2: <N> is incomplete. Copy it again.` |
| Bad magic, version or stem | `FIZZ2: <N> is not a Fizz 2 data file.` |
| Wrong target | `FIZZ2: <N> was made for the Web version.` |
| Compatibility number differs | `FIZZ2: <N> does not match this FIZZ2.EXE (data <hex8>, program <hex8>). Use files from the same download.` |
| Checksum mismatch | `FIZZ2: <N> is damaged (checksum <hex8>, expected <hex8>). Copy it again.` |
| Directory rule broken, or a section fails to decode | `FIZZ2: <N> is damaged (section <TAG> <index>). Copy it again.` |
| Required section missing, or a placeholder where content is needed | `FIZZ2: <N> has no <TAG> <index>.` |
| A section does not fit in memory | `FIZZ2: not enough memory to load <TAG> <index> from <N> (needs <bytes> bytes, <bytes> free).` |

The exact wording is final only once `tools/check-dos.sh` has a negative case for each row. After a successful load the platform prints a line for each pack, `PACK <name> <bytes> <checksum>`, which the DOS gate checks with the rest of the `SIZE` and `HEAP` lines; `SIZE` counts the executable and the packs together.

## Memory limits

- **Free memory.** The `/PROBE` memory ladder reads about 96 KB free at `memsize = 2` (2 MB of RAM) and about 56 KB at `memsize = 1`, after the 256 KB heap and the executable. A decoded section, its stored bytes (while decoding) and the decoder's working tables must all fit.
- **Levels.** A level is small (the slice's decoded section is 1,840 bytes; a Fizz 1-sized level of 192 by 28 tiles is about 6 KB). The tiles are copied into the `TileMap`, so the section buffer is freed once the world is built.
- **Audio.** The largest lab section, the `I3` stream of sunrise-psy for OPL2, is 61,720 bytes decoded and about 11.9 KB stored, which together are about 74 KB: inside 96 KB, not inside 56 KB. The stored bytes can be freed as soon as the decode finishes, so what stays resident is the decoded length.
- **Rule.** The baker takes a per-section limit on decoded length (and checks decoded plus stored length against it). A section over the limit is an error at bake time, not a surprise at play time. **Open:** the default limit, which has to be chosen for the floor memory size. 48 KB is a starting guess for 1 MB machines and is not measured.
- **Later option: chunks.** A longer song is split into several sections (one per scene, each a stream with its own header), and the player decodes the next chunk while the current one plays, from the main loop. Another option is a decoder that fills a smaller buffer in pieces and is called again. **Not yet specified:** the chunk metadata that links the sections of one song.
- Packs are read with a single `read` into a buffer whenever the file is small, and by seek and read for sections of a big file. A pack is never memory-mapped.

## The baker

`fizz-bake` is a Rust host tool (library and binary). It depends on `fizz-pack` (the writer behind its `write` feature), `fizz-ep2` (tile ids, object enums and the bake-time world build) and `fizz-sim`. The existing Bun generators stay until their content moves. The commands, as planned:

```text
fizz-bake fmt [--check] assets/levels/*.fizz
fizz-bake bake --target dos|web -o <file> <sources...>
fizz-bake check --target dos|web
```

**Sources.** Levels are written in the Fizz 2 dialect of the Level Studio script, committed as `assets/levels/*.fizz`: a `map` grid of letters, `at=X,Y` for exact centres and `chicken` as an object kind. The dialect's grammar is **not yet specified** here; it belongs with the baker's package.

**The manifest.** `check` re-bakes in memory and compares with a committed text manifest, `assets/packs/<name>.<target>.manifest`, so reviewers see changes to the bytes without binaries in git. One line per field, in this order:

```text
pack SHARED
target dos
format 1
abi 1a2b3c4d
section LEVL 0 stored 1052 decoded 1840 codec 1 fnv_stored 0a1b2c3d fnv_decoded 4e5f6071 count 8
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

1. **Same digest.** `fizz_ep2::level::digest(&LevelSource)` is FNV-1a 64 (offset basis 0xcbf29ce484222325, prime 0x100000001b3) over, in this order and little-endian: `w`, `h` (as `u16`), the tile bytes, `switches` (`u32`), `floor_solid` (`u8`), the bits of the spawn x and y, then for each object in order `kind`, `sub`, `flags`, `channel` and the bits of `x`, `y` and `p`. The name is not included. The digest of the baked script equals the digest of `const SLICE`.
2. **Same starting world.** `world_from_pack(view)` and `level::world()` give an equal `hash()`, and equal tiles and snack data field by field.
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
- **Open:** the tool that builds the images (the floppy packaging lives with the DOS build) and the list of files on the plain image.

## Open points

These need an owner decision before or during the packages that implement them. None blocks the format's shape.

1. **File names.** `MUSIC.OP3`, `MUSIC.OP2`, `MUSIC.SPK` and world stems such as `LIGHT.FAP` are proposals.
2. **Search order.** The plan says the current directory, then the executable's directory. Whether a game started from another directory should ever look in the current one first is unconfirmed.
3. **Pack stem in the header.** Worth 8 bytes, or leave them reserved?
4. **File checksum of `MUSIC.*` at start-up.** Checking costs reading the whole file from a floppy; skipping it relies on section checksums and the build gate.
5. **Compatibility number inputs.** Whether tile property flags belong in the schema text, and whether audio files need a number of their own.
6. **Compaction and key-on registers.** The experiment's savings assume every register is last-write-wins; the spec keeps writes to 0xB0 to 0xB8 and 0xBD. Re-measure, and check the assumption against an emulator.
7. **Per-section decoded limit.** The default (48 KB is a guess) depends on the memory floor. At 56 KB free, the largest lab OPL2 stream does not fit with its stored bytes.
8. **Effects granularity.** One section per effect, or one section with an index table.
9. **Song length and chunks.** The record count is a `u16`, and long songs need either splitting into chunk sections or a wider count.
10. **Sections not yet specified.** `SPRT`, `BDRP`, `MUSC` and the `SPKR` record layout.
11. **LZMA.** Codec 2 is reserved; whether it is ever worth 22.9 KB (about 1.6% of a floppy) for a 3 to 5 KB decoder is a later measurement.
12. **Image building.** Which tool builds the two floppy images, and who owns the check of the reserve.
