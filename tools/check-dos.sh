#!/bin/bash
# Builds the DOS programs and checks them in headless DOSBox: the slice replay against its golden transcript, a keyboard smoke
# of the slice, the bench, the panic clean-up and the executable's size.
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

# 0. The guard: a CPU setting DOSBox rejects must fail the run (status 125) instead of being replaced by its default. The form
# `fixed 26800` is the one that went unnoticed once: DOSBox Staging 0.83 logs it as invalid and runs at 3,000 cycles.
status=0
DOS_CYCLES='fixed 26800' DOS_TIMEOUT=30 tools/dosrun.sh dist/dos RTSMOKE.EXE > /dev/null 2>&1 || status=$?
[ "$status" = 125 ] || fail "a rejected cpu_cycles setting exited $status (expected 125)"
echo "GUARD rejected cpu_cycles exit $status"

# 1. The replay: /REPLAY plays the slice script (1,268 ticks, about 21 seconds). The lines that start with `TICK `, `FINAL ` or
# `EVENT ` equal the golden transcript, whatever it holds, and Ben must reach the exit. The BIOS tick counter runs at 18.2065 Hz,
# so 1,268 ticks of 60.0011 Hz is about 385 of its ticks; the bound is two either side of that.
golden=crates/fizz-ep2/slice-replay.txt
status=0
out="$(tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/REPLAY exited $status"
got="$(printf '%s\n' "$out" | tr -d '\r' | grep -E '^(TICK|FINAL|EVENT) ' || true)"
want="$(tr -d '\r' < "$golden" | grep -E '^(TICK|FINAL|EVENT) ')"
[ "$got" = "$want" ] || fail "/REPLAY transcript differs from $golden"
header="$(head -n 1 "$golden" | tr -d '\r')"
printf '%s\n' "$out" | tr -d '\r' | grep -qxF "$header" || fail "no '$header' line"
printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/REPLAY did not reach the exit"
printf '%s\n' "$out" | grep -qx 'FPU CW 027F' || fail "FPU control word is not 027F"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "vectors not restored"
bios="$(printf '%s\n' "$out" | sed -n 's/^BIOS TICKS \([0-9]*\)$/\1/p')"
{ [ -n "$bios" ] && [ "$bios" -ge 383 ] && [ "$bios" -le 387 ]; } || fail "BIOS TICKS '$bios' is outside 383-387"

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

# 3b. The bench: /BENCH plays the script once without pacing and times the step, the drawing and the copy to the screen with
# the PIT. DOSBox runs a fixed 26,800 cycles per millisecond (about a 486DX2-66) on its normal core; the setting is the plain
# number, because the form `fixed 26800` is refused (see 0). The figures are DOSBox's, not a 486's: the bounds are loose sanity
# checks (a figure of zero means the timer did not run, and a step slower than the 16,667 us tick means the slice cannot hold
# 60 Hz even in the emulator) and the numbers go in the log. /BENCH also times a loop of `dec`/`jnz` (two instructions per loop),
# and the rate that implies must match the configured cycles: DOSBox's normal core charges one cycle per simple instruction.
# Measured at 3,000, 12,000, 26,800 and 80,000 cycles per millisecond the rate came out within 1% each time; the bound here is 10%.
CYCLES=26800
status=0
out="$(DOS_CYCLES="$CYCLES" DOS_TIMEOUT=120 tools/dosrun.sh dist/dos FIZZ2.EXE /BENCH)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/BENCH exited $status"
printf '%s\n' "$out" | grep -qx 'BENCH TICKS 1268' || fail "no 'BENCH TICKS 1268' line"
printf '%s\n' "$out" | grep -qx 'BENCH BUDGET 16667 us per tick at 60 Hz' || fail "no BENCH BUDGET line"
printf '%s\n' "$out" | grep -qx "BENCH HASH $(sed -n 's/^FINAL TICK [0-9]* HASH //p' "$golden" | tr -d '\r')" || fail "/BENCH ended on a different hash from $golden"
printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/BENCH did not reach the exit"
printf '%s\n' "$out" | grep -qx 'FPU CW 027F' || fail "/BENCH: FPU control word is not 027F"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/BENCH did not restore the vectors"
# The DOS build's drawing path (its own memset and memcpy among others) must draw the pixels the portable code draws: the digests of
# the picture below the HUD strip at seven ticks equal the ones the host computes from the same shared routines.
host_frames="$(cargo run -q --release -p fizz-present-dos --example frames)"
dos_frames="$(printf '%s\n' "$out" | tr -d '\r' | grep '^BENCH FRAME ' || true)"
[ -n "$host_frames" ] && [ "$dos_frames" = "$host_frames" ] || fail "/BENCH frame digests differ from the host's: DOS '$dos_frames', host '$host_frames'"
cal="$(printf '%s\n' "$out" | tr -d '\r' | sed -n 's/^BENCH CALIBRATE \([0-9]*\) loops in \([0-9]*\) us (2 instructions per loop)$/\1 \2/p')"
[ -n "$cal" ] || fail "no 'BENCH CALIBRATE <n> loops in <n> us (2 instructions per loop)' line"
set -- $cal
[ "$2" -gt 0 ] || fail "BENCH CALIBRATE took $2 us"
# Instructions per millisecond = 2 * loops * 1000 / us. Within 10% of CYCLES means |rate - CYCLES| * 10 <= CYCLES.
rate=$(( $1 * 2 * 1000 / $2 ))
diff=$(( rate > CYCLES ? rate - CYCLES : CYCLES - rate ))
echo "CALIBRATE implied $rate instructions per ms against $CYCLES configured"
[ $(( diff * 10 )) -le "$CYCLES" ] || fail "BENCH CALIBRATE implies $rate instructions per ms, more than 10% from the configured $CYCLES"
for section in STEP DRAW PRESENT TOTAL; do
  line="$(printf '%s\n' "$out" | grep -E "^BENCH $section avg [0-9]+ max [0-9]+ units us$" || true)"
  [ -n "$line" ] || fail "no 'BENCH $section avg <n> max <n> units us' line"
  avg="$(printf '%s\n' "$line" | sed -n 's/^BENCH [A-Z]* avg \([0-9]*\) .*/\1/p')"
  [ "$avg" -gt 0 ] || fail "BENCH $section average is $avg"
  if [ "$section" = STEP ]; then
    [ "$avg" -lt 16667 ] || fail "BENCH STEP average $avg us is over the 60 Hz tick budget"
  fi
done

# 4. Size.
size="$(wc -c < dist/dos/FIZZ2.EXE | tr -d ' ')"
echo "SIZE $size"
echo "BUDGET $BUDGET"
[ "$size" -le "$BUDGET" ] || fail "FIZZ2.EXE is $size bytes, over the budget of $BUDGET"

echo "DOS CHECK ok"
