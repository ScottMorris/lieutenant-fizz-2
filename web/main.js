// Browser shell for the Milestone 0 Web presenter: keyboard in, canvas and HUD out.
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
const ACTION = 16;

const KEYS = {
  ArrowUp: UP,
  KeyW: UP,
  ArrowDown: DOWN,
  KeyS: DOWN,
  ArrowLeft: LEFT,
  KeyA: LEFT,
  ArrowRight: RIGHT,
  KeyD: RIGHT,
  Space: ACTION,
};

const canvas = document.getElementById("screen");
const hud = document.getElementById("hud");
const ctx = canvas.getContext("2d");

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

async function main() {
  await init();
  const app = new App();

  const handle = (event, down) => {
    const bits = KEYS[event.code];
    if (bits === undefined) return;
    event.preventDefault();
    if (event.repeat) return;
    app.key(bits, down);
  };
  addEventListener("keydown", (event) => handle(event, true));
  addEventListener("keyup", (event) => handle(event, false));

  const loop = () => {
    app.frame(performance.now());
    draw(app);
    showHud(app);
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

main();
