#!/bin/bash
# Counts the instructions the ground costs a frame on the host, for the indoor and the gallery (open air) cameras of `frame bench`.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/bench-draw.sh
#
# Builds the `frame` example at opt-level "s" (the DOS build's) with debug information, runs one round of `bench` under
# `valgrind --tool=callgrind`, collecting only while `frame::draw` runs and dumping after each call, so each part of the output is
# one frame's instructions, and prints the average and the worst frame for each class of camera and for all of them. Host
# instructions say nothing about a 486's cycles, but they are exact and repeatable, which timing is not; the figure to compare
# between changes. Needs valgrind. Its build goes to target/bench-draw so it does not rebuild the workspace's.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v valgrind > /dev/null 2>&1 || { echo "bench-draw: valgrind is not installed" >&2; exit 1; }
export CARGO_TARGET_DIR=target/bench-draw CARGO_PROFILE_RELEASE_DEBUG=true CARGO_PROFILE_RELEASE_OPT_LEVEL=s
cargo build --release --quiet -p fizz-slice-view --example frame
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
valgrind --tool=callgrind --toggle-collect=frame::draw --dump-after=frame::draw --callgrind-out-file="$out/part" \
  target/bench-draw/release/examples/frame bench all 1 > "$out/cameras.txt" 2> "$out/valgrind.txt"
python3 -I - "$out" << 'PY'
import sys

out = sys.argv[1]
cameras = [line.split()[2:] for line in open(out + "/cameras.txt") if line.startswith("BENCH CAM")]


def instructions(path):
    for line in open(path):
        if line.startswith(("totals:", "summary:")):
            return int(line.split()[1])
    raise SystemExit("bench-draw: no totals in " + path)


frames = [(instructions(f"{out}/part.{i + 1}"), *cam) for i, cam in enumerate(cameras)]
for name in ("indoor", "gallery", None):
    chosen = [f for f in frames if name is None or f[1] == name]
    if not chosen:
        continue
    worst = max(chosen)
    print(f"{name or 'all':8} {len(chosen):3} cameras  average {sum(f[0] for f in chosen) // len(chosen):>9,}  "
          f"worst {worst[0]:>9,} at ({worst[2]}, {worst[3]})")
PY
