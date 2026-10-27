// Proves that every assets/sprites/*.json parses and serialises back to the same bytes, and that Fizz 1 still regenerates fizz1-slice.json.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/sprites/roundtrip.ts
// Exits non-zero on the first difference. The Fizz 1 comparison runs only when the FIZZ1 environment variable names a Fizz 1
// checkout; otherwise it is skipped with a message.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { exportFizz1 } from './export-fizz1.ts';
import { SPRITES_DIR, parseSet, serialiseSet } from './lib.ts';

const files = readdirSync(SPRITES_DIR)
  .filter((f) => f.endsWith('.json'))
  .sort();
if (files.length === 0) throw new Error(`no sprite files in ${SPRITES_DIR}`);

let failed = false;
for (const f of files) {
  const text = readFileSync(`${SPRITES_DIR}/${f}`, 'utf8');
  const same = serialiseSet(parseSet(text, f)) === text;
  console.log(`${same ? 'ok  ' : 'FAIL'} ${f} round-trips byte for byte`);
  if (!same) failed = true;
}

const fizz1 = process.env.FIZZ1;
if (!fizz1 || !existsSync(fizz1)) {
  console.log('skip fizz1-slice.json equals a fresh export: set FIZZ1 to a Fizz 1 checkout to run it');
} else {
  const same = (await exportFizz1(fizz1)) === readFileSync(`${SPRITES_DIR}/fizz1-slice.json`, 'utf8');
  console.log(`${same ? 'ok  ' : 'FAIL'} fizz1-slice.json equals a fresh export`);
  if (!same) failed = true;
}
process.exit(failed ? 1 : 0);
