#!/bin/bash
# Builds the local preview site into dist/preview: a landing page, the Web build (web/) and FIZZ2.EXE in js-dos (dos/), one bundle per DOS
# mode. The js-dos settings are Fizz 1 DOS's browser settings (tools/preview/dosbox.web.conf) with an SB16 for the OPL3 pair. Bundle names
# carry a hash of their contents and the DOS page reads the current name from a never-cached bundles.json, so js-dos's aggressive caching
# cannot show an old build. The emulator is for looking: its timing is the host's, so pin nothing from it (tools/check-dos.sh pins).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/preview-site.sh [--no-build] [--serve [port]] [--bind ADDRESS]
#   --no-build   use the dist/web and dist/dos already there
#   --serve      then serve dist/preview (tools/preview-serve.py; port 8080 unless given)
set -euo pipefail
cd "$(dirname "$0")/.."

JSDOS_VERSION=8.5.3
JSDOS_SHA256=711f7741486daef927c7f2b6ac6c1037b830dffc75ebf7d7106173f18fbe8756
build=1 serve=0 port=8080 bind=127.0.0.1
while [ $# -gt 0 ]; do
  case "$1" in
    --no-build) build=0 ;;
    --serve) serve=1; if [[ "${2:-}" =~ ^[0-9]+$ ]]; then port="$2"; shift; fi ;;
    --bind) bind="${2:?--bind needs an address}"; shift ;;
    *) echo "usage: tools/preview-site.sh [--no-build] [--serve [port]] [--bind ADDRESS]" >&2; exit 2 ;;
  esac
  shift
done

if [ "$build" = 1 ]; then
  tools/build-web.sh
  tools/build-dos.sh
fi
[ -f dist/web/index.html ] || { echo "preview-site.sh: no dist/web (run tools/build-web.sh)" >&2; exit 1; }
[ -f dist/dos/FIZZ2.EXE ] || { echo "preview-site.sh: no dist/dos/FIZZ2.EXE (run tools/build-dos.sh)" >&2; exit 1; }
[ -f dist/dos/CWSDPMI.EXE ] || { echo "preview-site.sh: no dist/dos/CWSDPMI.EXE (run tools/build-dos.sh)" >&2; exit 1; }

# js-dos, unmodified, as Fizz 1 DOS vendors it: the page script, its style and the DOSBox backend with the zip reader. Kept in the
# git-ignored vendor/ and fetched once.
JS=vendor/js-dos
if [ ! -f "$JS/js-dos.js" ]; then
  command -v npm >/dev/null || { echo "preview-site.sh: npm is needed once, to fetch js-dos $JSDOS_VERSION" >&2; exit 1; }
  work="$(mktemp -d "${TMPDIR:-/tmp}/jsdos.XXXXXX")"
  trap 'rm -rf "$work"' EXIT
  (cd "$work" && npm pack "js-dos@$JSDOS_VERSION" --silent >/dev/null)
  sum="$(sha256sum "$work/js-dos-$JSDOS_VERSION.tgz" | cut -d' ' -f1)"
  [ "$sum" = "$JSDOS_SHA256" ] || { echo "preview-site.sh: js-dos $JSDOS_VERSION tarball is $sum, expected $JSDOS_SHA256" >&2; exit 1; }
  tar -xzf "$work/js-dos-$JSDOS_VERSION.tgz" -C "$work"
  rm -rf "$JS"
  mkdir -p "$JS/emulators"
  cp "$work/package/dist/js-dos.js" "$work/package/dist/js-dos.css" "$JS/"
  for f in emulators.js wdosbox.js wdosbox.wasm wlibzip.js wlibzip.wasm; do cp "$work/package/dist/emulators/$f" "$JS/emulators/$f"; done
  printf 'js-dos %s (GPL-2.0), https://js-dos.com, copied unmodified from the npm package (tarball sha256 %s).\n' "$JSDOS_VERSION" "$JSDOS_SHA256" > "$JS/PROVENANCE.txt"
fi

SITE=dist/preview
rm -rf "$SITE"
mkdir -p "$SITE/web" "$SITE/dos/js-dos"
cp -r dist/web/. "$SITE/web/"
cp tools/preview/index.html "$SITE/index.html"
cp tools/preview/fizz.css tools/preview/hero.svg "$SITE/"
cp tools/preview/dos.html "$SITE/dos/index.html"
cp tools/preview/player.js tools/preview/style.css "$SITE/dos/"
cp -r "$JS/." "$SITE/dos/js-dos/"

python3 -I - "$SITE" <<'PY'
import datetime, hashlib, json, os, subprocess, sys, zipfile

site = sys.argv[1]
exe = open("dist/dos/FIZZ2.EXE", "rb").read()
cws = open("dist/dos/CWSDPMI.EXE", "rb").read()
conf = open("tools/preview/dosbox.web.conf").read()
# Each mode is the switch of its own name, except the help screen and the bare DOS prompt. (The interactive /CHECK waits for a key, so
# no script may run it; this bundle is for a person at a browser, and the name comes from the mode so the line says nothing literal.)
modes = {m: "FIZZ2.EXE /" + m.upper() for m in ("live", "replay", "check", "bench")}
# A person at a browser plays without /LIVE's 30-second limit (it is there so a headless run cannot hang) and with the Full sky: on
# the default Auto sky the game flips between Full and Light by how long its frames take, and js-dos's timing follows the host's load.
modes["live"] += " /NOLIMIT /SKY:FULL"
modes["help"] = "FIZZ2.EXE /?"
modes["shell"] = "@rem the DOS prompt"
# The bundle's name carries a hash of everything in it, so a rebuilt game is a new URL.
h = hashlib.sha1(exe + cws + "".join(conf.replace("@COMMAND@", c) for c in modes.values()).encode()).hexdigest()[:10]
epoch = int(subprocess.run(["git", "log", "-1", "--format=%ct"], capture_output=True, text=True).stdout.strip() or 0)
stamp = datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc).timetuple()[:6]
stamp = max(stamp, (1980, 1, 1, 0, 0, 0))  # zip times start in 1980

def add(z, name, data):
    info = zipfile.ZipInfo(name, stamp)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = 0o644 << 16
    z.writestr(info, data)

manifest = {"hash": h, "bundles": {}}
for mode, cmd in modes.items():
    name = "fizz2-%s.%s.jsdos" % (mode, h)
    manifest["bundles"][mode] = name
    with zipfile.ZipFile(os.path.join(site, "dos", name), "w") as z:
        add(z, ".jsdos/jsdos.json", '{"version":"8.xx"}\n')
        add(z, ".jsdos/dosbox.conf", conf.replace("@COMMAND@", cmd))
        add(z, "FIZZ2.EXE", exe)
        add(z, "CWSDPMI.EXE", cws)
json.dump(manifest, open(os.path.join(site, "dos", "bundles.json"), "w"))
rev = subprocess.run(["git", "log", "-1", "--format=%h %s"], capture_output=True, text=True).stdout.strip()
json.dump({"built": datetime.datetime.now().strftime("%Y-%m-%d %H:%M"), "source": rev}, open(os.path.join(site, "build-info.json"), "w"))
print("DOS bundles %s" % h)
PY
du -sh "$SITE"

if [ "$serve" = 1 ]; then
  exec python3 tools/preview-serve.py "$SITE" "$port" --bind "$bind"
fi
