// Browser shell for the Web presenter: keyboard in, canvas and HUD out.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Every URL is relative so the bundle works under any base path. The wasm module is the only import.
import init, { App } from "./pkg/fizz_present_web.js";

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
    hint: "Arrow keys move, Ctrl jumps, Alt uses the pogo stick, Space fires.",
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
    hint: "Arrow keys or W A S D move, Z jumps, X uses the pogo stick, C or Space fires.",
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

// Every key either layout uses, for the Keyboard Lock request. Escape is left out so the browser's way out of fullscreen still works.
const LOCKED_KEYS = [...new Set(Object.values(LAYOUTS).flatMap((l) => Object.keys(l.keys)))];

const canvas = document.getElementById("screen");
const hud = document.getElementById("hud");
const hint = document.getElementById("hint");
const layoutButton = document.getElementById("layout");
const fullscreenButton = document.getElementById("fullscreen");
const ctx = canvas.getContext("2d");

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

function draw(app) {
  ctx.fillStyle = "#0000aa";
  ctx.fillRect(0, 0, 320, 200);
  ctx.strokeStyle = "#aaaaaa";
  ctx.lineWidth = 1;
  ctx.strokeRect(0.5, 0.5, 319, 199);
  const x = app.marker_px_x();
  const y = app.marker_px_y();
  ctx.fillStyle = "#55ffff";
  ctx.fillRect(x - 1, y - 1, 10, 10);
  ctx.fillStyle = "#ffff55";
  ctx.fillRect(x, y, 8, 8);
}

function showHud(app) {
  hud.textContent = `TICK ${app.tick()}\nHASH ${app.hash_hex()}\nDASH ${app.dashes()}`;
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

async function main() {
  await init();
  const app = new App();

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
  };
  showLayout();

  layoutButton.addEventListener("click", () => {
    releaseAll();
    layout = OTHER[layout];
    saveLayout(layout);
    showLayout();
    layoutButton.blur();
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
    if (event.code === "KeyF" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (down && !event.repeat && document.fullscreenEnabled) toggleFullscreen();
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
    draw(app);
    showHud(app);
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

main();
