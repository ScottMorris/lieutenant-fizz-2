// Lowers the level song to OPL register streams: an OPL2 arrangement (five melodic channels, rhythm-mode drums) and an OPL3 one
// (thirteen melodic channels, partner layers, stereo output bits by role).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from `convertParts` (`m4/sound/fizz-dos/music.ts`, lines 79-220) and `convertSong` (`song.ts`) of
// ScottMorris/lieutenant-fizz-dos (a.k.a. fizz-dos) at commit 221dd6c, the same author's code, MIT there and dual-licensed here. What
// changed:
// - the input is the flat `noteEvents` list of `assets/music/events.ts` (MIDI notes on a 96-tick bar grid, one stream per voice,
//   partner layers included) instead of Undertone haps, and the part of a stream is its first appearance, as the port keeps the
//   first appearance of a slot;
// - the output is `OplRecord`s (with a bank bit) rather than a Type-1 IMF chunk, and has a recorded loop point: the one-time setup
//   (waveform enable, rhythm mode, drum instruments, the first notes' instruments) comes first, as a prelude that is not repeated;
// - the five-channel allocator and its priority stealing are the port's, for the OPL2 arrangement, with channel 5 left to effects;
// - the OPL3 arrangement gives each voice a fixed channel (13 channels, 4-operator pairs never enabled, channel 14 left to effects),
//   so no note is dropped, and adds the output bits of `stereo.ts` to each channel's C0 register;
// - an instrument write is no longer made all at once at its note's tick (the port's tick 0 burst is 69 writes): every write that
//   is not a key-on or key-off is placed in the ticks between the channel's previous key-off and the note, latest first, so that no
//   tick carries more than `MAX_WRITES_PER_TICK`. A note that still cannot fit starts up to `MAX_DELAY` ticks late, or the note
//   before it stops up to `MAX_PULL` ticks early; both are counted in the arrangement's stats.
// Pure and deterministic: no clock, no randomness.

import { noteEvents, TICKS_PER_BAR, type NoteEvent } from '../../assets/music/events.ts';
import { songBars, type SongLayout } from '../../assets/music/song.ts';
import { ARP_STEPS, type MusicPart, type PartRole } from '../../assets/music/types.ts';
import { IMF_RATE, OPERATORS, attenuation, fnumber, type OplRecord, type Patch } from '../sfx/opl.ts';
import {
  DRUMS,
  DRUM_CHANNEL_HZ,
  DRUM_PATCHES,
  drumForRole,
  patchForPart,
  type DrumName,
} from './patches.ts';
import { maxWritesPerTick } from './format.ts';
import { OUTPUT_BITS, sideFor } from './stereo.ts';

export type Kind = 'opl2' | 'opl3';

/** The melodic channels of each arrangement. Channels 5 and 14 are the effects'; 6 to 8 are the rhythm-mode drums. */
export const MELODIC_CHANNELS: Record<Kind, readonly number[]> = {
  opl2: [0, 1, 2, 3, 4],
  opl3: [0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 15, 16, 17],
};
/** The channels the drums sit on in rhythm mode. */
export const DRUM_CHANNELS = [6, 7, 8] as const;

/** The most register writes any one tick of a music stream may carry, so an interrupt at 560 Hz stays inside its period. */
export const MAX_WRITES_PER_TICK = 16;
/** Steps of 0.75 dB taken off every music note, so the channels and the drums together stay clear of the chip's ceiling (the port's trim). */
export const MUSIC_TRIM = 6;
/** Further steps taken off the OPL3 arrangement, which has more channels sounding at once. */
export const OPL3_EXTRA_TRIM = 3;
/** How late (ticks of 1/560 s) a note may start when its instrument writes do not fit before it. */
export const MAX_DELAY = 3;
/** How early (ticks) the note before may be cut short to make room for the next note's instrument writes. */
export const MAX_PULL = 3;
/** How far back (ticks) before a note its instrument writes may be placed. */
const WINDOW = 48;
/** The gap (ticks) voices sharing a channel keep from each other: none, as the key-off before each note is added later. */
const SHARE_GAP = 0;

/** How each role ranks when there are more notes than melodic channels: the tune and the bass keep their place. The port's table. */
const PRIORITY: Partial<Record<PartRole, number>> = {
  lead: 9,
  bass: 8,
  counter: 7,
  arp: 6,
  bell: 5,
  pad: 3,
  drone: 2,
};
const priorityOf = (role: PartRole | undefined): number => (role && PRIORITY[role]) || 4;

export interface DroppedNote {
  /** The start on the 96-tick bar grid, as in the note list. */
  tick: number;
  part: string;
  midi: number;
}

/** A note as the arrangement finally plays it. */
export interface PlacedNote {
  channel: number;
  /** The key-on tick, in 560 Hz ticks from the loop start. */
  start: number;
  /** The key-off tick. */
  end: number;
  /** The tick the note was written at in the source, before any delay. */
  nominal: number;
  midi: number;
  part: string;
  tick96: number;
}

export interface Arrangement {
  kind: Kind;
  /** The melodic channels this arrangement may write. */
  channels: readonly number[];
  /** One pass: the prelude records, then one loop. */
  records: OplRecord[];
  /** The index of the first record of the loop; the player jumps back to it. */
  loopStart: number;
  /** Ticks from the first record to the first record of the loop. */
  preludeTicks: number;
  /** Ticks in one loop: from the loop's first record to the same record on the next pass. */
  loopTicks: number;
  notes: number;
  dropped: DroppedNote[];
  drumHits: number;
  maxWritesPerTick: number;
  placed: PlacedNote[];
  stats: {
    /** Notes that start late, and the most they are late by. */
    delayed: number;
    maxDelay: number;
    /** Notes cut short, and the most they are cut by. */
    pulled: number;
    maxPull: number;
    /** Instrument loads (full patch writes) in the loop, not counting the prelude's. */
    patchLoads: number;
  };
}

interface Raw {
  stream: number;
  label: string;
  role: PartRole | undefined;
  partner: boolean;
  start: number;
  end: number;
  midi: number;
  hz: number;
  patch: Patch;
  tl: number;
  bits: number;
  priority: number;
  tick96: number;
}

interface Seg extends Raw {
  id: number;
  channel: number;
  nominal: number;
  /** The previous note on the channel, going round the loop; itself when it is alone. */
  prev: Seg;
  /** Added to `prev.end` to put it before this note: -total when `prev` is later in the loop. */
  wrap: number;
  offAt: number;
}

interface Write {
  reg: number;
  val: number;
}

const hzOf = (midi: number): number => 440 * 2 ** ((midi - 69) / 12);

/** The register of `base` for a channel numbered 0 to 17: the bank's bit is bit 8. */
function address(channel: number, base: number, operator: 0 | 1 | null): number {
  const bank = channel >= 9 ? 0x100 : 0;
  const slot = channel % 9;
  return bank | (base + (operator === null ? slot : OPERATORS[slot]![operator]!));
}

const c0Value = (patch: Patch, bits: number): number => ((patch.feedback & 7) << 1) | (patch.additive ? 1 : 0) | bits;

/** The writes that load a patch into a channel (the port's `setPatch`), with the carrier level and output bits. */
function patchWrites(channel: number, patch: Patch, tl: number, bits: number): Write[] {
  const r = (o: Patch['mod']) => ({
    r20: (o.tremolo ? 0x80 : 0) | (o.vibrato ? 0x40 : 0) | (o.sustain ? 0x20 : 0) | (o.mult & 0x0f),
    r40: ((o.ksl ?? 0) << 6) | (o.tl & 0x3f),
    r60: ((o.ar & 0x0f) << 4) | (o.dr & 0x0f),
    r80: ((o.sl & 0x0f) << 4) | (o.rr & 0x0f),
    re0: (o.wave ?? 0) & 3,
  });
  const m = r(patch.mod);
  const c = r(patch.car);
  return [
    { reg: address(channel, 0x20, 0), val: m.r20 },
    { reg: address(channel, 0x20, 1), val: c.r20 },
    { reg: address(channel, 0x40, 0), val: m.r40 },
    { reg: address(channel, 0x40, 1), val: (c.r40 & 0xc0) | Math.min(63, (c.r40 & 0x3f) + tl) },
    { reg: address(channel, 0x60, 0), val: m.r60 },
    { reg: address(channel, 0x60, 1), val: c.r60 },
    { reg: address(channel, 0x80, 0), val: m.r80 },
    { reg: address(channel, 0x80, 1), val: c.r80 },
    { reg: address(channel, 0xe0, 0), val: m.re0 },
    { reg: address(channel, 0xe0, 1), val: c.re0 },
    { reg: address(channel, 0xc0, null), val: c0Value(patch, bits) },
  ];
}

const levelWrite = (channel: number, patch: Patch, tl: number): Write => ({
  reg: address(channel, 0x40, 1),
  val: ((patch.car.ksl ?? 0) << 6) | Math.min(63, patch.car.tl + tl),
});

const pitchLow = (channel: number, hz: number): Write => ({ reg: address(channel, 0xa0, null), val: fnumber(hz).fnum & 0xff });
const keyWrite = (channel: number, hz: number, on: boolean): Write => {
  const { fnum, block } = fnumber(hz);
  return { reg: address(channel, 0xb0, null), val: (on ? 0x20 : 0) | (block << 2) | (fnum >> 8) };
};

// ---------- reading the song ----------

interface Source {
  raw: Raw[];
  drumHits: Map<DrumName, number[]>;
  total: number;
  ticksPerBar: number;
}

function readSong(layout: SongLayout, kind: Kind): Source {
  const { song } = layout;
  const ticksPerBar = (240 / song.bpm) * IMF_RATE;
  const conv = (t96: number): number => Math.round((t96 / TICKS_PER_BAR) * ticksPerBar);
  const total = conv(songBars(song) * TICKS_PER_BAR);
  const partOf = (stream: number): MusicPart => song.streams[stream]!.find((p) => p !== null)!;
  const events = noteEvents(layout);
  const drumOf = (e: NoteEvent): DrumName | undefined => drumForRole(e.role) ?? (partOf(e.stream).noise ? 'cymbal' : undefined);

  // The loudest main melodic part is the reference every level is measured against, as in the port.
  const reference = Math.max(
    ...events.filter((e) => drumOf(e) === undefined && !e.partner && e.midi !== null).map((e) => partOf(e.stream).g),
    0.01,
  );
  const trim = MUSIC_TRIM + (kind === 'opl3' ? OPL3_EXTRA_TRIM : 0);

  const drumHits = new Map<DrumName, number[]>();
  const raw: Raw[] = [];
  for (const e of events) {
    const drum = drumOf(e);
    if (drum) {
      drumHits.set(drum, [...(drumHits.get(drum) ?? []), conv(e.tick)]);
      continue;
    }
    if (e.midi === null) continue;
    if (kind === 'opl2' && e.partner) continue;
    const part = partOf(e.stream);
    const side = kind === 'opl3' ? sideFor(e.role, e.partner, e.tick % TICKS_PER_BAR, TICKS_PER_BAR, ARP_STEPS) : null;
    raw.push({
      stream: e.stream,
      label: e.part,
      role: e.role,
      partner: e.partner,
      start: conv(e.tick),
      end: Math.min(conv(e.tick + e.length), total - 1),
      midi: e.midi,
      hz: hzOf(e.midi),
      patch: patchForPart(part.role, part.w),
      tl: attenuation(part.g, reference) + trim,
      bits: side ? OUTPUT_BITS[side] : 0,
      priority: priorityOf(part.role),
      tick96: e.tick,
    });
  }
  return { raw, drumHits, total, ticksPerBar };
}

// ---------- giving notes to channels ----------

type Placed = Omit<Seg, 'prev' | 'wrap' | 'offAt' | 'id'>;

/** The port's allocator: each note takes the channel its part used last, else an unused one, else any free one, else steals. */
function allocateOpl2(raw: Raw[], channels: readonly number[]): { placed: Placed[]; dropped: Raw[] } {
  const notes = [...raw].sort((a, b) => a.start - b.start || b.priority - a.priority);
  const current: (Placed | undefined)[] = channels.map(() => undefined);
  const placed: Placed[] = [];
  const dropped: Raw[] = [];
  for (const e of notes) {
    const seg: Placed = { ...e, end: Math.max(e.end, e.start + 2), channel: -1, nominal: e.start };
    const free = current
      .map((c, slot) => ({ c, slot }))
      .filter(({ c }) => !c || c.end <= seg.start);
    let slot: number | undefined = (
      free.find(({ c }) => c?.stream === e.stream) ??
      free.find(({ c }) => c === undefined) ??
      free[0]
    )?.slot;
    if (slot === undefined) {
      // Everything is sounding: take the channel of the least important note, if this one matters more.
      const busy = current
        .map((c, s) => ({ c: c!, slot: s }))
        .sort((a, b) => a.c.priority - b.c.priority || a.c.end - b.c.end);
      if (busy[0]!.c.priority < seg.priority) {
        slot = busy[0]!.slot;
        busy[0]!.c.end = Math.max(busy[0]!.c.start + 1, seg.start - 1);
      }
    }
    if (slot === undefined) {
      dropped.push(e);
      continue;
    }
    seg.channel = channels[slot]!;
    placed.push(seg);
    current[slot] = seg;
  }
  return { placed, dropped };
}

/**
 * A fixed channel for each voice: a voice that plays chords has one lane per simultaneous note, and each lane gets its own channel
 * while there are spare ones, then shares with a lane it never overlaps. A song that needs more than the channels allow is refused
 * rather than thinned.
 */
function allocateStatic(raw: Raw[], channels: readonly number[]): Placed[] {
  const byStream = new Map<number, Raw[]>();
  for (const e of raw) byStream.set(e.stream, [...(byStream.get(e.stream) ?? []), e]);
  const streams = [...byStream.entries()].sort((a, b) => b[1].length - a[1].length || a[0] - b[0]);
  const owned = new Map<number, Raw[]>(channels.map((c) => [c, []]));
  const channelOf = new Map<Raw, number>();
  const clash = (a: Raw[], b: Raw[]): boolean =>
    a.some((x) => b.some((y) => x.start < y.end + SHARE_GAP && y.start < x.end + SHARE_GAP));
  for (const [stream, notes] of streams) {
    const sorted = [...notes].sort((a, b) => a.start - b.start || a.midi - b.midi);
    const lanes: Raw[][] = [];
    for (const n of sorted) {
      const lane = lanes.find((l) => l[l.length - 1]!.end + SHARE_GAP <= n.start);
      if (lane) lane.push(n);
      else lanes.push([n]);
    }
    for (const lane of lanes) {
      const empty = channels.find((c) => owned.get(c)!.length === 0);
      const candidates = channels.filter((c) => !clash(owned.get(c)!, lane));
      const same = candidates.find((c) => owned.get(c)![0]?.patch === lane[0]!.patch && owned.get(c)![0]?.tl === lane[0]!.tl);
      const pick = empty ?? same ?? candidates[0];
      if (pick === undefined) {
        throw new Error(`the ${channels.length} channels cannot hold the voice "${notes[0]!.label}" (stream ${stream}) without dropping notes`);
      }
      owned.get(pick)!.push(...lane);
      for (const n of lane) channelOf.set(n, pick);
    }
  }
  return raw.map((e) => ({ ...e, end: Math.max(e.end, e.start + 2), channel: channelOf.get(e)!, nominal: e.start }));
}

// ---------- placing the writes ----------

/** The per-channel state the pre-writes are measured against: what the chip will hold when the next note comes. */
interface ChannelState {
  patch: Patch;
  tl: number;
  c0: number;
  a0: number;
}

const stateAfter = (g: Seg): ChannelState => ({ patch: g.patch, tl: g.tl, c0: c0Value(g.patch, g.bits), a0: pitchLow(g.channel, g.hz).val });

/** The writes a note needs before its key-on, given what the channel held after the note before it. `full` loads everything. */
function preWrites(g: Seg, prior: ChannelState | null): Write[] {
  const out: Write[] = [];
  if (prior === null || prior.patch !== g.patch) out.push(...patchWrites(g.channel, g.patch, g.tl, g.bits));
  else {
    if (prior.tl !== g.tl) out.push(levelWrite(g.channel, g.patch, g.tl));
    const c0 = c0Value(g.patch, g.bits);
    if (prior.c0 !== c0) out.push({ reg: address(g.channel, 0xc0, null), val: c0 });
  }
  const a0 = pitchLow(g.channel, g.hz);
  if (prior === null || prior.a0 !== a0.val) out.push(a0);
  return out;
}

interface Entry extends Write {
  rank: number;
}

interface Scheduled {
  loop: Entry[][];
  maxDelay: number;
  delayed: number;
  maxPull: number;
  pulled: number;
  patchLoads: number;
  preludeNotes: Write[];
}

function schedule(segs: Seg[], drumChanges: { tick: number; mask: number }[], total: number): Scheduled {
  const cap = new Array<number>(total).fill(MAX_WRITES_PER_TICK);
  const ticks: Entry[][] = Array.from({ length: total }, () => []);
  const idx = (t: number): number => ((t % total) + total) % total;
  const put = (t: number, e: Entry): void => {
    const i = idx(t);
    if (cap[i]! < 1) throw new Error(`no room in tick ${i}`);
    cap[i]!--;
    ticks[i]!.push(e);
  };

  for (const c of drumChanges) put(c.tick, { reg: 0xbd, val: 0x20 | c.mask, rank: 3 });

  // Order each channel's notes round the loop and cut each short of the next one, as the port does.
  const byChannel = new Map<number, Seg[]>();
  for (const g of [...segs].sort((a, b) => a.start - b.start || a.channel - b.channel)) {
    byChannel.set(g.channel, [...(byChannel.get(g.channel) ?? []), g]);
  }
  for (const list of byChannel.values()) {
    list.forEach((g, i) => {
      const next = list[i + 1];
      if (next) g.end = Math.min(g.end, next.start - 1);
      g.end = Math.max(g.end, g.start + 1);
      g.prev = list[(i + list.length - 1) % list.length]!;
      g.wrap = i === 0 ? -total : 0;
    });
  }

  // Key-offs are fixed; one that finds its tick full moves earlier.
  for (const g of segs) {
    let t = g.end;
    while (cap[idx(t)]! < 1 && t > g.start + 1) t--;
    g.end = t;
    g.offAt = t;
    put(t, { ...keyWrite(g.channel, g.hz, false), rank: 1 });
  }

  const moveOff = (g: Seg, to: number): void => {
    const from = idx(g.offAt);
    const list = ticks[from]!;
    const k = list.findIndex((e) => e.rank === 1 && e.reg === address(g.channel, 0xb0, null));
    const [entry] = list.splice(k, 1);
    cap[from]!++;
    put(to, entry!);
    g.offAt = to;
    g.end = to;
  };

  let maxDelay = 0;
  let delayed = 0;
  let maxPull = 0;
  let pulled = 0;
  let patchLoads = 0;
  const order = [...segs].sort((a, b) => a.start - b.start || b.priority - a.priority || a.id - b.id);
  for (const g of order) {
    const needs = preWrites(g, stateAfter(g.prev));
    if (g.prev.patch !== g.patch) patchLoads++;
    let done = false;
    for (let s = 0; s <= MAX_DELAY + MAX_PULL && !done; s++) {
      for (let d = Math.max(0, s - MAX_PULL); d <= Math.min(s, MAX_DELAY) && !done; d++) {
        const p = s - d;
        const u = g.start + d;
        if (u > g.end - 1 || u > total - 1 || cap[idx(u)]! < 1) continue;
        const prevOff = g.prev.end + g.wrap;
        if (p > 0 && (g.prev === g || g.prev.end - p < g.prev.start + 1 || cap[idx(g.prev.end - p)]! < 1)) continue;
        const was = g.prev.end;
        if (p > 0) moveOff(g.prev, g.prev.end - p);
        const lo = Math.max(prevOff - p + 1, u - WINDOW);
        let free = -1;
        for (let t = lo; t <= u; t++) free += cap[idx(t)]!;
        if (free >= needs.length) {
          put(u, { ...keyWrite(g.channel, g.hz, true), rank: 2 });
          let left = needs.length;
          for (let t = u; t >= lo && left > 0; t--) {
            while (cap[idx(t)]! > 0 && left > 0) put(t, { ...needs[--left]!, rank: 0 });
          }
          if (d > 0) {
            delayed++;
            maxDelay = Math.max(maxDelay, d);
          }
          if (p > 0) {
            pulled++;
            maxPull = Math.max(maxPull, p);
          }
          g.start = u;
          done = true;
        } else if (p > 0) {
          moveOff(g.prev, was);
        }
      }
    }
    if (!done) {
      throw new Error(`could not place the note ${g.label} midi ${g.midi} at tick ${g.start} on channel ${g.channel} within ${MAX_WRITES_PER_TICK} writes a tick`);
    }
  }

  // The first note of each channel is loaded in full by the prelude, because the loop's tail has not run yet.
  const preludeNotes: Write[] = [];
  for (const list of byChannel.values()) preludeNotes.push(...preWrites(list[0]!, null));
  return { loop: ticks, maxDelay, delayed, maxPull, pulled, patchLoads, preludeNotes };
}

// ---------- assembling a stream ----------

/** Splits a list of writes over ticks, `MAX_WRITES_PER_TICK` to a tick, and gives each its time. */
function packPrelude(writes: Write[]): { timed: { tick: number; w: Write }[]; ticks: number } {
  const timed = writes.map((w, i) => ({ tick: Math.floor(i / MAX_WRITES_PER_TICK), w }));
  return { timed, ticks: timed.length ? timed[timed.length - 1]!.tick + 1 : 0 };
}

function asRecords(timed: { tick: number; w: Write }[], end: number): OplRecord[] {
  return timed.map((t, i) => {
    const next = i + 1 < timed.length ? timed[i + 1]!.tick : end;
    return { bank: (t.w.reg >> 8) & 1, reg: t.w.reg & 0xff, val: t.w.val, wait: next - t.tick };
  });
}

/** Lowers the song to the arrangement of `kind`. */
export function lower(layout: SongLayout, kind: Kind): Arrangement {
  const channels = MELODIC_CHANNELS[kind];
  const { raw, drumHits, total } = readSong(layout, kind);

  let placed: Placed[];
  let dropped: Raw[] = [];
  if (kind === 'opl2') ({ placed, dropped } = allocateOpl2(raw, channels));
  else placed = allocateStatic(raw, channels);
  const segs = placed.map((p, id) => ({ ...p, id }) as Seg);

  // ---- the rhythm-mode drums ----
  const changes: { tick: number; bit: number; on: boolean }[] = [];
  for (const [drum, hits] of drumHits) {
    const sorted = [...new Set(hits)].sort((a, b) => a - b);
    sorted.forEach((t, i) => {
      const next = sorted[i + 1] ?? total + sorted[0]!;
      const off = Math.min(t + DRUMS[drum].holdTicks, next - 1, total - 1);
      changes.push({ tick: t, bit: DRUMS[drum].bit, on: true });
      changes.push({ tick: Math.max(off, t + 1), bit: DRUMS[drum].bit, on: false });
    });
  }
  changes.sort((a, b) => a.tick - b.tick || Number(a.on) - Number(b.on));
  const drumChanges: { tick: number; mask: number }[] = [];
  let mask = 0;
  for (let i = 0; i < changes.length; ) {
    const at = changes[i]!.tick;
    while (i < changes.length && changes[i]!.tick === at) {
      mask = changes[i]!.on ? mask | changes[i]!.bit : mask & ~changes[i]!.bit;
      i++;
    }
    drumChanges.push({ tick: at, mask });
  }

  const sched = schedule(segs, drumChanges, total);

  // ---- the one-time setup, before the loop point ----
  const setup: Write[] = [{ reg: 0x01, val: 0x20 }];
  if (drumHits.size) {
    for (const drum of drumHits.keys()) {
      DRUMS[drum].ops.forEach((offset, i) => {
        const o = DRUM_PATCHES[drum][i]!;
        setup.push(
          { reg: 0x20 + offset, val: (o.sustain ? 0x20 : 0) | o.mult },
          { reg: 0x40 + offset, val: o.tl },
          { reg: 0x60 + offset, val: (o.ar << 4) | o.dr },
          { reg: 0x80 + offset, val: (o.sl << 4) | o.rr },
          { reg: 0xe0 + offset, val: o.wave ?? 0 },
        );
      });
    }
    // The bass drum is a two-operator voice on channel 6; its feedback and connection are in C6. On an OPL3 the drum channels also
    // need their output bits, or they are silent.
    const drumBits = kind === 'opl3' ? OUTPUT_BITS.centre : 0;
    setup.push({ reg: 0xc6, val: drumBits });
    if (kind === 'opl3') setup.push({ reg: 0xc7, val: drumBits }, { reg: 0xc8, val: drumBits });
    DRUM_CHANNEL_HZ.forEach((hz, i) => setup.push(pitchLow(6 + i, hz), keyWrite(6 + i, hz, false)));
  }
  setup.push(...sched.preludeNotes);
  if (drumHits.size) setup.push({ reg: 0xbd, val: 0x20 });
  const prelude = packPrelude(setup);

  // ---- the loop ----
  const loopTimed: { tick: number; w: Write }[] = [];
  sched.loop.forEach((entries, t) => {
    for (const e of [...entries].sort((a, b) => a.rank - b.rank)) loopTimed.push({ tick: t, w: e });
  });
  const first = loopTimed[0]?.tick ?? 0;
  // The first record of the loop is due at `first`; the wait before it is carried by the record that precedes it.
  const preludeRecords = asRecords(prelude.timed, prelude.ticks + first);
  const loopRecords = asRecords(
    loopTimed.map((t) => ({ tick: t.tick - first, w: t.w })),
    total,
  );
  const records = [...preludeRecords, ...loopRecords];
  const hits = [...drumHits.values()].reduce((n, h) => n + h.length, 0);

  const sum = (rs: OplRecord[]): number => rs.reduce((n, r) => n + r.wait, 0);
  return {
    kind,
    channels,
    records,
    loopStart: preludeRecords.length,
    preludeTicks: sum(preludeRecords),
    loopTicks: sum(loopRecords),
    notes: segs.length,
    dropped: dropped.map((d) => ({ tick: d.tick96, part: d.label, midi: d.midi })),
    drumHits: hits,
    maxWritesPerTick: maxWritesPerTick(records),
    placed: segs.map((g) => ({ channel: g.channel, start: g.start, end: g.end, nominal: g.nominal, midi: g.midi, part: g.label, tick96: g.tick96 })),
    stats: { delayed: sched.delayed, maxDelay: sched.maxDelay, pulled: sched.pulled, maxPull: sched.maxPull, patchLoads: sched.patchLoads },
  };
}
