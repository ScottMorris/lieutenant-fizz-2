// The sound effects as Undertone voices, one per effect id: the rich source of truth the Web plays and the DOS downmixes derive from.
//
// An effect id is a small number: `EFFECTS[id]` is the effect's voices, the DOS data is generated in the same order (entry `id` of every
// table in crates/fizz-sfx/src/data.rs), and `audio::effect(id, ..)` on DOS and `Sfx.play(id, ..)` on the Web both play by it. Ids:
//
//   0-3  the chicken's cluck, variants 0..=3 (`Event::Cluck { variant }`; `CLUCK_VARIANTS = 4`); defined below.
//   Reserved for later packages (not defined yet, so nothing may play them; add each at exactly this index, in order):
//   4 fzzt, 5 stun, 6 bonk, 7 hurt, 8 win, 9 key, 10 clunk, 11 plink, 12 boing, 13 jump, 14 crunch.
//
// The cluck is described first.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Intent per variant (`fizz_ep2::Event::Cluck { variant }`, `CLUCK_VARIANTS = 4`, variants 0..=3). All are short and none sustains:
//
//   0  "buh-GAWK": a low, quick "buh" that nudges upward, then a loud "GAWK" whose pitch falls hard, with a breath of noise
//      at the attack of each part. About 345 ms.
//   1  The short single cluck: one quick falling note with a small noise rasp, about 150 ms.
//   2  The double cluck: two quick falling clucks a little apart, the second a touch higher and longer, about 320 ms.
//   3  The repeated cluck: variant 1 played twice, "cluck, cluck", with a short gap and the second a little lower and softer, about 330 ms.
//
// These are placeholders built from the project's goals and the shapes Fizz 1 uses (layered voices, a pitch curve as a chain of
// short notes, a filter that opens at the attack). Nobody has listened to them. How effects should really sound is a separate
// discovery track; this module is meant to be replaced by a designed effect without touching the pipeline, which reads only the
// `EFFECTS` table at the bottom. Layering rule the downmix relies on: voices that share a start time put the main layer at the
// highest gain, and a noise breath starts a little before the pitched note it belongs to (see docs/SOUND.md).

import { breath, curve, repeat, type SfxVoice } from './voice.ts';

/** Variant 0, "buh-GAWK". */
const buhGawk: SfxVoice[] = [
  breath('white', 0, 0.015, 0.06, 1800),
  // "buh": a low square nudging up a major third.
  ...curve({ fromHz: 190, semitones: 4, seconds: 0.085, wave: 'square', gain: 0.2, lpf: 900 }),
  breath('white', 0.115, 0.025, 0.1, 3200),
  // "GAWK": a harsh sawtooth falling an octave and a half, with a square one octave under it for body.
  ...curve({ fromHz: 780, semitones: -18, seconds: 0.18, wave: 'sawtooth', gain: 0.3, lpf: 2800, start: 0.13 }),
  ...curve({ fromHz: 390, semitones: -18, seconds: 0.18, wave: 'square', gain: 0.12, lpf: 1400, start: 0.13 }),
];

/** Variant 1, the short single cluck. */
const single: SfxVoice[] = [
  breath('white', 0, 0.02, 0.07, 3000),
  ...curve({ fromHz: 640, semitones: -11, seconds: 0.09, wave: 'square', gain: 0.26, lpf: 2200, start: 0.012 }),
];

/** Variant 2, the double cluck. */
const double: SfxVoice[] = [
  breath('white', 0, 0.015, 0.06, 2600),
  ...curve({ fromHz: 560, semitones: -8, seconds: 0.075, wave: 'square', gain: 0.22, lpf: 2000, start: 0.01 }),
  breath('white', 0.15, 0.02, 0.07, 2800),
  ...curve({ fromHz: 650, semitones: -12, seconds: 0.125, wave: 'square', gain: 0.27, lpf: 2400, start: 0.165 }),
];

/**
 * Variant 3, the repeated cluck: a base cluck played twice, "cluck, cluck". The base is variant 1 (`single`); pointing `REPEAT_BASE`
 * at another variant repeats that one instead. The second cluck starts `REPEAT_GAP` after the first ends, a semitone lower and a
 * tenth quieter, so it is plainly the same cluck again and not a copy.
 */
const REPEAT_BASE: readonly SfxVoice[] = single;
const REPEAT_GAP = 0.075;
const repeated: SfxVoice[] = repeat(REPEAT_BASE, REPEAT_GAP, { pitch: 2 ** (-1 / 12), gain: 0.9 });

/** The cluck's variants, indexed by `Event::Cluck { variant }`: effect ids 0 to 3. */
export const CLUCK: readonly (readonly SfxVoice[])[] = [buhGawk, single, double, repeated];


/**
 * Every effect in the source, indexed by effect id; the downmix writes one chunk per entry, in this order. The reserved ids above
 * are appended here, in order, as their effects are designed: a gap is not allowed, because the index is the id.
 */
export const EFFECTS: readonly (readonly SfxVoice[])[] = [...CLUCK];
