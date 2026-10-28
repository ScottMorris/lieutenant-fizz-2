// Plays the rules' sound events on the Web: the chicken's cluck as the rich Undertone effect in assets/sfx/cluck.ts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The effect definitions are imported from assets/sfx/, never copied, so the Web, the DOS downmix and the WAV previews share one
// source. Each `SfxVoice` becomes an Undertone voice the way Fizz 1's `buildVoice` (packages/engine/src/audio.ts) does it, and a
// variant is a stack of those voices played once on the page's `AudioContext`.
//
// Browsers only let a page make sound after a user gesture, so nothing is created until `unlock()` runs from a key press or a
// click. A cluck that arrives before then, while muted, while the tab is hidden or while the context is not running is dropped and
// never queued: sound that arrives late is worse than none. Nothing here touches the rules, so it cannot change a state hash.
import * as Undertone from '@liminal-hq/undertone';
import type { AudioContextLike } from '@liminal-hq/undertone';
import { CLUCK } from '../assets/sfx/cluck.ts';
import { effectSeconds, isNoise, type SfxVoice } from '../assets/sfx/voice.ts';

/** Most clucks sounding at once. Further ones are dropped, so a pile-up cannot clip. */
export const MAX_SIMULTANEOUS = 4;

/** Seconds past an effect's last voice that it still counts as sounding: the release of its last note. */
const TAIL = 0.05;

/** The `localStorage` key holding the mute choice, `off` or `on`. */
export const MUTE_KEY = 'fizz2-sound';

/** The `localStorage` key holding the placement choice, `classic` or `enhanced`. */
export const AUDIO_KEY = 'fizz2-audio';

/**
 * How effects are placed. Classic plays every sound centred at full level, as the original game did. Enhanced places a sound by
 * where its source is on screen (Fizz 1's `placeSound`, worked out by `fizz-present-web` with the same function DOS uses): an
 * equal-power pan, and a quieter sound when the source is outside the view. Enhanced is the default.
 */
export type AudioMode = 'classic' | 'enhanced';

/** The mode a stored or queried word names; anything else is the default, Enhanced. */
export function parseAudioMode(word: string | null | undefined): AudioMode {
  return word === 'classic' ? 'classic' : 'enhanced';
}

/** The voices of cluck `variant`. A variant out of range wraps, as on DOS, so a bad value is a different cluck and never a failure. */
export function cluckVoices(variant: number): readonly SfxVoice[] {
  const n = CLUCK.length;
  return CLUCK[((Math.trunc(variant) % n) + n) % n];
}

/** The part of the Undertone module this file uses (the module, or a test double). */
export type UndertoneLike = Pick<typeof Undertone, 'note' | 'sound' | 'stack'>;

type Voice = ReturnType<typeof Undertone.stack>;

/**
 * One Undertone voice for an `SfxVoice`: Fizz 1's mapping with a sustain of 0, so the voice is gone after its attack and decay.
 * `pan` runs from -1 (left) to 1 (right); 0 adds no panner node, and Undertone's panner is an equal-power `StereoPannerNode`.
 * `gain` scales the voice's level (the distance, 1 for none), so the placed cluck is the authored one with a pan and a gain
 * applied to each of its voices.
 *
 * `lpenv` differs from Fizz 1 on purpose. `SfxVoice` says the filter opens *to* `lpenv` Hz, as the WAV previews and the downmix
 * read it, but Undertone 0.2 adds its amount to `lpf`, so the amount handed over is `lpenv - lpf`.
 */
export function buildVoice(U: UndertoneLike, v: SfxVoice, pan = 0, gain = 1): Voice {
  let p = isNoise(v.n) ? U.sound(v.n) : U.note(v.hz ?? v.n).sound(v.w ?? 'triangle');
  p = p
    .attack(v.a ?? 0.001)
    .decay(v.d ?? 0.1)
    .sustain(0)
    .release(v.r ?? 0.05)
    .gain(v.g * gain);
  if (pan !== 0) p = p.pan(pan);
  if (v.lpf) p = p.lpf(v.lpf);
  if (v.lpf && v.lpenv && v.lpenv > v.lpf) {
    p = p
      .lpenv(v.lpenv - v.lpf)
      .lpa(v.lpa ?? 0.001)
      .lpd(v.lpd ?? 0.1)
      .lps(v.lps ?? 0)
      .lpr(v.lpr ?? 0.05);
  }
  if (v.nudge) p = p.nudge(v.nudge);
  return p;
}

/** The slice of `AudioContext` this file needs. */
export interface ContextLike extends AudioContextLike {
  readonly state: string;
  resume(): Promise<void>;
  suspend(): Promise<void>;
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface SfxOptions {
  undertone?: UndertoneLike;
  /** Makes the page's audio context. The default is the browser's `AudioContext`. */
  createContext?: () => ContextLike;
  /** Whether the tab is hidden. The default reads `document.hidden`. */
  isHidden?: () => boolean;
  /** Where the mute choice is kept. The default is `localStorage`, when the browser allows it. */
  storage?: StorageLike | null;
  /** The page's query string, for `?audio=classic`. The default is `location.search`. */
  search?: string;
}

function defaultContext(): ContextLike {
  const w = globalThis as unknown as { AudioContext?: new () => ContextLike; webkitAudioContext?: new () => ContextLike };
  const Ctor = w.AudioContext ?? w.webkitAudioContext;
  if (!Ctor) throw new Error('no AudioContext in this browser');
  return new Ctor();
}

function defaultSearch(): string {
  try {
    return globalThis.location?.search ?? '';
  } catch {
    return '';
  }
}

function defaultStorage(): StorageLike | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export class Sfx {
  private readonly U: UndertoneLike;
  private readonly createContext: () => ContextLike;
  private readonly isHidden: () => boolean;
  private readonly storage: StorageLike | null;
  private ctx: ContextLike | null = null;
  private failed = false;
  private _muted: boolean;
  private _mode: AudioMode;
  private readonly cache = new Map<number, Voice>();
  /** When each effect still sounding ends, on the context's clock. */
  private ends: number[] = [];

  constructor(options: SfxOptions = {}) {
    this.U = options.undertone ?? Undertone;
    this.createContext = options.createContext ?? defaultContext;
    this.isHidden = options.isHidden ?? (() => typeof document !== 'undefined' && document.hidden);
    this.storage = options.storage === undefined ? defaultStorage() : options.storage;
    this._muted = this.readMuted();
    const queried = new URLSearchParams(options.search ?? defaultSearch()).get('audio');
    // A `?audio=` query chooses the mode for this visit and wins over the remembered choice, which it does not overwrite.
    this._mode = queried === 'classic' || queried === 'enhanced' ? parseAudioMode(queried) : this.readMode();
  }

  /** Classic or Enhanced. */
  get mode(): AudioMode {
    return this._mode;
  }

  /** Chooses the mode and remembers it. */
  setMode(mode: AudioMode): AudioMode {
    this._mode = mode;
    try {
      this.storage?.setItem(AUDIO_KEY, mode);
    } catch {
      // The choice just is not remembered.
    }
    return mode;
  }

  get muted(): boolean {
    return this._muted;
  }

  /** Whether the audio context exists, which is after the first call to `unlock()`. */
  get started(): boolean {
    return this.ctx !== null;
  }

  /**
   * Makes the audio context on the first call and resumes it on every call. Call it from a key or pointer event handler, which is
   * the gesture the browser wants. Does nothing, and never throws, if the browser has no audio.
   */
  unlock(): void {
    if (this.failed) return;
    try {
      this.ctx ??= this.createContext();
      if (!this.isHidden() && !this._muted && this.ctx.state !== 'running') void this.ctx.resume().catch(() => {});
    } catch (err) {
      this.failed = true;
      console.warn('Sound is unavailable', err);
    }
  }

  /** Mutes or unmutes, remembers the choice and returns it. */
  setMuted(muted: boolean): boolean {
    this._muted = muted;
    try {
      this.storage?.setItem(MUTE_KEY, muted ? 'off' : 'on');
    } catch {
      // The choice just is not remembered.
    }
    this.sync();
    return muted;
  }

  toggleMute(): boolean {
    return this.setMuted(!this._muted);
  }

  /** Call when the tab is hidden or shown: a hidden tab is suspended and a shown one resumed. */
  sync(): void {
    const ctx = this.ctx;
    if (!ctx) return;
    try {
      if (this.isHidden() || this._muted) {
        if (ctx.state === 'running') void ctx.suspend().catch(() => {});
      } else if (ctx.state !== 'running') {
        void ctx.resume().catch(() => {});
      }
    } catch {
      // A closed context cannot be resumed; the next cluck is dropped.
    }
  }

  /**
   * Plays cluck `variant` now, if it can be heard: sound has been unlocked, is not muted, the tab is visible, the context is
   * running and fewer than `MAX_SIMULTANEOUS` clucks are still sounding. Returns whether it was scheduled.
   *
   * `pan` is the stereo position from -1 (left) to 1 (right) and `gain` the distance loudness from 0 to 1, both from the presenter
   * (`App.cluck_pan()` and `App.cluck_gain()`). In Classic mode both are ignored and the cluck plays centred at full level. Each
   * voice of the cluck gets an equal-power panner and the gain; a centred full-level cluck is cached, a placed one is built each time.
   */
  playCluck(variant: number, pan = 0, gain = 1): boolean {
    const ctx = this.ctx;
    if (!ctx || this._muted || this.isHidden() || ctx.state !== 'running') return false;
    const now = ctx.currentTime;
    this.ends = this.ends.filter((end) => end > now);
    if (this.ends.length >= MAX_SIMULTANEOUS) return false;
    const voices = cluckVoices(variant);
    if (this._mode === 'classic' || !Number.isFinite(pan) || !Number.isFinite(gain)) {
      pan = 0;
      gain = 1;
    }
    pan = Math.max(-1, Math.min(1, pan));
    gain = Math.max(0, Math.min(1, gain));
    try {
      const centred = pan === 0 && gain === 1;
      let fx = centred ? this.cache.get(Math.trunc(variant)) : undefined;
      if (!fx) {
        fx = this.U.stack(...voices.map((v) => buildVoice(this.U, v, pan, gain)));
        if (centred) this.cache.set(Math.trunc(variant), fx);
      }
      fx.play({ ctx });
    } catch (err) {
      console.warn('A cluck could not be played', err);
      return false;
    }
    this.ends.push(now + effectSeconds(voices) + TAIL);
    return true;
  }

  private readMode(): AudioMode {
    try {
      return parseAudioMode(this.storage?.getItem(AUDIO_KEY));
    } catch {
      return 'enhanced';
    }
  }

  private readMuted(): boolean {
    try {
      return this.storage?.getItem(MUTE_KEY) === 'off';
    } catch {
      return false;
    }
  }
}
