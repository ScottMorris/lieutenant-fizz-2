# Music Lab

The Music Lab is a page of music sketches for Episode 2, in `labs/music/`. It holds fourteen tunes written and played entirely in Undertone, a live player whose sections can be tapped between, a set of checks on every tune, and downloads made in the browser. It is a tool for choosing and shaping music; nothing in the game loads it, and it changes no game data. The game's own level track is still the placeholder described in [MUSIC.md](MUSIC.md).

Nobody has listened to these tunes as Fizz 2's music. The checks below read notes and numbers; they cannot say whether a tune is good.

## Running it

```bash
labs/music/build.sh            # builds dist/music-lab
labs/music/build.sh --serve    # builds, then serves it on http://127.0.0.1:8090/
bun test labs/music            # the lab's tests
bun labs/music/write-history.ts   # rewrite docs/MUSIC-LAB-HISTORY.md (--check: fail if stale)
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

The loop becomes a song through `assets/music/song.ts`, the builder the game's level track uses, unchanged: an intro, the tune, a contrast section in a related key, a breakdown and a build, and the tune again doubled an octave up; with a filter envelope on every voice and a quiet sine partner on the melodic ones. Each tune's `SongDef` chooses its contrast key and whether the pad is arpeggiated, and two tunes pin a form.

`buildTune` in `labs/music/build.ts` then changes two things about that form:

- **No fill.** The builder's closing one-bar fill is left out (`noFill`), and no tune lifts its last section to another key. The song therefore ends on the last bar of the tune in its home key, which sits on the dominant, and the intro that follows opens on the tonic: the end leads into the beginning and the loop has no seam.
- **An orchestral build.** The builder's build bar is a snare roll over a held pad. The lab replaces everything in that bar with `symphonicBuild`: strings holding the dominant chord and opening up through the bar, two string lines climbing the scale an octave apart from opposite sides, the bass on the dominant, and a timpani on the same note rolling faster and louder. Every voice is pitched and in the key. Tunes whose form has no breakdown have no build either.

## Pathfinder: six tunes from scenarios

Tunes nine to fourteen come from a game. Six environments were dreamed up from maze types of the level design boards, and four players (Haiku agents) took turns at Pathfinder: given a place, walk Ben through one scenario there in sixteen steps, saying at each step how high he is in the room (0 to 7), what he is doing (walk, climb, drop, leap, hold or ride) and how tense the moment is (0 to 3). A hold keeps the height of the step before it; other steps move by one or two, a drop or leap by up to four; and the path ends within one of where it began, because the song loops. Four paths are one player's. Two are relays: one player wrote the first eight steps and a hand-over note, and another, given the story so far, wrote the last eight. `labs/music/paths.ts` keeps every path as written, and a test holds each to the rules of the game.

| Path | Place and maze type | Players | Key and tempo |
|---|---|---|---|
| Chimes in the Cocoa | Dripstone Deep, a cavern of one-way drops | one | A minor, 84 bpm |
| Patience in Lift Nine | Lift Nine, a vertical shaft with lifts | three | D lydian, 80 bpm |
| Lamps for the Valves | The Valve Hall, hub and spoke | two | C major, 104 bpm |
| Behind the Painting | A gallery with a false wall and a secret room | four | A dorian, 92 bpm |
| The Quick Way Home | The Long Way Round, a loop with a shortcut | one out, two home | E minor, 118 bpm |
| Swapping Spans | The Bridge Room, a two-floor swap with a switch | three in, four out | D minor, 96 bpm |

A path is read as a pattern like the others, one cell a column at the step's height, and becomes the tune's hook: sixteen steps as eighth notes over two bars, every hold a held note. The path's tension becomes a velocity pattern on the tune (`tensionAccent`), so it leans in where the scenario tightens. The remaining six bars of each eight-bar loop are written around the hook and never repeat it: an answer (a sequence, or for Swapping Spans the hook upside down), a stretch where the tune holds one or two notes while the other voices move, and a varied return that ends on the dominant.

These tunes are written to flow. Three lines carry each one: the tune, a counter-melody that moves against it, and a bass that walks in half notes. All of them sustain and release slowly (`L` in `tunes.ts`), no note is shorter than an eighth, and the drums are a soft kick and a brushed hat at most. Four further checks hold them to that (below).

## What the lab adds to a voice

`labs/music/voices.ts` builds each part as the game's Web player does (`buildPartVoice` in `web/music.ts`) and adds:

- **A pan of its own.** A part may carry a mini-notation string of pans, sampled at each note, in place of its role's fixed pan. This is how the beam's lead sweeps across the field, the three keys sound left, middle and right, the tread drum rolls past, and the identical rooms alternate sides. The partner voice of a moving part leans a third as far the other way, so the pair still travels; the partner of a fixed part mirrors it, as in the game.
- **A crossing echo.** A part with `cross` gets a quieter copy one echo time late on the opposite side.
- **Accents, FM, a phaser.** Velocity patterns for hats and leads, FM for bell and metal tones.
- **Humanising.** Every voice goes through Undertone's `drift` with its role's settings from `HUMAN` in `labs/music/build.ts`. The kick moves at most 1.5 ms; the snare sits 4 ms behind and the hats 2 ms ahead; the tune and the pads move up to 9 ms, 12 percent in level and 4 cents. The seed is the tune, the section and the stream, and `drift` advances it each pass, so every time round is different and a replay is identical.
- **A 7.1 placement.** In the surround mix each note's pan becomes a position on Undertone's speaker ring (`ringGains`): the bass and kick in front with the subwoofer, the pads around the sides and rear, the arpeggio and bells behind. Beam Over the Point, the one tune with a 7.1 mix, sends its lead once round the ring every two bars with its partner directly opposite.

## The live player

Undertone's `loop` plays one fixed pattern. `labs/music/player.ts` instead asks each section's pattern for one beat at a time and books those notes with `playParams`, a quarter of a second ahead of the audio clock, so a tap can still change the rest of the bar. Tapping a section calls it, in one of three ways:

- **At the bar, with a turn** (the default). The rest of the current bar becomes a turn: the drums and bass drop out for a timpani roll that quickens and grows, strings swelling on an open fifth, and a bass that sits on the dominant of the note it is heading for and then climbs two semitones into it. Like the build, the turn has no drum fill in it. The tune and pads of the old section carry on over the turn, and the new section starts on the barline. The turn is itself an Undertone pattern (`turnPattern`); the player sounds only the beats that are left.
- **On the beat.** The called section takes over on the next beat, at the same beat of its first bar.
- **When this one ends.** The current section finishes, with a turn over its last two beats.

Left alone the song plays its sections in order and loops. Hold keeps the current section going round. Tapping the called section again cancels the call. Keys 1 to 9 tap a section.

## The visualizers

The player has a small screen above the section strip, 320 by 120 pixels scaled up with hard edges and drawn in the sixteen EGA colours only (`labs/music/viz.ts`). Five pictures can be chosen, or none:

- **Lighthouse.** The night coast: the beam sweeps out and back once a bar and reaches further when the music is louder, the sea swells with the bass, and the stars twinkle with the high end.
- **Fizz.** Soda bubbles pour on every beat, more of them on the louder side of the stereo field, and rise faster as the level rises; the can hops on the beat.
- **Vats.** A spectrum as twenty cocoa vats that fill, each with a froth line that falls slowly.
- **Stereo field.** Left against right: a centred sound draws a vertical line, a sound to one side leans, and width spreads the cloud. This is the one to watch for the pans.
- **Scope.** The left and right waveforms.

They read the left and right outputs through analyser nodes and the player's position in the bar, and they make no sound. Scope and Stereo field follow the level so a quiet tune still fills the picture. A magenta line along the top marks a turn. The page starts with the picture off for anyone whose system asks for reduced motion.

## The checks

`labs/music/validate.ts` runs fourteen checks on every tune and four more on the Pathfinder tunes, and the page shows each with what it measured. `bun test labs/music` fails if any tune fails any of them.

Music theory, read from the notes through Undertone's parser:

1. Each riff is its pattern: the notes are the pattern's cells, in order.
2. Every note belongs to the key, in the loop and in the built song with each section in its own key.
3. Where the tune lands on beat one or three, it is a unison, third, fifth or sixth above the bass (at least 85 percent, and at least four such landings).
4. No parallel fifths or octaves between the tune and the bass across those landings.
5. No leap wider than an octave, and a leap over a fifth is followed by a step or a change of direction (at least 80 percent).
6. The tune spans at most two octaves and the bass never crosses it.
7. The loop's bass opens on the tonic and its last bar sits on a root that leads back (the dominant, the subtonic, the subdominant or a supertonic).
8. The song is 45 to 80 seconds, every onset is on the game's 96-tick grid, there is no fill and no snare roll, and the last section is in the home key.

Feel, measured on the humanised patterns the lab plays:

1. No note is moved more than 12 ms, and the pitched voices are at least 1 ms off the grid (RMS).
2. No level changes more than 15 percent, and the typical change is at least 2 percent.
3. No note is detuned more than 6 cents or moved more than 0.05 of the field.
4. The moves have no pattern: the note-to-note correlation of timing is under 0.25, and no voice's average move is more than four standard errors from zero.
5. A second pass moves at least 90 percent of notes differently, and a rebuild from the same seed is identical.
6. The kick is the steadiest role, no role leans more than 5 ms, the snare leans behind and the hats ahead.

Flow, for the Pathfinder tunes only:

1. Legato: the tune sounds for at least 90 percent of the loop, no note of the three lines is shorter than an eighth, no pitched voice is clipped short, and no drum plays more than eight times a bar.
2. Polyphony: tune, counter-melody and bass sound together on at least 90 percent of eighths, at least 30 percent of note starts belong to one melody alone, and at most 40 percent of their moves are in the same direction.
3. The hook: no later bar repeats a bar of the hook note for note, and at least one opens with the hook's shape on other notes.
4. Stillness: somewhere the tune plays at most two notes in a bar while the counter-melody plays at least six, and somewhere it plays at least six itself.

## Downloads

Each tune saves a stereo WAV (48 kHz, 16-bit), its note list and its source as JSON. A WAV is exactly one pass of the song: the three seconds of reverb and echo that ring past the last bar are folded onto the first (Undertone's `wrap`), so the file loops without a seam, as the song does. Beam Over the Point also saves a 7.1 WAV: eight channels in the order front left, front right, centre, subwoofer, side left, side right, rear left, rear right, with the WAVE extensible header and speaker mask `0x63F`. `labs/music/render.ts` renders with Undertone's offline `render` and encodes with its `encodeWav`. The stereo render passes through Undertone's master limiter as the live player does; a limiter cannot hold eight channels, so the 7.1 render is scaled as a whole if its peak passes 0.94.

Served from a web server the page saves with an ordinary download link. Published as a Claude artifact it saves through the host, which accepts a zip but not a WAV, so there a WAV is saved inside a zip (`labs/music/files.ts`).

## History

[MUSIC-LAB-HISTORY.md](MUSIC-LAB-HISTORY.md) is the conversation behind every tune: the briefs sent to the Haiku agents, their replies as written, and a note of each decision taken afterwards (the key, how the picture or path was read, what a check rejected and how it was fixed). The page shows the same history in each tune's deck, under "How this track came to be", and includes it in the tune's saved source. It lives in `labs/music/provenance.ts`, which reads the agents' pictures and paths from `patterns.ts` and `paths.ts` so the history cannot drift from the data. `bun labs/music/write-history.ts` rewrites the document and `--check` fails if it is stale; the check runs in `tools/check.sh` and CI.

## What is and is not verified

- `bun test labs/music` covers the patterns and riffs, the Pathfinder paths against the rules of the game, the degree notation, every check on all fourteen tunes, the pans and the 7.1 placement, the live player's three ways of changing section on a clock stepped by hand, and that every visualizer draws in EGA colours only.
- The built page was driven in headless Chromium: every tune was played, a section was called, and every tune's stereo WAV and the 7.1 WAV were rendered and saved with no console error. The 7.1 file was read back: eight channels, mask `0x63F`, signal on every channel.
- Nobody has listened to any of it. Headless Chromium has no speakers, the 7.1 file has not been played on a 7.1 system, and the live 7.1 path to eight real speakers has not run at all; on stereo hardware the page folds the 7.1 mix down with Undertone's own gains.
- The lab is not part of the game, has no DOS path and is lowered to no OPL data. A tune chosen for the game would move to `assets/music/` and go through the pipeline in [MUSIC.md](MUSIC.md).
