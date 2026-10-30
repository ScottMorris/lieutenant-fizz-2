// The Undertone effect vocabulary the Fizz 2 sound source is written in: voices, pitch curves and a note-name table.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `SfxVoice` has the fields of Fizz 1's `SfxVoice` (packages/engine/src/audio.ts) so a Web presenter can hand the voices to
// Undertone unchanged: `n` the note or noise type, `w` the oscillator, `a`/`d`/`r` the envelope in seconds (Undertone's default
// sustain is 0, so a voice is gone after `a + d`), `g` the gain, `lpf` and the `lp*` filter envelope, `nudge` the start offset in
// seconds and `hz` an exact pitch used in place of `n`. Nothing here touches a browser or imports Undertone, so Bun can run it.
// `curve` follows the idea of Fizz 1's `curveVoices` (episodes/episode-1/src/audio/enhanced-sfx.ts): Undertone's `slide` only
// falls, so a pitch curve is a chain of very short notes, one every 25 ms, each at the next pitch.

export type Wave = 'sine' | 'triangle' | 'square' | 'sawtooth';
export type NoiseKind = 'white' | 'pink' | 'brown';

export interface SfxVoice {
  /** A note name (`e4`) or a noise type (`white`, `pink`, `brown`). */
  n: string;
  w?: Wave;
  a?: number;
  d?: number;
  r?: number;
  g: number;
  lpf?: number;
  /** The filter opens to `lpenv` Hz at the attack (`lpa` s), falls to `lps` of the way back over `lpd`, closes over `lpr`. */
  lpenv?: number;
  lpa?: number;
  lpd?: number;
  lps?: number;
  lpr?: number;
  nudge?: number;
  /** An exact pitch in Hz, used in place of `n`. */
  hz?: number;
  /**
   * Two-operator FM, the shape of Undertone's `fmVoice` definition (Tone.js `FMSynth` terms): a modulator at `ratio` times the
   * voice's pitch swings it by `index` times the voice's pitch (Tone's `modulationIndex`), and the swing falls to nothing over `decay`
   * seconds when `decay` is given. The web plays it as Undertone does; the DOS downmix lowers it to the nearest OPL modulator setting.
   */
  fm?: Fm;
  /**
   * A frame-stepped pitch curve (Undertone's `pitchsteps`): semitone offsets from the voice's pitch, one per 1/140 s (the classic
   * AdLib effect frame, which DOS steps at as well), holding the last. A voice with `steps` is one note that moves, not a chain of notes.
   */
  steps?: readonly number[];
}

export interface Fm {
  /** The modulator's frequency as a multiple of the voice's (Tone's `harmonicity`). Whole numbers are harmonic, fractions metallic. */
  ratio: number;
  /** Peak deviation as a multiple of the voice's frequency (Tone's `modulationIndex`). */
  index: number;
  /** Seconds for the modulation to fall to nothing; without it the depth is constant. */
  decay?: number;
  /** The modulator's waveform; sine when absent. */
  wave?: Wave;
}

/** The frame rate of `steps`, in hertz: the classic AdLib effect clock, which Undertone's `pitchsteps` defaults to as well. */
export const STEP_RATE = 140;

/** The time between the short notes of a pitch curve, in seconds. */
export const STEP = 0.025;

const SEMITONES: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };

/** The frequency of a note name such as `a4` or `f#3` (A4 is 440 Hz), as Undertone's `noteToFrequency`. */
export function noteHz(name: string): number {
  const m = /^([a-g])([#b]?)(-?\d+)$/.exec(name);
  if (!m) throw new Error(`not a note name: ${name}`);
  const midi = 12 * (Number(m[3]) + 1) + SEMITONES[m[1]] + (m[2] === '#' ? 1 : m[2] === 'b' ? -1 : 0);
  return 440 * 2 ** ((midi - 69) / 12);
}

export const isNoise = (name: string): boolean => name === 'white' || name === 'pink' || name === 'brown';

/** The pitch of a voice in Hz; a noise voice has none. */
export const voiceHz = (v: SfxVoice): number => v.hz ?? noteHz(v.n);

/** A falling or rising pitch: the starting pitch, the travel in semitones (negative falls), and how long it takes. */
export interface Curve {
  fromHz: number;
  semitones: number;
  seconds: number;
  wave: Wave;
  gain: number;
  lpf?: number;
  /** Seconds before the curve starts. */
  start?: number;
}

/**
 * A curve as a run of short notes. Each note lasts about 45 ms, so neighbours overlap; the gain eases down along the curve,
 * as the chip's carriers do. The note name is the nearest to the starting pitch and `hz` carries the exact value.
 */
export function curve(c: Curve): SfxVoice[] {
  const count = Math.max(2, Math.ceil(c.seconds / STEP));
  return Array.from({ length: count }, (_, step): SfxVoice => {
    const u = step / (count - 1);
    const hz = c.fromHz * 2 ** ((c.semitones * u) / 12);
    const v: SfxVoice = {
      n: nearestNote(c.fromHz),
      hz: round(hz, 2),
      w: c.wave,
      a: 0.002,
      d: STEP * 1.5,
      r: 0.012,
      g: round(c.gain * (1 - 0.45 * u), 4),
      nudge: round((c.start ?? 0) + step * STEP, 4),
    };
    if (c.lpf) {
      v.lpf = c.lpf;
      v.lpenv = Math.round(c.lpf * 1.3);
      v.lpa = 0.001;
      v.lpd = STEP;
      v.lps = 0.3;
      v.lpr = 0.012;
    }
    return v;
  });
}

/** The shapes of Fizz 1's Enhanced pitch curves that `chain` reproduces: a straight rise, and a swing that fades to nothing. */
export type ChainShape = 'rise' | 'wobble';

export interface Chain {
  /** The starting note name; its frequency is the curve's base pitch. */
  from: string;
  shape: ChainShape;
  /** Rise: the travel in semitones. Wobble: the size of the swing in semitones. */
  semitones: number;
  seconds: number;
  wave: Wave;
  gain: number;
  lpf?: number;
  /** Swings a second, for the wobble shape (Fizz 1's default is 8). */
  rate?: number;
}

/**
 * Fizz 1's `curveVoices` (episode-1 `enhanced-sfx.ts`), unchanged: a curve as a run of short notes, one every 25 ms, each at the next
 * pitch on the shape, with a 2 ms attack, 37.5 ms decay, 12 ms release, and the gain easing down by 55 percent along the curve. Not
 * `curve` above, which eases by 45 percent, adds a filter envelope and only rises or falls in a straight line.
 */
export function chain(c: Chain): SfxVoice[] {
  const base = noteHz(c.from);
  const count = Math.max(2, Math.ceil(c.seconds / STEP));
  return Array.from({ length: count }, (_, step): SfxVoice => {
    const u = step / (count - 1);
    const offset =
      c.shape === 'rise' ? c.semitones * u : c.semitones * (1 - u) * Math.sin(2 * Math.PI * (c.rate ?? 8) * c.seconds * u);
    const v: SfxVoice = {
      n: c.from,
      hz: round(base * 2 ** (offset / 12), 3),
      w: c.wave,
      a: 0.002,
      d: STEP * 1.5,
      r: 0.012,
      g: round(c.gain * (1 - 0.55 * u), 4),
      nudge: round(step * STEP, 4),
    };
    if (c.lpf) v.lpf = c.lpf;
    return v;
  });
}

/** Pitch steps for a straight glide of `semitones` over `seconds` (negative falls) from `from` semitones off the voice's pitch, then holding the end. */
export function glide(semitones: number, seconds: number, from = 0): number[] {
  const frames = Math.max(1, Math.round(seconds * STEP_RATE));
  return Array.from({ length: frames + 1 }, (_, i) => round(from + (semitones * i) / frames, 3));
}

/**
 * Pitch steps for a wobble: a swing of `semitones` at `rate` swings a second, fading linearly to nothing over `seconds`
 * (Fizz 1's Enhanced `wobble` shape), on top of a straight glide of `drift` semitones over the same time, from `from` semitones off the voice's pitch.
 */
export function wobble(semitones: number, rate: number, seconds: number, drift = 0, from = 0): number[] {
  const frames = Math.max(1, Math.round(seconds * STEP_RATE));
  return Array.from({ length: frames + 1 }, (_, i) => {
    const u = i / frames;
    return round(from + semitones * (1 - u) * Math.sin(2 * Math.PI * rate * seconds * u) + drift * u, 3);
  });
}

/** The semitone offset a voice's `steps` give `t` seconds into its own life (0 with none; the last step holds). */
export function stepOffset(v: SfxVoice, t: number): number {
  if (!v.steps || v.steps.length === 0) return 0;
  const frame = Math.min(v.steps.length - 1, Math.max(0, Math.floor(t * STEP_RATE + 1e-9)));
  return v.steps[frame];
}

/** A short burst of noise: a breath or the rasp of a clack. `start` is when it begins and `seconds` how long it lasts. */
export function breath(kind: NoiseKind, start: number, seconds: number, gain: number, lpf: number): SfxVoice {
  return { n: kind, g: gain, a: 0.003, d: seconds, r: 0.01, lpf, nudge: round(start, 4) };
}

/**
 * A base effect played twice: the voices unchanged, then again `gap` seconds after the base ends, with its pitched voices scaled by
 * `pitch` (a frequency ratio) and every gain by `gain`. Noise has no pitch and only takes the gain.
 */
export function repeat(base: readonly SfxVoice[], gap: number, second: { pitch?: number; gain?: number } = {}): SfxVoice[] {
  const offset = effectSeconds(base) + gap;
  const again = base.map((v): SfxVoice => {
    const w: SfxVoice = { ...v, g: round(v.g * (second.gain ?? 1), 4), nudge: round((v.nudge ?? 0) + offset, 4) };
    if (!isNoise(v.n) && second.pitch !== undefined) w.hz = round(voiceHz(v) * second.pitch, 2);
    return w;
  });
  return [...base, ...again];
}

/** How long a voice is heard, in seconds from its own start: attack and decay (Undertone's sustain is 0). */
export const voiceSeconds = (v: SfxVoice): number => (v.a ?? 0.01) + (v.d ?? 0.1);

/** The length of an effect: when its last voice ends. */
export const effectSeconds = (voices: readonly SfxVoice[]): number =>
  Math.max(...voices.map((v) => (v.nudge ?? 0) + voiceSeconds(v)));

function round(x: number, places: number): number {
  const k = 10 ** places;
  return Math.round(x * k) / k;
}

function nearestNote(hz: number): string {
  const names = ['c', 'c#', 'd', 'd#', 'e', 'f', 'f#', 'g', 'g#', 'a', 'a#', 'b'];
  const midi = Math.round(69 + 12 * Math.log2(hz / 440));
  return `${names[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
}
