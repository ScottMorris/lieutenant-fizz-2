# Music Lab

The Music Lab is a page of music sketches for Episode 2, in `labs/music/`. It holds eight tunes written and played entirely in Undertone, a live player whose sections can be tapped between, a set of checks on every tune, and downloads made in the browser. It is a tool for choosing and shaping music; nothing in the game loads it, and it changes no game data. The game's own level track is still the placeholder described in [MUSIC.md](MUSIC.md).

Nobody has listened to these tunes as Fizz 2's music. The checks below read notes and numbers; they cannot say whether a tune is good.

## Running it

```bash
labs/music/build.sh            # builds dist/music-lab
labs/music/build.sh --serve    # builds, then serves it on http://127.0.0.1:8090/
bun test labs/music            # the lab's tests
```

The build needs Bun and the pinned `@liminal-hq/undertone` 0.3.0, as `tools/build-web.sh` does. It writes `index.html`, `lab.css`, `lab.js` (the page's script and Undertone in one file) and the Fizz fonts, and also `artifact.html`, the same lab as one self-contained file. The page fetches nothing and ships no audio: every sound and every saved file is made in the browser when asked for.

## From a pattern to a song

Each tune begins as a picture. Six concepts from the game were each drawn as a grid of 8 rows by 16 columns (`labs/music/patterns.ts`, kept exactly as drawn):

| Pattern | Concept | Tune | Level type | Key and tempo |
|---|---|---|---|---|
| `beam` | Lighthouse beam sweep | Beam Over the Point | The vertical tower (Lighthouse Point) | E dorian, 108 bpm |
| `press` | Siphon press rhythm | Press Line | The conveyor and press gauntlet (Siphon Works) | D phrygian, 120 bpm |
| `keys` | Key-door chain | Three Keys | The key-door chain | A minor, 100 bpm |
| `crystal` | Zargoth crystal growth | Crystal Bloom | Zarg overgrowth | B minor, 80 bpm |
| `tanker` | Cocoa Siphon tanker | Tanker on Treads | The boss room | C minor, 138 bpm |
| `rooms` | Identical rooms, one landmark | Same Room Twice | Identical rooms with a landmark | F lydian, 88 bpm |
| `beam`, `keys`, `rooms` | | Night Map | The overworld | G major, 116 bpm |
| `tanker`, `press`, `crystal` | | Mildred's Plan | The last level | D minor, 132 bpm |

The level types are the maze concepts of the Fizz 2 level design boards (towers, key-door chains, the conveyor and press gauntlet, identical rooms with a landmark) and the places in [LORE.md](LORE.md).

A pattern is read as a riff by one rule (`riffOf`): a column is a step in time, a row is a scale degree with the bottom row lowest, the highest filled cell of a column is the melody and the lowest filled cell, where it differs, is a second line. A tune may sit the bottom row on its tonic or one step under it, and may read a run of equal cells as one held note. A tune is then written around its riff in scale degrees (`labs/music/scale.ts` turns a degree string into note names, leaving the rhythm of the mini-notation alone), which is what lets the two combination tunes take three riffs each into a new key.

The loop becomes a song through `assets/music/song.ts`, the builder the game's level track uses, unchanged: an intro, the tune, a contrast section in a related key, a breakdown and a build, the tune again lifted and doubled an octave up, and a one-bar fill; with a filter envelope on every voice and a quiet sine partner on the melodic ones. Each tune's `SongDef` chooses its contrast key, its lift and whether the pad is arpeggiated, and two tunes pin a form.

## What the lab adds to a voice

`labs/music/voices.ts` builds each part as the game's Web player does (`buildPartVoice` in `web/music.ts`) and adds:

- **A pan of its own.** A part may carry a mini-notation string of pans, sampled at each note, in place of its role's fixed pan. This is how the beam's lead sweeps across the field, the three keys sound left, middle and right, the tread drum rolls past, and the identical rooms alternate sides. The partner voice of a moving part leans a third as far the other way, so the pair still travels; the partner of a fixed part mirrors it, as in the game.
- **A crossing echo.** A part with `cross` gets a quieter copy one echo time late on the opposite side.
- **Accents, FM, a phaser.** Velocity patterns for hats and leads, FM for bell and metal tones.
- **Humanising.** Every voice goes through Undertone's `drift` with its role's settings from `HUMAN` in `labs/music/build.ts`. The kick moves at most 1.5 ms; the snare sits 4 ms behind and the hats 2 ms ahead; the tune and the pads move up to 9 ms, 12 percent in level and 4 cents. The seed is the tune, the section and the stream, and `drift` advances it each pass, so every time round is different and a replay is identical.
- **A 7.1 placement.** In the surround mix each note's pan becomes a position on Undertone's speaker ring (`ringGains`): the bass and kick in front with the subwoofer, the pads around the sides and rear, the arpeggio and bells behind. Beam Over the Point, the one tune with a 7.1 mix, sends its lead once round the ring every two bars with its partner directly opposite.

## The live player

Undertone's `loop` plays one fixed pattern. `labs/music/player.ts` instead asks each section's pattern for one beat at a time and books those notes with `playParams`, a quarter of a second ahead of the audio clock, so a tap can still change the rest of the bar. Tapping a section calls it, in one of three ways:

- **At the bar, with a turn** (the default). The rest of the current bar becomes a turn: the drums and bass drop out for a snare roll that doubles its speed and grows, two kicks on the last beat, and a bass that sits on the dominant of the note it is heading for and then climbs two semitones into it. The tune and pads of the old section carry on over the turn, and the new section starts on the barline. The turn is itself an Undertone pattern (`turnPattern`); the player sounds only the beats that are left.
- **On the beat.** The called section takes over on the next beat, at the same beat of its first bar.
- **When this one ends.** The current section finishes, with a turn over its last two beats.

Left alone the song plays its sections in order and loops. Hold keeps the current section going round. Tapping the called section again cancels the call. Keys 1 to 9 tap a section.

## The checks

`labs/music/validate.ts` runs fourteen checks on a tune, and the page shows each with what it measured. `bun test labs/music` fails if any tune fails any of them.

Music theory, read from the notes through Undertone's parser:

1. Each riff is its pattern: the notes are the pattern's cells, in order.
2. Every note belongs to the key, in the loop and in the built song with each section in its own key.
3. Where the tune lands on beat one or three, it is a unison, third, fifth or sixth above the bass (at least 85 percent, and at least four such landings).
4. No parallel fifths or octaves between the tune and the bass across those landings.
5. No leap wider than an octave, and a leap over a fifth is followed by a step or a change of direction (at least 80 percent).
6. The tune spans at most two octaves and the bass never crosses it.
7. The loop's bass opens on the tonic and its last bar sits on a root that leads back (the dominant, the subtonic, the subdominant or a supertonic).
8. The song is 45 to 80 seconds and every onset is on the game's 96-tick grid.

Feel, measured on the humanised patterns the lab plays:

1. No note is moved more than 12 ms, and the pitched voices are at least 1 ms off the grid (RMS).
2. No level changes more than 15 percent, and the typical change is at least 2 percent.
3. No note is detuned more than 6 cents or moved more than 0.05 of the field.
4. The moves have no pattern: the note-to-note correlation of timing is under 0.25, and no voice's average move is more than four standard errors from zero.
5. A second pass moves at least 90 percent of notes differently, and a rebuild from the same seed is identical.
6. The kick is the steadiest role, no role leans more than 5 ms, the snare leans behind and the hats ahead.

## Downloads

Each tune saves a stereo WAV (48 kHz, 16-bit, the whole song and a three-second tail), its note list and its source as JSON. Beam Over the Point also saves a 7.1 WAV: eight channels in the order front left, front right, centre, subwoofer, side left, side right, rear left, rear right, with the WAVE extensible header and speaker mask `0x63F`. `labs/music/render.ts` renders with Undertone's offline `render` and encodes with its `encodeWav`. The stereo render passes through Undertone's master limiter as the live player does; a limiter cannot hold eight channels, so the 7.1 render is scaled as a whole if its peak passes 0.94.

Served from a web server the page saves with an ordinary download link. Published as a Claude artifact it saves through the host, which accepts a zip but not a WAV, so there a WAV is saved inside a zip (`labs/music/files.ts`).

## What is and is not verified

- `bun test labs/music` covers the patterns and riffs, the degree notation, all fourteen checks on all eight tunes, the pans and the 7.1 placement, and the live player's three ways of changing section on a clock stepped by hand.
- The built page was driven in headless Chromium: every tune was played, a section was called, and every tune's stereo WAV and the 7.1 WAV were rendered and saved with no console error. The 7.1 file was read back: eight channels, mask `0x63F`, signal on every channel.
- Nobody has listened to any of it. Headless Chromium has no speakers, the 7.1 file has not been played on a 7.1 system, and the live 7.1 path to eight real speakers has not run at all; on stereo hardware the page folds the 7.1 mix down with Undertone's own gains.
- The lab is not part of the game, has no DOS path and is lowered to no OPL data. A tune chosen for the game would move to `assets/music/` and go through the pipeline in [MUSIC.md](MUSIC.md).
