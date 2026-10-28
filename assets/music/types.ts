// The Undertone music vocabulary the Fizz 2 song source is written in: parts, tracks, songs, mix roles and their stereo placement.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `MusicPart`, `MusicTrack` and `MusicSong` follow `packages/engine/src/audio.ts` (lines 69-118), and `PartRole`,
// `PartPan`, `PART_PAN`, `ARP_STEPS` and `partPanAt` are `packages/engine/src/sound-field.ts` (lines 82-128), of
// ~/source/liminal-hq/lieutenant-fizz at commit 4bce036, the same author's code, Apache-2.0 OR MIT there and here. What changed:
// only the imports; the Web Audio specific `undertonePan` and `partMakeup` are not carried. Nothing here touches a browser or
// imports Undertone, so Bun and the browser bundle both run it.

/** An oscillator shape (the Web Audio `OscillatorType` names the parts use). */
export type Wave = 'sine' | 'triangle' | 'square' | 'sawtooth';

/** What a music part does in the mix, which decides where it sits in the stereo field. */
export type PartRole =
  | 'bass'
  | 'kick'
  | 'snare'
  | 'pad'
  | 'drone'
  | 'lead'
  | 'counter'
  | 'bell'
  | 'hats'
  | 'perc'
  | 'arp';

/** A fixed pan, or a pair that alternates on each step of the cycle (ping-pong). */
export type PartPan = number | readonly [number, number];

/**
 * A filter envelope on `lpf`: it opens to `lpenv` Hz at the attack (`lpa` seconds), falls to `lps` of the way back over `lpd`, and
 * closes over `lpr` after the note.
 */
export interface FilterEnvelope {
  lpenv?: number;
  lpa?: number;
  lpd?: number;
  lps?: number;
  lpr?: number;
}

/** One part of a looping music track (a mini-notation pattern plus voice settings). */
export interface MusicPart extends FilterEnvelope {
  notes: string;
  w?: Wave;
  noise?: boolean;
  a?: number;
  d?: number;
  s?: number;
  r?: number;
  g: number;
  lpf?: number;
  hpf?: number;
  slide?: number;
  room?: number;
  delay?: number;
  /** What the part does in the mix; it places the part in the stereo field. */
  role?: PartRole;
  /** Seconds the part starts after the beat. */
  nudge?: number;
  /** Makes the part a partner layer that pans opposite its role (this is the pan it takes when the role is centred). */
  mirror?: number;
}

/** A looping track: a handful of parts that repeat every few bars. */
export interface MusicTrack {
  bpm: number;
  parts: MusicPart[];
}

/**
 * A longer arrangement of a track, in sections. Each stream is one voice across the sections: the part it plays in each, or null
 * where it is silent. The song loops from its last section to its first, at `bpm`, so its length is the total of `cycles` (a cycle,
 * or bar, is four beats).
 */
export interface MusicSong {
  bpm: number;
  /** The length of each section in cycles. */
  cycles: number[];
  streams: (MusicPart | null)[][];
}

/**
 * Where each role sits. Low and central things (bass, kick, snare, pads, drones) stay in the middle; the rest lean a little, never
 * past 0.3, so the field is felt more than heard and the music still reads on one speaker. The arpeggio alternates left and right
 * on every eighth of the cycle.
 */
export const PART_PAN: Record<PartRole, PartPan> = {
  bass: 0,
  kick: 0,
  snare: 0,
  pad: 0,
  drone: 0,
  lead: -0.15,
  counter: 0.2,
  bell: 0.3,
  hats: 0.25,
  perc: -0.3,
  arp: [-0.3, 0.3],
};

/** How many steps of an alternating pan fit in one cycle. */
export const ARP_STEPS = 8;

/** The pan of a part at `cyclePos`, the position in its cycle from 0 up to (not including) 1. */
export function partPanAt(role: PartRole, cyclePos: number): number {
  const p = PART_PAN[role];
  if (typeof p === 'number') return p;
  const step = Math.floor(cyclePos * ARP_STEPS + 1e-9);
  return p[((step % 2) + 2) % 2]!;
}
