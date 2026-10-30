# OPX, the OPL register-stream format

OPX (file extension `.OPX`) is an open, standalone format for a stream of timed OPL2 or OPL3 register writes, or of PC speaker divisors. It takes IMF's simplicity (a write, then a wait) and VGM's structure (a header, a loop point, a count of commands, room to grow), and puts both on an exact timed grid. It is the one stream format that Lieutenant Fizz 2 uses for its music and sound effects, and it is specified here so that anyone else can read, write, play or convert it.

Everything below stands on its own. [SOUND.md](SOUND.md) and [MUSIC.md](MUSIC.md) say how Fizz 2 makes and plays streams, and [FAP.md](FAP.md#audio-sections) says how a pack stores them; none of them is needed to read an OPX file.

Status in one line: this is a draft, version 0.1, experimental. The engine's `OplStep` record exists and plays today; the format below, the `oplstream` crate and the text twin are proposals that nothing reads or writes yet (see [Status](#status)).

## Purpose and non-goals

OPX is a small, steppable, loopable, compressible stream for games and tools that drive an OPL chip (or the PC speaker) from a timer interrupt.

- **Small.** A write is three bytes and a short wait is one.
- **Steppable.** A player advances by commands and a tick counter, in place, without allocation, so a 560 Hz interrupt handler can run it.
- **Loopable.** A stream has one loop point, held in the header so a player can seek to it.
- **Compressible.** The bytes are regular and deflate well; a container may compress them (a `.FAP` pack does).
- **Exact.** Time is a whole number of ticks of a declared grid. Nothing is ever a fraction of a sample.

It does not replace the formats that exist, each of which has its own job:

- **DRO** (DOSBox Raw OPL) and **Rdos RAW** are capture formats for OPL playback.
- **VGM** is a general sound-chip log with 44,100 Hz sample timing and dozens of chips.
- **IMF** (id Software) is the original write-and-delay record array.

OPX can import all four exactly (see [Importing](#importing-from-other-formats)). It is not a capture format, does not log other chips, and does not describe instruments or notes: it is the register writes that a composer's tool has already lowered to the chip.

## The time model

A stream has a **tick grid**: the header's `grid_hz` says how many ticks make one second, and the stream's clock is a counter of whole ticks that starts at 0.

- Every wait is a whole number of ticks of the grid. There is no fractional tick and no sample count.
- The writes between two waits all happen at the same tick (see [Semantics](#semantics)).
- The file declares its own grid. A player that runs a grid it does not support slows down or refuses; it does not guess.
- **Import is exact at the source's own rate.** An IMF file is already a tick stream at 560, 700 or 280 Hz, so it becomes an OPX file with that `grid_hz` and the same waits. A DRO file has a 1 ms delay unit, so it is a 1000 Hz grid. A VGM file counts waits in samples at 44,100 Hz, so it is a 44,100 Hz grid. In each case the waits are copied, not rescaled.
- **Re-timing is a separate, explicit step.** Moving a stream onto another grid (for example a 44,100 Hz VGM import onto 560 Hz for the game) moves each write to the nearest tick, never reorders writes, can merge writes into one tick and can change how a song sounds. A tool that does it must say so, report how many writes moved and by how much (the largest and the total error in seconds), and never do it silently while importing or playing. The re-timed file is a new OPX file with the new `grid_hz`.

A small grid is a good thing for a game: 560 Hz is 1.79 ms per tick, and a 140 Hz speaker grid is every fourth tick of it.

## The header

The header is 32 bytes at the start of the file, little endian, and is the same for every chip.

| Offset | Size | Field | Meaning |
| --- | --- | --- | --- |
| 0 | 4 | `magic` | The bytes `O`, `P`, `X`, 0x1A (0x1A stops a DOS `type` command, as in many old formats) |
| 4 | 1 | `major` | Format major version (0) |
| 5 | 1 | `minor` | Format minor version (1) |
| 6 | 1 | `chip` | 0 OPL2, 1 OPL3, 2 PC speaker; other values are refused |
| 7 | 1 | reserved | Zero |
| 8 | 2 | `flags` | `u16`. Bit 0: the stream has an extras tier (it contains a `TIER 1` command). Bit 1: OPL3 stereo bits are authored (some write to a register 0xC0 to 0xC8 sets bit 4 or bit 5). Bits 2 to 15 are zero |
| 10 | 2 | reserved | Zero |
| 12 | 4 | `grid_hz` | `u32`: ticks per second; never zero |
| 16 | 4 | `total_ticks` | `u32`: the tick count at the `END` command, which is the sum of all waits |
| 20 | 4 | `loop_offset` | `u32`: byte offset, from the first byte of the command stream, of the `LOOP` command; 0xFFFFFFFF for a stream that plays once |
| 24 | 4 | `stream_len` | `u32`: length in bytes of the command stream, which follows the header and runs to the end of the file |
| 28 | 4 | `reserved_channels` | `u32`: a mask of the chip channels that the stream promises never to write (see [Reserved channels](#reserved-channels)); zero for none |

Why these sizes:

- **32 bytes** is a power of two that fits every field below with a few reserved bytes, keeps the command stream 32-byte aligned in a file, and is small enough to cost little in a pack that holds many short effects (and the header deflates well).
- **`grid_hz` is a `u32`** so that 44,100 and 48,000 fit with room beyond a `u16`'s 65,535, and so that no import has to refuse a rate.
- **`total_ticks` is a `u32`**: 2^32 ticks is 90 days at 560 Hz and 27 hours at 44,100 Hz, more than any song. It is stored so a player can show a length or size a buffer without decoding.
- **`loop_offset` is a byte offset, not a record index**, because commands have different lengths. It is relative to the command stream, not the file, so the same offset works whether or not the header is present. A player that wants to loop jumps to it without scanning.
- **`stream_len` is stored** so a container can hold the file, or a reader can check the file, without scanning for `END`. The file's length must equal 32 plus `stream_len` exactly.
- **`reserved_channels` is a `u32`** because an OPL3 has 18 channels (0 to 8 in bank 0, 9 to 17 in bank 1), which does not fit in 16 bits.
- **There is no record count.** The command count is not needed to play, and a count would only be another field to keep in step with the bytes.

The version is two numbers, major and minor (see [Versioning](#versioning)). A reader refuses a file whose magic is wrong, whose major version it does not know, whose `chip` it does not know, whose reserved bytes are not zero or whose length does not match.

### Reserved channels

`reserved_channels` bit `n` means "this stream never writes any register that belongs to channel `n`", so a host may use that channel for something else (a sound effect) while the stream plays. Channel `n` is `bank * 9 + c`, where `c` (0 to 8) is the channel within the bank. The registers that belong to a channel are:

- 0xA0 to 0xA8, 0xB0 to 0xB8 and 0xC0 to 0xC8: channel `c`, taken from the low nibble.
- The operator registers 0x20 to 0x35, 0x40 to 0x55, 0x60 to 0x75, 0x80 to 0x95 and 0xE0 to 0xF5: take the low five bits `o`; if `o` mod 8 is 6 or 7 the register is unused and belongs to no channel; otherwise `c = (o div 8) * 3 + (o mod 8) mod 3`.

The registers 0x01 to 0x08, 0xBD (rhythm and key-on of the percussion), 0x104 and 0x105 are global and belong to no channel. A writer sets the mask from what the stream really writes (it may set a bit for a channel it merely leaves free, so the mask is a promise, not a measurement). A validating reader refuses a stream that writes a channel its mask reserves. A player that does not need the promise may skip the check.

## The command stream

The command stream is a sequence of commands. Each starts with a one-byte **opcode**. The stream is read from its first byte until an `END` command.

| Opcode | Name | Bytes | Meaning |
| --- | --- | --- | --- |
| `00` | `END` | `00` | End of the stream. Must be the last byte |
| `01` | `W0` | `01 reg val` | Write `val` to register `reg` of bank 0 (registers 0x000 to 0x0FF) |
| `02` | `W1` | `02 reg val` | Write `val` to register `reg` of bank 1 (0x100 to 0x1FF); `chip` must be OPL3 |
| `03`, `04` | | | Reserved |
| `05` | `TIER` | `05 t` | Set the tier of the writes that follow: 0 core, 1 extras. Extras are skippable (see [Tiers](#tiers)) |
| `06` | `LWAIT` | `06 lo hi` | Long wait: advance the clock by the `u16` count, 1 to 65,535 ticks (0 is invalid) |
| `07` | | | Reserved |
| `08` | `LOOP` | `08` | The loop point; a player that loops jumps to the byte after it. At most one per stream |
| `09` | `MARK` | `09 id` | A marker with an 8-bit `id`; no effect on the chip; reported to the host (for example to sync a visual) |
| `0A` | `SPK` | `0A lo hi` | Set the PC speaker divisor to the `u16`; 0 means gate off. `chip` must be PC speaker |
| `0B` to `7F` | | | Reserved |
| `80` to `EF` | `WAIT` | one byte | Short wait: advance the clock by `(opcode - 0x80) + 1` ticks, so 1 to 112 |
| `F0` to `FF` | extension | `Fx len payload` | `len` is one byte (0 to 255) giving the payload length; a reader that does not know the opcode skips `2 + len` bytes |

The short wait covers `80+n` for `n` from 0 to 111, not to 127 as a plain `80+n` scheme would: the values 0xF0 to 0xFF belong to extension commands, and a stream cannot have both.

Rules for the reader:

- An unknown opcode that is not an extension (`F0` to `FF`) is an error and the file is refused. The reserved opcodes are unknown opcodes until a version defines them. This is deliberate: a new core opcode changes the meaning of the stream, so an old reader must not guess.
- An unknown extension opcode is skipped by its length. An extension never changes what the chip is told or how long the stream takes; if a feature needs to, it needs a new core opcode and a new version.
- `W1` on an OPL2 stream, `SPK` on an OPL stream, and `W0`, `W1` or `TIER` on a speaker stream are errors. `MARK`, waits, `LOOP`, extensions and `END` are valid on every chip.

### Semantics

- The clock `now` starts at 0 and is measured in ticks of the grid.
- **A write happens at the current tick.** A `W0`, `W1` or `SPK` command takes effect at `now`.
- **Waits advance time.** `WAIT` and `LWAIT` add their count to `now`. Nothing else changes `now`.
- **Several writes before a wait share a tick.** They are performed in stream order, which is the order they hit the chip. Two consecutive waits are the same as one wait of the sum.
- `END` happens at the current tick; the stream ends there. A wait before `END` is silence (or a held note) at the end, and is how a looping song gets its length. `total_ticks` is `now` at `END`.
- **Looping.** When a looping player reaches `END`, it continues with the command after `LOOP` and `now` continues to count (the loop starts a new pass of `total_ticks - ticks_at_loop` ticks). The register state of the chip is not reset. The tier is reset to 0 at the start and at each loop jump, so `LOOP` is only valid while the tier is 0.
- A `LOOP` must be followed by at least one tick before `END`, so that looping does not spin forever in no time.
- `MARK` is reported to the host at its tick and otherwise ignored.

### Tiers

`TIER 0` is **core** and `TIER 1` is **extras**. The tier is 0 at the start of a stream and changes only at a `TIER` command. A write in the extras tier is one that a player may skip; waits, markers, `LOOP` and `END` are never skipped, so skipping extras never changes the timing.

- A player on a chip that cannot use extras (an OPL2 playing an OPL3 stream) skips the extras. A player may also skip them on any chip as a quality setting.
- A stream that has any `TIER 1` sets flag bit 0, and then every core write must be to bank 0, so that the core alone plays on an OPL2. A `TIER 1` in a stream whose flag bit 0 is clear is an error.
- The first intended use is the Fizz profile's idea of one OPL3 song whose bank 0 is a complete arrangement and whose extras (doubled lines, widening) are in bank 1 (see [Profiles](#profiles)). The tier is general: it does not require extras to be in bank 1.

### Validity

A reader (or a validator) refuses a file for any of these:

- The magic is wrong; the major version is unknown; `chip` is unknown; a reserved byte or flag bit is not zero (for a version the reader knows); `grid_hz` is zero.
- The file is shorter than 32 plus `stream_len` (truncated) or longer (trailing bytes).
- A command runs past the end of the stream, or the stream does not end with `END` as its final byte, or there are bytes after the first `END`.
- An unknown non-extension opcode; a `W1` on an OPL2; a speaker command on an OPL chip or the reverse; an `LWAIT` of 0; a `TIER` value above 1.
- The sum of the waits (the clock at `END`) differs from `total_ticks`, or exceeds 4,294,967,295 (a **wait overrun**).
- More than one `LOOP`; `loop_offset` not equal to the offset of the `LOOP` command (or not 0xFFFFFFFF when there is none); a `LOOP` while the tier is 1; no tick between `LOOP` and `END`.
- Flag bit 0 differing from "the stream contains a `TIER 1`"; a `TIER 1` with a core write to bank 1 (flag bit 0 set); flag bit 1 differing from "some write to 0xC0 to 0xC8 sets bit 4 or 5".
- A write to a channel that `reserved_channels` reserves.

A writer always writes the shortest wait form and splits a wait longer than 65,535 ticks into several `LWAIT`s. A reader accepts any valid split.

### Equivalence with the runtime `OplStep` record

Fizz 2's engine steps a 4-byte record, `fizz_sfx::OplStep`: `reg` (`u8`), `val` (`u8`) and `bank_wait` (`u16`, little endian) where bit 15 is the bank and bits 0 to 14 are the wait in ticks after the write (at most `MAX_WAIT`, 0x7FFF). The two forms hold the same information and the engine can step either.

- **`OplStep` list to OPX.** For each record emit `W0` or `W1` and then the wait if it is not zero. This is exact.
- **OPX to `OplStep` list.** For each write, emit one record and add the waits that follow it to that record's wait. A wait over 0x7FFF is split by inserting a neutral write (alternating values 1 and 0 to register 0xFF of bank 0, which is unused on the OPL2 and the OPL3) that carries up to 0x7FFF more ticks. A wait at the start, before the first write, is carried by the same neutral write. The neutral register is a proposal that has not been tried on hardware. `MARK`, `LOOP`, tiers and extensions have no `OplStep` form: the runtime keeps the loop point beside the list, and a tiered stream is lowered to one list per device.
- **Stepping either.** An OPX stepper keeps `now` and the tick it is due until; each tick it runs commands until the next wait takes it past the current tick. An `OplStep` stepper pops a record, writes it and sets the due tick to the record's wait. Both give the same writes on the same ticks.

### Not IMF-compatible by bytes

OPX files are **not** IMF files and IMF players cannot play them. An IMF record is `reg`, `val` and a `u16` delay in four bytes; an OPX stream starts with a 32-byte header and has one-byte opcodes. The bytes `01 B0 31` mean a bank 0 write to B0 in OPX but are the start of an IMF record whose delay is the next two bytes. The mapping is exact only through conversion (below). This is the price of one-byte waits, banks and a loop point, and it is accepted.

### Importing from other formats

- **IMF Type-0.** The records start at byte 0 and are four bytes each: `reg`, `val`, `delay` (`u16` little endian, the wait after the write). A Type-1 file has a `u16` payload length first; drop it and any trailing tag bytes. For each record emit `W0 reg val`, then the delay as waits if it is not zero. Set `chip` to OPL2, `grid_hz` to the IMF rate the song was written for (560, 700 or 280 Hz; the IMF file does not say, so the caller does), `total_ticks` to the sum of the delays, and end with `END`. This is exact. The file's last delay becomes the trailing wait before `END`.
- **DRO.** A DRO file's delays are in milliseconds, so a 1000 Hz grid; its codemap and the OPL2, dual-OPL2 and OPL3 forms map onto `W0`, `W1` and the chip type. Dual-OPL2 has no OPX form (it is two chips) and is refused or split by the caller.
- **VGM.** The OPL2, OPL3 (YMF262) commands map to `W0` and `W1`; waits are in samples at 44,100 Hz, so a 44,100 Hz grid; the loop offset becomes `LOOP`. Commands for other chips are dropped and reported.
- **Re-timing.** Any of these can then be re-timed onto another grid, for example 560 Hz, only as a separate, reported step.

## Profiles

A profile is a set of extra rules for one use of the format. The format does not enforce a profile; a tool does, and says which one it checked.

### The Fizz profile

For Lieutenant Fizz 2's music and sound effects.

- **Grids.** OPL music and OPL effects: `grid_hz` 560. In the engine, OPL effects step at 560 Hz (`OPL_TICK_HZ` in `fizz-sfx`) and speaker divisor lists step at 140 Hz (`SPEAKER_TICK_HZ`). A speaker effect is a speaker stream with `grid_hz` 140: for each 140 Hz step that has a divisor, `SPK d` and a one-tick wait (equal consecutive divisors may be held with a longer wait), and the list's final 0 is `SPK 0`. The speaker's tone is 1,193,180 divided by the divisor.
- **At most 16 writes per tick** in music, because the 560 Hz interrupt has to finish in about 1.79 ms and 16 writes are about 0.7 ms on an ISA bus by estimate (not measured on a 486).
- **Never channels 5 and 14, and never registers 0x104 and 0x105, in music.** Channel 5 is the effects' default channel and the left of the OPL3 pair; channel 14 is the right (see [SOUND.md](SOUND.md#reserved-channels)). The audio service owns 0x104 and 0x105. A music stream sets `reserved_channels` to 0x00004020 (bits 5 and 14) on OPL3 and 0x00000020 on OPL2.
- **Key-on registers are never dropped by compaction.** Writes to 0xB0 to 0xB8 (either bank) and 0xBD are kept even when they repeat the held value, because rewriting them can have side effects on the chip. Every other register is last-write-wins: a write that sets a register to the value it already holds may be dropped and its wait added to the previous wait. A register's first write is always kept.
- **Stereo.** OPL3 stereo bits (0x10 and 0x20 in 0xC0 to 0xC8) are ignored by an OPL2, so a stereo-authored OPL3 stream played on an OPL2 sounds centred. Flag bit 1 says a stream authors them.
- **Core and extras (idea, not built).** The tier above is meant for one OPL3 stream whose core is a complete bank-0 arrangement and whose extras are in bank 1 so that an OPL2 plays the core only. M4.5's `opl3-core-extras` roadmap item is the experiment to measure whether it is worth the lowering. Until it is, OPL2 and OPL3 forms of a song are two separate streams.

## The text twin

The text twin is an optional line-based form of an OPX file, for diffs, tests and hand-written examples. It is exact: every OPX file has a twin, and the round trip below returns the same bytes for a file in **canonical form**.

### Grammar

The twin is UTF-8 or ASCII text, one item per line. A `;` starts a comment that runs to the end of the line. Blank lines and comments are ignored. Mnemonics are case-insensitive on input and upper-case (or the lower-case shown for header keys) on output. Fields are separated by one or more spaces or tabs.

```
file        = header-block, "stream", NEWLINE, command-line*
header-line = "OPX" major "." minor
            | "chip" ("opl2" | "opl3" | "spkr")
            | "grid" decimal
            | "reserved" ("none" | decimal+)
```

- The header block comes first. `OPX` and `chip` and `grid` are each required once; `reserved` is optional (default `none`) and lists channel numbers 0 to 31 in ascending order on output. The block ends at the line `stream`.
- `OPX major.minor` is the version of the file. The assembler refuses a version below the lowest one that can express the stream, and writes the version given.
- The header fields that follow from the stream are not in the twin: `flags`, `total_ticks`, `loop_offset` and `stream_len` are derived by the assembler. A twin cannot disagree with its bytes about them.

Command lines, after the `stream` line:

| Line | Command | Notes |
| --- | --- | --- |
| `W0 rr vv` | `01 rr vv` | `rr` and `vv` are one or two hex digits, written as two upper-case digits on output |
| `W1 rr vv` | `02 rr vv` | As above |
| `T n` | waits totalling `n` ticks | `n` is decimal, 1 to 4,294,967,295 |
| `TIER t` | `05 t` | `t` is 0 or 1 |
| `LOOP` | `08` | |
| `MARK n` | `09 n` | `n` is decimal 0 to 255 |
| `SPK n` | `0A lo hi` | `n` is decimal 0 to 65,535; 0 is gate off |
| `X oo b b ...` | `oo len b b ...` | An extension command: `oo` is hex F0 to FF and the payload bytes are hex; `len` is their count |
| `END` | `00` | Required, last |

Waits are the one place where the twin and the bytes differ in granularity: one `T n` stands for a run of consecutive wait commands whose sum is `n`. On input, consecutive `T` lines add up.

### Canonical form

The assembler writes each run of waits in the shortest form: while the remaining count is above 65,535, an `LWAIT` of 65,535; then the remainder as one `WAIT` if it is 112 or fewer and otherwise one `LWAIT`. A file whose bytes are exactly what the assembler would write from its own twin is canonical. The disassembler writes the twin in canonical text: the header lines in the order `OPX`, `chip`, `grid`, `reserved`; then `stream`; then one command per line with single spaces, two-digit upper-case hex, a `T` line before each command that comes later than the previous one (and before `END` if the stream waits), no comments and a final newline.

- Binary to twin to binary is byte-exact for a canonical file, and gives the canonical form of any valid file.
- Twin to binary to twin is exact for a canonical twin.
- A tool that round-trips (a test, `diff` in a pull request) compares twins; a tool that wants the canonical bytes of an arbitrary file runs the round trip once.

The examples below use the twin, and a throwaway reference decoder and assembler were used to check every hex block and every expected listing in this document against each other.

## Versioning

The format version is two numbers, major and minor, and is **0.1** now. Versions below 1.0 are experimental: nothing has tried the format in production, and the layout may change until something has.

- A reader refuses a major version it does not know.
- While the major version is 0, a reader accepts minors up to the highest it knows and refuses a higher one, because a 0.x change may break old readers. From 1.0 on, a higher minor of a known major is read, ignoring what it does not understand.
- A writer sets the lowest version that can express the stream, so an old reader keeps reading a stream that does not use a newer feature.
- This document is the only definition. Every change to the format is recorded in the change table below, with its date and reason.

### Extension opcodes

Extension commands (`F0` to `FF`) are how the format grows without breaking readers.

- A new extension opcode is allocated by a row in the table below, in the same change that documents its payload. Nothing outside this file allocates one.
- An extension carries data that a reader may ignore without changing what the chip is told or how long the stream takes (a comment, a title, a segment label, a hint). Anything that does change that is a new core opcode, taken from the reserved range, with a new version.
- A reader that does not know an extension skips it by its length. A reader that knows it may use it. A writer may drop an extension it does not understand when copying a file, unless it is told to preserve unknown data.
- The payload is at most 255 bytes. Longer data is split across several commands.

| Extension | Name | Payload | Meaning |
| --- | --- | --- | --- |
| `F0` | comment | UTF-8 text, 0 to 255 bytes | Human-readable text; ignored |
| `F1` | segment | `index` (`u8`), `count` (`u8`), `loop_segment` (`u8`) | This file is segment `index` of `count` of one longer stream; the chip state carries on from the previous segment. `loop_segment` is the index of the segment a looping player returns to after the last one, or 0xFF for none. Must be the first command. A segment's own `LOOP` and `loop_offset` are not used (0xFFFFFFFF); song-wide lengths are the sum of the segments' `total_ticks` |
| `F2` to `FF` | | | Unallocated |

| Version | Date | Change |
| --- | --- | --- |
| 0.1 | 2026-10-30 | First specification: the 32-byte header, the command set and the text twin. Nothing reads or writes it yet |

### Possible future extensions

Not specified, and not decided:

- **Patch macros.** `DEFPATCH` and `USEPATCH`: define a named group of register writes (an instrument) once and apply it with a base register, to shrink streams that set the same instrument many times. They change what a stream means, so they would be core opcodes and a new minor version.
- **A fused wait-and-write.** See [Open questions](#open-questions).
- **Tempo or grid change** within a stream, which would need a careful rule for the clock; today a stream has one grid.

## Conformance

The examples are complete files. Each is shown as annotated hex (the 32-byte header and then the command stream), the equivalent text twin, and the writes the file decodes to with their tick times. In the listings `@n` is the tick, and `[extras]` marks a write in the extras tier. A conforming reader produces exactly these listings from these bytes, and a conforming writer produces these bytes from the twin.

### Example 1, minimal

An OPL2 stream: enable waveforms, set a pitch, key on, wait 16 ticks, key off, wait 4, end. 560 Hz.

```
4F 50 58 1A         ; magic
00 01               ; version 0.1
00                  ; chip OPL2
00                  ; reserved
00 00               ; flags
00 00               ; reserved
30 02 00 00         ; grid_hz 560
14 00 00 00         ; total_ticks 20
FF FF FF FF         ; loop_offset none
0F 00 00 00         ; stream_len 15
00 00 00 00         ; reserved_channels none
01 01 20            ; W0 01 20
01 A0 41            ; W0 A0 41
01 B0 31            ; W0 B0 31, key on
8F                  ; wait 16
01 B0 11            ; W0 B0 11, key off
83                  ; wait 4
00                  ; END
```

Text twin:

```
OPX 0.1
chip opl2
grid 560
reserved none
stream
W0 01 20
W0 A0 41
W0 B0 31
T 16
W0 B0 11
T 4
END
```

Decoded:

```
@0 W0 01 20
@0 W0 A0 41
@0 W0 B0 31
@16 W0 B0 11
@20 END
```

### Example 2, with a loop

An OPL2 stream with a 4-tick prelude and a 32-tick loop. Channel 5 is reserved by the mask (0x20) and not used. The loop offset is 4: the `LOOP` command is the fifth byte of the stream.

```
4F 50 58 1A         ; magic
00 01               ; version 0.1
00                  ; chip OPL2
00                  ; reserved
00 00               ; flags
00 00               ; reserved
30 02 00 00         ; grid_hz 560
24 00 00 00         ; total_ticks 36
04 00 00 00         ; loop_offset 4
0E 00 00 00         ; stream_len 14
20 00 00 00         ; reserved_channels: channel 5
01 01 20            ; offset 0: W0 01 20
83                  ; offset 3: wait 4
08                  ; offset 4: LOOP
01 B0 31            ; offset 5: key on
8F                  ; offset 8: wait 16
01 B0 11            ; offset 9: key off
8F                  ; offset 12: wait 16
00                  ; offset 13: END
```

Text twin:

```
OPX 0.1
chip opl2
grid 560
reserved 5
stream
W0 01 20
T 4
LOOP
W0 B0 31
T 16
W0 B0 11
T 16
END
```

Decoded (a looping player returns to the `LOOP` at tick 36 and repeats from `@4`):

```
@0 W0 01 20
@4 LOOP
@4 W0 B0 31
@20 W0 B0 11
@36 END
```

### Example 3, two banks, tiers, a marker and stereo bits

An OPL3 stream. Core writes are bank 0; the extras (bank 1) double the note. Flag bits 0 and 1 are set (extras present, stereo authored by the 0x30 in the C0 writes). An OPL2 player skips the `[extras]` writes and plays the rest at the same ticks.

```
4F 50 58 1A         ; magic
00 01               ; version 0.1
01                  ; chip OPL3
00                  ; reserved
03 00               ; flags: extras, stereo
00 00               ; reserved
30 02 00 00         ; grid_hz 560
10 00 00 00         ; total_ticks 16
FF FF FF FF         ; loop_offset none
1E 00 00 00         ; stream_len 30
00 00 00 00         ; reserved_channels none
01 C0 31            ; W0 C0 31, both outputs
01 B0 31            ; W0 B0 31, key on
05 01               ; TIER 1
02 C0 31            ; W1 C0 31
02 B0 31            ; W1 B0 31
05 00               ; TIER 0
8F                  ; wait 16
09 07               ; MARK 7
05 01               ; TIER 1
02 B0 11            ; W1 B0 11, key off
05 00               ; TIER 0
01 B0 11            ; W0 B0 11, key off
00                  ; END
```

Text twin:

```
OPX 0.1
chip opl3
grid 560
reserved none
stream
W0 C0 31
W0 B0 31
TIER 1
W1 C0 31
W1 B0 31
TIER 0
T 16
MARK 7
TIER 1
W1 B0 11
TIER 0
W0 B0 11
END
```

Decoded:

```
@0 W0 C0 31
@0 W0 B0 31
@0 TIER 1
@0 W1 C0 31 [extras]
@0 W1 B0 31 [extras]
@0 TIER 0
@16 MARK 7
@16 TIER 1
@16 W1 B0 11 [extras]
@16 TIER 0
@16 W0 B0 11
@16 END
```

### Example 4, PC speaker

A speaker stream at 140 Hz: a 1,193 divisor (about 1000 Hz) for 4 steps, silence for 2, a 607 divisor (about 1966 Hz) for 3, then gate off.

```
4F 50 58 1A         ; magic
00 01               ; version 0.1
02                  ; chip PC speaker
00                  ; reserved
00 00               ; flags
00 00               ; reserved
8C 00 00 00         ; grid_hz 140
09 00 00 00         ; total_ticks 9
FF FF FF FF         ; loop_offset none
10 00 00 00         ; stream_len 16
00 00 00 00         ; reserved_channels none
0A A9 04            ; SPK 1193
83                  ; wait 4
0A 00 00            ; SPK 0, gate off
81                  ; wait 2
0A 5F 02            ; SPK 607
82                  ; wait 3
0A 00 00            ; SPK 0
00                  ; END
```

Text twin:

```
OPX 0.1
chip spkr
grid 140
reserved none
stream
SPK 1193
T 4
SPK 0
T 2
SPK 607
T 3
SPK 0
END
```

Decoded:

```
@0 SPK 1193
@4 SPK 0
@6 SPK 607
@9 SPK 0
@9 END
```

### Example 5, an unknown extension

An OPL2 stream with an extension `F8` (unallocated: three payload bytes) and an empty `F9`. A reader that does not know them skips both and plays the same writes as it would without them. The twin and the decoded listing keep them, because they are part of the file.

```
4F 50 58 1A         ; magic
00 01               ; version 0.1
00                  ; chip OPL2
00                  ; reserved
00 00               ; flags
00 00               ; reserved
30 02 00 00         ; grid_hz 560
04 00 00 00         ; total_ticks 4
FF FF FF FF         ; loop_offset none
0F 00 00 00         ; stream_len 15
00 00 00 00         ; reserved_channels none
01 01 20            ; W0 01 20
F8 03 AA BB CC      ; extension F8, 3 bytes, skipped
F9 00               ; extension F9, 0 bytes, skipped
01 B0 31            ; W0 B0 31
83                  ; wait 4
00                  ; END
```

Text twin:

```
OPX 0.1
chip opl2
grid 560
reserved none
stream
W0 01 20
X F8 AA BB CC
X F9
W0 B0 31
T 4
END
```

Decoded (extensions are listed but have no effect):

```
@0 W0 01 20
@0 X F8 AA BB CC
@0 X F9
@0 W0 B0 31
@4 END
```

### Files a reader must refuse

Each of these is a change to a valid file above, and a conforming reader refuses it.

| # | Change | Why it is refused |
| --- | --- | --- |
| 1 | Example 1 with the first byte changed from `4F` to `51` | Bad magic |
| 2 | Example 1 with byte 4 (`major`) changed from `00` to `01` | Unknown major version |
| 3 | Example 1 with its last three bytes removed (44 bytes) | Truncated: the file is shorter than 32 plus `stream_len` |
| 4 | Example 1 with `stream_len` changed to `0E` and the final `00` removed (46 bytes) | Missing `END`: the stream ends without it |
| 5 | Example 1 with the first wait `8F` changed to `91` | Wait overrun: the waits sum to 22 ticks but `total_ticks` is 20 |
| 6 | Example 1 with `07` (a reserved opcode) in place of the first wait `8F` | Unknown non-extension opcode |
| 7 | Example 1 with the second `W0` (`01`) changed to `W1` (`02`) | Bank 1 write on an OPL2 stream |
| 8 | Example 2 with `loop_offset` changed from 4 to 5 | `loop_offset` does not point at the `LOOP` command |
| 9 | Example 3 with flags changed to `02 00` | `TIER 1` present without flag bit 0 |

## Pack integration

In a Fizz Asset Pack (`.FAP`, see [FAP.md](FAP.md#audio-sections)), each music section (`OPL3`, `OPL2`) and each effect stream in an `FXPK` effect set (OPL3, OPL2 and the speaker) holds a **complete OPX file**, header and command stream. The pack deflates the section as a whole. The same bytes, taken out of the pack, are a standalone `.OPX` file that any OPX reader plays, and a file written by a tool is packed unchanged. The pack's own header and directory carry only what the container needs (tag, count, checksum); nothing about the stream is duplicated outside the OPX header. For a very long song, the pack stores several OPX files as segments, using the `F1` extension (see [FAP.md](FAP.md#audio-sections)).

## The `oplstream` crate

The reference implementation is planned as one Rust crate, `oplstream`, in this repository. The name `opx` is already taken on crates.io by an unrelated 1Password tool (checked 2026-10-30 with `cargo search`); the name `oplstream` is to be re-checked before anything is published.

- **Core, `no_std`.** The header type with parse and write; a command **stepper** (an iterator-like decoder that yields commands and the clock, with no allocation) that a 560 Hz interrupt can drive; the **validator** for the rules above; and the **text twin** parser and printer.
- **Writer, with `alloc`.** A builder API that emits commands and the header: `write(bank, reg, val)`, `wait(ticks)` (merging and splitting waits into the shortest form), `tier`, `loop_point`, `marker`, `speaker`, `extension`, `end`, and a `finish` that fills in the derived header fields (`flags`, `total_ticks`, `loop_offset`, `stream_len`) and the lowest version.
- **`std` feature.** Importers and exporters for IMF, DRO and VGM, and the explicit, reported **re-timing** step.
- **Users.** `fizz-sfx`, `fizz-music` and `fizz-pack` depend on it, and `fizz-bake` writes with it. Today they have their own `OplStep` handling; moving them is a follow-up.
- **Publishing.** Possible later, under the repository's licences.

## Licence

This specification follows the repository's documentation terms. Reference code (the `oplstream` crate and its tools) lands under the repository's dual licence, Apache-2.0 or MIT, with the usual headers. No new licence is introduced.

## Status

What exists today:

- The engine's runtime layout: `fizz_sfx::OplStep`, a 4-byte record with the bank in bit 15 of the wait, used by `fizz-sfx` for effects and by `fizz-music` for music, and the steppers `OplPlayer`, `OplPair`, `SpeakerPlayer` and `MusicPlayer`.
- The earlier compact record layout, `I3`, in an earlier draft of [FAP.md](FAP.md#audio-sections). It is folded into OPX and is no longer a separate layout. Its measurements are kept in FAP.md with that caveat.
- id's IMF, DRO, Rdos RAW and VGM, which are other people's formats.

What is only proposed:

- OPX itself (this document, version 0.1), the extension `.OPX`, and its use as the pack's audio payload.
- The `oplstream` crate and its importers, exporters and re-timing step.
- The text twin.
- The tier idea for one OPL3 stream with skippable extras. The format has the tier command; the lowering that uses it is not built.

What is open:

- **A fused wait-and-write.** Whether writes should also be able to carry a short wait (one byte for write and wait together) as IMF's records do. Most writes in a music stream are followed by a wait of 0 or 1 tick, so it might save space, at the cost of more opcodes. It needs to be measured on real streams once the crate can write them.
- **Patch macros.** `DEFPATCH` and `USEPATCH`, as above.
- **A checksum in the header.** A pack already checksums its sections. A standalone file might want its own; it would use some of the reserved bytes or an extension.
- **A chip clock field.** The OPL's master clock (3.58 MHz on an AdLib, 14.318 MHz on some cards) decides the true pitch of a given register value. Today the stream assumes the usual clock. A field in the header would let a player correct it.
- **Dual-chip streams.** Two OPL2 chips (as a dual-OPL2 DRO) have no form.
- **The pack's reader limits.** The pack's per-section limit and record-count rules (FAP.md) were defined for the earlier layout and have to be re-checked for OPX.
- **Follow-up code, not part of this change.** `fizz-pack` (`AUDIO_HEADER_LEN`, the audio hash, the 16-byte audio header and the `chunk` and `chunks` fields); `fizz-sfx` and `fizz-music` (they would read OPX, and keep `OplStep` for the engine); `tools/music/generate.ts`, `tools/sfx/downmix.ts` and the baker (they would write OPX); and the new `oplstream` crate.
