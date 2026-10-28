# Music

The slice's level music is a song source: Undertone patterns that a builder lays out as a full song, and a script that lists every note. The Web page plays it; the DOS build does not yet. This document covers what exists (the source, its checks and the Web player) and marks the DOS parts as planned.

## Placeholder status

The song is "Crater Fields", and it is Fizz 1's track used as a placeholder test song for Fizz 2's test level. It is not an official Fizz 2 track, and the owner will replace it. Swapping it is a data-only change: add the new track beside `assets/music/crater.ts` and point `MANIFEST.level` in `assets/music/songs.ts` at it. Nobody has listened to it as Fizz 2's music, and nobody has listened to anything built from it.

## The source (built)

```text
assets/music/types.ts     MusicPart, MusicSong, PartRole and PART_PAN (the stereo placement of each role)
assets/music/pattern.ts   reads a part's mini-notation into timed events through Undertone's own parser
assets/music/song.ts      the builder: plan (the form nearest 60 seconds), withDepth (quiet partner voices), songFor
assets/music/crater.ts    the track and its settings (trim 0.75, contrast 5, lift 2, arpeggio on)
assets/music/songs.ts     the manifest: { level: 'crater' }
assets/music/events.ts    the song as a flat, deterministic list of note events
tools/music/notes.ts      writes assets/music/notes/crater.txt, or checks it with --check
```

Crater Fields is A minor at 148 bpm: a lead, a bass, a square pad, and a kick, snare and hats. The builder lays it out as Intro 2 bars, A 8, B 8 (up five semitones, the pad arpeggiated), A 8 (the lead doubled an octave up), Breakdown 3, Build 1, A' 8 (up two semitones, arpeggio and octave) and a one-bar Fill, which is 39 bars and 63.24 seconds. The song loops from the end straight back to the start. The lead, bass, arpeggio and octave parts also get a quiet sine partner on the same note, panned the other way.

Everything in `assets/music/` is pure, so it runs in Bun and in the browser bundle. The only dependency is `@liminal-hq/undertone` 0.2.0, as pinned.

## Provenance

The code is the same author's, Apache-2.0 OR MIT in Fizz 1 and here. The track is `crater` in `episodes/episode-1/src/audio/patterns.ts` (lines 375-399) and its settings are in `songs.ts` (line 17); the builder is `packages/engine/src/music-songs.ts` and the roles and pans are `packages/engine/src/sound-field.ts`, all of ~/source/liminal-hq/lieutenant-fizz at commit `4bce036`. The note reader and the way sections are laid end to end are adapted from `m4/sound/fizz-sheet-events.ts` and `fizz-dos/song.ts` of the DOS port (ScottMorris/lieutenant-fizz-dos) at commit `221dd6c`, MIT there and dual-licensed here. Each file's header names its source and what changed.

## Checks

- `bun test assets/music` holds the builder to Fizz 1. `assets/music/fizz1-crater.fixture.json` is Fizz 1's own `songFor` output, saved read-only from that commit, and the test asserts the same tempo, sections, parts and notes. Other tests cover the planner, the depth partners, the role pans and the note events.
- `bun tools/music/notes.ts --check` regenerates `assets/music/notes/crater.txt` in memory and fails with the first differing line if the committed list is stale. Run `bun tools/music/notes.ts` to rewrite it. The list is every note of the song (96 ticks to a bar): tick, seconds, part, MIDI note or noise, length and velocity, with the event count of each part in its header.
- Both run in CI, and in `tools/check.sh` when Bun and `node_modules` are present.

The velocity column is a convenience: the part's gain after the song's trim, scaled so the bass's gain is 127. It is not a Fizz 1 value.

## Data flow (planned)

```text
assets/music/*  ->  lowering generator  ->  fizz-music data  ->  DOS player
       |
       +--------------------------------->  Web player (Undertone)
```

- **Lowering generator (planned).** Reads `noteEvents` and lowers the song to OPL2 and OPL3 register streams for the DOS build, as the cluck's downmix does. The note list is the parity manifest it is held to.
- **`fizz-music` (planned).** A `no_std` crate holding the generated streams and a player that steps them at 560 Hz, in the style of `fizz-sfx`.
- **Web player (built).** Plays the same song with Undertone's `arrange` and `loop`, started by the presenter and never by the rules code. See below.

Music must not reach `fizz-ep2`: it adds no event, and no game logic reads the music clock.

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
