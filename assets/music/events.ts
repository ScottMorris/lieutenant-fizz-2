// The song as a flat, deterministic list of note events: the form the DOS lowering generator and the Web player both read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the way of reading notes (each section's slots queried from the section's own first bar, laid end to end, a note cut at
// the end of its section) follows `m4/sound/fizz-dos/song.ts` of ScottMorris/lieutenant-fizz-dos at commit 221dd6c, the same author's
// code, MIT there and dual-licensed here; here it reads the built `MusicSong` streams, so the quiet partner voices are included.
// Pure: it runs in Bun and in the browser bundle.

import { queryPart } from './pattern.ts';
import type { SongLayout } from './song.ts';
import type { MusicSong, PartRole } from './types.ts';

/** Ticks in one bar (a cycle): 24 per beat, enough for eighths, sixteenths and triplets. Every onset must land on a whole tick. */
export const TICKS_PER_BAR = 96;

/** The gain that maps to the top velocity (127): the bass's gain, the loudest part before the song's trim. */
export const VELOCITY_FULL_GAIN = 0.3;

export interface NoteEvent {
  /** The start, in ticks from the start of the song (96 to a bar). */
  tick: number;
  /** The length in ticks, cut at the end of the section. */
  length: number;
  /** Which stream of the song, in the order the builder lists them. */
  stream: number;
  /** The stream's unique label: its role, `-partner` for a quiet side voice, `#2` and up for a second stream of the same role. */
  part: string;
  role: PartRole | undefined;
  /** The MIDI note number, or null for noise. */
  midi: number | null;
  /** 'white', 'pink' or 'brown' for noise, else null. */
  noise: 'white' | 'pink' | 'brown' | null;
  /** 1 to 127: the part's gain after the song's trim, scaled so that `VELOCITY_FULL_GAIN` is 127. */
  velocity: number;
  /** Whether this stream is a partner layer of another (its pan is mirrored). */
  partner: boolean;
}

/** The unique label of every stream of a song. */
export function streamLabels(song: MusicSong): string[] {
  const seen = new Map<string, number>();
  return song.streams.map((stream) => {
    const part = stream.find((p) => p !== null)!;
    const base = part.role ?? 'part';
    const partner = part.mirror !== undefined;
    const key = `${base}${partner ? '-partner' : ''}`;
    // The nth main stream of a role is `role#n`; its partner shares the n of the main stream before it.
    if (!partner) {
      const n = (seen.get(base) ?? 0) + 1;
      seen.set(base, n);
      return n === 1 ? key : `${base}#${n}`;
    }
    const n = seen.get(base) ?? 1;
    return n === 1 ? key : `${base}#${n}-partner`;
  });
}

const velocityOf = (gain: number): number => Math.max(1, Math.min(127, Math.round((127 * gain) / VELOCITY_FULL_GAIN)));

/** Every note and noise burst of the song, in order of start, then stream, then pitch. */
export function noteEvents(layout: SongLayout): NoteEvent[] {
  const { song } = layout;
  const labels = streamLabels(song);
  const events: NoteEvent[] = [];
  layout.sections.forEach((section, sectionIndex) => {
    song.streams.forEach((stream, streamIndex) => {
      const part = stream[sectionIndex];
      if (!part) return;
      for (const e of queryPart(part, 0, section.cycles)) {
        const start = (section.startBar + e.t0) * TICKS_PER_BAR;
        const end = (section.startBar + Math.min(e.t1, section.cycles)) * TICKS_PER_BAR;
        const tick = Math.round(start);
        if (Math.abs(start - tick) > 1e-6 || Math.abs(end - Math.round(end)) > 1e-6) {
          throw new Error(`stream ${streamIndex} section ${section.name}: an onset is off the ${TICKS_PER_BAR}-tick grid`);
        }
        events.push({
          tick,
          length: Math.round(end) - tick,
          stream: streamIndex,
          part: labels[streamIndex]!,
          role: part.role,
          midi: e.midi ?? null,
          noise: e.noise ?? null,
          velocity: velocityOf(part.g),
          partner: part.mirror !== undefined,
        });
      }
    });
  });
  return events.sort((a, b) => a.tick - b.tick || a.stream - b.stream || (a.midi ?? -1) - (b.midi ?? -1));
}

/** The note events of one song as a time in seconds (a bar is four beats at the song's tempo). */
export const tickSeconds = (song: MusicSong, tick: number): number => (tick * 240) / song.bpm / TICKS_PER_BAR;

/** The number of events in each part, by label, in stream order. */
export function countsByPart(events: NoteEvent[], labels: string[]): Record<string, number> {
  return Object.fromEntries(labels.map((label) => [label, events.filter((e) => e.part === label).length]));
}
