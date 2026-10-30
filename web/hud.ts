// What the Web HUD shows, as pure data: the values, their text, the key tray's drawings, the scale and the announcements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The HUD is Fizz 1's desktop HUD in its look (a dark glass panel, small dim labels beside big Fizz pixel digits, the Fizz number in the
// accent colour and red at zero; Fizz 1's `pillItems`, episodes/episode-1/src/ui/hud.ts) laid out as the DOS HUD is, the Two-Row Stack:
// Fizz and Score on the top row; Deaths and the key tray on the bottom row. The numbers, the score counting up, the `+N` and the
// key rings come from the module (`App.hud_view`, which is `fizz_slice_view::hud`, shared with DOS), so this file only formats them.
// TICK and HASH are not here: they stay in the H panel for inspection. This file touches no DOM, so `hud.test.ts` runs it under Bun;
// web/main.js applies the result to the page.

/** The key kinds, in tray order: the module's `KeyKind` (crates/fizz-slice-view/src/hud.rs). */
export const KEY_KINDS = ['red', 'blue', 'green', 'gold'] as const;
export type KeyKind = (typeof KEY_KINDS)[number];

/** One socket of the tray: whether the level has this kind, how many Ben holds, and whether its ring shows. */
export interface KeyReading {
  inLevel: boolean;
  held: number;
  ring: boolean;
}

/** What the page reads from the module each frame. */
export interface HudValues {
  ammo: number;
  /** The score shown now, which counts up after a snack. */
  score: number;
  /** The points of the snack just taken, or 0. */
  pop: number;
  deaths: number;
  keys: KeyReading[];
}

/** The most a count shows. Fizz 1's number cells hold two digits here, and the panel must not grow. */
export const COUNT_CAP = 99;
/** The score field is five digits, and the display holds at this (the true score continues in the game). */
export const SCORE_DIGITS = 5;
export const SCORE_CAP = 99_999;
/** The most pips under a socket; more of one kind shows as a digit. */
export const MAX_PIPS = 4;

/** Reads the integers of `App.hud_view`: ammo, score, pop, deaths, then in_level, held and ring for each kind. */
export function parseHud(raw: ArrayLike<number>): HudValues {
  const keys: KeyReading[] = KEY_KINDS.map((_, i) => ({
    inLevel: raw[4 + 3 * i] !== 0,
    held: raw[5 + 3 * i],
    ring: raw[6 + 3 * i] !== 0,
  }));
  return { ammo: raw[0], score: raw[1], pop: raw[2], deaths: raw[3], keys };
}

// The Fizz number is light magenta (var(--fizz) in style.css, FIZZ_CSS in fizz-view's theme.rs; Fizz 1 uses cyan), red at zero as in Fizz 1, and the dim label colour is Fizz 1's.
export const RED = '#ff5555';
export const DIM = '#7c8796';

/** A count as the HUD prints it: a whole number from 0 to 99. */
export function formatCount(n: number): string {
  if (!Number.isFinite(n)) return '0';
  return String(Math.min(COUNT_CAP, Math.max(0, Math.trunc(n))));
}

/**
 * Fizz 1's whole-number pixel scale for its overlay (packages/engine/src/font/scale.ts `pixelScale`): 2 under 480 px of height,
 * 3 up to 900 px and 4 above. The Web view is 13 tiles tall and as tall as the window, so this is a rule of the tile's size on
 * screen (37 px, 69 px) just as Fizz 1's is. Fizz is an 11-row pixel font, so a whole scale keeps every glyph pixel square.
 */
export function hudScale(stageHeightPx: number): number {
  return stageHeightPx >= 900 ? 4 : stageHeightPx >= 480 ? 3 : 2;
}

/** The size of one sprite pixel of a tray socket in CSS pixels: the scale, so the 7 by 7 key is as tall as the digits' cap height. */
export function keyPixelSize(scale: number): number {
  return Math.max(2, scale);
}

/** One socket of the tray as it is drawn. */
export interface SocketView {
  kind: KeyKind;
  /** Whether the level has this kind. A socket for a kind it lacks is not shown. */
  shown: boolean;
  held: boolean;
  ring: boolean;
  /** Pips under the socket: 0 for none or one key, 2 to 4 for that many keys. */
  pips: number;
  /** The count over the key when there are more than four, or an empty string. */
  digit: string;
}

/** The HUD as it is drawn. */
export interface HudView {
  fizz: string;
  /** Whether the Fizz number is out, so it is drawn red. */
  fizzOut: boolean;
  /** The score as five digits, split into the padding zeros (dim) and the rest. */
  scoreLead: string;
  scoreRest: string;
  /** `+N` while a snack's points show in the place of the word Score, or an empty string. */
  pop: string;
  deaths: string;
  sockets: SocketView[];
}

/** The score field: five digits, held at 99,999, the leading zeros (all but the last digit) split off as padding. */
export function scoreText(n: number): { lead: string; rest: string } {
  const v = Number.isFinite(n) ? Math.min(SCORE_CAP, Math.max(0, Math.trunc(n))) : 0;
  const digits = String(v).padStart(SCORE_DIGITS, '0');
  const pad = Math.min(SCORE_DIGITS - 1, digits.length - String(v).length);
  return { lead: digits.slice(0, pad), rest: digits.slice(pad) };
}

export function hudView(v: HudValues): HudView {
  const score = scoreText(v.score);
  return {
    fizz: formatCount(v.ammo),
    fizzOut: v.ammo <= 0,
    scoreLead: score.lead,
    scoreRest: score.rest,
    pop: v.pop > 0 ? '+' + String(Math.min(99, Math.trunc(v.pop))) : '',
    deaths: formatCount(v.deaths),
    sockets: KEY_KINDS.map((kind, i) => {
      const k = v.keys[i];
      return {
        kind,
        shown: k.inLevel,
        held: k.held > 0,
        ring: k.ring && k.held > 0,
        pips: k.held >= 2 && k.held <= MAX_PIPS ? k.held : 0,
        digit: k.held > MAX_PIPS ? String(Math.min(9, k.held)) : '',
      };
    }),
  };
}

/** Whether two readings show the same HUD, so the page can leave the DOM alone. Counts past the cap show alike. */
export function sameHud(a: HudValues | null, b: HudValues | null): boolean {
  if (a === null || b === null) return a === b;
  return JSON.stringify(hudView(a)) === JSON.stringify(hudView(b));
}

/**
 * What a screen reader is told when the HUD changes, or null for nothing. Only moments that matter are spoken: the Fizz running
 * out, a key picked up (named by its kind) and a death. The score is not spoken (too chatty, and not an event). Every other change (a
 * shot, a key dropped on a death, a restart) is silent, and so is the first reading. When several happen in one frame they are
 * joined in that order. The current values are always in the H panel.
 */
export function announcement(prev: HudValues | null, next: HudValues): string | null {
  if (prev === null) return null;
  const said: string[] = [];
  if (prev.ammo > 0 && next.ammo <= 0) said.push('Out of Fizz');
  KEY_KINDS.forEach((kind, i) => {
    if (next.keys[i].held > prev.keys[i].held) said.push(kind[0].toUpperCase() + kind.slice(1) + ' key picked up');
  });
  if (next.deaths > prev.deaths) said.push('Deaths ' + formatCount(next.deaths));
  return said.length > 0 ? said.join('. ') : null;
}

/** A rectangle in any one unit: left, top, width, height. */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Whether two boxes come within `margin` of each other. */
export function overlaps(a: Box, b: Box, margin = 0): boolean {
  return a.x < b.x + b.w + margin && b.x < a.x + a.w + margin && a.y < b.y + b.h + margin && b.y < a.y + a.h + margin;
}

// --- The key tray -------------------------------------------------------------------------------------------------------------

/**
 * The four 7 by 7 socket keys of assets/sprites/ep2-hud-keys.json, the grids the DOS tray draws (hud.test.ts keeps them equal). Each
 * has a black mark besides its colour: a dot, a bar, a plus, and the gold one is a USB stick.
 */
export const KEY_ROWS: Record<KeyKind, readonly string[]> = {
  red: ['..kkk..', '.krrrk.', 'krrrrrk', 'krrkrrk', 'krrrrrk', 'krrrrrk', '.kkkkk.'],
  blue: ['..kkk..', '.kbbbk.', 'kbbbbbk', 'kbkkkbk', 'kbbbbbk', 'kbbbbbk', '.kkkkk.'],
  green: ['..kkk..', '.kgggk.', 'kggkggk', 'kgkkkgk', 'kggkggk', 'kgggggk', '.kkkkk.'],
  gold: ['..kkk..', '..kLk..', '.kkkkk.', '.kyyyk.', '.kyyyk.', '.kNNNk.', '.kkkkk.'],
};

// The EGA letters the keys use (tools/sprites/lib.ts PALETTE).
const KEY_COLOURS: Record<string, string> = {
  k: '#000000',
  r: '#ff5555',
  b: '#5555ff',
  g: '#55ff55',
  y: '#ffff55',
  L: '#aaaaaa',
  N: '#aa5500',
};

/** SVG path data covering the pixels for which `pick` returns true, as horizontal runs. */
function runs(rows: readonly string[], pick: (x: number, y: number, c: string) => boolean): string {
  const out: string[] = [];
  rows.forEach((row, y) => {
    for (let x = 0; x < row.length; ) {
      if (!pick(x, y, row[x])) {
        x++;
        continue;
      }
      let end = x;
      while (end < row.length && pick(end, y, row[end])) end++;
      out.push('M' + x + ' ' + y + 'h' + (end - x) + 'v1h-' + (end - x) + 'z');
      x = end;
    }
  });
  return out.join('');
}

/** The pixels of the sprite that touch an empty pixel or the edge: its outline. */
function isEdge(rows: readonly string[], x: number, y: number): boolean {
  const empty = (px: number, py: number) => (rows[py]?.[px] ?? '.') === '.';
  return empty(x - 1, y) || empty(x + 1, y) || empty(x, y - 1) || empty(x, y + 1);
}

/** A key as a 7 by 7 SVG of square pixels. In colour when held; when not, only its silhouette's edge, dim (the DOS empty socket). */
export function keySvg(kind: KeyKind, held: boolean, rows: readonly string[] = KEY_ROWS[kind]): string {
  const side = rows.length;
  const open = '<svg viewBox="0 0 ' + side + ' ' + side + '" shape-rendering="crispEdges" focusable="false" aria-hidden="true">';
  if (!held) {
    const d = runs(rows, (x, y, c) => c !== '.' && isEdge(rows, x, y));
    return open + '<path fill="' + DIM + '" fill-opacity="0.6" d="' + d + '"/></svg>';
  }
  const colours = [...new Set(rows.join('').replaceAll('.', ''))];
  const paths = colours.map((c) => '<path fill="' + (KEY_COLOURS[c] ?? '#ffffff') + '" d="' + runs(rows, (_x, _y, p) => p === c) + '"/>');
  return open + paths.join('') + '</svg>';
}

/**
 * Which top corner the HUD takes. It stays where it is until Ben's sprite comes within `margin` of it, then crosses to the other
 * corner (the mirror of its box in a stage `stageW` wide), and stays there until he comes near that one. If he is near both it stays.
 */
export function hudSide(side: 'left' | 'right', hud: Box, stageW: number, ben: Box, margin: number): 'left' | 'right' {
  if (!overlaps(hud, ben, margin)) return side;
  const across = { ...hud, x: stageW - hud.x - hud.w };
  if (overlaps(across, ben, margin)) return side;
  return side === 'left' ? 'right' : 'left';
}
