// Tests the screen mix: the table, the ramps and the automation the audio root records on the music's low-pass and gain.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/mix.test.ts`. Nothing here makes a sound: the stand-in records every automation call in order, which proves
// what the page asks the browser to do (the pattern of Fizz 1: cancelScheduledValues, then setValueAtTime, then the ramp). How it
// sounds has not been listened to.
import { describe, expect, test } from 'bun:test';
import { AudioRoot, type RootContext } from './audio.ts';
import { holdRamp, lpfHz, MIX, MIX_Q, MIX_RAMP, mixRamps, valueAt } from './mix.ts';

type Call = [kind: 'cancel' | 'set' | 'lin' | 'exp', value: number, time: number];

class RecParam {
  value = 0;
  calls: Call[] = [];
  cancelScheduledValues(t: number) {
    this.calls.push(['cancel', NaN, t]);
  }
  setValueAtTime(v: number, t: number) {
    this.calls.push(['set', v, t]);
  }
  linearRampToValueAtTime(v: number, t: number) {
    this.calls.push(['lin', v, t]);
  }
  exponentialRampToValueAtTime(v: number, t: number) {
    this.calls.push(['exp', v, t]);
  }
}

class Ctx {
  state = 'running';
  currentTime = 0;
  sampleRate = 48000;
  destination = { connect() {} };
  gains: { gain: RecParam; connect(): void }[] = [];
  filters: { type: string; frequency: RecParam; Q: RecParam; connect(): void }[] = [];
  createGain() {
    const g = { gain: new RecParam(), connect() {} };
    this.gains.push(g);
    return g as never;
  }
  createBiquadFilter() {
    const f = { type: '', frequency: new RecParam(), Q: new RecParam(), connect() {} };
    this.filters.push(f);
    return f as never;
  }
  createDynamicsCompressor() {
    return { threshold: new RecParam(), knee: new RecParam(), ratio: new RecParam(), attack: new RecParam(), release: new RecParam(), connect() {} } as never;
  }
  async resume() {}
  async suspend() {}
}

function setup(state = 'running') {
  const ctx = new Ctx();
  ctx.state = state;
  const root = new AudioRoot({ createContext: () => ctx as unknown as RootContext });
  root.context();
  const lpf = ctx.filters[0];
  const gain = ctx.gains[2]; // music bus, sfx bus, then the mix gain
  const clear = () => {
    lpf.frequency.calls.length = 0;
    gain.gain.calls.length = 0;
  };
  return { ctx, root, lpf, gain, clear };
}

const NYQ = 24000;

describe('the table', () => {
  test('holds Fizz 1 pause and card numbers, and play is open', () => {
    expect(MIX.play).toEqual({ lpf: 20000, gain: 1 });
    expect(MIX.pause).toEqual({ lpf: 900, gain: 0.7 });
    expect(MIX.options).toEqual(MIX.pause);
    expect(MIX.card).toEqual({ lpf: 2200, gain: 0.8 });
    expect(MIX_RAMP).toEqual({ close: 0.18, open: 0.35 });
    expect(MIX_Q).toBe(0);
  });
  test('closing takes the short time and opening the long one; both parameters arrive together', () => {
    const c = mixRamps(MIX.play, MIX.pause, 1);
    expect(c.lpf.t1).toBeCloseTo(1.18, 9);
    expect(c.gain.t1).toBe(c.lpf.t1);
    expect(c.lpf.kind).toBe('exp');
    expect(c.gain.kind).toBe('lin');
    expect(mixRamps(MIX.pause, MIX.play, 1).lpf.t1).toBeCloseTo(1.35, 9);
  });
  test('valueAt follows the exponential and the line, and an open cutoff is Nyquist', () => {
    const r = mixRamps(MIX.play, MIX.pause, 0);
    expect(valueAt(r.lpf, 0)).toBe(20000);
    expect(valueAt(r.lpf, 0.09)).toBeCloseTo(20000 * Math.pow(900 / 20000, 0.5), 6);
    expect(valueAt(r.gain, 0.09)).toBeCloseTo(0.85, 9);
    expect(valueAt(r.lpf, 5)).toBe(900);
    expect(valueAt(holdRamp('lin', 0.3, 2), 1)).toBe(0.3);
    expect(lpfHz(NYQ, 20000)).toBe(NYQ);
    expect(lpfHz(NYQ, 900)).toBe(900);
  });
});

describe('the audio root mix', () => {
  test('starts open: Nyquist, Q 0, gain 1, and nothing scheduled', () => {
    const { lpf, gain } = setup();
    expect(lpf.type).toBe('lowpass');
    expect(lpf.Q.value).toBe(0);
    expect(lpf.frequency.value).toBe(NYQ);
    expect(gain.gain.value).toBe(1);
    expect(lpf.frequency.calls).toEqual([]);
  });

  test('opening a menu cancels, sets, then ramps: 900 Hz and 0.7 over 0.18 s', () => {
    const { ctx, root, lpf, gain } = setup();
    ctx.currentTime = 2;
    root.setScreen('pause');
    expect(lpf.frequency.calls).toEqual([['cancel', NaN, 2], ['set', NYQ, 2], ['exp', 900, 2.18]] as Call[]);
    expect(gain.gain.calls).toEqual([['cancel', NaN, 2], ['set', 1, 2], ['lin', 0.7, 2.18]] as Call[]);
  });

  test('closing it ramps back to open over 0.35 s, from where the filter is', () => {
    const { ctx, root, lpf, gain, clear } = setup();
    ctx.currentTime = 2;
    root.setScreen('pause');
    clear();
    ctx.currentTime = 10; // long after: fully closed
    root.setScreen('play');
    expect(lpf.frequency.calls).toEqual([['cancel', NaN, 10], ['set', 900, 10], ['exp', NYQ, 10.35]] as Call[]);
    expect(gain.gain.calls).toEqual([['cancel', NaN, 10], ['set', 0.7, 10], ['lin', 1, 10.35]] as Call[]);
  });

  test('Paused to Options is the same mix: nothing is scheduled, the name is kept', () => {
    const { ctx, root, lpf, clear } = setup();
    root.setScreen('pause');
    clear();
    ctx.currentTime = 1;
    root.setScreen('options');
    expect(lpf.frequency.calls).toEqual([]);
    expect(root.mixScreen).toBe('options');
    root.setScreen('options');
    expect(lpf.frequency.calls).toEqual([]);
  });

  test('a rapid toggle starts each glide from where the last had got to: no jump, no stuck state', () => {
    const { ctx, root, lpf, gain, clear } = setup();
    ctx.currentTime = 1;
    root.setScreen('pause'); // closing 1 -> 1.18
    clear();
    ctx.currentTime = 1.09; // half way
    root.setScreen('play');
    const midHz = 20000 * Math.pow(900 / 20000, 0.5);
    expect(lpf.frequency.calls[0]).toEqual(['cancel', NaN, 1.09]);
    expect(lpf.frequency.calls[1]?.[0]).toBe('set');
    expect(lpf.frequency.calls[1]?.[1]).toBeCloseTo(midHz, 6);
    expect(lpf.frequency.calls[2]).toEqual(['exp', NYQ, 1.09 + 0.35]);
    expect(gain.gain.calls[1]?.[1]).toBeCloseTo(0.85, 9);
    expect(gain.gain.calls[2]).toEqual(['lin', 1, 1.09 + 0.35]);
    // And back again before it has opened: from the value it had reached, closing again.
    clear();
    ctx.currentTime = 1.2;
    root.setScreen('pause');
    expect(lpf.frequency.calls[1]?.[0]).toBe('set');
    expect(lpf.frequency.calls[1]?.[1]).toBeGreaterThan(midHz);
    expect(lpf.frequency.calls[1]?.[1]).toBeLessThan(20000);
    expect(lpf.frequency.calls[2]).toEqual(['exp', 900, 1.2 + 0.18]);
    expect(gain.gain.calls[2]).toEqual(['lin', 0.7, 1.2 + 0.18]);
    expect(root.mixScreen).toBe('pause');
  });

  test('every change is one cancel, one set and one ramp, in that order, on both parameters (no zipper)', () => {
    const { ctx, root, lpf, gain } = setup();
    const names: ('pause' | 'play' | 'options')[] = ['pause', 'play', 'options', 'play', 'pause', 'play'];
    let t = 0;
    for (const n of names) {
      t += 0.05;
      ctx.currentTime = t;
      root.setScreen(n);
    }
    for (const calls of [lpf.frequency.calls, gain.gain.calls]) {
      const kinds = calls.map((c) => c[0]);
      expect(kinds.length % 3).toBe(0);
      for (let i = 0; i < kinds.length; i += 3) expect(kinds.slice(i, i + 3)).toEqual(['cancel', 'set', kinds[i + 2]]);
      // The cancel and set times (not the ramp ends, which run on past the next change) never go backwards.
      const times = calls.filter((c) => c[0] === 'cancel').map((c) => c[2]);
      expect([...times].sort((a, b) => a - b)).toEqual(times);
    }
    // Six real changes (pause<->options is not one, but none of these steps is that), three calls each.
    expect(lpf.frequency.calls.length).toBe(18);
  });

  test('a suspended context (muted or hidden) takes the change at once, with no ramp', () => {
    const { ctx, root, lpf, gain } = setup('suspended');
    ctx.currentTime = 3;
    root.setScreen('pause');
    expect(lpf.frequency.calls).toEqual([['cancel', NaN, 3], ['set', 900, 3]] as Call[]);
    expect(gain.gain.calls).toEqual([['cancel', NaN, 3], ['set', 0.7, 3]] as Call[]);
  });

  test('a screen set before the context exists is built in, with no glide', () => {
    const ctx = new Ctx();
    const root = new AudioRoot({ createContext: () => ctx as unknown as RootContext });
    root.setScreen('pause');
    root.context();
    expect(ctx.filters[0].frequency.value).toBe(900);
    expect(ctx.gains[2].gain.value).toBe(0.7);
    expect(ctx.filters[0].frequency.calls).toEqual([]);
  });

  test('the reserved card row would use 2,200 Hz and 0.8', () => {
    const { ctx, root, lpf, gain } = setup();
    ctx.currentTime = 1;
    root.setScreen('card');
    expect(lpf.frequency.calls[2]).toEqual(['exp', 2200, 1.18]);
    expect(gain.gain.calls[2]).toEqual(['lin', 0.8, 1.18]);
  });

  test('effects are untouched: the sfx bus has no filter and no automation', () => {
    const { ctx, root } = setup();
    ctx.currentTime = 1;
    root.setScreen('pause');
    expect(ctx.filters.length).toBe(1);
    expect(ctx.gains[1].gain.calls).toEqual([]); // the sfx bus
    expect(ctx.gains[0].gain.calls).toEqual([]); // the music bus's own level
    expect(root.sfxBus).toBe(ctx.gains[1] as never);
  });
});
