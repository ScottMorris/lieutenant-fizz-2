// Tests the Web HUD's pure part: the text, the red at zero, the score field, the key tray, the scale, change detection and the announcements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/hud.test.ts`. The page itself is checked in headless Chromium by tools/check-web-hud.py.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  COUNT_CAP,
  KEY_KINDS,
  KEY_ROWS,
  MAX_PIPS,
  SCORE_CAP,
  announcement,
  formatCount,
  hudScale,
  hudSide,
  hudView,
  keyPixelSize,
  keySvg,
  overlaps,
  parseHud,
  sameHud,
  scoreText,
  type HudValues,
  type KeyReading,
} from './hud.ts';

const empty: KeyReading = { inLevel: false, held: 0, ring: false };
const socket = (held = 0, ring = false): KeyReading => ({ inLevel: true, held, ring });

/** A reading with the red and blue sockets in the level, as the slice has them. */
const at = (ammo: number, red = 0, deaths = 0, extra: Partial<HudValues> = {}): HudValues => ({
  ammo,
  score: 0,
  pop: 0,
  deaths,
  keys: [socket(red), socket(0), empty, empty],
  ...extra,
});

describe('the numbers', () => {
  test('a count is a whole number from 0 to 99', () => {
    expect(formatCount(5)).toBe('5');
    expect(formatCount(0)).toBe('0');
    expect(formatCount(99)).toBe('99');
    expect(formatCount(100)).toBe('99');
    expect(formatCount(123456)).toBe('99');
    expect(COUNT_CAP).toBe(99);
    expect(formatCount(-3)).toBe('0');
    expect(formatCount(2.9)).toBe('2');
    expect(formatCount(Number.NaN)).toBe('0');
  });

  test('the Fizz number is red at zero and not before', () => {
    expect(hudView(at(5))).toMatchObject({ fizz: '5', fizzOut: false });
    expect(hudView(at(1)).fizzOut).toBe(false);
    expect(hudView(at(0))).toMatchObject({ fizz: '0', fizzOut: true });
  });

  test('the deaths show in the lives slot and cap at 99', () => {
    expect(hudView(at(5, 0, 3)).deaths).toBe('3');
    expect(hudView(at(5, 0, 250)).deaths).toBe('99');
  });
});

describe('the score field', () => {
  // The same table as score_digits in crates/fizz-slice-view/src/hud.rs: the digits and how many leading ones only pad.
  test('is five digits with the leading zeros split off, the last digit always real', () => {
    expect(scoreText(0)).toEqual({ lead: '0000', rest: '0' });
    expect(scoreText(5)).toEqual({ lead: '0000', rest: '5' });
    expect(scoreText(1240)).toEqual({ lead: '0', rest: '1240' });
    expect(scoreText(99_999)).toEqual({ lead: '', rest: '99999' });
  });
  test('holds at 99999', () => {
    expect(SCORE_CAP).toBe(99_999);
    expect(scoreText(100_000)).toEqual({ lead: '', rest: '99999' });
    expect(scoreText(4_000_000_000)).toEqual({ lead: '', rest: '99999' });
    expect(scoreText(-4)).toEqual({ lead: '0000', rest: '0' });
    expect(scoreText(Number.NaN)).toEqual({ lead: '0000', rest: '0' });
  });
  test('the view shows the digits it is given and the pop in place of the word', () => {
    expect(hudView(at(5, 0, 0, { score: 7 }))).toMatchObject({ scoreLead: '0000', scoreRest: '7', pop: '' });
    expect(hudView(at(5, 0, 0, { score: 7, pop: 5 })).pop).toBe('+5');
    expect(hudView(at(5, 0, 0, { pop: 1234 })).pop).toBe('+99');
  });
});

describe('the key tray', () => {
  test('has a socket for each kind the level has, empty from the start', () => {
    const v = hudView(at(5));
    expect(v.sockets.map((s) => [s.kind, s.shown, s.held])).toEqual([
      ['red', true, false],
      ['blue', true, false],
      ['green', false, false],
      ['gold', false, false],
    ]);
  });
  test('a held key fills its socket, and the ring shows only on a held key', () => {
    const v = hudView(at(5, 1, 0, { keys: [socket(1, true), socket(0, true), empty, empty] }));
    expect(v.sockets[0]).toMatchObject({ held: true, ring: true, pips: 0, digit: '' });
    expect(v.sockets[1]).toMatchObject({ held: false, ring: false });
  });
  test('duplicates are pips up to four and a digit after', () => {
    const pips = (held: number) => hudView(at(5, 0, 0, { keys: [socket(held), empty, empty, empty] })).sockets[0];
    expect(MAX_PIPS).toBe(4);
    expect([0, 1].map((n) => pips(n).pips)).toEqual([0, 0]);
    expect([2, 3, 4].map((n) => pips(n).pips)).toEqual([2, 3, 4]);
    expect(pips(4).digit).toBe('');
    expect(pips(5)).toMatchObject({ pips: 0, digit: '5' });
    expect(pips(40).digit).toBe('9');
  });
  test("reads the module's integers", () => {
    const raw = [5, 12, 5, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 1, 3, 0];
    expect(parseHud(raw)).toEqual({
      ammo: 5,
      score: 12,
      pop: 5,
      deaths: 1,
      keys: [
        { inLevel: true, held: 1, ring: true },
        { inLevel: true, held: 0, ring: false },
        { inLevel: false, held: 0, ring: false },
        { inLevel: true, held: 3, ring: false },
      ],
    });
  });
});

describe('the scale', () => {
  test("follows Fizz 1's pixel scale: 2 under 480 px, 3 to 900 px, 4 above", () => {
    expect([200, 479, 480, 720, 899, 900, 1080, 2160].map(hudScale)).toEqual([2, 2, 3, 3, 3, 4, 4, 4]);
  });
  test('a tray key pixel is as big as a digit pixel', () => {
    expect([2, 3, 4, 5].map(keyPixelSize)).toEqual([2, 3, 4, 5]);
  });
});

describe('change detection', () => {
  test('equal readings are the same, and the first is never the same as a reading', () => {
    expect(sameHud(at(5), at(5))).toBe(true);
    expect(sameHud(null, null)).toBe(true);
    expect(sameHud(null, at(5))).toBe(false);
    expect(sameHud(at(5), at(4))).toBe(false);
    expect(sameHud(at(5), at(5, 1))).toBe(false);
    expect(sameHud(at(5), at(5, 0, 1))).toBe(false);
    expect(sameHud(at(5), at(5, 0, 0, { score: 1 }))).toBe(false);
    expect(sameHud(at(5), at(5, 0, 0, { pop: 1 }))).toBe(false);
    expect(sameHud(at(5, 1), at(5, 1, 0, { keys: [socket(1, true), socket(0), empty, empty] }))).toBe(false);
  });
  test('values past the cap show alike, so they do not touch the page', () => {
    expect(sameHud(at(120), at(130))).toBe(true);
    expect(sameHud(at(0, 0, 150), at(0, 0, 151))).toBe(true);
    expect(sameHud(at(5, 0, 0, { score: 100_000 }), at(5, 0, 0, { score: 200_000 }))).toBe(true);
  });
});

describe('announcements', () => {
  test('the first reading says nothing', () => {
    expect(announcement(null, at(0, 1, 4))).toBeNull();
  });
  test('a shot is silent until the Fizz runs out', () => {
    expect(announcement(at(5), at(4))).toBeNull();
    expect(announcement(at(2), at(1))).toBeNull();
    expect(announcement(at(1), at(0))).toBe('Out of Fizz');
    expect(announcement(at(0), at(0))).toBeNull();
  });
  test('a key is announced by its kind, once, when it is picked up', () => {
    expect(announcement(at(3, 0), at(3, 1))).toBe('Red key picked up');
    expect(announcement(at(3, 1), at(3, 1))).toBeNull();
    expect(announcement(at(3, 1), at(3, 0))).toBeNull();
    const blue = at(3, 0, 0, { keys: [socket(0), socket(1), empty, empty] });
    expect(announcement(at(3), blue)).toBe('Blue key picked up');
    const gold = at(3, 0, 0, { keys: [socket(0), socket(0), empty, socket(1)] });
    expect(announcement(at(3), gold)).toBe('Gold key picked up');
  });
  test('the score and the pop are not spoken', () => {
    expect(announcement(at(3), at(3, 0, 0, { score: 5, pop: 5 }))).toBeNull();
  });
  test('a death is announced with the count', () => {
    expect(announcement(at(3, 0, 0), at(3, 0, 1))).toBe('Deaths 1');
    expect(announcement(at(3, 0, 1), at(3, 0, 2))).toBe('Deaths 2');
  });
  test('a restart is silent', () => {
    expect(announcement(at(0, 1, 3), at(5, 0, 0))).toBeNull();
  });
  test('several things in one frame are joined in order', () => {
    expect(announcement(at(1, 0, 0), at(0, 1, 1))).toBe('Out of Fizz. Red key picked up. Deaths 1');
  });
});

describe('boxes', () => {
  const hud = { x: 16, y: 16, w: 100, h: 50 };
  test('overlap, touching and the margin', () => {
    expect(overlaps(hud, { x: 50, y: 40, w: 16, h: 16 })).toBe(true);
    expect(overlaps(hud, { x: 116, y: 16, w: 16, h: 16 })).toBe(false);
    expect(overlaps(hud, { x: 120, y: 16, w: 16, h: 16 }, 8)).toBe(true);
    expect(overlaps(hud, { x: 300, y: 300, w: 16, h: 16 }, 8)).toBe(false);
  });
});

describe('the key drawings', () => {
  const sheet = JSON.parse(readFileSync(new URL('../assets/sprites/ep2-hud-keys.json', import.meta.url), 'utf8'));
  const named = (name: string) => sheet.sprites.find((s: { name: string }) => s.name === name);

  test("are the grids of the sprite set the DOS tray draws", () => {
    for (const kind of KEY_KINDS) {
      expect([...KEY_ROWS[kind]]).toEqual(named('hudkey_' + kind).rows);
    }
  });
  test('each kind has its own colour and its own mark, so none is told apart by hue alone', () => {
    const grids = KEY_KINDS.map((k) => KEY_ROWS[k].join('/'));
    expect(new Set(grids).size).toBe(4);
    // Blacking out the fill leaves the marks: the colours are not what tells them apart.
    const marks = KEY_KINDS.map((k) =>
      KEY_ROWS[k]
        .map((r) => r.replace(/[^k.]/g, '.'))
        .join('/'),
    );
    expect(new Set(marks).size).toBe(4);
  });
  test('held is in colour, with the mark in black', () => {
    const red = keySvg('red', true);
    expect(red).toContain('#ff5555');
    expect(red).toContain('#000000');
    expect(red).toContain('aria-hidden="true"');
    expect(red).toContain('viewBox="0 0 7 7"');
    expect(keySvg('blue', true)).toContain('#5555ff');
    expect(keySvg('green', true)).toContain('#55ff55');
    expect(keySvg('gold', true)).toContain('#ffff55');
  });
  test('not held is a dim outline: no colour, and only the edge of the silhouette', () => {
    const svg = keySvg('red', false);
    expect(svg).not.toContain('#ff5555');
    expect(svg).toContain('fill-opacity');
    const count = (s: string) => [...s.matchAll(/h(\d+)v1/g)].reduce((n, m) => n + Number(m[1]), 0);
    // The inside of the silhouette is not drawn: the DOS socket draws the same edge.
    const body = KEY_ROWS.red.join('').replaceAll('.', '').length;
    expect(count(svg)).toBeLessThan(body);
    expect(count(svg)).toBeGreaterThan(15);
  });
  test('a sprite with an empty centre keeps only its edge', () => {
    const rows = ['###', '#.#', '###'];
    const svg = keySvg('red', false, rows);
    expect([...svg.matchAll(/h(\d+)v1/g)].reduce((n, m) => n + Number(m[1]), 0)).toBe(8);
  });
});

describe('the corner', () => {
  const hud = { x: 16, y: 16, w: 200, h: 80 };
  const ben = (x: number, y: number) => ({ x, y, w: 32, h: 32 });
  test('stays while Ben is clear of it', () => {
    expect(hudSide('left', hud, 1000, ben(500, 300), 8)).toBe('left');
    expect(hudSide('right', { ...hud, x: 784 }, 1000, ben(500, 300), 8)).toBe('right');
  });
  test('crosses when Ben comes near, and back when he comes near the other corner', () => {
    expect(hudSide('left', hud, 1000, ben(100, 40), 8)).toBe('right');
    expect(hudSide('right', { ...hud, x: 784 }, 1000, ben(850, 40), 8)).toBe('left');
  });
  test('the margin counts', () => {
    expect(hudSide('left', hud, 1000, ben(220, 20), 8)).toBe('right');
    expect(hudSide('left', hud, 1000, ben(230, 20), 8)).toBe('left');
  });
  test('stays put when Ben is near both corners', () => {
    expect(hudSide('left', hud, 300, ben(100, 40), 8)).toBe('left');
  });
});
