# OPL streams

This is the reference for the OPL register-stream formats that Lieutenant Fizz 2 meets: id Software's IMF, the owner's lab stream, the runtime `OplStep` and the pack's compact encoding. It converges them on one model and names the wide form of that model **OPX**. Read [SOUND.md](SOUND.md) and [MUSIC.md](MUSIC.md) for how streams are made and played, and [FAP.md](FAP.md) for how they are stored in a pack.

Status in one line: the model, `OplStep` and the lab's 5-byte stream exist today; the name OPX, the lab adopting it and any OPX file reader or writer are proposals (see [Status](#status)).

## The model

Every format here is the same thing: an ordered list of write records, each one a chip register write followed by a wait.

| Field | Meaning |
| --- | --- |
| `bank` | 0 for registers 0x000 to 0x0FF, 1 for 0x100 to 0x1FF (the OPL3's second bank). An OPL2 has only bank 0 |
| `reg` | The register's low byte (`u8`) |
| `val` | The value written (`u8`) |
| `wait` | Ticks to wait after this write before the next record is due, 0 to 0x7FFF in the wide form below |

The first record is due at tick 0. A record is due at the previous record's due tick plus the previous record's `wait`, and several writes in the same tick are records with a wait of 0 between them. The last record's wait runs the stream out to its total length, so a looping song's silence at the end is part of the data. Order matters: records with the same due tick play in list order.

A stream has a tick rate that the format itself does not carry; the context fixes it.

| Stream | Tick rate | Notes |
| --- | --- | --- |
| Fizz 2 music, and OPL sound effects | 560 Hz | `OPL_TICK_HZ` in `fizz-sfx`. Fizz 1's DOS port also runs its IMF at 560 Hz |
| PC speaker effects | 140 Hz | Not register writes at all: one PIT divisor per step, every fourth tick of the 560 Hz handler. Not an OPL stream, so not covered by OPX |
| id's Wolfenstein 3-D IMF | 700 Hz | For completeness. The same record layout at a different rate; a converter has to rescale waits (or keep the rate beside the data) to play such a file in Fizz 2 |

Take care with the 140 Hz figure: it belongs to the speaker list. Every OPL effect in `fizz-sfx` is stepped at 560 Hz, like the music, so one OPX tick rate (560 Hz) covers both.

## The family

### IMF, Type-0 and Type-1

id Software's IMF files are the origin. A Type-1 file starts with a `u16` payload length in bytes, then 4-byte records: `reg`, `val` and a little-endian `u16` delay in ticks. The delay is the wait after the write, as in the model. A Type-0 file has no length: the records start at byte 0 and run to the end of the file. A reader tells the two apart by the first word, which is zero in a Type-0 file. A register is one byte, so IMF can reach only bank 0: it has no way to name 0x100 and up, and so no way to enable or use an OPL3's second bank. The delay uses all 16 bits.

Fizz 1's DOS port writes IMF Type-1 chunks at 560 Hz (`m4/sound/fizz-dos/sfx.ts` and the song converter in its `m4/sound` folder, see its `docs/DOS_ARCHITECTURE.md`). Its sound blob holds a `u32 offset, u32 length` table of such chunks.

### The lab's OPL3 stream

The owner's private IMF lab writes a stream of its own for OPL3 (`toOpl3` in its `src/fizz-dos/opl.ts`; it reads it back in `ImfRunner` in `player.ts`). The lab's README calls it the lab's own OPL3 stream, "not a format any game used". Layout, little endian:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Payload length in bytes: five times the record count (the header is not counted) |
| 4 | 5 each | `reg` (low byte), `bank` (0 or 1), `val`, `delay` (`u16`, ticks at 560 Hz, the wait after the write) |

Notes on the lab's stream, read from its writer, its reader and its data files:

- The reader takes only bit 0 of the bank byte. The delay uses all 16 bits, and the writer clamps a gap longer than 0xFFFF to 0xFFFF rather than splitting it, so a gap that long would be shortened. A converter should split such a gap.
- The stream is padded with a final wait so that its total ticks equal the song's length.
- The OPL3 streams begin with a write of 0x105 (the OPL3 enable) and then 0x104, which is why the lab's songs cannot be fed to the game unchanged: the audio service owns those two registers (see [OPL3 specifics](#opl3-specifics)). One lab data file shows it: its first records are 0x105 = 1, 0x104 = 0, then 0x01 = 0x20.
- The lab's `.bin` files use this 5-byte layout for both chips: an OPL2 song is also saved as 5-byte records with every bank byte zero, beside a separate `.imf` for the songs that fit IMF's 16-bit length. An OPL3 song never has an `.imf`.
- It has no loop point of its own. The lab loops a whole stream, and reports the loop length in seconds in its data report.

The lab's code is private and is not copied here. This repository never changes the lab.

### The runtime `OplStep`

`fizz_sfx::OplStep` is the runtime record, used by `fizz-sfx` for effects and by `fizz-music` for the music arrangements. It is 4 bytes: `reg` (`u8`), `val` (`u8`) and `bank_wait` (`u16`, little endian), where bit 15 is the bank and bits 0 to 14 are the wait. `MAX_WAIT` is 0x7FFF. `OplStep::new(bank, reg, val, wait)` asserts the bank is 0 or 1 and the wait is at most `MAX_WAIT`. A stream in `fizz-music` is a list of these with a loop point (`loop_start`, `prelude_ticks`, `loop_ticks`); the generator `tools/music/generate.ts` writes them and the player in `crates/fizz-music/src/player.rs` steps them.

### The pack's compact encoding, `I3`

The Fizz Asset Pack stores OPL streams in the audio section layout `I3`: a 16-byte header (loop point and tick counts, the rate) followed by records of `reg`, `val` and a varint of `wait << 1 | bank`, after a lossless compaction step, then deflated. It is a transform of the same model, covered in [Compact encoding](#compact-encoding-i3) below and in full in [FAP.md](FAP.md#audio-sections).

## OPX, the converged wide form

**OPX** (file extension `.OPX`) is the wide, byte-aligned form of the model. It has two layers: the header-less **OPX chunk** that packs and the runtime use, and the self-describing **`.OPX` file**, which is a 16-byte header followed by exactly one chunk. The chunk comes first below.

### The OPX chunk

Little endian:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | `length`, `u32`: the payload in bytes, four times the record count (the header is not counted) |
| 4 | 4 each | `reg` (`u8`), `val` (`u8`), `bank_wait` (`u16`): bit 15 selects the bank, bits 0 to 14 are the wait in ticks |

The record is byte-for-byte `OplStep` (on a little-endian machine, as both targets are). An OPX file is therefore a 4-byte length followed by an `OplStep` array, and the runtime can step the payload in place with no conversion. A chunk has no loop point or rate of its own: in a pack or in the runtime the rate is fixed by what the stream is for (560 Hz for music and OPL effects) and the loop point is carried beside it (see [Loop and prelude](#loop-and-prelude)). The record count is the length divided by four.

The waits are limited to 0x7FFF. A gap longer than that is split by the source tool with a harmless write in the middle (the baker refuses a stream whose summed wait would exceed the limit, see [FAP.md](FAP.md#audio-sections)).

### The `.OPX` file

An `.OPX` file is for interchange between the lab, Undertone, the tools and this repository, where a stream has to describe itself. It is a 16-byte header, little endian, then one OPX chunk. A file with its 16-byte header removed is a plain OPX chunk, and a chunk with a header put in front is an `.OPX` file.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Magic: the bytes `O`, `P`, `X`, 0x1A |
| 4 | 1 | Format major version |
| 5 | 1 | Format minor version |
| 6 | 2 | `flags`, `u16`: bit 0 set when any record is in bank 1 (the stream needs an OPL3); bit 1 set when the stream authors stereo output bits (0x10 and 0x20 in the C0 to C8 registers); the other bits are zero |
| 8 | 2 | `rate`, `u16`: the tick rate in Hz (560 for the game's streams, 700 for Wolfenstein-rate files, and so on); never zero |
| 10 | 4 | `loop_start`, `u32`: the index of the first record of the loop, or 0xFFFFFFFF for a stream that plays once |
| 14 | 2 | Reserved, zero |

The record count is the chunk's length divided by four, so the header does not repeat it. The prelude length is not stored: it is the sum of the waits of the records before `loop_start`, which a reader gets in one pass, and the loop length is the stream's total ticks minus the prelude. This is the same model as the `loop_start`, `prelude_ticks` and `loop_ticks` of [FAP.md](FAP.md#audio-sections), without the redundant fields. A reader checks the magic, refuses an unknown major version (see [Versioning](#versioning)), refuses a zero rate, a length that is not a multiple of four, a `loop_start` that is neither 0xFFFFFFFF nor below the record count, and a clear flag bit 0 on a stream that has a bank-1 record. A reader ignores a flag bit it does not know in a minor version it does not know, and refuses it in a version it does know.

The `.OPX` file is **proposed**, not built.

### How a standard IMF reader and an OPX reader differ

A standard IMF Type-1 reader and an OPX reader disagree in two places, and only two:

| | IMF Type-1 | OPX |
| --- | --- | --- |
| Length | `u16` | `u32` |
| Delay | `u16`, all 16 bits are the wait | `u16`, bit 15 is the bank and only 15 bits are the wait |

Everything else (the 4-byte record, the register and value bytes, the delay being the wait after the write, little endian) is the same. So:

- An OPX in which every record has bank 0, and whose length fits in a `u16`, has the IMF record array exactly. A Type-1 reader takes the low 16 bits as the length, which are right, but then starts its records two bytes early (on the upper half of the `u32`), so every record is misaligned by two bytes until the header is skipped. A reader that reads a `u32` length (as the lab's reader does for its own stream) reads it correctly. The bytes after the length are the same, and only the header width differs.
- An OPX with any bank-1 record is not readable by an IMF reader: bit 15 of the delay would be taken as a 32.8 second wait, and the register would hit bank 0 instead of bank 1.
- A genuine IMF file read by an OPX reader reads its first 16-bit length as a `u32` and gets the wrong number, so a file needs its extension (`.IMF` or `.OPX`) or a converter to tell them apart. An IMF delay of 0x8000 or more would read as bank 1 with a short wait.

### What a converter does

- **IMF Type-1 to OPX.** Write the payload length as a `u32`. Copy every record; if its delay is 0x8000 or more, split the wait into a first record carrying 0x7FFF and a no-op write for the rest (see above). No record gets bank 1. Rescale the waits if the source rate is not 560 Hz (700 Hz Wolfenstein files), and say so in the output name or metadata, because OPX does not store the rate.
- **IMF Type-0 to OPX.** The same, with the payload length being the file length.
- **OPX to IMF Type-1.** Refuse a stream with any bank-1 record. Otherwise write the length as a `u16` (refuse if it does not fit; a longer stream has to be split), and write the waits as they are.
- **Anything to the runtime.** An OPX payload is the runtime record array. The converter's only job is to check each record (the bank is a bit, so it cannot be wrong; the waits are 15 bits, so they cannot be wrong) and, for music, the rules below.

## Compact encoding, `I3`

The compact encoding is a lossless transform of the OPX model, used in the pack. The definition is in [FAP.md](FAP.md#audio-sections) and is not repeated here. What matters for this document:

- Same fields: `bank`, `reg`, `val` and `wait` (at most 0x7FFF), the `wait` and `bank` being packed in a varint (`wait << 1 | bank`), so a record is three to five bytes instead of four.
- Compaction before the codec is the only lossy-looking step, and it keeps the sound: a write that sets a register to the value it already holds is dropped and its wait is added to the previous kept record, so total ticks and the sequence of state-changing writes are unchanged. The writes to the key-on and rhythm registers (0xB0 to 0xB8 in both banks, and 0xBD) are never dropped. A register's first write is always kept.
- OPX to `I3` is therefore: optionally compact, then re-pack each record. `I3` back to OPX is: decode each record and write the four bytes. The compaction is not reversible, because the dropped writes are gone; every other part of the transform is.
- A stepper reads `I3` directly. The 4-byte form is the in-memory form for tools, the runtime tables in `fizz-sfx` and `fizz-music`, and the lab.

## OPL3 specifics

- **The bank bit.** Bank 1 holds the second register set (0x100 to 0x1FF): channels 9 to 17 and the registers 0x104 and 0x105. An OPL2 has no bank 1, so an OPL2 stream has no bank-1 record; the generator refuses one.
- **0x105 and 0x104.** Register 0x105 bit 0 turns the OPL3 mode on: without it the second bank and the output bits do nothing, and the chip behaves as an OPL2. Register 0x104 selects the four-operator channel pairs. The game's audio service writes both (0x105 = 1, then 0x104 = 0) once at start-up, before any stream plays, and music and effect streams must not write either of them; the generator refuses a write to 0x104 or 0x105. The lab's OPL3 streams do write them as their first records, so a converter that takes a lab stream into the game strips those two records, adding their waits to the next record.
- **Stereo output bits.** In the C0 to C8 registers of both banks, bit 4 (0x10) sends the channel to the left output and bit 5 (0x20) to the right; both bits set (0x30) is both, the centre. An OPL2 ignores these bits, so a stream that sets them plays centred on an OPL2: the same stream, authored for stereo, is safe to play on a mono chip. The OPL3 mode has to be on for them to take effect. The mono OPL2-format streams get the bits ORed on by the audio service; an OPL3-aware stream keeps the bits as authored (see [SOUND.md](SOUND.md#the-opl3-pair)).
- **Reserved channels 5 and 14.** Channel 5 (bank 0, slot 5) is the effects' default channel on every card and the left channel of the stereo pair; channel 14 (bank 1, slot 5) is the right channel of the pair. A music stream must never write either channel's registers, in either bank. SOUND.md has the list of registers ([Reserved channels](SOUND.md#reserved-channels)) and MUSIC.md the generator's check ([Channels](MUSIC.md#channels)). The lab's channel list needs channel 14 dropped to meet this.
- **The write cap.** A music stream has at most 16 writes in any one tick, because the 560 Hz interrupt has to finish in about 1.79 ms; 16 writes are about 0.7 ms on an ISA bus by estimate (not measured on a 486). The generator and the pack baker both refuse a stream that exceeds it. The lab's songs do exceed it (up to 129 in a tick) and need lowering first, which is the job of the source tools.
- **Key-on registers.** Writes to 0xB0 to 0xB8 (bank 0) and 0x1B0 to 0x1B8 (bank 1), and to 0xBD (rhythm and drums), are never dropped by compaction, even when the value is the one already held, because rewriting them can have side effects on the chip. Everything else is last-write-wins.
- **Core and extras (idea).** M4.5's `opl3-core-extras` is a planned idea to measure, not a commitment: one OPL3 stream where bank 0 is a complete arrangement (channel 5 left free) and bank 1 holds only extras (doubled lines, harmonies, widening), so that an OPL2 plays the bank-0 records and skips the bank-1 records, and an OPL3 plays all of them. The bank bit already exists in the model, so OPX needs no change for it; what it needs is a lowering that follows the rule and a pack section version. It is not built. See the roadmap item `opl3-core-extras` in `tools/progress-report/roadmap.json` and [FAP.md](FAP.md#audio-sections). Until then the OPL2 and OPL3 forms of a song are two separate streams.

## Loop and prelude

A music stream is a prelude followed by a loop: the one-time setup (waveform enable, rhythm mode, instrument loads and the first notes) plays once, and the player jumps back to the loop start and never replays the prelude. The OPX record array does not record the loop point. Today `fizz-music` keeps three numbers beside the table: `loop_start` (the index of the first record of the loop), `prelude_ticks` and `loop_ticks`. The pack stores the same three in its 16-byte header and adds the rate ([FAP.md](FAP.md#audio-sections)); a stream that plays once, as an effect does, has no loop point (`loop_start` 0xFFFFFFFF in the pack). A file reader or writer for OPX should keep the loop point in a sidecar or in the pack, not in the OPX stream, so the OPX bytes stay the runtime's bytes. MUSIC.md describes how the generator places the loop point ([The streams](MUSIC.md#the-streams)) and how the stepper handles it ([The stepper](MUSIC.md#the-stepper-built)).

## Conversions

| From | To | What changes |
| --- | --- | --- |
| IMF Type-1 | OPX | `u16` length becomes `u32`; a delay of 0x8000 or more is split; rescale if the source is not 560 Hz |
| IMF Type-0 | OPX | As above, and the length is the file length |
| Lab 5-byte stream | OPX | Mechanical. Fold the bank into bit 15 of the delay (`delay \| bank << 15`) and drop the bank byte; the `u32` length stays, but becomes four times the record count rather than five times. A delay of 0x8000 or more needs the split. For music, also strip the 0x104 and 0x105 records and refuse channels 5 and 14 |
| OPX | IMF Type-1 | Only a stream with no bank-1 record; `u16` length |
| OPX | `I3` | Compact, then pack each record as `reg`, `val`, `varint(wait << 1 \| bank)`, then deflate in the pack; done by the baker |
| `I3` | OPX | Decode each record; the dropped redundant writes are not restored |
| OPX chunk | `.OPX` file, and back | Add or remove the 16-byte header; set the rate, the loop start, the version and the flags from the stream (the flags are derived: any bank-1 record, any stereo bit) |
| OPX | Runtime | None: the payload is the `OplStep` array. `fizz-music` and `fizz-sfx` hold generated tables of the same bytes |

The lab's writer (`toOpl3`) and runner (`ImfRunner`) are the owner's private code. They would adopt OPX with a small change: the writer packs the bank into the delay's top bit and writes a four-byte record, and the runner reads the bank from bit 15 and masks the wait to 15 bits. The owner decides whether to make that change. Undertone's companion OPL package (upstream issue #25: `fromOpl` and `toOpl`) is the natural place for the format to live on the composing side, and it should target OPX for its register-stream output, so the lab, Undertone and the game share one form.

## Versioning

The format version is two numbers, major and minor, and is 0.1 now. Versions below 1.0 are experimental: the lab and Undertone have not tried OPX yet, and the layout may change until they have. Rules:

- A reader refuses a file whose major version it does not know. A higher minor version of a known major is read, ignoring what it does not understand (new flag bits, and bytes in the reserved field).
- A writer sets the lowest version that can express the stream, so that an old reader keeps reading a stream that does not use a new feature.
- While the major version is 0, a change to the layout can break old readers, and it increments the minor version. Every version change is recorded in the table below with a date and the reason, and the doc is the only definition of the format.
- The chunk, being header-less, has no version of its own. It is the chunk of the 0.1 layout (4-byte records, a `u32` length), and a pack records which version of the chunk it was baked with in its own compatibility number.

| Version | Date | Change |
| --- | --- | --- |
| 0.1 | 2026-10-29 | Proposed: the chunk and the 16-byte header above. Nothing reads or writes it yet |

## Beyond Fizz

OPX is a small, fixed-record format for OPL2 and OPL3 register streams, for use as a game runtime form and for interchange. Its records are four bytes, so a 560 Hz interrupt handler can step it in place with no decompression, no allocation and no parsing. It does not try to replace other OPL formats, each of which has its own job: DRO (DOSBox Raw OPL, with its own codemap and delay commands), Rdos RAW, VGM (a general chip log whose waits are counted in 44.1 kHz samples) or IMF itself (which OPX widens by one bit, for the second bank). Converters to and from those formats are possible future work and are not built; a VGM or DRO import would convert the waits to ticks at the stream's rate, which loses sub-tick timing, and would drop the writes for other chips.

## Name check

On 2026-10-29 the owner's web search found no OPL, AdLib or AdPlug format named OPX. The extension `.opx` is used on desktops by unrelated software (OrgPlus organization charts and FlipAlbum albums), and a listing exists for an "EPOC OPL eXtension" of unknown meaning. So there is no collision inside the OPL world that has been found. This was a search, not an exhaustive check, and it can be repeated.

## Licence

The owner owns the format. The spec text (this document) is under the same terms as the repository's documentation, and any reference reader or writer lands in the repository's crates under the repository's dual licence (Apache-2.0 or MIT) with the usual headers. No new licence is introduced.

## Status

What exists today:

- The model, in `OplStep` (`fizz-sfx`) and the music and effect tables that use it (`fizz-sfx`, `fizz-music`), generated by `tools/sfx/downmix.ts` and `tools/music/generate.ts`.
- The runtime steppers `OplPlayer`, `OplPair` and `MusicPlayer`.
- The lab's 5-byte stream, in the owner's private lab, with its writer and runner.
- id's IMF, as Fizz 1's DOS port uses it.
- The pack's `I3` encoding is specified in FAP.md; the pack format is a specification and is not built.

What this document proposes, and is not decided until the owner confirms it:

- The name OPX and the file extension `.OPX`, for the 4-byte-record, `u32`-length chunk and the 16-byte-header file around it (version 0.1).
- The lab adopting OPX in its writer and runner.
- Undertone's companion OPL package targeting OPX.

What is not built:

- An OPX chunk and `.OPX` file reader and writer (and the IMF and lab-stream converters in the table above). The recommended owner is the audio pack package: a function pair in `fizz-pack` (the library that reads and writes packs) or in `fizz-bake` (the tool that builds them), because the baker already has to read source streams, check the 16-write cap and compact them. A standalone converter in `tools/music/` is the alternative if it is wanted before the pack exists.
- The `opl3-core-extras` lowering and its pack section version.
- A check that a lab stream meets the game's rules (channels, registers, cap) before import. MUSIC.md's generator checks are the model for it.
