// The Web page's Paused and Options screens: what is remembered, and the overlay that draws them over the picture.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Two halves. The first is pure and has `bun test` cases (options.test.ts): the remembered settings, read from and written to one
// `localStorage` blob, `fizz2-options`, with the older single-purpose keys migrated on the first read. The second is the DOM
// overlay in Fizz 1's Web menu look (packages/engine/src/menu/rows.ts, rows.css and tokens.css in lieutenant-fizz): a dark panel
// over the dimmed picture, rows that are plates in the Fizz accent when selected, a soda-can bullet, and `◄ value ►` on the
// selected choice. The words, the rows and the cursor come from the module (`fizz-menu`, through `App`); this file only draws them.
import { hudScale } from './hud.ts';

// --- What is remembered -------------------------------------------------------------------------------------------------------

/** The one `localStorage` key holding every remembered setting, as JSON `{"v":1,...}`. */
export const OPTIONS_KEY = 'fizz2-options';

/** The keys of earlier builds, one setting each. They are read once, folded into the blob and removed. */
export const LEGACY_KEYS = {
  layout: 'fizz2-layout',
  captions: 'fizz2-captions',
  motion: 'fizz2-motion',
  audio: 'fizz2-audio',
  sound: 'fizz2-sound',
  music: 'fizz2-music',
} as const;

export interface Options {
  sound: 'on' | 'off';
  music: 'on' | 'off';
  placement: 'enhanced' | 'classic';
  /** `system` follows the device's reduced-motion setting. */
  motion: 'system' | 'reduced' | 'full';
  captions: 'on' | 'off';
  keys: 'keen' | 'modern';
}

export const DEFAULT_OPTIONS: Readonly<Options> = {
  sound: 'on',
  music: 'on',
  placement: 'enhanced',
  motion: 'system',
  captions: 'on',
  keys: 'keen',
};

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem?(key: string): void;
}

const CHOICES: { [K in keyof Options]: readonly Options[K][] } = {
  sound: ['on', 'off'],
  music: ['on', 'off'],
  placement: ['enhanced', 'classic'],
  motion: ['system', 'reduced', 'full'],
  captions: ['on', 'off'],
  keys: ['keen', 'modern'],
};

const FIELDS = Object.keys(CHOICES) as (keyof Options)[];

/** Picks `word` if it is one of the field's choices, else `fallback`. */
function pick<K extends keyof Options>(field: K, word: unknown, fallback: Options[K]): Options[K] {
  return (CHOICES[field] as readonly unknown[]).includes(word) ? (word as Options[K]) : fallback;
}

/**
 * Reads the blob. Text that is not JSON, not an object or not version 1 gives `null`; inside a version 1 object a field that is
 * missing or not one of its words falls back to the default, so one bad field never costs the others.
 */
export function parseOptions(text: string | null | undefined): Options | null {
  if (!text) return null;
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return null;
  }
  if (typeof data !== 'object' || data === null || (data as { v?: unknown }).v !== 1) return null;
  const o = data as Record<string, unknown>;
  const out = { ...DEFAULT_OPTIONS };
  for (const f of FIELDS) (out as Record<string, unknown>)[f] = pick(f, o[f], DEFAULT_OPTIONS[f]);
  return out;
}

/** The blob as it is stored: version first, then the fields in a fixed order. */
export function serialiseOptions(o: Options): string {
  return JSON.stringify({ v: 1, ...Object.fromEntries(FIELDS.map((f) => [f, o[f]])) });
}

/**
 * The settings of the older keys. A key that is missing or holds something else leaves the default. Captions, sound and music were
 * only ever stored as `off` or `on`, and the default is on; motion as `full` or `reduced`, with no key meaning the system's.
 */
export function migrateLegacy(read: (key: string) => string | null): { options: Options; found: boolean } {
  const options = { ...DEFAULT_OPTIONS };
  let found = false;
  const take = <K extends keyof Options>(field: K, key: string, words: readonly Options[K][]) => {
    const raw = read(key);
    if (raw === null) return;
    found = true;
    const word = words.find((w) => w === raw);
    if (word !== undefined) options[field] = word;
  };
  take('keys', LEGACY_KEYS.layout, CHOICES.keys);
  take('captions', LEGACY_KEYS.captions, CHOICES.captions);
  take('motion', LEGACY_KEYS.motion, ['reduced', 'full']);
  take('placement', LEGACY_KEYS.audio, CHOICES.placement);
  take('sound', LEGACY_KEYS.sound, CHOICES.sound);
  take('music', LEGACY_KEYS.music, CHOICES.music);
  return { options, found };
}

/**
 * The remembered settings. If the blob is there it wins and the old keys are left alone; if not, the old keys are read once,
 * written as the blob and removed. Storage that is blocked or missing gives the defaults, and the page works without it.
 */
export function loadOptions(storage: StorageLike | null): Options {
  if (!storage) return { ...DEFAULT_OPTIONS };
  try {
    const stored = parseOptions(storage.getItem(OPTIONS_KEY));
    if (stored) return stored;
    const { options, found } = migrateLegacy((key) => storage.getItem(key));
    if (found) {
      storage.setItem(OPTIONS_KEY, serialiseOptions(options));
      for (const key of Object.values(LEGACY_KEYS)) storage.removeItem?.(key);
    }
    return options;
  } catch {
    return { ...DEFAULT_OPTIONS };
  }
}

/** Remembers the settings. Returns whether it could. */
export function saveOptions(storage: StorageLike | null, o: Options): boolean {
  try {
    storage?.setItem(OPTIONS_KEY, serialiseOptions(o));
    return storage != null;
  } catch {
    return false;
  }
}

/** `localStorage` when the browser allows it. Reading the property can itself throw. */
export function browserStorage(): StorageLike | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The placement `?audio=` asks for in a query string, or `null` when it asks for none. */
export function placementFromSearch(search: string): Options['placement'] | null {
  const word = new URLSearchParams(search).get('audio');
  return word === 'classic' || word === 'enhanced' ? word : null;
}

// --- The rows -----------------------------------------------------------------------------------------------------------------

/** The module's row ids (`fizz_menu::RowId`) and the names the page gives them. */
export const ROW = {
  music: 0,
  placement: 2,
  motion: 4,
  captions: 5,
  keys: 6,
  back: 7,
  sound: 8,
  fullscreen: 9,
  resume: 10,
  options: 11,
  restart: 13,
} as const;

const ROW_NAMES = new Map<number, string>(Object.entries(ROW).map(([name, id]) => [id, name]));

/** The settings row ids and the index each field's word has in the module's order (`App::menu_value`). */
export const ROW_OF: { [K in keyof Options]: { row: number; words: readonly Options[K][] } } = {
  sound: { row: ROW.sound, words: ['on', 'off'] },
  music: { row: ROW.music, words: ['on', 'off'] },
  placement: { row: ROW.placement, words: ['enhanced', 'classic'] },
  motion: { row: ROW.motion, words: ['system', 'reduced', 'full'] },
  captions: { row: ROW.captions, words: ['on', 'off'] },
  keys: { row: ROW.keys, words: ['keen', 'modern'] },
};

/** What the page asks the module for. */
export interface ValueApp {
  menu_value(row: number): number;
  set_value(row: number, index: number): void;
}

/** Loads remembered settings into the module. */
export function pushOptions(app: ValueApp, o: Options): void {
  for (const f of FIELDS) {
    const { row, words } = ROW_OF[f];
    app.set_value(row, Math.max(0, (words as readonly string[]).indexOf(o[f])));
  }
}

/** Reads the module's settings back. */
export function pullOptions(app: ValueApp): Options {
  const out = { ...DEFAULT_OPTIONS };
  for (const f of FIELDS) {
    const { row, words } = ROW_OF[f];
    (out as Record<string, unknown>)[f] = words[app.menu_value(row)] ?? DEFAULT_OPTIONS[f];
  }
  return out;
}

// --- Keys ---------------------------------------------------------------------------------------------------------------------

/** `fizz_menu::MenuKey` numbers as `App::menu_key` takes them. */
export const MENU_KEY = { up: 0, down: 1, left: 2, right: 3, confirm: 4, back: 5 } as const;

/** The menu key an `event.code` means while a screen is up, or `null`. Left is one step back; Right, Enter and Space go forward. */
export function menuKeyFor(code: string): number | null {
  switch (code) {
    case 'ArrowUp':
      return MENU_KEY.up;
    case 'ArrowDown':
      return MENU_KEY.down;
    case 'ArrowLeft':
      return MENU_KEY.left;
    case 'ArrowRight':
      return MENU_KEY.right;
    case 'Enter':
    case 'NumpadEnter':
    case 'Space':
      return MENU_KEY.confirm;
    case 'Escape':
    case 'Backspace':
      return MENU_KEY.back;
    default:
      return null;
  }
}

/** Keys that open Paused. Escape is the usual way; Pause is for fullscreen, where the browser takes the first Escape. */
export function opensPaused(code: string): boolean {
  return code === 'Escape' || code === 'Pause';
}

// --- The look -----------------------------------------------------------------------------------------------------------------

/** The plate's three shades, as Fizz 1 steps through its amber ones, derived from the Fizz accent, and the ink on it. */
export const PLATE = ['#ff55ff', '#ff77ff', '#ff99ff'] as const;
export const PLATE_INK = '#1a001a';

/**
 * Fizz 1's soda can (`soda()` in episodes/episode-1/src/sprites/items.ts) as the DOS menu draws it: the `soda` sprite of
 * assets/sprites/fizz1-slice.json after the Fizz 2 recolour, outline included. A grape (light magenta) body with a grey rim top and
 * bottom, a white band and highlight, and two white bubbles. options.test.ts keeps this equal to the sprite data.
 */
export const SODA_ROWS = [
  '................',
  '.....kkkkkk.....',
  '....kLLLLLLk....',
  '....kmmmmmmk....',
  '....kmWmmmmk....',
  '....kmmmmmmk....',
  '....kmmmmmmk....',
  '....kWWWWWWk....',
  '....kWWWWWWk....',
  '....kmmmmmmk....',
  '....kmmmmmmk....',
  '....kmmmmWmk....',
  '....kmmmWmmk....',
  '....kmmmmmmk....',
  '....kLLLLLLk....',
  '.....kkkkkk.....',
] as const;

const SODA_COLOURS: Record<string, string> = { k: '#000000', m: '#ff55ff', L: '#aaaaaa', W: '#ffffff' };

/** The can as a 16 by 16 SVG of square pixels with a black outline round it, like the HUD's key. */
export function sodaSvg(rows: readonly string[] = SODA_ROWS): string {
  const filled = (x: number, y: number) => (rows[y]?.[x] ?? '.') !== '.';
  const grid = rows.map((row, y) =>
    [...row].map((c, x) => (c !== '.' ? c : filled(x - 1, y) || filled(x + 1, y) || filled(x, y - 1) || filled(x, y + 1) ? 'k' : '.')).join(''),
  );
  const paths = Object.keys(SODA_COLOURS)
    .filter((c) => grid.some((r) => r.includes(c)))
    .map((c) => {
      const d: string[] = [];
      grid.forEach((row, y) => {
        for (let x = 0; x < row.length; ) {
          if (row[x] !== c) {
            x++;
            continue;
          }
          let end = x;
          while (end < row.length && row[end] === c) end++;
          d.push(`M${x} ${y}h${end - x}v1h-${end - x}z`);
          x = end;
        }
      });
      return `<path fill="${SODA_COLOURS[c]}" d="${d.join('')}"/>`;
    });
  return `<svg viewBox="0 0 16 16" shape-rendering="crispEdges" focusable="false" aria-hidden="true">${paths.join('')}</svg>`;
}

/** The text scale of the screens: the HUD's for this height, but never so large that a row (about 140 px a step) overflows the width. */
export function optionsScale(height: number, width: number): number {
  return Math.max(1, Math.min(hudScale(height), Math.floor(width / 140)));
}

/** What a screen reader hears for a row: the label, then the value without the `*` of an overridden row. */
export function spoken(label: string, value: string): string {
  const v = value.replace(/ \*$/, '');
  return v ? `${label}: ${v}` : label;
}

// --- The overlay --------------------------------------------------------------------------------------------------------------

/** The part of `App` the overlay uses. */
export interface MenuApp {
  menu_screen(): number;
  menu_open_paused(): void;
  menu_open_options(): void;
  menu_close(): void;
  menu_key(key: number): number;
  menu_last_row(): number;
  menu_select(i: number): void;
  menu_click(i: number): number;
  menu_step(i: number, d: number): number;
  menu_row_count(): number;
  menu_row_id(i: number): number;
  menu_row_label(i: number): string;
  menu_row_value(i: number): string;
  menu_row_selected(i: number): boolean;
  menu_row_overridden(i: number): boolean;
  menu_note(): string;
}

export interface OverlayHooks {
  /** A screen opened over play. Release held keys here. */
  onOpen(): void;
  /** The last screen closed; play resumes. */
  onClose(): void;
  /** A value changed on the Options screen (a row id). Apply and remember it here. */
  onChange(row: number): void;
}

const KEY_CHANGED = 2;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Record<string, string> = {}, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) node.setAttribute(name, value);
  if (text !== undefined) node.textContent = text;
  return node;
}

/** The overlay: made once, hidden until a screen opens, and redrawn from the module after every key and click. */
export class OptionsUi {
  private readonly root: HTMLElement;
  private readonly title: HTMLElement;
  private readonly list: HTMLElement;
  private readonly note: HTMLElement;
  private readonly foot: HTMLElement;
  private readonly status: HTMLElement;
  private screen = 0;
  private ids: number[] = [];
  private returnFocus: Element | null = null;
  private statusTimer = 0;

  constructor(
    private readonly app: MenuApp,
    private readonly stage: HTMLElement,
    private readonly hooks: OverlayHooks,
  ) {
    this.root = el('div', { id: 'options', hidden: '' });
    const panel = el('div', { class: 'opt-panel', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'opt-title' });
    this.title = el('h2', { id: 'opt-title' }, 'Options');
    this.list = el('div', { class: 'menu', role: 'menu', 'aria-labelledby': 'opt-title' });
    this.note = el('p', { class: 'opt-note' });
    this.foot = el('p', { class: 'opt-foot' });
    panel.append(this.title, this.list, this.note, this.foot);
    this.root.append(panel);
    this.stage.append(this.root);
    this.status = el('p', { id: 'options-status', class: 'sr-only', role: 'status', 'aria-live': 'polite', 'aria-atomic': 'true' });
    this.stage.parentElement?.append(this.status);
    this.scale();
  }

  get isOpen(): boolean {
    return this.screen !== 0;
  }

  /** Sets the text scale: the HUD's whole-number scale for the stage's height, kept small enough that a row fits a narrow stage. */
  scale(): void {
    this.root.style.setProperty('--n', String(optionsScale(this.stage.clientHeight, this.stage.clientWidth)));
  }

  /** Marks the overlay as reduced motion (the Motion option), on top of the system setting the CSS already honours. */
  setReducedMotion(reduced: boolean): void {
    this.root.classList.toggle('rm', reduced);
  }

  openPaused(): void {
    if (this.isOpen) return;
    this.app.menu_open_paused();
    this.sync();
  }

  openOptions(): void {
    if (this.isOpen) return;
    this.app.menu_open_options();
    this.sync();
  }

  /** Closes whatever is up and resumes play. */
  close(): void {
    if (!this.isOpen) return;
    this.app.menu_close();
    this.sync();
  }

  /** A menu key (a `MENU_KEY` number). */
  key(key: number): void {
    if (!this.isOpen) return;
    this.after(this.app.menu_key(key));
  }

  /** Redraws from the module, and handles the screen having changed. Call it after anything that changed a value from outside. */
  sync(): void {
    const now = this.app.menu_screen();
    const was = this.screen;
    this.screen = now;
    if (was === 0 && now !== 0) {
      this.hooks.onOpen();
      this.returnFocus = document.activeElement;
      this.root.hidden = false;
    }
    if (now !== 0) {
      this.scale();
      this.render();
    } else if (was !== 0) {
      this.root.hidden = true;
      // Focus goes back to where it was, unless that has gone or was in the overlay.
      const back = this.returnFocus;
      this.returnFocus = null;
      if (back instanceof HTMLElement && back !== document.body && document.contains(back) && !this.root.contains(back)) back.focus({ preventScroll: true });
      else if (document.activeElement instanceof HTMLElement && this.root.contains(document.activeElement)) document.activeElement.blur();
      this.hooks.onClose();
    }
  }

  private after(result: number): void {
    const row = this.app.menu_last_row();
    this.sync();
    if (result === KEY_CHANGED) {
      this.hooks.onChange(row);
      this.render();
      this.say(this.rowSpeech(row));
    }
  }

  private rowSpeech(row: number): string {
    for (let i = 0; i < this.app.menu_row_count(); i++) {
      if (this.app.menu_row_id(i) === row) return spoken(this.app.menu_row_label(i), this.app.menu_row_value(i));
    }
    return '';
  }

  /** Tells a screen reader, once; the text is cleared first so the same words twice are heard twice. */
  private say(text: string): void {
    if (!text) return;
    this.status.textContent = '';
    clearTimeout(this.statusTimer);
    this.statusTimer = window.setTimeout(() => (this.status.textContent = text), 60);
  }

  private build(ids: number[]): void {
    this.list.replaceChildren();
    ids.forEach((id, i) => {
      const row = el('div', { class: 'row', role: 'menuitem', tabindex: '-1', 'data-row': ROW_NAMES.get(id) ?? String(id), 'data-i': String(i) });
      const gut = el('span', { class: 'gut', 'aria-hidden': 'true' });
      const bullet = el('span', { class: 'bullet' });
      bullet.innerHTML = sodaSvg();
      gut.append(bullet);
      const plate = el('span', { class: 'plate' });
      const label = el('span', { class: 'lbl' });
      const val = el('span', { class: 'val', 'aria-hidden': 'true' });
      const left = el('span', { class: 'arrow l' }, '◄');
      const text = el('span', { class: 'vt' });
      const right = el('span', { class: 'arrow r' }, '►');
      val.append(left, text, right);
      plate.append(label, val);
      row.append(gut, plate);
      row.addEventListener('pointerenter', () => {
        if (this.app.menu_row_selected(i)) return;
        this.app.menu_select(i);
        this.render();
      });
      row.addEventListener('click', () => {
        if (!this.isOpen) return;
        this.after(this.app.menu_click(i));
      });
      for (const [arrow, d] of [[left, -1], [right, 1]] as const) {
        arrow.addEventListener('click', (e) => {
          e.stopPropagation();
          if (this.isOpen) this.after(this.app.menu_step(i, d));
        });
      }
      this.list.append(row);
    });
    this.ids = ids;
  }

  private render(): void {
    const n = this.app.menu_row_count();
    const ids = Array.from({ length: n }, (_, i) => this.app.menu_row_id(i));
    const screenName = this.screen === 1 ? 'Paused' : 'Options';
    if (this.title.textContent !== screenName) this.title.textContent = screenName;
    this.root.dataset.screen = screenName.toLowerCase();
    this.foot.textContent = this.screen === 1 ? 'Esc resumes.' : 'Left and Right change a value. Esc goes back.';
    if (ids.length !== this.ids.length || ids.some((id, i) => id !== this.ids[i])) this.build(ids);
    const rows = Array.from(this.list.children) as HTMLElement[];
    let selected: HTMLElement | null = null;
    rows.forEach((row, i) => {
      const label = this.app.menu_row_label(i);
      const value = this.app.menu_row_value(i);
      const sel = this.app.menu_row_selected(i);
      row.classList.toggle('sel', sel);
      row.classList.toggle('star', this.app.menu_row_overridden(i));
      row.tabIndex = sel ? 0 : -1;
      row.setAttribute('aria-label', spoken(label, value));
      row.toggleAttribute('data-choice', value !== '');
      (row.querySelector('.lbl') as HTMLElement).textContent = label;
      (row.querySelector('.vt') as HTMLElement).textContent = value;
      if (sel) selected = row;
    });
    const note = this.app.menu_note();
    this.note.textContent = note;
    this.note.hidden = note === '';
    const focused = document.activeElement;
    if (selected && focused !== selected) (selected as HTMLElement).focus({ preventScroll: true });
    else if (!selected && focused instanceof HTMLElement && this.root.contains(focused)) focused.blur();
  }
}
