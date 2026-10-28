// Tests the Web music player against the real Undertone and a recording stand-in for the browser's AudioContext.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Run with `bun test web/music.test.ts`. Nothing here makes a sound: the stand-in records which sources Undertone schedules, when,
// at what pitch and pan, which proves that the page plays the notes of `assets/music` (the same list `noteEvents()` gives the DOS
// build) and follows its lifecycle rules. How the song sounds has not been listened to.
import { describe, expect, test } from 'bun:test';
import { levelSong } from '../assets/music/songs.ts';
import { noteEvents, tickSeconds, type NoteEvent } from '../assets/music/events.ts';
import { PART_PAN, partPanAt } from '../assets/music/types.ts';
import { AudioRoot, MUSIC_BUS_GAIN, SFX_BUS_GAIN, type RootContext } from './audio.ts';
import { MUSIC_KEY, Music, PANNED_MAKEUP, mirrorPan, partMakeup, undertonePan, type StorageLike } from './music.ts';
import { Sfx } from './sfx.ts';

interface Param {
  value: number;
  sets: [number, number][];
  setValueAtTime(v: number, t: number): void;
  linearRampToValueAtTime(v: number, t: number): void;
  exponentialRampToValueAtTime(v: number, t: number): void;
}
const param = (): Param => {
  const p: Param = {
    value: 0,
    sets: [],
    setValueAtTime(v, t) {
      p.sets.push([v, t]);
    },
    linearRampToValueAtTime() {},
    exponentialRampToValueAtTime() {},
  };
  return p;
};

/** What one source (oscillator or noise) and the nodes built after it were told. */
interface Voice {
  noise: boolean;
  start: number;
  hz: number | null;
  pan: number | null;
  gain: number | null;
}

class FakeContext {
  state = 'suspended';
  currentTime = 0;
  sampleRate = 44100;
  destination = { connect() {}, kind: 'destination' };
  nodes: { kind: string; gain?: Param; pan?: Param; connectedTo?: unknown }[] = [];
  voices: Voice[] = [];
  resumed = 0;
  suspended = 0;
  private cur: Voice | null = null;

  private node(kind: string, extra: object = {}) {
    const n = {
      kind,
      connectedTo: undefined as unknown,
      connect(to: unknown) {
        n.connectedTo = to;
      },
      start() {},
      stop() {},
      ...extra,
    };
    this.nodes.push(n);
    return n;
  }
  private source(noise: boolean, extra: object) {
    const v: Voice = { noise, start: NaN, hz: null, pan: null, gain: null };
    this.voices.push(v);
    this.cur = v;
    const n = this.node(noise ? 'noise' : 'osc', {
      ...extra,
      start(t = 0) {
        v.start = t;
      },
    });
    return n;
  }
  createOscillator() {
    const v = this;
    const f = param();
    const set = f.setValueAtTime.bind(f);
    f.setValueAtTime = (val, t) => {
      set(val, t);
      // The slide's first value is a multiple of the target; the target is the last set or ramp, so keep the first set only for a plain note.
      if (v.cur && v.cur.hz === null) v.cur.hz = val;
    };
    // A slide (the kick) starts an octave up and ramps to the note: the ramp's target is the pitch.
    f.exponentialRampToValueAtTime = (val) => {
      if (v.cur) v.cur.hz = val;
    };
    return this.source(false, { frequency: f }) as never;
  }
  createBufferSource() {
    return this.source(true, { playbackRate: param() }) as never;
  }
  createGain() {
    const g = param();
    const set = g.setValueAtTime.bind(g);
    const cur = this.cur;
    let first = true;
    g.setValueAtTime = (val, t) => {
      set(val, t);
      if (cur && first && cur.gain === null && val > 0) {
        cur.gain = val;
        first = false;
      }
    };
    return this.node('gain', { gain: g }) as never;
  }
  createBiquadFilter() {
    return this.node('filter', { frequency: param(), Q: param() }) as never;
  }
  createStereoPanner() {
    const p = param();
    const cur = this.cur;
    const set = p.setValueAtTime.bind(p);
    p.setValueAtTime = (val, t) => {
      set(val, t);
      if (cur) cur.pan = val;
    };
    return this.node('pan', { pan: p }) as never;
  }
  createDynamicsCompressor() {
    return this.node('limiter', {
      threshold: param(),
      knee: param(),
      ratio: param(),
      attack: param(),
      release: param(),
    }) as never;
  }
  createChannelMerger() {
    return this.node('merger') as never;
  }
  createBuffer(_c: number, length: number) {
    return { getChannelData: () => new Float32Array(length) };
  }
  createConvolver() {
    return this.node('convolver') as never;
  }
  createDelay() {
    return this.node('delay', { delayTime: param() }) as never;
  }
  decodeAudioData() {
    return Promise.reject(new Error('unused'));
  }
  async resume() {
    this.resumed++;
    await Promise.resolve();
    this.state = 'running';
  }
  async suspend() {
    this.suspended++;
    this.state = 'suspended';
  }
}

class Memory implements StorageLike {
  data = new Map<string, string>();
  getItem(k: string) {
    return this.data.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.data.set(k, v);
  }
}

/** A loop timer the test steps by hand, so a minute of song runs in milliseconds. */
function manualTimer() {
  let cb: (() => void) | null = null;
  return {
    setInterval: (f: () => void) => {
      cb = f;
      return 1;
    },
    clearInterval: () => {
      cb = null;
    },
    get active() {
      return cb !== null;
    },
    tick: () => cb?.(),
  };
}

function setup(opts: { storage?: StorageLike | null; muted?: boolean } = {}) {
  const ctx = new FakeContext();
  const root = new AudioRoot({ createContext: () => ctx as unknown as RootContext });
  const timer = manualTimer();
  const storage = opts.storage === undefined ? new Memory() : opts.storage;
  const sfx = new Sfx({ createContext: () => root.context(), destination: () => root.sfxBus, isHidden: () => false, storage: new Memory() });
  if (opts.muted) sfx.setMuted(true);
  const music = new Music(root, { storage, isMuted: () => sfx.muted, timer });
  /** Runs the clock forward, ticking the loop's scheduler every 0.1 s as the browser's timer would. */
  const run = (seconds: number) => {
    const end = ctx.currentTime + seconds;
    while (ctx.currentTime < end - 1e-9) {
      ctx.currentTime += 0.1;
      timer.tick();
    }
  };
  return { ctx, root, sfx, music, timer, run };
}

const layout = levelSong();
const events = noteEvents(layout);
const loopSeconds = tickSeconds(layout.song, layout.sections.reduce((n, s) => n + s.cycles, 0) * 96);
const HZ = (midi: number) => 440 * 2 ** ((midi - 69) / 12);

/** The part an event came from, to read its nudge. */
function partOf(e: NoteEvent) {
  const section = [...layout.sections].reverse().find((s) => s.startBar * 96 <= e.tick)!;
  return layout.song.streams[e.stream]![layout.sections.indexOf(section)]!;
}

const key = (r: readonly [number, string, number | string]) => `${r[0].toFixed(4)}|${String(r[1]).padStart(9, '0')}|${r[2]}`;
const byKey = (a: readonly [number, string, number | string], b: readonly [number, string, number | string]) => (key(a) < key(b) ? -1 : key(a) > key(b) ? 1 : 0);

describe('the pan table', () => {
  test('centred roles get no panner and no make-up; panned roles get both', () => {
    for (const role of ['bass', 'kick', 'snare', 'pad', 'drone'] as const) {
      expect(undertonePan(role)).toBeUndefined();
      expect(partMakeup(role)).toBe(1);
    }
    expect(undertonePan('lead')).toBe(-0.15);
    expect(undertonePan('hats')).toBe(0.25);
    expect(partMakeup('lead')).toBe(PANNED_MAKEUP);
    expect(PANNED_MAKEUP).toBeCloseTo(Math.SQRT2, 12);
  });

  test('the arpeggio alternates and its partner is the mirror image', () => {
    expect(undertonePan('arp')).toBe('[-0.3 0.3]*4');
    expect(mirrorPan('arp', 0.4)).toBe('[0.3 -0.3]*4');
  });

  test('a partner of a panned role sits opposite it, and of a centred role on its own side', () => {
    expect(mirrorPan('lead', 0.4)).toBe(0.15);
    expect(mirrorPan('bass', -0.2)).toBe(-0.2);
  });
});

describe('the audio root', () => {
  test('makes nothing until context() is called, then one context with two buses and a limiter', () => {
    const ctx = new FakeContext();
    const root = new AudioRoot({ createContext: () => ctx as unknown as RootContext });
    expect(root.existing).toBeNull();
    expect(ctx.nodes.length).toBe(0);
    const a = root.context();
    expect(root.context()).toBe(a);
    const gains = ctx.nodes.filter((n) => n.kind === 'gain');
    expect(gains.map((g) => g.gain!.value)).toEqual([MUSIC_BUS_GAIN, SFX_BUS_GAIN]);
    const limiter = ctx.nodes.find((n) => n.kind === 'limiter')!;
    expect(gains[0].connectedTo).toBe(limiter);
    expect(gains[1].connectedTo).toBe(limiter);
    expect(limiter.connectedTo).toBe(ctx.destination);
  });
});

describe('the song over one loop', () => {
  test('schedules the same note starts and pitches as noteEvents(), at the pans of their roles', () => {
    const { ctx, root, music, run } = setup();
    root.context();
    music.sync();
    expect(music.playing).toBe(true);
    run(loopSeconds + 1);
    const limit = 0.1 + loopSeconds - 1e-6;
    const got = ctx.voices
      .filter((v) => v.start < limit)
      .map((v) => [v.start, v.noise ? 'n' : v.hz!.toFixed(2), v.pan === null ? 'c' : v.pan] as const)
      .sort(byKey);
    const want = events
      .map((e) => {
        const part = partOf(e);
        let pan: number | 'c' = 'c';
        if (e.role && PART_PAN[e.role] !== 0) {
          const p = PART_PAN[e.role];
          const own = typeof p === 'number' ? p : partPanAt(e.role, (e.tick % 96) / 96);
          const other = typeof p === 'number' ? -p : partPanAt(e.role, (e.tick % 96) / 96) * -1;
          pan = e.partner ? other : own;
        } else if (e.partner) {
          pan = part.mirror!;
        }
        return [0.1 + tickSeconds(layout.song, e.tick) + (part.nudge ?? 0), e.midi === null ? 'n' : HZ(e.midi).toFixed(2), pan] as const;
      })
      .sort(byKey);
    expect(got.length).toBe(events.length);
    for (let i = 0; i < want.length; i++) {
      expect(got[i][0]).toBeCloseTo(want[i][0], 4);
      expect(got[i][1]).toBe(want[i][1]);
      expect(got[i][2]).toBe(want[i][2]);
    }
  });

  test('loops from the end straight back to the start with no gap', () => {
    const { ctx, root, music, run } = setup();
    root.context();
    music.sync();
    run(loopSeconds + 1);
    const first = ctx.voices.filter((v) => v.start < 0.1 + 0.5).map((v) => v.start - 0.1);
    const second = ctx.voices.filter((v) => v.start >= 0.1 + loopSeconds - 1e-6 && v.start < 0.1 + loopSeconds + 0.5).map((v) => v.start - 0.1 - loopSeconds);
    expect(second.length).toBe(first.length);
    first.sort((a, b) => a - b);
    second.sort((a, b) => a - b);
    first.forEach((t, i) => expect(second[i]).toBeCloseTo(t, 4));
    // The widest silence is the song's own (its breakdown): looping adds no gap beyond the final note's distance to bar 1.
    const widestOf = (starts: number[]) => {
      starts.sort((a, b) => a - b);
      let w = 0;
      for (let i = 1; i < starts.length; i++) w = Math.max(w, starts[i] - starts[i - 1]);
      return w;
    };
    const wanted = events.map((e) => tickSeconds(layout.song, e.tick));
    const twice = [...wanted, ...wanted.map((t) => t + loopSeconds)];
    const played = ctx.voices.filter((v) => v.start < 0.1 + 2 * loopSeconds - 1e-6).map((v) => v.start - 0.1);
    expect(widestOf(played)).toBeLessThanOrEqual(widestOf(twice) + 0.02);
  });

  test('music voices end in the music bus, not the speakers', () => {
    const { ctx, root, music } = setup();
    root.context();
    music.sync();
    const bus = root.musicBus;
    // Every gain the voices made that connected straight to a destination connected to the bus.
    const toDestination = ctx.nodes.filter((n) => n.connectedTo === ctx.destination);
    expect(toDestination.length).toBe(1); // only the limiter
    expect(ctx.nodes.some((n) => n.connectedTo === bus)).toBe(true);
  });
});

describe('lifecycle', () => {
  test('nothing is made or scheduled before unlock', () => {
    const { ctx, root, music, run } = setup();
    music.sync();
    run(2);
    expect(music.playing).toBe(false);
    expect(root.existing).toBeNull();
    expect(ctx.nodes.length).toBe(0);
  });

  test('the first unlock starts the song at bar 1', () => {
    const { ctx, sfx, music } = setup();
    sfx.unlock();
    music.sync();
    expect(music.playing).toBe(true);
    // Bar 1: a bass voice, a pad and hats are scheduled at the first beat (0.1 s of start latency).
    const first = ctx.voices.filter((v) => Math.abs(v.start - 0.1) < 1e-6);
    expect(first.length).toBeGreaterThan(0);
    // A second call does not start a second song.
    const n = ctx.voices.length;
    music.sync();
    expect(ctx.voices.length).toBe(n);
  });

  test('muted, the song does not start; M then starts it from bar 1', () => {
    const { ctx, sfx, music } = setup({ muted: true });
    sfx.unlock();
    music.sync();
    expect(music.playing).toBe(false);
    expect(ctx.voices.length).toBe(0);
    sfx.toggleMute();
    music.sync();
    expect(music.playing).toBe(true);
  });

  test('M suspends the shared context, which freezes the song, and unmuting resumes it', async () => {
    const { ctx, sfx, music, run } = setup();
    sfx.unlock();
    await Promise.resolve();
    await Promise.resolve();
    music.sync();
    run(1);
    expect(ctx.state).toBe('running');
    sfx.toggleMute();
    music.sync();
    expect(ctx.state).toBe('suspended');
    expect(music.playing).toBe(true);
    sfx.toggleMute();
    music.sync();
    await Promise.resolve();
    expect(ctx.state).toBe('running');
  });

  test('the Music toggle stops and restarts only the music, and is remembered', () => {
    const storage = new Memory();
    const { ctx, sfx, music, timer } = setup({ storage });
    sfx.unlock();
    music.sync();
    expect(music.enabled).toBe(true);
    music.toggle();
    expect(music.enabled).toBe(false);
    expect(music.playing).toBe(false);
    expect(timer.active).toBe(false);
    expect(storage.getItem(MUSIC_KEY)).toBe('off');
    expect(sfx.muted).toBe(false);
    expect(ctx.suspended).toBe(0);
    // A new page with the same storage starts with music off, and does not play at unlock.
    const again = setup({ storage });
    expect(again.music.enabled).toBe(false);
    again.sfx.unlock();
    again.music.sync();
    expect(again.music.playing).toBe(false);
    expect(again.ctx.voices.length).toBe(0);
    // Turning it back on starts from bar 1 and is remembered as on.
    music.toggle();
    expect(music.playing).toBe(true);
    expect(storage.getItem(MUSIC_KEY)).toBe('on');
  });

  test('music is on by default, and a storage that throws is not fatal', () => {
    expect(setup().music.enabled).toBe(true);
    const bad: StorageLike = {
      getItem() {
        throw new Error('blocked');
      },
      setItem() {
        throw new Error('blocked');
      },
    };
    const { music, sfx } = setup({ storage: bad });
    expect(music.enabled).toBe(true);
    sfx.unlock();
    expect(() => music.toggle()).not.toThrow();
    expect(music.enabled).toBe(false);
  });

  test('if Undertone fails there is no music and one warning, not one per call', () => {
    const ctx = new FakeContext();
    const root = new AudioRoot({ createContext: () => ctx as unknown as RootContext });
    const broken = {
      note() {
        throw new Error('boom');
      },
      sound() {
        throw new Error('boom');
      },
      stack() {
        throw new Error('boom');
      },
      arrange() {
        throw new Error('boom');
      },
      silence: {},
    } as never;
    const music = new Music(root, { undertone: broken, storage: new Memory() });
    root.context();
    const warn = console.warn;
    let warnings = 0;
    console.warn = () => {
      warnings++;
    };
    try {
      music.sync();
      music.sync();
      music.sync();
    } finally {
      console.warn = warn;
    }
    expect(music.playing).toBe(false);
    expect(warnings).toBe(1);
  });
});

describe('the cluck keeps working on the shared root', () => {
  test('a cluck ends in the sfx bus and is unchanged by the music', async () => {
    const { ctx, root, sfx, music } = setup();
    sfx.unlock();
    await Promise.resolve();
    await Promise.resolve();
    music.sync();
    const before = ctx.voices.length;
    expect(sfx.playCluck(0)).toBe(true);
    expect(ctx.voices.length).toBeGreaterThan(before);
    expect(ctx.nodes.some((n) => n.connectedTo === root.sfxBus)).toBe(true);
  });
});
