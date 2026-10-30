#!/bin/bash
# Checks that the native, Web (wasm) and DOS builds replay the slice to the same transcript as the golden file.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-parity.sh [--quick]
#
#   --quick  reuse the DOS transcript that tools/check-dos.sh left in dist/dos-replay.txt when it is newer than dist/dos/FIZZ2.EXE, instead of
#            running the 28-second DOS /REPLAY again. The comparison is the same; only the run is shared. Without that file it runs as usual.
#
# The golden file, crates/fizz-ep2/slice-replay.txt, drives every comparison: its first line is the header each platform must
# print, and the lines that start with `TICK `, `FINAL ` or `EVENT ` (carriage returns removed) must be identical on native, Web
# and DOS. `EVENT ` lines carry the semantic events the rules emit; a golden without any is compared the same way. The final hash
# printed at the end is read from the golden, never written here. Builds dist/web and dist/dos first when they are missing. The
# DOS leg needs the toolchain from tools/setup-dos.sh; without it the script fails unless PARITY_SKIP_DOS=1 is set, which skips
# DOS and prints `PARITY partial (DOS skipped)`.
set -euo pipefail
quick=0
case "${1:-}" in
  --quick) quick=1 ;;
  "") ;;
  *) echo "check-parity: unknown argument $1" >&2; exit 2 ;;
esac
cd "$(dirname "$0")/.."

golden=crates/fizz-ep2/slice-replay.txt
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fizz2-parity.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

fail() { echo "check-parity: FAIL: $*" >&2; exit 1; }

# The lines that carry state hashes and events, with DOS line ends removed.
hashes() { tr -d '\r' | grep -E '^(TICK|FINAL|EVENT) ' || true; }

hashes < "$golden" > "$scratch/golden"
[ -s "$scratch/golden" ] || fail "no TICK or FINAL lines in $golden"
header="$(head -n 1 "$golden" | tr -d '\r')"
printf '%s\n' "$header" | grep -qE '^[A-Z]+ REPLAY [0-9]+$' || fail "$golden does not start with a REPLAY header (found '$header')"

# compare <platform> <transcript file>
compare() {
  local name="$1" file="$2"
  if ! tr -d '\r' < "$file" | grep -qxF "$header"; then
    fail "$name transcript has no '$header' header"
  fi
  hashes < "$file" > "$scratch/$name.hashes"
  if ! diff -u --label "golden ($golden)" --label "$name" "$scratch/golden" "$scratch/$name.hashes"; then
    fail "$name transcript differs from $golden"
  fi
  echo "check-parity: $name matches"
}

# 1. Native.
cargo run -q -p fizz-ep2 --example replay > "$scratch/native.txt"
compare native "$scratch/native.txt"

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
compare web "$scratch/web.txt"

# 3. DOS: the same replay inside headless DOSBox (FIZZ2.EXE /REPLAY plays the slice script).
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
  if [ "$quick" = 1 ] && [ dist/dos-replay.txt -nt dist/dos/FIZZ2.EXE ]; then
    echo "check-parity: reusing the DOS /REPLAY transcript from tools/check-dos.sh (dist/dos-replay.txt)"
    cp dist/dos-replay.txt "$scratch/dos.txt"
  else
    status=0
    tools/dosrun.sh dist/dos "FIZZ2.EXE /REPLAY" > "$scratch/dos.txt" || status=$?
    [ "$status" = 0 ] || { cat "$scratch/dos.txt" >&2; fail "DOS /REPLAY exited $status"; }
  fi
  compare dos "$scratch/dos.txt"
fi

final="$(sed -n 's/^FINAL TICK [0-9]* HASH \([0-9a-f]\{16\}\)$/\1/p' "$scratch/golden")"
[ -n "$final" ] || fail "no final hash in $golden"
if [ "$platforms" = "native, web" ]; then
  echo "PARITY partial (DOS skipped)"
else
  echo "PARITY ok $final"
fi
echo "platforms compared: $platforms"
