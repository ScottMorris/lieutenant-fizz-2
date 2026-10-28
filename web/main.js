// Browser shell for the Web presenter: keyboard in, the slice drawn to the canvas, a text HUD out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every URL is relative so the bundle works under any base path. The wasm module is the only import. The Rust side draws the frame
// into its own memory; this file only copies those pixels onto the canvas and prints the HUD.
import init, { App } from "./pkg/fizz_present_web.js";
import { Sfx } from "./sfx.ts";

// Button bits match `fizz_sim::Buttons`.
const UP = 1;
const DOWN = 2;
const LEFT = 4;
const RIGHT = 8;
const JUMP = 16;
const POGO = 32;
const FIRE = 64;

// The two layouts are exclusive sets, keyed on `event.code` so they follow the key's position, not the keyboard language. The arrows
// move and Space fires in both. Keen is Fizz 1's classic layout and the default; Modern keeps clear of the Ctrl and Alt shortcuts.
const LAYOUTS = {
  keen: {
    label: "Keen",
    hint: "Arrow keys move, Ctrl jumps, Alt switches the pogo stick on and off, Space fires.",
    keys: {
      ArrowUp: UP,
      ArrowDown: DOWN,
      ArrowLeft: LEFT,
      ArrowRight: RIGHT,
      ControlLeft: JUMP,
      ControlRight: JUMP,
      AltLeft: POGO,
      AltRight: POGO,
      Space: FIRE,
    },
  },
  modern: {
    label: "Modern",
    hint: "Arrow keys or W A S D move, Z jumps, X switches the pogo stick on and off, C or Space fires.",
    keys: {
      ArrowUp: UP,
      KeyW: UP,
      ArrowDown: DOWN,
      KeyS: DOWN,
      ArrowLeft: LEFT,
      KeyA: LEFT,
      ArrowRight: RIGHT,
      KeyD: RIGHT,
      KeyZ: JUMP,
      KeyX: POGO,
      KeyC: FIRE,
      Space: FIRE,
    },
  },
};
const OTHER = { keen: "modern", modern: "keen" };
const STORAGE_KEY = "fizz2-layout";
const CAPTIONS_KEY = "fizz2-captions";

// Every key either layout uses, for the Keyboard Lock request. Escape is left out so the browser's way out of fullscreen still works.
const LOCKED_KEYS = [...new Set(Object.values(LAYOUTS).flatMap((l) => Object.keys(l.keys)))];

const canvas = document.getElementById("screen");
const hud = document.getElementById("hud");
const hint = document.getElementById("hint");
const stage = document.getElementById("stage");
const panel = document.getElementById("panel");
const panelToggle = document.getElementById("panel-toggle");
const toast = document.getElementById("toast");
const resetButton = document.getElementById("reset");
const layoutButton =document.getElementById("layout");
const captionsButton = document.getElementById("captions");
const fullscreenButton = document.getElementById("fullscreen");
const soundButton = document.getElementById("sound");
const ctx = canvas.getContext("2d");
const captionLayer = document.getElementById("caption-layer");

// Cluck captions follow Fizz 1's Web text (design/melting/engine.js, `cap`): 15 px bold italic at a 960 px canvas, centred, rising
// from half a line to 1.8 lines above its anchor over the caption's life, popping in at 0.8 to 1.05 times its size, and fading
// out. The rest of the timing, which chicken has one and for how long, comes from the module (`caption_progress`).
const CAPTION_FONT_PX = 15;
const CAPTION_REFERENCE_WIDTH = 960;
// The word sits this many picture pixels above the chicken's feet: two Fizz 1 tiles, which is 32 here.
const CAPTION_ANCHOR_ABOVE_FEET = 32;
const CAPTION_POP = 0.2;
// CSS `ease-out`, near enough.
const easeOut = (t) => 1 - (1 - t) * (1 - t);
const lerp = (a, b, t) => a + (b - a) * t;
const captionEls = [];

// Fits the canvas to the window at 8:5: the largest whole-number multiple of the picture that fits or, in a window smaller than one
// picture, the largest fit. The browser scales with nearest-neighbour either way; the rest of the window is letterbox.
function fitStage() {
  const w = window.innerWidth;
  const h = window.innerHeight;
  const whole = Math.floor(Math.min(w / 320, h / 200));
  const width = whole >= 1 ? 320 * whole : Math.min(w, (h * 8) / 5);
  stage.style.width = `${Math.floor(width)}px`;
  stage.style.height = `${Math.floor((width * 5) / 8)}px`;
}

// Shows the module's live captions as text over the canvas. Position comes in picture pixels and follows the canvas's displayed
// size; the type size follows it too, but only modestly, so it stays readable on a narrow page and does not balloon in fullscreen.
function showCaptions(app) {
  const shownWidth = captionLayer.clientWidth;
  const shownHeight = captionLayer.clientHeight;
  const scale = shownWidth / app.width();
  const fontPx = Math.min(40, Math.max(11, (CAPTION_FONT_PX * shownWidth) / CAPTION_REFERENCE_WIDTH));
  const count = shownWidth > 0 ? app.caption_count() : 0;
  while (captionEls.length < count) {
    const el = document.createElement("div");
    el.className = "caption";
    captionLayer.appendChild(el);
    captionEls.push(el);
  }
  captionEls.forEach((el, i) => {
    if (i >= count) {
      el.hidden = true;
      return;
    }
    const word = app.caption_word(i);
    el.hidden = false;
    if (el.textContent !== word) el.textContent = word;
    el.style.fontSize = `${fontPx}px`;
    el.style.color = app.caption_colour(i);
    const p = app.caption_progress(i);
    // Fizz 1's keyframes: pop in over the first fifth, then drift up and fade.
    let lift, size, opacity;
    if (p < CAPTION_POP) {
      const t = easeOut(p / CAPTION_POP);
      lift = lerp(0.5, 0.6, t);
      size = lerp(0.8, 1.05, t);
      opacity = 1;
    } else {
      const t = easeOut((p - CAPTION_POP) / (1 - CAPTION_POP));
      lift = lerp(0.6, 1.8, t);
      size = lerp(1.05, 1, t);
      opacity = 1 - t;
    }
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    // The anchor is the word's centre; lift is in word heights. It follows the chicken exactly, as in Fizz 1: never moved to stay
    // inside, so the layer's overflow clips it as the chicken leaves. The module drops it a tile outside the picture.
    const cx = app.caption_x(i) * scale;
    const cy = (app.caption_y(i) - CAPTION_ANCHOR_ABOVE_FEET) * scale + (0.5 - lift) * h;
    el.style.opacity = String(opacity);
    el.style.transform = `translate(${cx - w / 2}px, ${cy - h / 2}px) scale(${size})`;
  });
}

function loadLayout() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved in LAYOUTS) return saved;
  } catch {
    // Storage can be blocked or missing; the default applies.
  }
  return "keen";
}

function saveLayout(name) {
  try {
    localStorage.setItem(STORAGE_KEY, name);
  } catch {
    // The choice just is not remembered.
  }
}

// Captions are on unless the player turned them off; only an explicit "off" is remembered as off.
function loadCaptions() {
  try {
    return localStorage.getItem(CAPTIONS_KEY) !== "off";
  } catch {
    // Storage can be blocked or missing; the default applies.
    return true;
  }
}

function saveCaptions(on) {
  try {
    localStorage.setItem(CAPTIONS_KEY, on ? "on" : "off");
  } catch {
    // The choice just is not remembered.
  }
}

// Copies the frame the module just drew onto the canvas. The view is made again each frame because the module's memory can move
// when it grows, which would leave an older view pointing at nothing.
function draw(app, memory) {
  app.render();
  const pixels = new Uint8ClampedArray(memory.buffer, app.rgba_ptr(), app.rgba_len());
  ctx.putImageData(new ImageData(pixels, app.width(), app.height()), 0, 0);
}

function showHud(app) {
  hud.textContent =
    `TICK ${app.tick()}\n` +
    `HASH ${app.hash_hex()}\n` +
    `AMMO ${app.ammo()}\n` +
    `KEY  ${app.has_key() ? "carried" : "no"}\n` +
    `DOOR ${app.door_open() ? "open" : "shut"}\n` +
    `DEATHS ${app.deaths()}\n` +
    `EXIT ${app.exit_reached() ? "reached" : "not yet"}`;
}

// Keyboard Lock (Chromium) keeps the browser from taking the game's keys while fullscreen. Elsewhere this does nothing.
async function setKeyLock(on) {
  const keyboard = navigator.keyboard;
  if (!keyboard) return;
  try {
    if (on) await keyboard.lock(LOCKED_KEYS);
    else keyboard.unlock();
  } catch {
    // Not allowed or not supported here: play on without it.
  }
}

async function toggleFullscreen() {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  } catch {
    // Fullscreen can be refused; nothing else to do.
  }
}

// The overlay panel holds the controls, notes and HUD. H or the corner button toggles it; it starts hidden.
function setPanel(open) {
  panel.hidden = !open;
  panelToggle.setAttribute("aria-expanded", String(open));
  if (open) toast.classList.add("gone");
}

async function main() {
  fitStage();
  addEventListener("resize", fitStage);
  panelToggle.addEventListener("click", () => {
    setPanel(panel.hidden);
    panelToggle.blur();
  });
  setTimeout(() => toast.classList.add("gone"), 6000);
  const wasm = await init();
  const app = new App();

  // Sound: made on the first key press or click (browsers insist on a gesture), muted with M. Neither layout uses M.
  const sfx = new Sfx();
  const showSound = () => {
    soundButton.textContent = `Sound: ${sfx.muted ? "off" : "on"}, M to toggle`;
    soundButton.setAttribute("aria-pressed", String(!sfx.muted));
  };
  showSound();
  soundButton.addEventListener("click", () => {
    sfx.unlock();
    sfx.toggleMute();
    showSound();
    soundButton.blur();
  });
  document.addEventListener("visibilitychange", () => sfx.sync());

  let layout = loadLayout();
  // Button bits held per physical key, so a release always undoes its own press even after a layout change.
  const held = new Map();

  const releaseAll = () => {
    for (const bits of held.values()) app.key(bits, false);
    held.clear();
  };

  const showLayout = () => {
    layoutButton.textContent = `Keys: ${LAYOUTS[layout].label}`;
    layoutButton.title = `Switch to the ${LAYOUTS[OTHER[layout]].label} keys`;
    hint.textContent = LAYOUTS[layout].hint;
    toast.textContent = `${LAYOUTS[layout].hint} H shows the panel.`;
  };
  showLayout();

  layoutButton.addEventListener("click", () => {
    releaseAll();
    layout = OTHER[layout];
    saveLayout(layout);
    showLayout();
    layoutButton.blur();
  });

  const setCaptions = (on) => {
    app.set_captions(on);
    saveCaptions(on);
    captionsButton.textContent = `Captions: ${on ? "on" : "off"}`;
    captionsButton.title = "Show or hide the words above a clucking chicken (V)";
  };
  // Apply the remembered choice without saving it again.
  app.set_captions(loadCaptions());
  captionsButton.textContent = `Captions: ${app.captions() ? "on" : "off"}`;
  captionsButton.title = "Show or hide the words above a clucking chicken (V)";
  captionsButton.addEventListener("click", () => {
    setCaptions(!app.captions());
    captionsButton.blur();
  });

  resetButton.addEventListener("click", () => {
    releaseAll();
    app.reset();
    resetButton.blur();
  });

  if (document.fullscreenEnabled) {
    fullscreenButton.hidden = false;
    fullscreenButton.addEventListener("click", () => {
      toggleFullscreen();
      fullscreenButton.blur();
    });
  }
  document.addEventListener("fullscreenchange", () => setKeyLock(Boolean(document.fullscreenElement)));

  const handle = (event, down) => {
    if (down) sfx.unlock();
    if (event.code === "KeyM" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (down && !event.repeat) {
        sfx.toggleMute();
        showSound();
      }
      return;
    }
    if (event.code === "KeyF" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (down && !event.repeat && document.fullscreenEnabled) toggleFullscreen();
      return;
    }
    // H shows and hides the panel in both layouts; neither layout uses it.
    if (event.code === "KeyH" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (down && !event.repeat) setPanel(panel.hidden);
      return;
    }
    // V toggles the captions in both layouts; neither layout uses it.
    if (event.code === "KeyV" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (down && !event.repeat) setCaptions(!app.captions());
      return;
    }
    const bits = LAYOUTS[layout].keys[event.code];
    if (bits === undefined) return;
    // Stops Alt from focusing the menu bar, Space from scrolling and the arrows from moving the page, wherever the browser allows.
    event.preventDefault();
    if (event.repeat) return;
    if (down) held.set(event.code, bits);
    else held.delete(event.code);
    app.key(bits, down);
  };
  addEventListener("keydown", (event) => handle(event, true));
  addEventListener("keyup", (event) => handle(event, false));
  // A window that loses focus never sees the key releases.
  addEventListener("blur", releaseAll);

  const loop = () => {
    app.frame(performance.now());
    // Read after every frame: the module keeps the clucks of all the ticks the frame ran.
    for (let v = app.take_cluck(); v >= 0; v = app.take_cluck()) sfx.playCluck(v);
    draw(app, wasm.memory);
    showCaptions(app);
    showHud(app);
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

main();
