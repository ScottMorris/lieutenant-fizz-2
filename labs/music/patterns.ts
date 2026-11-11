// The six concept patterns the lab's tunes grow from, and the rule that reads a pattern as a riff.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each pattern is an 8 by 16 picture of one idea from the game, kept exactly as it was drawn. Reading one as music is mechanical: a
// column is a step in time, a row is a scale degree (the bottom row is the lowest), the highest filled cell of a column is the melody
// and the lowest is its shadow. Nothing here knows a key or a tempo; a tune chooses those when it places the riff.

export interface ConceptPattern {
  id: string;
  /** The concept, in the words it was given. */
  concept: string;
  idea: string;
  mood: string;
  /** Eight rows of sixteen cells, top row first: `#` filled, `.` empty. */
  grid: readonly string[];
}

export const ROWS = 8;
export const COLUMNS = 16;

export const PATTERNS: readonly ConceptPattern[] = [
  {
    id: 'beam',
    concept: 'Lighthouse beam sweep',
    idea: 'A diagonal double-width beam fans from the lower left up to the top right, so the sparse sweep reads as a lamp turning across the night sky.',
    mood: 'lonely, restless, bright',
    grid: [
      '.............##.',
      '...........##...',
      '.........##.....',
      '.......##.......',
      '.....##.........',
      '...##...........',
      '.##.............',
      '#...............',
    ],
  },
  {
    id: 'press',
    concept: 'Siphon press rhythm',
    idea: 'Evenly spaced stamp-and-drop pairs repeat every four columns, the way timed presses set a beat the player must follow.',
    mood: 'mechanical, insistent, regular',
    grid: [
      '..#...#...#...#.',
      '..#...#...#...#.',
      '................',
      '................',
      '................',
      '................',
      '#...#...#...#...',
      '##..##..##..##..',
    ],
  },
  {
    id: 'keys',
    concept: 'Key-door chain',
    idea: 'Three stepped bars of growing length, linked by single cells, show keys that must be collected and used in strict order.',
    mood: 'patient, winding, ordered',
    grid: [
      '................',
      '.##.............',
      '..#.............',
      '..###...........',
      '....#...........',
      '....####........',
      '.......#........',
      '................',
    ],
  },
  {
    id: 'crystal',
    concept: 'Zargoth crystal growth',
    idea: 'Jagged spikes push up out of a dense ground line, showing alien chocolate crystal spreading wherever cocoa leaks.',
    mood: 'jagged, cold, spreading',
    grid: [
      '................',
      '................',
      '................',
      '................',
      '................',
      '...##......#....',
      '..####....###...',
      '#######..######.',
    ],
  },
  {
    id: 'tanker',
    concept: 'Cocoa Siphon tanker',
    idea: 'A wide, blocky tank body sits on a row of wheels, a heavy silhouette that reads as a tanker on treads.',
    mood: 'heavy, blunt, menacing',
    grid: [
      '................',
      '................',
      '................',
      '...##########...',
      '..############..',
      '.#..#..#..#...#.',
      '................',
      '................',
    ],
  },
  {
    id: 'rooms',
    concept: 'Identical rooms, one landmark',
    idea: 'Four identical rooms repeat across the grid, and the single antenna above one of them is the only thing that tells it apart.',
    mood: 'quiet, repetitive, uncanny',
    grid: [
      '................',
      '................',
      '.........#......',
      '#..##..##..##..#',
      '#..##..##..##..#',
      '................',
      '................',
      '................',
    ],
  },
];

/** Patterns that arrive from elsewhere: the Pathfinder paths register themselves here (paths.ts). */
const added: ConceptPattern[] = [];
export const addPatterns = (patterns: readonly ConceptPattern[]): void => void added.push(...patterns);

export const patternById = (id: string): ConceptPattern => {
  const found = PATTERNS.find((p) => p.id === id) ?? added.find((p) => p.id === id);
  if (!found) throw new Error(`labs/music/patterns.ts has no pattern "${id}"`);
  return found;
};

export interface Riff {
  /** The melody: the scale degree of the highest filled cell in each column (0 is the bottom row), or null for an empty column. */
  top: (number | null)[];
  /** The shadow: the lowest filled cell where it differs from the highest, else null. */
  low: (number | null)[];
}

/**
 * Reads a pattern as a riff. `base` is the scale degree of the bottom row, which lets a tune sit the picture on its tonic (0) or a
 * step under it (-1).
 */
export function riffOf(pattern: ConceptPattern, base = 0): Riff {
  if (pattern.grid.length !== ROWS || pattern.grid.some((row) => row.length !== COLUMNS)) {
    throw new Error(`pattern "${pattern.id}" is not ${ROWS} rows of ${COLUMNS} cells`);
  }
  const top: (number | null)[] = [];
  const low: (number | null)[] = [];
  for (let column = 0; column < COLUMNS; column++) {
    const filled: number[] = [];
    for (let row = 0; row < ROWS; row++) if (pattern.grid[row]![column] === '#') filled.push(ROWS - 1 - row + base);
    const highest = filled.length ? Math.max(...filled) : null;
    const lowest = filled.length ? Math.min(...filled) : null;
    top.push(highest);
    low.push(lowest !== highest ? lowest : null);
  }
  return { top, low };
}

/**
 * A line of a riff as a degree string for `degrees()` (scale.ts): one token per column, `~` for a rest. With `tie`, a run of the
 * same degree becomes one held note (`4@3`), which is how a wide cell reads as a long note rather than a repeated one.
 */
export function riffLine(line: readonly (number | null)[], tie = false): string {
  const tokens: string[] = [];
  for (let i = 0; i < line.length; ) {
    let run = 1;
    if (tie) while (i + run < line.length && line[i + run] === line[i]) run++;
    const token = line[i] === null ? '~' : String(line[i]);
    tokens.push(run > 1 ? `${token}@${run}` : token);
    i += run;
  }
  return tokens.join(' ');
}

/** The columns `from` up to `to` of a riff line, for a riff spread over two bars. */
export const riffSlice = (line: readonly (number | null)[], from: number, to: number, tie = false): string =>
  riffLine(line.slice(from, to), tie);
