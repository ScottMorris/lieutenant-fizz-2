// The page's one audio root: a single AudioContext, a music bus and an sfx bus, and a gentle limiter on the master.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `routedContext` is `packages/engine/src/sound-graph.ts` (lines 19-39) of ~/source/liminal-hq/lieutenant-fizz at commit
// 4bce036, the same author's code, Apache-2.0 OR MIT there and here; only the type of the destination changed.
//
// Undertone connects every voice to `ctx.destination`. Handing it `routedContext(ctx, bus)` as the context to play into sends the
// voices into `bus` instead, so the cluck and the music share one context but have their own level:
//
//   cluck voices  -> sfx gain ---------------------------------\
//                                                                +-> limiter (DynamicsCompressor) -> speakers
//   music voices  -> music gain -> mix low-pass -> mix gain -----/
//
// The mix low-pass and gain (web/mix.ts) are the screen mix: they muffle and lower the music while Paused or Options holds the game.
// They are on the music's way only, so the effects are never filtered.
//
// Nothing here makes sound by itself, and nothing is created until `context()` is first called from a user gesture.
import type { AudioContextLike } from '@liminal-hq/undertone';
import { holdRamp, lpfHz, MIX, MIX_Q, mixRamps, scheduleRamp, valueAt, type MixName, type MixShape, type Ramp } from './mix.ts';

/** The level of the music bus. See `docs/MUSIC.md` for why it is not 1. */
export const MUSIC_BUS_GAIN = 0.65;

/** The level of the sfx bus: the cluck as it always was. */
export const SFX_BUS_GAIN = 1;

/** The slice of `AudioContext` the root needs: Undertone's, plus the clock state and the limiter. */
export interface RootContext extends AudioContextLike {
  readonly state: string;
  resume(): Promise<void>;
  suspend(): Promise<void>;
  createDynamicsCompressor(): DynamicsCompressorNode;
}

/** A view of `ctx` whose `destination` is `destination`; every other member delegates to the real context. */
export function routedContext(ctx: AudioContextLike, destination: unknown): AudioContextLike {
  return {
    get currentTime() {
      return ctx.currentTime;
    },
    get sampleRate() {
      return ctx.sampleRate;
    },
    destination: destination as AudioContextLike['destination'],
    createOscillator: () => ctx.createOscillator(),
    createGain: () => ctx.createGain(),
    createBiquadFilter: () => ctx.createBiquadFilter(),
    createStereoPanner: () => ctx.createStereoPanner(),
    createChannelMerger: (n) => ctx.createChannelMerger(n),
    createBufferSource: () => ctx.createBufferSource(),
    createBuffer: (c, l, r) => ctx.createBuffer(c, l, r),
    createConvolver: () => ctx.createConvolver(),
    createDelay: (max) => ctx.createDelay(max),
    decodeAudioData: (data) => ctx.decodeAudioData(data),
  };
}

export interface AudioRootOptions {
  /** Makes the page's audio context. The default is the browser's `AudioContext`. */
  createContext?: () => RootContext;
}

function defaultContext(): RootContext {
  const w = globalThis as unknown as { AudioContext?: new () => RootContext; webkitAudioContext?: new () => RootContext };
  const Ctor = w.AudioContext ?? w.webkitAudioContext;
  if (!Ctor) throw new Error('no AudioContext in this browser');
  return new Ctor();
}

export class AudioRoot {
  private readonly createContext: () => RootContext;
  private ctx: RootContext | null = null;
  private musicGain: GainNode | null = null;
  private sfxGain: GainNode | null = null;
  private mixLpf: BiquadFilterNode | null = null;
  private mixGain: GainNode | null = null;
  private screen: MixName = 'play';
  private plan: { lpf: Ramp; gain: Ramp } = { lpf: holdRamp('exp', MIX.play.lpf, 0), gain: holdRamp('lin', MIX.play.gain, 0) };

  constructor(options: AudioRootOptions = {}) {
    this.createContext = options.createContext ?? defaultContext;
  }

  /** The context, or null before the first `context()` call (that is, before the first user gesture). */
  get existing(): RootContext | null {
    return this.ctx;
  }

  /** The music bus, or null before the context exists. */
  get musicBus(): GainNode | null {
    return this.musicGain;
  }

  /** The sfx bus, or null before the context exists. */
  get sfxBus(): GainNode | null {
    return this.sfxGain;
  }

  /** The screen whose mix was last asked for (`play` until something asks). */
  get mixScreen(): MixName {
    return this.screen;
  }

  /**
   * Sets which screen the music is heard under (see `MIX` in web/mix.ts). The music bus glides to that row, closing over 0.18 s and
   * opening over 0.35 s, from wherever the last glide had got to, so a rapid toggle never jumps. Asking for the screen already set does
   * nothing. Before the context exists the name is only kept, and the graph is built already at it; while the context is not running
   * (muted, or the page hidden) the change is made at once, so it is already in place when the sound comes back.
   */
  setScreen(name: MixName): void {
    if (name === this.screen) return;
    const to = MIX[name];
    const same = MIX[this.screen].lpf === to.lpf && MIX[this.screen].gain === to.gain;
    this.screen = name;
    const ctx = this.ctx;
    const lpf = this.mixLpf;
    const gain = this.mixGain;
    if (!ctx || !lpf || !gain || same) return;
    const now = ctx.currentTime;
    const hz = (v: number) => lpfHz(ctx.sampleRate / 2, v);
    if (ctx.state !== 'running') {
      this.plan = { lpf: holdRamp('exp', to.lpf, now), gain: holdRamp('lin', to.gain, now) };
    } else {
      const here: MixShape = { lpf: valueAt(this.plan.lpf, now), gain: valueAt(this.plan.gain, now) };
      this.plan = mixRamps(here, to, now);
    }
    scheduleRamp(lpf.frequency, this.plan.lpf, hz);
    scheduleRamp(gain.gain, this.plan.gain);
  }

  /** Makes the context and the graph on the first call; every later call returns the same context. Call it from a gesture. */
  context(): RootContext {
    if (this.ctx) return this.ctx;
    const ctx = this.createContext();
    const music = ctx.createGain();
    music.gain.value = MUSIC_BUS_GAIN;
    const sfx = ctx.createGain();
    sfx.gain.value = SFX_BUS_GAIN;
    // A gentle limiter, not a squeeze: it only acts when the cluck lands on a loud bar. Undertone has no limiter of its own.
    const limiter = ctx.createDynamicsCompressor();
    limiter.threshold.value = -6;
    limiter.knee.value = 6;
    limiter.ratio.value = 6;
    limiter.attack.value = 0.003;
    limiter.release.value = 0.15;
    // The screen mix: music -> low-pass -> gain -> limiter. Built at the row of the screen already asked for, with no glide.
    const row = MIX[this.screen];
    const lpf = ctx.createBiquadFilter();
    lpf.type = 'lowpass';
    lpf.Q.value = MIX_Q;
    lpf.frequency.value = lpfHz(ctx.sampleRate / 2, row.lpf);
    const mix = ctx.createGain();
    mix.gain.value = row.gain;
    this.plan = { lpf: holdRamp('exp', row.lpf, ctx.currentTime), gain: holdRamp('lin', row.gain, ctx.currentTime) };
    music.connect(lpf);
    lpf.connect(mix);
    mix.connect(limiter);
    sfx.connect(limiter);
    limiter.connect(ctx.destination as unknown as AudioNode);
    this.ctx = ctx;
    this.musicGain = music;
    this.sfxGain = sfx;
    this.mixLpf = lpf;
    this.mixGain = mix;
    return ctx;
  }
}
