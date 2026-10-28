// "Crater Fields": a PLACEHOLDER test song for Fizz 2's test level. It is Fizz 1's level track, not an official Fizz 2 track.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the track is `crater` in `MUSIC` (`episodes/episode-1/src/audio/patterns.ts`, lines 375-399, with the defaults `P()` at
// 198-207 and the `kick`, `snare` and `hats` helpers at 208-215) and the settings are `crater` in `DEFS`
// (`episodes/episode-1/src/audio/songs.ts`, line 17), of ~/source/liminal-hq/lieutenant-fizz at commit 4bce036. The DOS port's
// copy ("Crater Run", `m4/sound/fizz-songs.ts` lines 304-317 of ScottMorris/lieutenant-fizz-dos at commit 221dd6c) is the same
// definition. The same author's code, Apache-2.0 OR MIT there and here; the notes and voice settings are copied unchanged.
//
// A driving A-minor run-and-jump, 148 bpm. The owner will replace it with a Fizz 2 song: swap `assets/music/songs.ts` to point at
// the new track and nothing else changes. Nobody has listened to it as Fizz 2's music.

import type { SongDef } from './song.ts';
import type { MusicPart, MusicTrack } from './types.ts';

/** The title this placeholder is called in code and docs. */
export const CRATER_TITLE = 'Crater Fields';

// Fizz 1's helpers: a part with a triangle voice and a plain envelope, overridden per part.
const P = (notes: string, o: Partial<MusicPart> = {}): MusicPart => ({
  notes,
  w: 'triangle',
  a: 0.005,
  d: 0.12,
  s: 0.3,
  r: 0.08,
  g: 0.2,
  ...o,
});
const kick = (pat: string, g = 0.7): MusicPart => P(pat, { role: 'kick', w: 'sine', d: 0.12, s: 0, r: 0.06, g, slide: 0.09 });
const hats = (pat: string, g = 0.1): MusicPart => P(pat, { role: 'hats', noise: true, d: 0.03, s: 0, r: 0.01, g, hpf: 6000 });
const snare = (pat: string, g = 0.28): MusicPart => P(pat, { role: 'snare', noise: true, d: 0.09, s: 0, r: 0.05, g, hpf: 1600 });

/** The looping track: eight bars of lead over four bars of bass and pad, with a kick, snare and hats bar. */
export const CRATER: MusicTrack = {
  bpm: 148,
  parts: [
    P(
      '<[a4 ~ c5 e5 a5 ~ g5 e5] [f5 ~ e5 c5 a4 ~ c5 e5] [d5 ~ f5 a5 g5 f5 e5 d5] [e5 ~ b4 ~ g#4 ~ b4 ~] [a4 ~ c5 e5 a5 ~ b5 c6] [b5 ~ a5 g5 f5 ~ e5 f5] [d5 e5 f5 g5 a5 g5 f5 d5] [e5 ~ ~ ~ e4 ~ ~ ~]>',
      { role: 'lead', w: 'square', g: 0.07, lpf: 2300, d: 0.09 },
    ),
    P(
      '<[a2 a2 a3 a2 a2 a2 a3 g2] [f2 f2 f3 f2 f2 f2 f3 e2] [d2 d2 d3 d2 d2 d2 d3 c2] [e2 e2 e3 e2 e2 e2 e3 g#2]>',
      { role: 'bass', g: 0.3, s: 0.35, d: 0.08 },
    ),
    P('<[c4,e4] [a3,c4] [f3,a3] [g#3,b3]>', { role: 'pad', w: 'square', a: 0.05, s: 0.6, g: 0.025, lpf: 900 }),
    kick('c1 ~ ~ c1 c1 ~ ~ ~'),
    snare('~ white ~ white'),
    hats('white*8'),
  ],
};

/** How the builder lays the track out as a song: Fizz 1's `crater` entry in `songs.ts`. */
export const CRATER_DEF: SongDef = { trim: 0.75, contrast: 5, lift: 2, arp: true };
