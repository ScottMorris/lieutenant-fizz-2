// The sound effects as Undertone voices, one per effect id: the rich source of truth the Web plays and the DOS downmixes derive from.
//
// An effect id is a small number: `EFFECTS[id]` is the effect's voices, the DOS data is generated in the same order (entry `id` of every
// table in crates/fizz-sfx/src/data.rs), and `audio::effect(id, ..)` on DOS and `Sfx.play(id, ..)` on the Web both play by it. Ids:
//
//   0-3  the chicken's cluck, variants 0..=3 (`Event::Cluck { variant }`; `CLUCK_VARIANTS = 4`).
//   4 fzzt (a shot), 5 stun ("fizzled"), 6 bonk (a stomp), 7 hurt ("whoa!"), 8 win ("ta-da!"), 9 key (a gumdrop), 10 clunk (a
//   door or a head-bump), 11 plink ("click - no fizz"), 12 boing, 13 jump (no caption), 14 crunch (a snack).
//   `crates/fizz-slice-view/src/captions.rs` says which event plays which id; a new effect is appended at its index, with no gaps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: effects 4 to 14 are written from Fizz 1's `SFX` table (`episodes/episode-1/src/audio/patterns.ts`) and its Enhanced pitch
// curves and bell partials (`enhanced-sfx.ts`) in lieutenant-fizz at 4bce036, the same author's code, dual-licensed there as here: the
// notes, lengths, gains and filters are Fizz 1's, and its curve shapes are written as Undertone `pitchsteps`. Where a voice here has
// an `fm`, that is Undertone's two-operator FM (`fmVoice`) standing in for what Fizz 1 did with a second, inharmonic partial voice (the
// bells) or could not do at all (a springy boing, a buzzing fizzle). Fizz 1's `hpf` has no field here, so its high-passed noise is a
// short burst of white noise with a high `lpf`, the nearest the vocabulary has.
//
// Rule for the downmix: voices that start together put the main layer first at the highest gain, and a noise breath starts a little
// before the pitched note it belongs to. DOS plays one voice at a time, so the layers beyond the lead are Web-only.
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

import { breath, curve, glide, repeat, wobble, type SfxVoice } from './voice.ts';

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

/** A Fizz 1 voice: its defaults (a triangle, 1 ms attack, 100 ms decay, 50 ms release), with the overrides given. */
const V = (n: string, o: Partial<SfxVoice> = {}): SfxVoice => ({ n, w: 'triangle', a: 0.001, d: 0.1, r: 0.05, g: 0.4, ...o });

/**
 * Effect 4, fzzt (a shot). Fizz 1: a high-passed noise burst and a sine falling an octave (its `slide`) in 80 ms. Here the sine
 * starts at c7 and falls to c6 as pitch steps, and it is FM at a metallic ratio that clears as it falls, so it fizzes at the attack.
 * About 0.13 s.
 */
const fzzt: SfxVoice[] = [
  breath('white', 0, 0.08, 0.2, 8000),
  V('c6', { w: 'sine', d: 0.12, g: 0.25, nudge: 0.006, steps: glide(-12, 0.08, 12), fm: { ratio: 3.5, index: 3, decay: 0.09 } }),
];

/**
 * Effect 5, stun ("fizzled"). Fizz 1: a falling square and a puff of noise; its Enhanced version wobbles the pitch at 18 swings a
 * second. Here the voice is FM, so the wobble sputters. About 0.17 s.
 */
const stun: SfxVoice[] = [
  breath('white', 0, 0.05, 0.15, 8000),
  V('a4', { w: 'sine', d: 0.14, g: 0.2, lpf: 1800, nudge: 0.006, steps: wobble(3, 18, 0.16, -12, 12), fm: { ratio: 1.5, index: 4, decay: 0.12 } }),
];

/** Effect 6, bonk (a stomp): a deep knock that falls an octave from c3 to c2 in 70 ms, with a click of noise. About 0.11 s. */
const bonk: SfxVoice[] = [
  breath('white', 0, 0.02, 0.3, 3000),
  V('c2', { d: 0.1, g: 0.6, lpf: 400, nudge: 0.008, steps: glide(-12, 0.07, 12), fm: { ratio: 1, index: 2, decay: 0.05 } }),
];

/**
 * Effect 7, hurt ("whoa!"): two square voices falling an octave together, the second a fifth under and 150 ms late, the first with a
 * slight wobble (Fizz 1's Enhanced `fallWobble`). About 0.6 s.
 */
const hurt: SfxVoice[] = [
  V('a3', { w: 'square', d: 0.45, g: 0.3, lpf: 900, steps: wobble(0.6, 14, 0.46, -12, 12), fm: { ratio: 1, index: 1.5, decay: 0.3 } }),
  V('e3', { w: 'square', d: 0.4, g: 0.25, lpf: 700, nudge: 0.15, steps: glide(-12, 0.35, 12) }),
];

/**
 * Effect 8, win ("ta-da!"): a rising arpeggio c5 e5 g5 c6 and a long e6. Fizz 1's Enhanced version adds a quiet sine at 2.76 times
 * the top notes for a bell glint; here that is FM at 2.76 on the same voice. About 0.75 s.
 */
const win: SfxVoice[] = [
  V('c5', { d: 0.1, g: 0.35 }),
  V('e5', { d: 0.1, g: 0.35, nudge: 0.08 }),
  V('g5', { d: 0.1, g: 0.35, nudge: 0.16 }),
  V('c6', { d: 0.1, g: 0.35, nudge: 0.24, fm: { ratio: 2.76, index: 0.8, decay: 0.05 } }),
  V('e6', { w: 'sine', d: 0.4, g: 0.35, nudge: 0.32, fm: { ratio: 2.76, index: 1.2, decay: 0.12 } }),
];

/** Effect 9, key (a gumdrop): three rising notes c5 e5 g5, the last a bell (Fizz 1's Enhanced `key` partial as FM). About 0.35 s. */
const key: SfxVoice[] = [
  V('c5', { d: 0.1, g: 0.35 }),
  V('e5', { d: 0.1, g: 0.35, nudge: 0.07 }),
  V('g5', { d: 0.2, g: 0.35, nudge: 0.14, fm: { ratio: 2.76, index: 1, decay: 0.08 } }),
];

/** Effect 10, clunk (a door or a head-bump): a low thud at c2 sagging three semitones, with a breath of brown noise first. About 0.16 s. */
const clunk: SfxVoice[] = [
  breath('brown', 0, 0.08, 0.4, 800),
  V('c2', { d: 0.15, g: 0.6, lpf: 300, nudge: 0.008, steps: [3, 2, 1, 0], fm: { ratio: 1, index: 1.2, decay: 0.06 } }),
];

/** Effect 11, plink ("click - no fizz"): Fizz 1's plink, a very short high sine at c7, with a glint of FM. About 0.05 s. */
const plink: SfxVoice[] = [V('c7', { w: 'sine', d: 0.05, g: 0.25, fm: { ratio: 2, index: 1.5, decay: 0.03 } })];

/**
 * Effect 12, boing. Fizz 1's Enhanced version: g3 wobbling five semitones at nine swings a second, fading over 0.4 s, with a bright
 * sine on top. FM makes the body springy. About 0.4 s.
 */
const boing: SfxVoice[] = [
  V('g3', { d: 0.4, g: 0.42, steps: wobble(5, 9, 0.4), fm: { ratio: 1, index: 2, decay: 0.3 } }),
  V('d5', { w: 'sine', d: 0.08, g: 0.2, nudge: 0.07 }),
];

/** Effect 13, jump: a square chirp rising nine semitones from e4 in 80 ms (Fizz 1's Enhanced curve). No caption. About 0.1 s. */
const jump: SfxVoice[] = [V('e4', { w: 'square', d: 0.1, g: 0.2, lpf: 2600, steps: glide(9, 0.08) })];

/** Effect 14, crunch (a snack): a noise burst, then c5 and e5 with an inharmonic crackle (FM). About 0.19 s. */
const crunch: SfxVoice[] = [
  breath('white', 0, 0.03, 0.3, 2500),
  V('c5', { d: 0.1, g: 0.35, lpf: 4000, nudge: 0.008, fm: { ratio: 1.41, index: 3, decay: 0.06 } }),
  V('e5', { d: 0.14, g: 0.35, lpf: 4500, nudge: 0.05, fm: { ratio: 1.41, index: 3, decay: 0.08 } }),
];

/** The effects after the cluck, in id order from 4. */
export const OTHER_EFFECTS: readonly (readonly SfxVoice[])[] = [fzzt, stun, bonk, hurt, win, key, clunk, plink, boing, jump, crunch];

/** The name of each effect by id, for tools and the audition picker. */
export const EFFECT_NAMES: readonly string[] = [
  'cluck 0 (buh-GAWK)',
  'cluck 1 (single)',
  'cluck 2 (double)',
  'cluck 3 (repeated)',
  'fzzt',
  'stun (fizzled)',
  'bonk',
  'hurt (whoa!)',
  'win (ta-da!)',
  'key (gumdrop)',
  'clunk',
  'plink (click)',
  'boing',
  'jump',
  'crunch',
];

/**
 * Every effect in the source, indexed by effect id; the downmix writes one chunk per entry, in this order. A new effect is appended
 * at the next index: a gap is not allowed, because the index is the id.
 */
export const EFFECTS: readonly (readonly SfxVoice[])[] = [...CLUCK, ...OTHER_EFFECTS];
