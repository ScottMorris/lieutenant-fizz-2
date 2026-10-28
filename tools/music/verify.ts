// The generator's refusal checks and the note-parity check for a lowered music stream.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Refusals (any one fails the generator): a write to a register of channel 5 or channel 14 (the effects'), whichever bank it is in;
// a write to the OPL3 control registers 0x104 or 0x105 (the audio service owns them, and 0x104 would enable the 4-operator pairs 2/5
// and 11/14 that contain the effects' channels); a write to a channel the arrangement does not own; a bank-1 write in an OPL2
// stream; more than `MAX_WRITES_PER_TICK` writes in a tick; a channel still keyed on at the end of the loop; and a loop whose second
// pass does not set each note up as the first did. Parity: the note-ons decoded back from the registers must be the notes of
// `assets/music/notes/<song>.txt` (less the OPL2 arrangement's dropped notes), and the drum hits must be the manifest's.

import { TICKS_PER_BAR } from '../../assets/music/events.ts';
import { IMF_RATE, OPERATORS, OPL_CLOCK, RESERVED_OPL_CHANNELS, type OplRecord } from '../sfx/opl.ts';
import { ticksOf } from './format.ts';
import { DRUMS, drumForRole, type DrumName } from './patches.ts';
import { DRUM_CHANNELS, MAX_DELAY, MAX_WRITES_PER_TICK, type Arrangement } from './lower.ts';

/**
 * The low bytes of the registers of channel 5 (bank 0) and, in the second bank, channel 14: the two operators' 0x20, 0x40, 0x60, 0x80
 * and 0xE0 registers (offsets 0x0A and 0x0D), then 0xA5, 0xB5 and 0xC5. They are listed by hand and tested against `OPERATORS`.
 */
export const RESERVED_REGISTERS: readonly number[] = [
  0x2a, 0x2d, 0x4a, 0x4d, 0x6a, 0x6d, 0x8a, 0x8d, 0xea, 0xed, 0xa5, 0xb5, 0xc5,
];

/** The control registers the audio service owns: 0x104 (4-operator enables) and 0x105 (OPL3 enable), as (bank, register). */
export const SERVICE_REGISTERS: readonly (readonly [number, number])[] = [
  [1, 0x04],
  [1, 0x05],
];

/** The channel (0 to 17) a per-channel register belongs to, or null for a global register. */
export function channelOfRegister(bank: number, reg: number): number | null {
  const slotOfOffset = (offset: number): number => OPERATORS.findIndex(([m, c]) => m === offset || c === offset);
  let slot = -1;
  if ((reg >= 0x20 && reg <= 0x35) || (reg >= 0x40 && reg <= 0x55) || (reg >= 0x60 && reg <= 0x75) || (reg >= 0x80 && reg <= 0x95) || (reg >= 0xe0 && reg <= 0xf5)) {
    slot = slotOfOffset(reg & 0x1f);
  } else if ((reg >= 0xa0 && reg <= 0xa8) || (reg >= 0xb0 && reg <= 0xb8) || (reg >= 0xc0 && reg <= 0xc8)) {
    slot = reg & 0x0f;
  }
  return slot < 0 ? null : slot + bank * 9;
}

export interface Problem {
  check: string;
  message: string;
}

/** Runs every refusal check on one arrangement; an empty list means it may be written. */
export function refusals(a: Arrangement): Problem[] {
  const out: Problem[] = [];
  const owned = new Set<number>([...a.channels, ...(a.drumHits > 0 ? DRUM_CHANNELS : [])]);
  const ticks = ticksOf(a.records);
  const perTick = new Map<number, number>();
  a.records.forEach((r, i) => {
    const where = `record ${i} (tick ${ticks[i]}) bank ${r.bank} register 0x${r.reg.toString(16)}`;
    perTick.set(ticks[i]!, (perTick.get(ticks[i]!) ?? 0) + 1);
    if (RESERVED_REGISTERS.includes(r.reg)) {
      out.push({ check: 'reserved-channel', message: `${where} is a register of the effects' channel ${r.bank === 0 ? 5 : 14}` });
    }
    if (SERVICE_REGISTERS.some(([b, g]) => b === r.bank && g === r.reg)) {
      out.push({ check: 'service-register', message: `${where} belongs to the audio service (0x104 and 0x105)` });
    }
    if (a.kind === 'opl2' && r.bank !== 0) out.push({ check: 'opl2-bank', message: `${where}: an OPL2 stream writes only bank 0` });
    const channel = channelOfRegister(r.bank, r.reg);
    if (channel !== null && !owned.has(channel)) {
      out.push({ check: 'channel-ownership', message: `${where} is channel ${channel}, which this arrangement does not own` });
    }
    if (channel === null && !(r.bank === 0 && (r.reg === 0x01 || r.reg === 0xbd))) {
      out.push({ check: 'global-register', message: `${where} is not a register music may write` });
    }
  });
  for (const c of RESERVED_OPL_CHANNELS) {
    if (owned.has(c)) out.push({ check: 'reserved-channel', message: `channel ${c} is in the arrangement's channel list` });
  }
  for (const [t, n] of perTick) {
    if (n > MAX_WRITES_PER_TICK) out.push({ check: 'writes-per-tick', message: `tick ${t} has ${n} writes (limit ${MAX_WRITES_PER_TICK})` });
  }
  if (a.records.some((r) => r.wait < 0)) out.push({ check: 'wait', message: 'a record has a negative wait' });

  // Every channel must be keyed off when the loop ends.
  const regs = new Map<number, number>();
  for (const r of a.records) regs.set(r.bank * 256 + r.reg, r.val);
  for (let ch = 0; ch < 18; ch++) {
    const key = regs.get((ch >= 9 ? 256 : 0) + 0xb0 + (ch % 9));
    if (key !== undefined && key & 0x20) out.push({ check: 'keyed-off', message: `channel ${ch} is still keyed on at the end of the loop` });
  }
  if (((regs.get(0xbd) ?? 0) & 0x1f) !== 0) out.push({ check: 'keyed-off', message: 'a rhythm-mode drum is still keyed on at the end of the loop' });

  out.push(...loopConsistency(a));
  return out;
}

// ---------- decoding ----------

export interface Decoded {
  /** Key-ons, with the tick counted from the first loop record. */
  notes: { tick: number; midi: number; channel: number; c0: number }[];
  /** Rhythm-mode drum hits: a drum's bit going from 0 to 1 in 0xBD. */
  drums: { tick: number; drum: DrumName }[];
}

const midiOf = (fnum: number, block: number): number => Math.round(69 + 12 * Math.log2(((fnum * OPL_CLOCK) / 2 ** (20 - block)) / 440));

/** Replays one pass of the stream (the prelude, then the loop) and reads the notes back out of the registers. */
export function decode(a: Arrangement): Decoded {
  const regs = [new Uint8Array(256), new Uint8Array(256)];
  const notes: Decoded['notes'] = [];
  const drums: Decoded['drums'] = [];
  let tick = 0;
  let loopOrigin = 0;
  a.records.forEach((r, i) => {
    if (i === a.loopStart) loopOrigin = tick;
    const bank = regs[r.bank]!;
    const old = bank[r.reg]!;
    bank[r.reg] = r.val;
    if (r.bank === 0 && r.reg === 0xbd) {
      for (const [drum, d] of Object.entries(DRUMS)) {
        if (r.val & d.bit && !(old & d.bit)) drums.push({ tick: tick - loopOrigin, drum: drum as DrumName });
      }
    } else if (r.reg >= 0xb0 && r.reg <= 0xb8 && r.val & 0x20 && !(old & 0x20)) {
      const slot = r.reg - 0xb0;
      const fnum = bank[0xa0 + slot]! | ((r.val & 3) << 8);
      notes.push({ tick: tick - loopOrigin, midi: midiOf(fnum, (r.val >> 2) & 7), channel: slot + r.bank * 9, c0: bank[0xc0 + slot]! });
    }
    tick += r.wait;
  });
  return { notes, drums };
}

/** The sound of each note at its key-on: the registers of its channel, as the chip would hold them. */
function snapshots(a: Arrangement, passes: number): string[][] {
  const regs = [new Uint8Array(256), new Uint8Array(256)];
  const result: string[][] = [];
  const run = (from: number, to: number, into: string[]): void => {
    for (let i = from; i < to; i++) {
      const r = a.records[i]!;
      const old = regs[r.bank]![r.reg]!;
      regs[r.bank]![r.reg] = r.val;
      if (r.reg >= 0xb0 && r.reg <= 0xb8 && r.val & 0x20 && !(old & 0x20)) {
        const slot = r.reg - 0xb0;
        const bank = regs[r.bank]!;
        const [m, c] = OPERATORS[slot]!;
        const parts = [0x20, 0x40, 0x60, 0x80, 0xe0].flatMap((base) => [bank[base + m]!, bank[base + c]!]);
        into.push(`${slot + r.bank * 9}:${parts.join(',')}:${bank[0xc0 + slot]}:${bank[0xa0 + slot]}:${r.val}`);
      }
    }
  };
  for (let p = 0; p < passes; p++) {
    const into: string[] = [];
    run(p === 0 ? 0 : a.loopStart, a.records.length, into);
    result.push(into);
  }
  return result;
}

/** The second pass of the loop must set each note up exactly as the first did: the tail of the loop carries the next pass's instruments. */
function loopConsistency(a: Arrangement): Problem[] {
  const [first, second] = snapshots(a, 2);
  if (first!.length !== second!.length) return [{ check: 'loop', message: `the loop has ${first!.length} note-ons the first time and ${second!.length} the second` }];
  const at = first!.findIndex((s, i) => s !== second![i]);
  if (at >= 0) {
    return [{ check: 'loop', message: `note-on ${at} differs on the second pass:\n  first:  ${first![at]}\n  second: ${second![at]}` }];
  }
  return [];
}

// ---------- parity with the note list ----------

export interface ManifestNote {
  tick: number;
  part: string;
  /** The MIDI note, or null for noise. */
  midi: number | null;
}

/** Reads `assets/music/notes/<song>.txt`. */
export function parseManifest(text: string): ManifestNote[] {
  return text
    .split('\n')
    .filter((l) => l.trim() !== '' && !l.startsWith('#'))
    .map((l) => {
      const [tick, , part, note] = l.trim().split(/\s+/);
      return { tick: Number(tick), part: part!, midi: /^\d+$/.test(note!) ? Number(note) : null };
    });
}

/** The role a stream label is for: `lead#2-partner` is a `lead`. */
export const roleOf = (label: string): string => label.replace(/-partner$/, '').replace(/#\d+$/, '');

/** Compares what the stream plays with the manifest. Returns the mismatches. */
export function parity(a: Arrangement, manifest: ManifestNote[], bpm: number): Problem[] {
  const out: Problem[] = [];
  const conv = (t96: number): number => Math.round((t96 / TICKS_PER_BAR) * (240 / bpm) * IMF_RATE);
  const decoded = decode(a);
  const drumOf = (part: string): DrumName | undefined => drumForRole(roleOf(part) as never);

  const dropped = new Set(a.dropped.map((d) => `${d.tick}:${d.part}:${d.midi}`));
  const expected = manifest
    .filter((n) => n.midi !== null && drumOf(n.part) === undefined)
    .filter((n) => a.kind === 'opl3' || !n.part.endsWith('-partner'))
    .filter((n) => !dropped.has(`${n.tick}:${n.part}:${n.midi}`));
  if (expected.length !== a.notes) out.push({ check: 'parity', message: `the manifest expects ${expected.length} notes but the arrangement reports ${a.notes}` });
  if (decoded.notes.length !== expected.length) {
    out.push({ check: 'parity', message: `the stream plays ${decoded.notes.length} notes; the manifest (less dropped) has ${expected.length}` });
  }
  const byMidi = new Map<number, number[]>();
  for (const n of decoded.notes) byMidi.set(n.midi, [...(byMidi.get(n.midi) ?? []), n.tick]);
  for (const list of byMidi.values()) list.sort((x, y) => x - y);
  let shown = 0;
  for (const n of [...expected].sort((x, y) => x.tick - y.tick)) {
    const list = byMidi.get(n.midi!) ?? [];
    const want = conv(n.tick);
    const i = list.findIndex((t) => t >= want && t <= want + MAX_DELAY);
    if (i < 0) {
      if (shown++ < 10) out.push({ check: 'parity', message: `the note ${n.part} midi ${n.midi} at tick ${want} (bar tick ${n.tick}) is not played within ${MAX_DELAY} ticks` });
    } else list.splice(i, 1);
  }
  for (const [midi, list] of byMidi) {
    if (list.length && shown++ < 10) out.push({ check: 'parity', message: `the stream plays midi ${midi} at tick(s) ${list.slice(0, 4).join(', ')} that the manifest does not have` });
  }

  // Drums are not delayed: the same ticks, drum by drum.
  const hits = new Map<DrumName, Set<number>>();
  for (const n of manifest) {
    const drum = drumOf(n.part);
    if (drum) hits.set(drum, (hits.get(drum) ?? new Set()).add(conv(n.tick)));
  }
  for (const drum of Object.keys(DRUMS) as DrumName[]) {
    const want = [...(hits.get(drum) ?? [])].sort((x, y) => x - y);
    const got = decoded.drums.filter((d) => d.drum === drum).map((d) => d.tick).sort((x, y) => x - y);
    if (want.length !== got.length || want.some((t, i) => t !== got[i])) {
      out.push({ check: 'parity', message: `drum ${drum}: the stream hits ${got.length} times, the manifest ${want.length}` });
    }
  }
  return out;
}
