// The screen mix: how the music is heard while a screen holds the game (Paused, Options), as a small table and the ramps between rows.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the shape, the ramp times, the choice of an exponential cutoff and a linear gain, `valueAt` and the numbers of the
// `pause` and `card` rows are `packages/engine/src/mix.ts` and `episodes/episode-1/src/audio/mix.ts` of ~/source/liminal-hq/lieutenant-fizz
// at commit 4bce036, the same author's code, Apache-2.0 OR MIT there and here.
//
// THE SHARED TABLE. `MIX` below is the one place the names and numbers live; `docs/SOUND.md` (The screen mix) repeats it for the DOS
// side, which should take the same names (`play`, `pause`, `options`, `card`) and the same numbers: a cutoff in Hz, a gain
// (linear), and the ramp seconds `MIX_RAMP`. The mix applies to the MUSIC only; effects never pass through it.
// A new screen is one more line in `MIX`, and one place that names it (the loop in `web/main.js` for a screen that holds the game).

/** How the music is heard: a low-pass cutoff in Hz (`MIX_OPEN_LPF` is open) and a gain (linear). */
export interface MixShape {
  lpf: number;
  gain: number;
}

/** The names of the screens that have a mix. */
export type MixName = 'play' | 'pause' | 'options' | 'card';

/** A low-pass at or above this cutoff is out of the way: the mix is open. The graph sets it to the context's Nyquist frequency. */
export const MIX_OPEN_LPF = 20000;

/**
 * The mix of each screen.
 *
 * - `play`: the music as it is (open)
 * - `pause`: Paused, muffled to a dull room-through-a-wall sound, and lower (Fizz 1's pause)
 * - `options`: Options opened from Paused, the same as `pause` (in Fizz 1 everything opened from Pause is muffled)
 * - `card`: RESERVED. Fizz 1's lightly muffled mix behind the level-cleared and game-over cards. Fizz 2 has no cards yet, so nothing
 *   asks for it; it is here so the numbers are in one place when they arrive. Dialogue (open, 0.7) and cinematics (open, 0.85) are
 *   not listed: there are none either.
 */
export const MIX: Record<MixName, MixShape> = {
  play: { lpf: MIX_OPEN_LPF, gain: 1 },
  pause: { lpf: 900, gain: 0.7 },
  options: { lpf: 900, gain: 0.7 },
  card: { lpf: 2200, gain: 0.8 },
};

/** The filter's Q, in dB: 0 is flat at the cutoff, with none of the default 1 dB ring (Fizz 1 sets it the same). */
export const MIX_Q = 0;

/** Seconds a change takes: it closes quickly, so the muffle is there as the screen appears, and opens slowly, so the music comes back. */
export const MIX_RAMP = { close: 0.18, open: 0.35 };

/** A ramp of one parameter from `from` at `t0` to `to` at `t1`, exponential or linear. */
export interface Ramp {
  kind: 'exp' | 'lin';
  from: number;
  to: number;
  t0: number;
  t1: number;
}

/** The ramps from one mix to another: the cutoff exponentially (pitch-like), the gain linearly. */
export interface MixRamps {
  lpf: Ramp;
  gain: Ramp;
}

/**
 * The ramps from `from` to `to` starting at `now`. A change that lowers the cutoff, or lowers the gain at the same cutoff, closes
 * over `MIX_RAMP.close`; any other change opens over `MIX_RAMP.open`. Both parameters take the same time so they arrive together.
 */
export function mixRamps(from: MixShape, to: MixShape, now: number): MixRamps {
  const closing = to.lpf < from.lpf || (to.lpf === from.lpf && to.gain < from.gain);
  const t1 = now + (closing ? MIX_RAMP.close : MIX_RAMP.open);
  return {
    lpf: { kind: 'exp', from: from.lpf, to: to.lpf, t0: now, t1 },
    gain: { kind: 'lin', from: from.gain, to: to.gain, t0: now, t1 },
  };
}

/** A ramp that is already at `value`, for a change applied without gliding. */
export function holdRamp(kind: Ramp['kind'], value: number, at: number): Ramp {
  return { kind, from: value, to: value, t0: at, t1: at };
}

/**
 * Where a ramp is at time `t`: its start before `t0`, its end from `t1`, and between them the exponential `from * (to / from) ^ u`
 * or the straight line, where `u` runs 0 to 1. A change in mid-ramp starts from here, so a rapid toggle never jumps.
 */
export function valueAt(r: Ramp, t: number): number {
  if (r.t1 <= r.t0 || t >= r.t1) return r.to;
  if (t <= r.t0) return r.from;
  const u = (t - r.t0) / (r.t1 - r.t0);
  return r.kind === 'exp' ? r.from * Math.pow(r.to / r.from, u) : r.from + (r.to - r.from) * u;
}

/** The frequency to give the filter for a cutoff of `hz` on a context whose Nyquist frequency is `nyquist`: open means Nyquist, where a biquad low-pass is transparent (at 20 kHz it is not). */
export function lpfHz(nyquist: number, hz: number): number {
  return hz >= MIX_OPEN_LPF ? nyquist : hz;
}

/**
 * Sets `param` to run along `ramp`, from its value at the ramp's start: cancel what is scheduled, set the start, ramp to the end.
 * (No `cancelAndHoldAtTime`: Firefox lacks it. The start value comes from `valueAt` instead.)
 */
export function scheduleRamp(param: AudioParam, ramp: Ramp, map: (v: number) => number = (v) => v): void {
  param.cancelScheduledValues(ramp.t0);
  param.setValueAtTime(map(ramp.from), ramp.t0);
  if (ramp.t1 <= ramp.t0) return;
  if (ramp.kind === 'exp') param.exponentialRampToValueAtTime(map(ramp.to), ramp.t1);
  else param.linearRampToValueAtTime(map(ramp.to), ramp.t1);
}
