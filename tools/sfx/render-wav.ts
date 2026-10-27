// Renders the cluck variants to WAV files for listening: the rich source, an approximate OPL2 preview and the PC speaker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/sfx/render-wav.ts <output-directory>
//
// A listening aid only; nothing in the build or the checks uses it, and its output is not committed. None of the three renderers is
// the real thing, and all three are approximate:
//
// - rich: Undertone renders through Web Audio, which Bun does not have, and its engine takes an `AudioContext`, so this is a small
//   synth of our own that follows Undertone's voice model (oscillator or noise, a linear attack, a linear decay to a sustain of 0,
//   a low-pass with a filter envelope). Waves are naive, not band-limited. The Web will sound like Undertone, not like this.
// - opl2: a two-operator FM emulation of channel 5 driven by the generated register records (so it plays exactly the data DOS will
//   get), with the OPL2's feedback and level steps but a rough envelope-rate table and no key scaling, tremolo or vibrato. A real
//   chip or DOSBox's emulator will differ.
// - speaker: a square wave at the divisor's frequency, held for each 1/140 s step. A real speaker's cone and a case colour it.

import { mkdirSync, writeFileSync } from 'node:fs';
import { EFFECTS } from '../../assets/sfx/cluck.ts';
import { isNoise, voiceHz, type SfxVoice } from '../../assets/sfx/voice.ts';
import type { Conversion } from './convert.ts';
import { convertAll } from './downmix.ts';
import { OPL_CLOCK, IMF_RATE, EFFECT_RATE, type OplRecord } from './opl.ts';

const SR = 44100;

/** A small seeded generator so every render is the same. */
function seeded(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

function wav(samples: Float32Array, rate: number): Uint8Array {
  const out = new Uint8Array(44 + samples.length * 2);
  const v = new DataView(out.buffer);
  const tag = (o: number, t: string): void => [...t].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  tag(0, 'RIFF');
  v.setUint32(4, 36 + samples.length * 2, true);
  tag(8, 'WAVE');
  tag(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  tag(36, 'data');
  v.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    v.setInt16(44 + i * 2, Math.round(Math.max(-1, Math.min(1, samples[i])) * 32767), true);
  }
  return out;
}

// ---- rich ----

function noiseSample(kind: string, rand: () => number, state: number[]): number {
  const white = rand() * 2 - 1;
  if (kind === 'white') return white;
  if (kind === 'pink') {
    const [b0, b1, b2, b3, b4, b5, b6] = state;
    state[0] = 0.99886 * b0 + white * 0.0555179;
    state[1] = 0.99332 * b1 + white * 0.0750759;
    state[2] = 0.969 * b2 + white * 0.153852;
    state[3] = 0.8665 * b3 + white * 0.3104856;
    state[4] = 0.55 * b4 + white * 0.5329522;
    state[5] = -0.7616 * b5 - white * 0.016898;
    const pink = state[0] + state[1] + state[2] + state[3] + state[4] + state[5] + b6 + white * 0.5362;
    state[6] = white * 0.115926;
    return pink * 0.11;
  }
  state[0] = (state[0] + 0.02 * white) / 1.02;
  return state[0] * 3.5;
}

function oscSample(wave: string | undefined, phase: number): number {
  const p = phase - Math.floor(phase);
  switch (wave) {
    case 'sine':
      return Math.sin(2 * Math.PI * p);
    case 'square':
      return p < 0.5 ? 1 : -1;
    case 'sawtooth':
      return 2 * p - 1;
    default:
      return 4 * Math.abs(p - 0.5) - 1;
  }
}

/** A biquad low-pass (RBJ cookbook) at Q 1, as Web Audio's default filter. */
class LowPass {
  private x1 = 0;
  private x2 = 0;
  private y1 = 0;
  private y2 = 0;
  private b0 = 1;
  private b1 = 0;
  private b2 = 0;
  private a1 = 0;
  private a2 = 0;
  set(cutoff: number): void {
    const w = (2 * Math.PI * Math.min(cutoff, SR * 0.45)) / SR;
    const alpha = Math.sin(w) / 2;
    const a0 = 1 + alpha;
    this.b0 = (1 - Math.cos(w)) / 2 / a0;
    this.b1 = (1 - Math.cos(w)) / a0;
    this.b2 = this.b0;
    this.a1 = (-2 * Math.cos(w)) / a0;
    this.a2 = (1 - alpha) / a0;
  }
  run(x: number): number {
    const y = this.b0 * x + this.b1 * this.x1 + this.b2 * this.x2 - this.a1 * this.y1 - this.a2 * this.y2;
    this.x2 = this.x1;
    this.x1 = x;
    this.y2 = this.y1;
    this.y1 = y;
    return y;
  }
}

export function renderRich(voices: readonly SfxVoice[]): Float32Array {
  const end = Math.max(...voices.map((v) => (v.nudge ?? 0) + (v.a ?? 0.01) + (v.d ?? 0.1) + (v.r ?? 0.05)));
  const out = new Float32Array(Math.ceil((end + 0.05) * SR));
  const rand = seeded(1);
  for (const v of voices) {
    const a = v.a ?? 0.01;
    const d = v.d ?? 0.1;
    const start = Math.round((v.nudge ?? 0) * SR);
    const count = Math.round((a + d) * SR);
    const noise = isNoise(v.n);
    const hz = noise ? 0 : voiceHz(v);
    const nState = [0, 0, 0, 0, 0, 0, 0];
    const filter = v.lpf ? new LowPass() : undefined;
    let phase = 0;
    for (let i = 0; i < count; i++) {
      const t = i / SR;
      const env = t < a ? t / a : Math.max(0, 1 - (t - a) / d);
      if (filter && i % 16 === 0) {
        let cutoff = v.lpf!;
        if (v.lpenv) {
          const la = v.lpa ?? 0.001;
          const ld = v.lpd ?? 0.1;
          const settle = v.lps ?? 0;
          const shape = t < la ? t / la : 1 - (1 - settle) * Math.min(1, (t - la) / ld);
          cutoff = v.lpf! + (v.lpenv - v.lpf!) * shape;
        }
        filter.set(cutoff);
      }
      let x = noise ? noiseSample(v.n, rand, nState) : oscSample(v.w, phase);
      phase += hz / SR;
      if (filter) x = filter.run(x);
      const j = start + i;
      if (j < out.length) out[j] += x * env * v.g;
    }
  }
  return out;
}

// ---- opl2 ----

const MULT = [0.5, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 12, 12, 15, 15];
const FEEDBACK = [0, Math.PI / 16, Math.PI / 8, Math.PI / 4, Math.PI / 2, Math.PI, 2 * Math.PI, 4 * Math.PI];

/** Seconds for an envelope to move 96 dB at a rate 1 to 15; a rough table (each step up halves the time twice every other step). */
const seconds96 = (rate: number): number => (rate === 0 ? Infinity : 0.004 * 2 ** ((15 - rate) / 2));

class Operator {
  phase = 0;
  /** Envelope attenuation in dB: 0 is full, 96 is silent. */
  db = 96;
  state: 'off' | 'attack' | 'decay' | 'sustain' | 'release' = 'off';
  out1 = 0;
  out2 = 0;
}

/** An approximate model of OPL2 channel 5 driven by register writes. */
class Channel5 {
  regs = new Uint8Array(256);
  mod = new Operator();
  car = new Operator();
  private fnum(): { hz: number } {
    const fnum = this.regs[0xa5] | ((this.regs[0xb5] & 3) << 8);
    const block = (this.regs[0xb5] >> 2) & 7;
    return { hz: (fnum * OPL_CLOCK) / 2 ** (20 - block) };
  }
  write(reg: number, val: number): void {
    const was = (this.regs[0xb5] & 0x20) !== 0;
    this.regs[reg] = val;
    if (reg === 0xb5) {
      const now = (val & 0x20) !== 0;
      if (now && !was) {
        for (const o of [this.mod, this.car]) {
          o.state = 'attack';
          o.phase = 0;
        }
      } else if (!now && was) {
        for (const o of [this.mod, this.car]) o.state = 'release';
      }
    }
  }
  private step(o: Operator, base: number, dt: number): void {
    const r20 = this.regs[0x20 + base];
    const ar = this.regs[0x60 + base] >> 4;
    const dr = this.regs[0x60 + base] & 15;
    const sl = this.regs[0x80 + base] >> 4;
    const rr = this.regs[0x80 + base] & 15;
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
  private shape(wave: number, x: number): number {
    const s = Math.sin(x);
    switch (wave & 3) {
      case 1:
        return s > 0 ? s : 0;
      case 2:
        return Math.abs(s);
      case 3:
        return Math.sin(x) > 0 && ((x / (2 * Math.PI)) % 0.5) < 0.25 ? Math.abs(s) : 0;
      default:
        return s;
    }
  }
  sample(dt: number): number {
    const { hz } = this.fnum();
    const [mo, ca] = [0x0a, 0x0d];
    this.step(this.mod, mo, dt);
    this.step(this.car, ca, dt);
    const fbRaw = (this.regs[0xc5] >> 1) & 7;
    const mf = hz * MULT[this.regs[0x20 + mo] & 15];
    const cf = hz * MULT[this.regs[0x20 + ca] & 15];
    const m = this.mod;
    const c = this.car;
    m.phase += mf * dt;
    c.phase += cf * dt;
    const mAmp = 10 ** (-((this.regs[0x40 + mo] & 63) * 0.75 + m.db) / 20);
    const cAmp = 10 ** (-((this.regs[0x40 + ca] & 63) * 0.75 + c.db) / 20);
    const fb = FEEDBACK[fbRaw] * ((m.out1 + m.out2) / 2);
    const mOut = this.shape(this.regs[0xe0 + mo], 2 * Math.PI * m.phase + fb) * mAmp;
    m.out2 = m.out1;
    m.out1 = mOut;
    const additive = (this.regs[0xc5] & 1) !== 0;
    const cRaw = this.shape(this.regs[0xe0 + ca], 2 * Math.PI * c.phase + (additive ? 0 : mOut * 4 * Math.PI)) * cAmp;
    return additive ? (cRaw + mOut) * 0.5 : cRaw;
  }
}

export function renderOpl(records: readonly OplRecord[], ticks: number): Float32Array {
  const rate = OPL_CLOCK;
  const out = new Float32Array(Math.ceil(((ticks + 28) / IMF_RATE) * rate));
  const ch = new Channel5();
  let i = 0;
  let next = 0;
  let waitSamples = 0;
  for (let n = 0; n < out.length; n++) {
    while (waitSamples <= 0 && next < records.length) {
      const r = records[next++];
      ch.write(r.reg, r.val);
      waitSamples += (r.wait * rate) / IMF_RATE;
    }
    waitSamples -= 1;
    out[i++] = ch.sample(1 / rate) * 0.5;
  }
  return out;
}

// ---- speaker ----

export function renderSpeaker(divisors: readonly number[]): Float32Array {
  const out = new Float32Array(Math.ceil(((divisors.length + 4) / EFFECT_RATE) * SR));
  let phase = 0;
  for (let n = 0; n < out.length; n++) {
    const d = divisors[Math.floor((n / SR) * EFFECT_RATE)] ?? 0;
    if (d === 0) continue;
    phase += 1193180 / d / SR;
    out[n] = (phase - Math.floor(phase) < 0.5 ? 1 : -1) * 0.25;
  }
  return out;
}

if (import.meta.main) {
  const dir = process.argv[2];
  if (!dir) {
    console.error('usage: bun tools/sfx/render-wav.ts <output-directory>');
    process.exit(2);
  }
  mkdirSync(dir, { recursive: true });
  const conv: Conversion[] = convertAll();
  EFFECTS.cluck.forEach((voices, i) => {
    const files: [string, Uint8Array][] = [
      [`cluck-${i}-rich.wav`, wav(renderRich(voices), SR)],
      [`cluck-${i}-opl2.wav`, wav(renderOpl(conv[i].opl.records, conv[i].opl.ticks), OPL_CLOCK)],
      [`cluck-${i}-speaker.wav`, wav(renderSpeaker(conv[i].speaker.divisors), SR)],
    ];
    for (const [name, bytes] of files) {
      writeFileSync(`${dir}/${name}`, bytes);
      console.log(`${dir}/${name}  ${bytes.length} bytes`);
    }
  });
}
