# Sound effects

The chicken's cluck is the first sound effect. It is defined once, as a rich Undertone effect, and the DOS data is derived from that definition by a script. Web leads on fidelity and DOS on budgets, so the Web plays the rich source and DOS plays two downmixes of it: an OPL2 chunk for AdLib-compatible cards and a PC speaker list for everything else. Nothing here plays yet. This document covers the source, the downmix, the generated crate and what each step loses.

Nobody has listened to any of this. The cluck is a placeholder built from the project's goals and from the shapes Fizz 1 uses for its effects. How effects should really sound is a separate discovery track, run in the owner's IMF lab; when that produces a designed effect it replaces `assets/sfx/cluck.ts` and the pipeline stays as it is.

## The pipeline

```text
assets/sfx/cluck.ts            the rich source: layered Undertone voices, three variants
        |
        |  tools/sfx/downmix.ts  (convert.ts, opl.ts, patches.ts)
        v
crates/fizz-sfx/src/data.rs    generated: OPL2 records and PC speaker divisors per variant
        |
        |  fizz-sfx: OplPlayer, SpeakerPlayer
        v
DOS presenter                  steps them from its interrupt (Milestone 1, a later package)
```

The Web presenter reads `assets/sfx/cluck.ts` directly and hands the voices to Undertone when the rules emit `Event::Cluck { variant }`. That presenter, and the DOS audio service, are separate packages; this one builds the source, the script and the crate.

## The source

`assets/sfx/voice.ts` defines `SfxVoice`, which has the fields of Fizz 1's type of the same name (`packages/engine/src/audio.ts`): a note name or noise type `n`, an oscillator `w`, envelope times `a`, `d` and `r` in seconds, a gain `g`, a low-pass `lpf` with its envelope `lpenv`, `lpa`, `lpd`, `lps` and `lpr`, a start offset `nudge` and an exact pitch `hz`. Undertone's default sustain is 0, so a voice is gone after its attack and decay. The module imports nothing from a browser or from Undertone, so Bun runs it, and its `noteHz` matches Undertone's note table.

A pitch curve is a chain of short notes, one every 25 ms, each at the next pitch, because Undertone's `slide` only falls and only from an octave above the note. Fizz 1's `curveVoices` does the same. `curve()` builds a chain; `breath()` builds a short burst of noise.

`assets/sfx/cluck.ts` exports `CLUCK`, indexed by `Event::Cluck { variant }` (`CLUCK_VARIANTS` is 3), and an `EFFECTS` table the downmix reads. The three variants are all short, none sustains, and each has a noise breath at the attack:

| Variant | Intent | Voices | Source length |
| --- | --- | --- | --- |
| 0 | "buh-GAWK": a low square "buh" nudging up a major third, then a loud sawtooth "GAWK" falling an octave and a half, with a square an octave under it for body | 20 | 300 ms |
| 1 | The short single cluck: one quick falling square note with a small noise rasp | 5 | 127 ms |
| 2 | The double cluck: two quick falling clucks a little apart, the second higher and longer | 10 | 305 ms |

Two layering rules make the source downmix well. Voices that share a start time put the main layer at the highest gain, and a noise breath starts a little before the pitched note it belongs to.

## The downmix

`tools/sfx/downmix.ts` converts every variant and writes `crates/fizz-sfx/src/data.rs`. It runs on Bun and uses no clock and no unseeded randomness, so the same source gives byte-identical output.

Both outputs run on the Keen effect clock of 140 frames a second. The OPL2 chunk is timed in 560 Hz ticks (four per frame), the rate of the IMF music format and of the DOS timer the audio service needs; the speaker list has one entry per frame.

**OPL2.** Effects use channel 5, which the music never touches. A chunk is a list of records, each a register, a value and the ticks to wait before the next record. It opens with the chip's waveform-select enable, writes a patch, keys the note on, and then once a frame sets the carrier level and the pitch (the F-number and block registers). The last record is the key-off, and the last wait pads the chunk to the whole frame count plus eight ticks. The patch follows the voice's oscillator: `fxSquare` for squares, `fxSaw` for sawtooths (more feedback, a buzzier edge) and `noise` for breaths (maximum feedback at a pitch that jitters from a seeded generator). The operator settings are the first guesses of Fizz 1's DOS port, which that repository's notes say are to be tuned by ear.

| Variant | Records | Bytes (4 each) | Ticks | Length |
| --- | --- | --- | --- | --- |
| 0 | 160 | 640 | 184 | 329 ms |
| 1 | 74 | 296 | 88 | 157 ms |
| 2 | 147 | 588 | 188 | 336 ms |

**PC speaker.** The list has one PIT channel 2 divisor per frame, where the tone is 1,193,180 divided by the divisor and 0 closes the speaker gate. The last entry is always 0. Pitches are clamped to 60 to 9000 Hz. Quiet frames are thinned: below 10% of the loudest level the speaker is silent, and below 30% it sounds every other frame. A noise voice becomes a random pitch.

| Variant | Entries | Bytes (2 each) | Length |
| --- | --- | --- | --- |
| 0 | 42 | 84 | 300 ms |
| 1 | 16 | 32 | 114 ms |
| 2 | 42 | 84 | 300 ms |

In all, the three variants take 1,524 bytes of OPL2 records and 200 bytes of speaker divisors.

### What is lost, and where

From the rich source to the OPL2 chunk:

- **One voice at a time.** The channel sounds the voice that started last; on a tie, the louder. Variant 0's square body an octave under the sawtooth is dropped entirely, and any quieter partner layer would be.
- **The filter.** The low-pass and its envelope have no counterpart. Brightness comes from the patch's modulator level and feedback, fixed for the whole voice.
- **Waveforms.** Square, sawtooth and noise become what two-operator FM can say with the three patches. They are approximations and have not been compared by ear.
- **Time and level resolution.** The pitch and level move once per 1/140 s, and the level in 0.75 dB steps relative to the loudest voice. Between two notes of a chain the pitch is interpolated per frame, so a fall is a sweep and not a staircase of 25 ms steps.
- **Noise.** A breath is a buzzy tone at a jittering pitch, not noise.
- **Stereo, reverb and any Web-only treatment.**

From the OPL2 chunk to the speaker list: level (the speaker is on or off, with only the thinning of quiet frames standing in for loudness), the patch (every voice is a square wave) and anything below 60 Hz or above 9000 Hz. The same single voice is heard, by the same tie rule.

### Converter changes from Fizz 1's DOS port

The converter is adapted from `convertSfx` in `m4/sound/fizz-dos/sfx.ts` of `lieutenant-fizz-dos`, which was written for one-note-after-another jingles. A cluck is layered chains of overlapping short notes, so these things differ:

- The note frequency comes from `voice.ts`, and an exact `hz` on a voice wins over its note name. The port imports Undertone for this; the tools here do not need it.
- On a tie the louder voice is heard. The port takes turns, which would retrigger the patch every frame.
- A pitched voice that begins while the previous one still sounds, on the same patch, continues it with a pitch and level move and no key-off and key-on. A chain is one gliding note.
- The pitch between chained notes is interpolated per frame.
- The level of a frame is that of the loudest live voice of the heard kind, so overlapping notes do not flutter the level.
- The output is records and a plain divisor list. The port's IMF byte writer is not needed.

## The crate

`crates/fizz-sfx` is `no_std`, forbids `unsafe` and has no dependencies (a dev-dependency on `fizz-ep2` lets one test prove that its `VARIANTS` equals `CLUCK_VARIANTS`). Its `data.rs` is generated. The rest is a small stepping API with no allocation and no hardware access:

- `VARIANTS`, the number of variants; `opl_steps(variant)`, `opl_ticks(variant)` and `speaker_steps(variant)` return the data. A `variant` out of range wraps, so a bad value is a different cluck and never a panic inside an interrupt handler.
- `OplStep { reg, val, wait }`, and `OplPlayer`: call `tick(&mut |reg, val| ...)` once per 560 Hz tick. It emits the writes due on that tick and returns `false` once the chunk's last wait has run out.
- `SpeakerStep::{Divisor(u16), Off}`, and `SpeakerPlayer`: call `tick()` once per 140 Hz tick. It returns `None` once the list is spent; the step before that is always `Off`.
- `EFFECT_CHANNEL` (5), `OPL_TICK_HZ` (560) and `SPEAKER_TICK_HZ` (140).

Last-wins between effects, detection of an OPL2, the 560 Hz timer and the ports are the DOS audio service's business, not this crate's.

## Regenerate and check

```text
bun tools/sfx/downmix.ts           regenerate crates/fizz-sfx/src/data.rs
bun tools/sfx/downmix.ts --check   fail with a diff if the committed file is out of date
```

The CI parity job runs `--check`, so a source change committed without its regenerated data fails CI. The crate's tests check that every variant terminates, ends keyed off, writes only channel 5's registers, stays in the audible divisor range and plays the same every time.

To listen, `bun tools/sfx/render-wav.ts <directory>` writes three WAV files per variant: the rich source, an approximate OPL2 preview of the generated records and a square-wave speaker preview. All three are approximations and none is committed.

- The rich file does not come from Undertone. Undertone renders through Web Audio, which Bun does not have, so the script has a small synth that follows Undertone's voice model (an oscillator or noise, a linear attack, a linear decay to a sustain of 0, a low-pass with a filter envelope). Its waves are naive, not band-limited, and the Web will sound like Undertone, not like this file.
- The OPL2 file is a two-operator FM model of channel 5 driven by the generated records, with the chip's feedback and level steps but a rough envelope-rate table and no key scaling, tremolo or vibrato. DOSBox's emulator or a real card will differ.
- The speaker file holds a square wave for each 1/140 s step. A real speaker and its case colour the sound.

## Provenance

The converter is the same author's code, MIT in `lieutenant-fizz-dos` and dual-licensed here: `tools/sfx/convert.ts` from `m4/sound/fizz-dos/sfx.ts`, `tools/sfx/opl.ts` from `opl.ts` and `tools/sfx/patches.ts` from `patches.ts`, all at commit `221dd6c`. Each file's header says what changed. The voice model and the idea of a pitch curve as a chain of short notes come from Fizz 1 (`packages/engine/src/audio.ts` and `episodes/episode-1/src/audio/enhanced-sfx.ts` at commit `4bce036`); the cluck's voices are written for this repository.

## Not verified

- Nobody has listened to the cluck in any form: not the source, not the OPL2 chunk, not the speaker list. The WAV previews are approximations of each and are not evidence of how the game will sound.
- The OPL2 data has not run on an OPL2 chip or in an emulator, and the patches are untuned first guesses.
- Nothing plays in a build yet. Playback arrives with the Web and DOS sound packages, and the player's manual has nothing to say about sound until then.
