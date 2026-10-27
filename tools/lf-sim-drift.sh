#!/bin/bash
# Reports how far `fizz-phys` has drifted from Fizz 1's `lf-sim` by re-applying the recorded mechanical transformations.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/lf-sim-drift.sh [--emit DIR]
#
# Reads `crates/sim/src/{tilemap,body,platform,rng}.rs` from the Fizz 1 checkout (never writes to it), applies the
# transformations listed in each fizz-phys file's Provenance comment, and diffs the result against `crates/fizz-phys/src`.
# Prints `drift: none` and exits 0 when they match, or the diff and exits 1. `--emit DIR` writes the transformed files
# to DIR instead of comparing. It needs the Fizz 1 checkout, so it is not part of `tools/check.sh` or CI.
#
# FIZZ1  path to the Fizz 1 checkout (default: ../../liminal-hq/lieutenant-fizz relative to the repository root)
set -euo pipefail
cd "$(dirname "$0")/.."

FIZZ1="${FIZZ1:-../../liminal-hq/lieutenant-fizz}"
PIN=4bce036
SRC="$FIZZ1/crates/sim/src"
DST=crates/fizz-phys/src

emit_dir=""
if [ "${1:-}" = "--emit" ]; then
  emit_dir="${2:?--emit needs a directory}"
  mkdir -p "$emit_dir"
fi

head_now="$(git -C "$FIZZ1" rev-parse --short HEAD)"
if [ "$head_now" != "$PIN" ]; then
  echo "note: Fizz 1 is at $head_now, the pinned commit is $PIN" >&2
fi

# The imports each module gains (the DOS port's `use alloc` and `use crate::fmath` lines, trimmed to what is used).
imports_for() {
  case "$1" in
    tilemap) echo 'use alloc::{vec, vec::Vec};' ;;
    body | platform) echo '#[cfg_attr(test, allow(unused_imports))]
use crate::fmath::FloatMath;' ;;
    rng) echo '' ;;
  esac
}

# The edits a module's Provenance comment lists beyond the common line.
edits_for() {
  case "$1" in
    tilemap) echo 'Edits: `use alloc::{vec, vec::Vec};` added; imports ordered by rustfmt for the 2024 edition.' ;;
    body) echo 'Edits: `use crate::fmath::FloatMath;` added (unused in test builds, where `std` supplies `floor`); imports ordered by rustfmt for the 2024 edition.' ;;
    platform) echo 'Edits: `use crate::fmath::FloatMath;` added (unused in test builds, where `std` supplies `floor` and `cos`); `std::f64::consts::PI` became `core::f64::consts::PI`.' ;;
    rng) echo 'Edits: none beyond this comment.' ;;
  esac
}

# Mechanical transformations: a Provenance line after the SPDX line, the import block after the header,
# `std::f64::consts` becoming `core::f64::consts`, and rustfmt in the 2024 edition's style (import order).
# The `fmath` import is unused in test builds, where `std` supplies inherent `floor` and `cos`.
transform() {
  local name="$1" imports edits
  imports="$(imports_for "$name")"
  edits="$(edits_for "$name")"
  IMPORTS="$imports" EDITS="$edits" awk -v name="$name" -v pin="$PIN" '
    { sub(/std::f64::consts/, "core::f64::consts") }
    { print }
    /^\/\/ SPDX-License-Identifier:/ && !prov {
      print "//"
      print "// Provenance: Derived from Fizz 1 `crates/sim/src/" name ".rs` at commit `" pin "`; transformed as in fizz-dos `m2/patches/sim.patch`."
      print "// " ENVIRON["EDITS"]
      prov = 1
      next
    }
    prov && !done && /^$/ {
      imports = ENVIRON["IMPORTS"]; if (imports != "") { print imports; print "" }
      done = 1
    }
  ' "$SRC/$name.rs" | rustfmt --edition 2024 --emit stdout
}

status=0
for name in tilemap body platform rng; do
  if [ -n "$emit_dir" ]; then
    transform "$name" > "$emit_dir/$name.rs"
    continue
  fi
  if ! diff -u --label "fizz1/$name.rs (transformed)" --label "fizz-phys/$name.rs" \
    <(transform "$name") "$DST/$name.rs"; then
    status=1
  fi
done

[ -n "$emit_dir" ] && exit 0
[ "$status" = 0 ] && echo "drift: none"
exit "$status"
