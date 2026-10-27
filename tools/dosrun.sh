#!/bin/bash
# Runs a DOS command in headless DOSBox Staging and passes back its output and exit status.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Provenance: adapted from Desk tools/dosrun.sh. Changes here: the examples and messages name this repository's programs and
# setup script, the scratch directory is `fizz2-dosrun.*`, and the reference to Desk's Xvfb container is dropped.
#
# usage: tools/dosrun.sh <dir> <command> [args...]
#
#   <dir>      mounted as C: and made the current directory; holds the program and CWSDPMI.EXE
#   <command>  run there, e.g. `FIZZ2.EXE /PANIC`
#
# The program's standard output goes to this script's standard output (with DOS line ends removed) and its exit status becomes
# this script's, so `tools/dosrun.sh dist/dos RTSMOKE.EXE` behaves like running the program. Two statuses are the script's own: 124 when
# the command outlived DOS_TIMEOUT seconds (default 60), and 125 when DOSBox itself failed or refused a setting, in which case its log is printed to
# standard error. Set DOS_VERBOSE=1 to always see DOSBox's log.
#
# The emulated video card is `svga_s3` (VESA BIOS extensions, a linear frame buffer). A program that must see another card sets
# DOS_MACHINE (for example `vgaonly`, which has no VBE) for that run only; the default is unchanged.
#
# The emulated CPU speed is `cpu_cycles = max` unless DOS_CYCLES names another (a plain number such as `26800`, about a 486DX2-66), for
# that run only. Timing runs need a fixed figure: `max` gives whatever the host offers. DOSBox Staging 0.83 rejects the form
# `fixed 26800` and quietly runs at 3000 instead, so a setting it refuses is an error here: the script exits 125 and names the
# line from DOSBox's log.
#
# Keys can be typed into the program with DOS_AUTOTYPE, the arguments of DOSBox's AUTOTYPE command (for example `-w 3 -p 0.3 a b esc`):
# the keys go in through the emulated keyboard controller, so a program that reads port 0x60 sees them as a keyboard would send them.
# DOS_CONF appends text to the generated dosbox.conf, for a section the script does not set (for example `[mouse]` and
# `builtin_dos_mouse_driver = off`, which takes the mouse driver away). DOS_X11=1 keeps the environment's DISPLAY and uses SDL's x11
# driver instead of the dummy one, for a run under a virtual X server.
#
# DOSBox needs no display: SDL_VIDEODRIVER=dummy gives it a video driver that draws nowhere, and SDL_AUDIODRIVER=dummy the same
# for audio. The environment's DISPLAY and WAYLAND_DISPLAY are removed so a developer's desktop is never used by accident.
#
# DOS has no way to hand an exit status to its host, and DOSBox Staging does not expand %ERRORLEVEL%. The generated batch file
# recovers it with a ladder of `IF ERRORLEVEL n` tests, which hold for every n up to the status, so the last line that matches
# is the status.
set -euo pipefail
HERE="$(cd "$(dirname "$0")/.." && pwd)"

[ $# -ge 2 ] || { sed -n '/^# usage/,/^$/p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }
DIR="$(cd "$1" && pwd)"
shift

DOSBOX=("$HERE"/vendor/dosbox/*/dosbox)
[ -x "${DOSBOX[0]}" ] || { echo "dosrun: no DOSBox in vendor/; run tools/setup-dos.sh" >&2; exit 125; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/fizz2-dosrun.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# The scratch directory is mounted as D: so the program's own directory only gets what the program writes.
{
  echo '@echo off'
  [ -z "${DOS_AUTOTYPE:-}" ] || echo "autotype $DOS_AUTOTYPE"
  echo "$* > D:\\STDOUT.TXT"
  echo 'set RC=0'
  for n in $(seq 1 255); do echo "if errorlevel $n set RC=$n"; done
  echo 'echo %RC%> D:\EXITCODE.TXT'
} | sed 's/$/\r/' > "$WORK/RUN.BAT"

cat > "$WORK/dosbox.conf" <<CONF
[sdl]
fullscreen = false
# 'texture' draws through SDL's software renderer. The default asks for OpenGL, which the dummy video driver cannot provide.
output = texture

[dosbox]
machine = ${DOS_MACHINE:-svga_s3}
memsize = 16

[cpu]
# The interpreter is the slowest core and the most predictable. cputype 486 makes DOSBox fault on any instruction a 486 lacks
# (CPUID, CMOV and so on), so a build that needs a Pentium fails here instead of on the target.
core = normal
cputype = 486
cpu_cycles = ${DOS_CYCLES:-max}
cpu_cycles_protected = auto

[mixer]
nosound = true

[midi]
mididevice = none

[sblaster]
sbtype = none

[speaker]
pcspeaker = off

[autoexec]
mount c "$DIR"
mount d "$WORK"
c:
call d:\\run.bat
exit
CONF
[ -z "${DOS_CONF:-}" ] || printf '\n%s\n' "$DOS_CONF" >> "$WORK/dosbox.conf"

status=0
if [ -n "${DOS_X11:-}" ]; then
  video=(env SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy)
else
  video=(env -u DISPLAY -u WAYLAND_DISPLAY SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy)
fi
"${video[@]}" \
  timeout "${DOS_TIMEOUT:-60}" "${DOSBOX[0]}" --noprimaryconf --nolocalconf --conf "$WORK/dosbox.conf" \
  > "$WORK/dosbox.log" 2>&1 || status=$?

# DOSBox answers a setting it cannot parse with a log line and carries on with its own default, which makes every later figure
# wrong without any sign of it. Settings the script writes are therefore checked: any "Invalid '<name>' setting" line is fatal.
if grep -q "Invalid '" "$WORK/dosbox.log"; then
  echo "dosrun: DOSBox rejected a setting and used its default instead; the run is not valid:" >&2
  grep "Invalid '" "$WORK/dosbox.log" >&2
  exit 125
fi

if [ "$status" = 124 ]; then
  echo "dosrun: timed out after ${DOS_TIMEOUT:-60}s: $*" >&2
  [ -f "$WORK/STDOUT.TXT" ] && tr -d '\r' < "$WORK/STDOUT.TXT"
  exit 124
fi
if [ "$status" != 0 ] || [ ! -f "$WORK/EXITCODE.TXT" ]; then
  echo "dosrun: DOSBox failed (status $status); its log follows" >&2
  cat "$WORK/dosbox.log" >&2
  exit 125
fi
[ -z "${DOS_VERBOSE:-}" ] || cat "$WORK/dosbox.log" >&2

[ ! -f "$WORK/STDOUT.TXT" ] || tr -d '\r' < "$WORK/STDOUT.TXT"
exit "$(tr -d '\r\n ' < "$WORK/EXITCODE.TXT")"
