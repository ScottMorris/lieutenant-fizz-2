// Shared reader, canonical writer and palette for the sprite grid files under assets/sprites/.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The format is specified in docs/SPRITES.md. Every tool reads and writes grids through this module so the bytes are canonical.

/** The EGA palette letters in index order 0..15, as Fizz 1's `EGA` table in packages/engine/src/palette.ts. */
export const PALETTE: Record<string, string> = {
  k: '#000000',
  B: '#0000aa',
  G: '#00aa00',
  C: '#00aaaa',
  R: '#aa0000',
  M: '#aa00aa',
  N: '#aa5500',
  L: '#aaaaaa',
  D: '#555555',
  b: '#5555ff',
  g: '#55ff55',
  c: '#55ffff',
  r: '#ff5555',
  m: '#ff55ff',
  y: '#ffff55',
  W: '#ffffff',
};
export const LETTERS = Object.keys(PALETTE).join('');
export const TRANSPARENT = '.';

export const SOURCES = ['generated:fizz1', 'generated:ep2-canvas', 'drawn'] as const;
export type Source = (typeof SOURCES)[number];

export interface Sprite {
  name: string;
  w: number;
  h: number;
  anchor: [number, number];
  source: Source;
  rows: string[];
}
export interface SpriteSet {
  set: string;
  note?: string;
  sprites: Sprite[];
}

function fail(where: string, msg: string): never {
  throw new Error(`${where}: ${msg}`);
}

/** Parses and validates the text of a sprite set file. Keys other than the specified ones are errors. */
export function parseSet(text: string, where: string): SpriteSet {
  const j = JSON.parse(text) as Record<string, unknown>;
  for (const k of Object.keys(j)) if (!['set', 'note', 'sprites'].includes(k)) fail(where, `unknown key ${k}`);
  if (typeof j.set !== 'string' || j.set === '') fail(where, 'set must be a non-empty string');
  if (j.note !== undefined && typeof j.note !== 'string') fail(where, 'note must be a string');
  if (!Array.isArray(j.sprites)) fail(where, 'sprites must be an array');
  const seen = new Set<string>();
  const sprites = (j.sprites as Record<string, unknown>[]).map((s, i) => {
    const at = `${where} sprite ${i}`;
    for (const k of Object.keys(s)) if (!['name', 'w', 'h', 'anchor', 'source', 'rows'].includes(k)) fail(at, `unknown key ${k}`);
    const { name, w, h, anchor, source, rows } = s as { name: unknown; w: unknown; h: unknown; anchor: unknown; source: unknown; rows: unknown };
    if (typeof name !== 'string' || !/^[A-Za-z0-9_]+$/.test(name)) fail(at, 'name must match [A-Za-z0-9_]+');
    if (seen.has(name)) fail(at, `duplicate name ${name}`);
    seen.add(name);
    const here = `${at} ${name}`;
    if (!Number.isInteger(w) || !Number.isInteger(h) || (w as number) < 1 || (w as number) > 255 || (h as number) < 1 || (h as number) > 255)
      fail(here, 'w and h must be integers in 1..255');
    if (!Array.isArray(anchor) || anchor.length !== 2 || !anchor.every((v) => Number.isInteger(v) && v >= -128 && v <= 127))
      fail(here, 'anchor must be [x, y] integers in -128..127');
    if (!SOURCES.includes(source as Source)) fail(here, `source must be one of ${SOURCES.join(', ')}`);
    if (!Array.isArray(rows) || rows.length !== h) fail(here, 'rows must have h entries');
    for (const r of rows) {
      if (typeof r !== 'string' || r.length !== w) fail(here, 'every row must be a string of w characters');
      for (const c of r) if (c !== TRANSPARENT && !(c in PALETTE)) fail(here, `bad palette letter ${c}`);
    }
    return { name, w: w as number, h: h as number, anchor: anchor as [number, number], source: source as Source, rows: rows as string[] };
  });
  return { set: j.set, ...(j.note !== undefined ? { note: j.note as string } : {}), sprites };
}

/** The canonical text: fixed key order, 2-space indentation, one row per line, one-line anchor, final newline. */
export function serialiseSet(s: SpriteSet): string {
  const out: string[] = ['{', `  "set": ${JSON.stringify(s.set)},`];
  if (s.note !== undefined) out.push(`  "note": ${JSON.stringify(s.note)},`);
  if (s.sprites.length === 0) {
    out.push('  "sprites": []');
  } else {
    out.push('  "sprites": [');
    s.sprites.forEach((p, i) => {
      out.push('    {');
      out.push(`      "name": ${JSON.stringify(p.name)},`);
      out.push(`      "w": ${p.w},`);
      out.push(`      "h": ${p.h},`);
      out.push(`      "anchor": [${p.anchor[0]}, ${p.anchor[1]}],`);
      out.push(`      "source": ${JSON.stringify(p.source)},`);
      out.push('      "rows": [');
      p.rows.forEach((r, k) => out.push(`        ${JSON.stringify(r)}${k < p.rows.length - 1 ? ',' : ''}`));
      out.push('      ]');
      out.push(`    }${i < s.sprites.length - 1 ? ',' : ''}`);
    });
    out.push('  ]');
  }
  out.push('}');
  return out.join('\n') + '\n';
}

/** The default anchor: horizontal centre and the bottom edge (feet). */
export function feetCentre(w: number, h: number): [number, number] {
  return [Math.floor(w / 2), h];
}

export const ROOT = new URL('../../', import.meta.url).pathname;
export const SPRITES_DIR = `${ROOT}assets/sprites`;
