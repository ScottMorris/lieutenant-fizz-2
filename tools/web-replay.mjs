// Runs the Milestone 0 replay inside the built wasm module and prints the transcript, for Web and DOS parity checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Works under Node and Bun. Build the bundle first with tools/build-web.sh.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const pkg = new URL("../dist/web/pkg/", import.meta.url);
const { initSync, replay_transcript } = await import(
  fileURLToPath(new URL("fizz_present_web.js", pkg))
);
initSync({ module: readFileSync(fileURLToPath(new URL("fizz_present_web_bg.wasm", pkg))) });
process.stdout.write(replay_transcript());
