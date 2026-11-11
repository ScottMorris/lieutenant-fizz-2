// The Music Lab page: lists the tunes, plays one with sections you can tap between, shows its checks and saves its files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything the page plays or saves is made here, in the browser, by Undertone from the tune sources. Nothing is fetched and no
// audio file is shipped. The page needs a click before it may make sound, so the audio context is made by the first Play.

import { createMasterBus, enableMultichannel, foldToStereo } from '@liminal-hq/undertone';
import { noteEvents, tickSeconds } from '../../assets/music/events.ts';
import { HUMAN, buildTune, type BuiltTune } from './build.ts';
import { saveFile } from './files.ts';
import { COLUMNS, ROWS, patternById, riffOf } from './patterns.ts';
import { LabPlayer, type Quantise, type Step } from './player.ts';
import { SURROUND_CHANNELS, renderTune } from './render.ts';
import { degreeMidi, noteName } from './scale.ts';
import { SURROUND_TUNE, TUNES, type LabTune } from './tunes.ts';
import { validateTune, type Check } from './validate.ts';
import { EGA, VIZ_HEIGHT, VIZ_WIDTH, makeVisualizers, type VizFrame } from './viz.ts';
import type { Mix } from './voices.ts';

type Child = Node | string | null | undefined | false;

function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Record<string, string | boolean | number> = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === false && !key.startsWith('aria-')) continue;
    el.setAttribute(key, value === true && !key.startsWith('aria-') ? '' : String(value));
  }
  for (const child of children) if (child) el.append(child);
  return el;
}

/** Each tune's colour, from the sixteen. */
const COLOUR: Record<string, string> = {
  beam: 'var(--yellow)',
  press: 'var(--light-red)',
  keys: 'var(--light-green)',
  crystal: 'var(--light-cyan)',
  tanker: 'var(--brown)',
  rooms: 'var(--light-blue)',
  'night-map': 'var(--white)',
  plan: 'var(--light-magenta)',
};

/** The same colours as hex, for the canvas. */
const EGA_INK: Record<string, string> = {
  beam: EGA.yellow,
  press: EGA.lightRed,
  keys: EGA.lightGreen,
  crystal: EGA.lightCyan,
  tanker: EGA.brown,
  rooms: EGA.lightBlue,
  'night-map': EGA.white,
  plan: EGA.lightMagenta,
};

const built = new Map<string, BuiltTune>();
const builtOf = (tune: LabTune): BuiltTune => {
  let b = built.get(tune.id);
  if (!b) built.set(tune.id, (b = buildTune(tune)));
  return b;
};
const checks = new Map<string, Check[]>();
const checksOf = (tune: LabTune): Check[] => {
  let c = checks.get(tune.id);
  if (!c) checks.set(tune.id, (c = validateTune(tune)));
  return c;
};

// ---------- State ----------

const state = {
  tune: TUNES[0]!,
  mix: 'stereo' as Mix,
  human: true,
  quantise: 'bar' as Quantise,
  hold: false,
  /** The visualizer showing, or 'off'. People who ask for less motion start with none. */
  viz: globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ? 'off' : 'lighthouse',
};

const visualizers = makeVisualizers();

// ---------- Audio ----------

interface Rig {
  ctx: AudioContext;
  /** Where stereo voices go: the limiter. */
  stereoIn: AudioNode;
  /** Where 7.1 voices go: an eight-channel bus, metered a speaker at a time. */
  surroundIn: AudioNode;
  stereoMeters: AnalyserNode[];
  surroundMeters: AnalyserNode[];
  /** How many speakers the output really has. */
  speakers: number;
}

let rig: Rig | null = null;
let player: LabPlayer | null = null;

function makeRig(): Rig {
  const ctx = new AudioContext();
  const speakers = enableMultichannel(ctx);
  const meter = (source: AudioNode, channel: number): AnalyserNode => {
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 1024;
    // A range that suits music at this level, for the pictures that read the spectrum.
    analyser.minDecibels = -78;
    analyser.maxDecibels = -18;
    analyser.smoothingTimeConstant = 0.6;
    source.connect(analyser, channel);
    return analyser;
  };

  // Stereo: the limiter, then the speakers, with a meter on each side.
  const out = ctx.createGain();
  out.connect(ctx.destination);
  const master = createMasterBus(ctx, { output: out });
  const split2 = ctx.createChannelSplitter(2);
  out.connect(split2);

  // 7.1: eight discrete channels. Each is metered; then they go to eight real speakers if the output has them, and otherwise
  // through Undertone's own fold-down gains into the stereo limiter.
  const bus = ctx.createGain();
  bus.channelCount = 8;
  bus.channelCountMode = 'explicit';
  bus.channelInterpretation = 'discrete';
  const split8 = ctx.createChannelSplitter(8);
  bus.connect(split8);
  if (speakers >= 8) {
    bus.connect(ctx.destination);
  } else {
    const fold = ctx.createChannelMerger(2);
    for (let channel = 0; channel < 8; channel++) {
      const unit = Array.from({ length: 8 }, (_, i) => (i === channel ? 1 : 0));
      foldToStereo(unit).forEach((level, side) => {
        if (level <= 0) return;
        const gain = ctx.createGain();
        gain.gain.value = level;
        split8.connect(gain, channel);
        gain.connect(fold, 0, side);
      });
    }
    fold.connect(master.input as unknown as AudioNode);
  }
  return {
    ctx,
    stereoIn: master.input as unknown as AudioNode,
    surroundIn: bus,
    stereoMeters: [0, 1].map((c) => meter(split2, c)),
    surroundMeters: Array.from({ length: 8 }, (_, c) => meter(split8, c)),
    speakers,
  };
}

function stopPlayer(): void {
  player?.stop();
  player = null;
}

function play(section = 0): void {
  rig ??= makeRig();
  void rig.ctx.resume();
  stopPlayer();
  const options = { mix: state.mix, human: state.human };
  player = new LabPlayer(rig.ctx as never, state.mix === 'surround' ? rig.surroundIn : rig.stereoIn, builtOf(state.tune), options);
  player.hold = state.hold;
  player.start(section);
}

/** Restarts the player where it is after a change of mix: the two mixes go to different buses. */
function replay(): void {
  if (!player || !rig) return;
  const at = player.stepAt(rig.ctx.currentTime);
  play(at?.section ?? 0);
}

// ---------- Drawing ----------

function grid(patternId: string, base: number, colour: string, live = false): HTMLElement {
  const pattern = patternById(patternId);
  const riff = riffOf(pattern, base);
  const el = h('div', { class: live ? 'grid live' : 'grid', role: 'img', 'aria-label': `${pattern.concept}: ${pattern.idea}` });
  el.style.setProperty('--ink', colour);
  for (let row = 0; row < ROWS; row++) {
    for (let column = 0; column < COLUMNS; column++) {
      const filled = pattern.grid[row]![column] === '#';
      const degree = ROWS - 1 - row + base;
      const kind = !filled ? 'off' : riff.top[column] === degree ? 'top' : riff.low[column] === degree ? 'low' : 'on';
      el.append(h('i', { class: kind, 'data-column': column }));
    }
  }
  return el;
}

const root = document.getElementById('lab')!;
const cartridges = h('div', { class: 'cartridges' });
const deck = h('section', { class: 'panel deck', 'aria-live': 'off' });
const status = h('p', { class: 'status', role: 'status' });

function drawCartridges(): void {
  cartridges.replaceChildren(
    ...TUNES.map((tune, index) => {
      const selected = tune.id === state.tune.id;
      const results = checks.get(tune.id);
      const passed = results?.filter((c) => c.pass).length;
      const card = h(
        'button',
        { class: 'cartridge', type: 'button', id: `tune-${tune.id}`, 'aria-pressed': selected },
        h('span', { class: 'number' }, String(index + 1)),
        h('span', { class: 'grids' }, ...[...new Set(tune.motifs.map((m) => m.pattern))].map((id) => grid(id, 0, COLOUR[tune.id]!))),
        h('span', { class: 'name' }, tune.title),
        h('span', { class: 'meta' }, `${tune.keyName} · ${tune.track.bpm} bpm`),
        h('span', { class: 'meta level' }, tune.level),
        h('span', { class: results && passed === results.length ? 'badge ok' : 'badge' }, results ? `${passed}/${results.length} checks` : 'checking'),
        tune.id === SURROUND_TUNE ? h('span', { class: 'badge surround' }, '7.1') : null,
      );
      card.style.setProperty('--ink', COLOUR[tune.id]!);
      card.addEventListener('click', () => select(tune));
      return card;
    }),
  );
}

function select(tune: LabTune): void {
  if (tune.id === state.tune.id) return;
  const wasPlaying = player !== null;
  stopPlayer();
  state.tune = tune;
  if (tune.id !== SURROUND_TUNE) state.mix = 'stereo';
  drawCartridges();
  drawDeck();
  if (wasPlaying) play();
}

function toggle(id: string, label: string, pressed: boolean, onChange: (next: boolean) => void): HTMLButtonElement {
  const button = h('button', { class: 'switch', type: 'button', id, 'aria-pressed': pressed }, label);
  button.addEventListener('click', () => {
    const next = button.getAttribute('aria-pressed') !== 'true';
    button.setAttribute('aria-pressed', String(next));
    onChange(next);
  });
  return button;
}

function choice<T extends string>(name: string, legend: string, options: [T, string][], value: T, onChange: (next: T) => void): HTMLElement {
  const group = h('fieldset', { class: 'choice' }, h('legend', {}, legend));
  for (const [key, label] of options) {
    const input = h('input', { type: 'radio', name, id: `${name}-${key}`, value: key, checked: key === value });
    input.addEventListener('change', () => onChange(key));
    group.append(h('label', { for: `${name}-${key}` }, input, h('span', {}, label)));
  }
  return group;
}

let strip: HTMLButtonElement[] = [];
let meters: HTMLElement[] = [];
let liveGrids: HTMLElement[] = [];
let playButton: HTMLButtonElement;
const screen = h('canvas', { class: 'screen', width: VIZ_WIDTH, height: VIZ_HEIGHT, 'aria-hidden': 'true' });
const pen = screen.getContext('2d');

/** Where each 7.1 speaker sits round the listener, as a percentage of the ring's box. */
const RING_AT: [number, number][] = [
  [22, 12],
  [78, 12],
  [50, 4],
  [50, 30],
  [4, 50],
  [96, 50],
  [18, 90],
  [82, 90],
];
const RING_NAME = ['FL', 'FR', 'C', 'SUB', 'SL', 'SR', 'RL', 'RR'];

function drawMeters(): HTMLElement {
  const surround = state.mix === 'surround';
  const box = h('div', { class: surround ? 'ring' : 'ring stereo', 'aria-hidden': 'true' }, h('span', { class: 'listener' }));
  meters = (surround ? RING_NAME : ['L', 'R']).map((label, i) => {
    const speaker = h('span', { class: 'speaker' }, label);
    const [x, y] = surround ? RING_AT[i]! : ([i === 0 ? 16 : 84, 50] as [number, number]);
    speaker.style.left = `${x}%`;
    speaker.style.top = `${y}%`;
    box.append(speaker);
    return speaker;
  });
  return box;
}

function drawChecks(tune: LabTune): HTMLElement {
  const results = checksOf(tune);
  const list = (group: Check['group'], title: string, note: string) =>
    h(
      'div',
      { class: 'checks' },
      h('h3', {}, title),
      h('p', { class: 'note' }, note),
      h(
        'ul',
        {},
        ...results
          .filter((c) => c.group === group)
          .map((c) => h('li', { class: c.pass ? 'pass' : 'fail' }, h('b', {}, c.pass ? 'OK' : 'NO'), h('span', {}, h('strong', {}, c.label), h('br'), c.detail))),
      ),
    );
  return h(
    'div',
    { class: 'checkgrid' },
    list('theory', 'Music theory', 'Read from the notes: the key, how the tune meets the bass, how it moves, and whether it really starts from its pattern.'),
    list('feel', 'Played by a person, quietly', 'Measured on the humanised notes the lab plays: how far each one is moved in time, level, pitch and place, and whether the moves have any pattern.'),
  );
}

function drawDeck(): void {
  const tune = state.tune;
  const b = builtOf(tune);
  const colour = COLOUR[tune.id]!;
  deck.style.setProperty('--ink', colour);

  playButton = h('button', { class: 'go', type: 'button', id: 'play' }, 'Play');
  playButton.addEventListener('click', () => {
    if (player) stopPlayer();
    else play();
    refreshTransport();
  });

  strip = b.sections.map((section, index) => {
    const button = h(
      'button',
      { class: 'section', type: 'button', id: `section-${index}`, title: `${section.name}: ${section.cycles} bars` },
      h('span', { class: 'fill' }),
      h('span', { class: 'label' }, section.name),
      h('span', { class: 'bars' }, `${section.cycles}`),
    );
    button.style.flexGrow = String(section.cycles);
    button.addEventListener('click', () => {
      if (!player) play(index);
      else player.call(index, state.quantise);
      refreshTransport();
    });
    return button;
  });

  const motifGrids = tune.motifs.map((motif) => {
    const pattern = patternById(motif.pattern);
    const riff = riffOf(pattern, motif.base)[motif.line];
    const notes = riff.map((cell) => (cell === null ? '·' : noteName(degreeMidi(tune.key, motif.octave, cell)))).join(' ');
    const g = grid(motif.pattern, motif.base, colour, true);
    return h(
      'figure',
      { class: 'motif' },
      g,
      h(
        'figcaption',
        {},
        h('strong', {}, pattern.concept),
        ` (${pattern.mood}). The ${motif.line === 'top' ? 'highest' : 'lowest'} cells are the ${motif.role}${motif.tie ? ', with runs held as one note' : ''}:`,
        h('code', {}, notes),
      ),
    );
  });
  liveGrids = motifGrids.map((figure) => figure.querySelector('.grid') as HTMLElement);

  const save = (label: string, id: string, run: () => Promise<void>) => {
    const button = h('button', { class: 'save', type: 'button', id }, label);
    button.addEventListener('click', async () => {
      button.disabled = true;
      try {
        await run();
      } catch (error) {
        status.textContent = `That did not work: ${(error as Error).message}`;
      }
      button.disabled = false;
    });
    return button;
  };
  const wav = (mix: Mix) => async () => {
    status.textContent = mix === 'surround' ? 'Rendering eight channels. This takes a few seconds.' : 'Rendering. This takes a few seconds.';
    await new Promise((resolve) => setTimeout(resolve, 30));
    const rendered = await renderTune(b, { mix, human: state.human });
    const name = `fizz2-${tune.id}${mix === 'surround' ? '-7.1' : ''}.wav`;
    const saved = await saveFile(name, rendered.wav, 'audio/wav');
    status.textContent = saved.ok
      ? `Saved ${saved.filename}: ${rendered.channels} channels, ${rendered.seconds.toFixed(1)} s, ${(rendered.wav.length / 1048576).toFixed(1)} MB${saved.filename.endsWith('.zip') ? ' (the WAV is inside the zip)' : ''}.`
      : saved.reason;
  };
  const notes = async () => {
    const events = noteEvents(b.layout);
    const lines = [
      `# ${tune.title}: ${tune.keyName}, ${tune.track.bpm} bpm, ${b.bars} bars, ${b.seconds.toFixed(2)} s`,
      `# sections: ${b.sections.map((s) => `${s.name} ${s.cycles}`).join(', ')}`,
      '# tick (96 to a bar)  seconds  part  note  length  velocity',
      ...events.map((e) => `${e.tick}\t${tickSeconds(b.layout.song, e.tick).toFixed(4)}\t${e.part}\t${e.midi === null ? e.noise : noteName(e.midi)}\t${e.length}\t${e.velocity}`),
    ];
    const saved = await saveFile(`fizz2-${tune.id}-notes.txt`, `${lines.join('\n')}\n`, 'text/plain');
    status.textContent = saved.ok ? `Saved ${saved.filename}: ${events.length} notes.` : saved.reason;
  };
  const source = async () => {
    const body = {
      title: tune.title,
      level: tune.level,
      key: tune.keyName,
      bpm: tune.track.bpm,
      patterns: [...new Set(tune.motifs.map((m) => m.pattern))].map((id) => patternById(id)),
      motifs: tune.motifs,
      song: tune.def,
      parts: tune.track.parts,
      sections: b.sections.map(({ name, cycles, startBar, shift }) => ({ name, bars: cycles, startBar, shift })),
      humanising: HUMAN,
      checks: checksOf(tune),
    };
    const saved = await saveFile(`fizz2-${tune.id}.json`, `${JSON.stringify(body, null, 2)}\n`, 'application/json');
    status.textContent = saved.ok ? `Saved ${saved.filename}.` : saved.reason;
  };

  const surround = tune.id === SURROUND_TUNE;
  status.textContent = '';
  screen.hidden = state.viz === 'off';
  const parts: Child[] = [
    h('h2', { class: 'tab' }, 'Now on the deck'),
    h(
      'div',
      { class: 'about' },
      h('div', {}, h('h3', { class: 'title' }, tune.title), h('p', { class: 'kind' }, `${tune.kind} · ${tune.level} · ${tune.keyName}, ${tune.track.bpm} bpm`), h('p', {}, tune.story), h('p', { class: 'stereo' }, h('b', {}, 'In the speakers: '), tune.stereo)),
      drawMeters(),
    ),
    h(
      'div',
      { class: 'transport' },
      playButton,
      toggle('hold', 'Hold section', state.hold, (next) => {
        state.hold = next;
        if (player) player.hold = next;
      }),
      toggle('human', 'Human feel', state.human, (next) => {
        state.human = next;
        player?.setOptions({ mix: state.mix, human: next });
      }),
      surround
        ? toggle('surround', '7.1 mix', state.mix === 'surround', (next) => {
            state.mix = next ? 'surround' : 'stereo';
            const was = deck.querySelector('.ring')!;
            was.replaceWith(drawMeters());
            replay();
          })
        : null,
      choice<Quantise>(
        'quantise',
        'A tap changes section',
        [
          ['beat', 'on the beat'],
          ['bar', 'at the bar, with a turn'],
          ['section', 'when this one ends'],
        ],
        state.quantise,
        (next) => (state.quantise = next),
      ),
    ),
    h(
      'div',
      { class: 'viz' },
      screen,
      choice<string>('viz', 'Picture', [...visualizers.map((v): [string, string] => [v.id, v.name]), ['off', 'Off']], state.viz, (next) => {
        state.viz = next;
        screen.hidden = next === 'off';
      }),
    ),
    h('div', { class: 'strip', role: 'group', 'aria-label': 'Sections. Tap one to send the song there.' }, ...strip),
    h('p', { class: 'hint' }, 'Tap a section to send the song there. At the bar, the drums and bass turn into a roll and a walk-up that lands on the new section’s first note. Tap the same section again to cancel.'),
    h('div', { class: 'motifs' }, ...motifGrids),
    h(
      'div',
      { class: 'saves' },
      save('Save stereo WAV', 'save-stereo', wav('stereo')),
      surround ? save('Save 7.1 WAV', 'save-surround', wav('surround')) : null,
      save('Save note list', 'save-notes', notes),
      save('Save tune source', 'save-source', source),
    ),
    surround ? h('p', { class: 'hint' }, `The 7.1 file has eight channels: ${SURROUND_CHANNELS.join(', ').toLowerCase()}. Played here on two speakers, the 7.1 mix is folded down to stereo; the ring above shows all eight.`) : null,
    status,
    drawChecks(tune),
  ];
  deck.replaceChildren(...parts.filter((part): part is Node => part instanceof Node));
  refreshTransport();
}

function refreshTransport(): void {
  playButton.textContent = player ? 'Stop' : 'Play';
  playButton.setAttribute('aria-pressed', String(player !== null));
}

// ---------- The frame loop ----------

const LEVELS = ['l0', 'l1', 'l2', 'l3', 'l4'];
const scratch = new Float32Array(1024);

function levelOf(analyser: AnalyserNode): number {
  analyser.getFloatTimeDomainData(scratch);
  let peak = 0;
  for (let i = 0; i < scratch.length; i++) peak = Math.max(peak, Math.abs(scratch[i]!));
  return peak;
}

const wave = { left: new Float32Array(1024), right: new Float32Array(1024), spectrum: new Uint8Array(512), other: new Uint8Array(512) };

/** Draws the chosen visualizer. Before the first Play there is no audio yet, so it draws the picture at rest. */
function drawViz(step: Step | null, now: number): void {
  const viz = visualizers.find((v) => v.id === state.viz);
  if (!viz || !pen) return;
  if (rig && player) {
    // The two sides of what is heard: the stereo output, or the front pair when eight real speakers are in use.
    const direct = state.mix === 'surround' && rig.speakers >= 8;
    const [l, r] = direct ? rig.surroundMeters : rig.stereoMeters;
    l!.getFloatTimeDomainData(wave.left);
    r!.getFloatTimeDomainData(wave.right);
    l!.getByteFrequencyData(wave.spectrum);
    r!.getByteFrequencyData(wave.other);
    for (let i = 0; i < wave.spectrum.length; i++) wave.spectrum[i] = Math.max(wave.spectrum[i]!, wave.other[i]!);
  } else {
    wave.left.fill(0);
    wave.right.fill(0);
    wave.spectrum.fill(0);
  }
  const beatSeconds = player ? player.barSeconds / 4 : 1;
  const frameData: VizFrame = {
    time: performance.now() / 1000,
    left: wave.left,
    right: wave.right,
    spectrum: wave.spectrum,
    playing: player !== null && step !== null,
    beat: step?.beat ?? 0,
    phase: step ? Math.min(1, Math.max(0, (now - step.time) / beatSeconds)) : 0,
    bar: step?.bar ?? 0,
    turning: step?.turning ?? false,
    ink: EGA_INK[state.tune.id]!,
  };
  viz.draw(pen, frameData);
}

function frame(): void {
  requestAnimationFrame(frame);
  if (!rig) {
    drawViz(null, 0);
    return;
  }
  const now = rig.ctx.currentTime;
  const step = player?.stepAt(now) ?? null;
  const queued = player?.queued ?? null;
  strip.forEach((button, index) => {
    const current = step?.section === index;
    button.classList.toggle('current', current);
    button.classList.toggle('queued', queued === index);
    button.classList.toggle('turning', current && !!step?.turning);
    const fill = button.firstElementChild as HTMLElement;
    if (current && step && player) {
      const section = player.built.sections[index]!;
      const beatSeconds = player.barSeconds / 4;
      const along = (step.bar * 4 + step.beat + Math.min(1, (now - step.time) / beatSeconds)) / (section.cycles * 4);
      fill.style.width = `${Math.min(100, along * 100)}%`;
    } else fill.style.width = '0';
  });
  const column = step && player ? Math.floor(((step.beat + Math.min(0.999, (now - step.time) / (player.barSeconds / 4))) / 4) * COLUMNS) : -1;
  for (const g of liveGrids) {
    if (g.dataset.at === String(column)) continue;
    g.dataset.at = String(column);
    for (const cell of g.children) cell.classList.toggle('now', (cell as HTMLElement).dataset.column === String(column));
  }
  drawViz(step, now);
  const analysers = state.mix === 'surround' ? rig.surroundMeters : rig.stereoMeters;
  meters.forEach((speaker, i) => {
    const level = player ? levelOf(analysers[i]!) : 0;
    const band = level > 0.5 ? 4 : level > 0.2 ? 3 : level > 0.06 ? 2 : level > 0.012 ? 1 : 0;
    if (speaker.dataset.level !== LEVELS[band]) speaker.dataset.level = LEVELS[band]!;
  });
}

// ---------- Start ----------

root.replaceChildren(
  h('section', { class: 'panel' }, h('h2', { class: 'tab' }, 'Eight tunes'), h('p', { class: 'lede' }, 'Six pictures of the game, each read as a riff and grown into a song of about a minute; then two more that braid three pictures each. Pick one.'), cartridges),
  deck,
);
drawCartridges();
drawDeck();
requestAnimationFrame(frame);

// The checks for the other tunes are worked out a tune at a time, after the first paint, so the badges fill in without a stall.
let pending = TUNES.filter((tune) => !checks.has(tune.id));
const work = () => {
  const tune = pending.shift();
  if (!tune) return;
  checksOf(tune);
  drawCartridges();
  setTimeout(work, 30);
};
setTimeout(work, 200);

document.addEventListener('keydown', (event) => {
  if (event.target instanceof HTMLInputElement || event.metaKey || event.ctrlKey || event.altKey) return;
  const index = Number(event.key) - 1;
  if (Number.isInteger(index) && index >= 0 && index < strip.length) strip[index]!.click();
});
