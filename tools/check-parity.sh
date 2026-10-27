#!/bin/bash
# Checks that the native, Web (wasm) and DOS builds replay Milestone 0 to the same state hashes as the golden transcript.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-parity.sh
#
# Compares the `TICK ` and `FINAL ` lines of each platform with crates/fizz-hello/m0-replay.txt (carriage returns removed), and
# requires the `M0 REPLAY 1` header from native and Web. Builds dist/web and dist/dos first when they are missing. The DOS leg needs
# the toolchain from tools/setup-dos.sh; without it the script fails unless PARITY_SKIP_DOS=1 is set, which skips DOS and prints
# `PARITY partial (DOS skipped)`.
set -euo pipefail
cd "$(dirname "$0")/.."

golden=crates/fizz-hello/m0-replay.txt
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fizz2-parity.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

fail() { echo "check-parity: FAIL: $*" >&2; exit 1; }

# The lines that carry state hashes, with DOS line ends removed.
hashes() { tr -d '\r' | grep -E '^(TICK|FINAL) ' || true; }

hashes < "$golden" > "$scratch/golden"
[ -s "$scratch/golden" ] || fail "no TICK or FINAL lines in $golden"

# compare <platform> <transcript file> <require header: yes|no>
compare() {
  local name="$1" file="$2" header="$3"
  if [ "$header" = yes ] && ! tr -d '\r' < "$file" | grep -qx 'M0 REPLAY 1'; then
    fail "$name transcript has no 'M0 REPLAY 1' header"
  fi
  hashes < "$file" > "$scratch/$name.hashes"
  if ! diff -u --label "golden ($golden)" --label "$name" "$scratch/golden" "$scratch/$name.hashes"; then
    fail "$name transcript differs from $golden"
  fi
  echo "check-parity: $name matches"
}

# 1. Native.
cargo run -q -p fizz-hello --example replay > "$scratch/native.txt"
compare native "$scratch/native.txt" yes

# 2. Web: the wasm module run under Node or Bun.
[ -d dist/web/pkg ] || tools/build-web.sh
if command -v node > /dev/null 2>&1; then
  js=node
elif command -v bun > /dev/null 2>&1; then
  js=bun
else
  fail "neither node nor bun is installed, so the Web leg cannot run"
fi
"$js" tools/web-replay.mjs > "$scratch/web.txt"
compare web "$scratch/web.txt" yes

# 3. DOS: the same replay inside headless DOSBox.
platforms="native, web, dos"
if [ ! -d vendor/djgpp ] && [ ! -d "${DOS_TOOLCHAIN:-/nonexistent}/djgpp" ]; then
  if [ "${PARITY_SKIP_DOS:-}" = 1 ]; then
    platforms="native, web"
  else
    fail "no DOS toolchain (run tools/setup-dos.sh with DOS_TOOLCHAIN set), or set PARITY_SKIP_DOS=1 to skip the DOS leg"
  fi
else
  [ -d vendor/djgpp ] || tools/setup-dos.sh
  [ -f dist/dos/FIZZ2.EXE ] || tools/build-dos.sh
  status=0
  tools/dosrun.sh dist/dos "FIZZ2.EXE /REPLAY" > "$scratch/dos.txt" || status=$?
  [ "$status" = 0 ] || { cat "$scratch/dos.txt" >&2; fail "DOS /REPLAY exited $status"; }
  compare dos "$scratch/dos.txt" no
fi

final="$(sed -n 's/^FINAL TICK [0-9]* HASH \([0-9a-f]\{16\}\)$/\1/p' "$scratch/golden")"
[ -n "$final" ] || fail "no final hash in $golden"
if [ "$platforms" = "native, web" ]; then
  echo "PARITY partial (DOS skipped)"
else
  echo "PARITY ok $final"
fi
echo "platforms compared: $platforms"
