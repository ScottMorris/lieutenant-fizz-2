// Turns the parts of a built tune into Undertone patterns: the stereo mix, the 7.1 mix, and the seeded variations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: `partVoice` follows `buildPartVoice` in web/music.ts (the game's Web music player, itself from Fizz 1's `buildVoice`),
// with the same envelope, filter, pan make-up and partner mirroring. What the lab adds: a part may carry its own pan pattern, accents,
// FM, a phaser and a crossing echo; every voice is humanised with Undertone's `drift`; and in the 7.1 mix each note's pan becomes a
// position on the speaker ring. Pure pattern building: nothing here touches an audio context, so Bun can run it.

import * as U from '@liminal-hq/undertone';
import { Pattern, drift, surroundGains, type ControlPatch, type SoundType } from '@liminal-hq/undertone';
import { ARP_STEPS, PART_PAN, type PartRole } from '../../assets/music/types.ts';
import { humanOf, humanSeed, type BuiltSection, type BuiltTune } from './build.ts';
import type { LabPart, LabTune } from './tunes.ts';

export type Voice = Pattern<ControlPatch>;
export type Mix = 'stereo' | 'surround';

export interface VoiceOptions {
  mix: Mix;
  /** Whether the seeded variations are on. Off is the same notes dead on the grid, for comparison. */
  human: boolean;
}

/** Make-up for a panned part, as in the game's player: a mono voice through a stereo panner at centre is 3 dB down. */
const PANNED_MAKEUP = Math.SQRT2;

/** Applies `fn` to every pan in a number or a mini-notation string of pans, leaving counts such as `*2` and `@3` alone. */
const mapPan = (pan: number | string, fn: (p: number) => number): number | string => {
  const fixed = (p: number): string => String(Math.round(fn(p) * 1000) / 1000);
  return typeof pan === 'number'
    ? fn(pan)
    : pan.replace(/-?(\d+\.?\d*|\.\d+)/g, (n, _d, at: number, all: string) => (/[@*!/]/.test(all[at - 1] ?? '') ? n : fixed(Number(n))));
};
const negate = (pan: number | string): number | string => mapPan(pan, (p) => (p === 0 ? 0 : -p));

/** How far a partner voice leans the other way when its part moves about the field: enough to widen the note, not to cancel the move. */
export const PARTNER_LEAN = -0.35;

/**
 * The pan of a part: its own if it has one, else its role's (the arpeggio alternates). A partner of a fixed part takes the mirror
 * image, as in the game; a partner of a moving part leans a third as far the other way, so the pair still travels together.
 */
export function panOf(part: LabPart): number | string | undefined {
  const mirrored = part.mirror !== undefined;
  if (part.pan !== undefined) return mirrored ? mapPan(part.pan, (p) => (p === 0 ? 0 : p * PARTNER_LEAN)) : part.pan;
  const rolePan = part.role ? PART_PAN[part.role] : 0;
  if (typeof rolePan !== 'number') {
    const [a, b] = mirrored ? [rolePan[1], rolePan[0]] : rolePan;
    return `[${a} ${b}]*${ARP_STEPS / 2}`;
  }
  if (mirrored) return rolePan === 0 ? part.mirror : -rolePan;
  return rolePan === 0 ? undefined : rolePan;
}

/** One voice for a part, before any variation or 7.1 placement. `echo` is the tune's echo time in seconds. */
export function partVoice(part: LabPart, tune: LabTune): Voice {
  const echo = (tune.echoBeats * 60) / tune.track.bpm;
  let p: Voice = part.noise ? U.sound(part.notes as SoundType) : U.note(part.notes).sound(part.w ?? 'triangle');
  const pan = panOf(part);
  p = p
    .attack(part.a ?? 0.001)
    .decay(part.d ?? 0.1)
    .sustain(part.s ?? 0.3)
    .release(part.r ?? 0.05)
    .gain(part.g * (pan === undefined ? 1 : PANNED_MAKEUP));
  if (part.accent) p = p.velocity(part.accent);
  if (pan !== undefined) p = p.pan(pan);
  if (part.lpf) p = p.lpf(part.lpf);
  if (part.lpenv) {
    p = p
      .lpenv(part.lpenv)
      .lpa(part.lpa ?? 0.001)
      .lpd(part.lpd ?? 0.1)
      .lps(part.lps ?? 0)
      .lpr(part.lpr ?? 0.05);
  }
  if (part.hpf) p = p.hpf(part.hpf);
  if (part.slide) p = p.slide(part.slide);
  if (part.fm) {
    p = p.fm(part.fm).fmh(part.fmh ?? 2);
    if (part.fmdecay) p = p.fmdecay(part.fmdecay);
  }
  if (part.phaser) p = p.phaser(part.phaser);
  if (part.nudge) p = p.nudge(part.nudge);
  if (part.room) p = p.room(part.room).roomsize(tune.roomSize).orbit(1);
  if (part.delay) p = p.delay(part.delay).delaytime(echo).delayfeedback(0.35).orbit(part.room ? 1 : 2);
  return p;
}

// ---------- The 7.1 placement ----------

/** Where a role sits on the speaker ring (degrees, 0 front, positive to the right) and how far a full pan moves it. */
const RING: Record<PartRole, { at: number; spread: number }> = {
  lead: { at: 0, spread: 60 },
  counter: { at: 180, spread: -70 },
  bell: { at: 180, spread: 50 },
  arp: { at: 0, spread: 370 },
  hats: { at: 20, spread: 100 },
  perc: { at: 0, spread: 110 },
  bass: { at: 0, spread: 0 },
  kick: { at: 0, spread: 0 },
  snare: { at: 0, spread: 0 },
  pad: { at: 0, spread: 0 },
  drone: { at: 0, spread: 0 },
};

/** Fixed beds, in the order FL, FR, C, LFE, SL, SR, RL, RR: the low end in front with the subwoofer, the pads around the room. */
export const BED: Partial<Record<PartRole, number[]>> = {
  bass: [0.6, 0.6, 0.35, 0.5, 0, 0, 0, 0],
  kick: [0.4, 0.4, 0.7, 0.7, 0, 0, 0, 0],
  snare: [0.45, 0.45, 0.65, 0, 0.12, 0.12, 0, 0],
  pad: [0.35, 0.35, 0, 0, 0.75, 0.75, 0.6, 0.6],
  drone: [0.35, 0.35, 0.3, 0.4, 0.4, 0.4, 0.4, 0.4],
};

const wrapDegrees = (angle: number): number => ((((angle + 180) % 360) + 360) % 360) - 180;

/**
 * The speaker gains of one note in the 7.1 mix. `pan` is the note's stereo pan and `bar` its position in bars from the start of
 * its section. A tune with `spinBars` sends its lead once round the ring in that many bars, its partner directly opposite.
 */
export function ringGains(part: LabPart, tune: LabTune, pan: number, bar: number): number[] {
  const role = part.role ?? 'lead';
  const bed = BED[role];
  if (bed) return bed;
  if (role === 'lead' && tune.spinBars) {
    const turn = (bar / tune.spinBars) * 360 + (part.mirror !== undefined ? 180 : 0);
    return surroundGains(wrapDegrees(turn));
  }
  const { at, spread } = RING[role];
  return surroundGains(wrapDegrees(at + pan * spread));
}

function placed(voice: Voice, part: LabPart, tune: LabTune): Voice {
  return new Pattern<ControlPatch>((span) =>
    voice.query(span).map((hap) => {
      if (!hap.whole) return hap;
      const value: ControlPatch = { ...hap.value };
      const pan = value.pan ?? 0;
      delete value.pan;
      value.channelGains = ringGains(part, tune, pan, hap.whole.begin.toNumber());
      return { ...hap, value };
    }),
  );
}

// ---------- Sections and songs ----------

/** The voices one part contributes: itself, and its crossing echo if it has one. */
function voicesOf(part: LabPart, tune: LabTune): { part: LabPart; voice: Voice }[] {
  const out = [{ part, voice: partVoice(part, tune) }];
  if (part.cross && part.mirror === undefined) {
    const echo = (tune.echoBeats * 60) / tune.track.bpm;
    const copy: LabPart = { ...part, g: part.g * part.cross, pan: negate(panOf(part) ?? 0.4), delay: 0, room: (part.room ?? 0) * 0.6 };
    out.push({ part: copy, voice: partVoice(copy, tune).late(echo) });
  }
  return out;
}

/**
 * One section as a pattern that repeats every `cycles` bars. Query bar `b + pass * cycles` for bar `b` of its `pass`th playing:
 * the notes are the same every pass and the variations are different every pass, the same way each time.
 */
export function sectionPattern(
  built: BuiltTune,
  section: BuiltSection,
  options: VoiceOptions,
  keep: (part: LabPart) => boolean = () => true,
): Voice {
  const voices = section.parts
    .filter(({ part }) => keep(part))
    .flatMap(({ stream, part }) =>
      voicesOf(part, built.tune).map(({ part: voiced, voice }, layer) => {
        let v = U.arrange([section.cycles, voice]) as Voice;
        if (options.human) {
          v = drift(v, { ...humanOf(voiced), seed: `${humanSeed(built.tune, section.name, stream)}|${layer}`, cyclesPerPass: section.cycles });
        }
        return options.mix === 'surround' ? placed(v, voiced, built.tune) : v;
      }),
    );
  return voices.length ? (U.stack(...voices) as Voice) : (U.silence as unknown as Voice);
}

/** The whole song as one pattern, a section after another, for an offline render. */
export function songPattern(built: BuiltTune, options: VoiceOptions): Voice {
  return U.arrange(...built.sections.map((section): [number, Voice] => [section.cycles, sectionPattern(built, section, options)])) as Voice;
}
