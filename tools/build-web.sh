#!/bin/bash
# Builds the Web presenter into dist/web: the wasm module, its JavaScript bindings and the static page.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail
cd "$(dirname "$0")/.."

# The CLI and the `wasm-bindgen` crate must agree exactly or the bindings do not load.
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[^0-9.]/, "", $0); print; exit }' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ -z "$locked" ] || [ "$installed" != "$locked" ]; then
    printf 'wasm-bindgen CLI is %s but Cargo.lock pins %s.\n' "${installed:-missing}" "${locked:-nothing}" >&2
    printf 'Install the matching CLI: cargo install wasm-bindgen-cli --version %s --locked\n' "$locked" >&2
    exit 1
fi

cargo build --release --target wasm32-unknown-unknown -p fizz-present-web
rm -rf dist/web
mkdir -p dist/web
wasm-bindgen --target web --no-typescript --out-dir dist/web/pkg \
    target/wasm32-unknown-unknown/release/fizz_present_web.wasm
cp web/* dist/web/
printf 'Built dist/web\n'
