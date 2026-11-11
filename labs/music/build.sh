#!/bin/bash
# Builds the Music Lab into dist/music-lab: the page, its style, the Fizz fonts and one bundled script with Undertone inside.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: labs/music/build.sh [--serve [port]]
#   --serve   then serve dist/music-lab on 127.0.0.1 (port 8090 unless given)
#
# Also writes dist/music-lab/artifact.html: the same lab as one self-contained file (style, fonts and script inline, no document
# shell), the form a Claude artifact is published in.
set -euo pipefail
cd "$(dirname "$0")/../.."

serve=0 port=8090
while [ $# -gt 0 ]; do
  case "$1" in
    --serve) serve=1; if [[ "${2:-}" =~ ^[0-9]+$ ]]; then port="$2"; shift; fi ;;
    *) echo "usage: labs/music/build.sh [--serve [port]]" >&2; exit 2 ;;
  esac
  shift
done

OUT=dist/music-lab
rm -rf "$OUT"
mkdir -p "$OUT/fonts"
bun install --frozen-lockfile
bun build labs/music/main.ts --outfile "$OUT/lab.js" --target browser --minify
cp labs/music/lab.css "$OUT/"
cp web/fonts/*.woff2 web/fonts/OFL.txt "$OUT/fonts/"

python3 -I - "$OUT" <<'PY'
import base64, sys

out = sys.argv[1]
body = open("labs/music/body.html", encoding="utf-8").read()
page = open("labs/music/index.html", encoding="utf-8").read().replace("<!--BODY-->", body)
open(f"{out}/index.html", "w", encoding="utf-8").write(page)

css = open("labs/music/lab.css", encoding="utf-8").read()
for name in ("fizz-regular", "fizz-mono-bold"):
    data = base64.b64encode(open(f"web/fonts/{name}.woff2", "rb").read()).decode()
    css = css.replace(f'url("fonts/{name}.woff2")', f'url("data:font/woff2;base64,{data}")')
script = open(f"{out}/lab.js", encoding="utf-8").read().replace("</script", "<\\/script")
artifact = (
    '<title>Fizz 2 Music Lab</title>\n<meta name="claude:viewport-fit-sides" content="cover">\n'
    f"<style>\n{css}</style>\n{body}<script type=\"module\">\n{script}\n</script>\n"
)
open(f"{out}/artifact.html", "w", encoding="utf-8").write(artifact)
PY
printf 'Built %s\n' "$OUT"

if [ "$serve" = 1 ]; then
  exec python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$OUT"
fi
