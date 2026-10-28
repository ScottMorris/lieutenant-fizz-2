// Reads a music part's mini-notation into timed events through Undertone's own pattern parser.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `queryPart` is adapted from `m4/sound/fizz-sheet-events.ts` of ScottMorris/lieutenant-fizz-dos (a.k.a. fizz-dos) at
// commit 221dd6c, the same author's code, MIT there and dual-licensed here. What changed: it returns the MIDI note number (via
// Undertone's `noteToMidi`) beside the pitch in Hz, and reads the part type from this repository's `MusicPart`. It runs in Bun and
// in the browser bundle: Undertone is its only import.

import * as U from '@liminal-hq/undertone';
import { Fraction, noteToFrequency, noteToMidi, type Hap, type SoundType } from '@liminal-hq/undertone';
import type { MusicPart } from './types.ts';

export type NoiseKind = 'white' | 'pink' | 'brown';

/** One note or noise burst of a part, as Undertone's pattern reports it. Times are in cycles (bars) from the query's first cycle. */
export interface PatternEvent {
  /** Start and end, in cycles. */
  t0: number;
  t1: number;
  /** The MIDI note number, or undefined for noise. */
  midi?: number;
  /** The pitch in Hz, or undefined for noise. */
  hz?: number;
  noise?: NoiseKind;
}

const cyclesOf = (fraction: Fraction): number => fraction.n / fraction.d;

const patternOf = (part: MusicPart) => (part.noise ? U.sound(part.notes as SoundType) : U.note(part.notes));

/** The onsets of one part for cycles [from, to), in order of cycle and then of the pattern. Times are absolute cycles. */
export function queryPart(part: MusicPart, from: number, to: number): PatternEvent[] {
  const pattern = patternOf(part);
  const events: PatternEvent[] = [];
  for (let cycle = from; cycle < to; cycle++) {
    pattern.query({ begin: new Fraction(cycle), end: new Fraction(cycle + 1) }).forEach((hap: Hap<any>) => {
      if (!hap.whole || cyclesOf(hap.part.begin) !== cyclesOf(hap.whole.begin)) return;
      const value = hap.value;
      const isNoise = value.soundType === 'white' || value.soundType === 'pink' || value.soundType === 'brown';
      const pitched = !isNoise && value.pitch !== undefined;
      events.push({
        t0: cyclesOf(hap.whole.begin),
        t1: cyclesOf(hap.whole.end),
        midi: pitched ? (typeof value.pitch === 'number' ? undefined : noteToMidi(value.pitch)) : undefined,
        hz: pitched ? noteToFrequency(value.pitch) : undefined,
        noise: isNoise ? value.soundType : undefined,
      });
    });
  }
  return events;
}
