// The player's visualizers: five small EGA pictures drawn from what the lab is playing.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each visualizer draws one frame onto a 320 by 120 canvas that the page scales up with hard pixels, in the sixteen EGA colours
// only. They read the same things: the left and right waveforms, a spectrum, and where the player is in the bar. Nothing here makes
// or changes sound. A visualizer keeps what it needs between frames (bubbles, peaks) in its own closure.

export const VIZ_WIDTH = 320;
export const VIZ_HEIGHT = 120;

export const EGA = {
  black: '#000000', blue: '#0000aa', green: '#00aa00', cyan: '#00aaaa', red: '#aa0000', magenta: '#aa00aa', brown: '#aa5500', grey: '#aaaaaa',
  darkGrey: '#555555', lightBlue: '#5555ff', lightGreen: '#55ff55', lightCyan: '#55ffff', lightRed: '#ff5555', lightMagenta: '#ff55ff', yellow: '#ffff55', white: '#ffffff',
} as const;

/** What a visualizer is given each frame. */
export interface VizFrame {
  /** Seconds since the page loaded. */
  time: number;
  /** The left and right waveforms, -1 to 1, the same length. */
  left: Float32Array;
  right: Float32Array;
  /** The spectrum of the left and right together, 0 to 255 a bin, low frequencies first. */
  spectrum: Uint8Array;
  /** Whether the player is playing. */
  playing: boolean;
  /** The beat of the bar (0 to 3), how far through that beat (0 to 1), and the bar of the section. */
  beat: number;
  phase: number;
  bar: number;
  /** Whether the player is in a turn between sections. */
  turning: boolean;
  /** The tune's colour, one of the sixteen. */
  ink: string;
}

/** The part of a 2D canvas context the visualizers use. */
export type Pen = Pick<CanvasRenderingContext2D, 'fillStyle' | 'fillRect'>;

export interface Visualizer {
  id: string;
  name: string;
  draw(pen: Pen, frame: VizFrame): void;
}

const W = VIZ_WIDTH;
const H = VIZ_HEIGHT;
const rect = (pen: Pen, colour: string, x: number, y: number, w: number, h: number): void => {
  pen.fillStyle = colour;
  pen.fillRect(Math.round(x), Math.round(y), Math.round(w), Math.round(h));
};
const clear = (pen: Pen, colour: string = EGA.black): void => rect(pen, colour, 0, 0, W, H);
const peakOf = (wave: Float32Array): number => {
  let peak = 0;
  for (let i = 0; i < wave.length; i++) peak = Math.max(peak, Math.abs(wave[i]!));
  return peak;
};
/**
 * A gain that keeps a quiet tune filling the picture: it follows the loudest recent peak, rising at once and falling slowly, and
 * never lifts near-silence by more than twelve times.
 */
const follower = (): ((left: Float32Array, right: Float32Array) => number) => {
  let held = 0;
  return (left, right) => {
    held = Math.max(peakOf(left), peakOf(right), held * 0.985);
    return 0.9 / Math.max(held, 0.075);
  };
};
/** The average of a range of the spectrum, 0 to 1. `from` and `to` are fractions of the bins. */
const band = (spectrum: Uint8Array, from: number, to: number): number => {
  const a = Math.floor(from * spectrum.length);
  const b = Math.max(a + 1, Math.floor(to * spectrum.length));
  let sum = 0;
  for (let i = a; i < b; i++) sum += spectrum[i]!;
  return sum / (b - a) / 255;
};
/** A repeatable number from 0 to 1 for a whole number, so stars and bubbles sit in the same place every frame. */
const scatter = (n: number): number => {
  const x = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
};

// ---------- Scope: the two waveforms, left over right ----------

function scope(): Visualizer {
  const gain = follower();
  return {
    id: 'scope',
    name: 'Scope',
    draw(pen, f) {
      clear(pen);
      for (let x = 0; x < W; x += 16) rect(pen, EGA.blue, x, 0, 1, H);
      rect(pen, EGA.blue, 0, H / 2, W, 1);
      const lift = gain(f.left, f.right) * 34;
      const trace = (wave: Float32Array, mid: number, colour: string) => {
        let last = mid;
        for (let x = 0; x < W; x += 2) {
          // Half the buffer across the screen, so single waves can be seen.
          const y = mid - (wave[Math.floor((x / W) * wave.length * 0.5)] ?? 0) * lift;
          rect(pen, colour, x, Math.min(last, y), 2, Math.abs(y - last) + 2);
          last = y;
        }
      };
      trace(f.left, H * 0.27, f.ink);
      trace(f.right, H * 0.73, EGA.lightCyan);
    },
  };
}

// ---------- Vats: a spectrum of cocoa vats that fill, with a froth that falls slowly ----------

function vats(): Visualizer {
  const BARS = 20;
  const froth = new Float32Array(BARS);
  return {
    id: 'vats',
    name: 'Vats',
    draw(pen, f) {
      clear(pen);
      const width = W / BARS;
      for (let i = 0; i < BARS; i++) {
        // Low notes get more room than high ones: each vat covers a wider slice of the spectrum than the last.
        const from = (i / BARS) ** 2 * 0.22;
        const to = ((i + 1) / BARS) ** 2 * 0.22;
        const level = Math.min(1, Math.max(0, band(f.spectrum, from, to) - 0.06) * 1.25);
        const top = Math.floor(level * 13);
        froth[i] = Math.max(top, froth[i]! - 0.25);
        rect(pen, EGA.darkGrey, i * width, H - 4, width - 2, 4);
        for (let cell = 0; cell < top; cell++) {
          const colour = cell > 10 ? EGA.lightRed : cell > 7 ? EGA.yellow : EGA.brown;
          rect(pen, colour, i * width + 1, H - 12 - cell * 8, width - 4, 6);
        }
        rect(pen, i % 2 ? f.ink : EGA.white, i * width + 1, H - 12 - Math.floor(froth[i]!) * 8, width - 4, 2);
      }
    },
  };
}

// ---------- Field: the stereo picture, left against right ----------

function field(): Visualizer {
  const gain = follower();
  return {
    id: 'field',
    name: 'Stereo field',
    draw(pen, f) {
      // Fade, not clear, so a moving sound leaves a trail: a checker of black pixels eats half the old picture each frame.
      pen.fillStyle = EGA.black;
      const odd = Math.floor(f.time * 30) % 2;
      for (let y = 0; y < H; y += 2) for (let x = (y / 2 + odd) % 2 ? 0 : 2; x < W; x += 4) pen.fillRect(x, y, 2, 2);
      const cx = W / 2;
      const cy = H / 2;
      rect(pen, EGA.blue, cx, 4, 1, H - 8);
      rect(pen, EGA.blue, cx - 56, cy, 112, 1);
      // A sound in the middle draws a vertical line, one hard to a side leans 45 degrees, and width spreads the cloud.
      const lift = gain(f.left, f.right);
      for (let i = 0; i < f.left.length; i += 2) {
        const l = f.left[i]! * lift;
        const r = f.right[i]! * lift;
        const side = Math.max(-150, Math.min(150, (r - l) * 320));
        const mid = Math.max(-56, Math.min(56, (l + r) * 42));
        rect(pen, Math.abs(side) > 14 ? EGA.lightCyan : f.ink, cx + side, cy - mid, 2, 2);
      }
      rect(pen, EGA.grey, 6, cy - 3, 3, 7);
      rect(pen, EGA.grey, W - 9, cy - 3, 3, 7);
    },
  };
}

// ---------- Lighthouse: the night coast; the beam turns with the bar and the sea moves with the bass ----------

function lighthouse(): Visualizer {
  return {
    id: 'lighthouse',
    name: 'Lighthouse',
    draw(pen, f) {
      const bass = band(f.spectrum, 0, 0.02);
      const highs = band(f.spectrum, 0.12, 0.4);
      const loud = Math.max(peakOf(f.left), peakOf(f.right));
      clear(pen);
      for (let i = 0; i < 46; i++) {
        const lit = scatter(i * 3 + Math.floor(f.time * 3)) < 0.25 + highs * 2.5;
        rect(pen, lit ? EGA.white : EGA.darkGrey, scatter(i) * W, scatter(i + 60) * 62, 1, 1);
      }
      // The tower, with its lamp room.
      const tx = 54;
      rect(pen, EGA.grey, tx - 6, 34, 12, 60);
      rect(pen, EGA.red, tx - 6, 46, 12, 8);
      rect(pen, EGA.red, tx - 6, 68, 12, 8);
      rect(pen, EGA.darkGrey, tx - 9, 30, 18, 4);
      rect(pen, loud > 0.02 ? EGA.yellow : EGA.brown, tx - 4, 22, 8, 8);
      rect(pen, EGA.darkGrey, tx - 5, 18, 10, 4);
      // The beam sweeps out and back once a bar, drawn as dithered rows that widen with distance.
      if (f.playing) {
        const sweep = (f.beat + f.phase) / 4;
        const angle = (sweep < 0.5 ? sweep * 2 : 2 - sweep * 2) * 0.9 - 0.45;
        const reach = 150 + loud * 110;
        for (let d = 8; d < reach; d += 2) {
          const x = tx + 4 + d;
          const y = 26 + Math.sin(angle) * d;
          const spread = 1 + d * 0.09;
          for (let k = -spread; k <= spread; k += 2) {
            if ((Math.floor(d / 2) + Math.floor(k / 2)) % 2 === 0 && x < W && y + k < 94) rect(pen, d < reach * 0.5 ? EGA.yellow : f.ink, x, y + k, 2, 1);
          }
        }
      }
      // The shore and the sea: each column of water rides the bass.
      rect(pen, EGA.brown, 0, 94, 96, 4);
      for (let x = 0; x < W; x += 4) {
        const swell = Math.sin(x * 0.07 + f.time * 2.2) * (1.5 + bass * 9);
        const y = 100 + swell;
        rect(pen, EGA.blue, x, y, 4, H - y);
        rect(pen, (x / 4) % 3 === 0 ? EGA.lightBlue : EGA.cyan, x, y, 4, 1);
      }
      if (f.turning) rect(pen, EGA.lightMagenta, 0, 0, W, 2);
    },
  };
}

// ---------- Fizz: soda bubbles that rise; every beat shakes the can ----------

function fizz(): Visualizer {
  interface Bubble {
    x: number;
    y: number;
    size: number;
    speed: number;
    side: number;
  }
  let bubbles: Bubble[] = [];
  let lastBeat = -1;
  let count = 0;
  return {
    id: 'fizz',
    name: 'Fizz',
    draw(pen, f) {
      clear(pen);
      const left = peakOf(f.left);
      const right = peakOf(f.right);
      const key = f.bar * 4 + f.beat;
      if (f.playing && key !== lastBeat) {
        lastBeat = key;
        // A beat pours a burst; the louder side of the field gets more of it.
        const pour = 3 + Math.round((left + right) * 9) + (f.beat === 0 ? 4 : 0);
        for (let i = 0; i < pour; i++) {
          const n = count++;
          const side = scatter(n) < left / (left + right + 1e-6) ? -1 : 1;
          bubbles.push({ x: W / 2 + side * (10 + scatter(n + 7) * 140), y: H + scatter(n + 3) * 12, size: 2 + Math.floor(scatter(n + 11) * 4) * 2, speed: 0.5 + scatter(n + 5) * 1.3, side });
        }
      }
      const lift = 0.4 + (left + right) * 1.6;
      bubbles = bubbles.filter((b) => b.y + b.size > 0).slice(-140);
      for (const b of bubbles) {
        b.y -= b.speed * lift;
        b.x += Math.sin(f.time * 3 + b.y * 0.1) * 0.4;
        const colour = b.side < 0 ? f.ink : EGA.lightCyan;
        // A bubble is a ring with a glint: four edges and one white pixel.
        rect(pen, colour, b.x + 1, b.y, b.size - 2, 1);
        rect(pen, colour, b.x + 1, b.y + b.size - 1, b.size - 2, 1);
        rect(pen, colour, b.x, b.y + 1, 1, b.size - 2);
        rect(pen, colour, b.x + b.size - 1, b.y + 1, 1, b.size - 2);
        if (b.size > 2) rect(pen, EGA.white, b.x + 1, b.y + 1, 1, 1);
      }
      // The can in the middle jumps on the beat.
      const hop = f.playing ? Math.round((1 - f.phase) ** 3 * 6) : 0;
      const cy = H - 34 - hop;
      rect(pen, EGA.grey, W / 2 - 8, cy, 16, 2);
      rect(pen, EGA.lightMagenta, W / 2 - 8, cy + 2, 16, 26);
      rect(pen, EGA.white, W / 2 - 8, cy + 10, 16, 8);
      rect(pen, EGA.magenta, W / 2 - 8, cy + 2, 3, 26);
      rect(pen, EGA.darkGrey, W / 2 - 14, H - 6, 28, 2);
      if (f.turning) rect(pen, EGA.lightMagenta, 0, 0, W, 2);
    },
  };
}

/** A fresh set of visualizers, in the order the page offers them. */
export const makeVisualizers = (): Visualizer[] => [lighthouse(), fizz(), vats(), field(), scope()];
