// The Web music player: loops the level's song (assets/music) with Undertone, on the shared audio root's music bus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `buildVoice` and `loopSong` follow `packages/engine/src/audio.ts` (lines 253-295 and 891-912) and `undertonePan`,
// `mirrorPan` and `partMakeup` are `packages/engine/src/sound-field.ts` (lines 51, 140-159), of ~/source/liminal-hq/lieutenant-fizz
// at commit 4bce036, the same author's code, Apache-2.0 OR MIT there and here. What changed: the "Enhanced" placement is always on,
// there is no Classic path or built-in synth fallback, and the song comes from `assets/music/` rather than Fizz 1's tables.
//
// The song is a stack of streams, each one voice arranged across the song's sections, looped from its end straight back to its
// start by Undertone's `loop` (no gap: the loop is one pattern, scheduled ahead of the clock). Music is a presenter detail: the
// rules never start it, stop it or read its clock, and nothing here can change a state hash.
//
// Browsers only let a page make sound after a gesture, so nothing is scheduled until the page has made the audio context (the
// first key or click, via `Sfx.unlock`) and then called `sync()`. `sync()` starts the song at bar 1 when music is on and sound is
// not muted. Muting suspends the shared context (see `Sfx.sync`), which freezes the song where it is, so unmuting carries on. The
// panel's Music toggle stops the song for good and starts it again from bar 1. If Undertone fails there is no music and one warning.
import * as Undertone from '@liminal-hq/undertone';
import type { AudioContextLike } from '@liminal-hq/undertone';
import { levelSong } from '../assets/music/songs.ts';
import { ARP_STEPS, PART_PAN, type MusicPart, type MusicSong, type PartRole } from '../assets/music/types.ts';
import { routedContext, type AudioRoot } from './audio.ts';

/** The `localStorage` key holding the Music toggle, `off` or `on`. Music is on unless the player turned it off. */
export const MUSIC_KEY = 'fizz2-music';

/** Make-up for a panned part: a mono voice through a stereo panner at centre is 3 dB down (0.707 per side), so panned parts get sqrt 2. */
export const PANNED_MAKEUP = Math.SQRT2;

/** The part of the Undertone module this file uses (the module, or a test double). */
export type UndertoneLike = Pick<typeof Undertone, 'note' | 'sound' | 'stack' | 'arrange' | 'silence'>;

type Voice = ReturnType<typeof Undertone.stack>;

const isPanned = (role: PartRole | undefined): boolean => role !== undefined && PART_PAN[role] !== 0;

/** The pan for Undertone's `.pan()`: a number, a pattern that alternates every step (the arpeggio), or undefined for a centred role. */
export function undertonePan(role: PartRole | undefined): number | string | undefined {
  if (!role || !isPanned(role)) return undefined;
  const p = PART_PAN[role];
  return typeof p === 'number' ? p : `[${p[0]} ${p[1]}]*${ARP_STEPS / 2}`;
}

/** The pan of a partner layer: its part's pan reversed (the alternation swapped), or `side` for a centred role. */
export function mirrorPan(role: PartRole | undefined, side: number): number | string {
  const p = role ? PART_PAN[role] : 0;
  if (typeof p === 'number') return p === 0 ? side : -p;
  return `[${p[1]} ${p[0]}]*${ARP_STEPS / 2}`;
}

/** The make-up gain of a role's voices: 1 for a centred role, `PANNED_MAKEUP` for a panned one. */
export function partMakeup(role: PartRole | undefined): number {
  return isPanned(role) ? PANNED_MAKEUP : 1;
}

/**
 * One Undertone voice for a music part, as Fizz 1's `buildVoice` makes it with the sound field on: the envelope is held for the
 * note length (sustain defaults to 0.3), a panned role gets its pan and the make-up gain, a partner gets the mirrored pan.
 *
 * `lpenv` keeps Fizz 1's meaning on purpose: Undertone adds it to `lpf`, so the filter opens to `lpf + lpenv`. The cluck reads
 * `lpenv` as the Hz it opens to (docs/SOUND.md); music is written for Undertone's reading, so it is handed over as it is.
 */
export function buildPartVoice(U: UndertoneLike, v: MusicPart): Voice {
  let p = v.noise ? U.sound(v.notes as Parameters<UndertoneLike['sound']>[0]) : U.note(v.notes).sound(v.w ?? 'triangle');
  p = p
    .attack(v.a ?? 0.001)
    .decay(v.d ?? 0.1)
    .sustain(v.s ?? 0.3)
    .release(v.r ?? 0.05)
    .gain(v.g * partMakeup(v.role));
  const pan = undertonePan(v.role);
  if (pan !== undefined) p = p.pan(pan);
  if (v.mirror !== undefined) p = p.pan(mirrorPan(v.role, v.mirror));
  if (v.lpf) p = p.lpf(v.lpf);
  if (v.lpenv) {
    p = p
      .lpenv(v.lpenv)
      .lpa(v.lpa ?? 0.001)
      .lpd(v.lpd ?? 0.1)
      .lps(v.lps ?? 0)
      .lpr(v.lpr ?? 0.05);
  }
  if (v.hpf) p = p.hpf(v.hpf);
  if (v.slide) p = p.slide(v.slide);
  if (v.nudge) p = p.nudge(v.nudge);
  if (v.room) p = p.room(v.room).roomsize(6).orbit(1);
  if (v.delay) p = p.delay(v.delay).delaytime(0.33).delayfeedback(0.35).orbit(2);
  return p;
}

/** The whole song as one pattern: one arranged stream per voice, stacked. */
export function songPattern(U: UndertoneLike, song: MusicSong): Voice {
  const silence = U.silence as unknown as Voice;
  const streams = song.streams.map((stream) =>
    U.arrange(
      ...song.cycles.map((cycles, section): [number, Voice] => {
        const part = stream[section];
        return [cycles, part ? buildPartVoice(U, part) : silence];
      }),
    ),
  );
  return U.stack(...streams);
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface MusicOptions {
  undertone?: UndertoneLike;
  /** The song to play. The default is the level's song from `assets/music/songs.ts`. */
  song?: MusicSong;
  /** Where the Music toggle is kept. The default is `localStorage`, when the browser allows it. */
  storage?: StorageLike | null;
  /** Whether sound is muted (`M`). Music never starts while it is. */
  isMuted?: () => boolean;
  /** Replaces Undertone's loop timer (a test steps the scheduler by hand). */
  timer?: { setInterval(cb: () => void, ms: number): unknown; clearInterval(h: unknown): void };
}

function defaultStorage(): StorageLike | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

export class Music {
  private readonly U: UndertoneLike;
  private readonly song: MusicSong;
  private readonly root: AudioRoot;
  private readonly storage: StorageLike | null;
  private readonly isMuted: () => boolean;
  private readonly timer: MusicOptions['timer'];
  private _enabled: boolean;
  private handle: { stop(): void } | null = null;
  private failed = false;

  constructor(root: AudioRoot, options: MusicOptions = {}) {
    this.root = root;
    this.U = options.undertone ?? Undertone;
    this.song = options.song ?? levelSong().song;
    this.storage = options.storage === undefined ? defaultStorage() : options.storage;
    this.isMuted = options.isMuted ?? (() => false);
    this.timer = options.timer;
    this._enabled = this.readEnabled();
  }

  /** Whether the Music toggle is on. */
  get enabled(): boolean {
    return this._enabled;
  }

  /** Whether the song is scheduled and looping (it is frozen, not stopped, while the context is suspended). */
  get playing(): boolean {
    return this.handle !== null;
  }

  /** Turns the Music toggle on or off, remembers it and applies it. Returns the new state. */
  setEnabled(on: boolean): boolean {
    this._enabled = on;
    try {
      this.storage?.setItem(MUSIC_KEY, on ? 'on' : 'off');
    } catch {
      // The choice just is not remembered.
    }
    this.sync();
    return on;
  }

  toggle(): boolean {
    return this.setEnabled(!this._enabled);
  }

  /**
   * Brings the song in line with the toggle, the mute and whether the audio context exists. Call it after anything that could
   * change those: a key press or click (which may have just made the context), `M`, the toggle. It is cheap and does nothing when
   * nothing changed. A muted song that is already playing is left to the suspended context, so unmuting carries on mid-bar.
   */
  sync(): void {
    if (!this._enabled) {
      this.stop();
      return;
    }
    if (this.handle || this.failed || this.isMuted()) return;
    const ctx = this.root.existing;
    const bus = this.root.musicBus;
    if (!ctx || !bus) return;
    try {
      const pattern = songPattern(this.U, this.song);
      const into: AudioContextLike = routedContext(ctx, bus);
      this.handle = pattern.loop({ ctx: into, bpm: this.song.bpm, ...(this.timer ? { timer: this.timer } : {}) } as Parameters<typeof pattern.loop>[0]);
    } catch (err) {
      this.failed = true;
      this.handle = null;
      console.warn('Music is unavailable', err);
    }
  }

  /** Stops scheduling. Notes already scheduled ring out (at most the lookahead), so a stop is clean without a fade. */
  stop(): void {
    const h = this.handle;
    this.handle = null;
    try {
      h?.stop();
    } catch {
      // Nothing to undo.
    }
  }

  private readEnabled(): boolean {
    try {
      return this.storage?.getItem(MUSIC_KEY) !== 'off';
    } catch {
      return true;
    }
  }
}
