---
id: effects
n: 10
title: Sound effects for every event
date: 2026-10-29
summary: The eleven effects after the cluck, written in Undertone with FM voices and pitch steps, downmixed to the OPL3 pair, OPL2 and the PC speaker, played for every rules event on the Web and DOS, with Fizz 1's overlap rules and an audition list on the Web.
prs: [99, 100, 102, 103, 104, 105, 106]
---
Fizz 1 pairs every floating word with a sound. Chapter 9 ends with the cluck as the only sound in Fizz 2; this chapter covers the other eleven and the table that wires them to the rules' events. The rules did not change: the golden `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}` is the same, and sound is a view, like the captions and the stereo place.

## The effects and where they come from

`assets/sfx/effects.ts` holds nineteen effects by id: the cluck's four variants (0 to 3), then fzzt 4, stun 5, bonk 6, hurt 7, win 8, key 9, clunk 10, plink 11, boing 12, jump 13, crunch 14, three alternates no event plays, boing (FM) 15, jump (FM) 16 and fzzt (FM) 18, and 17, held for the can pickup sound another change adds. After listening the owner chose Fizz 1's own jump, pogo and fizz shot on both platforms. On the Web the jump (13), the boing (12) and the fzzt (4) are Fizz 1's Enhanced voices unchanged, as plain oscillators (chains of short notes, and a noise burst through a high-pass with a sliding sine, for which `SfxVoice` gained Fizz 1's `hpf` and `slide`). On DOS they are Fizz 1 DOS's own records and speaker lists (`assets/sfx/fizz1-dos.ts`, extracted from the port's sound blob), because the converter lowered the Enhanced boing's sixteen notes to one gliding note, which is what made the DOS pogo sound like the FM one, whereas the port converted Fizz 1's Classic notes. The set-aside voices are the alternates (the FM boing, the FM jump and the FM fzzt), whose DOS data is still the downmix of their Web voices, and switching an event to one is a one-line change in `captions.rs` (`docs/SOUND.md` says how). Effects 4 to 11 and 14 are Fizz 1's `SFX` entries (`patterns.ts`) with its Enhanced pitch curves and bell partials (`enhanced-sfx.ts`), rewritten as Undertone data: the notes, lengths, gains and filters are Fizz 1's. Two fields are new in `SfxVoice`. `fm` is Undertone's `fmVoice` two-operator FM (a modulator ratio, an index and a decay), which stands in for Fizz 1's inharmonic bell partials and gives the boing its spring. `steps` is Undertone's `pitchsteps`, a pitch curve in semitones with one value for each 1/140 s, which replaces Fizz 1's chains of short notes. Undertone 0.3.0 plays both, and the Web builds the voices with `fmVoice()` and `.pitchsteps()`.

`fizz_slice_view::captions` is the one table that names the effect and the caption of each event, and `event_sound` turns an event into its effect id and a place in the stereo field: the point where the event happened, against the camera the picture is drawn with. DOS's `play_events` and the Web's `App` both call it, so the two platforms play the same effect from the same place. A shot ending has a puff and no sound.

## The downmix

`tools/sfx/downmix.ts` lowers each effect to an OPL2 register chunk and a PC speaker divisor list, and `fizz-sfx` stores them: 9,508 bytes of records and 1,116 bytes of divisors for all nineteen, 8,124 bytes more than the cluck alone. The OPL3 pair needs no data of its own; it plays the same records on channels 5 and 14. A pitch step already is a chip frame, so it carries over exactly. An FM voice becomes a patch whose modulator takes the nearest multiple the chip has, a level from the index and a decay rate from a rough table; this is a best-effort stopgap until the companion OPL package in Undertone issue 25 is reviewed, and it loses the extra layers because the chip sounds one voice at a time. `docs/SOUND.md` has a table of what each effect loses. The busiest tick of every effect is its patch, 11 register writes (22 on the pair), inside the 16 writes a tick the music is held to; a `fizz-sfx` test fails if any effect writes more.

## What Fizz 1 does when sounds meet

Fizz 1 at `4bce036` answers four questions, and Fizz 2 copies each answer. On the Web it has no limit or priority: every `play` builds a stack and plays it, so effects overlap. In its DOS port one volatile word holds the next effect and the 560 Hz handler restarts channel 5 for it, so the last request wins and cuts what was playing, for every effect alike. Its 180 ms dedupe drops caption text and never the sound, which plays first. Its click comes from the press edge of Fire, so a held button clicks once. At the exit the level's song keeps playing under ta-da!; the mix only changes when the cleared card shows 700 ms later, and Fizz 2 has no card.

In Fizz 2 the DOS service has one effect slot and takes the last request of a tick, so a snack taken a tick after a jump plays the snack. The replay's 62 sounding events fall on 61 distinct ticks, so the service starts 61 effects. The Web plays each as its own sound and keeps a cap of six at once so a pile-up cannot clip; whoa! and ta-da! bypass it.

## Checking it, and the audition list

`tools/check-dos.sh` runs the replay under every sound configuration and requires the golden transcript each time. It requires `AUDIO WRITES` of 5,936 on OPL2 channel 5, 5,957 on an OPL3 on the AdLib role and 11,812 on the pair (the 41 start and stop writes, twice the 5,916 records, less 61 enables), and 1,207 speaker steps (5,178, 5,199, 10,296 and 1,264 with the FM fzzt, boing and jump of the first listen); the numbers were 569, 590 and 1,135 when only the clucks played. They are exact because the replay is deterministic and DOSBox runs at fixed cycles. `FIZZ2.EXE` grew by 10,240 bytes, from 190,976 to 201,216 (the sound data is 6,360 of that and the bench lines about 4 KB).

On the Web, `web/sfx.test.ts` snapshots what each new effect schedules on a stand-in `AudioContext`, and checks that FM voices schedule a modulator at the ratio times the pitch and that pitch steps arrive a frame at a time. The H panel has a list, "Hear an effect", and a Play button that play any of the nineteen by id, centred and on their own; `tools/check-web-sfx.py` drives it in headless Chromium.

<!-- figure shots web-effects.png -->

`/BENCH` also times one busy picture with no captions and with all six caption slots full, and with `/AUDIO` while effects play over the music. The figures are in `docs/DOS-CHECK.md`.

## The shelves

### Catalogued
- The owner's playtest of 29 October 2026: they listened to the new effects on the Web and found them good ("they sound decent", and they may revisit them), and said Milestone 4 "looks good in gameplay" on the Web and in the DOS player. One person's statement, not a measurement.
- The fifteen effects exist in `assets/sfx/effects.ts`, and `bun tools/sfx/downmix.ts --check` keeps `crates/fizz-sfx/src/data.rs` equal to what the converter makes of them. `cargo test -p fizz-sfx` pins each effect's records, ticks and speaker frames and the 11-write ceiling.
- Every sounding event of the replay maps to one effect: a `fizz-slice-view` test walks the golden's `EVENT` lines against the sounds the table gives, in order, with the same sounds whether captions are on or off.
- The Web schedules each effect's oscillators, modulators and noise bursts on a stand-in `AudioContext` (`bun test web/sfx.test.ts`) and in headless Chromium through the audition list (`tools/check-web-sfx.py`).
- The DOS paths write what they should: `tools/check-dos.sh` requires 5,936, 5,957 and 11,812 writes and 1,207 speaker steps for the replay, with `FINAL TICK {{golden.finalTick}} HASH {{golden.final}}` unchanged on native, Web and DOS (`tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}`).
- The six-caption bench runs under DOSBox and prints the same picture's cost with and without the captions.

### Uncatalogued
- Hearing the eleven new effects on DOS or on real hardware. The owner's remark that Milestone 4 looks good in gameplay on the DOS player does not name a DOS effect or the sound. The stand-in cannot hear, DOSBox's register writes are counted and not played to a person, and the FM voices are authored by their numbers.
- The best-effort FM downmix compared by ear against the Web source, and the effects on a real OPL2, OPL3, Sound Blaster or PC speaker.
- The loudness of each effect beside the others and beside the music, and the cap of six on a real browser.
- The cost of the 560 Hz interrupt with effects and music together on a 486: DOSBox charges no time for port I/O, so the `/BENCH /AUDIO` counts are exact and the microseconds are the emulator's.

### Wish list
- Effects designed by ear in place of these first drafts, and stereo-authored effects, which the pair already allows with one record stream for each side.
- The exact FM-to-OPL mapping from the companion package in Undertone issue 25, and separately authored songs; both are Milestone 4.5 on the roadmap, as are the stereo-authored effects, the OPL3 core-and-extras idea, music compression and the audit of redundant register writes.

> **From the workbench — FM synthesis.** In frequency-modulation synthesis one oscillator, the carrier, has its pitch swung by another, the modulator. A modulator at a whole-number multiple of the carrier gives a harmonic tone, and a fraction such as 2.76 gives the inharmonic partials of a bell. John Chowning developed the method at Stanford in the late 1960s, and the Yamaha OPL2 and OPL3 chips build their tones from pairs of such operators.
> Source: [Frequency modulation synthesis, Wikipedia](https://en.wikipedia.org/wiki/Frequency_modulation_synthesis)

```figures
id: web-effects-shot
kind: screenshot
track: web
title: The Hear an effect list
alt: The Web slice at 1280 by 720 with the H panel open on its right: the Options button, the list Hear an effect showing all fifteen entries from 0 cluck 0 (buh-GAWK) to 14 crunch with crunch chosen, a Play button, and the debug lines TICK 40 and HASH.
caption: The H panel's picker lists the fifteen effects by id. In the page it is a drop-down; the capture draws it open.
at: figure shots web-effects.png
thumb: img:media/web-effects.png
```
