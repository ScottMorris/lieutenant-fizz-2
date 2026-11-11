// The lab's live player: plays a built tune a beat at a time, so a tap can send it to another section in time and in key.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Undertone's own `loop` plays one fixed pattern. To change section while playing, this player asks each section's pattern for one
// beat at a time and books those notes through Undertone's `playParams`, a quarter of a second ahead of the audio clock. Because
// nothing past that is booked, a tap can still change what the rest of the bar plays. Three ways to move:
//
// - bar: the rest of the current bar becomes a "turn". The drums and bass drop out for a timpani roll, a string swell and a bass
//   that walks, from the dominant of the section being called, up two leading notes into its first bass note. The tune and pads
//   of the old section carry on over it, and the new section starts on the barline.
// - beat: the new section takes over on the very next beat, at the same beat of its first bar: a hop with no join.
// - section: the current section finishes, with a turn over its last two beats, and the called one follows.
//
// Left alone, the song plays its sections in order and loops; Hold keeps the current section going round. Every time a section
// comes round its notes are the same and its humanising is new (voices.ts).

import * as U from '@liminal-hq/undertone';
import { Fraction, playParams, type AudioContextLike, type ControlPatch, type PlaybackHandle } from '@liminal-hq/undertone';
import { queryPart } from '../../assets/music/pattern.ts';
import type { BuiltSection, BuiltTune } from './build.ts';
import type { LabPart } from './tunes.ts';
import { BED, sectionPattern, type Mix, type Voice, type VoiceOptions } from './voices.ts';

export type Quantise = 'beat' | 'bar' | 'section';

/** Seconds of notes booked ahead of the audio clock, and how often the player looks. */
export const LOOKAHEAD = 0.25;
export const TICK_MS = 40;
const BEATS = 4;

/** What the player was doing at one beat, for the page to draw. */
export interface Step {
  /** The audio-clock time the beat starts. */
  time: number;
  section: number;
  bar: number;
  beat: number;
  /** Whether this beat is part of a turn into another section. */
  turning: boolean;
}

export interface TimerLike {
  setInterval(callback: () => void, ms: number): unknown;
  clearInterval(handle: unknown): void;
}

const RHYTHM: ReadonlySet<string> = new Set(['kick', 'snare', 'hats', 'perc', 'bass']);
const isRhythm = (part: LabPart): boolean => part.role !== undefined && RHYTHM.has(part.role);

const NAMES = ['c', 'c#', 'd', 'd#', 'e', 'f', 'f#', 'g', 'g#', 'a', 'a#', 'b'];
const name = (midi: number): string => `${NAMES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;

/** The first bass note of a section, or undefined if it has no bass. */
export function firstBass(section: BuiltSection): { midi: number; part: LabPart } | undefined {
  const bass = section.parts.find(({ part }) => part.role === 'bass' && part.mirror === undefined);
  const first = bass && queryPart(bass.part, 0, 1).find((e) => e.midi !== undefined);
  return bass && first ? { midi: first.midi!, part: bass.part } : undefined;
}

/**
 * The turn into a section, as one bar of Undertone pattern: the player sounds only the beats that are left. It is orchestral, not
 * a drum fill. The bass sits on the dominant of the note it is heading for and climbs two semitones into it; a timpani on that
 * dominant rolls faster and louder; and strings swell on the open fifth above it.
 */
export function turnPattern(target: BuiltSection, mix: Mix): Voice {
  const home = firstBass(target);
  if (!home) return U.silence as unknown as Voice;
  const bed = (role: 'bass' | 'kick' | 'pad', v: Voice): Voice => (mix === 'surround' ? v.channels(BED[role]!) : v);
  const dominant = home.midi + 7 > 55 ? home.midi - 5 : home.midi + 7;
  const walk = `${name(dominant)} ${name(dominant)} ${name(dominant)} [${name(home.midi - 2)} ${name(home.midi - 1)}]`;
  const drum = name(dominant - 12);
  return U.stack(
    bed(
      'bass',
      U.note(walk)
        .sound(home.part.w ?? 'triangle')
        .attack(0.005)
        .decay(0.1)
        .sustain(0.5)
        .release(0.06)
        .lpf(home.part.lpf ?? 2600)
        .gain(home.part.g),
    ),
    bed('kick', U.note(`${drum}*2 ${drum}*2 ${drum}*3 ${drum}*4`).sound('sine').attack(0.004).decay(0.2).sustain(0).release(0.12).lpf(400).gain(home.part.g * 1.3).velocity('.45 .6 .8 1')),
    bed(
      'pad',
      U.note(`[${name(dominant + 12)},${name(dominant + 19)}]*4`)
        .sound('sawtooth')
        .attack(0.12)
        .decay(0.1)
        .sustain(0.9)
        .release(0.1)
        .lpf(1400)
        .gain(home.part.g * 0.16)
        .velocity('.4 .55 .75 1'),
    ),
  ) as Voice;
}

/** The notes of `pattern` that start in one beat of bar `cycle`, each with the seconds after the bar's start it begins. */
export function beatOnsets(pattern: Voice, cycle: number, beat: number, barSeconds: number): { at: number; patch: ControlPatch }[] {
  const begin = new Fraction(cycle * BEATS + beat, BEATS);
  const end = new Fraction(cycle * BEATS + beat + 1, BEATS);
  const out: { at: number; patch: ControlPatch }[] = [];
  for (const hap of pattern.query({ begin, end })) {
    if (!hap.whole || !hap.whole.begin.eq(hap.part.begin)) continue;
    const length = hap.whole.end.sub(hap.whole.begin).toNumber() * barSeconds;
    const legato = hap.value.legato;
    out.push({
      at: (hap.whole.begin.toNumber() - cycle) * barSeconds,
      patch: { duration: legato !== undefined ? length * legato : length, ...hap.value },
    });
  }
  return out;
}

export class LabPlayer {
  private readonly cache = new Map<string, Voice>();
  private readonly passes: number[];
  private timer: unknown = null;
  private live: PlaybackHandle[] = [];
  private steps: Step[] = [];
  private section = 0;
  private bar = 0;
  private beat = 0;
  private nextTime = 0;
  private pending: { section: number; quantise: Quantise } | null = null;
  hold = false;

  /**
   * `ctx` is the audio context whose clock the player follows and `output` the node its voices go to. `book` sounds one note at a
   * time on the clock; it is Undertone's `playParams` unless a test passes its own.
   */
  constructor(
    private readonly ctx: Pick<AudioContextLike, 'currentTime'>,
    private readonly output: unknown,
    readonly built: BuiltTune,
    private options: VoiceOptions,
    private readonly clock: TimerLike = { setInterval: (cb, ms) => setInterval(cb, ms), clearInterval: (h) => clearInterval(h as number) },
    private readonly book: (patch: ControlPatch, when: number) => PlaybackHandle = (patch, when) =>
      playParams(this.ctx as AudioContextLike, [patch], { when, output: this.output as never }),
  ) {
    this.passes = built.sections.map(() => -1);
  }

  get playing(): boolean {
    return this.timer !== null;
  }

  /** The section a tap has called and not yet reached, if any. */
  get queued(): number | null {
    return this.pending?.section ?? null;
  }

  get barSeconds(): number {
    return 240 / this.built.tune.track.bpm;
  }

  start(section = 0): void {
    this.stop();
    this.passes.fill(-1);
    this.enter(section);
    this.beat = 0;
    this.pending = null;
    this.nextTime = this.ctx.currentTime + 0.08;
    this.timer = this.clock.setInterval(() => this.tick(), TICK_MS);
    this.tick();
  }

  /** Stops, fading whatever is sounding. */
  stop(): void {
    if (this.timer !== null) this.clock.clearInterval(this.timer);
    this.timer = null;
    for (const handle of this.live) handle.stop(undefined, { fade: 0.03 });
    this.live = [];
    this.steps = [];
    this.pending = null;
  }

  /** Calls a section. Tapping the section that is already called cancels the call. */
  call(section: number, quantise: Quantise): void {
    if (!this.playing) return;
    this.pending = this.pending?.section === section && this.pending.quantise === quantise ? null : { section, quantise };
  }

  /** Changes the mix or the humanising from the next beat. */
  setOptions(options: VoiceOptions): void {
    this.options = options;
  }

  /** The beat sounding at audio-clock time `now`, or null before the first. */
  stepAt(now: number): Step | null {
    let found: Step | null = null;
    for (const step of this.steps) if (step.time <= now) found = step;
    return found;
  }

  private pattern(section: number, kind: 'all' | 'keep' | 'turn'): Voice {
    const key = `${section}|${kind}|${this.options.mix}|${this.options.human}`;
    let pattern = this.cache.get(key);
    if (!pattern) {
      const s = this.built.sections[section]!;
      pattern =
        kind === 'turn'
          ? turnPattern(s, this.options.mix)
          : sectionPattern(this.built, s, this.options, kind === 'keep' ? (part) => !isRhythm(part) : undefined);
      this.cache.set(key, pattern);
    }
    return pattern;
  }

  /** Moves to the first bar of a section. Each visit is a new pass, so its humanising differs from the last time round. */
  private enter(section: number): void {
    this.passes[section]!++;
    this.section = section;
    this.bar = 0;
  }

  private tick(): void {
    const now = this.ctx.currentTime;
    // After a stall (a throttled background tab) pick up from now rather than rushing through the missed beats.
    if (this.nextTime < now - 0.05) this.nextTime = now + 0.05;
    while (this.nextTime < now + LOOKAHEAD) this.scheduleBeat();
    this.live = this.live.filter((handle) => handle.endTime > now);
    this.steps = this.steps.filter((step) => step.time > now - 4);
  }

  private scheduleBeat(): void {
    const beatSeconds = this.barSeconds / BEATS;
    const call = this.pending;
    if (call && (call.quantise === 'beat' || (call.quantise === 'bar' && this.beat === 0))) {
      // A hop lands on this beat, keeping its place in the bar; a bar call made just before a barline needs no turn.
      const beat = this.beat;
      this.enter(call.section);
      this.beat = call.quantise === 'beat' ? beat : 0;
      this.pending = null;
    }
    const section = this.built.sections[this.section]!;
    const lastBar = this.bar === section.cycles - 1;
    const turning =
      this.pending !== null && ((this.pending.quantise === 'bar' && this.beat > 0) || (this.pending.quantise === 'section' && lastBar && this.beat >= 2));
    const barStart = this.nextTime - this.beat * beatSeconds;
    const cycle = this.bar + this.passes[this.section]! * section.cycles;
    const book = (pattern: Voice, bar: number) => {
      for (const { at, patch } of beatOnsets(pattern, bar, this.beat, this.barSeconds)) this.live.push(this.book(patch, barStart + at));
    };
    if (turning) {
      book(this.pattern(this.section, 'keep'), cycle);
      book(this.pattern(this.pending!.section, 'turn'), 0);
    } else {
      book(this.pattern(this.section, 'all'), cycle);
    }
    this.steps.push({ time: this.nextTime, section: this.section, bar: this.bar, beat: this.beat, turning });

    this.nextTime += beatSeconds;
    this.beat++;
    if (this.beat < BEATS) return;
    this.beat = 0;
    this.bar++;
    if (this.pending?.quantise === 'bar') {
      this.enter(this.pending.section);
      this.pending = null;
    } else if (this.bar >= section.cycles) {
      if (this.pending?.quantise === 'section') {
        this.enter(this.pending.section);
        this.pending = null;
      } else {
        this.enter(this.hold ? this.section : (this.section + 1) % this.built.sections.length);
      }
    }
  }
}
