// What the Web HUD shows, as pure data: the values, their text, the key chip's drawing, the scale and the announcements.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The HUD is Fizz 1's desktop HUD in its look: a dark glass panel, small dim labels over big Fizz pixel digits, the number cyan and
// turning red at zero (Fizz 1's `pillItems`, episodes/episode-1/src/hud.ts). Fizz 2 shows the ammunition (Fizz), the deaths and
// whether the red key is carried. TICK and HASH are not here: they stay in the H panel for inspection. This file touches no DOM, so
// `hud.test.ts` runs it under Bun; web/main.js applies the result to the page.

/** What the page reads from the module each frame. */
export interface HudValues {
  ammo: number;
  key: boolean;
  deaths: number;
}

/** The most a count shows. Fizz 1's number cells hold two digits here, and the panel must not grow. */
export const COUNT_CAP = 99;

// Fizz 1's colours (EGA light cyan and light red) and its dim label colour.
export const CYAN = '#55ffff';
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

/** The size of one sprite pixel of the key chip in CSS pixels (Fizz 1's menu bullet rule: half the scale, at least 2). */
export function keyPixelSize(scale: number): number {
  return Math.max(2, Math.ceil(scale / 2));
}

/** The HUD as it is drawn. */
export interface HudView {
  fizz: string;
  /** Whether the Fizz number is out, so it is drawn red. */
  fizzOut: boolean;
  deaths: string;
  key: boolean;
}

export function hudView(v: HudValues): HudView {
  return {
    fizz: formatCount(v.ammo),
    fizzOut: v.ammo <= 0,
    deaths: formatCount(v.deaths),
    key: v.key,
  };
}

/** Whether two readings show the same HUD, so the page can leave the DOM alone. Counts past the cap show alike. */
export function sameHud(a: HudValues | null, b: HudValues | null): boolean {
  if (a === null || b === null) return a === b;
  const x = hudView(a);
  const y = hudView(b);
  return x.fizz === y.fizz && x.fizzOut === y.fizzOut && x.deaths === y.deaths && x.key === y.key;
}

/**
 * What a screen reader is told when the HUD changes, or null for nothing. Only moments that matter are spoken: the Fizz running
 * out, the key picked up and a death. Every other change (a shot, the key dropped on a death, a restart) is silent, and so is the
 * first reading. When several happen in one frame they are joined in that order. The current values are always in the H panel.
 */
export function announcement(prev: HudValues | null, next: HudValues): string | null {
  if (prev === null) return null;
  const said: string[] = [];
  if (prev.ammo > 0 && next.ammo <= 0) said.push('Out of Fizz');
  if (!prev.key && next.key) said.push('Red key picked up');
  if (next.deaths > prev.deaths) said.push(`Deaths ${formatCount(next.deaths)}`);
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

// --- The key chip -------------------------------------------------------------------------------------------------------------

/** Fizz 1's `keyRed` sprite (assets/sprites/fizz1-slice.json), the red gumdrop Fizz 2 calls the red key. hud.test.ts keeps it equal. */
export const KEY_ROWS = [
  '................',
  '................',
  '................',
  '................',
  '......kkkk......',
  '.....krrrrk.....',
  '....krrrrrrk....',
  '...krrrrrWrrk...',
  '..krrrWrrrrrrk..',
  '..krrrrrrrrrrk..',
  '..krrrrrrrrrrk..',
  '..krrRrrrrWrrk..',
  '..krrrrrrrrrrk..',
  '..krrrrrrrrrrk..',
  '...kkkkkkkkkk...',
  '................',
] as const;

// The EGA letters the key uses (tools/sprites/lib.ts PALETTE).
const KEY_COLOURS: Record<string, string> = { k: '#000000', r: '#ff5555', R: '#aa0000', W: '#ffffff' };

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
      out.push(`M${x} ${y}h${end - x}v1h-${end - x}z`);
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

/** The key as a 16 by 16 SVG of square pixels. In colour when carried; when not, only its outline, dim. */
export function keySvg(carried: boolean, rows: readonly string[] = KEY_ROWS): string {
  const open = '<svg viewBox="0 0 16 16" shape-rendering="crispEdges" focusable="false" aria-hidden="true">';
  if (!carried) {
    const d = runs(rows, (x, y, c) => c !== '.' && isEdge(rows, x, y));
    return `${open}<path fill="${DIM}" fill-opacity="0.45" d="${d}"/></svg>`;
  }
  const colours = [...new Set(rows.join('').replaceAll('.', ''))];
  const paths = colours.map((c) => `<path fill="${KEY_COLOURS[c] ?? '#ffffff'}" d="${runs(rows, (_x, _y, p) => p === c)}"/>`);
  return `${open}${paths.join('')}</svg>`;
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
