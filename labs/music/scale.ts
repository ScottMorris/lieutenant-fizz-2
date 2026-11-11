// Scales, and degree strings: the lab's tunes are written in scale degrees and turned into note names for Undertone.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Writing in degrees keeps a riff the same shape in any key, which is what lets two tunes share one. The output is ordinary
// mini-notation with note names, so the song builder in assets/music (which moves and doubles note names) works on it unchanged.

export type Mode = 'major' | 'minor' | 'dorian' | 'phrygian' | 'lydian' | 'mixolydian';

export const MODES: Record<Mode, readonly number[]> = {
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  phrygian: [0, 1, 3, 5, 7, 8, 10],
  lydian: [0, 2, 4, 6, 7, 9, 11],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
};

const NAMES = ['c', 'c#', 'd', 'd#', 'e', 'f', 'f#', 'g', 'g#', 'a', 'a#', 'b'];
const LETTERS: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

export interface Key {
  /** The tonic's pitch class, 0 (C) to 11 (B). */
  tonic: number;
  mode: Mode;
}

/** A key from a name such as `e dorian` or `f# minor`. */
export function keyOf(name: string): Key {
  const match = /^([a-g])(#|b)?\s+(\w+)$/.exec(name.trim().toLowerCase());
  if (!match || !(match[3]! in MODES)) throw new Error(`"${name}" is not a key the lab knows`);
  const tonic = (LETTERS[match[1]!]! + (match[2] === '#' ? 1 : match[2] === 'b' ? 11 : 0)) % 12;
  return { tonic, mode: match[3] as Mode };
}

export const noteName = (midi: number): string => `${NAMES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;

/** The MIDI note of a scale degree: degree 0 is the tonic in `octave`, 7 the tonic above, -1 the step below. */
export function degreeMidi(key: Key, octave: number, degree: number): number {
  const steps = MODES[key.mode];
  const index = ((degree % 7) + 7) % 7;
  return 12 * (octave + 1) + key.tonic + steps[index]! + 12 * Math.floor(degree / 7);
}

/** The pitch classes of a key moved by `shift` semitones. */
export const scaleClasses = (key: Key, shift = 0): Set<number> =>
  new Set(MODES[key.mode].map((step) => (((key.tonic + step + shift) % 12) + 12) % 12));

// A degree is a whole number that is not the count of an operator: `4@3`, `c*2`, `x!2` and `y/2` keep their counts.
const DEGREE = /(?<![@*!/\w.])-?\d+(?![\w.])/g;

/**
 * Turns a degree string into note names. Everything that is not a degree (rests, brackets, `<>`, commas, `@`, `*`) passes through,
 * so the string keeps its rhythm: `degrees(key, 4, '<[0 ~ 2 4] [0,2,4]>')`.
 */
export function degrees(key: Key, octave: number, pattern: string): string {
  return pattern.replace(DEGREE, (token) => noteName(degreeMidi(key, octave, Number(token))));
}
