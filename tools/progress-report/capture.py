#!/usr/bin/env python3
# Refreshes the progress report's captured media: DOSBox and Chromium screenshots, the DOS check screen and the DOSBox text runs, with captures.json.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/capture.py [--manual [--reuse DIR]] [--keep DIR | --reuse DIR]      (after tools/build-dos.sh and tools/build-web.sh)
#
# With --manual it retakes the manual's screenshots instead (docs/img for DOS, docs/images for the Web) with the same tools and writes no
# records: the manual's captions say what each shows.
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

# DOS grabs from capture-dos.sh: (media file, raw file, what it shows). The replay grabs are every 0.4 s of /REPLAY, picked by index. The HUD
# no longer prints the tick, so each replay entry says where in the script its picture falls rather than a tick number.
DOS = [
    ("dos-puff.png", "replay-04.png", "FIZZ2.EXE /REPLAY in DOSBox, a few seconds into the script: Ben stands at the foot of the left shaft with Fizz 3 and Score 00000 in the translucent HUD box, the caption fzzt beside him, and a small light magenta puff sits where his shot hit the wall on the shaft's right side."),
    ("dos-cluck.png", "replay-08.png", "FIZZ2.EXE /REPLAY in DOSBox, a little before the middle of the script: Ben stands on the second ledge and the chicken, on the ledge above, has its beak open with bwuk-BWUK floating above it in Fizz 1's pixel font."),
    ("dos-stars.png", "replay-12.png", "FIZZ2.EXE /REPLAY in DOSBox, just after Ben took the red key: the key tray in the HUD box shows the red socket filled (with its dot) beside the empty blue one, Fizz reads 2, the chicken, stunned by Ben's shot, has two yellow and white stars above its head, and the caption red gumdrop floats where Ben took the key."),
    ("dos-can.png", "replay-46.png", "FIZZ2.EXE /REPLAY in DOSBox on the gallery in the snack nook: Ben has fired all five Fizz shots (Fizz 0, in light red, and Deaths 1 in the HUD box) and the caption click - no fizz is fading beside him; a soda can stands on the floor at his right, next to the wall, and a second one sits on the right end of the shelf above, with a chocolate bar, a cookie and the blue key."),
    ("dos-hud-snack.png", "replay-61.png", "FIZZ2.EXE /REPLAY in DOSBox on the pogo stick above the shelf, a moment after Ben took a soda can and a snack: the HUD box shows Fizz 5, the points of the snacks as +3 in the place of the word Score, and the score counting up to 00005 (the leading zeros dim), Deaths 1 and the two empty key sockets; the captions crunch (yellow) and fsssht (cyan) float where they happened."),
    ("dos-hud-key.png", "replay-62.png", "FIZZ2.EXE /REPLAY in DOSBox on the pogo stick over the shelf, a moment after Ben took the blue key: the second socket of the HUD's key tray is filled with the blue key and its bar mark, inside a white ring, with +8 above the score that is counting up to 00010, Fizz 5 and Deaths 1; the caption blue gumdrop floats at the key."),
    ("dos-nook-boing.png", "replay-59.png", "FIZZ2.EXE /REPLAY in DOSBox in the snack nook: Ben bounces on the pogo stick beside the right-hand wall of the gallery and the caption boing (yellow) floats at the spot of the bounce, with Fizz 0, Score 00002 and Deaths 1 in the HUD box."),
    ("dos-exit.png", "replay-65.png", "FIZZ2.EXE /REPLAY in DOSBox at the end of the script: Ben stands on the pogo stick at the open blue door in front of the exit with the caption ta-da! (light green) above him, Fizz 5, Score 00010 and Deaths 1 in the HUD box and LEVEL CLEAR below."),
    ("dos-look-up.png", "look-up.png", "FIZZ2.EXE /LIVE in DOSBox, keys typed through the X server: after a jump onto the first ledge Ben holds Up alone for 1.8 seconds, so he looks up and the view has risen to show the chicken, which has just clucked bwuk, bwuk."),
    ("dos-aim-up.png", "aim-up.png", "FIZZ2.EXE /LIVE in DOSBox: Up and Space pressed together, so Ben raises the blaster and a shot rises above him, with the caption fzzt (Fizz 4 in the HUD box)."),
    ("dos-aim-down.png", "aim-down.png", "FIZZ2.EXE /LIVE in DOSBox: Down and Space pressed while Ben is in the air after a jump, so he holds the blaster down, a shot falls below him and the caption fzzt floats by his head."),
    ("dos-paused.png", "paused.png", "FIZZ2.EXE /LIVE in DOSBox after Esc, typed through the X server: the frozen picture dimmed behind the heading Paused with a light magenta rule, the rows Resume (chosen, on a light magenta plate with the grape soda can), Options and Quit to DOS, and the grey footer."),
    ("dos-options.png", "options.png", "FIZZ2.EXE /LIVE in DOSBox after Esc, Down, Enter and Down: the Options screen over the dimmed picture with Music, Effects (chosen), Sound placement, Frame rate, Motion, Captions, Keys and Back, the values on the right in the Fizz colour, and the footer. A genuine DOSBox grab of the game, not the host drawing of the same screen."),
]
# The manual's DOS screenshots: (file under docs/img, raw grab).
MANUAL_DOS = [("dos-slice.png", "replay-00.png"), ("dos-can.png", "replay-46.png"), ("dos-hud-key.png", "replay-62.png"), ("dos-look-up-caption.png", "look-up.png"), ("dos-aim-up.png", "aim-up.png"), ("dos-aim-down.png", "aim-down.png"),
              ("dos-paused.png", "paused.png"), ("dos-options.png", "options.png")]
WEB = {
    "web-start.png": "The Web slice in headless Chromium in a 960 by 540 window, 40 ticks after load with no key pressed: Ben on the floor of the left shaft, the spikes, the first ledges and, at the top of the view, the red key and the chicken; the in-picture HUD panel is in the top left corner, the controls sit in a hint line and the H panel is closed.",
    "web-portrait.png": "The Web slice in a 390 by 780 window (a phone held upright), 40 ticks after load: the view is the same 13 tiles tall and about six tiles across, and the picture fills the window.",
    "web-wide.png": "The Web slice in a 1600 by 520 window, 40 ticks after load: the view is the same 13 tiles tall and much wider, so the right wall and the sky beyond it show, and dark bars fill the rest past the view's width limit.",
    "web-look-up.png": "The Web slice after the script's first 150 ticks put Ben on the first ledge, then 90 ticks of ArrowUp held alone: the view has risen to show the chicken and its caption bwuk-BWUK, fading above it.",
    "web-aim-up.png": "The Web slice, ArrowUp and Space pressed together at tick 30 and shown at tick 36: Ben has his blaster raised and a shot rises above him (Fizz 4 in the HUD panel).",
    "web-nook.png": "The Web slice in a 1280 by 720 window at tick 1195, five ticks after Ben took the soda can on the gallery floor (EVENT 1190 can of the golden transcript) and the tick that fired again (EVENT 1195 fire): Fizz reads 4 in the HUD panel (it was 0), the caption fsssht is fresh at Ben's head with a new shot in front of him, and click - no fizz, from the empty blaster before the can, is still fading behind. Two cheezies hang under the grey shelf, and the shelf carries the blue key, a cookie, a chocolate bar, a cheezie and the second soda can.",
    "web-nook-boing.png": "The Web slice in a 1280 by 720 window at tick 1354, three ticks after a pogo bounce in the snack nook: Ben is in the air on the stick beside the right-hand wall of the gallery and the caption boing sits at the gallery's floor (Fizz 0 and Deaths 1 in the HUD panel).",
    "web-effects.png": "The Web slice in a 1280 by 720 window, 40 ticks after load and H pressed: the panel lists the fifteen effects of the Hear an effect picker by id, from 0 cluck 0 (buh-GAWK) to 14 crunch, with crunch chosen and the Play button beside it. In the page the picker is a drop-down; the capture shows it open as a list because headless Chromium does not draw a drop-down open.",
    "web-can.png": "The Web slice in a 1280 by 720 window at tick 1183, with the blaster empty (Fizz 0 in light red, Deaths 1 in the HUD panel): Ben stands on the gallery floor beside the right-hand wall with a soda can on the floor at his right and a second one on the right end of the shelf above, beside a chocolate bar and a cookie; click - no fizz is still fading.",
    "web-hud-snack.png": "The Web slice in a 1280 by 720 window at tick 1530, eight ticks after Ben took the soda can on the shelf and a snack: the HUD panel shows Fizz 5, +1 in the place of the word Score and the score counting up to 00003, Deaths 1 and two empty key sockets, and the captions crunch and fsssht float where they happened.",
    "web-hud-key.png": "The Web slice in a 1280 by 720 window at tick 1583, four ticks after Ben took the blue key: the HUD panel's second key socket is filled with the blue key inside a white ring, +8 sits over the score, which is counting up (00009 on its way to 00010), Fizz 5 and Deaths 1; the caption blue gumdrop is at the key.",
    "web-puff.png": "The Web slice at tick 108, two ticks after the script's second shot ended against the wall of the shaft: a small light magenta puff hangs at the wall beside the spikes (Fizz 3 in the HUD panel).",
    "web-cluck.png": "The Web slice at tick 201, four ticks after the first cluck of the golden transcript: the chicken's beak is open and its caption floats above it as text over the canvas.",
    "web-paused.png": "The Web slice in a 960 by 540 window after 60 ticks and Esc: the picture is dimmed behind a dark panel titled Paused with Resume (chosen, on a light magenta plate with a soda can), Options and Restart, and a footer line.",
    "web-options.png": "The Web slice in a 960 by 540 window after 60 ticks, Esc, Down and Enter: the Options panel with Sound (chosen), Music, Placement, Motion, Captions, Keys, Fullscreen and Back, each with its value on the right.",
    "web-stars.png": "The Web slice at tick 300, with the chicken stunned by Ben's shot: two yellow and white stars sit above its head (the key chip filled, Fizz 2 in the HUD panel). The page's hash at this tick is the golden transcript's TICK 300 line.",
}
CHECK = ("check.png", "The check screen of FIZZ2.EXE in DOSBox's default machine (VGA, a mouse, no joystick, no AdLib and no Sound Blaster, a 486 with an x87 and 14 MB free), drawn from the 80 by 25 text cells that FIZZ2.EXE /CHECKCELLS printed, by tools/render-cells.py with the machine's VGA console font: the PC speaker row is lit as the sound the game would use.")
TEXT = [
    ("dos-help.txt", "help.txt", "The text of FIZZ2.EXE /? in DOSBox: the modes, the keys, the captions and the sound switches on one screen of 22 lines."),
    ("dos-probe.txt", "probe.txt", "The output of FIZZ2.EXE /PROBE in DOSBox's default machine: the PROBE lines for video, mouse, joystick, FM chip, Sound Blaster, BLASTER, x87, processor and free memory, then the CELLS hash, the SOUND line and the usual platform lines."),
]


def run(cmd, **kw):
    print("+ " + " ".join(cmd), file=sys.stderr)
    return subprocess.run(cmd, check=True, cwd=ROOT, **kw)


def reduce(src, dst, colours=32):
    run([shutil.which("magick") or "convert", src, "-colors", str(colours), "PNG8:" + dst])
    if shutil.which("optipng"):
        run(["optipng", "-quiet", "-o5", dst])


def manual():
    # --reuse DIR takes the DOS grabs from a run that capture.py --keep DIR left (the replay's indexes then match the archive's pictures)
    # and takes only the Web shots fresh.
    reuse = argv_value("--reuse")
    raw = tempfile.mkdtemp(prefix="fizz2-capture.", dir=os.environ.get("TMPDIR") or None)
    dos_raw = os.path.abspath(reuse) if reuse else raw
    try:
        if not reuse:
            run(["xvfb-run", "-a", "-s", "-screen 0 1024x768x24", os.path.join(HERE, "capture-dos.sh"), raw])
        run([sys.executable, "-I", os.path.join(HERE, "capture-web.py"), raw, "--manual"])
        for name, grab in MANUAL_DOS:
            reduce(os.path.join(dos_raw, grab), os.path.join(ROOT, "docs", "img", name))
        with open(os.path.join(raw, "web.json"), encoding="utf-8") as f:
            for shot in json.load(f)["shots"]:
                reduce(os.path.join(raw, shot["file"]), os.path.join(ROOT, "docs", "images", shot["file"]), 128 if "paused" in shot["file"] or "options" in shot["file"] else 32)
                print("%s  %s" % (shot["file"], shot["hud"].replace("\n", " | ")))
    finally:
        shutil.rmtree(raw, ignore_errors=True)
    return 0


def argv_value(flag):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else None


def main():
    if "--manual" in sys.argv:
        return manual()
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    today = datetime.date.today().isoformat()
    os.makedirs(MEDIA, exist_ok=True)
    # --keep DIR leaves the raw grabs in DIR for choosing replay indexes by eye (DOSBox's pace varies, so index n is not the same tick every
    # run); --reuse DIR makes the records from the grabs already in DIR without running anything.
    keep = argv_value("--keep") or argv_value("--reuse")
    raw = os.path.abspath(keep) if keep else tempfile.mkdtemp(prefix="fizz2-capture.", dir=os.environ.get("TMPDIR") or None)
    try:
        if not argv_value("--reuse"):
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
            reduce(os.path.join(raw, shot["file"]), os.path.join(MEDIA, shot["file"]), 128 if shot["file"] in ("web-paused.png", "web-options.png") else 32)
            records.append({"file": shot["file"], "kind": "screenshot", "track": "web", "source": "%s, headless, Page.captureScreenshot, %dx%d window, fake 60 Hz clock" % (web["chromium"], shot["width"], shot["height"]),
                            "shows": WEB[shot["file"]], "hud": shot["hud"], "command": "python3 tools/progress-report/capture.py (capture-web.py)", "commit": commit, "date": today})
    finally:
        if not keep:
            shutil.rmtree(raw, ignore_errors=True)
    with open(os.path.join(MEDIA, "captures.json"), "w", encoding="utf-8") as f:
        json.dump(records, f, indent=1)
        f.write("\n")
    print("wrote %d captures to %s" % (len(records), MEDIA))
    return 0


if __name__ == "__main__":
    sys.exit(main())
