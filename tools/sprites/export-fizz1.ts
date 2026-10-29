// Exports the Fizz 1 sprites that the first slice needs to assets/sprites/fizz1-slice.json, reading Fizz 1 and changing nothing in it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/sprites/export-fizz1.ts            writes assets/sprites/fizz1-slice.json
//   bun tools/sprites/export-fizz1.ts --stdout   prints the file instead
// FIZZ1 names the Fizz 1 checkout; it defaults to ~/source/liminal-hq/lieutenant-fizz.

import { writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { PALETTE, SPRITES_DIR, feetCentre, serialiseSet, type Sprite } from './lib.ts';

/** Fizz 1's catalog names for the slice, in file order. Fizz 1 has no dead or hurt pose for Ben. */
export const SLICE_NAMES = [
  'ben_stand',
  'ben_run1',
  'ben_run2',
  'ben_jump',
  'ben_shoot',
  'ben_aimUp',
  'ben_aimDown',
  'ben_lookUp',
  'ben_lookDown',
  'ben_pogo',
  'ben_pogo2',
  'keyRed',
  'doorRed',
  // The snack nook (items.ts `cheezie()`, `choc()`, `cookie()` and `gumdrop('b', 'B')`, tiles `doorTile('b')`).
  'keyBlue',
  'doorBlue',
  'cheezie',
  'choc',
  'cookie',
  'bubble',
  // Fizz 1's soda can (items.ts `soda()`), the menus' cursor bullet.
  'soda',
  // Fizz 1's stun stars (enemies.ts `stars(f)`, drawn by render.rs above a stunned enemy), 16 by 8, two frames.
  'stars0',
  'stars1',
];

/** The baked sizes of the shot-end puff, in the order they play: Fizz 1's 16 by 16 puff grows from 0.5x by 1.5 a second for 0.3 s, so 0.5x to 0.95x. */
export const PUFF_SIZES = [8, 12, 15];
/** Names of the baked puff frames: one per size, then the last size thinned to a dither (Fizz 1 fades the puff out; DOS has no alpha). */
export const PUFF_NAMES = ['puff0', 'puff1', 'puff2', 'puff3'];

/**
 * The Fizz 2 recolour: Fizz 1 draws Ben's shots and puffs in cyan, and Fizz 2's Fizz colour is light magenta (EGA 13, "grape",
 * `crates/fizz-view/src/theme.rs`). The letters below are swapped, by sprite name, after the Fizz 1 grid is read and before it is
 * written, so re-running the exporter gives the same committed file. Light cyan `c` becomes light magenta `m`; cyan `C` and light
 * blue `b` (the shot's body and the puff's core) become magenta `M`. Fizz 1's soda can (`soda()` in sprites/items.ts) has two
 * light cyan bubble pixels and takes the same swap when it joins the slice.
 */
export const FIZZ_RECOLOUR: Record<string, Record<string, string>> = {
  bubble: { c: 'm', b: 'M' },
  puff0: { c: 'm', C: 'M' },
  puff1: { c: 'm', C: 'M' },
  puff2: { c: 'm', C: 'M' },
  puff3: { c: 'm', C: 'M' },
  soda: { c: 'm' },
};

/** Applies [`FIZZ_RECOLOUR`] to a sprite's rows, leaving every other letter alone. */
export function recolour(name: string, rows: string[]): string[] {
  const map = FIZZ_RECOLOUR[name];
  if (!map) return rows;
  return rows.map((r) => [...r].map((ch) => map[ch] ?? ch).join(''));
}

export const DEFAULT_FIZZ1 = `${homedir()}/source/liminal-hq/lieutenant-fizz`;

export async function exportFizz1(fizz1: string): Promise<string> {
  const { defineSprites } = await import(`${fizz1}/episodes/episode-1/src/sprites/catalog`);
  const { EGA } = await import(`${fizz1}/packages/engine/src/palette`);
  if (JSON.stringify(Object.entries(EGA)) !== JSON.stringify(Object.entries(PALETTE)))
    throw new Error('Fizz 1 EGA table differs from tools/sprites/lib.ts PALETTE');
  type Grid = { w: number; h: number; at(x: number, y: number): string | null };
  const defs = new Map<string, Grid>(defineSprites().map((d: { name: string; grid: Grid }) => [d.name, d.grid]));
  const sprites: Sprite[] = SLICE_NAMES.map((name) => {
    const g = defs.get(name);
    if (!g) throw new Error(`Fizz 1 has no sprite named ${name}`);
    const rows: string[] = [];
    for (let y = 0; y < g.h; y++) {
      let r = '';
      for (let x = 0; x < g.w; x++) r += g.at(x, y) ?? '.';
      rows.push(r);
    }
    return { name, w: g.w, h: g.h, anchor: feetCentre(g.w, g.h), source: 'generated:fizz1', rows: recolour(name, rows) };
  });
  // The shot-end puff. Fizz 1's `puff()` (enemies.ts) is a 16 by 16 dithered ring (`W`) round a dithered core (`L`), tinted at
  // run time: cyan 0x55ffff for Ben's shots, which multiplies `W` to light cyan and `L` to about cyan. Baked here for the
  // palette: a smaller drawing of the same two ellipses and conditions at each size in PUFF_SIZES (resampling the 16 by 16 grid
  // would turn its checkerboard into solid or empty), `W` to `c` and `L` to `C`, then the largest thinned to every other pixel.
  const { Pen } = await import(`${fizz1}/packages/engine/src/pen`);
  const bake = (n: number): string[] => {
    const k = n / 16;
    const p = new Pen(n, n);
    const c = (n - 1) / 2;
    p.ell(n / 2, n / 2, 7 * k, 7 * k, 'W', (i: number, j: number) => (i + j) % 2 === 0 && (i - c) ** 2 + (j - c) ** 2 > 12 * k * k);
    p.ell(n / 2, n / 2, 4 * k, 4 * k, 'L', (i: number, j: number) => (i + j) % 3 === 0);
    const rows: string[] = [];
    for (let y = 0; y < n; y++) {
      let r = '';
      for (let x = 0; x < n; x++) r += ({ W: 'c', L: 'C' } as Record<string, string>)[p.at(x, y) ?? ''] ?? '.';
      rows.push(r);
    }
    return rows;
  };
  const puffs = PUFF_SIZES.map(bake);
  const last = puffs[puffs.length - 1];
  puffs.push(last.map((r, y) => [...r].map((ch, x) => (y % 2 === 0 && x % 4 === 0 ? ch : '.')).join('')));
  puffs.forEach((rows, i) => {
    const n = rows.length;
    sprites.push({ name: PUFF_NAMES[i], w: n, h: n, anchor: [n >> 1, n >> 1], source: 'generated:fizz1', rows: recolour(PUFF_NAMES[i], rows) });
  });
  return serialiseSet({ set: 'fizz1-slice', sprites });
}

if (import.meta.main) {
  const text = await exportFizz1(process.env.FIZZ1 ?? DEFAULT_FIZZ1);
  if (process.argv.includes('--stdout')) process.stdout.write(text);
  else writeFileSync(`${SPRITES_DIR}/fizz1-slice.json`, text);
}
