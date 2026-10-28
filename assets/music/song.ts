// The song builder: a looping track written out as a full song of about a minute, with depth.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ported from `packages/engine/src/music-songs.ts` of ~/source/liminal-hq/lieutenant-fizz at commit 4bce036, the same
// author's code, Apache-2.0 OR MIT there and here (the DOS port's copy, `m4/sound/fizz-songs.ts` of ScottMorris/lieutenant-fizz-dos at
// commit 221dd6c, agrees with it). What changed: the sections carry names ('Intro', 'A', 'B', ...) so the layout can be reported;
// `loopCycles` reads the pattern through Undertone's parser (`./pattern.ts`) in place of Fizz 1's own mini-notation evaluator;
// `plan` is exported; the arithmetic, the forms and the output `MusicSong` are otherwise unchanged, and
// `song.test.ts` holds them to Fizz 1's output. Pure: it runs in Bun and in the browser bundle, with no Node-only API.
//
// A cue loops a bar or two to eight bars, which is right for a level crossed in seconds and thin for one you linger in. The builder
// plays a cue as a song of about a minute, written with ordinary song-writing devices:
//
// - sections: an intro that brings the band in, the tune, a contrast section in a related key, a breakdown and a build, then the
//   tune again doubled an octave up (some a key higher), and a one-bar fill that turns the end back into the start so the loop has
//   no seam;
// - depth: a filter that opens at each attack and closes as the note settles, and for the melodic parts a sine partner on the same
//   note a few milliseconds late, panned to the other side.
//
// The notes are the cue's own, moved or doubled, so the song is the tune the player already knows.

import { queryPart } from './pattern.ts';
import type { MusicPart, MusicSong, MusicTrack } from './types.ts';

// ---------- Rewriting notes ----------

const SHARP_NAMES = ['c', 'c#', 'd', 'd#', 'e', 'f', 'f#', 'g', 'g#', 'a', 'a#', 'b'];
const SEMITONES: Record<string, number> = { c: 0, d: 2, e: 4, f: 5, g: 7, a: 9, b: 11 };
const NOTE = /(?<![a-z#])([a-g])(#|b)?(-?\d)(?!\d)/g;

/** Shifts every note name in a mini-notation string by `semitones`. Noise words are untouched. */
export function transposeNotes(notes: string, semitones: number): string {
  if (!semitones) return notes;
  return notes.replace(NOTE, (_, letter: string, accidental: string | undefined, octave: string): string => {
    const midi =
      12 * (Number(octave) + 1) + SEMITONES[letter]! + (accidental === '#' ? 1 : accidental === 'b' ? -1 : 0) + semitones;
    return `${SHARP_NAMES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
  });
}

/** A bass line reduced to the first note of each bar: the root, held or repeated. */
export function rootsOnly(notes: string): string {
  return notes.replace(/\[([^[\],]*)\]/g, (_, group: string) => group.trim().split(/\s+/)[0] ?? group);
}

/** Block chords `[a,b,c]` spread into a rising and falling arpeggio of eight notes a bar. */
export function arpeggiate(notes: string): string {
  return notes.replace(/\[([^[\]]+,[^[\]]+)\]/g, (_, group: string) => {
    const tones = group.split(',').map((tone) => tone.trim());
    const up = [...tones, ...tones.slice(1, -1).reverse()];
    return `[${Array.from({ length: 8 }, (_, step) => up[step % up.length]).join(' ')}]`;
  });
}

/** The cycles after which every part of a track repeats exactly (1, 2, 3, 4, 6, 8, 12 or 16, or 32 when none of them does). */
export function loopCycles(track: MusicTrack): number {
  const horizon = 32;
  const all = track.parts.flatMap((part, index) => queryPart(part, 0, horizon).map((e) => ({ ...e, index })));
  const signatures = Array.from({ length: horizon }, (_, cycle) =>
    all
      .filter((e) => Math.floor(e.t0 + 1e-9) === cycle)
      .map((e) => `${e.index}:${(e.t0 - cycle).toFixed(4)}:${(e.t1 - e.t0).toFixed(4)}:${e.hz?.toFixed(1) ?? e.noise}`)
      .sort()
      .join('|'),
  );
  for (const candidate of [1, 2, 3, 4, 6, 8, 12, 16]) {
    let repeats = true;
    for (let cycle = 0; cycle + candidate < horizon && repeats; cycle++) {
      repeats = signatures[cycle] === signatures[cycle + candidate];
    }
    if (repeats) return candidate;
  }
  return horizon;
}

// ---------- Sections ----------

export type Slot = { key: string; part: MusicPart };

export interface Section {
  /** The section's name in the form: 'Intro', 'A', 'B', 'A (octave)', 'Breakdown', 'Build', "A'" or 'Fill'. */
  name: string;
  cycles: number;
  slots: Slot[];
}

const SOFT: ReadonlySet<string> = new Set(['pad', 'drone', 'bell']);
const PERCUSSIVE: ReadonlySet<string> = new Set(['kick', 'snare', 'hats', 'perc']);

const melodic = (part: MusicPart): boolean => !part.noise && part.role !== 'kick';
const moved = (part: MusicPart, shift: number): MusicPart =>
  melodic(part) ? { ...part, notes: transposeNotes(part.notes, shift) } : part;

/** The part that carries the tune: a lead if there is one, else an arpeggio, a bell or a counter. */
function primary(track: MusicTrack): number {
  for (const role of ['lead', 'arp', 'bell', 'counter']) {
    const index = track.parts.findIndex((part) => part.role === role);
    if (index >= 0) return index;
  }
  return 0;
}

const slots = (track: MusicTrack, pick: (part: MusicPart, index: number) => MusicPart | undefined): Slot[] =>
  track.parts.flatMap((part, index): Slot[] => {
    const chosen = pick(part, index);
    return chosen ? [{ key: `p${index}`, part: chosen }] : [];
  });

const arpOf = (track: MusicTrack, shift: number): Slot[] => {
  const pad = track.parts.find((part) => part.role === 'pad');
  if (!pad || !/\[[^[\]]+,[^[\]]+\]/.test(pad.notes)) return [];
  return [
    {
      key: 'arp',
      part: {
        ...pad,
        notes: transposeNotes(arpeggiate(pad.notes), shift),
        role: 'arp',
        w: 'triangle',
        a: 0.005,
        d: 0.1,
        s: 0.2,
        r: 0.1,
        g: 0.06,
        lpf: 2400,
        room: 0.3,
      },
    },
  ];
};

const roll = (g: number): Slot => ({
  key: 'roll',
  part: { notes: 'white*16', role: 'snare', noise: true, d: 0.05, s: 0, r: 0.03, g, hpf: 1500 },
});

const fourOnFloor: Slot = {
  key: 'floor',
  part: { notes: 'c1*4', role: 'kick', w: 'sine', d: 0.12, s: 0, r: 0.06, g: 0.6, slide: 0.09 },
};

/** The cast only: pad or bell, a bass reduced to roots, and the hats. */
function intro(track: MusicTrack, cycles: number): Section {
  return {
    name: 'Intro',
    cycles,
    slots: slots(track, (part) => {
      if (part.role && SOFT.has(part.role)) return part;
      if (part.role === 'bass') return { ...part, notes: rootsOnly(part.notes), s: 0.7 };
      if (part.role === 'hats') return part;
      return undefined;
    }),
  };
}

interface MainOptions {
  shift?: number;
  arp?: boolean;
  octave?: boolean;
}

/** The full band, optionally in another key, with the pad arpeggiated or the tune doubled an octave up. */
function main(track: MusicTrack, name: string, cycles: number, { shift = 0, arp = false, octave = false }: MainOptions): Section {
  const lead = primary(track);
  const list = slots(track, (part) => moved(part, shift));
  if (arp) list.push(...arpOf(track, shift));
  if (octave) {
    const part = track.parts[lead]!;
    list.push({ key: 'octave', part: { ...part, notes: transposeNotes(part.notes, shift + 12), g: part.g * 0.55 } });
  }
  return { name, cycles, slots: list };
}

/** Everything drops out but the pad, the roots and a kick; a build adds a snare roll. */
function breakdown(track: MusicTrack, cycles: number, build: boolean): Section {
  const base = slots(track, (part) => {
    if (part.role && SOFT.has(part.role)) return part;
    // Held roots carry a lot of low energy, so the bass and kick drop back in a breakdown.
    if (part.role === 'bass') return { ...part, notes: rootsOnly(part.notes), s: 0.5, g: part.g * 0.5 };
    if (part.role === 'kick') return { ...part, g: part.g * 0.6 };
    return undefined;
  });
  return { name: build ? 'Build' : 'Breakdown', cycles, slots: build ? [...base, roll(0.2)] : base };
}

/** One bar that turns the end of the song back into its beginning: a roll, four kicks, a held pad and root. */
function turnaround(track: MusicTrack): Section {
  const pad = track.parts.find((part) => part.role && SOFT.has(part.role));
  const bass = track.parts.find((part) => part.role === 'bass');
  return {
    name: 'Fill',
    cycles: 1,
    slots: [
      roll(0.22),
      fourOnFloor,
      ...(pad ? [{ key: 'pad', part: pad }] : []),
      ...(bass ? [{ key: 'bass', part: { ...bass, notes: rootsOnly(bass.notes), s: 0.7, g: bass.g * 0.8 } }] : []),
    ],
  };
}

// ---------- Songs ----------

export interface SongDef {
  /** A level trim that keeps the summed peak under about 0.9: Undertone writes straight to the output. */
  trim: number;
  /** Where the contrast section goes: semitones above the main key. */
  contrast: number;
  /** Where the last tune goes. */
  lift: number;
  /** Whether the pad is arpeggiated for the contrast and the last tune. */
  arp: boolean;
  /** Bars of intro before the first main section; 0 starts straight on the tune. Default 2. */
  intro?: number;
  /** Leave out the closing fill, so the last section runs straight into the first. */
  noFill?: boolean;
  /** Pin the form to one of the named forms instead of the one nearest a minute. */
  form?: string;
}

/** The length the form is chosen to land nearest, in seconds. */
export const TARGET_SECONDS = 60;

/** Chooses the form whose length lands nearest a minute, and lays out its sections. */
export function plan(track: MusicTrack, def: SongDef): Section[] {
  const loop = loopCycles(track);
  const cycleSeconds = 240 / track.bpm;
  const opening = def.intro === 0 ? [] : [intro(track, def.intro ?? 2)];
  const closing = def.noFill ? [] : [turnaround(track)];

  const formsFor = (unit: number): { name: string; sections: Section[] }[] => {
    const half = Math.max(2, unit / 2);
    const A = (options: MainOptions = {}, name = 'A'): Section => main(track, name, unit, options);
    const contrast = A({ shift: def.contrast, arp: def.arp }, 'B');
    const last = A({ shift: def.lift, arp: def.arp, octave: true }, "A'");
    const back = A({ octave: true }, 'A (octave)');
    const drop = [breakdown(track, half - 1, false), breakdown(track, 1, true)];
    return [
      {
        name: "A B'",
        sections: [...opening, A(), A({ shift: def.contrast, arp: def.arp, octave: true }, 'B'), ...closing],
      },
      { name: "A B A'", sections: [...opening, A(), contrast, last, ...closing] },
      { name: "A B breakdown A'", sections: [...opening, A(), contrast, ...drop, last, ...closing] },
      { name: "A B A breakdown A'", sections: [...opening, A(), contrast, back, ...drop, last, ...closing] },
      {
        name: "A A B A breakdown A'",
        sections: [...opening, A(), A({ arp: def.arp }), contrast, back, ...drop, last, ...closing],
      },
    ];
  };

  // A short loop can play each section twice through; the length decides which suits.
  const units = loop < 8 ? [loop, loop * 2] : [loop];
  const measured = units
    .flatMap((unit) => formsFor(unit))
    .map((form) => ({ form, cycles: form.sections.reduce((total, section) => total + section.cycles, 0) }));
  const pinned = def.form ? measured.filter((item) => item.form.name === def.form) : [];
  const pool = pinned.length ? pinned : measured;
  const best = pool.reduce((a, b) =>
    Math.abs(a.cycles * cycleSeconds - TARGET_SECONDS) <= Math.abs(b.cycles * cycleSeconds - TARGET_SECONDS) ? a : b,
  );
  return best.form.sections;
}

// ---------- Depth ----------

/**
 * A part with the shaping a sound chip's envelopes gave a note: a filter that opens at the attack and closes as the note settles
 * (the audible effect of a decaying modulator), and for a melodic part a partner on the same note a few milliseconds late, a sine
 * with more sustain, panned opposite its part. Drums and noise are left as they are.
 */
export function withDepth(part: MusicPart, index: number): MusicPart[] {
  if (part.noise || (part.role && PERCUSSIVE.has(part.role))) return [part];
  const slow = part.role === 'pad' || part.role === 'drone';
  const cutoff = part.lpf ?? 2600;
  const settle = slow ? 1 : Math.min(0.4, Math.max(0.08, (part.d ?? 0.12) * 1.5));
  const main: MusicPart = {
    ...part,
    g: part.g * (slow ? 1 : 0.85),
    lpf: cutoff,
    lpenv: cutoff * (slow ? 0.8 : 1.4),
    lpa: slow ? 0.3 : 0.001,
    lpd: settle,
    lps: 0.2,
    lpr: part.r ?? 0.08,
  };
  if (slow) return [main];
  const partner: MusicPart = {
    ...part,
    w: 'sine',
    g: part.g * 0.5,
    a: Math.max(part.a ?? 0.005, 0.01),
    s: Math.max(part.s ?? 0.3, 0.5),
    lpf: 3200,
    nudge: 0.002,
    mirror: index % 2 === 0 ? -0.22 : 0.22,
  };
  return [main, partner];
}

/** A song laid out as one stream per voice, with the sections that made it. */
export interface SongLayout {
  song: MusicSong;
  /** The sections in order, with the bar each begins at (from 0). */
  sections: { name: string; cycles: number; startBar: number }[];
}

/** The song for a track and its settings, with its section names and start bars. */
export function layoutFor(track: MusicTrack, def: SongDef): SongLayout {
  const sections = plan(track, def);
  const keys = [...new Set(sections.flatMap((section) => section.slots.map((slot) => slot.key)))];
  const streams: (MusicPart | null)[][] = [];
  keys.forEach((key, index) => {
    const voicesIn = (section: Section): MusicPart[] | undefined => {
      const slot = section.slots.find((candidate) => candidate.key === key);
      return slot ? withDepth({ ...slot.part, g: slot.part.g * def.trim }, index) : undefined;
    };
    const first = sections.map(voicesIn).find((voices) => voices !== undefined);
    if (!first) return;
    for (let layer = 0; layer < first.length; layer++) {
      streams.push(sections.map((section) => voicesIn(section)?.[layer] ?? null));
    }
  });
  let startBar = 0;
  return {
    song: { bpm: track.bpm, cycles: sections.map((section) => section.cycles), streams },
    sections: sections.map((section) => {
      const entry = { name: section.name, cycles: section.cycles, startBar };
      startBar += section.cycles;
      return entry;
    }),
  };
}

/** The song for one cue: its sections, laid out as one stream per voice. */
export function songFor(track: MusicTrack, def: SongDef): MusicSong {
  return layoutFor(track, def).song;
}

/** The length of a song in bars. */
export const songBars = (song: MusicSong): number => song.cycles.reduce((total, cycles) => total + cycles, 0);

/** The length of a song in seconds (a bar is four beats). */
export const songSeconds = (song: MusicSong): number => (songBars(song) * 240) / song.bpm;
