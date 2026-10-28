# Music

The slice's level music is a song source (Undertone patterns that a builder lays out as a full song), a script that lists every note, a generator that lowers the song to OPL2 and OPL3 register streams, and a Web player. The `fizz-music` crate steps the streams (the player below), but the DOS build does not play them yet. This document covers what exists (the source, the lowering, the checks, the stepper and the Web player) and marks the DOS audio service as planned.

## Placeholder status

The song is "Crater Fields", and it is Fizz 1's track used as a placeholder test song for Fizz 2's test level. It is not an official Fizz 2 track, and the owner will replace it. Swapping it is a data-only change: add the new track beside `assets/music/crater.ts` and point `MANIFEST.level` in `assets/music/songs.ts` at it. Everything song-specific below (counts, loop point, hashes, the note-parity manifest) is generated, so a new song changes data and no code. Nobody has listened to it as Fizz 2's music, and nobody has listened to anything built from it.

## The source (built)

```text
assets/music/types.ts     MusicPart, MusicSong, PartRole and PART_PAN (the stereo placement of each role)
assets/music/pattern.ts   reads a part's mini-notation into timed events through Undertone's own parser
assets/music/song.ts      the builder: plan (the form nearest 60 seconds), withDepth (quiet partner voices), songFor
assets/music/crater.ts    the track and its settings (trim 0.75, contrast 5, lift 2, arpeggio on)
assets/music/songs.ts     the manifest: { level: 'crater' }
assets/music/events.ts    the song as a flat, deterministic list of note events
tools/music/notes.ts      writes assets/music/notes/crater.txt, or checks it with --check
tools/music/*.ts          the lowering to OPL streams (below)
```

Crater Fields is A minor at 148 bpm: a lead, a bass, a square pad, and a kick, snare and hats. The builder lays it out as Intro 2 bars, A 8, B 8 (up five semitones, the pad arpeggiated), A 8 (the lead doubled an octave up), Breakdown 3, Build 1, A' 8 (up two semitones, arpeggio and octave) and a one-bar Fill, which is 39 bars and 63.24 seconds. The song loops from the end straight back to the start. The lead, bass, arpeggio and octave parts also get a quiet sine partner on the same note, panned the other way.

Everything in `assets/music/` is pure, so it runs in Bun and in the browser bundle. The only dependency is `@liminal-hq/undertone` 0.2.0, as pinned.

## Provenance

The code is the same author's, Apache-2.0 OR MIT in Fizz 1 and here. The track is `crater` in `episodes/episode-1/src/audio/patterns.ts` (lines 375-399) and its settings are in `songs.ts` (line 17); the builder is `packages/engine/src/music-songs.ts` and the roles and pans are `packages/engine/src/sound-field.ts`, all of ~/source/liminal-hq/lieutenant-fizz at commit `4bce036`. The note reader and the way sections are laid end to end are adapted from `m4/sound/fizz-sheet-events.ts` and `fizz-dos/song.ts` of the DOS port (ScottMorris/lieutenant-fizz-dos) at commit `221dd6c`, MIT there and dual-licensed here. The lowering (`tools/music/lower.ts`, `patches.ts`) is adapted from the port's `fizz-dos/music.ts` (`convertParts`) and `patches.ts` at the same commit. Each file's header names its source and what changed.

## Checks on the source

- `bun test assets/music` holds the builder to Fizz 1. `assets/music/fizz1-crater.fixture.json` is Fizz 1's own `songFor` output, saved read-only from that commit, and the test asserts the same tempo, sections, parts and notes. Other tests cover the planner, the depth partners, the role pans and the note events.
- `bun tools/music/notes.ts --check` regenerates `assets/music/notes/crater.txt` in memory and fails with the first differing line if the committed list is stale. Run `bun tools/music/notes.ts` to rewrite it. The list is every note of the song (96 ticks to a bar): tick, seconds, part, MIDI note or noise, length and velocity, with the event count of each part in its header.
- Both run in CI, and in `tools/check.sh` when Bun and `node_modules` are present.

The velocity column is a convenience: the part's gain after the song's trim, scaled so the bass's gain is 127. It is not a Fizz 1 value.

## Lowering to OPL streams (built)

```text
assets/music/*                        the song: note events on a 96-tick bar grid, partner layers included
   |  tools/music/lower.ts            lower(layout, 'opl2' | 'opl3'), with patches.ts (instruments, drums) and stereo.ts (output sides)
   |  tools/music/verify.ts           the refusal checks and the note-parity check
   |  tools/music/format.ts           the record bytes and the FNV-1a hash
   |  tools/music/generate.ts         writes, or --check compares:
   v
crates/fizz-music/data/crater-opl2.bin, crater-opl3.bin   4-byte records
crates/fizz-music/src/data.rs         generated: the song table, loop point, tick counts, note counts, dropped notes, hashes
crates/fizz-music/src/lib.rs          hand-written: the types, and the tables exposed through include_bytes!
crates/fizz-music/src/player.rs       hand-written: the stepper (MusicPlayer), Fnv, writes_for and count_loops
```

Run `bun tools/music/generate.ts` to rewrite the three files and `bun tools/music/generate.ts --check` to regenerate in memory and fail with a diff if a committed file is stale. Both run the checks first and refuse to write if any fails. `--check` runs in CI and in `tools/check.sh` next to the note-list check, and `bun test tools/music` holds the lowering to the numbers below.

### The streams

A stream is a list of 4-byte records in the layout of `fizz_sfx::OplStep`: the register's low byte, the value, then a little-endian 16-bit field whose top bit is the register bank (0 for registers 0x000 to 0x0FF, 1 for 0x100 to 0x1FF) and whose low 15 bits are the 560 Hz ticks to wait after the write. A stream is a prelude followed by the loop. The one-time setup plays once, before the loop point: the waveform enable, the rhythm-mode switch, the drum instruments, the drum channels' pitch and output bits, and the instrument of the first note of each channel. The player jumps back to the record `loop_start` at the end and never repeats the prelude. The tail of the loop carries whatever instrument or pitch the first notes of the next pass need that the last notes changed, and the generator replays two passes to prove that the second sets every note up as the first did.

| | OPL2 | OPL3 |
|---|---|---|
| Records | 2,961 | 5,035 |
| Bytes | 11,844 | 20,140 |
| Loop | 35,416 ticks (63.24 s) | 35,416 ticks |
| Prelude | 6 ticks, 89 records | 12 ticks, 187 records |
| Melodic notes played | 731 | 1,400 |
| Melodic notes dropped | 8 | 0 |
| Drum hits | 480 | 480 |
| Most writes in a tick | 16 | 16 |
| FNV-1a of one pass | `0x4ae16486` | `0xb31d1e26` |

The hash is the 32-bit FNV-1a over each record in order: its due tick (four bytes, little-endian, counted from the first record), then its bank, register and value. It is integer-only, so the DOS build can compute it in an interrupt handler, and the `fizz-music` test recomputes it in Rust from the committed bytes.

Why the sizes differ from the planner's estimates. The DOS port measured 3,866 records for its OPL2 song. This lowering writes a register only when the channel's value would change: it skips the pitch low byte and the level of a note that repeats the channel's last, where the port rewrites both for every note. It also plays the setup once instead of on every loop. That is why OPL2 is 2,961 records, and why OPL3 with all 661 partner notes is 20 KB rather than the 26 KB estimated. The tick count (35,416), the melodic notes (731 played plus 8 dropped is the port's 739) and the 480 drum hits are the port's own.

### Channels

| Channels | OPL2 | OPL3 |
|---|---|---|
| Melodic music | 0 to 4 | 0 to 4, 9 to 13, 15 to 17 (13 of them) |
| Rhythm-mode drums | 6 to 8 | 6 to 8 |
| Effects (never written by music) | 5 | 5 and 14 |

Channel 5 (bank 0) and channel 14 (bank 1, slot 5) are reserved for the cluck, which is the OPL3 stereo pair. The generator refuses any record whose register low byte is one of the 13 registers of those channels (0x2A, 0x2D, 0x4A, 0x4D, 0x6A, 0x6D, 0x8A, 0x8D, 0xEA, 0xED, 0xA5, 0xB5, 0xC5), in either bank, and any write to 0x104 or 0x105, which the audio service owns. The 4-operator pairs 2 and 5 and 11 and 14 contain the effects' channels, and they are never enabled because 0x104 is never written. It also refuses a write to a channel the arrangement does not own, and an OPL2 stream with any bank-1 write. The OPL3 arrangement assumes the service has set register 0x105 to 1, as it does when the music plays.

The OPL2 arrangement is the DOS port's allocator. Each note takes the channel its part used last, else an unused one, else any free one. When all five are sounding, the least important note's channel is taken if the new note matters more (lead, bass, counter, arpeggio, bell, then everything else, with pad and drone last). The 8 dropped notes are pad notes in the last A section; they are named in `data.rs`, and the manifest check removes them from what it expects. The OPL3 arrangement gives each voice a fixed channel instead. A voice that plays chords has one channel per simultaneous note, and a voice that is only in some sections may share a channel with one it never overlaps, so nothing is dropped. In this song the bass takes channel 0, its partner 1, the lead 2 and its partner 3, the arpeggio 4 and its partner 9, the second lead 10 and its partner 11, the pad 12 and 13, the closing pad 15 and 16, and the closing bass 17 (its partner shares channel 0, which has the same instrument). At most 10 melodic notes sound at once.

The rhythm-mode drums are the port's: the kick is the bass drum on channel 6, the snare and the hats share channel 7, and the chip's own noise makes the rest. The hats cannot be placed apart from the snare, because they share a channel.

### Stereo output bits by role (OPL3)

The OPL3 can send each channel to the left, the right or both, and nothing in between. The sides are the C0 to C8 output bits (0x10 left, 0x20 right, 0x30 both), written with each channel's feedback. They live in one table, `ROLE_OUTPUT` in `tools/music/stereo.ts`, for the owner to tune by ear:

| Role | The part's own voice | Its partner layer |
|---|---|---|
| bass, kick, snare, pad, drone | centre | centre |
| every rhythm-mode drum | centre (C6, C7 and C8 are written once in the prelude) | none |
| lead | centre | right |
| counter, bell | centre | left |
| arpeggio | left, right, left, right on each eighth of the bar | the other side of the main voice |

A partner's gain is half its part's (6 dB down), which the song builder sets, and its side mirrors Fizz 1's: the lead leans left there, so its partner goes right. The OPL2 arrangement has no partners and sets no output bits. These bits only reach the speakers if `fizz_machine::route` leaves them alone. Today it sets both bits on every bank-0 write to C0 to C8 in OPL3 mode, so the DOS package must teach it to pass an OPL3 music stream through untouched.

### Levels

A note's level is the port's: its gain against the loudest main melodic part in 0.75 dB steps, plus a trim of 6 steps (4.5 dB) on every music note. The OPL3 arrangement takes 3 more steps off (2.25 dB), because more channels sound at once. Both numbers are constants in `lower.ts`. The song builder's own gains are used, so the pad is about 1.4 dB louder against the bass than in the DOS port, which used the unbuilt track. Each stream keeps the gain and waveform of its first appearance, as the port does for a slot, so the quieter bass in the breakdown is not carried.

### The write cap

The DOS port put 69 writes in tick 0, and again on every loop. Here no tick of either stream carries more than 16, because an interrupt at 560 Hz has to finish inside its 1.79 ms period; 16 writes are about 0.7 ms on an ISA bus by the planner's estimate, which nobody has measured on a 486. Every write that is not a key-on, a key-off or a drum switch (the instrument, the level, the output bits and the pitch's low byte) is placed in the ticks between the channel's previous key-off and the note, latest first. A note whose instrument writes still do not fit starts late by up to 3 ticks (1.8 ms each), or the note before it on its channel is cut short by up to 3. In this song OPL2 delays 6 notes and shortens 3, OPL3 delays 38 and shortens 45, and the most any note moves is one tick. The parity check allows the 3.

### The checks

`generate.ts` runs these on both arrangements of every song and refuses to write if any fails. `bun test tools/music` runs them again, and feeds them broken streams to show that each one fires.

- No write to channel 5's or channel 14's registers, in either bank. No write to 0x104 or 0x105. No write to a channel the arrangement does not own, and no bank-1 write in an OPL2 stream.
- At most 16 writes in any tick, the prelude included.
- Every channel and every drum is keyed off at the end of the loop.
- The second pass of the loop sets every note up (all of its operator registers, feedback, output bits and pitch) as the first did.
- Note parity. Each arrangement's note-ons are decoded back from the registers (the frequency registers are converted to MIDI) and compared with `assets/music/notes/crater.txt`. The OPL2 stream must play the manifest's 739 main notes less the 8 it lists as dropped, and the OPL3 stream all 1,400. Each must start on its tick or at most 3 ticks late. The drum hits must be the manifest's, tick for tick.

### What each step loses

- The song to the note list: only the velocity, which is a convenience.
- The note list to the streams: the partner voices' 2 ms nudge, the filters and their envelopes, the slides and the noise colours. A sine, triangle, square or saw becomes one of ten FM patches, and a drum is a rhythm-mode drum. The patches are the DOS port's first guesses.
- OPL2 against OPL3: the partner layers, the stereo placement and 8 pad notes. OPL3 against the Web: the stereo is only left, right or both.
- Timing: a note may start up to 3 ticks late, and a note whose channel changes instrument is cut a tick short to make room for the writes. Both are small for a pad or a partner, but nobody has listened.

### Previews

`bun tools/music/render-wav.ts <directory> [seconds]` writes an approximate WAV of the first minute (the default) of each arrangement for listening: mono for OPL2 and stereo for OPL3. It is `tools/sfx/render-wav.ts`'s two-operator model widened to the 18 channels in both banks with their output bits, plus a crude model of the drums. It is a listening aid only, its output is not committed, and it is not a chip: the envelopes are a rough table, there is no key scaling, tremolo or vibrato, and each drum is one operator over noise. Nobody has judged it against hardware or against DOSBox.

## Players (planned)

```text
crates/fizz-music tables  ->  DOS player (the stepper, then the audio service)
assets/music/*            ->  Web player (Undertone)
```

- **`fizz-music` player (built).** A `no_std` stepper in the crate that steps these records at 560 Hz, loops at `loop_start`, and keys off only the music channels on stop, in the style of `fizz-sfx`. See below.
- **DOS audio service (planned, package D).** Runs the stepper from the 560 Hz interrupt beside the cluck. It needs `fizz_machine::route` to leave an OPL3 music stream's output bits alone, a `MUSIC` log line carrying the writes and the hash, and `/BENCH /AUDIO`. The lowering does not touch the DOS runtime.
- **Web player (built).** Plays the same song with Undertone's `arrange` and `loop`, started by the presenter and never by the rules code. See below.

Music must not reach `fizz-ep2`: it adds no event, and no game logic reads the music clock.

## The stepper (built)

`fizz_music::MusicPlayer` is `no_std`, allocation-free, `forbid(unsafe_code)` and has no dependencies. It touches no hardware: it hands the writes that are due to the caller, which sends them to the chip. It is the music's counterpart of `fizz_sfx::OplPlayer`, with a slice in place of a callback so that an interrupt handler can fill a fixed buffer.

```text
MusicPlayer::new(level_song().arrangement(Card::Opl2 | Card::Opl3))
tick(&mut [Write]) -> usize           the writes due on this 560 Hz tick (bank, reg, val); 0 while paused
silence(&mut [Write]) -> usize        key the music off (below)
pause(&mut [Write]) -> usize          stop stepping, then silence
resume()                              carry on from the next record
reset()                               back to the start of the prelude; writes nothing
loops() -> u32, is_paused(), is_silent()
Fnv::new() / .byte(b) / .write(tick, Write) / .value()
writes_for(arrangement, ticks) -> (writes, hash)      a host helper: step a fresh player and hash it
count_loops(arrangement, ticks) -> u32
```

`tick` plays the prelude once, then jumps back to `loop_start` at the end of each pass, forever, and `loops()` counts the jumps (0 until the loop first restarts). A buffer of `MAX_WRITES_PER_TICK` (16) is always enough. If the buffer is smaller the rest of that tick's writes come on the next call, late and in order, and the stream's waits are held until they have gone out. The test that steps the stream "as fast as possible" (the plan's `/MUSICTEST` idea) is `two_loops_repeat_the_pass_and_never_touch_a_reserved_register`: the DOS check can do the same with no clock.

### Silence

The player keeps a copy of the last value it handed out for each music channel's `0xB0+ch` register (both banks, so an OPL3 stream is covered) and for `0xBD`. `silence` writes each of those that has the key bit set (bit 5) again with that bit cleared, and `0xBD` with the five drum key bits (0x1f) cleared, leaving the rhythm-enable and depth bits alone. Nothing else is written, so the instruments, levels and pitches survive, and the music can carry on. It never writes channel 5's or 14's registers (the player does not even remember them) or `0x104` and `0x105`, and a second call writes nothing. At most 17 writes (`MAX_SILENCE_WRITES`) are made; with a smaller buffer, call again for the rest. The test calls it after every tick of a pass, on a clone of the player, and checks that every music channel ends keyed off and no reserved register is written.

`pause` is `silence` plus a flag that stops `tick` (it returns 0 and the stream does not advance). `resume` clears the flag; the next `tick` continues with the next record, with no record skipped or repeated, but a note that was sounding stays off until its next key-on (a held pad is cut). `reset` restarts from the prelude without writing anything and keeps the copy of the key registers, so call `silence` first if the music is sounding.

### The hash

`Fnv` is the generator's FNV-1a accumulator (`tools/music/format.ts`): feed it each write as its due tick (u32 little-endian, counted from the first record), bank, register and value. It is integer-only, so the DOS build can run it in the 560 Hz handler. `writes_for(arrangement, pass_ticks())` steps one pass and returns the write count and the hash, which equal the arrangement's `record_count` and `hash` (2,961 and `0x4ae16486` for OPL2, 5,035 and `0xb31d1e26` for OPL3); the tests assert it. The later passes are the loop only, so their ticks restart at the loop point and the two passes hash alike. `cargo run -q -p fizz-music --example stream -- opl3 [ticks]` prints the stream as `tick bank reg val` lines.

The stepper has been run only on the host. Nobody has run it on a DOS machine, driven a chip with it or listened to it.

## The Web player (built)

```text
web/audio.ts     AudioRoot: one AudioContext, a music bus, an sfx bus, a limiter; routedContext
web/music.ts     Music: builds the song as Fizz 1's loopSong does and loops it on the music bus
web/sfx.ts       Sfx: the cluck, which now ends in the sfx bus when given a destination
web/main.js      the lifecycle: first key or click, M, the panel's Music button, the hidden tab
```

**Graph.** The cluck voices go into the sfx gain, the song into the music gain, and both into a `DynamicsCompressor` and out to the speakers. Undertone connects every voice to `ctx.destination`; `routedContext(ctx, bus)` is a view of the context whose destination is a bus, which is how each player is sent to its own gain. The limiter (threshold -6 dB, knee 6, ratio 6, attack 3 ms, release 150 ms) is there because Undertone has none and a cluck landing on a loud bar could clip; it should not be audible otherwise.

**Levels.** The sfx bus is 1.0, so the cluck is exactly as loud as before the music existed. The music bus is 0.65. The reasoning: the song's parts already carry Fizz 1's own trim (0.75, `crater.ts`), the cluck is a short, bright event that has to cut through a continuous bed, and 0.65 is about 3.7 dB under unity, which keeps the bass and lead clearly below a cluck peak without making the song feel thin. It is a first guess made without listening; the constants are `MUSIC_BUS_GAIN` and `SFX_BUS_GAIN` in `web/audio.ts`.

**Voices.** `buildPartVoice` is Fizz 1's `buildVoice` with the sound field on: the envelope is held for the note's length (sustain 0.3 unless the part says otherwise), and a part's role places it. Bass, kick, snare and pad are centred with no panner; the lead leans left (-0.15), the hats right (0.25); the arpeggio alternates -0.3 and 0.3 on every eighth of the bar; a partner voice sits opposite its part (a part with a centred role uses its own `mirror` side). A panned part gets the make-up gain of the square root of 2, because a mono voice through a stereo panner at centre is 3 dB down. `lpenv` keeps Fizz 1's meaning for music (Undertone adds it to `lpf`), so the song sounds as it does in Fizz 1; the cluck deliberately reads `lpenv` as the Hz the filter opens to (see [SFX.md](SFX.md)) and is not changed. (Only the quiet partner voices use `lpenv`; the builder sets it.)

**Looping.** The song is one pattern, `stack(...streams)`, each stream an `arrange` of its section voices, played by `.loop({ ctx, bpm: 148 })`. Undertone schedules the next 0.3 seconds on the audio clock every 100 ms, so the end of the last bar runs straight into bar 1 with no gap; a test checks that the second pass repeats the first note for note.

**Lifecycle.**

- Nothing is created or scheduled before a user gesture. The first key press or click (`pointerdown`, `keydown`) makes the context through `Sfx.unlock()` and then calls `Music.sync()`, which starts the song from bar 1 if the Music toggle is on and sound is not muted.
- `M` mutes everything: `Sfx` suspends the shared context, which freezes the song and silences the cluck. Unmuting resumes it where it was. If the page started muted, the song starts from bar 1 the first time it is unmuted.
- A hidden tab suspends the context and a shown one resumes it (`Sfx.sync`).
- The panel's Music button stops and starts only the song, always from bar 1. The choice is remembered in `localStorage` as `fizz2-music` (`on` or `off`, default on); blocked storage just means it is not remembered.
- The song keeps playing through death, respawn and the level exit. The rules never start, stop or read it: no `Event`, no change to the state hash or the golden transcript.
- If Undertone throws, there is no music and one console warning; there is no built-in fallback synth.

**Checks.** `bun test web/music.test.ts` runs the real Undertone against a recording stand-in for `AudioContext` and a hand-stepped timer: over one loop the sources scheduled match `noteEvents()` for start time (nudge included), pitch (or noise) and pan, one for one; the loop repeats; the buses and the limiter are wired; nothing exists before unlock; `M` and the toggle behave; a broken Undertone warns once. `tools/check-web-sfx.py` drives the built page in headless Chromium with a stand-in: nothing before a key, bar 1's first beat after one, `M` suspends the context, a muted page starts no song, and the Music button works and is remembered. Listening renders are not committed.

**Not verified.** Nobody has heard it. Browser autoplay behaviour in practice, the level of the two buses, whether the limiter is ever heard, the pans and how the song sounds against the cluck all need the owner's ears.
