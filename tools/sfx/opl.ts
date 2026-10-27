// OPL2 register helpers and a register-write stream for the sound-effect downmix.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from `m4/sound/fizz-dos/opl.ts` of ScottMorris/lieutenant-fizz-dos (a.k.a. fizz-dos) at commit 221dd6c,
// the same author's code, MIT there and dual-licensed here. What changed: the music-only IMF byte writer (`toImf`, with its
// 16-bit length header) is replaced by `toRecords`, which returns plain `{ reg, val, wait }` records so the Rust crate stores
// them without a byte parser; the register arithmetic is otherwise unchanged. A record carries a bank bit so registers 0x100 and up
// (the OPL3's second bank) are expressible, after the owner's IMF lab (`src/fizz-dos/opl.ts`: `bankOf`, `OPL3_ENABLE` and the five-byte
// `toOpl3` record); the lab's files are not copied.

/** The IMF clock the Keen games use: register writes are timed in 1/560 s. */
export const IMF_RATE = 560;
/** The effect clock of the Keen AdLib and PC-speaker effects. */
export const EFFECT_RATE = 140;
/** The OPL2's sample clock in Hz. */
export const OPL_CLOCK = 49_716;

/** The OPL channel sound effects use, in bank 0: unpanned effects and plain OPL2 cards. Scores must not use it. */
export const EFFECT_CHANNEL = 5;
/**
 * The OPL3 channel a panned effect uses: 14 is slot 5 of the second register bank (9 + 5), so its registers are 0x1a5, 0x1b5 and
 * 0x1c5 and its operators sit at the same offsets as channel 5's, 0x0a and 0x0d, plus 0x100. Scores must not use it either. Two
 * effects panned at once would need a third reserved channel; that is not done.
 */
export const EFFECT_CHANNEL_PANNED = 14;
/** The channels (0 to 17) that effects own; a score-writing tool imports this list to refuse them. Mirrors `fizz_sfx::RESERVED_OPL_CHANNELS`. */
export const RESERVED_OPL_CHANNELS: readonly number[] = [EFFECT_CHANNEL, EFFECT_CHANNEL_PANNED];

/** The bank (0 or 1) and in-bank slot (0 to 8) of a channel numbered 0 to 17, as the owner's IMF lab's `bankOf`. */
export const bankOf = (channel: number): { bank: number; slot: number } => ({ bank: channel >= 9 ? 1 : 0, slot: channel % 9 });

/** Register offsets of the modulator and carrier operators of each of the nine channels. */
export const OPERATORS: readonly (readonly [number, number])[] = [
  [0x00, 0x03],
  [0x01, 0x04],
  [0x02, 0x05],
  [0x08, 0x0b],
  [0x09, 0x0c],
  [0x0a, 0x0d],
  [0x10, 0x13],
  [0x11, 0x14],
  [0x12, 0x15],
];

export interface Operator {
  /** Frequency multiple, as the 4-bit register value (0 is 0.5, 1 is 1, 2 is 2). */
  mult: number;
  /** Total level, 0 (loudest) to 63 in 0.75 dB steps. */
  tl: number;
  /** Key scale level, 0 to 3. */
  ksl?: number;
  /** Attack rate 0 to 15 (15 is instant). */
  ar: number;
  /** Decay rate 0 to 15. */
  dr: number;
  /** Sustain level 0 (loudest) to 15 (silent). */
  sl: number;
  /** Release rate 0 to 15. */
  rr: number;
  /** Hold at the sustain level while the key is down instead of decaying through it. */
  sustain?: boolean;
  /** Waveform select: 0 sine, 1 half sine, 2 absolute sine, 3 quarter-sine pulses. */
  wave?: number;
  vibrato?: boolean;
  tremolo?: boolean;
}

export interface Patch {
  name: string;
  mod: Operator;
  car: Operator;
  /** Modulator feedback, 0 to 7. */
  feedback: number;
  /** 0 is FM (the modulator drives the carrier); 1 adds both operators. */
  additive?: boolean;
}

/** A register write at a time, in IMF ticks. */
export interface Write {
  tick: number;
  register: number;
  value: number;
}

/** One register write and the ticks to wait before the next, as the Rust crate stores it. `bank` is 1 for registers 0x100 to 0x1ff (OPL3). */
export interface OplRecord {
  bank: number;
  reg: number;
  val: number;
  wait: number;
}

export class Stream {
  readonly writes: Write[] = [];
  at(tick: number, register: number, value: number): void {
    this.writes.push({ tick: Math.max(0, Math.round(tick)), register, value: value & 0xff });
  }
}

/** The OPL F-number and block that make `hz`, with the F-number kept as high as it goes for resolution. */
export function fnumber(hz: number): { fnum: number; block: number } {
  const safe = Math.min(Math.max(hz, 16), 6000);
  for (let block = 0; block < 8; block++) {
    const fnum = Math.round((safe * 2 ** (20 - block)) / OPL_CLOCK);
    if (fnum <= 1023) return { fnum, block };
  }
  return { fnum: 1023, block: 7 };
}

const operatorRegisters = (op: Operator) => ({
  r20: (op.tremolo ? 0x80 : 0) | (op.vibrato ? 0x40 : 0) | (op.sustain ? 0x20 : 0) | (op.mult & 0x0f),
  r40: ((op.ksl ?? 0) << 6) | (op.tl & 0x3f),
  r60: ((op.ar & 0x0f) << 4) | (op.dr & 0x0f),
  r80: ((op.sl & 0x0f) << 4) | (op.rr & 0x0f),
  re0: (op.wave ?? 0) & 3,
});

/** Writes a patch's registers to a channel, with `extraTl` (steps of 0.75 dB) added to the carrier. */
export function setPatch(s: Stream, tick: number, channel: number, patch: Patch, extraTl = 0): void {
  const [mod, car] = OPERATORS[channel];
  const m = operatorRegisters(patch.mod);
  const c = operatorRegisters(patch.car);
  s.at(tick, 0x20 + mod, m.r20);
  s.at(tick, 0x20 + car, c.r20);
  s.at(tick, 0x40 + mod, m.r40);
  s.at(tick, 0x40 + car, (c.r40 & 0xc0) | Math.min(63, (c.r40 & 0x3f) + extraTl));
  s.at(tick, 0x60 + mod, m.r60);
  s.at(tick, 0x60 + car, c.r60);
  s.at(tick, 0x80 + mod, m.r80);
  s.at(tick, 0x80 + car, c.r80);
  s.at(tick, 0xe0 + mod, m.re0);
  s.at(tick, 0xe0 + car, c.re0);
  s.at(tick, 0xc0 + channel, ((patch.feedback & 7) << 1) | (patch.additive ? 1 : 0));
}

/** Sets only the carrier level of a channel (the note's volume). */
export function setLevel(s: Stream, tick: number, channel: number, patch: Patch, extraTl: number): void {
  const car = OPERATORS[channel][1];
  s.at(tick, 0x40 + car, ((patch.car.ksl ?? 0) << 6) | Math.min(63, patch.car.tl + extraTl));
}

/** Sets the pitch of a channel and its key bit. */
export function setPitch(s: Stream, tick: number, channel: number, hz: number, keyOn: boolean): void {
  const { fnum, block } = fnumber(hz);
  s.at(tick, 0xa0 + channel, fnum & 0xff);
  s.at(tick, 0xb0 + channel, (keyOn ? 0x20 : 0) | (block << 2) | (fnum >> 8));
}

/** Releases the key of a channel, leaving the pitch bits as they are. */
export function keyOff(s: Stream, tick: number, channel: number, hz: number): void {
  const { fnum, block } = fnumber(hz);
  s.at(tick, 0xb0 + channel, (block << 2) | (fnum >> 8));
}

/** Attenuation in 0.75 dB steps for a gain relative to a reference gain. */
export function attenuation(gain: number, reference: number): number {
  if (gain <= 0) return 63;
  return Math.max(0, Math.min(63, Math.round((20 * Math.log10(reference / gain)) / 0.75)));
}

/**
 * The writes as time-ordered records (stable for equal ticks), each with the ticks to wait before the next; the last record
 * waits until `totalTicks`, so a player knows when the effect is over.
 */
export function toRecords(s: Stream, totalTicks: number): OplRecord[] {
  const writes = s.writes
    .map((w, order) => ({ ...w, order }))
    .sort((a, b) => a.tick - b.tick || a.order - b.order);
  const at = (i: number): number => Math.min(writes[i].tick, totalTicks);
  return writes.map((w, i) => {
    const next = i + 1 < writes.length ? at(i + 1) : totalTicks;
    const wait = Math.max(0, next - at(i));
    if (wait > 0x7fff) throw new Error('an effect gap does not fit 15 bits');
    return { bank: (w.register >> 8) & 1, reg: w.register & 0xff, val: w.value, wait };
  });
}
