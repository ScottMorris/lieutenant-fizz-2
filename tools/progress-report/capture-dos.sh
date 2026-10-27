#!/bin/bash
# Captures screenshots of FIZZ2.EXE from DOSBox Staging running under a virtual X server, for the progress report's media.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: xvfb-run -a -s "-screen 0 1024x768x24" tools/progress-report/capture-dos.sh OUTDIR
#
# Needs dist/dos from tools/check-dos.sh, xdotool and ImageMagick. tools/dosrun.sh runs the program with DOS_X11=1, so DOSBox
# draws through SDL's x11 driver into the virtual server, and ImageMagick's `import` grabs DOSBox's window from it. Two runs:
#
#   /REPLAY        three grabs, about 3, 6 and 9 seconds into the 600-tick replay (dos-replay-1.png to -3.png)
#   /LIVE          one grab after DOSBox's AUTOTYPE has pressed Right four times and Down twice (dos-live.png)
#
# The program's output of each run goes to OUTDIR/<run>.txt. The grabs are DOSBox's scaled window, not the raw 320x200 frame.
set -euo pipefail
cd "$(dirname "$0")/../.."
out="$(mkdir -p "$1" && cd "$1" && pwd)"
[ -n "${DISPLAY:-}" ] || { echo "capture-dos: no DISPLAY; run under xvfb-run" >&2; exit 2; }
[ -f dist/dos/FIZZ2.EXE ] || { echo "capture-dos: no dist/dos/FIZZ2.EXE; run tools/check-dos.sh" >&2; exit 2; }

# grab <name> waits for the DOSBox window, then saves it.
window() { xdotool search --sync --name 'FIZZ2' | head -n 1; }

DOS_X11=1 DOS_TIMEOUT=40 tools/dosrun.sh dist/dos FIZZ2.EXE /REPLAY > "$out/replay.txt" 2>&1 &
pid=$!
win="$(window)"
for n in 1 2 3; do
  sleep 3
  import -window "$win" "$out/dos-replay-$n.png"
done
wait "$pid"

DOS_X11=1 DOS_TIMEOUT=50 DOS_AUTOTYPE='-w 2 -p 0.3 right right right right down down' tools/dosrun.sh dist/dos FIZZ2.EXE /LIVE > "$out/live.txt" 2>&1 &
pid=$!
win="$(window)"
sleep 6
import -window "$win" "$out/dos-live.png"
wait "$pid"
ls "$out"
