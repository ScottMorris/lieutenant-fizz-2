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
// - opl3-pair-left, -centre, -right, -distant: the OPL3 pair, from the register writes of the real Rust player (the `stream` example of
//   `fizz-sfx`, which needs cargo) at a source on the view's left edge, middle, right edge and three half-views right, played through
//   two copies of the one-channel model (channel 5 on the left output, channel 14 on the right), written as stereo. Approximate in the
//   same ways. The left and right edges are the hardest pans Enhanced reaches (Fizz 1's width is 0.6: the far side is about -10 dB).
// - opl2-distant: channel 5 with the distance attenuation of a source at the distance floor (11 steps, -8 dB), mono.
// - rich-distant: the rich cluck with the Web's equal-power pan (0.6) and gain (0.4), as stereo.
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

function wav(samples: Float32Array, rate: number, channels = 1): Uint8Array {
  const out = new Uint8Array(44 + samples.length * 2);
  const v = new DataView(out.buffer);
  const tag = (o: number, t: string): void => [...t].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  tag(0, 'RIFF');
  v.setUint32(4, 36 + samples.length * 2, true);
  tag(8, 'WAVE');
  tag(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, channels, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2 * channels, true);
  v.setUint16(32, 2 * channels, true);
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
  /** Set by a write of 1 to register 0x105; the output bits in register 0xc5 then gate the two sides. */
  opl3 = false;
  /** The model has one channel, so bank 1's slot 5 (channel 14) and bank 0's channel 5 share its registers; only the enable is special. */
  writeBanked(bank: number, reg: number, val: number): void {
    if (bank === 1 && reg === 0x05) {
      this.opl3 = (val & 1) !== 0;
      return;
    }
    this.write(reg, val);
  }
  /** The left and right gains of the channel: both on an OPL2, and per the C0 output bits on an OPL3. */
  sides(): [number, number] {
    if (!this.opl3) return [1, 1];
    const bits = this.regs[0xc5];
    return [bits & 0x10 ? 1 : 0, bits & 0x20 ? 1 : 0];
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

/** One register write from the Rust player: the 560 Hz tick, the bank, the register and the value. */
export type Write = readonly [tick: number, bank: number, reg: number, val: number];

/** The writes the DOS audio service makes for variant `variant`, from the real player (the `stream` example of `fizz-sfx`). */
export function streamWrites(variant: number, ...args: (string | number)[]): Write[] {
  const run = Bun.spawnSync(['cargo', 'run', '-q', '-p', 'fizz-sfx', '--example', 'stream', '--', String(variant), ...args.map(String)], {
    stderr: 'inherit',
  });
  if (run.exitCode !== 0) throw new Error('the stream example failed');
  return run.stdout
    .toString()
    .trim()
    .split('\n')
    .map((l) => l.split(' ').map(Number) as unknown as Write);
}

/**
 * The OPL3 pair as the Rust player plays it: the writes of the stream, bank 0 to the left channel (5) and bank 1 to the right (14),
 * each with its own output bit and carrier level already in the values. Two copies of the one-channel model run side by side, the
 * left one heard only where its C0 register says left (0x10) and the right one where it says right (0x20). Returns interleaved
 * stereo at the chip's rate. Approximate in the same ways as the OPL2 model.
 */
export function renderOplPair(writes: readonly Write[], ticks: number): Float32Array {
  const rate = OPL_CLOCK;
  const frames = Math.ceil(((ticks + 28) / IMF_RATE) * rate);
  const out = new Float32Array(frames * 2);
  const chips = [new Channel5(), new Channel5()];
  chips.forEach((c) => (c.opl3 = true));
  let next = 0;
  for (let n = 0; n < frames; n++) {
    while (next < writes.length && (writes[next][0] / IMF_RATE) * rate <= n) {
      const [, bank, reg, val] = writes[next++];
      chips[bank === 0 ? 0 : 1].write(reg, val);
    }
    for (const c of chips) {
      const x = c.sample(1 / rate) * 0.5;
      const [l, r] = c.sides();
      out[n * 2] += x * l;
      out[n * 2 + 1] += x * r;
    }
  }
  return out;
}

/** The OPL2 channel 5 with the distance attenuation: the stream of `OplPlayer::mono`, as mono at the chip's rate. */
export function renderOplMono(writes: readonly Write[], ticks: number): Float32Array {
  const rate = OPL_CLOCK;
  const out = new Float32Array(Math.ceil(((ticks + 28) / IMF_RATE) * rate));
  const ch = new Channel5();
  let next = 0;
  for (let n = 0; n < out.length; n++) {
    while (next < writes.length && (writes[next][0] / IMF_RATE) * rate <= n) {
      const [, , reg, val] = writes[next++];
      ch.write(reg, val);
    }
    out[n] = ch.sample(1 / rate) * 0.5;
  }
  return out;
}

/** The rich cluck placed as the Web places it: an equal-power pan and a gain on the whole sound, as stereo at 44.1 kHz. */
export function renderRichPlaced(voices: readonly SfxVoice[], pan: number, gain: number): Float32Array {
  const mono = renderRich(voices);
  const a = ((Math.max(-1, Math.min(1, pan)) + 1) * Math.PI) / 4;
  const out = new Float32Array(mono.length * 2);
  mono.forEach((x, i) => {
    out[i * 2] = x * gain * Math.cos(a);
    out[i * 2 + 1] = x * gain * Math.sin(a);
  });
  return out;
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
      ch.writeBanked(r.bank, r.reg, r.val);
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
  // Where the source stands in a 320 by 200 view whose top left is (0, 0): a pixel x and y for `place`. Hard left and right are the
  // view's edges, which is as far as Enhanced pans (Fizz 1's width 0.6); the distant source is three half-views to the right,
  // which is the floor of the distance factor (0.4) at the full pan.
  const spots: Record<string, number> = { left: 0, centre: 160, right: 320, distant: 160 + 3 * 160 };
  const view = [0, 0, 320, 200];
  const write = (name: string, bytes: Uint8Array): void => {
    writeFileSync(`${dir}/${name}`, bytes);
    console.log(`${dir}/${name}  ${bytes.length} bytes`);
  };
  EFFECTS.cluck.forEach((voices, i) => {
    const ticks = conv[i].opl.ticks;
    write(`cluck-${i}-rich.wav`, wav(renderRich(voices), SR));
    write(`cluck-${i}-opl2.wav`, wav(renderOpl(conv[i].opl.records, ticks), OPL_CLOCK));
    write(`cluck-${i}-speaker.wav`, wav(renderSpeaker(conv[i].speaker.divisors), SR));
    for (const [spot, x] of Object.entries(spots)) {
      write(`cluck-${i}-opl3-pair-${spot}.wav`, wav(renderOplPair(streamWrites(i, 'at', x, 100, ...view), ticks), OPL_CLOCK, 2));
    }
    // One distant source on the single-channel and the rich paths: the OPL2's distance attenuation, and the Web's pan and gain.
    write(`cluck-${i}-opl2-distant.wav`, wav(renderOplMono(streamWrites(i, 'mono', 11), ticks), OPL_CLOCK));
    write(`cluck-${i}-rich-distant.wav`, wav(renderRichPlaced(voices, 0.6, 0.4), SR, 2));
  });
}
