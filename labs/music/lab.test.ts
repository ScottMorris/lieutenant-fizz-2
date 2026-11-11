// Tests for the Music Lab: the patterns and riffs, the degree notation, every tune against every check, the live player and the zip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import type { ControlPatch, PlaybackHandle } from '@liminal-hq/undertone';
import { HUMAN, buildTune } from './build.ts';
import { crc32, zipOf } from './files.ts';
import { COLUMNS, PATTERNS, ROWS, patternById, riffLine, riffOf } from './patterns.ts';
import { LabPlayer, beatOnsets, firstBass, turnPattern, type Step } from './player.ts';
import { degreeMidi, degrees, keyOf, noteName } from './scale.ts';
import { SURROUND_TUNE, TUNES, tuneById } from './tunes.ts';
import { validateTune } from './validate.ts';
import { panOf, ringGains, sectionPattern, songPattern } from './voices.ts';

describe('patterns', () => {
  test('there are six, each 8 by 16 with 10 to 30 cells and at most three to a column', () => {
    expect(PATTERNS.length).toBe(6);
    for (const pattern of PATTERNS) {
      expect(pattern.grid.length).toBe(ROWS);
      for (const row of pattern.grid) expect(row).toMatch(/^[#.]{16}$/);
      const cells = pattern.grid.join('').split('#').length - 1;
      expect(cells).toBeGreaterThanOrEqual(10);
      expect(cells).toBeLessThanOrEqual(30);
      for (let column = 0; column < COLUMNS; column++) {
        expect(pattern.grid.filter((row) => row[column] === '#').length).toBeLessThanOrEqual(3);
      }
    }
  });

  test('a riff is the highest cell of each column, and the lowest where it differs', () => {
    const beam = riffOf(patternById('beam'));
    expect(riffLine(beam.top)).toBe('0 1 1 2 2 3 3 4 4 5 5 6 6 7 7 ~');
    expect(beam.low.every((cell) => cell === null)).toBe(true);
    const press = riffOf(patternById('press'), -1);
    expect(riffLine(press.top)).toBe('0 -1 6 ~ 0 -1 6 ~ 0 -1 6 ~ 0 -1 6 ~');
    expect(riffLine(press.low)).toBe('-1 ~ 5 ~ -1 ~ 5 ~ -1 ~ 5 ~ -1 ~ 5 ~');
  });

  test('held runs become one long note', () => {
    expect(riffLine(riffOf(patternById('keys')).top.slice(0, 8), true)).toBe('~ 6@2 4@2 2@3');
    expect(riffLine(riffOf(patternById('crystal')).top, true)).toBe('0@2 1 2@2 1 0 ~@2 0 1 2 1 0@2 ~');
  });
});

describe('degrees', () => {
  const key = keyOf('e dorian');
  test('a degree is a note of the key, and counts are left alone', () => {
    expect(degrees(key, 4, '0 1 2 7 -1')).toBe('e4 f#4 g4 e5 d4');
    expect(degrees(key, 3, '<[0,2,4] [~ 3@2 -1]*2>')).toBe('<[e3,g3,b3] [~ a3@2 d3]*2>');
    expect(noteName(degreeMidi(keyOf('d phrygian'), 4, 1))).toBe('d#4');
  });
});

describe('tunes', () => {
  test('there are eight: six from one pattern each, two that combine three', () => {
    expect(TUNES.length).toBe(8);
    const patternsOf = (id: string) => new Set(tuneById(id).motifs.map((m) => m.pattern)).size;
    expect(TUNES.slice(0, 6).map((t) => patternsOf(t.id))).toEqual([1, 1, 1, 1, 1, 1]);
    expect(TUNES.slice(6).map((t) => patternsOf(t.id))).toEqual([3, 3]);
    expect(new Set(TUNES.slice(0, 6).flatMap((t) => t.motifs.map((m) => m.pattern))).size).toBe(6);
    expect(new Set(TUNES.map((t) => t.level)).size).toBe(8);
  });

  for (const tune of TUNES) {
    test(`${tune.title} passes every check`, () => {
      const checks = validateTune(tune);
      expect(checks.filter((c) => c.group === 'theory').length).toBe(8);
      expect(checks.filter((c) => c.group === 'feel').length).toBe(6);
      expect(checks.filter((c) => !c.pass).map((c) => `${c.id}: ${c.detail}`)).toEqual([]);
    });
  }

  test('the humanising is small: at most 9 ms, 12 percent and 4 cents', () => {
    for (const human of Object.values(HUMAN)) {
      expect(human.timing).toBeLessThanOrEqual(0.009);
      expect(human.level).toBeLessThanOrEqual(0.12);
      expect(human.cents).toBeLessThanOrEqual(4);
      expect(Math.abs(human.push)).toBeLessThanOrEqual(0.005);
    }
  });
});

describe('voices', () => {
  const built = buildTune(tuneById(SURROUND_TUNE));
  const lead = built.sections[1]!.parts.find(({ part }) => part.role === 'lead' && part.mirror === undefined)!.part;
  const partner = built.sections[1]!.parts.find(({ part }) => part.role === 'lead' && part.mirror !== undefined)!.part;

  test('a partner of a moving part leans a third as far the other way; a partner of a fixed part mirrors it', () => {
    expect(panOf(lead)).toBe('<[-.7 -.5 -.3 -.1 .1 .3 .5 .7] [.7 .5 .3 .1 -.1 -.3 -.5 -.7]>');
    expect(panOf(partner)).toBe('<[0.245 0.175 0.105 0.035 -0.035 -0.105 -0.175 -0.245] [-0.245 -0.175 -0.105 -0.035 0.035 0.105 0.175 0.245]>');
    expect(panOf({ notes: 'c4', g: 0.1, role: 'counter' })).toBe(0.2);
    expect(panOf({ notes: 'c4', g: 0.1, role: 'counter', mirror: 0.22 })).toBe(-0.2);
    expect(panOf({ notes: 'c4', g: 0.1, role: 'bass', mirror: 0.22 })).toBe(0.22);
    expect(panOf({ notes: 'c4', g: 0.1, role: 'perc', pan: '[-.8 .8]*2' })).toBe('[-.8 .8]*2');
  });

  test('in the 7.1 mix the lead circles the ring once in two bars, its partner opposite', () => {
    const loudest = (gains: number[]) => gains.indexOf(Math.max(...gains));
    // FL, FR, C, LFE, SL, SR, RL, RR
    expect(loudest(ringGains(lead, built.tune, 0, 0))).toBe(2);
    expect(loudest(ringGains(lead, built.tune, 0, 0.5))).toBe(5);
    expect(loudest(ringGains(lead, built.tune, 0, 1.5))).toBe(4);
    expect(loudest(ringGains(partner, built.tune, 0, 0.5))).toBe(4);
    expect(ringGains(lead, built.tune, 0, 2)).toEqual(ringGains(lead, built.tune, 0, 0));
  });

  test('every note of the 7.1 song has eight speaker gains and no pan; the stereo song has neither more than two channels', () => {
    const surround = beatOnsets(songPattern(built, { mix: 'surround', human: true }), 2, 0, 1).map((o) => o.patch);
    expect(surround.length).toBeGreaterThan(4);
    for (const patch of surround) {
      expect(patch.channelGains?.length).toBe(8);
      expect(patch.pan).toBeUndefined();
    }
    const stereo = beatOnsets(songPattern(built, { mix: 'stereo', human: true }), 2, 0, 1).map((o) => o.patch);
    for (const patch of stereo) expect(patch.channelGains).toBeUndefined();
  });

  test('every tune builds in both mixes, and every bar of it can be asked for', () => {
    for (const tune of TUNES) {
      const b = buildTune(tune);
      for (const mix of ['stereo', 'surround'] as const) {
        const song = songPattern(b, { mix, human: true });
        let notes = 0;
        for (let bar = 0; bar < b.bars; bar++) for (let beat = 0; beat < 4; beat++) notes += beatOnsets(song, bar, beat, 1).length;
        expect(notes).toBeGreaterThan(500);
      }
    }
  });

  test('a second pass of a section plays the same notes, moved differently', () => {
    const section = built.sections[1]!;
    const pattern = sectionPattern(built, section, { mix: 'stereo', human: true });
    const pass = (n: number) => beatOnsets(pattern, n * section.cycles, 0, 1);
    const first = pass(0);
    const second = pass(1);
    expect(second.map((o) => o.at)).toEqual(first.map((o) => o.at));
    expect(second.map((o) => o.patch.nudgeTime)).not.toEqual(first.map((o) => o.patch.nudgeTime));
    expect(pass(0).map((o) => o.patch.nudgeTime)).toEqual(first.map((o) => o.patch.nudgeTime));
  });
});

describe('the live player', () => {
  const built = buildTune(tuneById('press'));
  const beat = 240 / built.tune.track.bpm / 4;

  /** A player on a clock the test moves by hand. `run(beats)` advances that many beats and returns the steps taken. */
  function rig() {
    const ctx = { currentTime: 0 };
    let tick: () => void = () => {};
    const booked: { patch: ControlPatch; when: number }[] = [];
    const handle = (when: number): PlaybackHandle => ({ startTime: when, endTime: when + 0.2, stop: () => undefined });
    const player = new LabPlayer(
      ctx,
      null,
      built,
      { mix: 'stereo', human: true },
      { setInterval: (cb) => ((tick = cb), 1), clearInterval: () => undefined },
      (patch, when) => (booked.push({ patch, when }), handle(when)),
    );
    const seen: Step[] = [];
    const run = (beats: number): Step[] => {
      const from = seen.length;
      for (let i = 0; i < beats * 4; i++) {
        ctx.currentTime += beat / 4;
        tick();
        const step = player.stepAt(ctx.currentTime);
        if (step && seen[seen.length - 1]?.time !== step.time) seen.push(step);
      }
      return seen.slice(from);
    };
    return { player, run, booked, ctx };
  }
  const at = (step: Step) => `${step.section}:${step.bar}.${step.beat}${step.turning ? 't' : ''}`;

  test('left alone it plays the sections in order', () => {
    const { player, run } = rig();
    player.start(0);
    const steps = run(12).map(at);
    expect(steps.slice(0, 9)).toEqual(['0:0.0', '0:0.1', '0:0.2', '0:0.3', '0:1.0', '0:1.1', '0:1.2', '0:1.3', '1:0.0']);
  });

  test('a bar call turns the rest of the bar and lands on the barline', () => {
    const { player, run } = rig();
    player.start(1);
    run(1.2);
    player.call(3, 'bar');
    const steps = run(5).map(at);
    expect(steps).toContain('1:0.3t');
    expect(steps.filter((s) => s.endsWith('t')).every((s) => s.startsWith('1:0.'))).toBe(true);
    expect(steps).toContain('3:0.0');
    expect(steps.indexOf('3:0.0')).toBe(steps.indexOf('1:0.3t') + 1);
    expect(player.queued).toBeNull();
  });

  test('a beat call hops on the next beat and keeps its place in the bar', () => {
    const { player, run } = rig();
    player.start(1);
    run(1.2);
    player.call(2, 'beat');
    const steps = run(3).map(at);
    const landed = steps.find((s) => s.startsWith('2:'))!;
    expect(landed).toMatch(/^2:0\.[23]$/);
    expect(steps.some((s) => s.endsWith('t'))).toBe(false);
  });

  test('a section call waits for the end, turning the last two beats', () => {
    const { player, run } = rig();
    player.start(0);
    run(1);
    player.call(5, 'section');
    const steps = run(8).map(at);
    expect(steps).toContain('0:1.1');
    expect(steps).toContain('0:1.2t');
    expect(steps).toContain('0:1.3t');
    expect(steps[steps.indexOf('0:1.3t') + 1]).toBe('5:0.0');
  });

  test('hold keeps a section going round, and calling the called section again cancels', () => {
    const { player, run } = rig();
    player.hold = true;
    player.start(0);
    expect(new Set(run(12).map((s) => s.section))).toEqual(new Set([0]));
    player.call(2, 'section');
    player.call(2, 'section');
    expect(player.queued).toBeNull();
  });

  test('the turn walks the bass from the dominant up two semitones into the target’s first note', () => {
    const target = built.sections[2]!;
    const home = firstBass(target)!.midi;
    const last = beatOnsets(turnPattern(target, 'stereo'), 0, 3, 1).map((o) => o.patch);
    const pitches = last.filter((p) => typeof p.pitch === 'string' && p.pitch !== 'c1').map((p) => p.pitch);
    expect(pitches).toEqual([noteName(home - 2), noteName(home - 1)]);
    const first = beatOnsets(turnPattern(target, 'stereo'), 0, 1, 1).map((o) => o.patch.pitch).filter((p) => typeof p === 'string');
    expect(first).toEqual([noteName(home + 7)]);
  });

  test('stopping cuts what is sounding and forgets the call', () => {
    const { player, run, booked } = rig();
    player.start(0);
    run(2);
    expect(booked.length).toBeGreaterThan(4);
    player.call(1, 'bar');
    player.stop();
    expect(player.playing).toBe(false);
    expect(player.queued).toBeNull();
  });
});

describe('files', () => {
  test('crc32 is the standard one', () => {
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
  });

  test('a zip holds its one file, stored', () => {
    const data = new TextEncoder().encode('RIFF....WAVE');
    const zip = zipOf('tune.wav', data);
    const view = new DataView(zip.buffer);
    expect(view.getUint32(0, true)).toBe(0x04034b50);
    expect(new TextDecoder().decode(zip.subarray(30, 38))).toBe('tune.wav');
    expect(new TextDecoder().decode(zip.subarray(38, 38 + data.length))).toBe('RIFF....WAVE');
    expect(view.getUint32(zip.length - 22, true)).toBe(0x06054b50);
    expect(view.getUint32(zip.length - 6, true)).toBe(38 + data.length);
  });
});
