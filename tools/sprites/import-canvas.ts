// One-off import of Ben variant A and Cluckzarg from the Episode 2 sprite design canvas into assets/sprites/ep2-*.json.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/sprites/import-canvas.ts <path to the canvas's ep2-sprites.json>
// The canvas file has { palette, transparent, sets, sprites: [{ name, set, label, width, height, rows }] } with the same palette letters
// as Fizz 1 and "." for transparent. The import is provisional art: the committed files carry a note saying so, and once committed they
// are the source; this script documents how they were produced and is not part of any check. The close-up bust (benA_bust) is not
// imported, nor is the aim-up pose, which the first slice does not use.

import { readFileSync, writeFileSync } from 'node:fs';
import { PALETTE, SPRITES_DIR, feetCentre, parseSet, serialiseSet, type Sprite } from './lib.ts';

const path = process.argv[2];
if (!path) throw new Error('usage: bun tools/sprites/import-canvas.ts <ep2-sprites.json>');
const canvas = JSON.parse(readFileSync(path, 'utf8')) as {
  palette: Record<string, string>;
  transparent: string;
  sprites: { name: string; set: string; width: number; height: number; rows: string[] }[];
};
if (JSON.stringify(canvas.palette) !== JSON.stringify(PALETTE) || canvas.transparent !== '.') throw new Error('canvas palette differs from lib.ts');

/** [canvas name, committed name] in file order. */
function take(pairs: [string, string][]): Sprite[] {
  return pairs.map(([from, name]) => {
    const s = canvas.sprites.find((c) => c.name === from);
    if (!s) throw new Error(`canvas has no sprite ${from}`);
    return { name, w: s.width, h: s.height, anchor: feetCentre(s.width, s.height), source: 'generated:ep2-canvas', rows: s.rows };
  });
}

const write = (file: string, set: string, note: string, sprites: Sprite[]): void => {
  const text = serialiseSet({ set, note, sprites });
  parseSet(text, file);
  writeFileSync(`${SPRITES_DIR}/${file}`, text);
};

write(
  'ep2-ben-a.json',
  'ep2-ben-a',
  'Provisional: Ben variant A from the Episode 2 sprite design canvas, imported once by tools/sprites/import-canvas.ts. The art may change.',
  take(
    ['idle', 'run1', 'run2', 'run3', 'run4', 'jump', 'fall', 'pogoUp', 'pogoDown', 'shoot', 'hurt'].map((p): [string, string] => [
      `benA_${p}`,
      `benA_${p}`,
    ]),
  ),
);
write(
  'ep2-cluckzarg.json',
  'ep2-cluckzarg',
  'Provisional: Cluckzarg from the Episode 2 sprite design canvas, imported once by tools/sprites/import-canvas.ts. A placeholder for the final art.',
  take(['idle', 'walk', 'alert'].map((p): [string, string] => [`cluckzarg_${p}`, `cluckzarg_${p}`])),
);
