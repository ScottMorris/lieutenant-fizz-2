// The byte format of a music register stream, and the hash the Rust tests and the DOS check compare it by.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A stream is a list of 4-byte records in the layout of `fizz_sfx::OplStep`: the register's low byte, the value, then a little-endian
// u16 whose top bit is the register bank (0 for registers 0x000 to 0x0ff, 1 for 0x100 to 0x1ff) and whose low 15 bits are the number
// of 560 Hz ticks to wait after the write. `toRecords` in `tools/sfx/opl.ts` makes the same records for the cluck.

import type { OplRecord } from '../sfx/opl.ts';

/** Bytes in one record. */
export const RECORD_BYTES = 4;
/** The largest wait a record can hold, in 560 Hz ticks. */
export const MAX_WAIT = 0x7fff;

export function encode(records: readonly OplRecord[]): Uint8Array {
  const out = new Uint8Array(records.length * RECORD_BYTES);
  records.forEach((r, i) => {
    if (r.wait < 0 || r.wait > MAX_WAIT) throw new Error(`record ${i}: wait ${r.wait} does not fit 15 bits`);
    if (r.bank !== 0 && r.bank !== 1) throw new Error(`record ${i}: bank ${r.bank}`);
    const packed = (r.bank << 15) | r.wait;
    out.set([r.reg & 0xff, r.val & 0xff, packed & 0xff, packed >> 8], i * RECORD_BYTES);
  });
  return out;
}

export function decode(bytes: Uint8Array): OplRecord[] {
  if (bytes.length % RECORD_BYTES !== 0) throw new Error('a record stream is a whole number of 4-byte records');
  const out: OplRecord[] = [];
  for (let i = 0; i < bytes.length; i += RECORD_BYTES) {
    const packed = bytes[i + 2]! | (bytes[i + 3]! << 8);
    out.push({ reg: bytes[i]!, val: bytes[i + 1]!, bank: packed >> 15, wait: packed & MAX_WAIT });
  }
  return out;
}

/** The tick (from the start of the stream, one pass) at which each record's write is due. */
export function ticksOf(records: readonly OplRecord[]): number[] {
  const ticks: number[] = [];
  let t = 0;
  for (const r of records) {
    ticks.push(t);
    t += r.wait;
  }
  return ticks;
}

/**
 * The 32-bit FNV-1a hash of one pass of a stream (the prelude and one loop): for each record in order, the bytes of its tick
 * (little-endian u32, counted from the first record), its bank, its register and its value. Integer-only, so the DOS build can
 * compute it in an interrupt handler.
 */
export function fnv1a(records: readonly OplRecord[]): number {
  let h = 0x811c9dc5;
  const feed = (b: number): void => {
    h = Math.imul(h ^ (b & 0xff), 0x01000193) >>> 0;
  };
  ticksOf(records).forEach((tick, i) => {
    const r = records[i]!;
    feed(tick);
    feed(tick >>> 8);
    feed(tick >>> 16);
    feed(tick >>> 24);
    feed(r.bank);
    feed(r.reg);
    feed(r.val);
  });
  return h >>> 0;
}

/** The most records due at any one tick of a pass. */
export function maxWritesPerTick(records: readonly OplRecord[]): number {
  const counts = new Map<number, number>();
  for (const t of ticksOf(records)) counts.set(t, (counts.get(t) ?? 0) + 1);
  return Math.max(0, ...counts.values());
}
