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
tools/check-headers.sh
python3 -I tools/check-docs.py
printf 'All checks passed.\n'
