// Tests the Web HUD's pure part: the text, the red at zero, the key chip, the scale, change detection and the announcements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/hud.test.ts`. The page itself is checked in headless Chromium by tools/check-web-hud.py.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  COUNT_CAP,
  KEY_ROWS,
  announcement,
  formatCount,
  hudScale,
  hudSide,
  hudView,
  keyPixelSize,
  keySvg,
  overlaps,
  sameHud,
  type HudValues,
} from './hud.ts';

const at = (ammo: number, key = false, deaths = 0): HudValues => ({ ammo, key, deaths });

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

  test('the deaths show beside it and cap at 99', () => {
    expect(hudView(at(5, false, 3)).deaths).toBe('3');
    expect(hudView(at(5, false, 250)).deaths).toBe('99');
  });

  test('the key chip follows the key', () => {
    expect(hudView(at(5, false)).key).toBe(false);
    expect(hudView(at(5, true)).key).toBe(true);
  });
});

describe('the scale', () => {
  test("follows Fizz 1's pixel scale: 2 under 480 px, 3 to 900 px, 4 above", () => {
    expect([200, 479, 480, 720, 899, 900, 1080, 2160].map(hudScale)).toEqual([2, 2, 3, 3, 3, 4, 4, 4]);
  });
  test('the key chip is 2 px a pixel at scales 2 to 4', () => {
    expect([2, 3, 4, 5].map(keyPixelSize)).toEqual([2, 2, 2, 3]);
  });
});

describe('change detection', () => {
  test('equal readings are the same, and the first is never the same as a reading', () => {
    expect(sameHud(at(5), at(5))).toBe(true);
    expect(sameHud(null, null)).toBe(true);
    expect(sameHud(null, at(5))).toBe(false);
    expect(sameHud(at(5), at(4))).toBe(false);
    expect(sameHud(at(5), at(5, true))).toBe(false);
    expect(sameHud(at(5), at(5, false, 1))).toBe(false);
  });
  test('values past the cap show alike, so they do not touch the page', () => {
    expect(sameHud(at(120), at(130))).toBe(true);
    expect(sameHud(at(0, false, 150), at(0, false, 151))).toBe(true);
  });
});

describe('announcements', () => {
  test('the first reading says nothing', () => {
    expect(announcement(null, at(0, true, 4))).toBeNull();
  });
  test('a shot is silent until the Fizz runs out', () => {
    expect(announcement(at(5), at(4))).toBeNull();
    expect(announcement(at(2), at(1))).toBeNull();
    expect(announcement(at(1), at(0))).toBe('Out of Fizz');
    expect(announcement(at(0), at(0))).toBeNull();
  });
  test('the key is announced once, when it is picked up', () => {
    expect(announcement(at(3, false), at(3, true))).toBe('Red key picked up');
    expect(announcement(at(3, true), at(3, true))).toBeNull();
    expect(announcement(at(3, true), at(3, false))).toBeNull();
  });
  test('a death is announced with the count', () => {
    expect(announcement(at(3, false, 0), at(3, false, 1))).toBe('Deaths 1');
    expect(announcement(at(3, false, 1), at(3, false, 2))).toBe('Deaths 2');
  });
  test('a restart is silent', () => {
    expect(announcement(at(0, true, 3), at(5, false, 0))).toBeNull();
  });
  test('several things in one frame are joined in order', () => {
    expect(announcement(at(1, false, 0), at(0, true, 1))).toBe('Out of Fizz. Red key picked up. Deaths 1');
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

describe('the key chip drawing', () => {
  test("is Fizz 1's keyRed sprite", () => {
    const sheet = JSON.parse(readFileSync(new URL('../assets/sprites/fizz1-slice.json', import.meta.url), 'utf8'));
    const key = sheet.sprites.find((s: { name: string }) => s.name === 'keyRed');
    expect([...KEY_ROWS]).toEqual(key.rows);
  });
  test('carried is in colour, with the sprite at its red', () => {
    const svg = keySvg(true);
    expect(svg).toContain('#ff5555');
    expect(svg).toContain('#aa0000');
    expect(svg).toContain('aria-hidden="true"');
    expect(svg).toContain('viewBox="0 0 16 16"');
  });
  test('not carried is a dim outline: no red, and fewer pixels than the sprite', () => {
    const svg = keySvg(false);
    expect(svg).not.toContain('#ff5555');
    expect(svg).toContain('fill-opacity');
    // The outline is the sprite's black ring.
    const count = (s: string) => [...s.matchAll(/h(\d+)v1/g)].reduce((n, m) => n + Number(m[1]), 0);
    expect(count(svg)).toBe(KEY_ROWS.join('').replaceAll('.', '').split('').filter((c) => c === 'k').length);
  });
  test('a sprite with an empty centre keeps only its edge', () => {
    const rows = ['###', '#.#', '###'];
    const svg = keySvg(false, rows);
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
