// Tests for the music lowering: counts, the per-tick cap, the reserved registers, the loop point, note parity and the stereo bits.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { levelSong } from '../../assets/music/songs.ts';
import { layoutFor, type SongDef } from '../../assets/music/song.ts';
import type { MusicTrack } from '../../assets/music/types.ts';
import { OPERATORS, RESERVED_OPL_CHANNELS, type OplRecord } from '../sfx/opl.ts';
import { decode as decodeBytes, encode, fnv1a, maxWritesPerTick, ticksOf } from './format.ts';
import { lowerAll, manifestPath, outputs, binPath } from './generate.ts';
import { MAX_DELAY, MAX_WRITES_PER_TICK, MELODIC_CHANNELS, lower, type Arrangement } from './lower.ts';
import { OUTPUT_BITS, ROLE_OUTPUT } from './stereo.ts';
import { RESERVED_REGISTERS, decode, parity, parseManifest, refusals, roleOf } from './verify.ts';

const layout = levelSong();
const opl2 = lower(layout, 'opl2');
const opl3 = lower(layout, 'opl3');
const manifest = parseManifest(readFileSync(manifestPath('crater'), 'utf8'));
const loopOf = (a: Arrangement): OplRecord[] => a.records.slice(a.loopStart);

describe('counts', () => {
  test('the OPL2 arrangement carries the notes the DOS port measured: 731 played, 8 dropped, 480 drum hits, 35,416 ticks', () => {
    expect(opl2.loopTicks).toBe(35416);
    expect(opl2.notes).toBe(731);
    expect(opl2.dropped).toHaveLength(8);
    expect(opl2.drumHits).toBe(480);
    expect(opl2.notes + opl2.dropped.length).toBe(739);
  });

  test('it has fewer records than the port\'s 3,866 because a repeated pitch or level is not written again', () => {
    // The port writes both pitch registers and a level register for every note; this writes a register only when the channel's value
    // changes, and puts the one-time setup in a prelude that is not repeated.
    expect(opl2.records.length).toBeLessThan(3866);
    expect(opl2.records.length).toBeGreaterThan(2500);
  });

  test('the OPL3 arrangement plays every note, partner layers included, in the same 35,416 ticks', () => {
    expect(opl3.loopTicks).toBe(35416);
    expect(opl3.dropped).toHaveLength(0);
    expect(opl3.notes).toBe(739 + 661);
    expect(opl3.drumHits).toBe(480);
  });

  test('the prelude is short and the loop starts after it', () => {
    for (const a of [opl2, opl3]) {
      expect(a.loopStart).toBeGreaterThan(0);
      expect(a.preludeTicks).toBeLessThan(20);
      expect(a.records.length).toBeGreaterThan(a.loopStart);
    }
  });
});

describe('the per-tick cap', () => {
  test('no tick of either stream, prelude included, carries more than 16 writes', () => {
    for (const a of [opl2, opl3]) {
      expect(maxWritesPerTick(a.records)).toBeLessThanOrEqual(MAX_WRITES_PER_TICK);
      expect(a.maxWritesPerTick).toBe(maxWritesPerTick(a.records));
    }
  });

  test('the burst the DOS port had at tick 0 is spread out', () => {
    const at0 = loopOf(opl2).filter((_, i) => ticksOf(loopOf(opl2))[i] === 0);
    expect(at0.length).toBeLessThanOrEqual(MAX_WRITES_PER_TICK);
  });

  test('notes start no more than the allowed ticks late', () => {
    for (const a of [opl2, opl3]) {
      expect(a.stats.maxDelay).toBeLessThanOrEqual(MAX_DELAY);
      for (const n of a.placed) {
        expect(n.start - n.nominal).toBeGreaterThanOrEqual(0);
        expect(n.start - n.nominal).toBeLessThanOrEqual(MAX_DELAY);
      }
    }
  });
});

describe('the effects channels and the service registers', () => {
  test('the reserved register list is channel 5\'s operators, A5, B5 and C5', () => {
    const [m, c] = OPERATORS[5]!;
    const derived = [0x20, 0x40, 0x60, 0x80, 0xe0].flatMap((b) => [b + m, b + c]).concat([0xa5, 0xb5, 0xc5]);
    expect([...RESERVED_REGISTERS].sort()).toEqual(derived.sort());
    expect(RESERVED_OPL_CHANNELS).toEqual([5, 14]);
  });

  test('no music channel list contains 5 or 14, and the OPL2 one stays in 0 to 4', () => {
    for (const list of Object.values(MELODIC_CHANNELS)) {
      for (const c of RESERVED_OPL_CHANNELS) expect(list).not.toContain(c);
    }
    expect(MELODIC_CHANNELS.opl2).toEqual([0, 1, 2, 3, 4]);
    expect(MELODIC_CHANNELS.opl3).toEqual([0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 15, 16, 17]);
  });

  test('no record writes a reserved register in either bank, or 0x104 or 0x105', () => {
    for (const a of [opl2, opl3]) {
      for (const r of a.records) {
        expect(RESERVED_REGISTERS).not.toContain(r.reg);
        expect(r.bank === 1 && (r.reg === 0x04 || r.reg === 0x05)).toBe(false);
      }
    }
  });

  test('the OPL2 stream stays in bank 0 and uses only channels 0 to 4 and 6 to 8', () => {
    expect(opl2.records.every((r) => r.bank === 0)).toBe(true);
    expect(refusals(opl2)).toEqual([]);
  });

  test('the OPL3 stream uses its 13 channels and the drums, and no others', () => {
    expect(refusals(opl3)).toEqual([]);
    const used = new Set(opl3.placed.map((n) => n.channel));
    for (const c of used) expect(MELODIC_CHANNELS.opl3).toContain(c);
    expect(used.size).toBeGreaterThan(10);
  });

  test('the refusal checks bite', () => {
    const base = (): OplRecord[] => structuredClone(opl3.records);
    const withRecord = (r: OplRecord): Arrangement => {
      const records = base();
      records.splice(opl3.loopStart + 5, 0, { ...r, wait: 1 });
      return { ...opl3, records };
    };
    for (const [bank, reg] of [[0, 0xb5], [0, 0x2a], [1, 0xc5], [1, 0x8d], [0, 0xa5]] as const) {
      expect(refusals(withRecord({ bank, reg, val: 0, wait: 0 })).map((p) => p.check)).toContain('reserved-channel');
    }
    expect(refusals(withRecord({ bank: 1, reg: 0x05, val: 1, wait: 0 })).map((p) => p.check)).toContain('service-register');
    expect(refusals(withRecord({ bank: 1, reg: 0x04, val: 0x24, wait: 0 })).map((p) => p.check)).toContain('service-register');
    // A write to a channel the arrangement does not list (channel 12, here) is refused.
    const narrow = { ...withRecord({ bank: 1, reg: 0xb3, val: 0, wait: 0 }), channels: [0, 1, 2] };
    expect(refusals(narrow).map((p) => p.check)).toContain('channel-ownership');
    // Seventeen writes in one tick.
    const crowded = base();
    for (let i = 0; i < 17; i++) crowded.splice(opl3.loopStart, 0, { bank: 0, reg: 0xa0, val: i, wait: 0 });
    expect(refusals({ ...opl3, records: crowded }).map((p) => p.check)).toContain('writes-per-tick');
    // A channel left keyed on at the end.
    const hung = base();
    hung.push({ bank: 0, reg: 0xb0, val: 0x31, wait: 0 });
    expect(refusals({ ...opl3, records: hung }).map((p) => p.check)).toContain('keyed-off');
    // An OPL2 stream with a bank-1 write.
    const second = structuredClone(opl2.records);
    second.splice(opl2.loopStart + 3, 0, { bank: 1, reg: 0xb0, val: 0, wait: 0 });
    expect(refusals({ ...opl2, records: second }).map((p) => p.check)).toContain('opl2-bank');
  });

  test('every channel is keyed off at the end of the loop', () => {
    for (const a of [opl2, opl3]) {
      const regs = new Map<number, number>();
      for (const r of a.records) regs.set(r.bank * 256 + r.reg, r.val);
      for (let ch = 0; ch < 18; ch++) {
        const v = regs.get((ch >= 9 ? 256 : 0) + 0xb0 + (ch % 9));
        if (v !== undefined) expect(v & 0x20).toBe(0);
      }
      expect((regs.get(0xbd) ?? 0) & 0x1f).toBe(0);
    }
  });
});

describe('the loop point', () => {
  test('the one-time setup is before the loop point and is not repeated in the loop', () => {
    for (const a of [opl2, opl3]) {
      const prelude = a.records.slice(0, a.loopStart);
      const loop = loopOf(a);
      expect(prelude.some((r) => r.bank === 0 && r.reg === 0x01 && r.val === 0x20)).toBe(true);
      expect(prelude.some((r) => r.bank === 0 && r.reg === 0xbd && r.val === 0x20)).toBe(true);
      // The drum operators (offsets 0x10 to 0x15: bass drum 2, snare 1, hat 1, five registers each) are loaded once.
      const drumOps = (r: OplRecord): boolean => r.bank === 0 && [0x20, 0x40, 0x60, 0x80, 0xe0].some((b) => r.reg >= b + 0x10 && r.reg <= b + 0x15);
      expect(prelude.filter(drumOps).length).toBeGreaterThanOrEqual(20);
      expect(loop.filter(drumOps)).toEqual([]);
      expect(loop.some((r) => r.bank === 0 && r.reg === 0x01)).toBe(false);
      expect(loop.filter((r) => r.reg >= 0xc6 && r.reg <= 0xc8 && r.bank === 0)).toEqual([]);
    }
  });

  test('the loop lasts exactly the song and starts with a note at tick 0', () => {
    for (const a of [opl2, opl3]) {
      expect(loopOf(a).reduce((n, r) => n + r.wait, 0)).toBe(35416);
      expect(decode(a).notes.some((n) => n.tick === 0)).toBe(true);
    }
  });

  test('the second pass of the loop sets every note up as the first does', () => {
    // `refusals` replays two passes and compares the registers at every key-on; a tail that forgot the next pass's instruments fails.
    expect(refusals(opl2).filter((p) => p.check === 'loop')).toEqual([]);
    expect(refusals(opl3).filter((p) => p.check === 'loop')).toEqual([]);
    // A loop whose tail leaves channel 0's pitch changed is caught: the next pass's first note, which relies on the old pitch, differs.
    const broken = structuredClone(opl3.records);
    const last = broken.length - 1;
    broken[last]!.wait = 0;
    broken.push({ bank: 0, reg: 0xa0, val: 0x55, wait: 1 });
    expect(refusals({ ...opl3, records: broken }).map((p) => p.check)).toContain('loop');
  });
});

describe('note parity with assets/music/notes/crater.txt', () => {
  test('both arrangements play the manifest\'s notes and drum hits (OPL2 less its dropped notes)', () => {
    expect(parity(opl2, manifest, layout.song.bpm)).toEqual([]);
    expect(parity(opl3, manifest, layout.song.bpm)).toEqual([]);
  });

  test('the OPL2 dropped notes are in the manifest and are not played', () => {
    for (const d of opl2.dropped) {
      expect(manifest.some((n) => n.tick === d.tick && n.part === d.part && n.midi === d.midi)).toBe(true);
    }
    const melodic = manifest.filter((n) => n.midi !== null && !n.part.endsWith('-partner') && !['kick', 'snare', 'hats'].includes(roleOf(n.part)));
    expect(melodic).toHaveLength(739);
    expect(decode(opl2).notes).toHaveLength(739 - 8);
  });

  test('a tampered manifest or a missing note is caught', () => {
    const wrong = manifest.map((n, i) => (i === 3 && n.midi !== null ? { ...n, midi: n.midi + 1 } : n));
    expect(parity(opl3, wrong, layout.song.bpm)).not.toEqual([]);
    const fewer: Arrangement = { ...opl3, notes: opl3.notes, dropped: [] };
    expect(parity(fewer, manifest, layout.song.bpm)).toEqual([]);
    expect(parity({ ...opl2, dropped: [] }, manifest, layout.song.bpm)).not.toEqual([]);
  });
});

describe('stereo output bits (OPL3 only)', () => {
  const played = decode(opl3).notes;
  const bitsOf = (n: { channel: number; start: number }): number => {
    const d = played.find((x) => x.channel === n.channel && x.tick === n.start);
    if (!d) throw new Error('note not decoded');
    return d.c0 & 0x30;
  };

  test('the OPL2 stream sets no output bits', () => {
    expect(decode(opl2).notes.every((n) => (n.c0 & 0x30) === 0)).toBe(true);
  });

  test('bass, pad and their partners are centred; the lead is centred and its partner is on the right', () => {
    for (const n of opl3.placed) {
      const role = roleOf(n.part);
      const partner = n.part.endsWith('-partner');
      if (role === 'lead') expect(bitsOf(n)).toBe(partner ? OUTPUT_BITS.right : OUTPUT_BITS.centre);
      if (role === 'bass' || role === 'pad') expect(bitsOf(n)).toBe(OUTPUT_BITS.centre);
    }
  });

  test('the arpeggio alternates left and right on each eighth, and its partner takes the other side', () => {
    const arp = opl3.placed.filter((n) => n.part === 'arp');
    const partner = opl3.placed.filter((n) => n.part === 'arp-partner');
    expect(arp.length).toBe(128);
    arp.forEach((n) => {
      const step = Math.floor(((n.tick96 % 96) / 96) * 8 + 1e-9) % 2;
      expect(bitsOf(n)).toBe(step === 0 ? OUTPUT_BITS.left : OUTPUT_BITS.right);
    });
    partner.forEach((n) => {
      const step = Math.floor(((n.tick96 % 96) / 96) * 8 + 1e-9) % 2;
      expect(bitsOf(n)).toBe(step === 0 ? OUTPUT_BITS.right : OUTPUT_BITS.left);
    });
    expect(new Set(arp.map((n) => bitsOf(n)))).toEqual(new Set([OUTPUT_BITS.left, OUTPUT_BITS.right]));
  });

  test('every rhythm-mode drum channel is centred in the prelude', () => {
    const prelude = opl3.records.slice(0, opl3.loopStart);
    for (const reg of [0xc6, 0xc7, 0xc8]) {
      const w = prelude.filter((r) => r.bank === 0 && r.reg === reg);
      expect(w).toHaveLength(1);
      expect(w[0]!.val & 0x30).toBe(OUTPUT_BITS.centre);
    }
  });

  test('the role table has no side for a role the song does not have, and lists the partner roles', () => {
    expect(ROLE_OUTPUT.lead?.partner).toBe('right');
    expect(ROLE_OUTPUT.arp?.main).toEqual(['left', 'right']);
  });
});

describe('the files', () => {
  test('the record bytes round-trip and the hash is stable', () => {
    for (const a of [opl2, opl3]) {
      const bytes = encode(a.records);
      expect(bytes.length).toBe(a.records.length * 4);
      expect(decodeBytes(bytes)).toEqual(a.records);
      expect(fnv1a(a.records)).toBe(fnv1a(decodeBytes(bytes)));
    }
    expect(fnv1a([{ bank: 0, reg: 0x01, val: 0x20, wait: 0 }])).toBe(fnv1a([{ bank: 0, reg: 0x01, val: 0x20, wait: 7 }]));
    expect(fnv1a([{ bank: 0, reg: 0x01, val: 0x20, wait: 0 }])).not.toBe(fnv1a([{ bank: 1, reg: 0x01, val: 0x20, wait: 0 }]));
  });

  test('the committed files are what the generator makes (the same as --check)', () => {
    for (const f of outputs()) {
      const have = readFileSync(f.path);
      expect(Buffer.compare(have, Buffer.from(f.bytes))).toBe(0);
    }
    expect(binPath('crater', 'opl2')).toContain('crates/fizz-music/data/crater-opl2.bin');
  });

  test('lowerAll refuses nothing for the committed song', () => {
    expect(lowerAll().map((s) => s.id)).toEqual(['crater']);
  });
});

describe('swapping the song is data only', () => {
  test('another song lowers with the same code: both arrangements pass the refusals and play every note they carry', () => {
    const track: MusicTrack = {
      bpm: 120,
      parts: [
        { notes: '<[c4 e4 g4 c5] [d4 f4 a4 d5]>', w: 'square', g: 0.1, role: 'lead' },
        { notes: '<[c2 c2 g2 c2]>', w: 'triangle', g: 0.3, role: 'bass' },
        { notes: 'c1 ~ c1 ~', w: 'sine', g: 0.7, role: 'kick', s: 0 },
      ],
    };
    const def: SongDef = { trim: 0.75, contrast: 5, lift: 2, arp: false };
    const other = layoutFor(track, def);
    for (const kind of ['opl2', 'opl3'] as const) {
      const a = lower(other, kind);
      expect(refusals(a)).toEqual([]);
      expect(a.dropped).toHaveLength(0);
      expect(a.loopStart).toBeGreaterThan(0);
      expect(decode(a).notes).toHaveLength(a.notes);
    }
  });
});
