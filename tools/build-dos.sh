#!/bin/bash
# Builds the DOS programs with no C compiler and no C library, and packs them into DOS executables in dist/dos/.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Provenance: adapted from Desk spikes/d0-rust-dos/build.sh and fizz-2-dos-poc1 tools/build-dos.sh. Changes here: the workspace is dos/,
# the package list is an argument, and the solo (CWSDSTUB) variant is dropped.
#
# usage: tools/build-dos.sh [package...]      (after tools/setup-dos.sh)
#
#   package   a package of the dos/ workspace; its binary becomes dist/dos/<PACKAGE>.EXE in upper case. Default: every program.
#
#   dos/ (Rust, no_std)        --cargo -Zbuild-std, target dos-elf.json, linked by rust-lld with link.ld--> ELF executables
#   the ELF + the DJGPP stub   --tools/elf2go32.py-----------------------------------------------------> dist/dos/NAME.EXE
#   vendor/cwsdpmi                                                                                       --> dist/dos/CWSDPMI.EXE
#
# The DJGPP pieces still used are data, not a toolchain: the stub (written out by `stubify -g`). CWSDPMI is the DPMI host the
# executables load when none is running, and must sit beside them. File names are 8.3 upper case.
set -euo pipefail
cd "$(dirname "$0")/.."

# Every program of the dos/ workspace; a new one is added here with its package.
ALL_PROGRAMS="rtsmoke fizz2"
PROGRAMS="${*:-$ALL_PROGRAMS}"

OUT=dist/dos
STUBIFY="$PWD/vendor/djgpp/djgpp/i586-pc-msdosdjgpp/bin/stubify"
[ -x "$STUBIFY" ] || { echo "build-dos: DJGPP not found in vendor/; run tools/setup-dos.sh" >&2; exit 1; }
[ -f vendor/cwsdpmi/bin/CWSDPMI.EXE ] || { echo "build-dos: CWSDPMI not found in vendor/; run tools/setup-dos.sh" >&2; exit 1; }

mkdir -p "$OUT/obj"

# 1. Rust: cargo runs from dos/ because the linker script path in .cargo/config.toml is relative to it.
pkgs=()
for p in $PROGRAMS; do pkgs+=(-p "$p"); done
(cd dos && RUSTC_BOOTSTRAP=1 cargo build --release --locked "${pkgs[@]}")

# 2. The stub. `stubify -g` writes DJGPP's STUB.EXE (2 KiB) and does nothing else.
rm -f "$OUT/obj/stub.exe"
"$STUBIFY" -g "$OUT/obj/stub.exe" > /dev/null

# 3. Pack. The unit of work is a byte shuffle: tools/elf2go32.py reads the linked ELF and writes stub + go32 COFF image.
for p in $PROGRAMS; do
  name="$(echo "$p" | tr '[:lower:]' '[:upper:]')"
  [ "${#name}" -le 8 ] || { echo "build-dos: $name is not an 8.3 file name" >&2; exit 1; }
  python3 -I tools/elf2go32.py "$OUT/obj/stub.exe" "dos/target/dos-elf/release/$p" "$OUT/$name.EXE"
  echo "build-dos: $OUT/$name.EXE ($(wc -c < "$OUT/$name.EXE") bytes)"
done

# 4. The DPMI host the executables load when none is running. It must sit beside them.
cp vendor/cwsdpmi/bin/CWSDPMI.EXE "$OUT/CWSDPMI.EXE"
