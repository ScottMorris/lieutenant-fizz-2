// Tests the Web sound player against the real Undertone and a recording stand-in for the browser's AudioContext.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/sfx.test.ts`. Nothing here makes a sound: the stand-in records which oscillators, noise sources, filters
// and gains Undertone schedules, which proves the mapping from `SfxVoice` to Undertone and the player's rules (first gesture,
// mute, hidden tab, the cap). How a cluck sounds has not been listened to.
import { describe, expect, test } from 'bun:test';
import { EFFECTS, EFFECT_NAMES } from '../assets/sfx/effects.ts';
import { noteHz, isNoise, STEP_RATE, stepOffset, voiceHz, effectSeconds } from '../assets/sfx/voice.ts';
import {
  ALWAYS,
  AUDIO_KEY,
  MAX_SIMULTANEOUS,
  MUTE_KEY,
  Sfx,
  effectVoices,
  parseAudioMode,
  type ContextLike,
  type StorageLike,
} from './sfx.ts';

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

function setup(opts: { hidden?: () => boolean; storage?: StorageLike | null; search?: string } = {}) {
  const ctx = new FakeContext();
  const sfx = new Sfx({
    createContext: () => ctx,
    isHidden: opts.hidden ?? (() => false),
    storage: opts.storage ?? new Memory(),
    search: opts.search ?? '',
  });
  return { ctx, sfx };
}

/** Lets the stand-in's `resume()` promise settle, as the browser's would. */
const settle = () => new Promise((r) => setTimeout(r, 0));

describe('variant mapping', () => {
  test('each id maps to its own table entry and a bad value wraps', () => {
    expect(EFFECTS.length).toBe(19);
    expect(EFFECT_NAMES.length).toBe(EFFECTS.length);
    for (let v = 0; v < EFFECTS.length; v++) expect(effectVoices(v)).toBe(EFFECTS[v]);
    expect(effectVoices(19)).toBe(EFFECTS[0]);
    expect(effectVoices(-1)).toBe(EFFECTS[18]);
  });

  test('every cluck variant schedules one source per voice, at the voice pitch, kind and start', async () => {
    for (let variant = 0; variant < 4; variant++) {
      const { ctx, sfx } = setup();
      sfx.unlock();
      await settle();
      expect(sfx.playEffect(variant)).toBe(true);
      const voices = EFFECTS[variant];
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
    sfx.playEffect(1);
    const filtered = EFFECTS[1].filter((v) => v.lpf);
    const voice = filtered.find((v) => v.lpenv)!;
    const filter = ctx.of('filter')[filtered.indexOf(voice)];
    const peaks = [...filter.frequency!.ramps.map((r) => r[0]), ...filter.frequency!.sets.map((s) => s[0])];
    expect(Math.max(...peaks)).toBeCloseTo(voice.lpenv!, 6);
  });

  test('a pan other than centre adds a panner per voice', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    sfx.playEffect(1, -0.5);
    expect(ctx.of('pan').length).toBe(EFFECTS[1].length);
  });
});

describe('the effects after the cluck (ids 4 to 18)', () => {
  /** Plays effect `id` centred on a fresh stand-in and returns what it scheduled. */
  async function play(id: number, pan = 0, gain = 1) {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    expect(sfx.playEffect(id, pan, gain)).toBe(true);
    return ctx;
  }

  for (let id = 4; id < EFFECTS.length; id++) {
    const voices = EFFECTS[id];
    describe(`${id} ${EFFECT_NAMES[id]}`, () => {
      test('schedules one carrier per pitched voice, one modulator per FM voice and one source per noise voice', async () => {
        const ctx = await play(id);
        const tones = voices.filter((v) => !isNoise(v.n));
        const fms = tones.filter((v) => v.fm);
        expect(ctx.of('noise').length).toBe(voices.length - tones.length);
        expect(ctx.of('osc').length).toBe(tones.length + fms.length);
        // Every carrier starts at its voice's nudge, with the voice's exact pitch; an FM voice's modulator follows its carrier.
        const oscs = ctx.of('osc');
        let at = 0;
        for (const v of tones) {
          const carrier = oscs[at++];
          expect(carrier.started).toBeCloseTo(v.nudge ?? 0, 6);
          if (v.fm) {
            const modulator = oscs[at++];
            expect(modulator.started).toBeCloseTo(v.nudge ?? 0, 6);
            expect(modulator.type).toBe(v.fm.wave ?? 'sine');
          }
        }
      });

      test('has a length the player knows, and a caption-free, loud-enough voice at the top', () => {
        const seconds = effectSeconds(voices);
        expect(seconds).toBeGreaterThan(0.04);
        expect(seconds).toBeLessThan(0.9);
        expect(Math.max(...voices.map((v) => v.g))).toBeGreaterThanOrEqual(0.2);
      });

      test('schedules these values, so a later Undertone that moves any shows up here', async () => {
        const ctx = await play(id);
        expect(schedule(ctx)).toMatchSnapshot();
      });
    });
  }

  test('fzzt (4), boing (12) and jump (13) are Fizz 1 Enhanced: plain oscillators, no FM and no steps; 15, 16 and 18 are the FM-era alternates', () => {
    // Fizz 1's jump (13): a square e4 chirp rising nine semitones in four notes (e4, g4, a#4, c#5), a 2600 Hz low-pass, no FM, no steps.
    expect(EFFECTS[13].length).toBe(4);
    expect(EFFECTS[13].map((v) => Math.round(voiceHz(v)))).toEqual([330, 392, 466, 554]);
    expect(EFFECTS[13].map((v) => v.nudge)).toEqual([0, 0.025, 0.05, 0.075]);
    expect(EFFECTS[13].every((v) => v.w === 'square' && v.lpf === 2600 && !v.fm && !v.steps)).toBe(true);
    // Fizz 1's boing: sixteen triangle g3 notes swinging five semitones at nine swings a second, fading, and the sine d5 at 0.07 s.
    const body = EFFECTS[12].filter((v) => v.w === 'triangle');
    expect(body.length).toBe(16);
    expect(body[1].hz).toBeCloseTo(noteHz('g3') * 2 ** (5 * Math.sin((2 * Math.PI * 9 * 0.4) / 15) * (14 / 15) / 12), 1);
    expect(EFFECTS[12].filter((v) => v.w === 'sine').map((v) => [v.n, v.nudge])).toEqual([['d5', 0.07]]);
    // Fizz 1's fzzt: a white noise burst through a 3 kHz high-pass and a c6 sine that slides down an octave in 80 ms, with the shaped filter.
    expect(EFFECTS[4].length).toBe(2);
    expect(EFFECTS[4][0]).toMatchObject({ n: 'white', d: 0.08, g: 0.22, hpf: 3000 });
    expect(EFFECTS[4][1]).toMatchObject({ n: 'c6', w: 'sine', d: 0.12, g: 0.25, slide: 0.08 });
    expect(EFFECTS[4][1].lpf).toBeCloseTo(noteHz('c6') * 4, 6);
    expect(EFFECTS[4][1].lpenv).toBeCloseTo(noteHz('c6') * 4 * 1.2, 6);
    for (const id of [4, 12, 13]) expect(EFFECTS[id].some((v) => v.fm || v.steps)).toBe(false);
    // The alternates keep the FM-era voices; no event plays them.
    expect(EFFECT_NAMES.slice(12)).toEqual(['boing', 'jump', 'crunch', 'boing (FM)', 'jump (FM)', 'reserved (can pickup)', 'fzzt (FM)']);
    expect(EFFECTS[16].length).toBe(1);
    expect(EFFECTS[16][0].steps).toBeDefined();
    expect(EFFECTS[15].some((v) => v.fm)).toBe(true);
    expect(EFFECTS[18].some((v) => v.fm && v.steps)).toBe(true);
  });

  test('the slide and the high-pass reach Undertone: fzzt schedules a high-pass filter and a falling carrier', async () => {
    const ctx = await play(4);
    const highpass = ctx.of('filter').filter((f) => f.type === 'highpass');
    expect(highpass.length).toBe(1);
    expect(highpass[0].frequency!.sets[0][0]).toBeCloseTo(3000, 3);
    // The carrier starts an octave above c6 and ramps to it over the slide.
    const carrier = ctx.of('osc')[0];
    const all = [...carrier.frequency!.sets, ...carrier.frequency!.ramps].map((x) => x[0]);
    expect(Math.max(...all)).toBeCloseTo(noteHz('c6') * 2, 1);
    expect(Math.min(...all)).toBeCloseTo(noteHz('c6'), 1);
  });

  test('pitch steps become the carrier pitch a 140 Hz frame at a time', async () => {
    // Effect 18 (fzzt (FM)): the sine falls an octave from c7 to c6 in 80 ms, then holds.
    const voice = EFFECTS[18].find((v) => v.steps)!;
    const ctx = await play(18);
    const carrier = ctx.of('osc')[0];
    const times = carrier.frequency!.sets;
    expect(times.length + carrier.frequency!.ramps.length).toBeGreaterThan(8);
    const first = carrier.frequency!.sets[0][0];
    expect(first).toBeCloseTo(voiceHz(voice) * 2 ** (stepOffset(voice, 0) / 12), 1);
    expect(stepOffset(voice, 0)).toBe(12);
    expect(stepOffset(voice, 10)).toBe(0);
    expect(STEP_RATE).toBe(140);
  });

  test('FM depth is the Tone index over the harmonicity at the modulator, and the modulator runs at the ratio times the pitch', async () => {
    // Effect 8 (ta-da!), its last voice: the long e6 bell at 2.76 times, index 1.2.
    const ctx = await play(8);
    const oscs = ctx.of('osc');
    const bell = EFFECTS[8][4];
    const carrier = oscs[oscs.length - 2];
    const modulator = oscs[oscs.length - 1];
    expect(carrier.frequency!.sets[0][0]).toBeCloseTo(noteHz('e6'), 3);
    expect(modulator.frequency!.sets[0][0]).toBeCloseTo(noteHz('e6') * bell.fm!.ratio, 2);
  });

  test('a placed effect adds a panner to every voice, FM voices included', async () => {
    const ctx = await play(8, -0.4, 0.7);
    expect(ctx.of('pan').length).toBe(EFFECTS[8].length);
  });

  test('whoa! and ta-da! are heard even with the cap reached; others are not', async () => {
    expect(ALWAYS).toEqual([7, 8]);
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    for (let i = 0; i < MAX_SIMULTANEOUS; i++) expect(sfx.playEffect(13)).toBe(true);
    expect(sfx.playEffect(13)).toBe(false);
    expect(sfx.playEffect(7)).toBe(true);
    expect(sfx.playEffect(8)).toBe(true);
    ctx.currentTime += 2;
    expect(sfx.playEffect(13)).toBe(true);
  });

  test('the same effect played twice schedules the same values (no hidden state), noise sources aside', async () => {
    for (let id = 4; id < EFFECTS.length; id++) {
      const a = schedule(await play(id));
      const b = schedule(await play(id));
      expect(b).toEqual(a);
    }
  });
});

describe('when a cluck is heard', () => {
  test('nothing is made or scheduled before unlock', () => {
    const { ctx, sfx } = setup();
    expect(sfx.playEffect(0)).toBe(false);
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
    expect(sfx.playEffect(2)).toBe(true);
  });

  test('a context that is not running yet plays nothing', () => {
    const { sfx, ctx } = setup();
    sfx.unlock();
    expect(ctx.state).toBe('suspended');
    expect(sfx.playEffect(0)).toBe(false);
  });

  test('mute silences, is remembered and survives a new player', async () => {
    const storage = new Memory();
    const { ctx, sfx } = setup({ storage });
    sfx.unlock();
    await settle();
    expect(sfx.toggleMute()).toBe(true);
    expect(storage.getItem(MUTE_KEY)).toBe('off');
    expect(sfx.playEffect(0)).toBe(false);
    expect(ctx.nodes.length).toBe(0);
    const again = new Sfx({ createContext: () => ctx, isHidden: () => false, storage });
    expect(again.muted).toBe(true);
    expect(sfx.toggleMute()).toBe(false);
    expect(storage.getItem(MUTE_KEY)).toBe('on');
    await settle();
    expect(sfx.playEffect(0)).toBe(true);
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
    expect(sfx.playEffect(0)).toBe(false);
    hidden = false;
    sfx.sync();
    await settle();
    expect(sfx.playEffect(0)).toBe(true);
  });

  test('no more than the cap sound at once, and room returns as they end', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    for (let i = 0; i < MAX_SIMULTANEOUS; i++) expect(sfx.playEffect(1)).toBe(true);
    expect(sfx.playEffect(1)).toBe(false);
    ctx.currentTime += 1;
    expect(sfx.playEffect(1)).toBe(true);
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
    expect(sfx.playEffect(0)).toBe(false);
  });
});

/** The loudest value any gain node of the context is given, over its sets and ramps: the voice peak the cluck scheduled. */
const peakGain = (ctx: FakeContext) =>
  Math.max(...ctx.of('gain').flatMap((n) => [...n.gain!.sets.map((x) => x[0]), ...n.gain!.ramps.map((x) => x[0])]));

describe('placed clucks (Enhanced) and Classic', () => {
  test('Enhanced is the default and applies an equal-power panner and the gain to every voice', async () => {
    const { ctx, sfx } = setup();
    expect(sfx.mode).toBe('enhanced');
    sfx.unlock();
    await settle();
    sfx.playEffect(1);
    const centred = peakGain(ctx);
    expect(ctx.of('pan').length).toBe(0);

    const placed = setup();
    placed.sfx.unlock();
    await settle();
    expect(placed.sfx.playEffect(1, 0.6, 0.5)).toBe(true);
    const panners = placed.ctx.of('pan');
    expect(panners.length).toBe(EFFECTS[1].length);
    for (const p of panners) expect(p.pan!.sets[0][0]).toBeCloseTo(0.6, 6);
    expect(peakGain(placed.ctx)).toBeCloseTo(centred * 0.5, 6);
  });

  test('a left source pans left and a right one right, and the pan and gain are clamped', async () => {
    for (const [pan, want] of [[-0.6, -0.6], [0.6, 0.6], [-5, -1], [5, 1]] as const) {
      const { ctx, sfx } = setup();
      sfx.unlock();
      await settle();
      sfx.playEffect(0, pan, 7);
      expect(ctx.of('pan')[0].pan!.sets[0][0]).toBeCloseTo(want, 6);
    }
    const base = setup();
    base.sfx.unlock();
    await settle();
    base.sfx.playEffect(0);
    const over = setup();
    over.sfx.unlock();
    await settle();
    over.sfx.playEffect(0, 0, 7);
    expect(peakGain(over.ctx)).toBeCloseTo(peakGain(base.ctx), 6);
    // Not a number: played centred at full level, never thrown.
    const bad = setup();
    bad.sfx.unlock();
    await settle();
    expect(bad.sfx.playEffect(0, Number.NaN, Number.NaN)).toBe(true);
    expect(bad.ctx.of('pan').length).toBe(0);
  });

  test('Classic plays everything centred at full level whatever the position', async () => {
    const base = setup();
    base.sfx.unlock();
    await settle();
    base.sfx.playEffect(2);

    const { ctx, sfx } = setup();
    sfx.setMode('classic');
    expect(sfx.mode).toBe('classic');
    sfx.unlock();
    await settle();
    expect(sfx.playEffect(2, -0.6, 0.4)).toBe(true);
    expect(ctx.of('pan').length).toBe(0);
    expect(peakGain(ctx)).toBeCloseTo(peakGain(base.ctx), 6);
  });

  test('the mode is remembered, defaults to Enhanced and tolerates junk and blocked storage', () => {
    const mem = new Memory();
    const first = setup({ storage: mem }).sfx;
    expect(first.mode).toBe('enhanced');
    first.setMode('classic');
    expect(mem.getItem(AUDIO_KEY)).toBe('classic');
    expect(setup({ storage: mem }).sfx.mode).toBe('classic');
    first.setMode('enhanced');
    expect(setup({ storage: mem }).sfx.mode).toBe('enhanced');
    mem.setItem(AUDIO_KEY, 'loud');
    expect(setup({ storage: mem }).sfx.mode).toBe('enhanced');
    expect(parseAudioMode(null)).toBe('enhanced');
    expect(parseAudioMode('classic')).toBe('classic');
    const blocked: StorageLike = {
      getItem() {
        throw new Error('blocked');
      },
      setItem() {
        throw new Error('blocked');
      },
    };
    const sfx = setup({ storage: blocked }).sfx;
    expect(sfx.mode).toBe('enhanced');
    expect(sfx.setMode('classic')).toBe('classic');
    expect(sfx.mode).toBe('classic');
    expect(setup({ storage: null }).sfx.mode).toBe('enhanced');
  });

  test('?audio=classic chooses Classic for the visit over the remembered choice, and ?audio=enhanced the reverse', () => {
    const mem = new Memory();
    mem.setItem(AUDIO_KEY, 'enhanced');
    expect(setup({ storage: mem, search: '?audio=classic' }).sfx.mode).toBe('classic');
    expect(mem.getItem(AUDIO_KEY)).toBe('enhanced');
    mem.setItem(AUDIO_KEY, 'classic');
    expect(setup({ storage: mem, search: '?x=1&audio=enhanced' }).sfx.mode).toBe('enhanced');
    expect(setup({ storage: mem, search: '?audio=junk' }).sfx.mode).toBe('classic');
  });
});

/** A compact record of what a cluck variant scheduled: per source node, in creation order, its kind, pitch sets and gain/filter automation. */
function schedule(ctx: FakeContext) {
  const round = (list: [number, number][]) => list.map(([v, t]) => [Number(v.toPrecision(6)), Number(t.toPrecision(6))]);
  return ctx.nodes
    .filter((n) => n.kind !== 'merger')
    .map((n) => ({
      kind: n.kind,
      type: n.type,
      freq: n.frequency ? { sets: round(n.frequency.sets), ramps: round(n.frequency.ramps) } : undefined,
      gain: n.gain ? { sets: round(n.gain.sets), ramps: round(n.gain.ramps) } : undefined,
    }));
}

describe('Undertone 0.3 scheduling pinned for the cluck', () => {
  test('the short cluck (variant 1) schedules exactly these filter, gain and pitch values', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    sfx.playEffect(1);
    const nodes = schedule(ctx);
    // Voice 0 is the white-noise breath (lpf 3000 opening to 3000 + (lpenv - lpf)); voice 1 the square at 640 Hz (lpf 2200, start 12 ms).
    const filters = nodes.filter((n) => n.kind === 'filter');
    expect(filters.length).toBe(EFFECTS[1].filter((v) => v.lpf).length);
    EFFECTS[1].filter((v) => v.lpf).forEach((v, i) => {
      const f = filters[i].freq!;
      expect(f.sets[0][0]).toBeCloseTo(v.lpf!, 3); // rests at lpf
      if (v.lpenv && v.lpenv > v.lpf!) {
        // Fizz 2 hands Undertone lpenv - lpf, which 0.2 and 0.3 both add to lpf: the filter peaks at lpenv, no higher.
        const peak = Math.max(...f.sets.map((s) => s[0]), ...f.ramps.map((r) => r[0]));
        expect(peak).toBeCloseTo(v.lpenv, 3);
      }
    });
    // The whole schedule, so a later Undertone that moves any value shows up here.
    expect(nodes).toMatchSnapshot();
  });

  test('the player builds no compressor or master bus of its own, so 0.3 dropping compressor makeup gain cannot change a cluck', async () => {
    const { ctx, sfx } = setup();
    sfx.unlock();
    await settle();
    for (let v = 0; v < 4; v++) sfx.playEffect(v);
    expect(ctx.nodes.some((n) => n.kind === 'compressor')).toBe(false);
  });

  test('the noise breath is unseeded: each play draws new samples from Math.random, as in 0.2', async () => {
    const buffers: Float32Array[] = [];
    const { ctx, sfx } = setup();
    ctx.createBuffer = (_c: number, length: number) => {
      const data = new Float32Array(length);
      buffers.push(data);
      return { getChannelData: () => data };
    };
    sfx.unlock();
    await settle();
    sfx.playEffect(1);
    sfx.playEffect(1);
    expect(buffers.length).toBe(2);
    expect(buffers[0].length).toBe(buffers[1].length);
    expect(Array.from(buffers[0].slice(0, 32))).not.toEqual(Array.from(buffers[1].slice(0, 32)));
  });
});
