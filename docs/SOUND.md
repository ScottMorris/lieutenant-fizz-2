# Sound

This document is the sound story of Lieutenant Fizz 2: what plays on each platform, how the effects (the chicken's cluck, the twelve others that Fizz 1 pairs with its captions and three alternates no event plays) are made and placed, what Fizz 1 does when sounds overlap, and how music fits in. The Web leads on fidelity and DOS on budgets, so the Web plays rich Undertone sources, authored with Undertone's FM voices and pitch steps, and DOS plays downmixes of them (except the fzzt, the boing and the jump, which are Fizz 1 DOS's own data; see [What DOS plays for the fzzt, the boing and the jump](#what-dos-plays-for-the-fzzt-the-boing-and-the-jump)): to the OPL3 pair (channels 5 and 14), to OPL2 (channel 5) or to the PC speaker, whichever the machine has.

## Overview

What plays where, today:

| Platform | Effects (19 ids: the cluck, the other twelve and three alternates) | Music |
| --- | --- | --- |
| Web | The rich Undertone source, placed by where the event happened (Enhanced) or centred (Classic) | The rich Undertone song, built (see [MUSIC.md](MUSIC.md)) |
| DOS, OPL2 or AdLib | The OPL2 chunk on channel 5, with the distance as a carrier level and no pan | Planned, on channels 0 to 4 and 6 to 8 |
| DOS, OPL3 or Sound Blaster | The OPL3 pair: channel 5 is the left side and channel 14 (bank 1) the right, each at its own level. With effects set to AdLib, channel 5 alone, in both speakers | Planned, on the 13 channels `[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 15, 16, 17]` |
| DOS, PC speaker | The speaker list, with no placement | None |

Two rules hold on every path. Channel 5 is the default effects channel everywhere, and channel 14 is its right extension on an OPL3, so music never uses either. Positional sound is view-only: the rules never see it, so it cannot change a hash or a replay.

Every effect has one definition, as a rich Undertone effect, and a script derives the DOS data from it. The effect id is the one number that names an effect on every path: entry `id` of `EFFECTS` in the source, entry `id` of every table in `data.rs`, the id `audio::effect` stores for the DOS service and the id the Web's `take_effect` hands the page. The chicken's cluck is ids 0 to 3 (its `variant` is its id). The others, in this order, are fzzt 4, stun 5, bonk 6, hurt 7, win 8, key 9, clunk 10, plink 11, boing 12, jump 13 and crunch 14; `crates/fizz-slice-view/src/captions.rs` says which rules event plays which id (see [The effects after the cluck](#the-effects-after-the-cluck)). A new effect is appended at the next index; there are no gaps, because the index is the id.

```text
assets/sfx/effects.ts          the rich source: layered Undertone voices, one entry per effect id (cluck = ids 0 to 3)
        |
        |  tools/sfx/downmix.ts  (convert.ts, opl.ts, patches.ts)
        v
crates/fizz-sfx/src/data.rs    generated: OPL2 records and PC speaker divisors per effect id
        |
        |  fizz-sfx: OplPlayer, SpeakerPlayer
        v
DOS audio service              steps them from its 560 Hz timer interrupt (dos/rt/src/audio.rs)
```

The Web presenter imports `assets/sfx/effects.ts` and hands the voices of an effect id to Undertone when the rules emit a sound event (every `Event` but `ShotEnd`; the cluck's effect id is its `variant`) (see [The Web path](#the-web-path)). The DOS audio service is described under [DOS playback](#dos-playback).

Each arrow loses something:

| Step | What it keeps | What it loses |
| --- | --- | --- |
| Undertone source to the OPL2 chunk | pitch contour (including pitch steps, which are already at 1/140 s), timing at 1/140 s, level contour, the FM voice's brightness and how fast it clears (as a modulator setting) | all but one voice at a time, the filter, true waveforms and noise, stereo; an FM voice's exact index, ratio and decay (see [FM voices](#fm-voices-and-pitch-steps)) |
| OPL2 chunk to the speaker list | the same single voice, its pitch and level contour as on or off | loudness, timbre, and pitches below 60 Hz or above 9000 Hz |
| OPL2 chunk to the OPL3 pair | the chunk, played on channels 5 and 14 with a level for each side | nothing in the sound, but two FM voices stand in for one stereo voice, and the level is fixed when the effect starts |

The owner listened to the new effects on the Web on 29 October 2026 and found them good ("they sound decent", and they may revisit them); nobody has reported how any effect sounds on DOS or on real hardware (see [Not verified](#not-verified)).

## Effects

The chicken's cluck was the first effect and the other fifteen came after it (see [The effects after the cluck](#the-effects-after-the-cluck)). All are placeholders built from the project's goals and from the shapes Fizz 1 uses for its effects. How effects should really sound is a separate discovery track, done by ear outside this pipeline (see [Scores from other tools](#scores-from-other-tools)). When that produces a designed effect it replaces its entry in `assets/sfx/effects.ts` and the pipeline stays as it is.

### The source

`assets/sfx/voice.ts` defines `SfxVoice`, which has the fields of Fizz 1's type of the same name (`packages/engine/src/audio.ts`): a note name or noise type `n`, an oscillator `w`, envelope times `a`, `d` and `r` in seconds, a gain `g`, a low-pass `lpf` with its envelope `lpenv`, `lpa`, `lpd`, `lps` and `lpr`, a start offset `nudge` and an exact pitch `hz`, and two more that Fizz 1 does not have: `fm`, an Undertone `fmVoice` two-operator FM voice, and `steps`, an Undertone `pitchsteps` curve (see [FM voices and pitch steps](#fm-voices-and-pitch-steps)). Undertone's default sustain is 0, so a voice is gone after its attack and decay. The module imports nothing from a browser or from Undertone, so Bun runs it, and its `noteHz` matches Undertone's note table.

A pitch curve is a chain of short notes, one every 25 ms, each at the next pitch, because Undertone's `slide` only falls and only from an octave above the note. Fizz 1's `curveVoices` does the same. `curve()` builds a chain; `breath()` builds a short burst of noise.

### The four variants

`assets/sfx/effects.ts` exports `CLUCK`, indexed by the `variant` of `Event::Cluck` (`CLUCK_VARIANTS` is 4), and the `EFFECTS` table, indexed by effect id, which the downmix, the Web and the WAV previews read (`EFFECTS` is `CLUCK` followed by the fifteen others, so cluck variant `n` is effect id `n`). The four variants are all short, none sustains, and each has a noise breath at the attack:

| Variant | Intent | Voices | Source length |
| --- | --- | --- | --- |
| 0 | "buh-GAWK": a low square "buh" nudging up a major third, then a loud sawtooth "GAWK" falling an octave and a half, with a square an octave under it for body | 22 | 345 ms |
| 1 | The short single cluck: one quick falling square note with a small noise rasp | 5 | 127 ms |
| 2 | The double cluck: two quick falling clucks a little apart, the second higher and longer | 10 | 305 ms |
| 3 | The repeated cluck: variant 1 played twice, "cluck, cluck" | 10 | 328 ms |

Variant 3 is not written note by note. `repeat()` in `voice.ts` takes a base effect and plays it twice: the base voices as they are, then a copy shifted to start 75 ms after the base ends (`REPEAT_GAP`), its pitched voices a semitone lower and every gain a tenth quieter. The base is `REPEAT_BASE`, set to variant 1, so repeating a different variant (or changing the gap, pitch or gain) is a one-line change in `effects.ts`. Variants 1 and 3 are both meant to be usable: 1 is the quick single, 3 the same cluck twice.

Two layering rules make the source downmix well. Voices that share a start time put the main layer at the highest gain, and a noise breath starts a little before the pitched note it belongs to.

### The downmix

`tools/sfx/downmix.ts` converts every effect (every entry of `EFFECTS`, in id order) and writes `crates/fizz-sfx/src/data.rs`. It runs on Bun and uses no clock and no unseeded randomness, so the same source gives byte-identical output.

Both outputs run on the Keen effect clock of 140 frames a second. The OPL2 chunk is timed in 560 Hz ticks (four per frame), the rate of the IMF music format and of the DOS timer the audio service needs; the speaker list has one entry per frame.

**OPL2.** Effects use channel 5, which the music never touches. A chunk is a list of records, each a register, a value and the ticks to wait before the next record. It opens with the chip's waveform-select enable, writes a patch, keys the note on, and then once a frame sets the carrier level and the pitch (the F-number and block registers). The last record is the key-off, and the last wait pads the chunk to the whole frame count plus eight ticks. The patch follows the voice's oscillator: `fxSquare` for squares, `fxSaw` for sawtooths (more feedback, a buzzier edge) and `noise` for breaths (maximum feedback at a pitch that jitters from a seeded generator). The operator settings are the first guesses of Fizz 1's DOS port, which that repository's notes say are to be tuned by ear.

| Variant | Records | Bytes (4 each) | Ticks | Length |
| --- | --- | --- | --- | --- |
| 0 | 178 | 712 | 212 | 379 ms |
| 1 | 74 | 296 | 88 | 157 ms |
| 2 | 147 | 588 | 188 | 336 ms |
| 3 | 150 | 600 | 200 | 357 ms |

**PC speaker.** The list has one PIT channel 2 divisor per frame, where the tone is 1,193,180 divided by the divisor and 0 closes the speaker gate. The last entry is always 0. Pitches are clamped to 60 to 9000 Hz. Quiet frames are thinned: below 10% of the loudest level the speaker is silent, and below 30% it sounds every other frame. A noise voice becomes a random pitch.

| Variant | Entries | Bytes (2 each) | Length |
| --- | --- | --- | --- |
| 0 | 48 | 96 | 343 ms |
| 1 | 16 | 32 | 114 ms |
| 2 | 42 | 84 | 300 ms |
| 3 | 46 | 92 | 329 ms |

In all, the four cluck variants take 2,196 bytes of OPL2 records and 304 bytes of speaker divisors; all nineteen effects take 9,908 and 1,148.

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

### The crate

`crates/fizz-sfx` is `no_std`, forbids `unsafe` and has no dependencies (a dev-dependency on `fizz-ep2` lets one test prove that its `EFFECT_COUNT` is `CLUCK_VARIANTS` plus the fifteen others, and another pins each effect's records, ticks and speaker frames). Its `data.rs` is generated. The rest is a small stepping API with no allocation and no hardware access:

- `EFFECT_COUNT`, the number of effects; `opl_steps(id)`, `opl_ticks(id)` and `speaker_steps(id)` return the data of effect `id`. An `id` out of range wraps, so a bad value is a different effect and never a panic inside an interrupt handler.
- `OplStep`, and `OplPlayer`: call `tick(&mut |bank, reg, val| ...)` once per 560 Hz tick. It emits the writes due on that tick and returns `false` once the chunk's last wait has run out. A step holds a register's low byte `reg`, its value `val`, a `bank()` (0 for registers 0x000 to 0x0ff, 1 for 0x100 to 0x1ff) and a `wait()` in ticks. The bank is the top bit of the `u16` that holds the wait, so a step is still four bytes and the wait is at most 32,767 ticks. The converter and the generated data write every register of the cluck in bank 0.
- `OplPlayer::new(id)` is the plain path: exactly the stored writes on channel 5 in bank 0. `OplPlayer::mono(id, attenuation)` is channel 5 with the distance added to the carrier's total level (OPL2, and an OPL3 on the AdLib role). `OplPlayer::side(steps, Side::{Left, Right}, attenuation)` is one member of the OPL3 pair and `OplPair` steps two of them; see [The OPL3 pair](#the-opl3-pair).
- `place::place(x, y, View)` returns a `Placement` (a pan, a distance and a level for each side) and `attenuation_for_level` the nearest 0.75 dB step; see [Where a sound is placed](#where-a-sound-is-placed).
- `SpeakerStep::{Divisor(u16), Off}`, and `SpeakerPlayer`: call `tick()` once per 140 Hz tick. It returns `None` once the list is spent; the step before that is always `Off`.
- `EFFECT_CHANNEL` (5), `EFFECT_CHANNEL_RIGHT` (14), `RESERVED_OPL_CHANNELS` (`[5, 14]`), `is_reserved_opl_channel`, `OPL_TICK_HZ` (560) and `SPEAKER_TICK_HZ` (140).

Last-wins between effects, detection of an OPL2, the 560 Hz timer and the ports are the DOS audio service's business, not this crate's (see [DOS playback](#dos-playback)).

## The effects after the cluck

Fifteen more effects follow the cluck's four (twelve that events play, ids 4 to 14 and the soda at 17, and three alternates, ids 15, 16 and 18 below), written in `assets/sfx/effects.ts` from Fizz 1's `SFX` table (`episodes/episode-1/src/audio/patterns.ts`) and its Enhanced curves and bell partials (`enhanced-sfx.ts`), at commit `4bce036`: the notes, lengths, gains and filters are Fizz 1's, and where Fizz 1's Enhanced version adds a pitch curve or a bell partial the source writes it as pitch steps or an FM voice, except fzzt (4), boing (12) and jump (13), which are Fizz 1's Enhanced voices unchanged (see [Fizz 1's fzzt, pogo and jump, and the FM alternates](#fizz-1s-fzzt-pogo-and-jump-and-the-fm-alternates)). On DOS those three are Fizz 1 DOS's own records, so their OPL and speaker columns below are the port's, not a downmix. `crates/fizz-slice-view/src/captions.rs` is the one table that says which rules event plays which id and raises which caption, and both presenters read it through `fizz_slice_view::event_sound`, so DOS and the Web play the same effect for the same event. A shot ending (`ShotEnd`) has a puff and no sound. Sound plays whatever the Captions setting says.

| Id | Effect | Event, where in the level | Caption | Source (Undertone) | OPL chunk | Speaker | What the DOS downmix loses |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4 | fzzt | `Fire`: every shot, from the blaster | fzzt | Fizz 1's Enhanced fzzt, plain: a white noise burst through a 3 kHz high-pass (gain 0.22, 80 ms) and a sine c6 that slides down an octave to its note in 80 ms (Undertone `slide`), through a filter that opens at the attack. No FM. 0.13 s. **DOS: Fizz 1 DOS's own fzzt** (its Classic noise burst and sliding sine, which the port's converter plays by taking turns frame by frame, a rapid buzz) | 198 records, 792 bytes, 171 ms | 16 frames, 32 bytes | Nothing is lost from Fizz 1 DOS's version (it is the data as the port made it); against the Web the filters and the second layer are not there |
| 5 | stun | `Stun`: a shot stuns a chicken | fizzled | A noise puff, then a sine at a4 wobbling 3 semitones at 18 swings a second while falling an octave, FM at ratio 1.5, index 4. 0.17 s | 75 records, 300 bytes, 200 ms | 16 frames, 32 bytes | The puff; the ratio 1.5 becomes the multiple 2, so the sputter is cleaner and higher |
| 6 | bonk | `Stomp`: Ben lands on a chicken | bonk | A click of noise, then a triangle falling an octave from c3 to c2 in 70 ms through a 400 Hz low-pass, FM at ratio 1, index 2 for 50 ms. 0.11 s | 74 records, 296 bytes, 157 ms | 14 frames, 28 bytes | The low-pass that keeps the Web's knock round; the knock is brighter on the chip |
| 7 | hurt | `Death`: Ben dies | whoa! | Two squares falling an octave together, a4 with a slight wobble and a fifth under it 150 ms later, FM at ratio 1 on the first. 0.6 s | 223 records, 892 bytes, 600 ms | 62 frames, 124 bytes | The second voice replaces the first when it starts (one voice at a time), so the fall steps down at 150 ms; the 900 Hz low-pass |
| 8 | win | `Exit`: Ben touches the exit | ta-da! | Four triangle notes rising c5 e5 g5 c6 and a long sine e6, the top two as FM bells (ratio 2.76, index 0.8 and 1.2). 0.77 s | 317 records, 1,268 bytes, 771 ms | 86 frames, 172 bytes | The bell ratio 2.76 becomes the multiple 3, so the bell is a plain tone with a bright attack; no two notes ring together |
| 9 | key | `Key`: Ben takes a gumdrop (red, blue or green: one sound) | red, blue or green gumdrop | Three triangle notes c5 e5 g5, the last an FM bell (ratio 2.76, index 1, 80 ms). 0.39 s | 166 records, 664 bytes, 393 ms | 42 frames, 84 bytes | The bell, as ta-da! |
| 10 | clunk | `DoorOpen` and `HeadBump`: a door opening, or Ben's head hitting a ceiling | clunk | A breath of brown noise, then c2 sagging three semitones through a 300 Hz low-pass, FM at ratio 1, index 1.2 for 60 ms. 0.21 s | 95 records, 380 bytes, 207 ms | 20 frames, 40 bytes | The low-pass and the brown noise (a buzz on the chip); the thud is thinner |
| 11 | plink | `Dud`: Fire pressed with no ammo | `click - no fizz` | A sine at c7 for 50 ms with a glint of FM (ratio 2, index 1.5, 30 ms). 0.1 s | 39 records, 156 bytes, 100 ms | 8 frames, 16 bytes | The glint is a plainer tone; at 2,093 Hz it is among the quietest things on the chip |
| 12 | boing | `Bounce`: the pogo stick bounces off the ground | boing | Fizz 1's Enhanced boing, plain oscillators: sixteen short triangle g3 notes, 25 ms apart, swinging five semitones at 9 swings a second and fading over 0.4 s (gain 0.42 easing down 55 percent), and a sine d5 70 ms in through a filter that opens at the attack. No FM. 0.41 s. **DOS: Fizz 1 DOS's own boing** (the Classic three notes g3, g4 and d5, three struck notes) | 81 records, 324 bytes, 200 ms | 18 frames, 36 bytes | The wobble chain and the filter envelope: DOS plays the Classic three notes, not the Enhanced chain |
| 13 | jump | `Jump`: a jump from the ground. No caption | none | Fizz 1's Enhanced jump: four square notes 25 ms apart rising nine semitones from e4 (e4, g4, a#4, c#5) through a 2.6 kHz low-pass, gain 0.2 easing down 55 percent. No FM. 0.11 s. **DOS: Fizz 1 DOS's own jump** (the Classic two squares e4 and b4, 35 ms apart) | 73 records, 292 bytes, 186 ms | 18 frames, 36 bytes | The four-step chirp and the low-pass: DOS plays the Classic two notes |
| 14 | crunch | `Snack`: a cheezie, a chocolate bar or a cookie | crunch | A noise burst, then c5 and e5 triangles with an inharmonic crackle (FM ratio 1.41, index 3, decaying over 60 and 80 ms). 0.24 s | 118 records, 472 bytes, 243 ms | 24 frames, 48 bytes | The burst (a buzz on the chip) and the crackle's ratio, which becomes the multiple 1 |
| 15 | boing (FM) | none (alternate; see below) | none | The first Fizz 2 boing: one triangle g3 wobbling five semitones at 9 swings a second as pitch steps, FM at ratio 1, index 2 for 300 ms, and a short sine at d5 70 ms in. 0.40 s | 186 records, 744 bytes, 450 ms | 42 frames, 84 bytes | The sine on top and the FM's springy edge, which becomes the patch's brightness |
| 16 | jump (FM) | none (alternate; see below) | none | The FM-era jump: one square rising nine semitones from e4 in 80 ms as pitch steps through a 2.6 kHz low-pass. No FM in it. 0.10 s | 57 records, 228 bytes, 150 ms | 12 frames, 24 bytes | The low-pass; quieter on the chip by the level floor (see below) |
| 17 | soda | `Can`: Ben takes a soda can, five more shots | fsssht | Fizz 1's `soda`, with nothing from its Enhanced set (it has none): a soft-attack hiss of noise (30 ms in, through an 8 kHz low-pass, standing in for Fizz 1's high-pass) and an a5 sine from 50 ms in with a faint FM glint (ratio 3.5, index 0.8, 100 ms). 0.35 s | 157 records, 628 bytes, 357 ms | 28 frames, 56 bytes | The hiss (a buzz on the chip) and the glint, which becomes a plainer tone |
| 18 | fzzt (FM) | none (alternate; see below) | none | The first Fizz 2 fzzt: a noise burst, then a sine falling an octave from c7 to c6 in 80 ms as pitch steps, FM at ratio 3.5, index 3, clearing over 90 ms. 0.13 s | 69 records, 276 bytes, 179 ms | 16 frames, 32 bytes | The 6 ms noise burst is a click before the tone; the FM ratio becomes the chip's multiple 4 |

The cluck keeps ids 0 to 3 and its data did not change: the generated cluck records are byte for byte what they were. In all, the nineteen effects take 9,908 bytes of OPL records (2,477 records) and 1,148 bytes of speaker divisors, 8,556 bytes more than the cluck alone. The OPL3 pair adds no data: the same records play on channels 5 and 14, each at its own level.

### Fizz 1's fzzt, pogo and jump, and the FM alternates

The owner listened (Web and DOS) and chose Fizz 1's three: its jump, its pogo and its fizz shot. They are the defaults on both platforms: id 13 (jump), id 12 (boing) and id 4 (fzzt). The voices they replaced stay in the table, wired to no event, as ids 16 (jump (FM)), 15 (boing (FM)) and 18 (fzzt (FM)); id 17 is the soda (a fizz can), which an event plays.

- **Ids 4, 12 and 13 are Fizz 1's Enhanced voices, the ones the Web plays**, with the notes, lengths, gains, waveforms, filters and pitch curves exactly as `enhanceEffect` makes them (`CURVES.jump`, `CURVES.boing` and `shaped` in `enhanced-sfx.ts`; tests in `web/sfx.test.ts` hold the numbers, and a throwaway comparison against Fizz 1's own `enhanceEffect` output found every field equal, the jump's gains within the four decimals `chain()` rounds to). They are plain oscillators with no `fm` and no `steps`, as in Fizz 1: its curves are chains of short notes, each its own 45 ms envelope, which `chain()` in `assets/sfx/voice.ts` writes the same way. The fzzt is Fizz 1's noise burst through a 3 kHz high-pass and its c6 sine that slides down an octave in 80 ms, so `SfxVoice` gained Fizz 1's `hpf` and `slide` and `web/sfx.ts` hands them to Undertone's `.hpf()` and `.slide()`.
- **Ids 15, 16 and 18 are the alternates**: the first Fizz 2 boing (pitch steps and FM), the FM-era jump (one square gliding on pitch steps) and the first Fizz 2 fzzt (a noise burst and an FM sine on pitch steps). Their data is what the repository had before, byte for byte for the jump and the fzzt. No event plays them, and the picker lists them: choose "15  boing (FM)", "16  jump (FM)" or "18  fzzt (FM)" under "Hear an effect" to compare. On DOS they are the downmix of their Web voices, so the owner can compare them with Fizz 1 DOS's versions; they cost 1,248 bytes of the OPL list and 140 of the speaker list.
- **To switch an event to an alternate**, change the id it maps to in `presentation` in `crates/fizz-slice-view/src/captions.rs`: `Event::Bounce` plays `EFFECT_BOING` (12), so `EFFECT_BOING_FM` (15) is a one-line change; `Event::Jump` plays `EFFECT_JUMP` (13) and `EFFECT_JUMP_FM` (16) likewise; `Event::Fire` plays `EFFECT_FZZT` (4) and `EFFECT_FZZT_FM` (18). Both presenters read that table, so DOS and the Web change together. The replay's AUDIO counts in `tools/check-dos.sh` and the counts in `captions/tests.rs` (`the_replay_plays_these_effects_this_often`) then move, and a gate says by how much.
- **Id 17** is `EFFECT_SODA`, the can pickup (`Event::Can`). It is not an alternate: an event plays it. It sits between the alternates because the effect ids are indexes with no gaps (`EFFECT_COUNT` is the length of every table and an id out of range wraps) and the slot was already there, held by a placeholder blip, when the soda cans were added.

### What DOS plays for the fzzt, the boing and the jump

DOS plays Fizz 1 DOS's own records and speaker lists for ids 4, 12 and 13, not a downmix of the Web's Enhanced voices. They are in `assets/sfx/fizz1-dos.ts` (generated from the port's `m4/assets/sound.bin` by `tools/sfx/extract-fizz1-dos.ts`, with provenance in its header and a note on the layout), and `tools/sfx/downmix.ts` writes them into `data.rs` in place of its converter's output for those ids, in the same record layout (`OplStep`: register, value, wait, bank 0), so the engine, the OPL3 pair (the same chunk on channels 5 and 14, the carrier level and output bit rewritten as for any effect) and the speaker player play them like the rest. Every other id, the alternates included, is still the downmix.

Why: the owner heard the DOS pogo sound like the FM one. It was: the converter takes the Web's Enhanced boing, sixteen short notes 25 ms apart, and lowers them as one gliding note. A note that starts while the previous one of the same patch is still sounding is a continuation (a pitch move, no key-off and key-on, which is right for a cluck's overlapping notes), so the sixteen notes became one key press with 53 pitch changes, where the FM boing is one key press and 42 (and three patch loads): the same shape, a smooth wobbling note. Fizz 1 DOS plays something else, because it converted Fizz 1's Classic table, not the Enhanced one. The converter was not wrong for chains: given Fizz 1's Classic voices it makes nearly the records the port does (jump 72 records against 73, boing 79 against 81), so the two differ in their source voices (three Classic notes against a sixteen-note Enhanced chain), and the Web's Enhanced voice cannot be downmixed to the Classic result. So the data is taken as the port made it, not approximated.

What each plays, as register writes (channel 5, bank 0; "key-ons" are key-on edges, each a fresh attack, and the pitch count is the number of times the low F-number byte changes):

| Effect | Version | Records | Patch loads | Key-ons | Pitch changes | Level writes | Ticks (ms) | Speaker frames sounding |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| fzzt | the FM downmix (id 18) | 69 | 1 | 1 | 12 | 18 | 100 (179) | 12 of 16 |
| fzzt | Fizz 1 DOS (id 4) | 198 | 13 | 13 | 13 | 5 | 96 (171) | 10 of 16 |
| boing | the Enhanced downmix (before) | 185 | 1 | 1 | 53 | 56 | 248 (443) | 45 of 56 |
| boing | the FM downmix (id 15) | 186 | 3 | 3 | 42 | 39 | 252 (450) | 34 of 42 |
| boing | Fizz 1 DOS (id 12) | 81 | 1 | 3 | 2 | 20 | 112 (200) | 14 of 18 |
| jump | the FM downmix (id 16) | 57 | 1 | 1 | 11 | 14 | 84 (150) | 8 of 12 |
| jump | Fizz 1 DOS (id 13) | 73 | 1 | 2 | 2 | 19 | 104 (186) | 14 of 18 |

Fizz 1 DOS's fzzt is a different kind of sound: its converter plays the voices that start together "taking turns" frame by frame, so the noise burst and the sliding sine alternate for the 13 frames they overlap, which is 13 patch loads and 13 key-ons (a rapid buzz that is the "fzzt"), against the FM version's single smooth falling note. The boing is three separate struck notes (g3, g4 and d5, a key-off and key-on each) with no wobble, against a wobbling single note. The jump is two struck notes e4 and b4, a fifth apart, against a smooth glide. The boing and the jump have no pitch sweep (two pitch changes each); only the fzzt's sine still slides. Each is a few distinct notes, and the speaker lists follow (jump and boing change divisor 7 and 8 times, the FM ones 11 and 28).

The busiest tick of every effect is still its patch load of 11 writes (22 on the pair), and no other tick writes more than two; the fzzt has 13 loads, each on its own tick, a frame apart, so it never has two in a tick. That is inside the 16 writes a tick the music is held to on its own channels, and the OPL3 pair is the same chunk on both channels.

**Why DOS and the Web can differ.** The Web plays Fizz 1's Enhanced voices and DOS plays Fizz 1 DOS's Classic ones, as Fizz 1 itself did on each platform (the Web's Enhanced mode against the DOS port's table). So for these three the platforms are not the same sound at different quality; they are Fizz 1's two own versions. Everything else still follows the rule that DOS is a downmix of the Web's source.

**How the other eight compare.** A read-only comparison of our downmix of stun, bonk, hurt, win, key, clunk, plink and crunch with Fizz 1 DOS's version of the same sound (nothing was changed for them):

| Effect | Ours (records, patch loads, key-ons, pitch changes, ticks) | Fizz 1 DOS | In character |
| --- | --- | --- | --- |
| stun | 75, 1, 1, 21, 112 | 165, 10, 10, 17, 96 | Different: Fizz 1 DOS alternates a noise puff and a square every frame (10 key-ons), a sputter; ours is one wobbling note |
| bonk | 74, 2, 2, 12, 88 | 115, 6, 6, 10, 84 | Somewhat different: the same length and fall, but Fizz 1 DOS takes turns between the click and the knock for several frames |
| hurt | 223, 2, 2, 69, 336 | 213, 1, 2, 70, 336 | Close: the same two voices, the same length and fall; ours has a second patch load |
| win | 317, 3, 3, 5, 432 | 299, 1, 5, 5, 432 | Close in length and notes (four struck notes and a long one); ours differs in tone for the two FM bells |
| key | 166, 2, 2, 3, 220 | 157, 1, 3, 3, 220 | Close: three notes; ours has a bell patch for the last |
| clunk | 95, 2, 2, 5, 116 | 224, 14, 14, 14, 112 | Different: Fizz 1 DOS alternates the brown noise and the thud every frame, a rattle; ours is a noise breath then one sagging thud |
| plink | 39, 1, 1, 1, 56 | 39, 1, 1, 1, 56 | Identical (an equal record list) apart from a glint of FM that the chip patch drops |
| crunch | 118, 3, 3, 3, 136 | 163, 7, 8, 8, 136 | Different in texture: Fizz 1 DOS flutters the noise and the two notes (8 key-ons); ours is a noise burst and two clean notes |

So hurt, win, key and plink are close to Fizz 1 DOS; stun, clunk and crunch (and bonk, a little) differ in character, because Fizz 1 DOS's turn-taking on voices that start together makes a rapid alternating buzz, where our downmix keeps one voice at a time and lets the louder win a tie. Whether DOS should use Fizz 1 DOS's data for all of them is the owner's decision.

### FM voices and pitch steps

The effects use two Undertone features that Fizz 1's effects (written for Undertone 0.2) did not have, and `SfxVoice` carries them as plain data:

- `fm: { ratio, index, decay?, wave? }` is Undertone's `fmVoice` definition in Tone.js `FMSynth` terms: the modulator runs at `ratio` times the voice's pitch (Tone's `harmonicity`), swings it by `index` times the voice's pitch (Tone's `modulationIndex`; Undertone divides it by the harmonicity on the way through), and falls to nothing over `decay` seconds (a `modulationEnvelope` with sustain 0). `web/sfx.ts` builds it with `fmVoice()` and `.voice(patch)`, then the voice's own envelope and filter on top.
- `steps` is Undertone's `pitchsteps`: semitone offsets from the voice's pitch, one for each 1/140 s (the classic AdLib effect frame, and DOS's own), holding the last. A voice with `steps` is one note that moves. `glide()` and `wobble()` in `voice.ts` build the fall and Fizz 1's Enhanced wobble.

The downmix lowers them as well as one voice at a time allows. A pitch step already is a 1/140 s frame, so the chip and the speaker follow it exactly (no interpolation). An FM voice becomes a patch of its own (`fmPatch` in `tools/sfx/patches.ts`): the carrier of the voice's oscillator, a modulator at the nearest multiple the chip has (0.5, 1 to 10, 12 or 15; the nearest on a log scale, so 2.76 becomes 3 and 3.5 becomes 4), a modulator level from the index (6 dB, eight 0.75 dB steps, to a doubling of the swing) and, when the FM decays, a percussive modulator whose decay rate is read off a rough rate table to clear in about that time. This is a best effort and not an exact mapping: Undertone modulates frequency by an index times the modulator's frequency, the chip modulates phase by a level set in steps, and nothing was compared by ear. The proper mapping (`fromOpl` and `toOpl`, feedback, a strict mode) belongs to the companion OPL package in upstream Undertone issue 25 and the music milestone in the roadmap.

Three more things differ from the cluck's downmix:

- **A level floor.** The converter measures each frame's level against the effect's loudest voice, so every effect fills the channel. A floor of 0.25 on that reference keeps a quiet effect (the jump at 0.2) a little quieter than a loud one. No cluck variant is below it, so their data did not move.
- **No new Web-only layers.** The extra voices (the bell's carrier under its FM, the d5 sine on the boing) are lost on DOS because the chip sounds the voice that started last; the loss column above says which.
- **Fizz 1's `hpf`** is a field of `SfxVoice` (with `slide`), which the Web hands to Undertone and the fzzt (4) uses; the FM-era voices (the stun, the fzzt alternate 18) still use a short white noise burst with a high low-pass cutoff instead, and the downmix ignores `hpf` and `slide`. On DOS the fzzt is Fizz 1 DOS's data, so the downmix never sees them.

### Writes per tick and sizes

The busiest tick of every effect is its patch load: 11 register writes on one channel (the patch's operators and feedback), 22 on the OPL3 pair. Everything else is at most two writes a tick (a level and a pitch, or a pitch and a key bit, spread over a frame's four ticks). The music is held to 16 writes a tick on its own channels ([MUSIC.md](MUSIC.md)), so music with a pair patch load on the same tick is at most 38 writes, about 1.0 ms of the 1.79 ms tick on the documented settling times (an estimate for an ISA bus, not a measurement on a 486; DOSBox charges no time for port I/O). `/BENCH /AUDIO` prints the most writes any tick made with music and effects together (`MAX_WRITES_PER_TICK`).

| Id | Effect | Records | Ticks | Average writes a tick | Patch loads (11 writes) | Most in any other tick |
| --- | --- | --- | --- | --- | --- | --- |
| 0 to 3 | cluck variants | 178, 74, 147, 150 | 212, 88, 188, 200 | 0.84, 0.84, 0.78, 0.75 | 3, 2, 4, 4 | 2 |
| 4 | fzzt | 198 | 96 | 2.06 | 13 | 2 |
| 5 | stun | 75 | 112 | 0.67 | 1 | 2 |
| 6 | bonk | 74 | 88 | 0.84 | 2 | 2 |
| 7 | hurt | 223 | 336 | 0.66 | 2 | 2 |
| 8 | win | 317 | 432 | 0.73 | 3 | 2 |
| 9 | key | 166 | 220 | 0.75 | 2 | 2 |
| 10 | clunk | 95 | 116 | 0.82 | 2 | 2 |
| 11 | plink | 39 | 56 | 0.70 | 1 | 2 |
| 12 | boing | 81 | 112 | 0.72 | 1 | 2 |
| 13 | jump | 73 | 104 | 0.70 | 1 | 2 |
| 14 | crunch | 118 | 136 | 0.87 | 3 | 2 |
| 15 | boing (FM) | 186 | 252 | 0.74 | 3 | 2 |
| 16 | jump (FM) | 57 | 84 | 0.68 | 1 | 2 |
| 17 | soda | 157 | 200 | 0.79 | 3 | 2 |
| 18 | fzzt (FM) | 69 | 100 | 0.69 | 1 | 2 |

`bun tools/sfx/downmix.ts --stats` prints the records, bytes, length and busiest tick of each effect, and a unit test of `fizz-sfx` fails if any tick writes more than 11 registers.

## Overlap, priority and the music: what Fizz 1 does, and what Fizz 2 copies

Read from Fizz 1 at `4bce036` (the main game, `packages/engine/src/audio.ts` and `episodes/episode-1/src/game.ts`, and the DOS port `m4/c/sound.c` and `m2/dos-rt/src/shell.rs`). Fizz 2 copies each rule.

| Question | Fizz 1 | Fizz 2 |
| --- | --- | --- |
| Does an effect cut another off? | On the Web, no. `Audio.play` builds a fresh Undertone stack for every call and plays it on the context, so effects overlap freely; there is no slot, no limit and no priority. In the DOS port there is one effect slot: `snd_fx` writes one volatile word, `want_fx`, and the 560 Hz handler's `take_requests` keys off channel 5 and starts the new stream, so the last request wins and cuts whatever was playing. There is no exception for any effect. | The Web plays each event's effect as its own sound. Where Fizz 1 has no limit, the page keeps a cap of six at once so a pile-up cannot clip, and whoa! (hurt) and ta-da! (win) bypass it. DOS has one slot (`audio::effect` stores one word) and the last request in a tick wins; a later effect cuts an earlier one off. A shot can therefore cut off whoa! on DOS only if it comes after it in the same step or later, as in Fizz 1. |
| Does the 180 ms dedupe apply to sound? | No. `Game.caption` calls `audio.caption(text, at)` first and only then checks `now - seen < 180` for the text; the haptics are triggered before it as well. The DOS port calls `snd_caption` for every caption event before its text dedupe (`caption(s, a, b, c)`). | No. The dedupe (11 ticks) lives in `EventCaptions::observe`, which `event_sound` does not call. A test plays the replay and finds one effect for each sounding event and the same effects whatever the Captions setting. |
| Does the click play on the press or on every held tick? | On the press. `World::fire` runs from the input edge (`e & FIRE`, with `edge = held & !prev_held`), so a held Fire clicks once. Each press with no ammo raises "click — no fizz" and its sound. | On the press: the rules emit `Event::Dud` from the press edge, and the replay holds one `dud`. |
| What does ta-da! do to the music? | Nothing at the exit itself. The sim sets `won`, freezes Ben and raises the caption and `LEVEL_COMPLETE`; the music is chosen from the level and the screen (`musicFor`), so the level's song keeps playing while ta-da! sounds. 700 ms later the "cleared" card shows and the mix goes to its `card` setting (a low-pass at 2,200 Hz and 0.8 gain); the track changes to the map's when the player goes back. The DOS port likewise keeps the level's song until the map. | Nothing. The slice has no result card, the music is a view of the game and nothing in it listens for the exit, so the song plays on and ta-da! sounds over it on the effects channel (5, with 14 for the pair). |
| Is a sound placed? | Yes, in Enhanced, by the caption's position (`placeSound`); Classic ignores it. | The same, once, in `fizz_sfx::place`. |

The consequences to know: on DOS a head-bump that follows a jump and a snack within a few ticks (the playground's nook has all three in a row) plays only the last effect's start; that is Fizz 1's behaviour and the AUDIO counts in `tools/check-dos.sh` depend on it (the replay starts 61 effects for its 62 sounding events: a shot and the stun it causes share a tick, and the stun wins).

## Hearing the effects

Nobody can hear them headlessly: the checks prove what is scheduled and what is written, not how it sounds. The owner's listen of 29 October 2026 covered the Web.

**On the Web**, press H (or the ? button) to open the panel. Under the hint there is a list, "Hear an effect", with all nineteen in id order (boing (FM) 15, jump (FM) 16 and fzzt (FM) 18 are the alternates no event plays, and 17 is the soda), and a Play button. Choosing an entry plays that effect on its own, centred, at full level; the arrow keys on the list play each as they move, and Esc hands the keyboard back to the game. It makes the sound with the page's own player (first key or click, the M mute and the hidden-tab rule all apply). `?audio=classic` changes nothing for it, since it is always centred.

**On DOS** there is no picker. The effects play where the events happen in the level, and `FIZZ2.EXE /REPLAY` (or a run of `/LIVE`) plays the whole route through the chip or the speaker the machine has.

**Where to hear each effect in the level** (the route the replay takes, from the bottom of the left shaft to the exit):

- Jump (13) on every jump from the ground, from the first. Fzzt (4) on every shot, from Space; the sixth press on the gallery with no ammo left is the click (plink, 11). A shot that stuns the chicken plays fizzled (stun, 5) in place of the fzzt on DOS (same tick) and after it on the Web.
- Whoa! (hurt, 7) on the spikes at the start; key (9) at the red key on the chicken's ledge; bonk (6) on a stomp on the chicken; clunk (10) when the red door opens and again at the blue one, and at every head-bump under the nook's low ceiling.
- In the snack nook at the top: soda (17) at the can on the floor by the right-hand wall, once the five shots are spent (and again at the shelf's can); crunch (14) on each snack, key (9) at the blue key, boing (12) on the pogo bounces, and ta-da! (win, 8) at the exit.
- The cluck (0 to 3) as before.

What to expect, as a listener (these are intentions, not results): fzzt is Fizz 1's fizz, a burst of noise over a sine that slides down; fizzled is a short wobbling sputter; bonk a deep round knock; whoa! a falling "wooo" over about half a second; ta-da! the longest, a rising four-note flourish ending on a long bell-like note; key three quick rising notes; clunk a low thud; click a tiny high tick; boing (Fizz 1's) a ragged, stepped wobble that fades over about 0.4 s, with a quick bright blip near the start; jump Fizz 1's short stepped rising chirp (the smooth FM one is the alternate, id 16); crunch a noisy crackle with two quick notes. On DOS each is one voice, so the layers and the noise are thinner and the FM bells are plain tones; the fzzt, the boing and the jump are Fizz 1 DOS's own (a buzzing zap, three struck notes, two struck notes).


## Where a sound is placed

A chicken on the left sounds as if it is on the left, and a faraway chicken is quieter. The picture and the six steps below show how. The rule is Fizz 1's (`packages/engine/src/sound-field.ts`, `placeSound` and `FIELD`, read only), and it is implemented once, in `crates/fizz-sfx/src/place.rs`, as a pure function of whole numbers that the host tests, the Web page and the DOS program all call.

![How a cluck is placed on screen: where the chicken is, the level each ear gets, and how the two levels reach each platform](images/positional-sound.svg)

A chicken walks across the screen and back, and the level markers follow it. The panels show where the chicken is, how loud each ear is at every position, and how the two levels reach the OPL3 pair, an OPL2 card, the Web and the PC speaker. Some readers show an SVG as a still image and ignore its animation and its reduced-motion script: [the static PNG](images/positional-sound.png) is the fallback. `tools/docs/positional-sound.py` draws the picture and `--check` keeps it current.

1. **The rules only say that it happened.** The rules raise `Event::Cluck { variant, chicken }`. They never see the screen or the sound, so the state hash cannot change, whatever the window size or sound card.
2. **The presenter asks where that chicken is.** It takes the chicken's position, the camera's position and the size of the view, and measures how far out the chicken is in half-screens: `dx` sideways (0 is the middle, 1 is the screen's edge, 2 is a whole half-screen beyond it) and `dy` up and down.
3. **Pan comes from the sideways distance only.** `pan = 0.6 x dx`, never past 0.6. So even at the screen's edge the far ear is only about 10 dB quieter, not silent. A chicken never plays in one ear alone.
4. **Loudness comes from the further of the two distances.** It is 1 while the chicken is within one half-screen, then it loses 0.5 per half-screen, never below 0.4. Far away is quieter but never lost.
5. **The two ears get their share.** The split is equal power, so the middle is no louder or quieter than the sides: `left = cos x loudness` and `right = sin x loudness`.
6. **Each platform plays it its own way.** On an OPL3 Sound Blaster the same effect plays on channel 5 (left) and channel 14 (right), each at its own level in 0.75 dB steps. An AdLib or OPL2 card has one voice, so it plays channel 5 in both speakers and only the loudness changes. The Web uses a stereo panner and a gain. The PC speaker has no volume control, so it ignores placement. Classic mode plays everything dead centre at full level.

**Worked example.** A chicken one half-screen to the left (point B in the picture) has `dx = -1`, so the pan is -0.6 and it is just inside full loudness. The left ear gets 0.95 (about -0.4 dB) and the right ear 0.31 (about -10 dB). A chicken two half-screens to the right (point E) has pan 0.6 but loudness 0.5, so the right ear gets 0.48 and the left ear 0.15.

The position is fixed when the cluck starts: a walking chicken does not glide across the speakers during one cluck.

### The formula

The whole rule in one place:

```text
dx = (x - cam.x) / half.w          dy = (y - cam.y) / half.h
d    = max(|dx|, |dy|)
pan  = 0.6 * clamp(dx, -1, 1)
dist = 1 if d <= 1, else max(0.4, 1 - 0.5 * (d - 1))
left = dist * cos(a)    right = dist * sin(a)    a = (pan + 1) * PI / 4      (equal power)
```

`cam` is the middle of the view and `half` is half the view's size, so the function is given the source's pixel position and the view (its top left and size) and works the offsets out as ratios. The source is the middle of the chicken's body, the camera is the one the picture is drawn with (the shaken one, when shake is on). The view is half of the formula: on DOS it is 320 by 200 (10 by 6.25 tiles), and on the Web it follows the window, so the same chicken pans less in a wider window. The view is for sound only. The rules never read it: `fizz-ep2` does not depend on any sound or view crate, `place` takes plain integers, and a test (`placing_sounds_never_changes_the_rules_the_hash_or_the_events`) replays the slice with a placement worked out for every cluck, each in a different view, and requires the golden transcript, hashes and `EVENT` lines unchanged.

**Why whole numbers.** The DOS crates deny float arithmetic and the result must be the same on the host, the page and the machine. The ratios are Q12 (4096 is 1.0). The equal-power gains come from a 129-entry Q15 cosine table (entry `i` is `cos(i * PI / 256)`, the right level is the mirrored entry), one entry for each 1/64 of the pan range, about 0.07 dB at the steepest; the OPL's own step is 0.75 dB. The dB step is found by comparing a level with 63 precomputed thresholds, each half a step (in dB) between two attenuations, so there is no logarithm either. The tests check both tables against `f64`, and the result against Fizz 1's formula in `f64` over a grid of positions and views (the levels agree within 0.006). The one deliberate rounding: the distance floor 0.4 is 1638 of 4096.

**What the formula reaches.** The pan is 0.6 of the way across at most, so Enhanced never plays hard left or hard right. At the view's edge the near side is 0.951 (-0.4 dB) and the far side 0.309 (-10.2 dB): the far ear is quieter by about ten decibels and never silent. The distance factor only dips below 1 outside the view, and stops at 0.4 (-8 dB). So a source far off one side plays at 0.38 and 0.12 (-8.4 and -18.1 dB). The hard extremes (pan -1 and 1, a silent far side) are never played by the game; the listening clips' "left" and "right" are the view's edges, which is as far as Enhanced goes.

### Data flow, per platform

The program (not the interrupt handler) works the placement out, so the 560 Hz handler only reads three small numbers.

```text
World::step -> Event::Cluck { variant, chicken }
  -> fizz_slice_view::cluck_placement(world, chicken, view_w, view_h, options)
       chicken's body centre in map pixels, the camera as drawn -> fizz_sfx::place::place(x, y, view)
  -> Placement { pan, distance, left, right }
```

| Platform | From the placement to the speaker |
| --- | --- |
| DOS, OPL3 pair | `audio::effect(id, &placement)` packs the effect id and three attenuations (left, right, single) in one atomic. The next tick starts `OplPair::same(id, left, right)`. Channel 5 writes go to ports 0x388 and 0x389 with output bit 0x10, channel 14's to 0x38a and 0x38b with 0x20, both with their own carrier level. |
| DOS, OPL2 or AdLib (`FX ch5`) | The single attenuation (the distance only; the pan is dropped) is added to the carrier level by `OplPlayer::mono`. The count of writes is unchanged: the level is a value in a write the effect makes anyway. |
| DOS, OPL3 with effects on AdLib (`FX ch5`, music on the Sound Blaster) | The same, then `route` ORs 0x30 into the config writes, so the one voice sounds in both speakers at the distance level. |
| DOS, PC speaker | The placement is ignored. The speaker is on or off and has no volume control, so distance does nothing and there is no pan. |
| DOS, `/NOSOUND` or no device | Silent. |
| Web, Enhanced | `App.take_effect()` returns the effect id and `effect_pan()` and `effect_gain()` give the pan (-0.6 to 0.6) and distance (0.4 to 1) of that effect. `Sfx.playEffect(id, pan, gain)` gives each Undertone voice an equal-power `StereoPannerNode` and scales its gain. Undertone already takes both per voice, so there are no extra nodes of ours and no fork. |
| Web, Classic | The pan and gain are ignored: centred, full level, the cached stack. |

On DOS the choice is the Sound placement row of the Options screen (Enhanced by default), saved in `FIZZ2.CFG`, and it applies at once. On the Web it is the Placement row of the Options screen, remembered in the `placement` field of the `fizz2-options` blob in `localStorage` (`classic` or `enhanced`); a `?audio=classic` or `?audio=enhanced` query decides for that visit without being saved. Classic is Fizz 1's Classic mode: everything centred at full level.

### Levels and the dB step

The attenuation is the OPL's 0.75 dB step. A level is rounded to the nearest step. Some positions (a 320 pixel view; `place` in `crates/fizz-sfx/src/place.rs`):

| Source | Pan | Distance | Left level, steps (dB) | Right level, steps (dB) | Single channel |
| --- | --- | --- | --- | --- | --- |
| View's left edge | -0.6 | 1 | 0.951, 1 (-0.75) | 0.314, 13 (-9.75) | 0 |
| Centre | 0 | 1 | 0.707, 4 (-3.0) | 0.707, 4 (-3.0) | 0 |
| View's right edge | 0.6 | 1 | 0.314, 13 (-9.75) | 0.951, 1 (-0.75) | 0 |
| 80 pixels past the right edge (dx 1.5) | 0.6 | 0.75 | 0.235, 17 (-12.75) | 0.713, 4 (-3.0) | 3 (-2.25) |
| 160 pixels past it (dx 2) | 0.6 | 0.5 | 0.157, 21 (-15.75) | 0.475, 9 (-6.75) | 8 (-6.0) |
| Far off (dx 2.2 or more) | 0.6 | 0.4 (the floor) | 0.125, 24 (-18.0) | 0.380, 11 (-8.25) | 11 (-8.25) |

The steps, in general: `steps = round(-20 * log10(level) / 0.75)`, 0 to 63.

### What each path loses

- **Pair:** pan at about 64 positions and the equal-power law, but only to the 0.6 width; one level for the whole effect, fixed when it starts; the sum of two FM voices, not a stereo recording of one.
- **OPL2 and AdLib:** the pan; the distance remains, as a carrier level (and the modulator, so the timbre, is unchanged).
- **PC speaker:** pan and distance both.
- **Web:** nothing the formula says; the level at the start of the sound only (the chicken does not move the sound once it plays).

## DOS playback

`dos/rt/src/audio.rs` is the DOS audio service. `audio::start(&Resolved)` hooks IRQ 0 at 560 Hz (`timer::hook_560hz`, PIT divisor 2131, 559.9165 Hz) and installs a tick hook that the timer handler calls on every interrupt. The BIOS accumulator is the one `hook_60hz` used, now adding 2131 a tick: the BIOS handler runs on every 30.75th interrupt. The simulation is still paced by the integer `Pacer`, which reads the 560 Hz tick count as a clock of 559,916 millihertz and releases 60 steps a second. `audio::effect(id, &placement)` stores the effect id and the placement's attenuations in an atomic (the game calls it for every event with a sound, in `FIZZ2.EXE`'s `play_events`, with the placement of that chicken), and the handler takes it up on the next tick; a second cluck replaces one still sounding (last wins). The handler allocates nothing, calls no DOS service and uses no x87. What it does with the cluck depends on the resolved plan (`fizz_machine::Resolved`, see [`DOS-CHECK.md`](DOS-CHECK.md)):

| Plan | Device | What the service does each tick |
| --- | --- | --- |
| `FX ch5`, chip `Opl2` | AdLib or Sound Blaster OPL2 | `OplPlayer::mono(id, distance)` once per tick; each `(bank, reg, val)` is written to ports 0x388 and 0x389. |
| `FX ch5`, chip `Opl3` (music on the Sound Blaster, effects on AdLib) | OPL3 in its own mode | The same, with `fizz_machine::route` on every write as an OPL2-format stream: a bank-0 write to registers 0xC0 to 0xC8 gets bits 4 and 5 set, so the mono voice sounds in both speakers. |
| `FX ch5+14`, chip `Opl3` | OPL3, both banks | `OplPair::same(id, left, right)`: channel 5 (ports 0x388 and 0x389, output bit 0x10) and channel 14 (bank 1, ports 0x38A and 0x38B, output bit 0x20), each at its own carrier level; it goes through `route` as an OPL3-aware stream, which leaves its output bits as authored. See "The OPL3 pair". |
| `FX speaker` | PC speaker | `SpeakerPlayer::new(id)` stepped on every fourth tick (140 Hz): a divisor is written to PIT channel 2 (command 0xB6, port 0x42) and bits 0 and 1 of port 0x61 are set, `Off` clears them. |
| `FX off`, or no device | none | The service runs and does nothing, so the clock is the same in every case. |

**Start-up and shutdown.** When the plan uses the chip (`opl` is not `None`), `start` keys off every channel of bank 0 and the rhythm register and, for an OPL3, first writes 0x105 = 1 and 0x104 = 0 and keys off bank 1. `audio::stop` is on the clean-up list, so it runs at exit and in the panic handler: it keys off both banks, writes 0x105 = 0 so an OPL3 leaves OPL3 mode, and closes the speaker gate. The timer's own clean-up puts the PIT and the vector back. A write to the chip is the register port, six status reads, the data port and 35 status reads (Fizz 1's settling times), so the busiest tick of an effect, the patch with eleven writes, takes roughly 40 port reads each.

Music is played by the same service (the song source, the streams and the DOS service are in [MUSIC.md](MUSIC.md)): each tick it steps the music first, then the effect, through the same register write with the same 6 and 35 status reads, so a tick that carries both takes the sum of their writes. Channels 5 and 14 are the effects' whatever the music does, and the music never writes them. Music writes are not counted in the `AUDIO` line, and while a menu is up the music's operator levels are offset (see [The screen mix](#the-screen-mix)). The placement is worked out in the program from the chicken and the camera and is for sound only (see [Where a sound is placed](#where-a-sound-is-placed)).

The run's `AUDIO WRITES <n> SPEAKER STEPS <m>` line counts every OPL register write the service made (the start and stop key-offs included) and every speaker step. `tools/check-dos.sh` runs `/REPLAY` under an OPL3 Sound Blaster, a Pro 2, an OPL2 Sound Blaster, an OPL2 with no Sound Blaster, no card and the flags, requires the golden `TICK`, `FINAL` and `EVENT` lines every time, and requires the counts that tell the paths apart (channel 5 alone, an OPL3 on the AdLib role, and the pair, which is twice channel 5's effect writes less one global enable per effect that starts), speaker steps on the speaker and zeros when silent.

### Changing devices while the game runs

`audio::replan(&Resolved)` moves the running service to a new plan, for the Options screen. It runs in the program, never in the interrupt: it clears the service's active flag (the handler then returns before it touches anything, and cannot be part way through a tick because it does not nest and this is the program), silences the old plan, stores a new service state built for the new plan into the static the handler reads, and sets the flag again. Nothing is allocated, the handler never sees a half-built state, and the few audio ticks that arrive in between are skipped (the timer and the BIOS clock go on). A plan equal to the one running changes nothing and returns `false`.

- When only the effects device changed and the chip mode and the music are as they were, the music keeps playing. The old effect is silenced (channel 5 of bank 0, and channel 14 of bank 1 when it was the pair, are keyed off; the speaker gate is closed when leaving the speaker) and the new device starts silent.
- Otherwise every channel of both banks is keyed off, the speaker gate is closed if the old plan used the speaker, OPL3 mode is switched off (register 0x105 = 0, after the key-offs) when the new plan has no OPL3 music and no pair, and switched on (0x105 = 1, 0x104 = 0, as `start` does) when it has. The music, if the program had asked for it with `music_start`, starts again from the first tick of its prelude on the new arrangement. The music's own counters (`MUSIC` line) start again with it; the `AUDIO` counters, the speaker steps and the handler tick count keep counting.
- The reserved-channel rules do not change: effects on channel 5 of bank 0 (output bit 0x10, left) and channel 14 of bank 1 (0x20, right), music on the others, and no music write to channel 5, channel 14, register 0x104 or 0x105.

`FIZZ2.EXE /REPLANTEST` (a hidden developer switch) walks eight plans through `replan` with a cluck sounding at each switch and prints a `REPLAN <from> -> <to> GATE0 <g> OPL3 <on|off> GATE <g> CHANGED <yes|no> MUSIC0 <t> WRITES <n> STEPS <n> TICKS <n> MUSIC <t>` line for each; [`DOS-CHECK.md`](DOS-CHECK.md) lists what `tools/check-dos.sh` requires of them. That the register writes happen in the right order and the counters keep rising is verified under DOSBox; nobody has listened to a live switch, on DOSBox or on real hardware, so whether a keyed-off voice cuts cleanly or clicks, and whether a real OPL3 leaves OPL3 mode without a glitch, is unknown.

### Reserved channels

Sound effects own two OPL channels, numbered 0 to 17 with the second register bank's from 9, and music and scores must not use either:

| Channel | Bank and slot | Registers | Used by |
| --- | --- | --- | --- |
| 5 | bank 0, slot 5 | operators 0x0a and 0x0d (`0x2a`, `0x2d`, `0x4a`, `0x4d`, `0x6a`, `0x6d`, `0x8a`, `0x8d`, `0xea`, `0xed`), `0xa5`, `0xb5`, `0xc5` | the default effects channel everywhere: the whole effect on an OPL2 or the AdLib role, the left channel of the OPL3 pair |
| 14 | bank 1, slot 5 (9 + 5) | the same offsets plus 0x100: `0x12a`, `0x12d` and so on, `0x1a5`, `0x1b5`, `0x1c5` | the right channel of the OPL3 pair; only ever the extension that makes channel 5 stereo |

The constants are `fizz_sfx::EFFECT_CHANNEL`, `EFFECT_CHANNEL_RIGHT` and `RESERVED_OPL_CHANNELS` in the crate, and the same three in `tools/sfx/opl.ts`. A score-writing tool imports the list and refuses those channels. Channel 5 keeps its reservation whatever the card; channel 14 is the right half of the pair on an OPL3.

This costs a score one channel: the OPL3 list loses channel 14 (see [Scores from other tools](#scores-from-other-tools) for the rule a score-writing tool must follow). A second effect while one sounds cuts the first on both channels (last wins, as with every effect).

### The OPL3 pair

When the chip is an OPL3 in its own mode and effects are on the Sound Blaster (`FX ch5+14` in the `SOUND` line, the pair named by both its channels), the effect plays on two channels at once. Channel 5 (bank 0) is the left channel, as it is on every other path; channel 14 (bank 1) is the extension that makes it stereo, the right channel. Both play the same records:

| | Channel 5, left | Channel 14, right |
| --- | --- | --- |
| Bank and slot | 0, slot 5 | 1, slot 5 (9 + 5) |
| Config register | 0xc5: output bits exactly 0x10 | 0x1c5: output bits exactly 0x20 |
| Carrier total level | 0x4d: stored level + the left attenuation | 0x14d: stored level + the right attenuation |
| Everything else | as stored | as stored, except that the chip-wide waveform-select enable (register 0x01) is left to channel 5's stream |

`OplPair::same(id, left_attenuation, right_attenuation)` steps the two `OplPlayer::side` players together. The OPL3 mode was switched on by `audio::start` (register 0x105 = 1), so an effect adds no enable write. The attenuation is added to the carrier operator's total level, a 6-bit field in 0.75 dB steps (the key-scale bits above it are kept), clamped at 63 (47 dB: quiet, not silent). No effect is additive-connected (a test pins that), so the modulator, which only shapes the timbre, is left alone. The attenuation is chosen once, when the effect starts: a cluck does not follow a chicken that walks while it sounds. Centre is both channels at -3 dB (4 steps, 3.0 dB), so the sum matches one centred voice's power.

`route`'s OR rule (every bank-0 write to registers 0xc0 to 0xc8 gets 0x30) applies to OPL2-format streams only: the mono effect on channel 5 and the OPL2 music arrangement. The pair and the OPL3 music arrangement are OPL3-aware streams (`Stream::Opl3`), and `route` leaves their output bits as authored, because the OR would turn channel 5's left-only bit into both.

The pair is a hook for stereo-authored effects, which are not built (Milestone 4.5). Every effect today plays one record stream on both sides. `OplPair::new(left_steps, right_steps, ..)` already takes one stream for each side, and a stream's records carry a bank bit (`OplStep::bank`), so a later effect could store a different chunk for bank 0 and bank 1 and play different content on each side with no change to the player. Positional levels are power per channel, which is why the placement returns a level for each side (`Placement::left`, `right`, and `distance` apart for single-output paths) and each side's attenuation is applied to its own channel's carrier, so they keep working when the sides differ.

A Sound Blaster Pro 1, which has two separate OPL2 chips for stereo, is not supported; it is an OPL2 here.

### Cost on the 560 Hz tick

Each OPL write is the register port, six status reads, the data port and 35 status reads, about 43 port accesses, or about 26 microseconds at the chip's documented settling times. The pair doubles the writes of an effect (less the one chip-wide enable). The busiest tick of every effect is its patch, with 11 writes on one channel and 22 on the pair, about 0.3 ms and 0.6 ms of a 1.79 ms tick; with the music's 16-write ceiling on the same tick that is at most 38 writes, about 1.0 ms (see [Writes per tick and sizes](#writes-per-tick-and-sizes)). The handler stays bounded (at most two players, no loop longer than their records) and allocation-free, and the attenuation is a table lookup done in the program. The microseconds come from the documented delays, not from a measurement on an ISA bus.

`/REPLAY` counts the writes. The replay (1,757 ticks) has 70 sounding events on 69 distinct ticks, so the service starts 69 effects (it has one slot and takes the last request of a tick; a shot and the stun it causes share a tick). `AUDIO WRITES` is 7,224 on OPL2 channel 5 (20 start and stop key-offs and 7,204 records), 7,245 on an OPL3 on the AdLib role and 14,380 on the pair (the 41 start and stop writes and twice the 7,204 records, less 69 enables), and the speaker makes 1,358 steps. The replay fires 13 times (the soda cans refill the blaster twice) and Fizz 1 DOS's fzzt is 198 records against 69 for the FM one, its jump 73 against 57 and its boing 81 against 185; the soda is 157 records and 28 speaker frames. They were 5,936, 5,957, 11,812 and 1,207 (61 effects, the 1,681-tick replay) before the soda cans, 5,972, 5,993, 11,876 and 1,388 with the cans but the FM-era fzzt, boing and jump, 5,178, 5,199, 10,296 and 1,264 for the 1,681-tick replay with the FM-era voices, 5,221, 5,242, 10,382 and 1,213 before the boing and the jump were Fizz 1's at all, 569, 590 and 1,135 when only the four clucks of the same script played, and 419, 440 and 836 for the three clucks of the shorter script before the playground. The numbers are exact because the replay is deterministic and DOSBox runs at fixed cycles; two runs in a row gave the same figures. `tools/check-dos.sh` requires exactly those relations. The attenuation changes values and not the count, so `AUDIO WRITES` does not show it; the register stream of the left, centre and right pair is checked by the host tests of `OplPair`.

## The Web path

`web/sfx.ts` plays the rich effect. Undertone is the owner's package `@liminal-hq/undertone`, public on npm and MIT licensed, ES modules with no dependencies and no side effects. `package.json` pins exactly 0.3.0 (Fizz 1 asks for ^0.2.0), and `bun.lock` is committed. `tools/build-web.sh` bundles it into `dist/web/main.js` (see [`PLATFORM.md`](PLATFORM.md#the-web-build)). Undertone 0.3.0 honours every field of `SfxVoice`: an exact `hz` as a numeric pitch, `nudge`, the low-pass with its envelope, white, pink and brown noise, `fmVoice` FM and `pitchsteps`. The Web uses no OPL emulator and none is planned.

How the page gets from a rules event to a sound:

1. `App::frame` queues the effect of every event with a sound (`fizz_slice_view::event_sound`: the effect id from the `captions.rs` table and the placement of the event's point against the camera) after the step that emitted it; the page calls `take_effect()` until it returns -1 after every frame, so a frame that runs several ticks loses none. `web/main.js` plays each one, whatever the Captions setting says.
2. `Sfx.playEffect(id, pan = 0)` takes the effect's voices from `EFFECTS` (an id out of range wraps, as on DOS), builds an Undertone stack the way Fizz 1's `buildVoice` does with a sustain of 0, caches it per variant and plays it once on the page's `AudioContext`.
3. The context is made on the first key press or click and resumed. Nothing is made before that, and an effect that arrives before then, while muted, while the tab is hidden, while the context is not running, or with six effects already sounding (whoa! and ta-da! are never dropped for that), is dropped, never queued.
4. `M` toggles mute and the Options screen's Sound row shows it. The choice is kept in the `sound` field of the `fizz2-options` blob in `localStorage` (`off` or `on`; `web/options.ts` writes it inside try/catch and migrates the older `fizz2-sound` key), so blocked storage only means it is not remembered. The page makes `Sfx` with no storage of its own; `Sfx` itself still reads and writes `fizz2-sound` and `fizz2-audio` when given some, which `web/sfx.test.ts` covers. A hidden tab suspends the context and a shown one resumes it.
5. Enhanced placement. `App` records the placement of each cluck when it queues it (the chicken's position after the step, the camera and the window-sized view the page last reported). After `take_effect()` returns an id, `effect_pan()` (-1 to 1, within +-0.6) and `effect_gain()` (1 to 0.4) hold its placement, and `playEffect(id, pan, gain)` builds the stack with a per-voice `.pan(pan)` (Undertone makes a `StereoPannerNode`, which is equal power) and `.gain(g * gain)`. A centred full-level stack is cached; a placed one is built each time. Classic mode passes 0 and 1 whatever the page says. The pan and gain come from the same pure function as DOS; the Web does not recompute them.

One mapping differs from Fizz 1. `SfxVoice.lpenv` is the cutoff the filter opens to, and the WAV preview and the downmix read it that way, but Undertone adds the envelope amount to `lpf` (0.2 and 0.3 alike; `web/music.test.ts` and `web/sfx.test.ts` pin it). The page passes `lpenv - lpf`, so the Web filter peaks where the preview does. Fizz 1 passes `lpenv` unchanged, so its filters open further than their definitions say.

Tests: `bun test web/sfx.test.ts` runs the real Undertone against a recording stand-in for `AudioContext` and checks the mapping, the first-gesture rule, mute, the hidden tab, the cap, the panner and gain of a placed cluck, Classic, and the remembered choice and the query. `python3 tools/check-web-sfx.py` (Chromium, after `tools/build-web.sh`) injects the same kind of stand-in before the built page loads and checks that the cluck at tick 197 schedules variant 2's oscillators after a key press and nothing before one, and that `M` mutes and the choice survives a reload. Both prove what the page asks the browser to play; neither hears it.

## The screen mix

When a screen holds the game, the music changes character: it is muffled and quieter, and it comes back when the game resumes. This is Fizz 1's screen-driven mix, not a duck under sound effects (neither Fizz 1 nor Fizz 2 ducks the music under an effect; see [MUSIC.md](MUSIC.md)). It is global and follows the screen, so it cannot depend on which effect plays.

**What plays, on the Web.** Opening Paused or Options closes a low-pass filter on the music to 900 Hz and lowers its gain to 0.7 over 0.18 s; closing the last screen opens both again over 0.35 s. It closes fast so the muffle is there as the menu appears, and opens slowly so the music comes back instead of snapping. Paused to Options (and back) is the same mix, so nothing moves. The cutoff glides exponentially (pitch-like) and the gain linearly, both arriving together. Each change starts from where the last glide had got to, so toggling quickly never jumps and never gets stuck; it is the pattern of Fizz 1: `cancelScheduledValues`, then `setValueAtTime` at the current value, then the ramp (no `cancelAndHoldAtTime`, which Firefox lacks). The filter's Q is 0 dB, flat at the cutoff with no ring, and open means the context's Nyquist frequency, where a biquad low-pass is transparent (at 20 kHz it is not). While the context is suspended (the page muted or hidden) the change is made at once with no ramp, so it is already in place when sound returns.

**The table.** The names and numbers live in one place, `MIX` in `web/mix.ts`, and the DOS side should take the same names and numbers (a cutoff in Hz, a gain as a linear multiplier, and the two ramp times).

| Screen | Cutoff | Gain | Where it comes from |
| --- | --- | --- | --- |
| `play` | open (Nyquist) | 1 | the music as it is |
| `pause` | 900 Hz | 0.7 | Fizz 1's pause |
| `options` | 900 Hz | 0.7 | Fizz 1 muffles everything opened from Pause |
| `card` | 2,200 Hz | 0.8 | Fizz 1's cleared and game-over cards; reserved, nothing asks for it yet |

Ramps: 0.18 s to close (a lower cutoff, or a lower gain at the same cutoff), 0.35 s to open. A later screen is one more line in `MIX` and one place that names it (`web/main.js` reads the screen each frame and calls `AudioRoot.setScreen`).

**Music only.** The mix sits on the music's way to the limiter (`music bus -> low-pass -> mix gain -> limiter`). The effects bus has no filter and no automation, so a menu sound or a cluck during a pause is unfiltered. The music is what a screen should recede behind; an effect is a message and should stay clear. The Mute and Motion settings do not change it: a filter ramp is sound, not movement, and a muted page simply suspends the context. The rules, the replay and every hash are untouched.

**Not built.** The `card` row is a table entry only: Fizz 2 has no cleared or game-over card. Fizz 1's dialogue mix (open, gain 0.7) and its cinematic mix (open, gain 0.85) are not listed, because there are no dialogue or cinematic screens. Fizz 1's phone variant of the pause (1,400 Hz, for small speakers) is not built either. Classic placement does not turn the mix off here, as it does in Fizz 1. DOS has no screen mix yet: an OPL has no mixer, so it would be a carrier level change on the music channels, staggered across ticks so no tick overruns its write budget. Nobody has listened to this; the numbers are Fizz 1's and were tuned there, not here.

Tests: `bun test web/mix.test.ts` records the filter and gain automation (open, close, rapid toggling, suspended context, the effects bus untouched), and `python3 tools/check-web-sfx.py` observes the same automation in the built page when Esc opens Paused and Options and when they close.

### On DOS

An OPL chip has no filter and no volume for a group of channels, so DOS approximates "a low-pass at 0.7 gain" with the two controls the chip does have, the operators' total levels (registers 0x40 to 0x55 of each bank: bits 0 to 5 are the level in 0.75 dB steps, larger is quieter; bits 6 and 7, the key scale level, are kept):

| What | Change at full depth | Why this much |
| --- | --- | --- |
| Quieter: every carrier of the sounding music channels | +4 steps (3.0 dB) | 0.7 gain is -3.1 dB, and 4 steps is the nearest whole number of steps |
| Darker: the modulator of an FM channel (connection bit 0 of `0xC0 + channel` clear) | +6 steps (4.5 dB) | A weaker modulator bends its carrier less (a lower modulation index), and the index sets how much energy is in the upper partials. Halving the index (6 dB) would take a second partial down about 12 dB and a third about 18 dB, harder than a 900 Hz low-pass on a mid-range note, so 4.5 dB is the deliberately gentler choice. It is the constant to adjust by ear (`MODULATOR_STEPS` in `crates/fizz-music/src/mix.rs`) |
| An additive channel (connection bit 0 set) | both operators +4 steps | Both operators sound and there is no modulator, so there is nothing to darken; it only gets quieter |
| Channels 7 and 8 while the rhythm bit is on | both operators +4 steps | Each operator is a drum voice (hi-hat, snare, tom, cymbal) |

Every level is clamped at 63. Channels 5 and 14, registers 0x104 and 0x105, and every register that is not an operator level are never touched, and the effects are not changed: no effect write is altered, and no mix write is made on a tick where an effect (channel 5's player or the pair) is playing.

**How it is applied.** `fizz_music::MixState` keeps a shadow of the level the music authored for every operator it has written and the connection bits and rhythm bit, and `audio::set_mix(Mix)` (called by `dos/fizz2/src/menu.rs`: `Pause` when Paused opens, `Options` when Options opens, `Play` when the game resumes, on Quit to DOS and when `/OPTIONS` ends) chooses the target. In the music write path every authored write goes through `observe`, which updates the shadow and returns the value to send, so a note started or a patch changed while a menu is up is not un-muffled. A ramp variable of 12 units moves one unit per 9 ticks while closing (100 ticks, 0.18 s at 560 Hz) and one per 18 ticks while opening (199 ticks, 0.36 s), and a unit moves a level by at most one step (carriers every third unit, modulators every second), so a held note changes 0.75 dB at a time and does not click. The levels that differ from the ramp are sent by `step`, at most two writes a tick, round-robin over the operators, and only while the music's own writes on that tick leave room: music writes plus mix writes never pass 16, the cap that was there before. An effect playing holds the mix writes back (the ramp keeps moving and they catch up after). A full change of the OPL3 arrangement's 30 operators takes about 150 mix writes over the ramp, and the OPL2 arrangement's 14 about 70, one way. The authored hash (`MUSIC` line), `AUDIO WRITES` and the music write counts are of the authored stream and are unchanged by the mix, and with the mix at `play` and settled there is no extra write and no changed value. A device switch in Options (`audio::replan`) builds the new service with the offset already in force, so the new chip is muffled from its first note.

**What it will and will not sound like.** It is not a filter. The tone will be quieter by about 3 dB and rounder or duller on the FM voices, without a resonant peak or a clean roll-off at one frequency; additive voices and the drums only get quieter. The amount is a first guess from arithmetic, not from listening. Nobody has heard it, on DOSBox or on a real card. Each OPL clone and emulator treats a level change on a sounding note differently (a real chip applies it at once, which is why the steps are small), so a faint zipper or tick is possible on some; that is what to listen for.

**Checked by.** `FIZZ2.EXE /MIXTEST` (a hidden developer switch, see [`DOS-CHECK.md`](DOS-CHECK.md#mixtest)) plays the music's first ticks, switches to the paused mix, switches back and audits the shadow against the register state the writes left, under an AdLib, an SB16 pair, an SB16 with `/ADLIB`, an SB16 with `/MUSIC:ADLIB /FX:SB` and no card. `cargo test -p fizz-music` covers the ramp, clamp, key scale bits, additive and rhythm cases and the write budget.

## Music

Music has its own document, [MUSIC.md](MUSIC.md). In short: the song is an Undertone source (Fizz 1's "Crater Fields", a placeholder) that a builder lays out and a script lists note by note. The Web plays it through a music bus beside the effects bus and a limiter (#75). A generator lowers the song to OPL2 and OPL3 register streams (#76), a stepper plays them (PR #79, in review) and the DOS audio service steps them at 560 Hz beside the effects (the stack's top PR, in review); until those merge, `main` plays music on the Web only.

Channel allocation is settled and shared with effects. OPL2 music uses channels 0 to 4 and 6 to 8. OPL3 music uses `[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 15, 16, 17]`, thirteen channels. Channels 5 and 14 are reserved for effects on every card (see [Reserved channels](#reserved-channels)). Music is a view of the game, like positional sound: it adds no event and no game logic reads the music clock.

## Scores from other tools

Songs and effects can be authored in tools outside this repository. What the game requires of them is a spec, and nothing here depends on any particular tool.

The stream format for OPL and speaker effects and music, the proposed OPX, is specified in [OPX.md](OPX.md).

- Music source songs come from separately authored material: an Undertone source for the Web, and an OPL register stream for DOS. A register stream made by another tool can be imported through the stream spec. That import is not built.
- The reservations are a rule of the game's audio service, and any score-writing tool must respect them. Channel 5 is reserved for effects on every card. On an OPL3, channel 14 is reserved too, because effects use it as the right half of the pair (see [Reserved channels](#reserved-channels)). A tool that writes OPL3 scores must therefore use only the thirteen music channels listed above, `[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 15, 16, 17]`, and a list that includes channel 14 must drop it.
- The register numbers for bank 1 follow the chip's documented layout, and the five-byte OPL3 record (a register, a value, a wait and a bank byte) is the shape the game's player reads.

## Regenerate and check

```text
bun tools/sfx/downmix.ts           regenerate crates/fizz-sfx/src/data.rs
bun tools/sfx/downmix.ts --check   fail with a diff if the committed file is out of date
bun tools/sfx/downmix.ts --stats   print each effect's records, bytes, length and busiest tick
```

The CI parity job runs `--check`, so a source change committed without its regenerated data fails CI. The crate's tests check that every variant terminates, ends keyed off, writes only channel 5's registers in bank 0, stays in the audible divisor range and plays the same every time.

To listen, `bun tools/sfx/render-wav.ts <directory>` (it needs cargo as well) writes per effect id (files named `effect-<id>-...`): the rich source, an approximate OPL2 preview of the generated records, a square-wave speaker preview, four stereo OPL3 pair previews (`-opl3-pair-left`, `-centre`, `-right` for a source at the view's left edge, middle and right edge, and `-distant`, three half-views off to the right), and two distant sources, `-opl2-distant` (channel 5 at the distance floor, 11 steps) and `-rich-distant` (the Web's pan 0.6 and gain 0.4). All are approximations and none is committed.

- The rich file does not come from Undertone. Undertone renders through Web Audio, which Bun does not have, so the script has a small synth that follows Undertone's voice model (an oscillator or noise, a linear attack, a linear decay to a sustain of 0, a low-pass with a filter envelope). Its waves are naive, not band-limited, and the Web will sound like Undertone, not like this file.
- The OPL2 file is a two-operator FM model of channel 5 driven by the generated records, with the chip's feedback and level steps but a rough envelope-rate table and no key scaling, tremolo or vibrato. DOSBox's emulator or a real card will differ.
- The OPL3 pair files take the register writes from the real Rust player (the `stream` example of `fizz-sfx`) and run two copies of the one-channel model, channel 5 on the left output and channel 14 on the right. They show the pan and the levels, not the isolation of the pair from the music.
- The speaker file holds a square wave for each 1/140 s step. A real speaker and its case colour the sound.

The positional sound diagram has its own generator. `python3 tools/docs/positional-sound.py` writes `docs/images/positional-sound.svg`, `--check` fails with a diff if the committed file is out of date (`tools/check.sh` runs it), and `--png` also renders `docs/images/positional-sound.png` with `rsvg-convert` when it is installed.

## Provenance

The converter is the same author's code, MIT in `lieutenant-fizz-dos` and dual-licensed here: `tools/sfx/convert.ts` from `m4/sound/fizz-dos/sfx.ts`, `tools/sfx/opl.ts` from `opl.ts` and `tools/sfx/patches.ts` from `patches.ts`, all at commit `221dd6c`. Each file's header says what changed. The voice model and the idea of a pitch curve as a chain of short notes come from Fizz 1 (`packages/engine/src/audio.ts` and `episodes/episode-1/src/audio/enhanced-sfx.ts` at commit `4bce036`); the cluck's voices are written for this repository, and the voices of effects 4 to 14 are Fizz 1's `SFX` entries and Enhanced curves rewritten as Undertone data (both repositories are the same author's code, dual-licensed; the provenance header of `assets/sfx/effects.ts` says so).

## Not verified

- Apart from the owner's playtests of 28 October 2026, in which the cluck played in the builds and sounded good (one listener, not a measurement), nobody has listened to the cluck in any form: not the source, not the OPL2 chunk, not the speaker list. The WAV previews are approximations of each and are not evidence of how the game will sound.
- Nobody has heard any of the fifteen effects after the cluck, on the Web or on DOS, in any form: the checks show which oscillators the page schedules and which registers the chip is written. The FM voices are authored by their numbers; whether they sound like zaps, knocks and bells, how loud each is beside the music and beside the others, and whether the cap of six at once is ever reached are unknown. The downmix of an FM voice is a best effort that was not compared by ear. The listening guide above says what to expect.
- The OPL2 data has not run on an OPL2 chip or in an emulator, and the patches are untuned first guesses. The OPL3 pair has run in DOSBox's headless emulator only, where the writes are counted and the positions are never heard; the register numbers for bank 1 and the total-level attenuation rest on common OPL3 conventions and the chip's documented layout, not on a test against hardware. Whether the pan and the distance sound right, whether the -3 dB centre matches a single voice by ear, and whether Fizz 1's 0.6 pan width is enough on small speakers are all unheard.
- The Web plays the cluck, but that has been checked only against stand-ins for `AudioContext`. A real browser's autoplay policy, its hidden-tab behaviour, the overall loudness and any clipping (each voice's gain is at most 0.3, but the sum is not measured) have not been tried. The manual describes the Web's cluck and the DOS one.
- Nobody has heard the music in any form either, on the Web or in a lowered one; [MUSIC.md](MUSIC.md) lists what is unverified there.
