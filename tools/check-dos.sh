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
# 3a. The same with an OPL3 in its own mode and a cluck sounding when the panic comes (the audio service is up): the clean-up list
# silences the chip and puts the timer back, and the exit is still a status 3, not a hang.
status=0
DOS_CONF=$'[sblaster]\nsbtype = sb16' DOS_TIMEOUT=30 tools/dosrun.sh dist/dos FIZZ2.EXE /PANIC > /dev/null || status=$?
[ "$status" = 3 ] || fail "/PANIC with the audio service on an OPL3: status $status (expected 3)"
echo "PANIC (OPL3) exit $status"

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
    speaker) { [ "$writes" -eq 0 ] && [ "$steps" -gt 0 ]; } || fail "/REPLAY ($name): '$audio' (expected speaker steps and no OPL writes)" ;;
    # Effects on the speaker while music holds the chip: the chip is only keyed off at start and stop (no cluck reaches it).
    speaker_chip) { { [ "$writes" -gt 0 ] && [ "$writes" -le 40 ] && [ "$steps" -gt 0 ]; } || fail "/REPLAY ($name): '$audio' (expected speaker steps and only the chip's start and stop writes)"; } ;;
    silent) { [ "$writes" -eq 0 ] && [ "$steps" -eq 0 ]; } || fail "/REPLAY ($name): '$audio' (expected no writes and no steps)" ;;
  esac
  echo "REPLAY case $name ok ($expect; $audio)"
}
cfg_none
AUDIO=speaker replay_case 'no file, no card' '' 'SOUND MUSIC off FX speaker'
AUDIO=silent replay_case '/NOSOUND, sb16' "$SB16" 'SOUND MUSIC off FX off' /NOSOUND
AUDIO=opl replay_case '/SB, sb16 (OPL3, channel 14)' "$SB16" 'SOUND MUSIC opl3 FX ch14' /SB
AUDIO=opl replay_case 'auto, sbpro2 (OPL3, channel 14)' "$(sb sbpro2)" 'SOUND MUSIC opl3 FX ch14'
AUDIO=opl replay_case '/ADLIB /MODERN, opl2 (channel 5)' "$(sb sb1)" 'SOUND MUSIC opl2 FX ch5' /MODERN /ADLIB
AUDIO=opl replay_case '/ADLIB, sb16 with oplmode opl2' "$(sb sb16 'oplmode = opl2')" 'SOUND MUSIC opl2 FX ch5'
AUDIO=opl replay_case 'AdLib card alone (no Sound Blaster)' "$(sb none 'oplmode = opl2')" 'SOUND MUSIC opl2 FX ch5'
AUDIO=opl replay_case '/MUSIC:SB /FX:ADLIB, sb16 (channel 5 through the OR rule)' "$SB16" 'SOUND MUSIC opl3 FX ch5' /MUSIC:SB /FX:ADLIB
AUDIO=speaker replay_case '/SPEAKER, sb16' "$SB16" 'SOUND MUSIC off FX speaker' /SPEAKER
cfg_put "$V2"
AUDIO=speaker_chip replay_case 'file v2, sb16 (music on AdLib, effects on the speaker)' "$SB16" 'SOUND MUSIC opl2 FX speaker'
AUDIO=opl replay_case 'file v2 with a flag over it' "$SB16" 'SOUND MUSIC opl2 FX ch14' /FX:SB
cfg_put 'junk'
AUDIO=opl replay_case 'corrupt file' "$SB16" 'SOUND MUSIC opl3 FX ch14 cfg-length'
cfg_none

# 4. Size.
size="$(wc -c < dist/dos/FIZZ2.EXE | tr -d ' ')"
echo "SIZE $size"
echo "BUDGET $BUDGET"
[ "$size" -le "$BUDGET" ] || fail "FIZZ2.EXE is $size bytes, over the budget of $BUDGET"

echo "DOS CHECK ok"
