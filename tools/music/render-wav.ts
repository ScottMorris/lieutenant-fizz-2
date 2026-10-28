// Renders the first minute of each music arrangement to a WAV file for listening, through an approximate multi-channel FM model.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/music/render-wav.ts <output-directory> [seconds]
//
// A listening aid only; nothing in the build or the checks uses it, and its output is not committed. It is the two-operator model of
// `tools/sfx/render-wav.ts` (that file's envelope table, multiples and feedback steps, reused here) widened to the chip's 18 channels in
// two register banks: every channel has its own registers, key bit, output bits and level. Approximate in the same ways: a rough
// envelope-rate table, no key scaling, tremolo or vibrato, naive (not band-limited) arithmetic. The rhythm-mode drums are cruder still:
// the bass drum is channel 6 keyed by its 0xBD bit, and the snare, hi-hat, tom and cymbal are each one operator's envelope over noise
// or a sine, not the chip's own noise generator and phase tricks. A real chip or an emulator will differ, and nobody has listened to
// this as Fizz 2's music.
//
// The OPL2 clip is mono; the OPL3 clip is stereo from the C0 to C8 output bits (the OPL3 enable, register 0x105, is the audio
// service's and is assumed on, as it is when the music plays).

import { mkdirSync, writeFileSync } from 'node:fs';
import { levelSong } from '../../assets/music/songs.ts';
import { IMF_RATE, OPERATORS, OPL_CLOCK } from '../sfx/opl.ts';
import { FEEDBACK, MULT, seconds96, seeded, wav } from '../sfx/render-wav.ts';
import { lower, type Arrangement } from './lower.ts';

class Op {
  phase = 0;
  /** Envelope attenuation in dB: 0 is full, 96 is silent. */
  db = 96;
  state: 'off' | 'attack' | 'decay' | 'sustain' | 'release' = 'off';
  out1 = 0;
  out2 = 0;
  key(on: boolean): void {
    if (on) {
      this.state = 'attack';
      this.phase = 0;
    } else if (this.state !== 'off') this.state = 'release';
  }
  get live(): boolean {
    return this.state !== 'off' && !(this.state === 'release' && this.db >= 95);
  }
}

const shape = (wave: number, x: number): number => {
  const s = Math.sin(x);
  switch (wave & 3) {
    case 1:
      return s > 0 ? s : 0;
    case 2:
      return Math.abs(s);
    case 3:
      return s > 0 && (x / (2 * Math.PI)) % 0.5 < 0.25 ? Math.abs(s) : 0;
    default:
      return s;
  }
};

/** The chip: 18 two-operator channels, the rhythm-mode drums and the output bits. */
class Chip {
  regs = [new Uint8Array(256), new Uint8Array(256)];
  mod = Array.from({ length: 18 }, () => new Op());
  car = Array.from({ length: 18 }, () => new Op());
  /** The drums' single operators: hi-hat, snare, tom, cymbal. */
  drum = { hat: new Op(), snare: new Op(), tom: new Op(), cymbal: new Op() };
  rand = seeded(7);
  last = 0;
  constructor(readonly stereo: boolean) {}

  private rhythm(): boolean {
    return (this.regs[0]![0xbd]! & 0x20) !== 0;
  }

  write(bank: number, reg: number, val: number): void {
    const was = this.regs[bank]![reg]!;
    this.regs[bank]![reg] = val;
    if (reg >= 0xb0 && reg <= 0xb8) {
      const ch = reg - 0xb0 + bank * 9;
      const on = (val & 0x20) !== 0;
      if (on !== ((was & 0x20) !== 0)) {
        this.mod[ch]!.key(on);
        this.car[ch]!.key(on);
      }
    } else if (bank === 0 && reg === 0xbd) {
      const edge = (bit: number): boolean | null => ((val ^ was) & bit ? (val & bit) !== 0 : null);
      const bd = edge(0x10);
      if (bd !== null) {
        this.mod[6]!.key(bd);
        this.car[6]!.key(bd);
      }
      const hat = edge(0x01);
      const cym = edge(0x02);
      const tom = edge(0x04);
      const snare = edge(0x08);
      if (hat !== null) this.drum.hat.key(hat);
      if (cym !== null) this.drum.cymbal.key(cym);
      if (tom !== null) this.drum.tom.key(tom);
      if (snare !== null) this.drum.snare.key(snare);
    }
  }

  private step(o: Op, bank: number, base: number, dt: number): void {
    const r = this.regs[bank]!;
    const r20 = r[0x20 + base]!;
    const ar = r[0x60 + base]! >> 4;
    const dr = r[0x60 + base]! & 15;
    const sl = r[0x80 + base]! >> 4;
    const rr = r[0x80 + base]! & 15;
    const slDb = sl === 15 ? 93 : sl * 3;
    switch (o.state) {
      case 'attack':
        if (ar === 15) o.db = 0;
        else o.db = Math.max(0, o.db - (96 * dt) / Math.max(1e-4, seconds96(ar) * 0.4));
        if (o.db <= 0) o.state = 'decay';
        break;
      case 'decay':
        o.db = Math.min(slDb, o.db + (96 * dt) / seconds96(dr));
        if (o.db >= slDb) o.state = 'sustain';
        break;
      case 'sustain':
        if (!(r20 & 0x20)) o.db = Math.min(96, o.db + (96 * dt) / seconds96(rr));
        break;
      case 'release':
        o.db = Math.min(96, o.db + (96 * dt) / seconds96(rr));
        break;
      default:
        break;
    }
  }

  private hz(bank: number, slot: number): number {
    const r = this.regs[bank]!;
    const fnum = r[0xa0 + slot]! | ((r[0xb0 + slot]! & 3) << 8);
    return (fnum * OPL_CLOCK) / 2 ** (20 - ((r[0xb0 + slot]! >> 2) & 7));
  }

  /** One sample of a two-operator channel. */
  private voice(ch: number, dt: number): number {
    const bank = ch >= 9 ? 1 : 0;
    const slot = ch % 9;
    const [mo, ca] = OPERATORS[slot]!;
    const r = this.regs[bank]!;
    const m = this.mod[ch]!;
    const c = this.car[ch]!;
    this.step(m, bank, mo, dt);
    this.step(c, bank, ca, dt);
    if (!c.live && !m.live) return 0;
    const hz = this.hz(bank, slot);
    m.phase += hz * MULT[r[0x20 + mo]! & 15]! * dt;
    c.phase += hz * MULT[r[0x20 + ca]! & 15]! * dt;
    const mAmp = 10 ** (-((r[0x40 + mo]! & 63) * 0.75 + m.db) / 20);
    const cAmp = 10 ** (-((r[0x40 + ca]! & 63) * 0.75 + c.db) / 20);
    const fb = FEEDBACK[(r[0xc0 + slot]! >> 1) & 7]! * ((m.out1 + m.out2) / 2);
    const mOut = shape(r[0xe0 + mo]!, 2 * Math.PI * m.phase + fb) * mAmp;
    m.out2 = m.out1;
    m.out1 = mOut;
    const additive = (r[0xc0 + slot]! & 1) !== 0;
    const cRaw = shape(r[0xe0 + ca]!, 2 * Math.PI * c.phase + (additive ? 0 : mOut * 4 * Math.PI)) * cAmp;
    return additive ? (cRaw + mOut) * 0.5 : cRaw;
  }

  /** One drum operator over noise (or a sine for the tom). */
  private drumVoice(o: Op, offset: number, kind: 'noise' | 'tone' | 'hiss', slot: number, dt: number): number {
    this.step(o, 0, offset, dt);
    if (!o.live) return 0;
    const amp = 10 ** (-((this.regs[0]![0x40 + offset]! & 63) * 0.75 + o.db) / 20);
    const white = this.rand() * 2 - 1;
    let x: number;
    if (kind === 'tone') {
      o.phase += this.hz(0, slot) * MULT[this.regs[0]![0x20 + offset]! & 15]! * dt;
      x = Math.sin(2 * Math.PI * o.phase);
    } else if (kind === 'hiss') {
      x = white - o.out1;
      o.out1 = white;
    } else x = white;
    return x * amp;
  }

  /** The next sample as (left, right). */
  sample(dt: number): [number, number] {
    let l = 0;
    let r = 0;
    const add = (ch: number, x: number): void => {
      if (x === 0) return;
      if (!this.stereo) {
        l += x;
        r += x;
        return;
      }
      const bits = this.regs[ch >= 9 ? 1 : 0]![0xc0 + (ch % 9)]!;
      if (bits & 0x10) l += x;
      if (bits & 0x20) r += x;
    };
    const rhythm = this.rhythm();
    for (let ch = 0; ch < 18; ch++) {
      if (rhythm && ch >= 6 && ch <= 8) continue;
      add(ch, this.voice(ch, dt));
    }
    if (rhythm) {
      add(6, this.voice(6, dt));
      add(7, this.drumVoice(this.drum.hat, 0x11, 'hiss', 7, dt) + this.drumVoice(this.drum.snare, 0x14, 'noise', 7, dt) * 0.8);
      add(8, this.drumVoice(this.drum.tom, 0x12, 'tone', 8, dt) + this.drumVoice(this.drum.cymbal, 0x15, 'hiss', 8, dt));
    }
    return [l, r];
  }
}

export interface Rendered {
  samples: Float32Array;
  channels: 1 | 2;
  /** The loudest sample before the soft clip. */
  peak: number;
  rms: number;
}

/** Plays the stream from its first record for `seconds` of music, looping at the loop point. */
export function render(a: Arrangement, seconds: number): Rendered {
  const stereo = a.kind === 'opl3';
  const chip = new Chip(stereo);
  const frames = Math.floor(seconds * OPL_CLOCK);
  const out = new Float32Array(frames * (stereo ? 2 : 1));
  const dt = 1 / OPL_CLOCK;
  let next = 0;
  let wait = 0;
  let peak = 0;
  let sum = 0;
  for (let n = 0; n < frames; n++) {
    while (wait <= 0) {
      const rec = a.records[next]!;
      chip.write(rec.bank, rec.reg, rec.val);
      wait += (rec.wait * OPL_CLOCK) / IMF_RATE;
      next = next + 1 < a.records.length ? next + 1 : a.loopStart;
    }
    wait -= 1;
    const [l, r] = chip.sample(dt);
    // A fixed mix gain, then a soft clip so a loud chord does not wrap; the peak before the clip is reported.
    const gl = l * 0.3;
    const gr = r * 0.3;
    peak = Math.max(peak, Math.abs(gl), Math.abs(gr));
    sum += gl * gl + gr * gr;
    if (stereo) {
      out[n * 2] = Math.tanh(gl);
      out[n * 2 + 1] = Math.tanh(gr);
    } else out[n] = Math.tanh(gl);
  }
  return { samples: out, channels: stereo ? 2 : 1, peak, rms: Math.sqrt(sum / (frames * 2)) };
}

if (import.meta.main) {
  const dir = process.argv[2];
  const seconds = Number(process.argv[3] ?? 60);
  if (!dir) {
    console.error('usage: bun tools/music/render-wav.ts <output-directory> [seconds]');
    process.exit(2);
  }
  mkdirSync(dir, { recursive: true });
  const layout = levelSong();
  for (const kind of ['opl2', 'opl3'] as const) {
    const a = lower(layout, kind);
    const r = render(a, seconds);
    const file = `${dir}/${layout.id}-${kind}-${seconds}s.wav`;
    writeFileSync(file, wav(r.samples, OPL_CLOCK, r.channels));
    console.log(`${file}  ${r.channels === 2 ? 'stereo' : 'mono'}  peak before clip ${r.peak.toFixed(3)}  rms ${r.rms.toFixed(3)}`);
  }
}
