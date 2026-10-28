// The OPL instruments and rhythm-mode drums the level music uses, chosen by what a part does and the wave it was given.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from `m4/sound/fizz-dos/patches.ts` (lines 116-186: the music patches, `patchForPart`, the rhythm-mode drum
// tables and `drumForRole`) of ScottMorris/lieutenant-fizz-dos (a.k.a. fizz-dos) at commit 221dd6c, the same author's code, MIT
// there and dual-licensed here. What changed: the effect patches are left to `tools/sfx/patches.ts`; the types come from the Fizz 2
// source (`Wave`, `PartRole`) and `tools/sfx/opl.ts`. The operator settings are the originals, which the DOS port's own notes call
// first guesses to be tuned by ear: nobody has listened to them in Fizz 2.

import type { PartRole, Wave } from '../../assets/music/types.ts';
import type { Operator, Patch } from '../sfx/opl.ts';

const op = (o: Partial<Operator> & Pick<Operator, 'mult' | 'tl'>): Operator => ({
  ar: 15,
  dr: 4,
  sl: 3,
  rr: 7,
  sustain: true,
  ...o,
});

/**
 * The patches. A square, triangle, sawtooth or sine becomes what a two-operator FM voice can say: the modulator's level and feedback
 * shape the brightness (feedback 5 to 7 is the buzzy, saw-like edge), a sine carrier alone is the soft triangle or sine, and a
 * percussive envelope (no sustain) is a pluck or a bell.
 */
export const PATCHES = {
  leadSquare: {
    name: 'lead (square)',
    mod: op({ mult: 1, tl: 24, dr: 4, sl: 4 }),
    car: op({ mult: 1, tl: 0, wave: 0 }),
    feedback: 4,
  },
  leadSaw: {
    name: 'lead (saw)',
    mod: op({ mult: 1, tl: 14 }),
    car: op({ mult: 1, tl: 0 }),
    feedback: 7,
  },
  leadSoft: {
    name: 'lead (triangle/sine)',
    mod: op({ mult: 1, tl: 40 }),
    car: op({ mult: 1, tl: 0 }),
    feedback: 2,
  },
  bass: {
    name: 'bass',
    mod: op({ mult: 1, tl: 34, dr: 6, sl: 6 }),
    car: op({ mult: 1, tl: 0, dr: 5, sl: 3, rr: 8 }),
    feedback: 2,
  },
  bassSaw: {
    name: 'bass (saw/square)',
    mod: op({ mult: 1, tl: 20, dr: 6, sl: 5 }),
    car: op({ mult: 1, tl: 0, dr: 5, sl: 3, rr: 8 }),
    feedback: 6,
  },
  pad: {
    name: 'pad',
    mod: op({ mult: 1, tl: 38, ar: 6, dr: 3 }),
    car: op({ mult: 1, tl: 4, ar: 6, dr: 3, sl: 2, rr: 5 }),
    feedback: 2,
  },
  padBright: {
    name: 'pad (square/saw)',
    mod: op({ mult: 2, tl: 30, ar: 7, dr: 3 }),
    car: op({ mult: 1, tl: 4, ar: 7, dr: 3, sl: 2, rr: 5 }),
    feedback: 4,
  },
  pluck: {
    name: 'pluck',
    mod: op({ mult: 2, tl: 26, ar: 15, dr: 10, sl: 12, sustain: false }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 9, sl: 8, rr: 8, sustain: false }),
    feedback: 3,
  },
  bell: {
    name: 'bell',
    mod: op({ mult: 7, tl: 28, ar: 15, dr: 7, sl: 15, sustain: false }),
    car: op({ mult: 1, tl: 0, ar: 15, dr: 6, sl: 10, rr: 6, sustain: false }),
    feedback: 0,
  },
  drone: {
    name: 'drone',
    mod: op({ mult: 1, tl: 22, ar: 8, dr: 3 }),
    car: op({ mult: 1, tl: 2, ar: 8, dr: 3, sl: 2, rr: 5 }),
    feedback: 7,
  },
} satisfies Record<string, Patch>;

/** The patch for a music part, from its role and the oscillator it was given. */
export function patchForPart(role: PartRole | undefined, wave: Wave | undefined): Patch {
  const w = wave ?? 'triangle';
  const edgy = w === 'square' || w === 'sawtooth';
  switch (role) {
    case 'lead':
    case 'counter':
      return w === 'sawtooth' ? PATCHES.leadSaw : edgy ? PATCHES.leadSquare : PATCHES.leadSoft;
    case 'bass':
      return edgy ? PATCHES.bassSaw : PATCHES.bass;
    case 'pad':
      return edgy ? PATCHES.padBright : PATCHES.pad;
    case 'arp':
      return PATCHES.pluck;
    case 'bell':
      return PATCHES.bell;
    case 'drone':
      return PATCHES.drone;
    default:
      return edgy ? PATCHES.leadSquare : PATCHES.leadSoft;
  }
}

/**
 * The chip's rhythm mode: five drums on channels 6 to 8 (bass drum on channel 6, hi-hat and snare on 7, tom and cymbal on 8). Each
 * drum has its own operator and the noise comes from the chip. `DRUM_CHANNEL_HZ` tunes channels 6, 7 and 8.
 */
export const DRUM_CHANNEL_HZ = [150, 800, 300] as const;
export const DRUMS = {
  bass: { bit: 0x10, holdTicks: 40, ops: [0x10, 0x13] as const },
  snare: { bit: 0x08, holdTicks: 30, ops: [0x14] as const },
  tom: { bit: 0x04, holdTicks: 40, ops: [0x12] as const },
  cymbal: { bit: 0x02, holdTicks: 60, ops: [0x15] as const },
  hat: { bit: 0x01, holdTicks: 14, ops: [0x11] as const },
} as const;

export type DrumName = keyof typeof DRUMS;

/** How each drum's operators are set (the bass drum has two: a modulator and a carrier). */
export const DRUM_PATCHES: Record<DrumName, Operator[]> = {
  bass: [
    op({ mult: 0, tl: 26, ar: 15, dr: 9, sl: 15, rr: 7, sustain: false }),
    op({ mult: 0, tl: 6, ar: 15, dr: 8, sl: 15, rr: 7, sustain: false }),
  ],
  snare: [op({ mult: 1, tl: 12, ar: 15, dr: 10, sl: 15, rr: 8, sustain: false })],
  tom: [op({ mult: 1, tl: 12, ar: 15, dr: 9, sl: 15, rr: 8, sustain: false })],
  cymbal: [op({ mult: 1, tl: 20, ar: 15, dr: 9, sl: 15, rr: 6, sustain: false })],
  hat: [op({ mult: 1, tl: 22, ar: 15, dr: 12, sl: 15, rr: 9, sustain: false })],
};

/** Which drum a part plays. */
export function drumForRole(role: PartRole | undefined): DrumName | undefined {
  switch (role) {
    case 'kick':
      return 'bass';
    case 'snare':
      return 'snare';
    case 'hats':
      return 'hat';
    case 'perc':
      return 'cymbal';
    default:
      return undefined;
  }
}
