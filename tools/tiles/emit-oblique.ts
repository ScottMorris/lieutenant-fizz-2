// Writes crates/fizz-slice-view/src/oblique_data.rs, the baked textures and pieces of the oblique ground, or checks that the committed file is current.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   bun tools/tiles/emit-oblique.ts           regenerates the file
//   bun tools/tiles/emit-oblique.ts --check   regenerates in memory; exits non-zero with a diff if the committed file differs
//
// Every texture is a function of world position, tabulated over its period (256 px across; the height is named per table) so the
// renderer copies runs of a row and never hashes at run time. Nothing here reads a file: the output depends only on this source.
// The look is specified in docs/OBLIQUE.md.

import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { LETTERS, ROOT } from '../sprites/lib.ts';

const OUT = `${ROOT}crates/fizz-slice-view/src/oblique_data.rs`;

// ---------- geometry (docs/OBLIQUE.md, "The numbers") ----------

const T = 16; // px per tile
const STRIP = 26; // far edge to near edge
const BACK = 16; // feet line below the far edge
const DX = 10; // depth across
const FEET_DX = 4; // the near plane sits 4 px left of the feet plane
const PLANK_FH = 6; // the plank fascia: the rest of the tile under the near edge
const PERIOD = 256;

/** The x offset of strip row r (0 at the far edge, 25 at the near edge) from the near edge: round((25 - r) * 10 / 25). */
export function o(r: number): number {
  return Math.floor(((STRIP - 1 - r) * DX + Math.floor((STRIP - 1) / 2)) / (STRIP - 1));
}

/** A 32-bit integer hash of three small integers; a port of the design prototype's. */
export function hsh(x: number, y: number, s = 0): number {
  let h = (Math.imul(x, 374761393) + Math.imul(y, 668265263) + Math.imul(s, 2246822519)) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0;
  return (h ^ (h >>> 16)) & 0xffff;
}

const mod = (a: number, n: number) => ((a % n) + n) % n;
const idx = (c: string) => {
  const i = LETTERS.indexOf(c);
  if (i < 0) throw new Error(`no palette letter ${c}`);
  return i;
};

// ---------- masonry joints: block widths of 12 to 40 px that add up to the 256 px period ----------

const edgeCache = new Map<string, Set<number>>();

function courseEdges(course: number, salt: number): Set<number> {
  const key = `${course},${salt}`;
  let e = edgeCache.get(key);
  if (!e) {
    e = new Set<number>();
    const widths = [12, 16, 20, 24, 16, 28];
    let x = 0;
    const start = hsh(course, salt, 1) % 16;
    for (;;) {
      e.add(mod(x - start, PERIOD));
      const w = widths[hsh(course, x, salt) % 6];
      // The last block takes whatever is left, so the pattern closes on itself at the period.
      if (x + w > PERIOD - 12) break;
      x += w;
    }
    edgeCache.set(key, e);
  }
  return e;
}

function joint(u: number, course: number, salt: number): [boolean, boolean] {
  const e = courseEdges(course, salt);
  return [e.has(mod(u, PERIOD)), e.has(mod(u - 1, PERIOD))];
}

// ---------- the textures: u is the world x, v the row of the surface; each returns a palette letter ----------

/** Stone masonry on a 16 px wall face. `d` is the tiles to the nearest opening: 1 and 2 are ashlar (2 darkening), 3 is deep rock. */
function stoneFront(u: number, y: number, d: number): string {
  const course = Math.floor(y / 8);
  const row = y % 8;
  const [left, after] = joint(u, course, 3);
  if (d >= 3) {
    if (row === 7 || left) return hsh(u, y, 4) % 3 ? 'D' : 'k';
    const v = hsh(Math.floor(u / 32), Math.floor(y / 32), 6);
    if (v % 3 === 0 && (u + y + v) % 32 === 0) return 'B'; // thin diagonal veins
    if (v % 5 === 0 && mod(u - y + v, 32) === 0) return 'D';
    return 'k';
  }
  if (row === 7 || left) return 'k';
  if (row === 0 || after) return 'L'; // lit top and left rims
  if (row === 6) return (u + y) % 2 ? 'k' : 'D'; // shaded underside
  if (d === 2 && (u + y) % 2) return 'k'; // darkening towards the deep rock
  if (hsh(u, y, 5) % 23 === 0) return 'L';
  if (hsh(Math.floor(u / 2), Math.floor(y / 2), 8) % 37 === 0) return 'k';
  return 'D';
}

function stoneTop(u: number, v: number): string {
  if (v === 0) return 'D';
  if (v === STRIP - 1) return 'W'; // the lit edge: every solid top reads
  if (v === STRIP - 2) return 'L';
  const slab = v < 9 ? 0 : v < 17 ? 1 : 2;
  if (v === 8 || v === 16) return 'D';
  const [left, after] = joint(u, slab + 50, 7);
  if (left) return 'D';
  if (after || v === 1 || v === 9 || v === 17) return (u + v) % 2 ? 'L' : hsh(u, v, 2) % 3 === 0 ? 'W' : 'L';
  if (hsh(u, v, 11) % 13 === 0) return 'D';
  return 'L';
}

function brickFront(u: number, y: number): string {
  const course = Math.floor(y / 4);
  if (y % 4 === 3) return 'k';
  const off = (course % 2) * 4;
  if ((u + off) % 8 === 7) return 'k';
  if (y % 4 === 0 || (u + off) % 8 === 0) return 'r';
  if (hsh(Math.floor(((u + off) % PERIOD) / 8), course, 12) % 7 === 0) return (u + y) % 2 ? 'R' : 'k'; // a few darker bricks
  return 'R';
}

function plankTop(u: number, v: number): string {
  if (v === 0) return 'k';
  if (v === STRIP - 1) return 'y'; // the nosing: the one-way cue
  const seams = [6, 12, 18];
  if (seams.includes(v)) return 'k';
  const b = seams.filter((s) => v > s).length;
  if ((u + b * 13) % 32 === 0) return 'k';
  if ((u + b * 13) % 32 === 1) return v - [0, 7, 13, 19][b] === 0 ? 'y' : 'N';
  // Grain: long R streaks along the board.
  if (hsh(Math.floor(((u + b * 7) % PERIOD) / 8), v, b) % 6 === 0 && u % 4) return 'R';
  if (hsh(u, v, 17) % 61 === 0) return 'k'; // a nail or a knot
  return 'N';
}

function plankFront(u: number, k: number): string {
  if (k === 0) return 'N';
  if (k === PLANK_FH - 1) return 'k';
  if (u % 32 === 0) return 'k';
  if (k === PLANK_FH - 2) return 'R';
  return hsh(Math.floor(u / 4), k, 3) % 4 === 0 ? 'R' : 'N';
}

/** Vertical 8 px B boards with k seams, board ends, grain and D knots; the period is 256 across and 96 down. */
function backWall(x: number, y: number): string {
  const bx = Math.floor(x / 8);
  if (x % 8 === 7) return 'k';
  const end = (y + (hsh(bx, 0, 3) % 48)) % 48;
  if (end === 0) return 'k'; // board ends
  if (end === 1) return x % 8 < 3 ? 'D' : 'B';
  if (x % 8 === 0) return 'B';
  const gx = 1 + (hsh(bx, Math.floor(y / 8), 9) % 6);
  if (x % 8 === gx && hsh(bx, Math.floor(y / 8), 5) % 4 === 0 && y % 8 < 2 + (hsh(bx, Math.floor(y / 8), 6) % 5)) return 'k'; // grain
  const kx = 2 + (hsh(bx, Math.floor(y / 48), 2) % 3);
  const ky = hsh(bx, Math.floor(y / 48), 3) % 44;
  if (hsh(bx, Math.floor(y / 48), 4) % 3 === 0 && (x % 8 === kx || x % 8 === kx + 1) && ky <= y % 48 && y % 48 < ky + 2) return 'D'; // knot
  if (x % 8 === 6) return y % 2 ? 'k' : 'B'; // board shade
  return 'B';
}

// ---------- tables ----------

function table(rows: number, f: (u: number, v: number) => string): Uint8Array {
  const t = new Uint8Array(PERIOD * rows);
  for (let v = 0; v < rows; v++) for (let u = 0; u < PERIOD; u++) t[v * PERIOD + u] = idx(f(u, v));
  return t;
}

// ---------- pieces: small sprites with 255 for transparent, placed relative to a tile ----------

interface Piece {
  w: number;
  h: number;
  ox: number;
  oy: number;
  pixels: Uint8Array;
}

class Canvas {
  private px = new Map<string, string>();
  put(x: number, y: number, c: string | null) {
    if (c) this.px.set(`${x},${y}`, c);
  }
  get(x: number, y: number): string | undefined {
    return this.px.get(`${x},${y}`);
  }
  /** The bounding box of what was drawn, as a piece whose `(ox, oy)` is the box's top left in canvas coordinates. */
  piece(): Piece {
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const k of this.px.keys()) {
      const [x, y] = k.split(',').map(Number);
      x0 = Math.min(x0, x);
      y0 = Math.min(y0, y);
      x1 = Math.max(x1, x);
      y1 = Math.max(y1, y);
    }
    const w = x1 - x0 + 1;
    const h = y1 - y0 + 1;
    const pixels = new Uint8Array(w * h).fill(255);
    for (const [k, c] of this.px) {
      const [x, y] = k.split(',').map(Number);
      pixels[(y - y0) * w + (x - x0)] = idx(c);
    }
    return { w, h, ox: x0, oy: y0, pixels };
  }
}

/** One 16 by 16 slice of a 48 px door: boards, iron bands, a yellow lock and, on the top piece, the shape badge. */
function doorPiece(red: boolean, which: 0 | 1 | 2): Piece {
  const [light, dark] = red ? ['r', 'R'] : ['b', 'B'];
  const total = 48;
  const cv = new Canvas();
  for (let k = 0; k < T; k++) {
    const yy = which * T + k;
    for (let xx = 0; xx < T; xx++) {
      let c = light;
      if (xx === 0 || xx === T - 1 || yy === 0 || yy === total - 1) c = 'k';
      else if (xx === 1 || xx === T - 2 || yy === 1 || yy === total - 2) c = 'D';
      else if (xx === 5 || xx === 10) c = dark;
      else if (yy === 8 || yy === 9 || yy === total - 10 || yy === total - 9) c = yy % 2 === 0 ? 'D' : 'k';
      else if (total / 2 - 3 <= yy && yy <= total / 2 + 3 && xx >= 9 && xx <= 13) c = !(xx === 11 && total / 2 - 1 <= yy && yy <= total / 2 + 1) ? 'y' : 'k';
      else if (hsh(xx, yy, 8) % 13 === 0) c = dark;
      cv.put(xx, k, c);
    }
  }
  if (which === 0) {
    // The shape badge on a k plate: a W triangle for red, a W disc for blue, so colour is never the only cue.
    for (let yy = 0; yy < 9; yy++) for (let xx = 0; xx < 9; xx++) cv.put(4 + xx - 1, 3 + yy - 1, 'k');
    for (let yy = 0; yy < 7; yy++)
      for (let xx = 0; xx < 7; xx++) {
        const on = red ? Math.abs(xx - 3) <= Math.floor(yy / 2) && yy >= 1 : (xx - 3) ** 2 + (yy - 3) ** 2 <= 10;
        if (on) cv.put(4 + xx, 3 + yy, 'W');
      }
  }
  return cv.piece();
}

/** Two rows of two spikes standing on the floor strip, relative to the left edge and the top of the tile: a shaded back row, a lit front row. */
function spikesPiece(): Piece {
  const cv = new Canvas();
  const yfeet = T;
  for (const [rowR, shaded] of [
    [6, true],
    [20, false],
  ] as const) {
    const base = yfeet - BACK + rowR;
    for (let i = 0; i < 2; i++) {
      const cx = -FEET_DX + o(rowR) + 4 + i * 8;
      for (let k = 0; k < 11; k++) {
        const half = Math.floor(k / 3);
        const Y = base - 10 + k;
        for (let xx = -half - 1; xx < half + 2; xx++) {
          let c: string;
          if (Math.abs(xx) === half + 1) c = 'k';
          else if (xx < 0) c = shaded ? 'D' : 'L';
          else if (xx === 0) c = shaded ? 'L' : 'W';
          else c = shaded ? 'k' : 'D';
          cv.put(cx + xx, Y, c);
        }
      }
      for (let xx = -5; xx < 6; xx++) cv.put(cx + xx, base + 1, 'k');
    }
  }
  return cv.piece();
}

/** The two-tile exit doorway on the far edge, relative to the left edge and the top of the lower exit cell: a stone arch round a lit g opening with a W up arrow. */
function exitPiece(): Piece {
  const cv = new Canvas();
  const xl = DX - FEET_DX;
  const base = 0; // the sill: the far edge of the floor, the top of the lower exit cell
  const w = 16;
  const h = 32;
  for (let k = 0; k < h + 4; k++) {
    const Y = base - h - 4 + k;
    for (let xx = -2; xx < w + 2; xx++) {
      const X = xl + xx;
      const top = k < 8;
      const ax = xx - (w - 1) / 2;
      const ay = k - 9;
      const r2 = ax * ax + ay * ay;
      if (top && r2 > 81) continue;
      const inner = top && r2 > 49;
      const jamb = xx < 1 || xx > w - 2 || inner || k >= h + 3;
      let c: string;
      if (jamb) c = xx === -2 || xx === w + 1 || k >= h + 3 || (top && r2 > 72) ? 'k' : xx < w / 2 ? 'L' : 'D';
      else {
        c = k < h - 6 || (X + Y) % 2 ? 'g' : 'G';
        if (k > h - 3) c = 'G';
      }
      cv.put(X, Y, c);
    }
  }
  const ax0 = xl + 8;
  const ay0 = base - h + 6;
  for (let i = 0; i < 5; i++) for (let xx = -i; xx <= i; xx++) cv.put(ax0 + xx, ay0 + i, 'W');
  for (let k = 5; k < 12; k++) for (let dx = -1; dx <= 1; dx++) cv.put(ax0 + dx, ay0 + k, 'W');
  for (let xx = -5; xx < 6; xx++) if (cv.get(ax0 + xx, ay0 - 1) === 'g') cv.put(ax0 + xx, ay0 - 1, 'k');
  return cv.piece();
}

/** One tile of the gallery railing, relative to the left edge of the tile and the top of the plank cell below it. */
function railPiece(): Piece {
  const cv = new Canvas();
  const y = -BACK;
  for (let xx = 0; xx < T; xx++) {
    const X = DX - FEET_DX + xx;
    cv.put(X, y - 13, 'k');
    cv.put(X, y - 12, 'L');
    cv.put(X, y - 11, 'D');
    cv.put(X, y - 10, 'k');
    if (xx % 8 === 2) {
      for (let k = -13; k < 0; k++) {
        cv.put(X, y + k, 'k');
        cv.put(X + 1, y + k, 'L');
        cv.put(X + 2, y + k, 'D');
        cv.put(X + 3, y + k, 'k');
      }
    }
  }
  return cv.piece();
}

/** An iron bracket under a plank shelf, relative to its leg's second column and the bottom edge of the plank cell. */
function bracketPiece(): Piece {
  const cv = new Canvas();
  for (let k = 0; k < 10; k++) {
    cv.put(-1, k, 'k');
    cv.put(0, k, 'L');
    cv.put(1, k, 'D');
    cv.put(2, k, 'k');
  }
  for (let k = 0; k < 7; k++) {
    cv.put(-2 - k, 8 - k, 'k');
    cv.put(-1 - k, 8 - k, 'D');
  }
  cv.put(0, 10, 'k');
  cv.put(1, 10, 'k');
  return cv.piece();
}

/** A round window in the back wall, relative to its centre: a D and L iron ring round a night sky with a few stars. */
function portholePiece(): Piece {
  const cv = new Canvas();
  for (let yy = -9; yy < 10; yy++)
    for (let xx = -9; xx < 10; xx++) {
      const d = xx * xx + yy * yy;
      if (d > 81) continue;
      let c: string;
      if (d > 64) c = 'k';
      else if (d > 42) c = xx + yy < -2 ? 'L' : 'D';
      else if (d > 36) c = 'k';
      else {
        c = 'k';
        if (yy > 2) c = (xx + yy) % 2 ? 'B' : 'k';
        if (yy > 4) c = yy === 5 && hsh(Math.floor((xx + 9) / 3), 0, 1) % 2 ? 'C' : 'B';
        if ((xx === -2 && yy === -3) || (xx === 3 && yy === -1) || (xx === -3 && yy === -4)) c = 'W';
      }
      cv.put(xx, yy, c);
    }
  return cv.piece();
}

/**
 * The right side face of a prism, as a piece relative to the near plane's right edge and the strip's far edge (the left edge of the
 * tile plus 12, and the top of the tile less 16). Strip row `r` puts a column `O[r]` pixels right of the near edge and hangs
 * `h + 1` pixels below `r`; later rows paint over earlier ones, as the painter's order of a slanted edge goes.
 */
function sidePiece(h: number, tex: (a: number, b: number) => string): Piece {
  const cv = new Canvas();
  for (let r = 0; r < STRIP; r++) for (let k = 0; k <= h; k++) cv.put(o(r), r + k, k === h || (r === 0 && k < h) ? 'k' : tex(r, k));
  return cv.piece();
}

// ---------- the Rust file ----------

function bytes(t: Uint8Array, perLine: number, indent: string): string {
  const lines: string[] = [];
  for (let i = 0; i < t.length; i += perLine) lines.push(`${indent}${Array.from(t.subarray(i, i + perLine)).join(',')},`);
  return lines.join('\n');
}

function tableItem(name: string, doc: string, rows: number, t: Uint8Array): string {
  return `/// ${doc}\npub static ${name}: [u8; ${PERIOD * rows}] = [\n${bytes(t, PERIOD, '    ')}\n];\n`;
}

function pieceItem(name: string, doc: string, p: Piece): string {
  return `/// ${doc}\npub static ${name}: Piece = Piece {\n    w: ${p.w},\n    h: ${p.h},\n    ox: ${p.ox},\n    oy: ${p.oy},\n    pixels: &[\n${bytes(p.pixels, p.w, '        ')}\n    ],\n};\n`;
}

/** A side face as packed rows: each row of the piece has one run of opaque pixels, stored as (first column, length, offset into the packed pixels). */
function sideItem(name: string, doc: string, p: Piece): string {
  const spans: string[] = [];
  const packed: number[] = [];
  for (let y = 0; y < p.h; y++) {
    const row = p.pixels.subarray(y * p.w, (y + 1) * p.w);
    const a = row.findIndex((c) => c !== 255);
    let b = row.length;
    while (b > 0 && row[b - 1] === 255) b--;
    if (a < 0) throw new Error(`${name}: row ${y} is empty`);
    for (let x = a; x < b; x++) if (row[x] === 255) throw new Error(`${name}: row ${y} has a hole`);
    spans.push(`(${a}, ${b - a}, ${packed.length})`);
    for (let x = a; x < b; x++) packed.push(row[x]);
  }
  const pixels = bytes(Uint8Array.from(packed), 32, '        ');
  return `/// ${doc}\npub static ${name}: Side = Side {\n    h: ${p.h},\n    ox: ${p.ox},\n    oy: ${p.oy},\n    spans: &[${spans.join(', ')}],\n    pixels: &[\n${pixels}\n    ],\n};\n`;
}

export function emit(): string {
  const items: string[] = [];
  items.push(
    `/// Strip row \`r\` (0 at the far edge, 25 at the near edge) is shifted \`O[r]\` pixels right of the near edge: \`round((25 - r) * 10 / 25)\`.\npub static O: [u8; ${STRIP}] = [${Array.from({ length: STRIP }, (_, r) => o(r)).join(', ')}];\n`,
  );
  items.push(tableItem('PLANK_TOP', 'The top of a one-way plank: 26 rows (far edge to y nosing) of boards with R grain and k joints.', STRIP, table(STRIP, plankTop)));
  items.push(tableItem('PLANK_FRONT', 'The 6 px fascia of a plank.', PLANK_FH, table(PLANK_FH, plankFront)));
  items.push(tableItem('STONE_TOP', 'The top of stone, brick and doors: 26 rows of L flagstones with a W lit edge.', STRIP, table(STRIP, stoneTop)));
  items.push(tableItem('STONE_SHALLOW', 'Stone wall face within one tile of an opening: 64 rows (four tiles) of D ashlar with L rims and k mortar.', 64, table(64, (u, y) => stoneFront(u, y, 1))));
  items.push(tableItem('STONE_MID', 'Stone wall face two tiles from an opening: ashlar darkening towards the deep rock.', 64, table(64, (u, y) => stoneFront(u, y, 2))));
  items.push(tableItem('STONE_DEEP', 'Stone wall face three or more tiles from an opening: k rock with D joints and thin B veins.', 64, table(64, (u, y) => stoneFront(u, y, 3))));
  items.push(tableItem('BRICK_FRONT', 'Brick wall face: 32 rows of R bands with r lit tops and left edges.', 32, table(32, brickFront)));
  items.push(tableItem('BACK_WALL', 'The back wall of a room: 96 rows of vertical B boards with k seams, grain and D knots.', 96, table(96, backWall)));
  items.push(pieceItem('DOOR_RED_TOP', 'The red door, top 16 px, with the white triangle badge. A door piece is a 16 by 16 front face; `(ox, oy)` is zero.', doorPiece(true, 0)));
  items.push(pieceItem('DOOR_RED_MID', 'The red door, a middle 16 px, with the lock.', doorPiece(true, 1)));
  items.push(pieceItem('DOOR_RED_BOT', 'The red door, bottom 16 px.', doorPiece(true, 2)));
  items.push(pieceItem('DOOR_BLUE_TOP', 'The blue door, top 16 px, with the white disc badge.', doorPiece(false, 0)));
  items.push(pieceItem('DOOR_BLUE_MID', 'The blue door, a middle 16 px, with the lock.', doorPiece(false, 1)));
  items.push(pieceItem('DOOR_BLUE_BOT', 'The blue door, bottom 16 px.', doorPiece(false, 2)));
  items.push(pieceItem('EXIT_DOORWAY', 'The exit doorway; `(ox, oy)` is relative to the left edge and the top of the lower exit cell.', exitPiece()));
  items.push(pieceItem('SPIKES', 'A spike tile; `(ox, oy)` is relative to the left edge and the top of the tile.', spikesPiece()));
  items.push(pieceItem('RAIL', 'One tile of railing; `(ox, oy)` is relative to the left edge and the top of the plank cell it stands on.', railPiece()));
  items.push(pieceItem('BRACKET', 'An iron bracket under a plank; `(ox, oy)` is relative to its leg and the bottom edge of the plank cell.', bracketPiece()));
  items.push(pieceItem('PORTHOLE', 'A porthole; `(ox, oy)` is relative to its centre.', portholePiece()));
  const odd = (dark: string) => (a: number, b: number) => ((a + b) % 2 ? 'k' : dark);
  const side = 'relative to the near plane\'s right edge and the strip\'s far edge';
  items.push(sideItem('SIDE_STONE', `The right side face of a stone prism, 16 px tall; \`(ox, oy)\` is ${side}.`, sidePiece(T, odd('D'))));
  items.push(sideItem('SIDE_BRICK', 'The right side face of a brick block, 16 px tall.', sidePiece(T, odd('R'))));
  items.push(sideItem('SIDE_PLANK', 'The right side face of a plank, 5 px tall.', sidePiece(PLANK_FH - 1, odd('R'))));
  items.push(sideItem('SIDE_DOOR_RED', 'The right side face of the red door, 16 px tall.', sidePiece(T, odd('R'))));
  items.push(sideItem('SIDE_DOOR_BLUE', 'The right side face of the blue door, 16 px tall.', sidePiece(T, odd('B'))));
  return `// Generated EGA-indexed textures and pieces for the oblique ground; do not edit.
//
// Generated by tools/tiles/emit-oblique.ts. Regenerate with \`bun tools/tiles/emit-oblique.ts\`; \`--check\` fails if this file is out of date.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The width of every table in pixels. A table row repeats every 256 pixels of world x.
pub const PERIOD: usize = ${PERIOD};

/// A small picture of EGA indices 0..=15, with 255 for transparent, row by row from the top left. \`(ox, oy)\` is where its top left lies
/// from the reference point the piece's own documentation names.
pub struct Piece {
    pub w: u8,
    pub h: u8,
    pub ox: i8,
    pub oy: i8,
    pub pixels: &'static [u8],
}

/// A right side face, every row of it one run of opaque pixels: for each row, \`(first column, length, offset into pixels)\`. The
/// packed \`pixels\` hold only the opaque pixels, row after row. \`(ox, oy)\` is as for a [\`Piece\`].
pub struct Side {
    pub h: u8,
    pub ox: i8,
    pub oy: i8,
    pub spans: &'static [(u8, u8, u16)],
    pub pixels: &'static [u8],
}

${items.join('\n')}`;
}

if (import.meta.main) {
  const text = emit();
  if (process.argv.includes('--check')) {
    let current = '';
    try {
      current = readFileSync(OUT, 'utf8');
    } catch {
      // A missing file differs from any output.
    }
    if (current !== text) {
      const dir = mkdtempSync(`${tmpdir()}/oblique-check-`);
      writeFileSync(`${dir}/expected.rs`, text);
      const d = spawnSync('diff', ['-u', OUT, `${dir}/expected.rs`], { encoding: 'utf8' });
      process.stderr.write((d.stdout ?? '').split('\n').slice(0, 60).join('\n'));
      console.error('\ncrates/fizz-slice-view/src/oblique_data.rs is out of date; run: bun tools/tiles/emit-oblique.ts');
      process.exit(1);
    }
    console.log('oblique_data.rs: up to date');
  } else {
    writeFileSync(OUT, text);
  }
}
