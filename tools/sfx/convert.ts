// Converts one Undertone sound effect into an OPL2 register chunk and a PC speaker divisor list for the DOS build.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from `convertSfx` in `m4/sound/fizz-dos/sfx.ts` of ScottMorris/lieutenant-fizz-dos at commit 221dd6c, the
// same author's code, MIT there and dual-licensed here. What stayed: the frame model (both outputs run on the Keen effect clock
// of 140 frames a second, the OPL writes land on 560 Hz ticks inside each frame), the voice-length and level curves, the noise
// handling (a noise voice is a jittering pitch on a buzzy patch, or a random divisor on the speaker), the seeded generators and
// the speaker thinning of quiet frames. What changed, because a cluck is layered chains of overlapping short notes rather than
// a jingle's separate notes:
//
// - the note frequency comes from `assets/sfx/voice.ts` (no Undertone import), and an exact `hz` on a voice wins over its name;
// - the voice heard on a tie is the louder one, not the two in turn (taking turns would retrigger the patch every frame);
// - a voice that begins while the previous pitched voice is still sounding, on the same patch, continues it with a pitch move
//   and no key-off and key-on, so a chain of short notes is one gliding note;
// - the pitch between two chained notes is interpolated per frame (the chip moved its pitch every 1/140 s), so a fall is a
//   sweep and not a staircase of 25 ms steps;
// - the level of a frame is the loudest live voice of the heard kind, so overlapping notes do not flutter the level;
// - the output is records of register, value and wait (`OplRecord`) and a plain divisor list, not IMF bytes.

import { STEP, isNoise, voiceHz, type SfxVoice } from '../../assets/sfx/voice.ts';
import {
  EFFECT_CHANNEL,
  EFFECT_RATE,
  IMF_RATE,
  Stream,
  attenuation,
  keyOff,
  setLevel,
  setPatch,
  setPitch,
  toRecords,
  type OplRecord,
  type Patch,
} from './opl.ts';
import { PATCHES, patchForEffect } from './patches.ts';

/** The PC speaker's timer clock, so a divisor is 1193180 / frequency. */
export const PIT_CLOCK = 1_193_180;

const NOISE_HZ: Record<string, number> = { white: 3500, pink: 1400, brown: 520 };

interface Segment {
  voice: SfxVoice;
  index: number;
  start: number;
  /** Seconds the voice is heard: its attack and decay, and half the release. */
  length: number;
  hz: number;
  noise: boolean;
}

export interface Conversion {
  seconds: number;
  opl: { records: OplRecord[]; ticks: number };
  /** One PIT channel 2 divisor per 1/140 s frame; 0 is gate off. The last entry is always 0. */
  speaker: { divisors: number[] };
}

function segmentsOf(voices: readonly SfxVoice[]): Segment[] {
  return voices.map((voice, index) => ({
    voice,
    index,
    start: voice.nudge ?? 0,
    length: (voice.a ?? 0.001) + (voice.d ?? 0.1) + (voice.r ?? 0.05) * 0.5,
    hz: isNoise(voice.n) ? (NOISE_HZ[voice.n] ?? 2000) : voiceHz(voice),
    noise: isNoise(voice.n),
  }));
}

/** How loud a voice is `t` seconds into its own life, as a share of its gain. */
function level(seg: Segment, t: number): number {
  const a = seg.voice.a ?? 0.001;
  const d = seg.voice.d ?? 0.1;
  if (t < a) return seg.voice.g * (t / a);
  const decay = Math.max(0, 1 - (t - a) / (d + (seg.voice.r ?? 0.05) * 0.5));
  return seg.voice.g * decay * decay;
}

function seeded(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

function liveAt(segments: Segment[], t: number): Segment[] {
  return segments.filter((g) => t >= g.start && t < g.start + g.length && level(g, t - g.start) > 0.002);
}

/** The voice heard at a time: the one that started last; on a tie, the louder (then the earlier in the list). */
function heard(segments: Segment[], t: number): Segment | undefined {
  const live = liveAt(segments, t);
  if (!live.length) return undefined;
  return live.reduce((best, g) =>
    g.start > best.start || (g.start === best.start && g.voice.g > best.voice.g) ? g : best,
  );
}

/** The loudest live voice of the same kind (noise or pitched) as `seg`, so overlapping notes keep one steady level. */
function loudness(segments: Segment[], t: number, seg: Segment): number {
  return Math.max(...liveAt(segments, t).filter((g) => g.noise === seg.noise).map((g) => level(g, t - g.start)));
}

/** The next note of the same chain: the next voice on the same wave that starts within one chain step. */
function successor(segments: Segment[], seg: Segment): Segment | undefined {
  if (seg.noise) return undefined;
  return segments
    .filter(
      (g) =>
        !g.noise &&
        g.voice.w === seg.voice.w &&
        g.start > seg.start &&
        g.start - seg.start <= STEP * 1.2 &&
        g.start < seg.start + seg.length,
    )
    .sort((a, b) => a.start - b.start)[0];
}

/** The pitch of a voice at a time, moving toward the next note of its chain so a chain sweeps. */
function pitch(segments: Segment[], seg: Segment, t: number): number {
  const next = successor(segments, seg);
  if (!next) return seg.hz;
  const u = Math.min(1, Math.max(0, (t - seg.start) / (next.start - seg.start)));
  return seg.hz * (next.hz / seg.hz) ** u;
}

/** Converts one effect. Both outputs run on the Keen effect clock of 140 frames a second. */
export function convertSfx(voices: readonly SfxVoice[]): Conversion {
  const segments = segmentsOf(voices);
  const seconds = Math.max(...segments.map((g) => g.start + g.length));
  const frames = Math.ceil(seconds * EFFECT_RATE) + 1;
  const loudest = Math.max(...voices.map((v) => v.g));
  const ticksPerFrame = IMF_RATE / EFFECT_RATE;

  // ---- OPL2: one channel, a patch for the voice being heard, its level and pitch moved every frame ----
  const s = new Stream();
  s.at(0, 0x01, 0x20);
  const rand = seeded(2026);
  let patchOn: Patch | undefined;
  let sounding: Segment | undefined;
  let lastHz = 440;
  let lastTl = -1;
  for (let frame = 0; frame < frames; frame++) {
    const t = frame / EFFECT_RATE;
    const at = frame * ticksPerFrame;
    const seg = heard(segments, t);
    if (!seg) {
      if (sounding) keyOff(s, at, EFFECT_CHANNEL, lastHz);
      sounding = undefined;
      continue;
    }
    const tl = attenuation(loudness(segments, t, seg), loudest);
    const hz = seg.noise ? seg.hz * (0.55 + 0.9 * rand()) : pitch(segments, seg, t);
    const patch = seg.noise ? PATCHES.noise : patchForEffect(seg.voice.w);
    const continues =
      sounding !== undefined &&
      !seg.noise &&
      !sounding.noise &&
      patchOn === patch &&
      seg.start < sounding.start + sounding.length;
    const started = sounding?.index !== seg.index && !continues && !(seg.noise && sounding?.noise);
    if (started && sounding) keyOff(s, at, EFFECT_CHANNEL, lastHz);
    if (patchOn !== patch) {
      setPatch(s, at + (started && sounding ? 1 : 0), EFFECT_CHANNEL, patch, tl);
      patchOn = patch;
    } else if (tl !== lastTl) {
      setLevel(s, at + 1, EFFECT_CHANNEL, patch, tl);
    }
    setPitch(s, at + 2, EFFECT_CHANNEL, hz, true);
    sounding = seg;
    lastHz = hz;
    lastTl = tl;
  }
  if (sounding) keyOff(s, frames * ticksPerFrame, EFFECT_CHANNEL, lastHz);
  const ticks = frames * ticksPerFrame + 8;

  // ---- PC speaker: one square voice, a divisor a frame; noise is a random pitch, and quiet is thinned out ----
  const divisors: number[] = new Array<number>(frames).fill(0);
  const noiseRand = seeded(7);
  for (let frame = 0; frame < frames; frame++) {
    const t = frame / EFFECT_RATE;
    const seg = heard(segments, t);
    if (!seg) continue;
    const share = loudness(segments, t, seg) / loudest;
    if (share < 0.1 || (share < 0.3 && frame % 2 === 1)) continue;
    const hz = seg.noise ? 250 + noiseRand() * (seg.hz * 1.6) : pitch(segments, seg, t);
    divisors[frame] = Math.round(PIT_CLOCK / Math.min(Math.max(hz, 60), 9000));
  }
  let used = divisors.length;
  while (used > 1 && divisors[used - 1] === 0) used--;
  const trimmed = divisors.slice(0, used);
  trimmed.push(0);
  return { seconds, opl: { records: toRecords(s, ticks), ticks }, speaker: { divisors: trimmed } };
}
