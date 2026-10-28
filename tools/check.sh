#!/bin/bash
# Runs the host checks for Lieutenant Fizz 2: formatting, lints, tests, licence headers and markdown style.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd dos && cargo fmt --check)
tools/check-headers.sh
python3 -I tools/check-docs.py
python3 -I tools/progress-report/tests/test_report.py
python3 -I tools/progress-report/gather.py --dry-run
# The song source needs Bun and the pinned Undertone (`bun install --frozen-lockfile`); CI always runs these.
if command -v bun > /dev/null 2>&1 && [ -d node_modules/@liminal-hq/undertone ]; then
  bun test assets/music
  bun tools/music/notes.ts --check
  bun test tools/music
  bun tools/music/generate.ts --check
else
  printf 'skipped: the music source checks (need bun and node_modules)\n'
fi
printf 'All checks passed.\n'
