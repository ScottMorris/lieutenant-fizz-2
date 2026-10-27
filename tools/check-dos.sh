#!/bin/bash
# Builds the DOS programs and checks them in headless DOSBox: the Milestone 0 replay and the slice replay against their golden
# transcripts, a keyboard smoke of the slice, the panic clean-up and the executable's size.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/check-dos.sh      (after tools/setup-dos.sh)
set -euo pipefail
cd "$(dirname "$0")/.."

# Provisional: the 1.44 MB floppy leaves roughly 1.1 MB for the EXE on a FreeDOS-bootable disk. A tracked metric, not a design limit.
BUDGET=1100000

fail() { echo "check-dos: FAIL: $*" >&2; exit 1; }

tools/build-dos.sh

# 1. The replay: the TICK and FINAL lines equal the golden transcript, and the platform lines are right.
status=0
out="$(tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/REPLAY exited $status"
got="$(printf '%s\n' "$out" | tr -d '\r' | grep -E '^(TICK|FINAL) ' || true)"
want="$(tr -d '\r' < crates/fizz-hello/m0-replay.txt | grep -E '^(TICK|FINAL) ')"
[ "$got" = "$want" ] || fail "/REPLAY transcript differs from crates/fizz-hello/m0-replay.txt"
printf '%s\n' "$out" | grep -qx 'M0 REPLAY 1' || fail "no 'M0 REPLAY 1' line"
printf '%s\n' "$out" | grep -qx 'FPU CW 027F' || fail "FPU control word is not 027F"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "vectors not restored"
bios="$(printf '%s\n' "$out" | sed -n 's/^BIOS TICKS \([0-9]*\)$/\1/p')"
{ [ -n "$bios" ] && [ "$bios" -ge 180 ] && [ "$bios" -le 184 ]; } || fail "BIOS TICKS '$bios' is outside 180-184"

# 1b. The slice replay: /SLICE plays the slice script (1,268 ticks, about 21 seconds) and its TICK, FINAL and EVENT lines equal the slice's
# golden transcript. Ben must reach the exit, and the same platform lines hold. The BIOS tick counter runs at 18.2065 Hz, so 1,268
# ticks of 60.0011 Hz is about 385 of its ticks; the bound is the M0 one (two either side) around that.
status=0
out="$(tools/dosrun.sh dist/dos FIZZ2.EXE /SLICE)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/SLICE exited $status"
got="$(printf '%s\n' "$out" | tr -d '\r' | grep -E '^(TICK|FINAL|EVENT) ' || true)"
want="$(tr -d '\r' < crates/fizz-ep2/slice-replay.txt | grep -E '^(TICK|FINAL|EVENT) ')"
[ "$got" = "$want" ] || fail "/SLICE transcript differs from crates/fizz-ep2/slice-replay.txt"
printf '%s\n' "$out" | grep -qx 'SLICE REPLAY 1' || fail "no 'SLICE REPLAY 1' line"
printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/SLICE did not reach the exit"
printf '%s\n' "$out" | grep -qx 'FPU CW 027F' || fail "/SLICE: FPU control word is not 027F"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/SLICE: vectors not restored"
bios="$(printf '%s\n' "$out" | sed -n 's/^BIOS TICKS \([0-9]*\)$/\1/p')"
{ [ -n "$bios" ] && [ "$bios" -ge 383 ] && [ "$bios" -le 387 ]; } || fail "/SLICE BIOS TICKS '$bios' is outside 383-387"

# 2. Live play of the slice with the default Keen keys: type Ctrl (jump), right, right, down, then Esc once the program is up.
# MOVED means Ben is somewhere other than his spawn (a step changes x, a jump changes y). Movement only; autotype timing is not
# tied to ticks.
status=0
out="$(DOS_AUTOTYPE='-w 2 -p 0.2 lctrl right right down esc' tools/dosrun.sh dist/dos FIZZ2.EXE /LIVE)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/LIVE exited $status"
printf '%s\n' "$out" | grep -qx 'MOVED yes' || fail "/LIVE did not move Ben"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/LIVE did not restore the vectors"

# 2b. The layouts are exclusive. /MODERN moves on Z and the arrows; the default Keen keys ignore W, A, S, D, Z, X and C.
status=0
out="$(DOS_AUTOTYPE='-w 2 -p 0.2 z right right down esc' tools/dosrun.sh dist/dos FIZZ2.EXE /MODERN)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/MODERN exited $status"
printf '%s\n' "$out" | grep -qx 'MOVED yes' || fail "/MODERN did not move Ben"
status=0
out="$(DOS_AUTOTYPE='-w 2 -p 0.2 w a s d z x c esc' tools/dosrun.sh dist/dos FIZZ2.EXE /LIVE)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/LIVE (Keen) exited $status"
printf '%s\n' "$out" | grep -qx 'MOVED no' || fail "the Keen layout moved on modern-only keys"

# 3. Panic: the clean-up list runs and the exit status is 3, not a hang.
status=0
DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /PANIC > /dev/null || status=$?
[ "$status" = 3 ] || fail "/PANIC status $status (expected 3)"
echo "PANIC exit $status"

# 4. Size.
size="$(wc -c < dist/dos/FIZZ2.EXE | tr -d ' ')"
echo "SIZE $size"
echo "BUDGET $BUDGET"
[ "$size" -le "$BUDGET" ] || fail "FIZZ2.EXE is $size bytes, over the budget of $BUDGET"

echo "DOS CHECK ok"
