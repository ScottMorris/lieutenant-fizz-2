# Music

The slice's level music is a song source: Undertone patterns that a builder lays out as a full song, and a script that lists every note. Nothing plays it yet. This document covers what exists (the source and its checks) and marks the rest as planned.

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
- **Web player (planned).** Plays the same song with Undertone's `arrange` and `loop`, started by the presenter and never by the rules code.

Music must not reach `fizz-ep2`: it adds no event, and no game logic reads the music clock.
