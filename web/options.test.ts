// Tests the Web Options screen's pure part: reading, migrating and writing the remembered settings, the rows' order and the look's data.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/options.test.ts`. The overlay itself is checked in headless Chromium by tools/check-web-options.py.
import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import {
  DEFAULT_OPTIONS,
  LEGACY_KEYS,
  MENU_KEY,
  OPTIONS_KEY,
  PLATE,
  PLATE_INK,
  ROW,
  SODA_ROWS,
  loadOptions,
  menuKeyFor,
  migrateLegacy,
  opensPaused,
  optionsScale,
  parseOptions,
  placementFromSearch,
  pullOptions,
  pushOptions,
  saveOptions,
  serialiseOptions,
  sodaSvg,
  spoken,
  type Options,
  type StorageLike,
} from './options.ts';

class Memory implements StorageLike {
  data = new Map<string, string>();
  constructor(init: Record<string, string> = {}) {
    for (const [k, v] of Object.entries(init)) this.data.set(k, v);
  }
  getItem(key: string) {
    return this.data.get(key) ?? null;
  }
  setItem(key: string, value: string) {
    this.data.set(key, value);
  }
  removeItem(key: string) {
    this.data.delete(key);
  }
}

const some: Options = { sound: 'off', music: 'off', placement: 'classic', motion: 'reduced', captions: 'off', keys: 'modern' };

describe('the blob', () => {
  test('serialises version first and parses back', () => {
    const text = serialiseOptions(some);
    expect(text).toBe('{"v":1,"sound":"off","music":"off","placement":"classic","motion":"reduced","captions":"off","keys":"modern"}');
    expect(parseOptions(text)).toEqual(some);
    expect(parseOptions(serialiseOptions(DEFAULT_OPTIONS))).toEqual(DEFAULT_OPTIONS);
  });

  test('bad text, other versions and non-objects are refused', () => {
    for (const text of [null, undefined, '', 'nope', '{', '[]', '3', 'null', '{"v":2}', '{"sound":"off"}', '{"v":"1"}']) {
      expect(parseOptions(text)).toBeNull();
    }
  });

  test('a field that is missing or wrong falls back alone', () => {
    const got = parseOptions('{"v":1,"sound":"off","music":"loud","motion":7,"keys":"modern","extra":true}');
    expect(got).toEqual({ ...DEFAULT_OPTIONS, sound: 'off', keys: 'modern' });
  });

  test('the defaults are the settings of a fresh visit', () => {
    expect(DEFAULT_OPTIONS).toEqual({ sound: 'on', music: 'on', placement: 'enhanced', motion: 'system', captions: 'on', keys: 'keen' });
  });
});

describe('migrating the older keys', () => {
  const all = {
    [LEGACY_KEYS.layout]: 'modern',
    [LEGACY_KEYS.captions]: 'off',
    [LEGACY_KEYS.motion]: 'reduced',
    [LEGACY_KEYS.audio]: 'classic',
    [LEGACY_KEYS.sound]: 'off',
    [LEGACY_KEYS.music]: 'off',
  };

  test('the names are the ones earlier builds wrote', () => {
    expect(LEGACY_KEYS.layout).toBe('fizz2-layout');
    expect(LEGACY_KEYS.captions).toBe('fizz2-captions');
    expect(LEGACY_KEYS.motion).toBe('fizz2-motion');
    expect(LEGACY_KEYS.audio).toBe('fizz2-audio');
    expect(OPTIONS_KEY).toBe('fizz2-options');
  });

  test('each old key gives its setting', () => {
    const { options, found } = migrateLegacy((k) => (all as Record<string, string>)[k] ?? null);
    expect(found).toBe(true);
    expect(options).toEqual(some);
  });

  test('no old keys is not a migration', () => {
    expect(migrateLegacy(() => null)).toEqual({ options: DEFAULT_OPTIONS, found: false });
  });

  test('a value an old build never wrote leaves the default', () => {
    const { options } = migrateLegacy((k) => (k === LEGACY_KEYS.motion ? 'system' : k === LEGACY_KEYS.layout ? 'dvorak' : null));
    expect(options).toEqual(DEFAULT_OPTIONS);
  });

  test('the first load writes the blob and removes the old keys, once', () => {
    const store = new Memory(all);
    expect(loadOptions(store)).toEqual(some);
    expect(parseOptions(store.getItem(OPTIONS_KEY))).toEqual(some);
    for (const key of Object.values(LEGACY_KEYS)) expect(store.getItem(key)).toBeNull();
    expect(loadOptions(store)).toEqual(some);
  });

  test('the blob wins over leftover old keys', () => {
    const store = new Memory({ ...all, [OPTIONS_KEY]: serialiseOptions(DEFAULT_OPTIONS) });
    expect(loadOptions(store)).toEqual(DEFAULT_OPTIONS);
    expect(store.getItem(LEGACY_KEYS.layout)).toBe('modern');
  });

  test('a fresh visit writes nothing', () => {
    const store = new Memory();
    expect(loadOptions(store)).toEqual(DEFAULT_OPTIONS);
    expect(store.data.size).toBe(0);
  });

  test('a corrupt blob is replaced by the old keys, or the defaults', () => {
    expect(loadOptions(new Memory({ [OPTIONS_KEY]: 'junk', [LEGACY_KEYS.captions]: 'off' })).captions).toBe('off');
    expect(loadOptions(new Memory({ [OPTIONS_KEY]: 'junk' }))).toEqual(DEFAULT_OPTIONS);
  });
});

describe('storage that fails', () => {
  const broken: StorageLike = {
    getItem() {
      throw new Error('blocked');
    },
    setItem() {
      throw new Error('blocked');
    },
  };

  test('loading gives the defaults and saving reports it', () => {
    expect(loadOptions(broken)).toEqual(DEFAULT_OPTIONS);
    expect(loadOptions(null)).toEqual(DEFAULT_OPTIONS);
    expect(saveOptions(broken, some)).toBe(false);
    expect(saveOptions(null, some)).toBe(false);
  });

  test('saving into working storage can be read back', () => {
    const store = new Memory();
    expect(saveOptions(store, some)).toBe(true);
    expect(loadOptions(store)).toEqual(some);
  });
});

describe('the address override', () => {
  test('?audio= names a placement', () => {
    expect(placementFromSearch('?audio=classic')).toBe('classic');
    expect(placementFromSearch('?x=1&audio=enhanced')).toBe('enhanced');
    expect(placementFromSearch('?audio=loud')).toBeNull();
    expect(placementFromSearch('')).toBeNull();
  });
});

describe('the module rows', () => {
  test('the ids are the module\'s', () => {
    expect([ROW.music, ROW.placement, ROW.motion, ROW.captions, ROW.keys, ROW.back, ROW.sound, ROW.fullscreen]).toEqual([0, 2, 4, 5, 6, 7, 8, 9]);
    expect([ROW.resume, ROW.options, ROW.restart]).toEqual([10, 11, 13]);
  });

  test('pushing and pulling round-trip through the module\'s indexes', () => {
    const mem = new Map<number, number>();
    const app = { menu_value: (r: number) => mem.get(r) ?? 0, set_value: (r: number, i: number) => void mem.set(r, i) };
    pushOptions(app, some);
    expect(mem.get(ROW.sound)).toBe(1);
    expect(mem.get(ROW.motion)).toBe(1);
    expect(mem.get(ROW.keys)).toBe(1);
    expect(pullOptions(app)).toEqual(some);
    pushOptions(app, { ...some, motion: 'full' });
    expect(mem.get(ROW.motion)).toBe(2);
  });
});

describe('keys', () => {
  test('arrows move and change, Enter and Space go forward, Escape goes back', () => {
    expect(menuKeyFor('ArrowUp')).toBe(MENU_KEY.up);
    expect(menuKeyFor('ArrowDown')).toBe(MENU_KEY.down);
    expect(menuKeyFor('ArrowLeft')).toBe(MENU_KEY.left);
    expect(menuKeyFor('ArrowRight')).toBe(MENU_KEY.right);
    expect(menuKeyFor('Enter')).toBe(MENU_KEY.confirm);
    expect(menuKeyFor('Space')).toBe(MENU_KEY.confirm);
    expect(menuKeyFor('Escape')).toBe(MENU_KEY.back);
    expect(menuKeyFor('KeyZ')).toBeNull();
  });

  test('Escape and Pause open Paused', () => {
    expect(opensPaused('Escape')).toBe(true);
    expect(opensPaused('Pause')).toBe(true);
    expect(opensPaused('KeyP')).toBe(false);
  });
});

describe('the look', () => {
  test('the screens use the HUD scale but shrink on a narrow stage', () => {
    expect(optionsScale(1080, 1920)).toBe(4);
    expect(optionsScale(720, 1280)).toBe(3);
    expect(optionsScale(1140, 450)).toBe(3);
    expect(optionsScale(1140, 200)).toBe(1);
    expect(optionsScale(300, 5000)).toBe(2);
  });

  test('the plate shimmers through three shades of the Fizz accent with its ink', () => {
    expect(PLATE).toEqual(['#ff55ff', '#ff77ff', '#ff99ff']);
    expect(PLATE_INK).toBe('#1a001a');
    const css = readFileSync(new URL('./style.css', import.meta.url), 'utf8');
    expect(css).toContain('--fizz: #ff55ff;');
    expect(css).toContain('--plate-b: #ff77ff;');
    expect(css).toContain('--plate-c: #ff99ff;');
    expect(css).toContain(`--plate-ink: ${PLATE_INK};`);
  });

  test('the soda can is 16 by 16 with light magenta bubbles', () => {
    expect(SODA_ROWS.length).toBe(16);
    expect(SODA_ROWS.every((r) => r.length === 16)).toBe(true);
    expect(SODA_ROWS.join('').split('m').length - 1).toBe(2);
    const svg = sodaSvg();
    expect(svg).toContain('viewBox="0 0 16 16"');
    expect(svg).toContain('fill="#ff55ff"');
    expect(svg).toContain('fill="#000000"');
  });

  test('a screen reader hears the label and the value, without the override star', () => {
    expect(spoken('Music', 'Off')).toBe('Music: Off');
    expect(spoken('Placement', 'Classic *')).toBe('Placement: Classic');
    expect(spoken('Back', '')).toBe('Back');
  });
});
