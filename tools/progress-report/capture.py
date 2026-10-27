#!/usr/bin/env python3
# Refreshes the progress report's captured media: DOSBox and Chromium screenshots and the DOSBox replay transcript, with captures.json.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 tools/progress-report/capture.py      (after tools/check-dos.sh and tools/build-web.sh)
#
# Standard library only, plus xvfb-run, xdotool and ImageMagick for the DOS grabs (capture-dos.sh), Chromium for the Web grabs
# (capture-web.py), and optipng when it is installed. Raw grabs go to a scratch directory; each is reduced to a 32-colour PNG in
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

DOS = [
    ("dos-replay-1.png", "FIZZ2.EXE /REPLAY in DOSBox, about 3 seconds into the 600-tick script: the HUD shows the tick, the state hash and the dash count."),
    ("dos-replay-2.png", "FIZZ2.EXE /REPLAY in DOSBox, about 6 seconds in, after the script's dashes."),
    ("dos-replay-3.png", "FIZZ2.EXE /REPLAY in DOSBox, about 9 seconds in, near the end of the script."),
    ("dos-live.png", "FIZZ2.EXE /LIVE in DOSBox after AUTOTYPE pressed Right four times and Down twice: the marker has moved right and down from the centre."),
]
WEB = {
    "web-start.png": "The Web page in headless Chromium 1.5 seconds after loading, before any key: the 320 by 200 canvas, the marker at the centre and the HUD below.",
    "web-moved.png": "The Web page after ArrowRight held for 1.2 seconds and ArrowDown for 0.6 seconds, sent as real key events through the DevTools protocol.",
    "web-dash.png": "The Web page after one press of Space: the marker has dashed along its heading and the HUD counts one dash.",
}


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
        records = []
        for name, shows in DOS:
            reduce(os.path.join(raw, name), os.path.join(MEDIA, name))
            records.append({"file": name, "kind": "screenshot", "track": "dos", "source": "DOSBox Staging window, grabbed from Xvfb with ImageMagick import",
                            "shows": shows, "command": "python3 tools/progress-report/capture.py (capture-dos.sh)", "commit": commit, "date": today})
        with open(os.path.join(raw, "replay.txt"), encoding="utf-8") as f:
            text = f.read()
        with open(os.path.join(MEDIA, "dos-replay.txt"), "w", encoding="utf-8") as f:
            f.write(text)
        records.append({"file": "dos-replay.txt", "kind": "transcript", "track": "dos", "source": "standard output of FIZZ2.EXE /REPLAY in DOSBox, through tools/dosrun.sh",
                        "shows": "The transcript FIZZ2.EXE printed during the grabbed replay run.", "command": "python3 tools/progress-report/capture.py (capture-dos.sh)", "commit": commit, "date": today})
        with open(os.path.join(raw, "web.json"), encoding="utf-8") as f:
            web = json.load(f)
        for shot in web["shots"]:
            reduce(os.path.join(raw, shot["file"]), os.path.join(MEDIA, shot["file"]))
            records.append({"file": shot["file"], "kind": "screenshot", "track": "web", "source": "%s, headless, Page.captureScreenshot" % web["chromium"],
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
