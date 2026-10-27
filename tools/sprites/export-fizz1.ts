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
  'ben_pogo',
  'ben_pogo2',
  'keyRed',
  'doorRed',
  'bubble',
];

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
    return { name, w: g.w, h: g.h, anchor: feetCentre(g.w, g.h), source: 'generated:fizz1', rows };
  });
  return serialiseSet({ set: 'fizz1-slice', sprites });
}

if (import.meta.main) {
  const text = await exportFizz1(process.env.FIZZ1 ?? DEFAULT_FIZZ1);
  if (process.argv.includes('--stdout')) process.stdout.write(text);
  else writeFileSync(`${SPRITES_DIR}/fizz1-slice.json`, text);
}
