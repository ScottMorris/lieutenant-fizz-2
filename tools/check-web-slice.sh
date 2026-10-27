#!/bin/bash
# Checks that the Web (wasm) build replays the slice to the golden transcript, line for line.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-web-slice.sh
#
# Runs tools/web-slice-replay.mjs under Node or Bun and compares its output with crates/fizz-ep2/slice-replay.txt: the
# `SLICE REPLAY 1` header and every `TICK` and `FINAL` line must be identical. It stands beside tools/check-parity.sh, which still
# gates Milestone 0, until the parity gate switches to the slice and this script folds into it. Builds dist/web when it is missing.
set -euo pipefail
cd "$(dirname "$0")/.."

golden=crates/fizz-ep2/slice-replay.txt
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fizz2-web-slice.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

fail() { echo "check-web-slice: FAIL: $*" >&2; exit 1; }

[ -d dist/web/pkg ] || tools/build-web.sh
if command -v node > /dev/null 2>&1; then
  js=node
elif command -v bun > /dev/null 2>&1; then
  js=bun
else
  fail "neither node nor bun is installed"
fi

# The transcript lines that carry the header and the state hashes, with any carriage returns removed.
lines() { tr -d '\r' | grep -E '^(SLICE REPLAY|TICK|FINAL) ' || true; }

lines < "$golden" > "$scratch/golden"
[ -s "$scratch/golden" ] || fail "no transcript lines in $golden"
grep -qx 'SLICE REPLAY 1' "$scratch/golden" || fail "$golden has no 'SLICE REPLAY 1' header"

"$js" tools/web-slice-replay.mjs | lines > "$scratch/web"
if ! diff -u --label "golden ($golden)" --label "web" "$scratch/golden" "$scratch/web"; then
  fail "the Web slice transcript differs from $golden"
fi

final="$(sed -n 's/^FINAL TICK [0-9]* HASH \([0-9a-f]\{16\}\)$/\1/p' "$scratch/golden")"
echo "WEB SLICE ok $final"
