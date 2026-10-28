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
//   cluck voices  -> sfx gain   --\
//                                  +-> limiter (DynamicsCompressor) -> speakers
//   music voices  -> music gain --/
//
// Nothing here makes sound by itself, and nothing is created until `context()` is first called from a user gesture.
import type { AudioContextLike } from '@liminal-hq/undertone';

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
    music.connect(limiter);
    sfx.connect(limiter);
    limiter.connect(ctx.destination as unknown as AudioNode);
    this.ctx = ctx;
    this.musicGain = music;
    this.sfxGain = sfx;
    return ctx;
  }
}
