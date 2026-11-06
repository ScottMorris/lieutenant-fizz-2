#!/bin/bash
# Counts the instructions the ground costs a frame on the host, for the indoor and the gallery (open air) cameras of `frame
# bench`, or with --486 estimates the 486 clocks of the DOS build's own drawing.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/bench-draw.sh
#        tools/bench-draw.sh --486 [--full] [--night]
#
# Builds the `frame` example at opt-level "s" (the DOS build's) with debug information, runs one round of `bench` under
# `valgrind --tool=callgrind`, collecting only while `frame::draw` runs and dumping after each call, so each part of the output is
# one frame's instructions, and prints the average and the worst frame for each class of camera and for all of them. Host
# instructions say nothing about a 486's cycles, but they are exact and repeatable, which timing is not; the figure to compare
# between changes. Needs valgrind. Its build goes to target/bench-draw so it does not rebuild the workspace's.
#
# With --486 the drawing is the DOS build's own machine code: tools/bench486 (the same crates built for `dos/dos-elf.json`, i486
# with x87 floating point, at the DOS workspace's release profile, with the DOS runtime's `memcpy` and `memset`) as a static
# 32-bit Linux program, under 32-bit callgrind with instruction and jump counts. Two runs: the same 48 cameras, ground only,
# each a part (their list is checked against the `frame` example's), and the replay drawn every tick as FIZZ2.EXE /BENCH draws
# it (captions on, by day and the Light sky unless --full or --night), whose `BENCH FRAME` digests are FIZZ2.EXE's. For each,
# tools/cyc486.py prints the instructions (about DOSBox's cycles, since it charges one an instruction), the 486 clocks estimated
# from the i486 datasheet, and the functions that cost most. Needs valgrind with its 32-bit tools, objdump and the pinned
# toolchain's rust-src (no DOS toolchain). Its build goes to target/bench486.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v valgrind > /dev/null 2>&1 || { echo "bench-draw: valgrind is not installed" >&2; exit 1; }
i486=0
script_args=()
for a in "$@"; do
  case "$a" in
    --486) i486=1 ;;
    --full | --night) script_args+=("$a") ;;
    *) echo "bench-draw: unknown argument $a (see the usage at the top of the script)" >&2; exit 2 ;;
  esac
done
[ "$i486" = 1 ] || [ "${#script_args[@]}" = 0 ] || { echo "bench-draw: --full and --night go with --486" >&2; exit 2; }
export CARGO_TARGET_DIR=target/bench-draw CARGO_PROFILE_RELEASE_DEBUG=true CARGO_PROFILE_RELEASE_OPT_LEVEL=s
cargo build --release --quiet -p fizz-slice-view --example frame
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT

if [ "$i486" = 1 ]; then
  command -v objdump > /dev/null 2>&1 || { echo "bench-draw: objdump is not installed" >&2; exit 1; }
  root=$PWD
  (cd tools/bench486 && RUSTC_BOOTSTRAP=1 CARGO_TARGET_DIR="$root/target/bench486" \
    cargo build --release --quiet --locked --target "$root/dos/dos-elf.json")
  bin=target/bench486/dos-elf/release/bench486
  cg=(valgrind --tool=callgrind --dump-instr=yes --collect-jumps=yes --compress-pos=no)
  target/bench-draw/release/examples/frame bench all 1 | grep '^BENCH CAM' > "$out/host-cameras.txt"
  "${cg[@]}" --toggle-collect=bench486_draw_ground --dump-after=bench486_draw_ground --callgrind-out-file="$out/cam" \
    "$bin" cameras > "$out/cameras.txt" 2> "$out/valgrind-cameras.txt"
  if ! cmp -s "$out/cameras.txt" "$out/host-cameras.txt"; then
    echo "bench-draw: tools/bench486's cameras are not the frame example's" >&2
    diff "$out/host-cameras.txt" "$out/cameras.txt" >&2
    exit 1
  fi
  awk '{print $3}' "$out/cameras.txt" > "$out/labels.txt"
  parts=()
  for i in $(seq 1 "$(wc -l < "$out/labels.txt")"); do parts+=("$out/cam.$i"); done
  echo "The ground alone at the bench cameras (the DOS build's code, an estimate; see docs/PLATFORM.md):"
  python3 -I tools/cyc486.py --top 8 --parts "$out/labels.txt" "${parts[@]}"
  "${cg[@]}" --toggle-collect=bench486_draw --callgrind-out-file="$out/script" \
    "$bin" script "${script_args[@]}" > "$out/script.txt" 2> "$out/valgrind-script.txt"
  ticks=$(awk '/^BENCH TICKS/ {print $3}' "$out/script.txt")
  echo
  echo "The replay as FIZZ2.EXE /BENCH draws it ${script_args[*]:-(Light sky, by day)}, every tick:"
  grep '^BENCH FRAME' "$out/script.txt" | tr '\n' ' ' | sed 's/BENCH FRAME //g; s/^/digests (tick digest): /; s/ $/\n/'
  python3 -I tools/cyc486.py --top 12 --frames "$ticks" "$out/script"
  exit 0
fi

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
