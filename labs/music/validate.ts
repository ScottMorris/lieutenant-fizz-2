// Checks a lab tune against plain music theory and against "played by a person, quietly": the lab shows the results beside each tune.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The theory checks read the tune's notes (through Undertone's own parser, as the game's note list does) and hold them to rules a
// melody teacher would recognise. The feel checks read the humanised patterns the lab actually plays and measure how far each note
// was moved. A check never listens: it can say a tune is well formed and its variations are small and unpatterned, not that it is good.

import { Fraction, arrange, drift, noteToFrequency, type ControlPatch, type Hap, type Pattern } from '@liminal-hq/undertone';
import { noteEvents } from '../../assets/music/events.ts';
import { queryPart } from '../../assets/music/pattern.ts';
import { loopCycles } from '../../assets/music/song.ts';
import { HUMAN, buildTune, humanOf, humanSeed, type BuiltTune } from './build.ts';
import { patternById, riffOf } from './patterns.ts';
import { degreeMidi, noteName, scaleClasses } from './scale.ts';
import type { LabPart, LabTune } from './tunes.ts';
import { partVoice } from './voices.ts';

export interface Check {
  id: string;
  group: 'theory' | 'feel';
  /** What the check asks, as a short statement that is true when it passes. */
  label: string;
  pass: boolean;
  /** What was measured. */
  detail: string;
}

interface Onset {
  /** Bars from the start of the loop. */
  t: number;
  midi: number;
}

const EPS = 1e-6;
const CONSONANT = new Set([0, 3, 4, 7, 8, 9]);
/** Roots that lead back to the tonic, in semitones above it: the dominant, the subtonic, the subdominant and the two supertonics. */
const TURNS_HOME = new Set([7, 10, 5, 2, 1]);
const mod12 = (n: number): number => ((n % 12) + 12) % 12;
const pct = (n: number): string => `${Math.round(n * 100)}%`;
const ms = (seconds: number): string => `${(seconds * 1000).toFixed(1)} ms`;

function onsetsOf(part: LabPart, bars: number): Onset[] {
  return queryPart(part, 0, bars)
    .filter((e) => e.midi !== undefined)
    .map((e) => ({ t: e.t0, midi: e.midi! }))
    .sort((a, b) => a.t - b.t || b.midi - a.midi);
}

/** The single line of a part: the highest note of each onset. */
function lineOf(part: LabPart, bars: number): Onset[] {
  const line: Onset[] = [];
  for (const onset of onsetsOf(part, bars)) if (!line.length || onset.t - line[line.length - 1]!.t > EPS) line.push(onset);
  return line;
}

const partOf = (tune: LabTune, role: string): LabPart | undefined => tune.track.parts.find((part) => part.role === role);

/** The bass note sounding at `t`: the latest bass onset at or before it. */
function bassAt(bass: Onset[], t: number): Onset | undefined {
  let found: Onset | undefined;
  for (const onset of bass) if (onset.t <= t + EPS) found = onset;
  return found;
}

// ---------- Theory ----------

function theory(tune: LabTune, built: BuiltTune): Check[] {
  const checks: Check[] = [];
  const add = (id: string, label: string, pass: boolean, detail: string) => checks.push({ id, group: 'theory', label, pass, detail });
  const bars = loopCycles(tune.track);
  const lead = lineOf(partOf(tune, 'lead')!, bars);
  const bass = lineOf(partOf(tune, 'bass')!, bars);

  // The riff: each motif's notes are the pattern's cells, in order.
  const motifProblems: string[] = [];
  for (const motif of tune.motifs) {
    const riff = riffOf(patternById(motif.pattern), motif.base)[motif.line];
    const want: number[] = [];
    riff.forEach((cell, column) => {
      if (cell === null) return;
      if (motif.tie && column > 0 && riff[column - 1] === cell) return;
      want.push(degreeMidi(tune.key, motif.octave, cell));
    });
    const part = partOf(tune, motif.role);
    const got = part ? lineOf(part, bars).filter((o) => o.t >= motif.bar - EPS).slice(0, want.length).map((o) => o.midi) : [];
    if (got.join() !== want.join()) motifProblems.push(`${motif.pattern} in the ${motif.role}: wanted ${want.map(noteName).join(' ')}, found ${got.map(noteName).join(' ')}`);
  }
  add(
    'motif',
    'It starts from its pattern',
    motifProblems.length === 0,
    motifProblems.length ? motifProblems.join('; ') : `${tune.motifs.map((m) => `${m.pattern} (${m.role})`).join(', ')}: every cell is a note, in order`,
  );

  // In key: the loop, and then the built song with each section's own key.
  const scale = scaleClasses(tune.key);
  const loopNotes = tune.track.parts.filter((p) => !p.noise && p.role !== 'kick').flatMap((p) => onsetsOf(p, bars));
  const outside = loopNotes.filter((o) => !scale.has(mod12(o.midi)));
  let songNotes = 0;
  let songOutside = 0;
  const events = noteEvents(built.layout);
  for (const section of built.sections) {
    const classes = scaleClasses(tune.key, section.shift);
    const from = section.startBar * 96;
    const to = from + section.cycles * 96;
    for (const e of events) {
      if (e.midi === null || e.role === 'kick' || e.tick < from || e.tick >= to) continue;
      songNotes++;
      if (!classes.has(mod12(e.midi))) songOutside++;
    }
  }
  add(
    'key',
    `Every note belongs to ${tune.keyName}`,
    outside.length === 0 && songOutside === 0,
    `${loopNotes.length - outside.length} of ${loopNotes.length} notes of the loop; ${songNotes - songOutside} of ${songNotes} in the song, each section in its own key`,
  );

  // Strong beats: where the tune lands on beat one or three, it agrees with the bass.
  const strong = lead.filter((o) => Math.abs(((o.t % 1) * 2) % 1) < EPS);
  const points = strong.flatMap((o) => {
    const under = bassAt(bass, o.t);
    return under ? [{ t: o.t, lead: o.midi, bass: under.midi, interval: mod12(o.midi - under.midi) }] : [];
  });
  const agree = points.filter((p) => CONSONANT.has(p.interval)).length;
  add(
    'strong',
    'On strong beats the tune agrees with the bass',
    points.length >= 4 && agree / points.length >= 0.85,
    `${agree} of ${points.length} landings on beats one and three are a unison, third, fifth or sixth above the bass`,
  );

  // Parallel fifths and octaves between those landings, round the loop.
  let parallels = 0;
  points.forEach((p, i) => {
    const q = points[(i + 1) % points.length]!;
    const perfect = p.interval === q.interval && (p.interval === 0 || p.interval === 7);
    const leadMove = Math.sign(q.lead - p.lead);
    const bassMove = Math.sign(q.bass - p.bass);
    if (perfect && leadMove !== 0 && leadMove === bassMove) parallels++;
  });
  add('parallels', 'No parallel fifths or octaves between tune and bass', parallels === 0, `${parallels} found across ${points.length} strong-beat landings`);

  // Leaps: nothing wider than an octave, and a wide leap is followed by a step or a turn back.
  let widest = 0;
  let wide = 0;
  let recovered = 0;
  lead.forEach((o, i) => {
    const next = lead[(i + 1) % lead.length]!;
    const after = lead[(i + 2) % lead.length]!;
    const leap = next.midi - o.midi;
    widest = Math.max(widest, Math.abs(leap));
    if (Math.abs(leap) > 7) {
      wide++;
      const then = after.midi - next.midi;
      if (Math.abs(then) <= 2 || Math.sign(then) === -Math.sign(leap)) recovered++;
    }
  });
  add(
    'leaps',
    'Leaps stay within an octave and turn back afterwards',
    widest <= 12 && (wide === 0 || recovered / wide >= 0.8),
    `widest leap ${widest} semitones; ${recovered} of ${wide} leaps over a fifth are followed by a step or a change of direction`,
  );

  // Range and crossing.
  const high = Math.max(...lead.map((o) => o.midi));
  const low = Math.min(...lead.map((o) => o.midi));
  const crossings = lead.filter((o) => (bassAt(bass, o.t)?.midi ?? -Infinity) >= o.midi).length;
  add(
    'range',
    'The tune fits two octaves and stays above the bass',
    high - low <= 24 && crossings === 0,
    `${noteName(low)} to ${noteName(high)} (${high - low} semitones); the bass crosses it ${crossings} times`,
  );

  // The loop starts on the tonic and its last bar leads back to it.
  const first = bass.find((o) => o.t < 1);
  const last = bass.find((o) => o.t >= bars - 1 - EPS);
  const firstClass = first ? mod12(first.midi - tune.key.tonic) : -1;
  const lastClass = last ? mod12(last.midi - tune.key.tonic) : -1;
  add(
    'home',
    'The loop starts at home and its last bar leads back',
    firstClass === 0 && TURNS_HOME.has(lastClass),
    `bass opens on ${first ? noteName(first.midi) : 'nothing'} and the last bar sits on ${last ? noteName(last.midi) : 'nothing'}, ${lastClass} semitones above the tonic`,
  );

  // The form: about a minute, and every onset on the game's 96-tick grid (noteEvents throws if one is not).
  add(
    'form',
    'The song is about a minute and every note is on the grid',
    built.seconds >= 45 && built.seconds <= 80,
    `${built.sections.map((s) => `${s.name} ${s.cycles}`).join(', ')}: ${built.bars} bars, ${built.seconds.toFixed(1)} s, ${events.length} notes on the 96-tick grid`,
  );
  return checks;
}

// ---------- Feel ----------

interface Moved {
  role: string;
  pitched: boolean;
  /** Seconds the note was moved, beyond its role's constant lean. */
  timing: number;
  /** The lean: seconds every note of the role is moved. */
  push: number;
  /** The note's level as a fraction of its written level, minus one. */
  level: number;
  cents: number;
  pan: number;
}

/** How every note of one pass of every section was moved, voice by voice. */
function movedNotes(built: BuiltTune, pass: number, salt = ''): Moved[][] {
  const voices: Moved[][] = [];
  for (const section of built.sections) {
    for (const { stream, part } of section.parts) {
      // As the lab plays it: the section repeats from its own first bar, so a second pass is the same notes again.
      const plain = arrange([section.cycles, partVoice(part, built.tune)]) as Pattern<ControlPatch>;
      const human = humanOf(part);
      const moved = drift(plain, { ...human, seed: `${humanSeed(built.tune, section.name, stream)}|0${salt}`, cyclesPerPass: section.cycles });
      const out: Moved[] = [];
      for (let bar = 0; bar < section.cycles; bar++) {
        const span = { begin: bar + pass * section.cycles, end: bar + pass * section.cycles + 1 };
        const a = queryHaps(plain, span.begin);
        const b = queryHaps(moved, span.begin);
        a.forEach((hap, i) => {
          const other = b[i]!;
          const hz = (v: unknown): number | undefined => (typeof v === 'number' ? v : undefined);
          const was = hap.value.pitch;
          const now = hz(other.value.pitch);
          const wasHz = typeof was === 'number' ? was : was === undefined ? undefined : hzOf(was);
          out.push({
            role: part.role ?? 'lead',
            pitched: !part.noise && part.role !== 'kick',
            timing: (other.value.nudgeTime ?? 0) - (hap.value.nudgeTime ?? 0) - human.push,
            push: human.push,
            level: (other.value.velocity ?? 1) / (hap.value.velocity ?? 1) - 1,
            cents: now !== undefined && wasHz !== undefined ? 1200 * Math.log2(now / wasHz) : 0,
            pan: (other.value.pan ?? 0) - (hap.value.pan ?? 0),
          });
        });
      }
      voices.push(out);
    }
  }
  return voices;
}

const hzOf = (pitch: string): number => noteToFrequency(pitch);

function queryHaps(pattern: Pattern<ControlPatch>, cycle: number): Hap<ControlPatch>[] {
  return pattern
    .query({ begin: new Fraction(cycle), end: new Fraction(cycle + 1) })
    .filter((hap) => hap.whole && hap.whole.begin.eq(hap.part.begin));
}

const rms = (values: number[]): number => Math.sqrt(values.reduce((sum, v) => sum + v * v, 0) / Math.max(1, values.length));
const mean = (values: number[]): number => values.reduce((sum, v) => sum + v, 0) / Math.max(1, values.length);

/** The lag-one autocorrelation of a series: near zero when one value says nothing about the next. */
function lagOne(values: number[]): number {
  if (values.length < 3) return 0;
  const m = mean(values);
  let top = 0;
  let bottom = 0;
  for (let i = 0; i < values.length; i++) {
    bottom += (values[i]! - m) ** 2;
    if (i + 1 < values.length) top += (values[i]! - m) * (values[i + 1]! - m);
  }
  return bottom === 0 ? 0 : top / bottom;
}

function feel(built: BuiltTune): Check[] {
  const checks: Check[] = [];
  const add = (id: string, label: string, pass: boolean, detail: string) => checks.push({ id, group: 'feel', label, pass, detail });
  const voices = movedNotes(built, 0);
  const all = voices.flat();
  const timing = all.map((n) => n.timing);
  const worst = Math.max(...timing.map(Math.abs));
  const tune = all.filter((n) => n.pitched);

  add(
    'timing',
    'No note is moved more than 12 ms, and the band is not on a grid',
    worst <= 0.012 && rms(tune.map((n) => n.timing)) >= 0.001,
    `largest move ${ms(worst)}; the pitched voices average ${ms(rms(tune.map((n) => n.timing)))} off the grid (RMS) over ${all.length} notes`,
  );

  const levels = all.map((n) => n.level);
  add(
    'level',
    'Levels vary by a few percent, never more than 15',
    Math.max(...levels.map(Math.abs)) <= 0.15 && rms(levels) >= 0.02,
    `largest change ${pct(Math.max(...levels.map(Math.abs)))}, typical change ${pct(rms(levels))}`,
  );

  const cents = tune.map((n) => Math.abs(n.cents));
  const pans = all.map((n) => Math.abs(n.pan));
  add(
    'tuning',
    'Pitch stays within 6 cents and positions within a twentieth of the field',
    Math.max(...cents) <= 6 && Math.max(...pans) <= 0.05,
    `largest detune ${Math.max(...cents).toFixed(1)} cents, largest pan drift ${Math.max(...pans).toFixed(3)}`,
  );

  // The moves are unpatterned: one note's lateness says nothing of the next, and there is no hidden rush or drag.
  const long = voices.filter((v) => v.length >= 24);
  const correlations = long.map((v) => lagOne(v.map((n) => n.timing)));
  const typical = rms(correlations);
  // A voice's average move should be what chance gives: within four standard errors of zero for a uniform spread of its width.
  const leans = long.map((v) => {
    const width = HUMAN[v[0]!.role as keyof typeof HUMAN].timing;
    return Math.abs(mean(v.map((n) => n.timing))) / (width / Math.sqrt(3 * v.length));
  });
  add(
    'pattern',
    'The variations have no pattern and no hidden rush or drag',
    typical < 0.25 && Math.max(...leans) < 4,
    `note-to-note correlation ${typical.toFixed(2)} (0 is none) across ${long.length} voices; the largest average lean is ${Math.max(...leans).toFixed(1)} standard errors from zero`,
  );

  // Each pass is different, and the same seed plays the same way again.
  const again = movedNotes(built, 0).flat();
  const next = movedNotes(built, 1).flat();
  const same = again.every((n, i) => n.timing === all[i]!.timing && n.level === all[i]!.level);
  const differs = next.filter((n, i) => n.timing !== all[i]!.timing).length / Math.max(1, next.length);
  add(
    'passes',
    'Each time round is different, and a replay is identical',
    same && differs >= 0.9,
    `${pct(differs)} of notes move differently on the second pass; a rebuild from the same seed matches ${same ? 'exactly' : 'NOT at all'}`,
  );

  // The roles keep their places: the kick is the clock, the snare sits behind it and the hats ahead.
  const span = (role: keyof typeof HUMAN) => HUMAN[role].timing;
  const kickTightest = (Object.keys(HUMAN) as (keyof typeof HUMAN)[]).every((role) => span('kick') <= span(role));
  const pushes = Object.values(HUMAN).map((h) => Math.abs(h.push));
  add(
    'pocket',
    'The kick is the steadiest voice and no lean is over 5 ms',
    kickTightest && Math.max(...pushes) <= 0.005 && HUMAN.snare.push > 0 && HUMAN.hats.push < 0,
    `kick ±${ms(span('kick'))}; snare ${ms(HUMAN.snare.push)} behind, hats ${ms(-HUMAN.hats.push)} ahead, bass ${ms(HUMAN.bass.push)} behind`,
  );
  return checks;
}

/** Every check for one tune. */
export function validateTune(tune: LabTune): Check[] {
  const built = buildTune(tune);
  return [...theory(tune, built), ...feel(built)];
}
