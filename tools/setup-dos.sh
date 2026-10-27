#!/bin/bash
# Makes the DOS toolchain (DJGPP stub tools, DOSBox, CWSDPMI) available as vendor/ in this checkout or worktree.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Provenance: adapted from fizz-2-dos-poc1 tools/setup-dos.sh, which linked $DOS_TOOLCHAIN to vendor/ unconditionally. Changes here:
# an existing vendor/djgpp is left alone and the toolchain directory is checked before it is linked.
#
# usage: DOS_TOOLCHAIN=/path/to/toolchain tools/setup-dos.sh
#
#   vendor/djgpp already present   nothing to do
#   DOS_TOOLCHAIN set              a directory holding djgpp/, dosbox/ and cwsdpmi/; vendor becomes a symbolic link to it
#                                  (CI points it at /opt/dos-toolchain in the pinned ci-dos image)
#   neither                        an error
#
# vendor/ is ignored by version control and never committed.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -d vendor/djgpp ]; then
  echo "setup-dos: vendor/ already provides the toolchain"
  exit 0
fi

if [ -n "${DOS_TOOLCHAIN:-}" ]; then
  for part in djgpp dosbox cwsdpmi; do
    [ -d "$DOS_TOOLCHAIN/$part" ] || {
      echo "setup-dos: DOS_TOOLCHAIN=$DOS_TOOLCHAIN has no $part/ directory" >&2
      exit 1
    }
  done
  if [ -e vendor ] || [ -L vendor ]; then
    echo "setup-dos: vendor exists but holds no djgpp/; remove it, then run this command again" >&2
    exit 1
  fi
  ln -s "$DOS_TOOLCHAIN" vendor
  echo "setup-dos: vendor -> $DOS_TOOLCHAIN"
  exit 0
fi

echo "setup-dos: no DOS toolchain. Set DOS_TOOLCHAIN to a directory containing djgpp/, dosbox/ and cwsdpmi/ (CI uses /opt/dos-toolchain)," >&2
echo "setup-dos: then run tools/setup-dos.sh again." >&2
exit 1
