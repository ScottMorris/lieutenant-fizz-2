// The lab's eight tunes: six grown from one concept pattern each, and two that combine three patterns apiece.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A tune is a short loop of Undertone parts, written in scale degrees (scale.ts) around a riff read from its pattern (patterns.ts).
// The song builder in assets/music/song.ts lays each loop out as a song of about a minute, the same way it lays out the game's level
// track. These are lab sketches for listening and choosing from, not tracks the game plays; nobody has listened to them as Fizz 2's
// music yet.

import type { SongDef } from '../../assets/music/song.ts';
import type { MusicPart, MusicTrack } from '../../assets/music/types.ts';
import { patternById, riffLine, riffOf, riffSlice } from './patterns.ts';
import { degrees, keyOf, type Key } from './scale.ts';

/** A music part with the lab's extra voice settings. The builder copies them through untouched. */
export interface LabPart extends MusicPart {
  /** A pan for the part: a number, or a mini-notation string of pans sampled at each note. It replaces the role's pan. */
  pan?: number | string;
  /** A mini-notation string of velocities (accents), sampled at each note. */
  accent?: string;
  /** FM depth and ratio, with the seconds the depth takes to fall away, for bell and metal tones. */
  fm?: number;
  fmh?: number;
  fmdecay?: number;
  /** Phaser rate in Hz. */
  phaser?: number;
  /** A crossing echo: a copy this much quieter, one echo time late, on the opposite side. */
  cross?: number;
}

/** How a riff is placed in a tune, so a check can confirm the tune really starts from its pattern. */
export interface Motif {
  pattern: string;
  /** The role of the part that plays it. */
  role: NonNullable<MusicPart['role']>;
  /** The bar of the loop it starts in (from 0), and the octave degree 0 sits in. */
  bar: number;
  octave: number;
  /** The scale degree of the pattern's bottom row. */
  base: number;
  /** Whether a run of equal cells is one held note. */
  tie: boolean;
  /** Which line of the riff: the highest cells or the lowest. */
  line: 'top' | 'low';
}

export interface LabTune {
  id: string;
  title: string;
  /** The level type the tune is written for. */
  level: string;
  /** What kind of tune it is, in a few words. */
  kind: string;
  /** One or two sentences on how the pattern became the tune. */
  story: string;
  /** What the stereo field is doing. */
  stereo: string;
  keyName: string;
  key: Key;
  track: MusicTrack & { parts: LabPart[] };
  def: SongDef;
  motifs: Motif[];
  /** Reverb size of the tune's room, and its echo time in beats. */
  roomSize: number;
  echoBeats: number;
  /** Bars the lead takes to circle the speaker ring once in the 7.1 mix; only the 7.1 tune has it. */
  spinBars?: number;
}

// Fizz 1's part helpers (assets/music/crater.ts), with the lab's extra settings allowed.
const P = (notes: string, o: Partial<LabPart> = {}): LabPart => ({ notes, w: 'triangle', a: 0.005, d: 0.12, s: 0.3, r: 0.08, g: 0.2, ...o });
const kick = (pat: string, g = 0.7): LabPart => P(pat, { role: 'kick', w: 'sine', d: 0.12, s: 0, r: 0.06, g, slide: 0.09 });
const hats = (pat: string, g = 0.1, o: Partial<LabPart> = {}): LabPart =>
  P(pat, { role: 'hats', noise: true, d: 0.03, s: 0, r: 0.01, g, hpf: 6000, ...o });
const snare = (pat: string, g = 0.28): LabPart => P(pat, { role: 'snare', noise: true, d: 0.09, s: 0, r: 0.05, g, hpf: 1600 });
const perc = (pat: string, g: number, o: Partial<LabPart> = {}): LabPart =>
  P(pat, { role: 'perc', noise: true, d: 0.05, s: 0, r: 0.03, g, hpf: 2800, ...o });

/** The strong and weak eighths of a bar, for hats. */
const EIGHTHS = '[1 .6 .82 .6]*2';
const SIXTEENTHS = '[1 .5 .72 .5]*4';

const riff = (id: string, base = 0) => riffOf(patternById(id), base);

// ---------- 1. Beam Over the Point ----------

function beam(): LabTune {
  const key = keyOf('e dorian');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('beam');
  return {
    id: 'beam',
    title: 'Beam Over the Point',
    level: 'The vertical tower (Lighthouse Point)',
    kind: 'A climbing nocturne',
    story:
      'The beam is a staircase: sixteen columns read as eighth notes over two bars, each step sounded twice, climbing one octave of E dorian. The answer walks back down, and the climb starts again a step lower.',
    stereo:
      'The lead sweeps from the left speaker to the right as it climbs and back as it falls, like the lamp. In the 7.1 mix it circles the whole room once every two bars.',
    keyName: 'E dorian',
    key,
    roomSize: 7,
    echoBeats: 0.75,
    spinBars: 2,
    motifs: [{ pattern: 'beam', role: 'lead', bar: 0, octave: 4, base: 0, tie: false, line: 'top' }],
    def: { trim: 0.75, contrast: 5, lift: 2, arp: true },
    track: {
      bpm: 108,
      parts: [
        P(
          d(
            4,
            `<[${riffSlice(r.top, 0, 8)}] [${riffSlice(r.top, 8, 16)}] [5 ~ 7 ~ 7 5 3 ~] [5 ~ 4 3 5 ~ ~ ~] [-1 0 0 1 1 2 2 3] [3 4 4 5 5 6 6 ~] [6 ~ 4 ~ 6 4 2 ~] [6 ~ 4 2 1 ~ ~ ~]>`,
          ),
          {
            role: 'lead',
            w: 'square',
            g: 0.07,
            lpf: 2200,
            d: 0.1,
            delay: 0.22,
            pan: '<[-.7 -.5 -.3 -.1 .1 .3 .5 .7] [.7 .5 .3 .1 -.1 -.3 -.5 -.7]>',
            accent: '[1 .78 .9 .78]*2',
          },
        ),
        P(d(2, '<[0 ~ 0 4] [0 ~ 4 2] [3 ~ 3 7] [3 ~ 3 5] [-1 ~ -1 3] [-1 ~ 3 1] [4 ~ 4 8] [4 ~ 4 6]>'), {
          role: 'bass',
          g: 0.3,
          s: 0.4,
          d: 0.1,
        }),
        P(d(3, '<[0,2,4] [0,2,4] [3,5,7] [3,5,7] [-1,1,3] [-1,1,3] [4,6,8] [4,6,8]>'), {
          role: 'pad',
          w: 'square',
          a: 0.06,
          s: 0.6,
          g: 0.024,
          lpf: 900,
        }),
        P(d(6, '<[0 ~ ~ ~] ~ [~ ~ 4 ~] ~>'), { role: 'bell', w: 'sine', g: 0.05, d: 0.5, s: 0, r: 0.4, fm: 2, fmh: 3.01, fmdecay: 0.3, room: 0.5, delay: 0.3 }),
        kick('c1 ~ ~ ~ c1 ~ c1 ~', 0.55),
        snare('~ white ~ white', 0.16),
        hats('white*8', 0.07, { accent: EIGHTHS }),
      ],
    },
  };
}

// ---------- 2. Press Line ----------

function press(): LabTune {
  const key = keyOf('d phrygian');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('press', -1);
  return {
    id: 'press',
    title: 'Press Line',
    level: 'The conveyor and press gauntlet (Siphon Works)',
    kind: 'A machine march',
    story:
      'The picture is four presses: two low cells, a high pair, a gap. Sat one step under the tonic it reads stamp, drop, clang, wait, four times a bar, and the tune only ever moves the clang.',
    stereo:
      'The metal hits ping from hard left to hard right, press by press, and the clang’s lower note answers from the opposite side.',
    keyName: 'D phrygian',
    key,
    roomSize: 4,
    echoBeats: 0.5,
    motifs: [
      { pattern: 'press', role: 'lead', bar: 0, octave: 4, base: -1, tie: false, line: 'top' },
      { pattern: 'press', role: 'counter', bar: 0, octave: 4, base: -1, tie: false, line: 'low' },
    ],
    def: { trim: 0.72, contrast: 3, lift: 1, arp: false },
    track: {
      bpm: 120,
      parts: [
        P(
          d(
            4,
            `<[${riffLine(r.top)}] [0 -1 6 ~ 0 -1 6 ~ 0 -1 5 ~ 0 -1 4 ~] [3 1 5 ~ 3 1 5 ~ 3 1 7 ~ 3 1 8 ~] [1 -1 6 ~ 1 -1 6 ~ 3 1 6 ~ 4 3 1 0]>`,
          ),
          { role: 'lead', w: 'sawtooth', g: 0.055, lpf: 1700, d: 0.07, s: 0.2, accent: '[1 .8 .95 1]*4' },
        ),
        // The shadow of the clang: the lower cell of each high pair, on the far side.
        P(d(4, `[${riffLine(r.low)}]`), { role: 'counter', w: 'square', g: 0.03, lpf: 1500, d: 0.06, s: 0.1, pan: '[.7 -.7]*2' }),
        P(d(2, '<[0 0 ~ 0 0 0 ~ 0] [0 0 ~ 0 0 0 ~ -1] [1 1 ~ 1 1 1 ~ 1] [-1 -1 ~ -1 -1 -1 ~ 1]>'), {
          role: 'bass',
          w: 'sawtooth',
          g: 0.2,
          lpf: 500,
          s: 0.4,
          d: 0.07,
        }),
        P(d(3, '<[0,2,4] [0,2,4] [1,3,5] [-1,1,3]>'), { role: 'pad', w: 'square', a: 0.04, s: 0.6, g: 0.02, lpf: 800 }),
        kick('c1*4', 0.65),
        snare('~ white ~ white', 0.24),
        hats('white*16', 0.06, { accent: SIXTEENTHS }),
        perc('[~ ~ white ~]*4', 0.16, { pan: '[-.8 .8]*2', room: 0.35, hpf: 3600, d: 0.07 }),
      ],
    },
  };
}

// ---------- 3. Three Keys ----------

function keys(): LabTune {
  const key = keyOf('a minor');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('keys');
  return {
    id: 'keys',
    title: 'Three Keys',
    level: 'The key-door chain',
    kind: 'A sneaking puzzle tune',
    story:
      'Three stepped bars, each longer than the last: read with held notes they are three falling notes, an eighth, an eighth and a dotted eighth, starting off the beat. A two-note pickup follows on beat three, and the last beat is left for the door to answer.',
    stereo: 'One note per key: the first sounds on the left, the second in the middle, the third on the right. The door answers low and to one side.',
    keyName: 'A minor',
    key,
    roomSize: 5,
    echoBeats: 0.75,
    motifs: [{ pattern: 'keys', role: 'lead', bar: 0, octave: 4, base: 0, tie: true, line: 'top' }],
    def: { trim: 0.78, contrast: 5, lift: 2, arp: true },
    track: {
      bpm: 100,
      parts: [
        P(
          d(
            4,
            `<[${riffSlice(r.top, 0, 8, true)} 0@2 1@2 ~@4] [~ 7@2 5@2 2@3 2@2 1@2 ~@4] [~ 8@2 6@2 4@3 4@2 3@2 ~@4] [~ 6@2 4@2 1@3 -1@2 -1@2 ~@4]>`,
          ),
          {
            role: 'lead',
            w: 'triangle',
            g: 0.16,
            lpf: 2600,
            d: 0.14,
            s: 0.35,
            delay: 0.25,
            pan: '0 -.6 -.6 0 0 .6 .6 .6 -.25 -.25 .25 .25 0 0 0 0',
          },
        ),
        P(d(3, '<[~@12 4 2 ~@2] [~@12 2 0 ~@2] [~@12 6 4 ~@2] [~@12 4 1 ~@2]>'), {
          role: 'counter',
          w: 'square',
          g: 0.035,
          lpf: 1200,
          d: 0.08,
          s: 0.1,
          room: 0.3,
        }),
        P(d(2, '<[0 ~ ~ 0 ~ ~ 0 ~] [-2 ~ ~ -2 ~ ~ -2 ~] [2 ~ ~ 2 ~ ~ 2 ~] [-3 ~ ~ -3 ~ ~ -3 -1]>'), {
          role: 'bass',
          g: 0.3,
          s: 0.35,
          d: 0.1,
        }),
        P(d(3, '<[0,2,4] [-2,0,2] [2,4,6] [-3,-1,1]>'), { role: 'pad', w: 'square', a: 0.05, s: 0.6, g: 0.022, lpf: 850 }),
        kick('c1 ~ ~ ~ ~ ~ c1 ~', 0.55),
        snare('~ white ~ white', 0.14),
        hats('white*8', 0.06, { accent: EIGHTHS }),
        perc('~ ~ ~ [~ white]', 0.07, { hpf: 7000, d: 0.12, room: 0.3 }),
      ],
    },
  };
}

// ---------- 4. Crystal Bloom ----------

function crystal(): LabTune {
  const key = keyOf('b minor');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('crystal');
  return {
    id: 'crystal',
    title: 'Crystal Bloom',
    level: 'Zarg overgrowth (the deep rooms of Siphon Works)',
    kind: 'A slow, cold bell piece',
    story:
      'The ground line is a drone on the tonic, and the spikes are what the bell plays: three neighbouring notes creeping up from the ground and settling back, twice, the second spike a little smaller.',
    stereo: 'Each bell note lands somewhere different across the field, and its echo crosses to the other side, so the growth seems to spread around the listener.',
    keyName: 'B minor',
    key,
    roomSize: 9,
    echoBeats: 1.5,
    motifs: [{ pattern: 'crystal', role: 'lead', bar: 0, octave: 4, base: 0, tie: true, line: 'top' }],
    def: { trim: 0.8, contrast: 3, lift: 0, arp: true, form: "A B A breakdown A'" },
    track: {
      bpm: 80,
      parts: [
        P(
          d(
            4,
            `<[${riffLine(r.top, true)}] [2@2 3 4@2 3 2 ~@2 2 3 4 3 2@2 ~] [5@2 6 7@2 6 5 ~@2 3 4 5 4 3@2 ~] [4@2 5 6@2 5 4 ~@2 1 2 4 2 1@2 ~]>`,
          ),
          {
            role: 'lead',
            w: 'sine',
            g: 0.11,
            d: 0.35,
            s: 0.15,
            r: 0.3,
            lpf: 3600,
            fm: 1.8,
            fmh: 3.01,
            fmdecay: 0.25,
            room: 0.5,
            cross: 0.4,
            pan: '-.6 .1 .5 -.3 .7 -.1 -.7 .3 .6 -.5 .2 .7 -.2 -.6 .4 0',
          },
        ),
        P(d(1, '0'), { role: 'drone', w: 'sine', a: 0.4, s: 0.9, r: 0.5, g: 0.12 }),
        P(d(2, '<[0 ~ ~ 0] [-2 ~ ~ -2] [3 ~ ~ 3] [-3 ~ ~ -1]>'), { role: 'bass', g: 0.24, a: 0.02, s: 0.6, d: 0.3 }),
        P(d(3, '<[0,2,4] [-2,0,2] [3,5,7] [-3,-1,1]>'), { role: 'pad', w: 'sawtooth', a: 0.5, s: 0.7, r: 0.6, g: 0.02, lpf: 650 }),
        P(d(6, '<[~ ~ ~ 4] [~ 2 ~ ~] [~ ~ 0 ~] [~ ~ ~ [1 4]]>'), {
          role: 'bell',
          w: 'sine',
          g: 0.035,
          d: 0.6,
          s: 0,
          r: 0.5,
          fm: 2.4,
          fmh: 5.02,
          fmdecay: 0.2,
          room: 0.6,
          cross: 0.5,
          pan: '<.7 -.7 .5 -.5>',
        }),
        kick('c1 ~ ~ ~', 0.4),
        hats('~ white ~ white ~ white ~ [white white]', 0.035, { hpf: 8000 }),
      ],
    },
  };
}

// ---------- 5. Tanker on Treads ----------

function tanker(): LabTune {
  const key = keyOf('c minor');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('tanker');
  // The wheels: the five cells of the row under the body, three columns apart and then four. The low drum plays exactly these.
  const wheels = [...patternById('tanker').grid[5]!].map((cell) => (cell === '#' ? 'brown' : '~'));
  return {
    id: 'tanker',
    title: 'Tanker on Treads',
    level: 'The boss room (the Cocoa Siphon)',
    kind: 'A heavy boss stomp',
    story:
      'The tank body is one long flat line, so the lead climbs two steps and then hammers the fifth for ten sixteenths before stepping off. The five wheels underneath, three columns apart, are the lurching drum the bass rides.',
    stereo: 'The tread drum rolls across the field from left to right each bar, as if the tanker were driving past, while the lead is doubled wide on both sides.',
    keyName: 'C minor',
    key,
    roomSize: 5,
    echoBeats: 0.5,
    motifs: [{ pattern: 'tanker', role: 'lead', bar: 0, octave: 4, base: 0, tie: false, line: 'top' }],
    def: { trim: 0.68, contrast: 5, lift: 1, arp: false },
    track: {
      bpm: 138,
      parts: [
        P(
          d(
            4,
            `<[${riffLine(r.top)}] [~ 2 3 4 4 4 4 4 7 7 4 4 7 4 3 2] [~ 4 5 7 7 7 7 7 7 7 7 7 7 5 4 ~] [~ 3 4 6 6 6 6 6 6 6 8 8 6 6 4 3]>`,
          ),
          { role: 'lead', w: 'sawtooth', g: 0.055, lpf: 1500, d: 0.06, s: 0.25, phaser: 0.4, accent: '[1 .7 .85 .7]*4' },
        ),
        P(
          d(
            2,
            '<[0 0 ~ ~ 0 ~ ~ 0 ~ ~ 0 ~ ~ ~ 0 ~] [0 0 ~ ~ 0 ~ ~ 0 ~ ~ 0 ~ ~ ~ 0 -1] [5 5 ~ ~ 5 ~ ~ 5 ~ ~ 5 ~ ~ ~ 5 ~] [4 4 ~ ~ 4 ~ ~ 4 ~ ~ 4 ~ ~ ~ 4 6]>',
          ),
          { role: 'bass', w: 'sawtooth', g: 0.22, lpf: 420, s: 0.5, d: 0.08 },
        ),
        P(d(3, '<[0,4] [0,4] [5,9] [4,8]>'), { role: 'pad', w: 'sawtooth', a: 0.03, s: 0.7, g: 0.022, lpf: 700 }),
        kick('c1*4', 0.7),
        snare('~ white ~ white', 0.28),
        hats('white*8', 0.08, { accent: EIGHTHS }),
        perc(wheels.join(' '), 0.3, { hpf: 90, lpf: 380, d: 0.11, pan: '-.8 -.7 -.6 -.5 -.4 -.3 -.2 -.1 0 .1 .2 .3 .4 .5 .6 .7' }),
      ],
    },
  };
}

// ---------- 6. Same Room Twice ----------

function rooms(): LabTune {
  const key = keyOf('f lydian');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const r = riff('rooms');
  const room = (landmark: number): string => riffLine(r.top.map((cell) => (cell === 5 ? landmark : cell)));
  return {
    id: 'rooms',
    title: 'Same Room Twice',
    level: 'Identical rooms with a landmark',
    kind: 'An uncanny, quiet loop',
    story:
      'Four rooms, one cell each, all the same note; the antenna is the one note that differs. Every bar is the same room again with the landmark at a different height, and the lower cell shadows the tune a semitone below.',
    stereo: 'The rooms alternate left and right, identical on both sides; only the landmark note sounds in the middle, with a longer tail.',
    keyName: 'F lydian',
    key,
    roomSize: 8,
    echoBeats: 1,
    motifs: [
      { pattern: 'rooms', role: 'lead', bar: 0, octave: 4, base: 0, tie: false, line: 'top' },
      { pattern: 'rooms', role: 'counter', bar: 0, octave: 4, base: 0, tie: false, line: 'low' },
    ],
    def: { trim: 0.8, contrast: 2, lift: 0, arp: true, form: "A B A breakdown A'" },
    track: {
      bpm: 88,
      parts: [
        P(d(4, `<[${room(5)}] [${room(6)}] [${room(3)}] [${room(8)}]>`), {
          role: 'lead',
          w: 'triangle',
          g: 0.15,
          d: 0.16,
          s: 0.25,
          lpf: 2400,
          room: 0.45,
          pan: '-.6 0 0 .6 .6 0 0 -.6 -.6 0 0 .6 .6 0 0 -.6',
        }),
        P(d(4, `[${riffLine(r.low)}]`), {
          role: 'counter',
          w: 'sine',
          g: 0.03,
          d: 0.2,
          s: 0.2,
          room: 0.5,
          pan: '.6 0 0 -.6 -.6 0 0 .6 .6 0 0 -.6 -.6 0 0 .6',
        }),
        P(d(2, '<[0 ~ ~ ~ 0 ~ ~ ~] [2 ~ ~ ~ 2 ~ ~ ~] [-1 ~ ~ ~ -1 ~ ~ ~] [4 ~ ~ ~ 4 ~ 1 ~]>'), { role: 'bass', g: 0.28, s: 0.5, d: 0.15 }),
        P(d(3, '<[0,2,4] [2,4,6] [-1,1,4] [4,6,8]>'), { role: 'pad', w: 'triangle', a: 0.2, s: 0.7, r: 0.3, g: 0.05, lpf: 1200 }),
        kick('c1 ~ ~ ~ ~ ~ ~ ~', 0.45),
        hats('~ white ~ white ~ white ~ white', 0.04, { hpf: 7500 }),
      ],
    },
  };
}

// ---------- 7. Night Map ----------

function nightMap(): LabTune {
  const key = keyOf('g major');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const beamRiff = riff('beam');
  const keysRiff = riff('keys', 1);
  const roomsRiff = riff('rooms');
  return {
    id: 'night-map',
    title: 'Night Map',
    level: 'The overworld, walked between levels',
    kind: 'A bright walking theme',
    story:
      'Three patterns in one tune. The lead climbs the beam for two bars and comes down the three keys for two; underneath, the identical rooms tick away as a pedal on the fifth, with the landmark as its one bright note.',
    stereo: 'The climb sweeps left to right, the keys step left, middle, right, and the rooms pedal alternates sides behind them.',
    keyName: 'G major',
    key,
    roomSize: 6,
    echoBeats: 0.75,
    motifs: [
      { pattern: 'beam', role: 'lead', bar: 0, octave: 4, base: 0, tie: false, line: 'top' },
      { pattern: 'keys', role: 'lead', bar: 2, octave: 4, base: 1, tie: true, line: 'top' },
      { pattern: 'rooms', role: 'counter', bar: 0, octave: 4, base: 0, tie: false, line: 'top' },
    ],
    def: { trim: 0.72, contrast: 5, lift: 2, arp: true },
    track: {
      bpm: 116,
      parts: [
        P(
          d(
            4,
            `<[${riffSlice(beamRiff.top, 0, 8)}] [${riffSlice(beamRiff.top, 8, 16)}] [${riffSlice(keysRiff.top, 0, 8, true)} ~@4 5@2 4@2] [~ 8@2 6@2 4@3 ~@4 4@2 5@2] [5 6 6 7 7 8 8 9] [10 9 9 7 7 5 5 ~] [~ 7@2 5@2 3@3 ~@4 3@2 2@2] [~ 6@2 4@2 1@3 ~@4 1@2 -1@2]>`,
          ),
          {
            role: 'lead',
            w: 'square',
            g: 0.07,
            lpf: 2400,
            d: 0.1,
            delay: 0.18,
            pan: '<[-.6 -.4 -.2 -.1 .1 .2 .4 .6] [.6 .4 .2 .1 -.1 -.2 -.4 -.6] [0 -.6 -.6 0 0 .6 .6 .6 0 0 0 0 0 0 0 0] [0 -.6 -.6 0 0 .6 .6 .6 0 0 0 0 0 0 0 0]>',
          },
        ),
        P(d(4, `[${riffLine(roomsRiff.top)}]`), {
          role: 'counter',
          w: 'triangle',
          g: 0.05,
          d: 0.08,
          s: 0.1,
          lpf: 2000,
          room: 0.3,
          pan: '.5 0 0 -.5 -.5 0 0 .5 .5 0 0 -.5 -.5 0 0 .5',
        }),
        P(d(2, '<[0 ~ 0 4 0 ~ 2 4] [0 ~ 0 2 4 ~ 4 2] [3 ~ 3 7 3 ~ 3 5] [4 ~ 4 8 4 ~ 4 6] [5 ~ 5 9 5 ~ 7 5] [3 ~ 3 5 3 ~ 3 1] [1 ~ 1 5 1 ~ 1 3] [4 ~ 4 6 4 ~ 4 8]>'), {
          role: 'bass',
          g: 0.28,
          s: 0.35,
          d: 0.08,
        }),
        P(d(3, '<[0,2,4] [0,2,4] [3,5,7] [4,6,8] [5,7,9] [3,5,7] [1,3,5] [4,6,8]>'), {
          role: 'pad',
          w: 'square',
          a: 0.05,
          s: 0.6,
          g: 0.022,
          lpf: 900,
        }),
        kick('c1 ~ c1 ~ c1 ~ c1 c1', 0.6),
        snare('~ white ~ white', 0.22),
        hats('white*8', 0.08, { accent: EIGHTHS }),
      ],
    },
  };
}

// ---------- 8. Mildred's Plan ----------

function plan(): LabTune {
  const key = keyOf('d minor');
  const d = (octave: number, s: string) => degrees(key, octave, s);
  const tankerRiff = riff('tanker');
  const crystalRiff = riff('crystal');
  const pressRiff = riff('press', -1);
  return {
    id: 'plan',
    title: 'Mildred’s Plan',
    level: 'The last level: the factory, overgrown, with the tanker inside',
    kind: 'A driving finale',
    story:
      'Three patterns in one tune. The presses are the bass, stamp, drop and clang on every beat; the tanker hammers its fifth over the top; and every other bar a crystal spike grows out of the high bell.',
    stereo: 'The clang of each press jumps from side to side, the tanker lead is doubled wide, and the crystal bell scatters across the field with a crossing echo.',
    keyName: 'D minor',
    key,
    roomSize: 6,
    echoBeats: 0.75,
    motifs: [
      { pattern: 'tanker', role: 'lead', bar: 0, octave: 4, base: 0, tie: false, line: 'top' },
      { pattern: 'press', role: 'bass', bar: 0, octave: 2, base: -1, tie: false, line: 'top' },
      { pattern: 'crystal', role: 'bell', bar: 0, octave: 5, base: 0, tie: true, line: 'top' },
    ],
    def: { trim: 0.66, contrast: 3, lift: 2, arp: false },
    track: {
      bpm: 132,
      parts: [
        P(
          d(
            4,
            `<[${riffLine(tankerRiff.top)}] [~ 2 3 4 4 4 4 4 7 7 4 4 7 4 3 2] [~ 4 5 7 7 7 7 7 7 7 7 7 7 5 4 ~] [~ 3 4 6 6 6 6 6 6 6 8 8 6 6 4 3]>`,
          ),
          { role: 'lead', w: 'sawtooth', g: 0.055, lpf: 1700, d: 0.06, s: 0.25, phaser: 0.3, accent: '[1 .7 .85 .7]*4' },
        ),
        P(d(2, `<[${riffLine(pressRiff.top)}] [${riffLine(pressRiff.top)}] [5 4 11 ~]*4 [4 3 10 ~]*4>`), {
          role: 'bass',
          w: 'sawtooth',
          g: 0.2,
          lpf: 520,
          s: 0.4,
          d: 0.07,
        }),
        P(d(5, `<[${riffLine(crystalRiff.top, true)}] ~ [7@2 8 9@2 8 7 ~@2 7 8 9 8 7@2 ~] ~>`), {
          role: 'bell',
          w: 'sine',
          g: 0.04,
          d: 0.3,
          s: 0.1,
          r: 0.3,
          fm: 2,
          fmh: 3.01,
          fmdecay: 0.2,
          room: 0.5,
          cross: 0.45,
          pan: '-.6 .1 .5 -.3 .7 -.1 -.7 .3 .6 -.5 .2 .7 -.2 -.6 .4 0',
        }),
        P(d(3, '<[0,4] [0,4] [5,9] [4,8]>'), { role: 'pad', w: 'sawtooth', a: 0.03, s: 0.7, g: 0.022, lpf: 750 }),
        kick('c1*4', 0.7),
        snare('~ white ~ white', 0.27),
        hats('white*16', 0.055, { accent: SIXTEENTHS }),
        perc('[~ ~ white ~]*4', 0.13, { pan: '[-.8 .8]*2', room: 0.3, hpf: 3600, d: 0.07 }),
      ],
    },
  };
}

/** The eight tunes, in the order the lab lists them: the six pattern tunes, then the two combinations. */
export const TUNES: readonly LabTune[] = [beam(), press(), keys(), crystal(), tanker(), rooms(), nightMap(), plan()];

export const tuneById = (id: string): LabTune => {
  const found = TUNES.find((tune) => tune.id === id);
  if (!found) throw new Error(`labs/music/tunes.ts has no tune "${id}"`);
  return found;
};

/** The tune that also has a 7.1 mix. */
export const SURROUND_TUNE = 'beam';
