// Extracts Fizz 1's own DOS sound effects (jump, boing and fzzt) from the Fizz 1 DOS port's sound blob into assets/sfx/fizz1-dos.ts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/sfx/extract-fizz1-dos.ts path/to/fizz-dos/m4/assets/sound.bin
//
// The blob is `m4/assets/sound.bin` of ScottMorris/lieutenant-fizz-dos at commit 221dd6c (MIT there, the same author; dual-licensed here), written by
// that repository's `m4/export-sound.ts` from its `convertSfx`. Its layout is documented in that file: an effect table of
// `{ u32 oplOffset, u16 oplLength, u32 speakerOffset, u16 speakerFrames }`, an IMF chunk (a 16-bit length, then four bytes a write:
// register, value, and the delay in 1/560 s before the next) and u16 speaker divisors at 140 Hz. The effects are in the order of the
// keys of that port's `SFX` table, whose first three are jump, boing and fzzt. This tool is run by hand when the port's data changes;
// `downmix.ts` reads the generated file and needs neither the blob nor the port.

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const OUT = fileURLToPath(new URL('../../assets/sfx/fizz1-dos.ts', import.meta.url));

/** The port's effects we take: its table index, our name and the Fizz 2 effect id it plays as, with the length it must have in 560 Hz ticks. */
const TAKEN = [
  { index: 0, name: 'jump', id: 13, ticks: 104 },
  { index: 1, name: 'boing', id: 12, ticks: 112 },
  { index: 2, name: 'fzzt', id: 4, ticks: 96 },
] as const;

const blobPath = process.argv[2];
if (!blobPath) throw new Error('usage: bun tools/sfx/extract-fizz1-dos.ts path/to/fizz-dos/m4/assets/sound.bin');
const blob = readFileSync(blobPath);
if (blob.subarray(0, 4).toString('latin1') !== 'FZSN' || blob[4] !== 1) throw new Error('not a version 1 FZSN sound blob');
const songs = blob[5];
const table = 9 + songs * 8;

const rows: string[] = [];
for (const t of TAKEN) {
  const at = table + t.index * 12;
  const oplOffset = blob.readUInt32LE(at);
  const speakerOffset = blob.readUInt32LE(at + 6);
  const frames = blob.readUInt16LE(at + 10);
  const payload = blob.readUInt16LE(oplOffset);
  const records: string[] = [];
  let ticks = 0;
  for (let i = 0; i < payload; i += 4) {
    const p = oplOffset + 2 + i;
    const wait = blob[p + 2] | (blob[p + 3] << 8);
    ticks += wait;
    records.push(`[0x${blob[p].toString(16).padStart(2, '0')}, 0x${blob[p + 1].toString(16).padStart(2, '0')}, ${wait}]`);
  }
  if (ticks !== t.ticks) throw new Error(`${t.name}: ${ticks} ticks, expected ${t.ticks}: is this the port's 221dd6c blob?`);
  const divisors: number[] = [];
  for (let i = 0; i < frames; i++) divisors.push(blob.readUInt16LE(speakerOffset + i * 2));
  const lines: string[] = [];
  for (let k = 0; k < records.length; k += 6) lines.push(`      ${records.slice(k, k + 6).join(', ')},`);
  const spk: string[] = [];
  for (let k = 0; k < divisors.length; k += 12) spk.push(`      ${divisors.slice(k, k + 12).join(', ')},`);
  rows.push(`  // ${t.name}: ${records.length} records, ${ticks} ticks, ${frames} speaker frames.
  ${t.id}: {
    name: '${t.name}',
    ticks: ${ticks},
    records: [
${lines.join('\n')}
    ],
    divisors: [
${spk.join('\n')}
    ],
  },`);
}

writeFileSync(
  OUT,
  `// Fizz 1's own DOS sound effects for jump, boing and fzzt, as its DOS port converted them. Generated; do not edit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: the OPL2 register writes and the PC speaker divisors are the entries 0 (jump), 1 (boing) and 2 (fzzt) of \`m4/assets/sound.bin\`
// in ScottMorris/lieutenant-fizz-dos at commit 221dd6c, which that repository's \`convertSfx\` (\`m4/sound/fizz-dos/sfx.ts\`) wrote from
// Fizz 1's plain Classic voices (\`SFX\` in \`episodes/episode-1/src/audio/patterns.ts\`). The same author's data, MIT there and dual-licensed
// here. Regenerate with \`bun tools/sfx/extract-fizz1-dos.ts <path to that blob>\`.
//
// Loader note: \`tools/sfx/downmix.ts\` uses these in place of its own converter's output for the effect ids that are the keys here, and the
// engine plays them like any other effect. A record is [register, value, ticks to wait after it] on channel 5 of bank 0 (the same layout as
// \`OplStep\`, with the bank always 0), the speaker list is one divisor per 140 Hz frame with 0 for gate off, and \`ticks\` is the chunk
// length in 560 Hz ticks. The Web plays Fizz 1's Enhanced voices for these ids; DOS plays Fizz 1's DOS voices (see docs/SOUND.md).

export interface Fizz1Dos {
  name: string;
  ticks: number;
  records: readonly (readonly [number, number, number])[];
  divisors: readonly number[];
}

/** Fizz 1's DOS data by the Fizz 2 effect id it plays as. */
export const FIZZ1_DOS: Readonly<Record<number, Fizz1Dos>> = {
${rows.join('\n')}
};
`,
);
console.log(`wrote ${OUT}`);
