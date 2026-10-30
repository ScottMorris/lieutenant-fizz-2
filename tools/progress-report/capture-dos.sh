#!/bin/bash
# Captures screenshots and text output of FIZZ2.EXE from DOSBox Staging running under a virtual X server, for the progress report's media.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: xvfb-run -a -s "-screen 0 1024x768x24" tools/progress-report/capture-dos.sh OUTDIR
#
# Needs dist/dos from tools/check-dos.sh or tools/build-dos.sh, xdotool and ImageMagick. tools/dosrun.sh runs the program with DOS_X11=1,
# so DOSBox draws through SDL's x11 driver into the virtual server; ImageMagick's `import` grabs DOSBox's window from it and xdotool
# presses keys in it, as a person at the keyboard would. The runs:
#
#   /REPLAY        the 1,757-tick slice script in real time, one grab every 0.4 s (replay-00.png, replay-01.png, ...). The program's
#                  HUD no longer prints the tick; DOSBox's pace varies a little from run to run, so capture.py picks grabs by index
#                  and the ticks it names are approximate
#   /LIVE          Ctrl is held for 0.6 s (Ben jumps to the first ledge), then Up alone for 1.8 s (look-up.png), then Up and Space
#                  together (aim-up.png) and Down and Space with Ctrl in the air (aim-down.png), then Esc (paused.png, the Paused
#                  screen), Down and Enter and Down (options.png, the Options screen with Effects chosen), then Esc to leave Options
#                  (the cursor returns to Options on Paused) and Down and Enter to quit to DOS
#   /CHECKCELLS    the check screen's 80 by 25 text cells, printed as CELLROW lines (check-cells.txt), which tools/render-cells.py draws
#   /?             the help screen's text (help.txt)
#   /PROBE         what the program finds in DOSBox's machine (probe.txt)
#
# The program's output of each run goes to OUTDIR/<run>.txt. The grabs are DOSBox's scaled window, not the raw 320x200 frame.
set -euo pipefail
cd "$(dirname "$0")/../.."
out="$(mkdir -p "$1" && cd "$1" && pwd)"
[ -n "${DISPLAY:-}" ] || { echo "capture-dos: no DISPLAY; run under xvfb-run" >&2; exit 2; }
[ -f dist/dos/FIZZ2.EXE ] || { echo "capture-dos: no dist/dos/FIZZ2.EXE; run tools/build-dos.sh" >&2; exit 2; }

window() { xdotool search --sync --name 'FIZZ2' | head -n 1; }

# The replay, grabbed every 0.4 s for as long as it runs (it takes about 21 seconds, and the grabs themselves take time).
DOS_X11=1 DOS_TIMEOUT=60 tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY > "$out/replay.txt" 2>&1 &
pid=$!
win="$(window)"
for n in $(seq 0 79); do
  kill -0 "$pid" 2>/dev/null || break
  import -window "$win" "$out/replay-$(printf %02d "$n").png" 2>/dev/null || true
  sleep 0.4
done
wait "$pid"

# Live play with keys typed through the X server.
DOS_X11=1 DOS_TIMEOUT=60 tools/dosrun.sh dist/dos FIZZ2.EXE /LIVE > "$out/live.txt" 2>&1 &
pid=$!
win="$(window)"
xset r off 2>/dev/null || true
xdotool windowfocus "$win"
sleep 2
xdotool keydown Control_L; sleep 0.6; xdotool keyup Control_L
sleep 1.5
xdotool keydown Up; sleep 1.8
import -window "$win" "$out/look-up.png"
xdotool keyup Up; sleep 1.2
xdotool keydown Up; xdotool keydown space; sleep 0.08
import -window "$win" "$out/aim-up.png"
xdotool keyup space; xdotool keyup Up; sleep 1.2
xdotool keydown Control_L; sleep 0.4; xdotool keydown Down; xdotool keydown space; sleep 0.08
import -window "$win" "$out/aim-down.png"
xdotool keyup space; xdotool keyup Down; xdotool keyup Control_L; sleep 0.5
xdotool key Escape; sleep 0.6
import -window "$win" "$out/paused.png"
xdotool key Down; sleep 0.2; xdotool key Return; sleep 0.4
xdotool key Down; sleep 0.6
import -window "$win" "$out/options.png"
xdotool key Escape; sleep 0.4
xdotool key Down; sleep 0.2; xdotool key Return
wait "$pid"

# Text runs, with no display.
tools/dosrun.sh dist/dos FIZZ2.EXE /CHECKCELLS > "$out/check-cells.txt" 2>&1
tools/dosrun.sh dist/dos FIZZ2.EXE '/?' > "$out/help.txt" 2>&1
tools/dosrun.sh dist/dos FIZZ2.EXE /PROBE > "$out/probe.txt" 2>&1
ls "$out"
