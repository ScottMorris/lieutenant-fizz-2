#!/usr/bin/env python3
# Refreshes the progress report's captured media: DOSBox and Chromium screenshots, the DOS check screen and the DOSBox text runs, with captures.json.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/capture.py      (after tools/build-dos.sh and tools/build-web.sh)
#
# Standard library only, plus xvfb-run, xdotool and ImageMagick for the DOS grabs (capture-dos.sh), a Linux console font for the check
# screen (tools/render-cells.py), Chromium for the Web grabs (capture-web.py), and optipng when it is installed. Raw grabs go to a scratch directory; each is reduced to a 32-colour PNG in
# media/ and recorded in media/captures.json with what it shows, the command, the commit of the tree and the date. The build reads
# captures.json, shows each capture with that record, and refuses a media file that has no record or a record whose file is missing.
import datetime
import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
MEDIA = os.path.join(HERE, "media")

# DOS grabs from capture-dos.sh: (media file, raw file, what it shows). The replay grabs are every 0.4 s of /REPLAY, picked by index; the
# HUD in each picture prints its tick, and the indexes below showed the ticks named when they were chosen.
DOS = [
    ("dos-puff.png", "replay-04.png", "FIZZ2.EXE /REPLAY in DOSBox around tick 114 (the HUD in the picture reads the tick): Ben stands at the foot of the left shaft, and a small cyan puff hangs where his shot hit the wall of the shaft's right side eight ticks earlier."),
    ("dos-cluck.png", "replay-08.png", "FIZZ2.EXE /REPLAY in DOSBox around tick 218: Ben stands on the second ledge and the chicken, on the ledge above, has its beak open with BWUK-BWUK floating above it, drawn as pixels in capitals."),
    ("dos-stars.png", "replay-12.png", "FIZZ2.EXE /REPLAY in DOSBox around tick 321, with KEY RED in the HUD: the chicken, stunned by Ben's shot, has two yellow and white stars above its head."),
    ("dos-exit.png", "replay-48.png", "FIZZ2.EXE /REPLAY in DOSBox near the end of the script: Ben stands at the exit on the gallery in the open air, and the screen reads LEVEL CLEAR."),
    ("dos-look-up.png", "look-up.png", "FIZZ2.EXE /LIVE in DOSBox, keys typed through the X server: after a jump onto the first ledge Ben holds Up alone for 1.8 seconds, so he looks up and the view has risen to show the chicken, which has just clucked BWUK, BWUK."),
    ("dos-aim-up.png", "aim-up.png", "FIZZ2.EXE /LIVE in DOSBox: Up and Space pressed together, so Ben raises the blaster and a shot rises above him (AMMO 4)."),
    ("dos-aim-down.png", "aim-down.png", "FIZZ2.EXE /LIVE in DOSBox: Down and Space pressed while Ben is in the air after a jump, so he holds the blaster down and a shot falls below him (AMMO 3)."),
]
WEB = {
    "web-start.png": "The Web slice in headless Chromium in a 960 by 540 window, 40 ticks after load with no key pressed: Ben on the floor of the left shaft, the spikes, the first ledges and, at the top of the view, the red key and the chicken; the controls sit in a hint line and the panel is closed.",
    "web-portrait.png": "The Web slice in a 390 by 780 window (a phone held upright), 40 ticks after load: the view is the same 13 tiles tall and about six tiles across, and the picture fills the window.",
    "web-wide.png": "The Web slice in a 1600 by 520 window, 40 ticks after load: the view is the same 13 tiles tall and much wider, so the right wall and the sky beyond it show, and dark bars fill the rest past the view's width limit.",
    "web-look-up.png": "The Web slice after the script's first 150 ticks put Ben on the first ledge, then 90 ticks of ArrowUp held alone: the view has risen to show the chicken and its caption bwuk-BWUK, fading above it.",
    "web-aim-up.png": "The Web slice, ArrowUp and Space pressed together at tick 30 and shown at tick 36: Ben has his blaster raised and a shot rises above him (AMMO 4).",
    "web-puff.png": "The Web slice at tick 108, two ticks after the script's second shot ended against the wall of the shaft: a small cyan puff hangs at the wall beside the spikes (AMMO 3).",
    "web-cluck.png": "The Web slice at tick 201, four ticks after the first cluck of the golden transcript: the chicken's beak is open and its caption floats above it as text over the canvas.",
    "web-stars.png": "The Web slice at tick 300, with the chicken stunned by Ben's shot: two yellow and white stars sit above its head (KEY carried, AMMO 2). The page's hash at this tick is the golden transcript's TICK 300 line.",
}
CHECK = ("check.png", "The check screen of FIZZ2.EXE in DOSBox's default machine (VGA, a mouse, no joystick, no AdLib and no Sound Blaster, a 486 with an x87 and 14 MB free), drawn from the 80 by 25 text cells that FIZZ2.EXE /CHECKCELLS printed, by tools/render-cells.py with the machine's VGA console font: the PC speaker row is lit as the sound the game would use.")
TEXT = [
    ("dos-help.txt", "help.txt", "The text of FIZZ2.EXE /? in DOSBox: the modes, the keys, the captions and the sound switches on one screen of 22 lines."),
    ("dos-probe.txt", "probe.txt", "The output of FIZZ2.EXE /PROBE in DOSBox's default machine: the PROBE lines for video, mouse, joystick, FM chip, Sound Blaster, BLASTER, x87, processor and free memory, then the CELLS hash, the SOUND line and the usual platform lines."),
]


def run(cmd, **kw):
    print("+ " + " ".join(cmd), file=sys.stderr)
    return subprocess.run(cmd, check=True, cwd=ROOT, **kw)


def reduce(src, dst):
    run([shutil.which("magick") or "convert", src, "-colors", "32", "PNG8:" + dst])
    if shutil.which("optipng"):
        run(["optipng", "-quiet", "-o5", dst])


def main():
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    today = datetime.date.today().isoformat()
    os.makedirs(MEDIA, exist_ok=True)
    raw = tempfile.mkdtemp(prefix="fizz2-capture.", dir=os.environ.get("TMPDIR") or None)
    try:
        run(["xvfb-run", "-a", "-s", "-screen 0 1024x768x24", os.path.join(HERE, "capture-dos.sh"), raw])
        run([sys.executable, "-I", os.path.join(HERE, "capture-web.py"), raw])
        run([sys.executable, "-I", os.path.join(ROOT, "tools", "render-cells.py"), os.path.join(raw, "check-cells.txt"), os.path.join(raw, "cells")])
        records = []
        dos_source = "DOSBox Staging window, grabbed from Xvfb with ImageMagick import"
        for name, grab, shows in DOS:
            reduce(os.path.join(raw, grab), os.path.join(MEDIA, name))
            records.append({"file": name, "kind": "screenshot", "track": "dos", "source": dos_source, "shows": shows,
                            "command": "python3 tools/progress-report/capture.py (capture-dos.sh)", "commit": commit, "date": today})
        reduce(os.path.join(raw, "cells", "check.png"), os.path.join(MEDIA, CHECK[0]))
        records.append({"file": CHECK[0], "kind": "screenshot", "track": "dos", "source": "FIZZ2.EXE /CHECKCELLS in DOSBox through tools/dosrun.sh, drawn by tools/render-cells.py",
                        "shows": CHECK[1], "command": "python3 tools/progress-report/capture.py (capture-dos.sh, render-cells.py)", "commit": commit, "date": today})
        transcripts = [("dos-replay.txt", "replay.txt", "standard output of FIZZ2.EXE /REPLAY in DOSBox, through tools/dosrun.sh",
                        "The transcript FIZZ2.EXE printed during the grabbed replay run.")]
        transcripts += [(n, g, "standard output of FIZZ2.EXE %s in DOSBox, through tools/dosrun.sh" % ("/?" if "help" in n else "/PROBE"), shows) for n, g, shows in TEXT]
        for name, grab, source, shows in transcripts:
            with open(os.path.join(raw, grab), encoding="utf-8") as f:
                text = f.read().replace("\r", "")
            with open(os.path.join(MEDIA, name), "w", encoding="utf-8") as f:
                f.write(text)
            records.append({"file": name, "kind": "transcript", "track": "dos", "source": source, "shows": shows,
                            "command": "python3 tools/progress-report/capture.py (capture-dos.sh)", "commit": commit, "date": today})
        with open(os.path.join(raw, "web.json"), encoding="utf-8") as f:
            web = json.load(f)
        for shot in web["shots"]:
            reduce(os.path.join(raw, shot["file"]), os.path.join(MEDIA, shot["file"]))
            records.append({"file": shot["file"], "kind": "screenshot", "track": "web", "source": "%s, headless, Page.captureScreenshot, %dx%d window, fake 60 Hz clock" % (web["chromium"], shot["width"], shot["height"]),
                            "shows": WEB[shot["file"]], "hud": shot["hud"], "command": "python3 tools/progress-report/capture.py (capture-web.py)", "commit": commit, "date": today})
    finally:
        shutil.rmtree(raw, ignore_errors=True)
    with open(os.path.join(MEDIA, "captures.json"), "w", encoding="utf-8") as f:
        json.dump(records, f, indent=1)
        f.write("\n")
    print("wrote %d captures to %s" % (len(records), MEDIA))
    return 0


if __name__ == "__main__":
    sys.exit(main())
