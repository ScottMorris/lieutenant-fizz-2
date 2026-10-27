// Tests the Web sound player against the real Undertone and a recording stand-in for the browser's AudioContext.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/sfx.test.ts`. Nothing here makes a sound: the stand-in records which oscillators, noise sources, filters
// and gains Undertone schedules, which proves the mapping from `SfxVoice` to Undertone and the player's rules (first gesture,
// mute, hidden tab, the cap). How a cluck sounds has not been listened to.
import { describe, expect, test } from 'bun:test';
import { CLUCK } from '../assets/sfx/cluck.ts';
import { noteHz, isNoise } from '../assets/sfx/voice.ts';
import { MAX_SIMULTANEOUS, MUTE_KEY, Sfx, cluckVoices, type ContextLike, type StorageLike } from './sfx.ts';

interface Param {
  value: number;
  sets: [number, number][];
  ramps: [number, number][];
  setValueAtTime(v: number, t: number): void;
  linearRampToValueAtTime(v: number, t: number): void;
  exponentialRampToValueAtTime(v: number, t: number): void;
}

function param(): Param {
  const p: Param = {
    value: 0,
    sets: [],
    ramps: [],
    setValueAtTime(v, t) {
      p.sets.push([v, t]);
    },
    linearRampToValueAtTime(v, t) {
      p.ramps.push([v, t]);
    },
    exponentialRampToValueAtTime(v, t) {
      p.ramps.push([v, t]);
    },
  };
  return p;
}

interface Node {
  kind: string;
  type?: string;
  frequency?: Param;
  gain?: Param;
  Q?: Param;
  pan?: Param;
  started?: number;
  connect(): void;
  start(t?: number): void;
  stop(): void;
  [k: string]: unknown;
}

class FakeContext implements ContextLike {
  state = 'suspended';
  currentTime = 0;
  sampleRate = 44100;
  destination = { connect() {} };
  nodes: Node[] = [];
  resumed = 0;
  suspended = 0;

  private make(kind: string, extra: Partial<Node> = {}): Node {
    const n: Node = {
      kind,
      connect() {},
      start(t = 0) {
        n.started = t;
      },
      stop() {},
      ...extra,
    };
    this.nodes.push(n);
    return n;
  }
  createOscillator() {
    return this.make('osc', { frequency: param() }) as never;
  }
  createGain() {
    return this.make('gain', { gain: param() }) as never;
  }
  createBiquadFilter() {
    return this.make('filter', { frequency: param(), Q: param() }) as never;
  }
  createStereoPanner() {
    return this.make('pan', { pan: param() }) as never;
  }
  createChannelMerger() {
    return this.make('merger') as never;
  }
  createBufferSource() {
    return this.make('noise', { playbackRate: param() }) as never;
  }
  createBuffer(_c: number, length: number) {
    return { getChannelData: () => new Float32Array(length) };
  }
  createConvolver() {
    return this.make('convolver') as never;
  }
  createDelay() {
    return this.make('delay', { delayTime: param() }) as never;
  }
  decodeAudioData() {
    return Promise.reject(new Error('unused'));
  }
  async resume() {
    this.resumed++;
    await Promise.resolve();
    this.state = 'running';
  }
  async suspend() {
    this.suspended++;
    this.state = 'suspended';
  }
  of(kind: string) {
    return this.nodes.filter((n) => n.kind === kind);
  }
}

class Memory implements StorageLike {
  data = new Map<string, string>();
  getItem(k: string) {
    return this.data.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.data.set(k, v);
  }
}

function setup(opts: { hidden?: () => boolean; storage?: StorageLike | null } = {}) {
  const ctx = new FakeContext();
  const sfx = new Sfx({ createContext: () => ctx, isHidden: opts.hidden ?? (() => false), storage: opts.storage ?? new Memory() });
  return { ctx, sfx };
}

/** Lets the stand-in's `resume()` promise settle, as the browser's would. */
const settle = () => new Promise((r) => setTimeout(r, 0));

describe('variant mapping', () => {
  test('each variant maps to its own table entry and a bad value wraps', () => {
    expect(CLUCK.length).toBe(4);
    for (let v = 0; v < 4; v++) expect(cluckVoices(v)).toBe(CLUCK[v]);
    expect(cluckVoices(4)).toBe(CLUCK[0]);
    expect(cluckVoices(-1)).toBe(CLUCK[3]);
  });

  test('every variant schedules one source per voice, at the voice pitch, kind and start', async () => {
    for (let variant = 0; variant < 4; variant++) {
      const { ctx, sfx } = setup();
      sfx.unlock();
      await settle();
      expect(sfx.playCluck(variant)).toBe(true);
      const voices = CLUCK[variant];
      const noise = voices.filter((v) => isNoise(v.n));
      const tones = voices.filter((v) => !isNoise(v.n));
      expect(ctx.of('noise').length).toBe(noise.length);
      expect(ctx.of('osc').length).toBe(tones.length);
      // Oscillators come out in voice order: type, exact pitch and start time follow the voice.
      const oscs = ctx.of('osc');
      tones.forEach((v, i) => {
        expect(oscs[i].type).toBe(v.w ?? 'triangle');
        expect(oscs[i].frequency!.sets[0][0]).toBeCloseTo(v.hz ?? noteHz(v.n), 6);
        expect(oscs[i].started).toBeCloseTo(v.nudge ?? 0, 6);
      });
      // Every voice has a low-pass filter or none, as `lpf` says.
      expect(ctx.of('filter').length).toBe(voices.filter((v) => v.lpf).length);
      // Centre pan adds no panner.
      expect(ctx.of('pan').length).toBe(0);
    }
  });

  test('the filter envelope peaks at lpenv, not lpf plus lpenv', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    sfx.playCluck(1);
    const filtered = CLUCK[1].filter((v) => v.lpf);
    const voice = filtered.find((v) => v.lpenv)!;
    const filter = ctx.of('filter')[filtered.indexOf(voice)];
    const peaks = [...filter.frequency!.ramps.map((r) => r[0]), ...filter.frequency!.sets.map((s) => s[0])];
    expect(Math.max(...peaks)).toBeCloseTo(voice.lpenv!, 6);
  });

  test('a pan other than centre adds a panner per voice', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    sfx.playCluck(1, -0.5);
    expect(ctx.of('pan').length).toBe(CLUCK[1].length);
  });
});

describe('when a cluck is heard', () => {
  test('nothing is made or scheduled before unlock', () => {
    const { ctx, sfx } = setup();
    expect(sfx.playCluck(0)).toBe(false);
    expect(sfx.started).toBe(false);
    expect(ctx.nodes.length).toBe(0);
  });

  test('unlock makes the context once and resumes it', async () => {
    let made = 0;
    const ctx = new FakeContext();
    const sfx = new Sfx({ createContext: () => (made++, ctx), isHidden: () => false, storage: new Memory() });
    sfx.unlock();
    sfx.unlock();
    await settle();
    expect(made).toBe(1);
    expect(ctx.resumed).toBeGreaterThanOrEqual(1);
    expect(sfx.playCluck(2)).toBe(true);
  });

  test('a context that is not running yet plays nothing', () => {
    const { sfx, ctx } = setup();
    sfx.unlock();
    expect(ctx.state).toBe('suspended');
    expect(sfx.playCluck(0)).toBe(false);
  });

  test('mute silences, is remembered and survives a new player', async () => {
    const storage = new Memory();
    const { ctx, sfx } = setup({ storage });
    sfx.unlock();
    await settle();
    expect(sfx.toggleMute()).toBe(true);
    expect(storage.getItem(MUTE_KEY)).toBe('off');
    expect(sfx.playCluck(0)).toBe(false);
    expect(ctx.nodes.length).toBe(0);
    const again = new Sfx({ createContext: () => ctx, isHidden: () => false, storage });
    expect(again.muted).toBe(true);
    expect(sfx.toggleMute()).toBe(false);
    expect(storage.getItem(MUTE_KEY)).toBe('on');
    await settle();
    expect(sfx.playCluck(0)).toBe(true);
  });

  test('blocked storage does not break mute', () => {
    const broken: StorageLike = {
      getItem() {
        throw new Error('blocked');
      },
      setItem() {
        throw new Error('blocked');
      },
    };
    const { sfx } = setup({ storage: broken });
    expect(sfx.muted).toBe(false);
    expect(sfx.toggleMute()).toBe(true);
    expect(sfx.muted).toBe(true);
  });

  test('a hidden tab is silent and suspended, and resumes when shown', async () => {
    let hidden = false;
    const { ctx, sfx } = setup({ hidden: () => hidden });
    sfx.unlock();
    await settle();
    hidden = true;
    sfx.sync();
    await settle();
    expect(ctx.state).toBe('suspended');
    expect(sfx.playCluck(0)).toBe(false);
    hidden = false;
    sfx.sync();
    await settle();
    expect(sfx.playCluck(0)).toBe(true);
  });

  test('no more than the cap sound at once, and room returns as they end', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    for (let i = 0; i < MAX_SIMULTANEOUS; i++) expect(sfx.playCluck(1)).toBe(true);
    expect(sfx.playCluck(1)).toBe(false);
    ctx.currentTime += 1;
    expect(sfx.playCluck(1)).toBe(true);
  });

  test('a browser with no audio leaves the player silent and unbroken', () => {
    const sfx = new Sfx({
      createContext: () => {
        throw new Error('no audio');
      },
      isHidden: () => false,
      storage: new Memory(),
    });
    sfx.unlock();
    expect(sfx.playCluck(0)).toBe(false);
  });
});
