#!/bin/bash
# Builds the DOS programs and checks them in headless DOSBox: the slice replay against its golden transcript, a keyboard smoke
# of the slice, the bench, the panic clean-up, the machine probes under a ladder of DOSBox configurations and the executable's size.
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

# 2c. /NOCAPTIONS is a switch, not a mode: it combines with /MODERN, and the program still runs live and ends cleanly.
status=0
out="$(DOS_AUTOTYPE='-w 2 -p 0.2 z right right down esc' tools/dosrun.sh dist/dos FIZZ2.EXE /NOCAPTIONS /MODERN)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/NOCAPTIONS /MODERN exited $status"
printf '%s\n' "$out" | grep -qx 'MOVED yes' || fail "/NOCAPTIONS did not leave /MODERN live"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/NOCAPTIONS did not restore the vectors"

# 3. Panic: the clean-up list runs and the exit status is 3, not a hang.
status=0
DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /PANIC > /dev/null || status=$?
[ "$status" = 3 ] || fail "/PANIC status $status (expected 3)"
echo "PANIC exit $status"
# 3a. The same with an OPL3 in its own mode, a cluck sounding and the music playing when the panic comes (the audio service is up): the
# clean-up list silences the chip (every channel of both banks, so the music's too) and puts the timer back, and the exit is still a
# status 3, not a hang. The music must have written something before the panic, or the case proves nothing about it.
status=0
out="$(DOS_CONF=$'[sblaster]\nsbtype = sb16' DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /PANIC)" || status=$?
[ "$status" = 3 ] || fail "/PANIC with the audio service on an OPL3: status $status (expected 3)"
panic_writes="$(printf '%s\n' "$out" | tr -d '\r' | sed -n 's/^MUSIC PANIC WRITES \([0-9]*\)$/\1/p')"
{ [ -n "$panic_writes" ] && [ "$panic_writes" -gt 0 ]; } || fail "/PANIC on an OPL3: the music had written '$panic_writes' times before the panic"
echo "PANIC (OPL3, music running: $panic_writes music writes) exit $status"

# 3a. The help: /?, /HELP and -? print the screen the switch table writes (crates/fizz-machine/tests/help.txt, which a host test keeps
# equal to the table) and exit 0. It wins over every other switch, so `/? /LIVE` prints the help and does not play (the run ends at once and
# prints none of /LIVE's lines), and it hooks nothing, so none of the platform lines appear. An unknown switch prints one line and exits 2.
help_golden="$(tr -d '\r' < crates/fizz-machine/tests/help.txt)"
help_case() {
  local name="$1" out status=0
  shift
  out="$(DOS_TIMEOUT=10 tools/dosrun.sh dist/dos FIZZ2.EXE "$@")" || status=$?
  [ "$status" = 0 ] || fail "help ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  [ "$out" = "$help_golden" ] || fail "help ($name) differs from crates/fizz-machine/tests/help.txt"
  printf '%s\n' "$out" | grep -qxF '  /REPLAY       Run the built-in script and print its transcript' || fail "help ($name) lists no /REPLAY line"
  if printf '%s\n' "$out" | grep -qE '^(LIVE TICKS|MOVED|EXIT|SOUND|FPU CW|VECTORS RESTORED) '; then fail "help ($name) ran something"; fi
  echo "HELP case $name ok"
}
help_case '/?' '/?'
help_case '/HELP' /HELP
help_case '-?' '-?'
help_case '/help lower case' /help
help_case '/? /LIVE' '/?' /LIVE
help_case '/MODERN /HELP /PANIC' /MODERN /HELP /PANIC
help_case '/? with an unknown switch' '/?' /BOGUS
status=0
out="$(DOS_TIMEOUT=10 tools/dosrun.sh dist/dos FIZZ2.EXE /BOGUS)" || status=$?
[ "$status" = 2 ] || fail "an unknown switch exited $status (expected 2)"
[ "$(printf '%s\n' "$out" | tr -d '\r')" = 'FIZZ2.EXE: unknown switch /BOGUS, try FIZZ2.EXE /?' ] || fail "the unknown-switch line is '$out'"
status=0
out="$(DOS_TIMEOUT=10 tools/dosrun.sh dist/dos FIZZ2.EXE /LIVE /MUSIC:FOO)" || status=$?
[ "$status" = 2 ] || fail "a bad sound value exited $status (expected 2)"
printf '%s\n' "$out" | grep -qF 'unknown switch /MUSIC:FOO' || fail "a bad sound value was not named"
echo "HELP unknown switch exit 2"

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

# 3b2. /BENCH /AUDIO: the same bench with the 560 Hz audio service, once without music and once with it (an OPL3 here). The picture and the
# rules are untouched (the same final hash twice), the music writes only in the second pass and never more than 16 on a tick, and the
# interrupt's figures are present. The microseconds are DOSBox's, which charges no time for ISA port I/O: the write and status-read counts
# are the exact figures, and the time on a real 486 is not measured here.
status=0
out="$(DOS_CYCLES="$CYCLES" DOS_TIMEOUT=200 DOS_CONF=$'[sblaster]\nsbtype = sb16' tools/dosrun.sh dist/dos FIZZ2.EXE /BENCH /AUDIO)" || status=$?
echo "$out"
[ "$status" = 0 ] || fail "/BENCH /AUDIO exited $status"
out="$(printf '%s\n' "$out" | tr -d '\r')"
golden_hash="$(sed -n 's/^FINAL TICK [0-9]* HASH //p' "$golden" | tr -d '\r')"
printf '%s\n' "$out" | grep -qx "BENCH AUDIO HASH $golden_hash $golden_hash" || fail "/BENCH /AUDIO ended on a different hash from $golden"
printf '%s\n' "$out" | grep -qx 'BENCH AUDIO TICKS 1268 1268' || fail "/BENCH /AUDIO did not run the whole script twice"
printf '%s\n' "$out" | grep -qx 'BENCH AUDIO TIMER PIT divisor 2131 (559.9 Hz), ARR opl3' || fail "/BENCH /AUDIO is not at 560 Hz with the OPL3 arrangement"
printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/BENCH /AUDIO did not reach the exit"
printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/BENCH /AUDIO did not restore the vectors"
for pass in off on; do
  for section in STEP DRAW PRESENT TOTAL; do
    avg="$(printf '%s\n' "$out" | sed -n "s/^BENCH AUDIO $pass $section avg \([0-9]*\) max [0-9]* units us$/\1/p")"
    { [ -n "$avg" ] && [ "$avg" -gt 0 ]; } || fail "/BENCH /AUDIO $pass $section average is '$avg'"
  done
  irq="$(printf '%s\n' "$out" | grep -E "^BENCH AUDIO $pass IRQ count [0-9]+ avg [0-9]+\.[0-9] max [0-9]+ discarded [0-9]+ units us$" || true)"
  [ -n "$irq" ] || fail "/BENCH /AUDIO printed no '$pass IRQ' line"
  line="$(printf '%s\n' "$out" | grep -E "^BENCH AUDIO $pass MAX_WRITES_PER_TICK [0-9]+ OPL_WRITES [0-9]+ STATUS_READS [0-9]+$" || true)"
  [ -n "$line" ] || fail "/BENCH /AUDIO printed no '$pass MAX_WRITES_PER_TICK' line"
  set -- $line
  [ "${9}" -eq $(( ${7} * 41 )) ] || fail "/BENCH /AUDIO $pass: STATUS_READS ${9} is not 41 times OPL_WRITES ${7}"
  echo "BENCH AUDIO $pass: $irq; $line"
done
mline="$(printf '%s\n' "$out" | grep -E '^BENCH AUDIO on MUSIC TICKS [0-9]+ WRITES [0-9]+ MAX_PER_TICK [0-9]+ ' || true)"
[ -n "$mline" ] || fail "/BENCH /AUDIO printed no 'on MUSIC' line"
set -- $mline
{ [ "$8" -gt 0 ] && [ "${10}" -le 16 ]; } || fail "/BENCH /AUDIO music wrote $8 times, at most ${10} on a tick: $mline"

# 3c. The probes: /PROBE under one DOSBox configuration after another. tools/dosrun.sh sets `sbtype = none`, so a variant that wants
# a Sound Blaster appends an `[sblaster]` section (the later value wins); `nosound = true` still emulates the OPL. Each run must exit 0
# with the platform lines, print the PROBE lines the case expects (exact lines; `PROBE free_kb a-b` is a range), and print the CELLS
# hash the host computes from those same lines with the default settings. /PROBE needs no key; the interactive /CHECK is never run here.
cargo build -q -p fizz-machine --example cells-hash
probe_case() {
  local name="$1" machine="$2" conf="$3"
  shift 3
  local out status=0
  out="$(DOS_MACHINE="$machine" DOS_CONF="$conf" DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /PROBE)" || status=$?
  [ "$status" = 0 ] || fail "/PROBE ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  local line
  for line in 'EXIT yes' 'FPU CW 027F' 'VECTORS RESTORED yes'; do
    printf '%s\n' "$out" | grep -qxF "$line" || fail "/PROBE ($name) printed no '$line' line"
  done
  for line in "$@"; do
    case "$line" in
      'PROBE free_kb '*-*)
        local range="${line#PROBE free_kb }" kb
        kb="$(printf '%s\n' "$out" | sed -n 's/^PROBE free_kb \([0-9]*\)$/\1/p')"
        { [ -n "$kb" ] && [ "$kb" -ge "${range%-*}" ] && [ "$kb" -le "${range#*-}" ]; } || fail "/PROBE ($name) free_kb '$kb' is outside ${range}"
        ;;
      *) printf '%s\n' "$out" | grep -qxF "$line" || fail "/PROBE ($name) printed no '$line' line" ;;
    esac
  done
  local want got
  want="$(printf '%s\n' "$out" | cargo run -q -p fizz-machine --example cells-hash)"
  got="$(printf '%s\n' "$out" | grep '^CELLS ' || true)"
  [ -n "$got" ] && [ "$got" = "$want" ] || fail "/PROBE ($name) CELLS '$got' differs from the host's '$want'"
  echo "PROBE case $name ok ($got; $(printf '%s\n' "$out" | grep -E '^PROBE (fm|dsp|video|cpu|free_kb) ' | tr '\n' ' '))"
}
sb() { printf '[sblaster]\nsbtype = %s\n%s\n' "$1" "${2:-}"; }
probe_case 'no sound card' svga_s3 '' 'PROBE video vga' 'PROBE mouse yes' 'PROBE joystick no' 'PROBE fm none' 'PROBE dsp none' \
  'PROBE blaster none' 'PROBE x87 yes' 'PROBE cpu 486'
probe_case 'sb1' svga_s3 "$(sb sb1)" 'PROBE fm opl2' 'PROBE dsp 1.05 220' 'PROBE blaster A220 I7 D1 T1'
probe_case 'sb2' svga_s3 "$(sb sb2)" 'PROBE fm opl2' 'PROBE dsp 2.01 220' 'PROBE blaster A220 I7 D1 T3'
probe_case 'sbpro1' svga_s3 "$(sb sbpro1)" 'PROBE fm dualopl2' 'PROBE dsp 3.00 220' 'PROBE blaster A220 I7 D1 T2'
probe_case 'sbpro2' svga_s3 "$(sb sbpro2)" 'PROBE fm opl3' 'PROBE dsp 3.02 220' 'PROBE blaster A220 I7 D1 T4'
probe_case 'sb16' svga_s3 "$(sb sb16)" 'PROBE fm opl3' 'PROBE dsp 4.05 220' 'PROBE blaster A220 I7 D1 H5 T6'
probe_case 'sb16 at 240' svga_s3 "$(sb sb16 'sbbase = 240')" 'PROBE fm opl3' 'PROBE dsp 4.05 240' 'PROBE blaster A240 I7 D1 H5 T6'
probe_case 'sb16, oplmode opl2' svga_s3 "$(sb sb16 'oplmode = opl2')" 'PROBE fm opl2' 'PROBE dsp 4.05 220'
probe_case 'sb16, oplmode none' svga_s3 "$(sb sb16 'oplmode = none')" 'PROBE fm none' 'PROBE dsp 4.05 220'
probe_case 'no card, oplmode opl2' svga_s3 "$(sb none 'oplmode = opl2')" 'PROBE fm opl2' 'PROBE dsp none' 'PROBE blaster none'
probe_case 'mouse driver off' svga_s3 $'[mouse]\nbuiltin_dos_mouse_driver = off' 'PROBE mouse no'
probe_case 'joystick 2axis' svga_s3 $'[joystick]\njoysticktype = 2axis' 'PROBE joystick yes'
probe_case 'machine ega' ega '' 'PROBE video ega'
probe_case 'machine cga' cga '' 'PROBE video cga'
probe_case 'machine hercules' hercules '' 'PROBE video mdahercules'
probe_case 'cputype 386' svga_s3 $'[cpu]\ncputype = 386' 'PROBE cpu 386'
probe_case 'cputype pentium' svga_s3 $'[cpu]\ncputype = pentium' 'PROBE cpu 586'
# The memory ladder: free memory falls with the machine's memory. The ranges are loose around 14,544, 2,256, 208 and 168 KB.
probe_case 'memsize 16' svga_s3 $'[dosbox]\nmemsize = 16' 'PROBE free_kb 12000-16384'
probe_case 'memsize 4' svga_s3 $'[dosbox]\nmemsize = 4' 'PROBE free_kb 1500-4096'
probe_case 'memsize 2' svga_s3 $'[dosbox]\nmemsize = 2' 'PROBE free_kb 100-1024'
probe_case 'memsize 1' svga_s3 $'[dosbox]\nmemsize = 1' 'PROBE free_kb 40-512'
# 3d. The check screen: /CHECKCELLS draws it, reads the 4,000 bytes back from video memory (0xB8000, or 0xB0000 on a monochrome adapter)
# and prints them as CELLROW lines. They must equal what the pure builder makes for the PROBE lines printed in the same run. This is the
# only way the check screen is exercised here: the interactive /CHECK waits for a key and is never run by a script.
cargo build -q -p fizz-machine --example cells-hex
cells_case() {
  local name="$1" machine="$2" conf="$3"
  local out status=0
  out="$(DOS_MACHINE="$machine" DOS_CONF="$conf" DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /CHECKCELLS)" || status=$?
  [ "$status" = 0 ] || fail "/CHECKCELLS ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  printf '%s\n' "$out" | grep -qxF 'EXIT yes' || fail "/CHECKCELLS ($name) printed no 'EXIT yes' line"
  local want got
  want="$(printf '%s\n' "$out" | cargo run -q -p fizz-machine --example cells-hex)"
  got="$(printf '%s\n' "$out" | grep '^CELLROW ' || true)"
  [ -n "$got" ] && [ "$got" = "$want" ] || fail "/CHECKCELLS ($name): the cells read back from video memory differ from the host's"
  local written read
  written="$(printf '%s\n' "$out" | sed -n 's/^CELLS WRITTEN //p')"
  read="$(printf '%s\n' "$out" | sed -n 's/^CELLS \([0-9a-f]\{16\}\)$/\1/p')"
  [ -n "$read" ] && [ "$read" = "$written" ] || fail "/CHECKCELLS ($name): hash read back '$read' differs from the one written '$written'"
  echo "CHECKCELLS case $name ok (CELLS $read)"
}
cells_case 'no sound card' svga_s3 ''
cells_case 'sb16' svga_s3 "$(sb sb16)"
cells_case 'sbpro1' svga_s3 "$(sb sbpro1)"
cells_case 'machine ega' ega ''
cells_case 'machine cga' cga ''
cells_case 'machine hercules' hercules ''
# A test (crates/fizz-machine/tests/no-check-in-scripts.rs) fails if any script or workflow runs the interactive /CHECK.

# The slice's memory floor: the replay must still play, with the same transcript, on DOSBox's smallest memory size (1 MB).
status=0
out="$(DOS_CONF=$'[dosbox]\nmemsize = 1' tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY)" || status=$?
[ "$status" = 0 ] || fail "/REPLAY at memsize 1 exited $status"
got="$(printf '%s\n' "$out" | tr -d '\r' | grep -E '^(TICK|FINAL|EVENT) ' || true)"
[ "$got" = "$want" ] || fail "/REPLAY at memsize 1 differs from $golden"
printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/REPLAY at memsize 1 did not reach the exit"
echo "REPLAY at memsize 1 ok"

# 3d0. /MUSICTEST: two loops of the music through the real chip writes with no timer. The MUSIC line must equal the host's `writes_for` and
# `count_loops` for the arrangement the plan names, byte for byte: exact write count and FNV-1a, so the DOS stepper, the port path and the
# data in the executable agree with the generated tables. The OPL3 enable must precede the first write (ORDER ok). An OPL2 stream on an
# SB16 (/ADLIB) is the router's case: the numbers are the authored stream's, the output bits are the router's (host-tested).
cargo build -q -p fizz-music --example mline
musictest_case() {
  local name="$1" conf="$2" arr="$3" sound="$4" out status=0 want got
  shift 4
  out="$(DOS_CONF="$conf" DOS_TIMEOUT=60 tools/dosrun.sh dist/dos FIZZ2.EXE /MUSICTEST "$@")" || status=$?
  [ "$status" = 0 ] || fail "/MUSICTEST ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  printf '%s\n' "$out" | grep -qxF "$sound" || fail "/MUSICTEST ($name) printed no '$sound' line"
  want="$(cargo run -q -p fizz-music --example mline -- "$arr")"
  got="$(printf '%s\n' "$out" | grep '^MUSIC SONG ' || true)"
  [ "$got" = "$want" ] || fail "/MUSICTEST ($name): '$got' differs from the host's '$want'"
  printf '%s\n' "$out" | grep -qx 'FPU CW 027F' || fail "/MUSICTEST ($name): FPU control word is not 027F"
  if [ "$arr" = off ]; then
    printf '%s\n' "$out" | grep -qx 'MUSIC DETAIL ORDER none MAX_PER_TICK 0' || fail "/MUSICTEST ($name): music was off but wrote"
  else
    printf '%s\n' "$out" | grep -qx 'MUSIC DETAIL ORDER ok MAX_PER_TICK 16' || fail "/MUSICTEST ($name): the detail line is not 'ORDER ok MAX_PER_TICK 16'"
  fi
  echo "MUSICTEST case $name ok ($got)"
}
musictest_case 'AdLib machine (OPL2 arrangement)' "$(sb none 'oplmode = opl2')" opl2 'SOUND MUSIC opl2 FX ch5'
musictest_case 'SB16 (OPL3 arrangement)' "$(sb sb16)" opl3 'SOUND MUSIC opl3 FX ch14'
musictest_case 'SB16 with /ADLIB (OPL2 arrangement on an OPL3 chip)' "$(sb sb16)" opl2 'SOUND MUSIC opl2 FX ch5' /ADLIB
musictest_case 'SB16 with /MUSIC:ADLIB /FX:SB (OPL2 music beside the pair)' "$(sb sb16)" opl2 'SOUND MUSIC opl2 FX ch14' /MUSIC:ADLIB /FX:SB
musictest_case 'SB16 with /MUSIC:OFF' "$(sb sb16)" off 'SOUND MUSIC off FX ch14' /MUSIC:OFF
musictest_case 'SB16 with /NOSOUND' "$(sb sb16)" off 'SOUND MUSIC off FX off' /NOSOUND
musictest_case 'no sound card' '' off 'SOUND MUSIC off FX speaker'

# 3d. The sound settings: FIZZ2.CFG and the flags. Each case runs in a scratch copy of the program directory, so the file a case writes or
# plants never reaches dist/dos. /PROBE prints the `SOUND` line the same way /REPLAY and /LIVE do and needs no key. Flags win over the
# file, the file over Auto, per field; flags never write the file; and a missing file is Auto with no message.
cfgdir="$(mktemp -d "${TMPDIR:-/tmp}/fizz2-cfg.XXXXXX")"
trap 'rm -rf "$cfgdir"' EXIT
cp dist/dos/*.EXE "$cfgdir"/
cfg_none() { rm -f "$cfgdir/FIZZ2.CFG"; }
cfg_put() { printf "$1" > "$cfgdir/FIZZ2.CFG"; }
cfg_hex() { od -An -tx1 -v "$cfgdir/FIZZ2.CFG" | tr -d ' \n'; }
sound_case() {
  local name="$1" conf="$2" expect="$3" mode="$4" out status=0 line
  shift 4
  out="$(DOS_CONF="$conf" DOS_TIMEOUT=30 tools/dosrun.sh "$cfgdir" FIZZ2.EXE "$mode" "$@")" || status=$?
  [ "$status" = 0 ] || fail "sound case '$name' exited $status"
  line="$(printf '%s\n' "$out" | tr -d '\r' | grep '^SOUND ' || true)"
  [ "$line" = "$expect" ] || fail "sound case '$name': got '$line', expected '$expect'"
  echo "SOUND case $name ok ($line)"
}
SB16="$(sb sb16)"
# V2 is a version 2 file (music, effects, frame rate): music AdLib, effects Speaker, 60 Hz.
V2='FZ2C\x02\x02\x04\x02\x00'
cfg_none
sound_case 'no file, no card' '' 'SOUND MUSIC off FX speaker' /PROBE
sound_case 'no file, sb16' "$SB16" 'SOUND MUSIC opl3 FX ch14' /PROBE
sound_case '/NOSOUND' "$SB16" 'SOUND MUSIC off FX off' /PROBE /NOSOUND
sound_case '/SPEAKER' "$SB16" 'SOUND MUSIC off FX speaker' /PROBE /SPEAKER
sound_case '/ADLIB' "$SB16" 'SOUND MUSIC opl2 FX ch5' /PROBE /ADLIB
sound_case '/SB' "$SB16" 'SOUND MUSIC opl3 FX ch14' /PROBE /SB
sound_case '/MUSIC:OFF /FX:ADLIB' "$SB16" 'SOUND MUSIC off FX ch5' /PROBE /MUSIC:OFF /FX:ADLIB
sound_case 'lower case, later flag wins' "$SB16" 'SOUND MUSIC opl2 FX speaker' /PROBE /nosound /music:adlib /fx:speaker
sound_case '/SB without a chip falls back' '' 'SOUND MUSIC off FX speaker music-sb-to-off fx-sb-to-speaker' /PROBE /SB
sound_case '/SB on an OPL2 falls back' "$(sb sb1)" 'SOUND MUSIC opl2 FX ch5 music-sb-to-adlib fx-sb-to-adlib' /PROBE /SB
sound_case '/ADLIB without a chip falls back' '' 'SOUND MUSIC off FX speaker music-adlib-to-off fx-adlib-to-speaker' /PROBE /ADLIB
# The file alone, a flag over the file, and the file's older and damaged forms.
cfg_put "$V2"
sound_case 'file v2' "$SB16" 'SOUND MUSIC opl2 FX speaker' /PROBE
sound_case 'flag over file, per field' "$SB16" 'SOUND MUSIC opl2 FX off' /PROBE /FX:OFF
sound_case 'shortcut over file' "$SB16" 'SOUND MUSIC off FX off' /PROBE /NOSOUND
cfg_put 'FZ2C\x01\x01\x04\x00'
sound_case 'file v1 still read' "$SB16" 'SOUND MUSIC off FX speaker' /PROBE
cfg_put 'FZ2C\x02\x03\x03\x01\x00'
sound_case 'file sb on sb16' "$SB16" 'SOUND MUSIC opl3 FX ch14' /PROBE
sound_case 'file sb on no card' '' 'SOUND MUSIC off FX speaker music-sb-to-off fx-sb-to-speaker' /PROBE
cfg_put 'junk'
sound_case 'corrupt: length' "$SB16" 'SOUND MUSIC opl3 FX ch14 cfg-length' /PROBE
cfg_put 'XZ2C\x02\x02\x02\x00\x00'
sound_case 'corrupt: magic' "$SB16" 'SOUND MUSIC opl3 FX ch14 cfg-magic' /PROBE
cfg_put 'FZ2C\x07\x02\x02\x00\x00'
sound_case 'corrupt: version' "$SB16" 'SOUND MUSIC opl3 FX ch14 cfg-version' /PROBE
cfg_put 'FZ2C\x02\x09\x04\x00\x00'
sound_case 'corrupt: music byte only' "$SB16" 'SOUND MUSIC opl3 FX speaker cfg-music' /PROBE
cfg_put 'FZ2C\x02\x02\x09\x00\x00'
sound_case 'corrupt: effects byte only' "$SB16" 'SOUND MUSIC opl2 FX ch14 cfg-effects' /PROBE
cfg_put 'FZ2C\x02\x02\x02\x05\x00'
sound_case 'corrupt: frame-rate byte only' "$SB16" 'SOUND MUSIC opl2 FX ch5 cfg-framerate' /PROBE
cfg_put ''
sound_case 'empty file' "$SB16" 'SOUND MUSIC opl3 FX ch14' /PROBE
# /LIVE prints the line too (Esc ends it).
cfg_put "$V2"
status=0
out="$(DOS_AUTOTYPE='-w 2 -p 0.2 esc' DOS_CONF="$SB16" tools/dosrun.sh "$cfgdir" FIZZ2.EXE /LIVE /FX:OFF)" || status=$?
[ "$status" = 0 ] || fail "/LIVE with sound flags exited $status"
printf '%s\n' "$out" | grep -qx 'SOUND MUSIC opl2 FX off' || fail "/LIVE did not print 'SOUND MUSIC opl2 FX off'"
# Flags never write the file: the planted file is byte for byte what it was, and with none planted none appears.
[ "$(cfg_hex)" = '465a32430202040200' ] || fail "a flag run changed FIZZ2.CFG to $(cfg_hex)"
cfg_none
sound_case 'flags leave no file' "$SB16" 'SOUND MUSIC off FX off' /PROBE /NOSOUND
[ ! -e "$cfgdir/FIZZ2.CFG" ] || fail "a run with flags created FIZZ2.CFG"
# The save path: /CFGTEST (a test mode, not for players) saves the settings the flags give with the 60 Hz frame rate, reads the file back
# and prints it. The bytes are the documented version 2 layout, and a version 1 file is replaced by a version 2 one.
cfgtest_case() {
  local name="$1" hex="$2" out status=0
  shift 2
  out="$(DOS_TIMEOUT=30 tools/dosrun.sh "$cfgdir" FIZZ2.EXE /CFGTEST "$@")" || status=$?
  [ "$status" = 0 ] || fail "/CFGTEST ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  printf '%s\n' "$out" | grep -qx 'CFG SAVED yes' || fail "/CFGTEST ($name) did not save"
  printf '%s\n' "$out" | grep -qx 'CFG ROUNDTRIP yes' || fail "/CFGTEST ($name) did not read back what it saved"
  printf '%s\n' "$out" | grep -qx "CFG BYTES $hex" || fail "/CFGTEST ($name) bytes differ from $hex"
  printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/CFGTEST ($name) did not restore the vectors"
  [ "$(cfg_hex)" = "$(printf '%s' "$hex" | tr 'A-F' 'a-f')" ] || fail "/CFGTEST ($name): the file holds $(cfg_hex), not $hex"
  echo "CFGTEST case $name ok ($hex)"
}
cfg_none
cfgtest_case 'sb and speaker' 465A32430203040200 /MUSIC:SB /FX:SPEAKER
cfgtest_case 'nosound' 465A32430201010200 /NOSOUND
cfg_put 'FZ2C\x01\x03\x03\x00'
cfgtest_case 'replaces a version 1 file' 465A32430202020200 /ADLIB
cfgtest_case 'no flags is Auto' 465A32430200000200
# And what it saved is what the next run reads.
sound_case 'saved file is read back' "$SB16" 'SOUND MUSIC opl3 FX ch14' /PROBE

# 3e. Sound choices never change the game: the replay's TICK, FINAL and EVENT lines equal the golden transcript under every combination
# of file and flags, and each run prints the SOUND line its settings give.
replay_case() {
  local name="$1" conf="$2" expect="$3" out status=0 got_lines
  shift 3
  out="$(DOS_CONF="$conf" tools/dosrun.sh "$cfgdir" FIZZ2.EXE /REPLAY "$@")" || status=$?
  [ "$status" = 0 ] || fail "/REPLAY ($name) exited $status"
  out="$(printf '%s\n' "$out" | tr -d '\r')"
  got_lines="$(printf '%s\n' "$out" | grep -E '^(TICK|FINAL|EVENT) ' || true)"
  [ "$got_lines" = "$want" ] || fail "/REPLAY ($name) transcript differs from $golden"
  printf '%s\n' "$out" | grep -qxF "$expect" || fail "/REPLAY ($name) printed no '$expect' line"
  printf '%s\n' "$out" | grep -qx 'EXIT yes' || fail "/REPLAY ($name) did not reach the exit"
  printf '%s\n' "$out" | grep -qx 'VECTORS RESTORED yes' || fail "/REPLAY ($name) did not restore the vectors"
  # The AUDIO line proves which path ran, since the sound itself cannot be heard here: OPL register writes with a chip (opl), speaker steps
  # on the speaker (speaker), neither when silent (silent). The clucks the script causes are the same in every run, so the counts are not
  # compared with each other, only with zero.
  local audio writes steps
  audio="$(printf '%s\n' "$out" | grep '^AUDIO ' || true)"
  writes="$(printf '%s\n' "$audio" | sed -n 's/^AUDIO WRITES \([0-9]*\) SPEAKER STEPS [0-9]*$/\1/p')"
  steps="$(printf '%s\n' "$audio" | sed -n 's/^AUDIO WRITES [0-9]* SPEAKER STEPS \([0-9]*\)$/\1/p')"
  [ -n "$writes" ] && [ -n "$steps" ] || fail "/REPLAY ($name) printed no 'AUDIO WRITES <n> SPEAKER STEPS <m>' line"
  case "${AUDIO:-}" in
    opl) { [ "$writes" -gt 0 ] && [ "$steps" -eq 0 ]; } || fail "/REPLAY ($name): '$audio' (expected OPL writes and no speaker steps)" ;;
    # The three OPL paths, by how many writes the script's clucks cause. The attenuation of a distant source changes a value in a write the
    # effect makes anyway, never the count, so a path's count is a function of the path alone.
    #   opl2: channel 5 on a bare OPL2: 20 writes to key the chip off at start and stop, the rest the clucks' records.
    #   opl_or: channel 5 on an OPL3 in its own mode: the same plus 21 (OPL3 mode on, 0x104, and the second bank keyed off, and back).
    #   opl_pair: channels 5 and 14: the OPL3's 41 start and stop writes and the clucks' records twice, less the chip-wide waveform-select
    #     enable the right channel leaves to the left (one per cluck; the script has three). Also checked against the golden transcript.
    opl2) { [ "$writes" -gt 20 ] && [ "$steps" -eq 0 ]; } || fail "/REPLAY ($name): '$audio' (expected OPL writes and no speaker steps)"; CH5_WRITES="$writes" ;;
    opl_or) [ "$writes" -eq $((CH5_WRITES + 21)) ] || fail "/REPLAY ($name): '$audio' (expected $((CH5_WRITES + 21)) writes: channel 5 plus the OPL3's 21)" ;;
    opl_pair)
      clucks="$(grep -c '^EVENT [0-9]* cluck ' "$golden")"
      expected=$((41 + 2 * (CH5_WRITES - 20) - clucks))
      { [ "$writes" -eq "$expected" ] && [ "$steps" -eq 0 ]; } || fail "/REPLAY ($name): '$audio' (expected $expected writes: the OPL3 pair, about twice channel 5's $((CH5_WRITES - 20)))" ;;
    speaker) { [ "$writes" -eq 0 ] && [ "$steps" -gt 0 ]; } || fail "/REPLAY ($name): '$audio' (expected speaker steps and no OPL writes)" ;;
    # Effects on the speaker while music holds the chip: the chip is only keyed off at start and stop (no cluck reaches it).
    speaker_chip) { { [ "$writes" -gt 0 ] && [ "$writes" -le 40 ] && [ "$steps" -gt 0 ]; } || fail "/REPLAY ($name): '$audio' (expected speaker steps and only the chip's start and stop writes)"; } ;;
    silent) { [ "$writes" -eq 0 ] && [ "$steps" -eq 0 ]; } || fail "/REPLAY ($name): '$audio' (expected no writes and no steps)" ;;
  esac
  # The MUSIC line: the arrangement the SOUND line names (off when it says off), and for a chip, the exact writes and hash the host computes
  # for the player's own tick count. The music never changes the AUDIO counts above: those count effects and the start and stop key-offs only.
  local want_arr music parsed m_arr m_ticks m_writes m_loops m_hash host
  want_arr="$(printf '%s\n' "$expect" | sed -n 's/^SOUND MUSIC \([a-z0-9]*\) .*/\1/p')"
  music="$(printf '%s\n' "$out" | grep '^MUSIC SONG ' || true)"
  parsed="$(printf '%s\n' "$music" | sed -n 's/^MUSIC SONG crater ARR \([a-z0-9]*\) TICKS \([0-9]*\) WRITES \([0-9]*\) LOOPS \([0-9]*\) HASH \([0-9a-f]\{8\}\)$/\1 \2 \3 \4 \5/p')"
  [ -n "$parsed" ] || fail "/REPLAY ($name) printed no well-formed 'MUSIC SONG crater ARR ...' line: '$music'"
  set -- $parsed
  m_arr="$1"; m_ticks="$2"; m_writes="$3"; m_loops="$4"; m_hash="$5"
  [ "$m_arr" = "$want_arr" ] || fail "/REPLAY ($name): MUSIC ARR '$m_arr' but the SOUND line says '$want_arr'"
  if [ "$m_arr" = off ]; then
    { [ "$m_ticks" -eq 0 ] && [ "$m_writes" -eq 0 ] && [ "$m_loops" -eq 0 ]; } || fail "/REPLAY ($name): music is off and the line says '$music'"
  else
    # About 21 s at 559.9 Hz is about 11,800 ticks of the player's own clock; loops stay 0 (the loop is 63 s).
    { [ "$m_writes" -gt 0 ] && [ "$m_ticks" -ge 9000 ] && [ "$m_ticks" -le 14000 ] && [ "$m_loops" -eq 0 ]; } || fail "/REPLAY ($name): implausible '$music'"
    host="$(cargo run -q -p fizz-music --example mline -- "$m_arr" "$m_ticks")"
    [ "$music" = "$host" ] || fail "/REPLAY ($name): '$music' differs from the host's '$host'"
    printf '%s\n' "$out" | grep -qE '^MUSIC DETAIL ORDER ok MAX_PER_TICK ([0-9]|1[0-6])$' || fail "/REPLAY ($name): bad MUSIC DETAIL line"
  fi
  local bios
  bios="$(printf '%s\n' "$out" | sed -n 's/^BIOS TICKS \([0-9]*\)$/\1/p')"
  { [ -n "$bios" ] && [ "$bios" -ge 383 ] && [ "$bios" -le 387 ]; } || fail "/REPLAY ($name): BIOS TICKS '$bios' is outside 383-387"
  echo "REPLAY case $name ok ($expect; $audio; $music; BIOS TICKS $bios)"
}
cfg_none
AUDIO=speaker replay_case 'no file, no card' '' 'SOUND MUSIC off FX speaker'
AUDIO=silent replay_case '/NOSOUND, sb16' "$SB16" 'SOUND MUSIC off FX off' /NOSOUND
AUDIO=opl2 replay_case '/ADLIB /MODERN, opl2 (channel 5)' "$(sb sb1)" 'SOUND MUSIC opl2 FX ch5' /MODERN /ADLIB
AUDIO=opl_pair replay_case '/SB, sb16 (OPL3 pair, channels 5 and 14)' "$SB16" 'SOUND MUSIC opl3 FX ch14' /SB
AUDIO=opl_pair replay_case 'auto, sbpro2 (OPL3 pair, channels 5 and 14)' "$(sb sbpro2)" 'SOUND MUSIC opl3 FX ch14'
AUDIO=opl replay_case '/ADLIB, sb16 with oplmode opl2' "$(sb sb16 'oplmode = opl2')" 'SOUND MUSIC opl2 FX ch5'
AUDIO=opl replay_case 'AdLib card alone (no Sound Blaster)' "$(sb none 'oplmode = opl2')" 'SOUND MUSIC opl2 FX ch5'
AUDIO=opl_or replay_case '/MUSIC:SB /FX:ADLIB, sb16 (channel 5 through the OR rule)' "$SB16" 'SOUND MUSIC opl3 FX ch5' /MUSIC:SB /FX:ADLIB
AUDIO=speaker replay_case '/SPEAKER, sb16' "$SB16" 'SOUND MUSIC off FX speaker' /SPEAKER
# Music on and music off give the same transcript and the same AUDIO counts: the pair path is 836 writes either way.
AUDIO=opl_pair replay_case '/MUSIC:OFF, sb16 (the pair, no music)' "$SB16" 'SOUND MUSIC off FX ch14' /MUSIC:OFF
AUDIO=opl_pair replay_case '/MUSIC:ADLIB /FX:SB, sb16 (OPL2 music centred beside the pair)' "$SB16" 'SOUND MUSIC opl2 FX ch14' /MUSIC:ADLIB /FX:SB
AUDIO=opl2 replay_case '/MUSIC:OFF /FX:ADLIB, sb1 (channel 5, no music)' "$(sb sb1)" 'SOUND MUSIC off FX ch5' /MUSIC:OFF /FX:ADLIB
cfg_put "$V2"
AUDIO=speaker_chip replay_case 'file v2, sb16 (music on AdLib, effects on the speaker)' "$SB16" 'SOUND MUSIC opl2 FX speaker'
AUDIO=opl_pair replay_case 'file v2 with a flag over it' "$SB16" 'SOUND MUSIC opl2 FX ch14' /FX:SB
cfg_put 'junk'
AUDIO=opl_pair replay_case 'corrupt file' "$SB16" 'SOUND MUSIC opl3 FX ch14 cfg-length'
cfg_none

# 3f. The AUDIO counts per path are fixed by the effects alone, with music on the chip or not (the music has its own counters): channel 5 on an
# when music holds it, and silence 0.
audio_count() {
  local name="$1" conf="$2" want="$3" out
  shift 3
  out="$(DOS_CONF="$conf" tools/dosrun.sh "$cfgdir" FIZZ2.EXE /REPLAY "$@" | tr -d '\r')"
  printf '%s\n' "$out" | grep -qx "AUDIO WRITES $want" || fail "AUDIO count ($name): expected 'AUDIO WRITES $want', got '$(printf '%s\n' "$out" | grep '^AUDIO ')'"
  echo "AUDIO count $name ok (AUDIO WRITES $want)"
}
cfg_none
audio_count 'ch5 on an OPL2 (419)' "$(sb sb1)" '419 SPEAKER STEPS 0' /MODERN /ADLIB
audio_count 'ch5 through the OR rule on an OPL3 (440)' "$SB16" '440 SPEAKER STEPS 0' /MUSIC:SB /FX:ADLIB
audio_count 'the OPL3 pair (836)' "$SB16" '836 SPEAKER STEPS 0' /SB
audio_count 'silent (0)' "$SB16" '0 SPEAKER STEPS 0' /NOSOUND

# 4. Size.
size="$(wc -c < dist/dos/FIZZ2.EXE | tr -d ' ')"
echo "SIZE $size"
echo "BUDGET $BUDGET"
[ "$size" -le "$BUDGET" ] || fail "FIZZ2.EXE is $size bytes, over the budget of $BUDGET"

echo "DOS CHECK ok"
