// Tests the song source: the builder holds to Fizz 1's output, the planner picks the form nearest a minute, the depth partners and
// the role pans are as designed, and the note list is deterministic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test assets/music`. Nothing here makes a sound; nobody has listened to the song.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { CRATER, CRATER_DEF } from './crater.ts';
import { noteEvents, streamLabels, TICKS_PER_BAR, tickSeconds } from './events.ts';
import { layoutFor, loopCycles, plan, songBars, songFor, songSeconds, transposeNotes, withDepth, TARGET_SECONDS } from './song.ts';
import { levelSong, MANIFEST, songById, TRACKS } from './songs.ts';
import { ARP_STEPS, PART_PAN, partPanAt, type MusicTrack } from './types.ts';

const fixture = JSON.parse(readFileSync(new URL('./fizz1-crater.fixture.json', import.meta.url), 'utf8'));
const plain = <T>(value: T): T => JSON.parse(JSON.stringify(value));

describe('drift from Fizz 1 (fixture saved from commit 4bce036)', () => {
  test('the track is Fizz 1s track', () => {
    expect(plain(CRATER)).toEqual(fixture.track);
  });

  test('the song has the same tempo, sections, parts and notes', () => {
    const song = plain(songFor(CRATER, CRATER_DEF));
    expect(song.bpm).toBe(fixture.song.bpm);
    expect(song.cycles).toEqual(fixture.song.cycles);
    expect(song.streams.length).toBe(fixture.song.streams.length);
    song.streams.forEach((stream: unknown[], i: number) => expect(stream).toEqual(fixture.song.streams[i]));
    expect(song).toEqual(fixture.song);
  });

  test('39 bars, 63.24 seconds at 148 bpm', () => {
    const song = songFor(CRATER, CRATER_DEF);
    expect(song.bpm).toBe(148);
    expect(song.cycles).toEqual([2, 8, 8, 8, 3, 1, 8, 1]);
    expect(songBars(song)).toBe(39);
    expect(songSeconds(song).toFixed(2)).toBe('63.24');
  });
});

describe('the planner', () => {
  test('the track repeats every 8 bars', () => {
    expect(loopCycles(CRATER)).toBe(8);
  });

  test('lays out Intro, A, B, A (octave), Breakdown, Build, A prime, Fill', () => {
    expect(plan(CRATER, CRATER_DEF).map((s) => `${s.name} ${s.cycles}`)).toEqual([
      'Intro 2',
      'A 8',
      'B 8',
      'A (octave) 8',
      'Breakdown 3',
      'Build 1',
      "A' 8",
      'Fill 1',
    ]);
  });

  test('the form chosen is the one nearest 60 seconds', () => {
    const seconds = songSeconds(songFor(CRATER, CRATER_DEF));
    // The other candidate forms of an 8-bar loop are 20.9 s (A B'), 37.0 s (A B A'), 52.1 s... the chosen 63.24 s is nearest.
    const alternatives = [2 + 8 * 2 + 1, 2 + 8 * 3 + 1, 2 + 8 * 2 + 4 + 8 + 1, 2 + 8 * 5 + 4 + 1].map((bars) => (bars * 240) / 148);
    for (const alt of alternatives) {
      expect(Math.abs(seconds - TARGET_SECONDS)).toBeLessThanOrEqual(Math.abs(alt - TARGET_SECONDS));
    }
  });

  test('a pinned form wins over the nearest', () => {
    const sections = plan(CRATER, { ...CRATER_DEF, form: "A B A'" });
    expect(sections.map((s) => s.name)).toEqual(['Intro', 'A', 'B', "A'", 'Fill']);
  });

  test('a short loop may play each section twice', () => {
    const short: MusicTrack = { bpm: 120, parts: [{ notes: 'a4 c5 e5 a5', g: 0.1, role: 'lead' }] };
    expect(loopCycles(short)).toBe(1);
    expect(songBars(songFor(short, CRATER_DEF))).toBeGreaterThan(0);
  });

  test('transposing keeps noise words and shifts note names', () => {
    expect(transposeNotes('a4 white c#5 ~', 5)).toBe('d5 white f#5 ~');
    expect(transposeNotes('a4', 0)).toBe('a4');
  });

  test('is deterministic', () => {
    expect(plain(songFor(CRATER, CRATER_DEF))).toEqual(plain(songFor(CRATER, CRATER_DEF)));
  });
});

describe('depth', () => {
  const lead = CRATER.parts[0]!;

  test('a melodic part gets a main voice and a quiet sine partner on the same notes', () => {
    const [main, partner, extra] = withDepth(lead, 0);
    expect(extra).toBeUndefined();
    expect(main!.notes).toBe(lead.notes);
    expect(main!.g).toBeCloseTo(lead.g * 0.85, 12);
    expect(main!.lpenv).toBeCloseTo(2300 * 1.4, 9);
    expect(partner!.notes).toBe(lead.notes);
    expect(partner!.w).toBe('sine');
    expect(partner!.g).toBeCloseTo(lead.g * 0.5, 12);
    expect(partner!.nudge).toBe(0.002);
  });

  test('the partner is panned the other way, by the stream index', () => {
    expect(withDepth(lead, 0)[1]!.mirror).toBe(-0.22);
    expect(withDepth(lead, 1)[1]!.mirror).toBe(0.22);
  });

  test('a pad gets no partner, drums and noise are untouched', () => {
    expect(withDepth(CRATER.parts[2]!, 2)).toHaveLength(1);
    for (const index of [3, 4, 5]) expect(withDepth(CRATER.parts[index]!, index)).toEqual([CRATER.parts[index]!]);
  });

  test('every built partner carries a mirror and only melodic parts have one', () => {
    const { streams } = songFor(CRATER, CRATER_DEF);
    const roles = streams.map((s) => [s.find((p) => p)!.role, s.find((p) => p)!.mirror !== undefined] as const);
    for (const [role, partner] of roles) {
      if (partner) expect(['lead', 'bass', 'arp']).toContain(role);
    }
    expect(roles.filter(([, partner]) => partner).length).toBe(5);
  });
});

describe('role pans', () => {
  test('low and central parts are centred', () => {
    for (const role of ['bass', 'kick', 'snare', 'pad', 'drone'] as const) expect(PART_PAN[role]).toBe(0);
  });

  test('the others lean by at most 0.3', () => {
    for (const pan of Object.values(PART_PAN)) {
      for (const p of typeof pan === 'number' ? [pan] : pan) expect(Math.abs(p)).toBeLessThanOrEqual(0.3);
    }
    expect(PART_PAN.lead).toBe(-0.15);
    expect(PART_PAN.hats).toBe(0.25);
  });

  test('the arpeggio alternates left and right on every eighth', () => {
    expect(ARP_STEPS).toBe(8);
    expect(partPanAt('arp', 0)).toBe(-0.3);
    expect(partPanAt('arp', 1 / 8)).toBe(0.3);
    expect(partPanAt('arp', 2 / 8)).toBe(-0.3);
    expect(partPanAt('arp', 7 / 8)).toBe(0.3);
    expect(partPanAt('lead', 0.5)).toBe(-0.15);
  });

  test('every part of the song has a role the table knows', () => {
    for (const stream of songFor(CRATER, CRATER_DEF).streams) {
      const role = stream.find((p) => p)!.role!;
      expect(role in PART_PAN).toBe(true);
    }
  });
});

describe('the manifest', () => {
  test('the level plays Crater Fields, marked as a placeholder', () => {
    expect(MANIFEST.level).toBe('crater');
    expect(TRACKS.crater!.title).toBe('Crater Fields');
    expect(TRACKS.crater!.placeholder).toBe(true);
    expect(levelSong().id).toBe('crater');
    expect(() => songById('nope')).toThrow();
  });

  test('the layout names the sections and their start bars', () => {
    expect(layoutFor(CRATER, CRATER_DEF).sections.map((s) => s.startBar)).toEqual([0, 2, 10, 18, 26, 29, 30, 38]);
  });
});

describe('note events', () => {
  const layout = levelSong();
  const events = noteEvents(layout);
  const labels = streamLabels(layout.song);

  test('are deterministic and ordered', () => {
    expect(noteEvents(levelSong())).toEqual(events);
    for (let i = 1; i < events.length; i++) expect(events[i]!.tick).toBeGreaterThanOrEqual(events[i - 1]!.tick);
  });

  test('stay inside the song', () => {
    const end = songBars(layout.song) * TICKS_PER_BAR;
    for (const e of events) {
      expect(e.tick).toBeGreaterThanOrEqual(0);
      expect(e.length).toBeGreaterThan(0);
      expect(e.tick + e.length).toBeLessThanOrEqual(end);
      expect(e.velocity).toBeGreaterThanOrEqual(1);
      expect(e.velocity).toBeLessThanOrEqual(127);
    }
    expect(tickSeconds(layout.song, end).toFixed(2)).toBe('63.24');
  });

  test('labels are unique and every part sounds', () => {
    expect(new Set(labels).size).toBe(labels.length);
    for (const label of labels) expect(events.some((e) => e.part === label)).toBe(true);
  });

  test('the first bar is the intro: roots, pad and hats only', () => {
    const first = events.filter((e) => e.tick < TICKS_PER_BAR);
    expect(new Set(first.map((e) => e.part))).toEqual(new Set(['bass', 'bass-partner', 'pad', 'hats']));
    const bass = first.find((e) => e.part === 'bass')!;
    expect(bass.midi).toBe(45);
    expect(bass.length).toBe(TICKS_PER_BAR);
  });

  test('the lead opens A on a4, then c5 on the third eighth', () => {
    const lead = events.filter((e) => e.part === 'lead' && e.tick >= 2 * TICKS_PER_BAR);
    expect(lead[0]!.tick).toBe(2 * TICKS_PER_BAR);
    expect(lead[0]!.midi).toBe(69);
    expect(lead[1]!.midi).toBe(72);
    expect(lead[1]!.tick - lead[0]!.tick).toBe(24);
  });

  test('B is up 5 semitones and A prime up 2', () => {
    const at = (bar: number) => events.find((e) => e.part === 'lead' && e.tick === bar * TICKS_PER_BAR)!.midi;
    expect(at(10)).toBe(69 + 5);
    expect(at(30)).toBe(69 + 2);
  });

  test('noise parts carry no pitch, pitched parts carry no noise', () => {
    for (const e of events) {
      if (e.role === 'hats' || e.role === 'snare') expect([e.midi, e.noise]).toEqual([null, 'white']);
      else expect(e.noise).toBeNull();
    }
  });
});
