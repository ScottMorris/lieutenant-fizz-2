// Builds a lab tune into a song with the game's song builder, and holds the humanising settings each role plays with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The form (intro, tune, contrast, breakdown, build, the tune again) and the depth (filter envelopes, quiet partner voices) are
// assets/music/song.ts, unchanged: a lab tune becomes a song exactly as the game's level track does. What the lab adds sits on top:
// per-note pans, crossing echoes, a 7.1 placement (voices.ts) and the small seeded variations below.
//
// Two things differ from the game's form. There is no closing fill: the last section is the tune in its home key, whose last bar
// sits on the dominant, so the end of the song falls straight into the intro and the loop has no seam. And the build is not a snare
// roll: it is one orchestral bar on the dominant (`symphonicBuild`).

import { layoutFor, type SongLayout } from '../../assets/music/song.ts';
import type { MusicPart, PartRole } from '../../assets/music/types.ts';
import { degrees } from './scale.ts';
import type { LabPart, LabTune } from './tunes.ts';

export interface BuiltSection {
  name: string;
  cycles: number;
  startBar: number;
  /** Semitones this section sits above the tune's key. */
  shift: number;
  /** The voices that play in this section, with the index of the stream each belongs to. */
  parts: { stream: number; part: LabPart }[];
}

export interface BuiltTune {
  tune: LabTune;
  layout: SongLayout;
  sections: BuiltSection[];
  bars: number;
  seconds: number;
}

/**
 * The build, as an orchestra would play it: one bar on the dominant that swells into the tune's return. Strings hold the dominant
 * chord and open up through the bar, two string lines climb the scale an octave apart from opposite sides, the bass holds the
 * dominant, and a timpani on the same note rolls faster and louder. Every note is pitched and in the key; nothing ticks.
 */
export function symphonicBuild(tune: LabTune): LabPart[] {
  const bar = 240 / tune.track.bpm;
  const trim = tune.def.trim;
  const d = (octave: number, pattern: string): string => degrees(tune.key, octave, pattern);
  const climb = '-3 -2 -1 0 1 2 3 4';
  const swell = '.5 .57 .64 .71 .78 .85 .92 1';
  const string = (notes: string, o: Partial<LabPart>): LabPart => ({ notes, w: 'sawtooth', role: 'counter', a: 0.02, d: 0.1, s: 0.6, r: 0.08, g: 0.04 * trim, lpf: 2200, accent: swell, room: 0.3, ...o });
  return [
    { notes: d(3, '[4,6,8,11]'), role: 'pad', w: 'sawtooth', a: bar * 0.7, d: 0.1, s: 0.95, r: 0.12, g: 0.05 * trim, lpf: 700, lpenv: 2200, lpa: bar * 0.9, lpd: 0.2, lps: 1, lpr: 0.1, room: 0.4 },
    string(d(4, climb), { pan: '-.6 -.4 -.3 -.1 .1 .3 .4 .6' }),
    string(d(3, climb), { pan: '.6 .4 .3 .1 -.1 -.3 -.4 -.6', g: 0.05 * trim, lpf: 1500 }),
    { notes: d(2, '4'), role: 'bass', w: 'triangle', a: 0.05, d: 0.2, s: 0.85, r: 0.1, g: 0.22 * trim },
    { notes: d(1, '4*2 4*2 4*4 4*4'), role: 'perc', w: 'sine', a: 0.004, d: 0.2, s: 0, r: 0.12, g: 0.34 * trim, lpf: 400, pan: 0, accent: '.4 .55 .75 1' },
  ];
}

export function buildTune(tune: LabTune): BuiltTune {
  // No closing fill, whatever the tune's own settings say.
  const made = layoutFor(tune.track, { ...tune.def, noFill: true });
  // The build's own voices (the pad, the roots and the snare roll) make way for the orchestral bar, each line a stream of its own.
  const at = made.sections.findIndex((section) => section.name === 'Build');
  const kept = made.song.streams
    .map((stream) => stream.map((part, index) => (index === at ? null : part)))
    .filter((stream) => stream.some((part) => part !== null));
  const added = at < 0 ? [] : symphonicBuild(tune).map((part) => made.sections.map((_, index): MusicPart | null => (index === at ? part : null)));
  const layout: SongLayout = { song: { ...made.song, streams: [...kept, ...added] }, sections: made.sections };
  const sections = layout.sections.map((section, index): BuiltSection => ({
    ...section,
    shift: section.name === 'B' ? tune.def.contrast : section.name === "A'" ? tune.def.lift : 0,
    parts: layout.song.streams.flatMap((stream, streamIndex) => {
      const part = stream[index];
      return part ? [{ stream: streamIndex, part: part as LabPart }] : [];
    }),
  }));
  const bars = sections.reduce((total, section) => total + section.cycles, 0);
  return { tune, layout, sections, bars, seconds: (bars * 240) / tune.track.bpm };
}

/** How far one role's notes may wander. Times are seconds, `level` a fraction, `cents` hundredths of a semitone. */
export interface Human {
  /** Largest timing offset either way. */
  timing: number;
  /** Largest level change either way. */
  level: number;
  /** Largest detune either way. */
  cents: number;
  /** Largest pan offset either way. */
  pan: number;
  /** A constant lean: positive plays behind the beat, negative ahead of it. */
  push: number;
}

/**
 * A band that is steady but not a machine. The kick is the clock and barely moves; the snare sits a hair behind it and the hats a
 * hair ahead; the bass follows the kick; the tune and the pads are the loosest. Nothing moves more than nine milliseconds, a tenth
 * of its level or four cents, which is under what reads as a mistake and over what reads as a sequencer.
 */
export const HUMAN: Record<PartRole, Human> = {
  kick: { timing: 0.0015, level: 0.04, cents: 0, pan: 0, push: 0 },
  snare: { timing: 0.003, level: 0.08, cents: 0, pan: 0, push: 0.004 },
  hats: { timing: 0.004, level: 0.12, cents: 0, pan: 0.03, push: -0.002 },
  perc: { timing: 0.005, level: 0.1, cents: 0, pan: 0.03, push: 0 },
  bass: { timing: 0.004, level: 0.06, cents: 0, pan: 0, push: 0.002 },
  lead: { timing: 0.007, level: 0.08, cents: 4, pan: 0.03, push: 0 },
  counter: { timing: 0.008, level: 0.1, cents: 3, pan: 0.03, push: 0.001 },
  arp: { timing: 0.006, level: 0.1, cents: 3, pan: 0.02, push: 0 },
  bell: { timing: 0.009, level: 0.1, cents: 3, pan: 0.04, push: 0.002 },
  pad: { timing: 0.009, level: 0.05, cents: 3, pan: 0, push: 0 },
  drone: { timing: 0.009, level: 0.04, cents: 2, pan: 0, push: 0 },
};

export const humanOf = (part: LabPart): Human => HUMAN[part.role ?? 'lead'];

/** The seed of one voice's variations: the same tune, section and stream always wander the same way. */
export const humanSeed = (tune: LabTune, section: string, stream: number): string => `fizz2-lab|${tune.id}|${section}|${stream}`;
