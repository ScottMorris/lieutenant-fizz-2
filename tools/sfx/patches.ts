// The OPL2 instruments the sound-effect downmix uses, chosen by the oscillator an Undertone voice gives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from `m4/sound/fizz-dos/patches.ts` of ScottMorris/lieutenant-fizz-dos at commit 221dd6c, the same
// author's code, MIT there and dual-licensed here. What changed: only the four effect patches are kept (`fxSquare`, `fxSaw`,
// `fxSoft` and `noise`); the music patches, drums and part-role logic are not used by effects and are left behind. The
// operator settings are the originals, which that repository's own notes call first guesses to be tuned by ear.

import type { SfxVoice, Wave } from '../../assets/sfx/voice.ts';
import type { Operator, Patch } from './opl.ts';

const op = (o: Partial<Operator> & Pick<Operator, 'mult' | 'tl'>): Operator => ({
  ar: 15,
  dr: 4,
  sl: 3,
  rr: 7,
  sustain: true,
  ...o,
});

export const PATCHES = {
  /** A held, clean tone: the carrier does the work and the level is moved a frame at a time. */
  fxSquare: {
    name: 'effect (square)',
    mod: op({ mult: 1, tl: 26, ar: 15, dr: 0, sl: 0, rr: 12 }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 0, sl: 0, rr: 12 }),
    feedback: 4,
  },
  /** High feedback is the buzzy, saw-like edge. */
  fxSaw: {
    name: 'effect (saw)',
    mod: op({ mult: 1, tl: 16, ar: 15, dr: 0, sl: 0, rr: 12 }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 0, sl: 0, rr: 12 }),
    feedback: 7,
  },
  fxSoft: {
    name: 'effect (triangle/sine)',
    mod: op({ mult: 1, tl: 44, ar: 15, dr: 0, sl: 0, rr: 12 }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 0, sl: 0, rr: 12 }),
    feedback: 1,
  },
  /** A noisy buzz for noise bursts: high feedback at a jittering pitch. */
  noise: {
    name: 'noise',
    mod: op({ mult: 1, tl: 8, ar: 15, dr: 6, sl: 8 }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 6, sl: 8, rr: 8 }),
    feedback: 7,
  },
} satisfies Record<string, Patch>;

/** The effect patch for a voice's oscillator. */
export function patchForEffect(wave: Wave | undefined): Patch {
  const w = wave ?? 'triangle';
  return w === 'square' ? PATCHES.fxSquare : w === 'sawtooth' ? PATCHES.fxSaw : PATCHES.fxSoft;
}

/** What the OPL's 4-bit multiple register means: register values 0 to 15 give these multiples of the note's frequency. */
const OPL_MULTIPLES = [0.5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 12, 12, 15, 15];

/** The register value whose multiple is nearest `ratio` on a log scale (so 2.76 becomes 3, not 2). */
export function nearestMultiple(ratio: number): number {
  let best = 1;
  OPL_MULTIPLES.forEach((m, reg) => {
    if (Math.abs(Math.log2(m / ratio)) < Math.abs(Math.log2(OPL_MULTIPLES[best] / ratio))) best = reg;
  });
  return best;
}

const fmPatches = new Map<string, Patch>();

/**
 * A best-effort OPL2 patch for an FM voice. The carrier is the effect patch for the voice's oscillator; the modulator takes the nearest
 * OPL multiple of the voice's `ratio`, a total level from its `index` (a louder modulator is a brighter sound: 6 dB, eight steps of
 * 0.75 dB, to a doubling of the swing), and, when the voice's FM decays, a percussive envelope whose decay rate is picked so the
 * modulator is gone in about that time. It is not an exact mapping: Undertone's FM modulates frequency by an index times the
 * modulator's frequency, the chip modulates phase by an amount set in level steps, and the decay times are read off a rough rate
 * table. The same voice gives the same patch object, so the converter does not reload it between frames. The proper mapping is
 * upstream issue 25; this is the stopgap.
 */
export function fmPatch(voice: SfxVoice): Patch {
  const fm = voice.fm;
  if (!fm) return patchForEffect(voice.w);
  const key = JSON.stringify([voice.w ?? 'triangle', fm]);
  const known = fmPatches.get(key);
  if (known) return known;
  const base = patchForEffect(voice.w);
  const tl = Math.max(10, Math.min(48, Math.round(52 - 6 * Math.log2(1 + fm.index * 3))));
  const decays = fm.decay !== undefined;
  const dr = decays ? Math.max(2, Math.min(15, Math.round(15 - Math.log2(Math.max(fm.decay!, 0.005) / 0.005)))) : 0;
  const patch: Patch = {
    name: `effect (fm ${fm.ratio}:${fm.index}${decays ? `, ${fm.decay}s` : ''}, ${voice.w ?? 'triangle'})`,
    mod: op({ mult: nearestMultiple(fm.ratio), tl, ar: 15, dr, sl: decays ? 15 : 0, rr: 12, sustain: !decays }),
    car: base.car,
    feedback: Math.min(base.feedback, 3),
  };
  fmPatches.set(key, patch);
  return patch;
}
