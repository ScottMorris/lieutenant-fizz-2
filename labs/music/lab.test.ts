// Tests for the Music Lab: the patterns and riffs, the degree notation, every tune against every check, the live player and the zip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import type { ControlPatch, PlaybackHandle } from '@liminal-hq/undertone';
import { HUMAN, buildTune } from './build.ts';
import { crc32, zipOf } from './files.ts';
import { PATHS, pathPattern, tensionAccent } from './paths.ts';
import { COLUMNS, PATTERNS, ROWS, patternById, riffLine, riffOf } from './patterns.ts';
import { LabPlayer, beatOnsets, firstBass, turnPattern, type Step } from './player.ts';
import { degreeMidi, degrees, keyOf, noteName } from './scale.ts';
import { SURROUND_TUNE, TUNES, tuneById } from './tunes.ts';
import { validateTune } from './validate.ts';
import { EGA, VIZ_HEIGHT, VIZ_WIDTH, makeVisualizers, type Pen, type VizFrame } from './viz.ts';
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

describe('the Pathfinder paths', () => {
  test('there are six of sixteen steps, each obeying the rules of the game', () => {
    expect(PATHS.length).toBe(6);
    for (const path of PATHS) {
      expect(path.steps.length).toBe(16);
      path.steps.forEach((step, i) => {
        expect(step.height).toBeGreaterThanOrEqual(0);
        expect(step.height).toBeLessThanOrEqual(7);
        expect(step.tension).toBeGreaterThanOrEqual(0);
        expect(step.tension).toBeLessThanOrEqual(3);
        if (i === 0) return;
        const change = Math.abs(step.height - path.steps[i - 1]!.height);
        if (step.action === 'hold') expect(change).toBe(0);
        else expect(change).toBeLessThanOrEqual(step.action === 'drop' || step.action === 'leap' ? 4 : 2);
      });
      // The song loops, so the path ends within a step of where it began.
      expect(Math.abs(path.steps[15]!.height - path.steps[0]!.height)).toBeLessThanOrEqual(1);
    }
  });

  test('the four solo paths each hold still for three steps running, and two paths are relays', () => {
    const run = (path: (typeof PATHS)[number]) => {
      let best = 0;
      let now = 0;
      for (const step of path.steps) best = Math.max(best, (now = step.action === 'hold' ? now + 1 : 0));
      return best;
    };
    const relays = PATHS.filter((p) => /out|in,/.test(p.players) && p.players.includes(','));
    expect(relays.map((p) => p.id)).toEqual(['quick-way', 'spans']);
    for (const path of PATHS.filter((p) => !relays.includes(p))) expect(run(path)).toBeGreaterThanOrEqual(3);
  });

  test('a path is a pattern of one cell a column, and its tension is a swell', () => {
    const chimes = PATHS[0]!;
    expect(pathPattern(chimes).grid.map((row) => row.split('#').length - 1).reduce((a, b) => a + b)).toBe(16);
    expect(riffLine(riffOf(patternById('chimes')).top.slice(0, 8), true)).toBe('4 2 3 5@4 6');
    expect(tensionAccent(chimes)).toBe('<[0.78 0.85 0.85 0.85 0.93 1.00 1.00 0.93] [0.93 0.93 0.85 0.78 0.85 0.78 0.78 0.78]>');
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
  test('there are fourteen: six from one pattern each, two that combine three, six from the Pathfinder paths', () => {
    expect(TUNES.length).toBe(14);
    expect(TUNES.slice(8).map((t) => t.id)).toEqual(PATHS.map((p) => p.id));
    expect(TUNES.filter((t) => t.flow).length).toBe(6);
    const patternsOf = (id: string) => new Set(tuneById(id).motifs.map((m) => m.pattern)).size;
    expect(TUNES.slice(0, 6).map((t) => patternsOf(t.id))).toEqual([1, 1, 1, 1, 1, 1]);
    expect(TUNES.slice(6, 8).map((t) => patternsOf(t.id))).toEqual([3, 3]);
    expect(new Set(TUNES.slice(0, 6).flatMap((t) => t.motifs.map((m) => m.pattern))).size).toBe(6);
    expect(new Set(TUNES.map((t) => t.level)).size).toBe(14);
    expect(new Set(TUNES.map((t) => t.keyName)).size).toBeGreaterThanOrEqual(10);
  });

  for (const tune of TUNES) {
    test(`${tune.title} passes every check`, () => {
      const checks = validateTune(tune);
      expect(checks.filter((c) => c.group === 'theory').length).toBe(8);
      expect(checks.filter((c) => c.group === 'feel').length).toBe(6);
      expect(checks.filter((c) => c.group === 'flow').length).toBe(tune.flow ? 4 : 0);
      expect(checks.filter((c) => !c.pass).map((c) => `${c.id}: ${c.detail}`)).toEqual([]);
    });
  }

  test('no song has a fill or a snare roll, and each ends in its home key on the way back to the intro', () => {
    for (const tune of TUNES) {
      const b = buildTune(tune);
      expect(b.sections.map((s) => s.name)).not.toContain('Fill');
      expect(b.sections[b.sections.length - 1]!.shift).toBe(0);
      expect(b.sections[0]!.name).toBe('Intro');
      for (const section of b.sections) for (const { part } of section.parts) expect(part.role === 'snare' && part.notes === 'white*16').toBe(false);
      const build = b.sections.find((s) => s.name === 'Build');
      if (!build) continue;
      // The build is the orchestral bar and nothing else: five pitched lines, on the dominant.
      expect(build.parts.length).toBe(5);
      expect(build.parts.every(({ part }) => !part.noise)).toBe(true);
      expect(build.parts.map(({ part }) => part.role).sort()).toEqual(['bass', 'counter', 'counter', 'pad', 'perc']);
    }
  });

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
    const all = [0, 1, 2, 3].flatMap((beat) => beatOnsets(turnPattern(target, 'stereo'), 0, beat, 1).map((o) => o.patch));
    // Orchestral, not a drum fill: every voice of the turn is pitched.
    expect(all.length).toBeGreaterThan(10);
    expect(all.every((p) => typeof p.pitch === 'string' && p.soundType !== 'white')).toBe(true);
    const last = beatOnsets(turnPattern(target, 'stereo'), 0, 3, 1).map((o) => o.patch.pitch);
    expect(last).toContain(noteName(home - 2));
    expect(last.indexOf(noteName(home - 1))).toBeGreaterThan(last.indexOf(noteName(home - 2)));
    const second = beatOnsets(turnPattern(target, 'stereo'), 0, 1, 1).map((o) => o.patch.pitch);
    expect(second).toContain(noteName(home + 7));
    expect(second).toContain(noteName(home - 5));
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

describe('visualizers', () => {
  /** A pen that records what it is asked to fill. */
  function recorder() {
    const fills: { colour: string; x: number; y: number; w: number; h: number }[] = [];
    const pen = { fillStyle: '#000000', fillRect: (x: number, y: number, w: number, h: number) => void fills.push({ colour: String(pen.fillStyle), x, y, w, h }) };
    return { pen: pen as unknown as Pen, fills };
  }
  const frameAt = (time: number, playing: boolean): VizFrame => {
    const left = Float32Array.from({ length: 1024 }, (_, i) => (playing ? Math.sin(i / 9 + time) * 0.5 : 0));
    const right = Float32Array.from({ length: 1024 }, (_, i) => (playing ? Math.sin(i / 5 + time) * 0.3 : 0));
    const spectrum = Uint8Array.from({ length: 512 }, (_, i) => (playing ? Math.max(0, 220 - i) : 0));
    return { time, left, right, spectrum, playing, beat: Math.floor(time * 2) % 4, phase: (time * 2) % 1, bar: Math.floor(time / 2), turning: false, ink: EGA.yellow };
  };

  test('there are five, each drawing only EGA colours, at rest and while playing', () => {
    const set = makeVisualizers();
    expect(set.map((v) => v.id)).toEqual(['lighthouse', 'fizz', 'vats', 'field', 'scope']);
    const colours = new Set<string>(Object.values(EGA));
    expect(colours.size).toBe(16);
    for (const viz of set) {
      const { pen, fills } = recorder();
      viz.draw(pen, frameAt(0, false));
      const atRest = fills.length;
      expect(atRest).toBeGreaterThan(0);
      for (let i = 1; i <= 90; i++) viz.draw(pen, frameAt(i / 30, true));
      expect(fills.length).toBeGreaterThan(atRest + 90);
      for (const fill of fills) {
        expect(colours.has(fill.colour)).toBe(true);
        expect(Number.isFinite(fill.x + fill.y + fill.w + fill.h)).toBe(true);
      }
      expect(fills.every((fill) => fill.x < VIZ_WIDTH + 8 && fill.y < VIZ_HEIGHT + 24)).toBe(true);
    }
  });

  test('the fizz picture pours bubbles on each beat and keeps a bounded number', () => {
    const fizz = makeVisualizers().find((v) => v.id === 'fizz')!;
    const { pen, fills } = recorder();
    fizz.draw(pen, frameAt(0, false));
    const still = fills.length;
    fills.length = 0;
    for (let i = 1; i <= 600; i++) fizz.draw(pen, frameAt(i / 30, true));
    const perFrame = fills.length / 600;
    expect(perFrame).toBeGreaterThan(still);
    expect(perFrame).toBeLessThan(140 * 5 + 20);
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
