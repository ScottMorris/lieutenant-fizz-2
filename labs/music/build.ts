// Builds a lab tune into a song with the game's song builder, and holds the humanising settings each role plays with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The form (intro, tune, contrast, breakdown, build, lifted tune, fill) and the depth (filter envelopes, quiet partner voices) are
// assets/music/song.ts, unchanged: a lab tune becomes a song exactly as the game's level track does. What the lab adds sits on top:
// per-note pans, crossing echoes, a 7.1 placement (voices.ts) and the small seeded variations below.

import { layoutFor, type SongLayout } from '../../assets/music/song.ts';
import type { PartRole } from '../../assets/music/types.ts';
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

export function buildTune(tune: LabTune): BuiltTune {
  const layout = layoutFor(tune.track, tune.def);
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
