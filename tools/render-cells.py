#!/usr/bin/env python3
# Renders the check screen's text cells to PNG pictures with a VGA font, for the manual and the archive.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/render-cells.py <input> <output-dir> [--font <psf-file>] [--scale <n>] [--only <text>]
#
#   <input>       either the golden file (crates/fizz-machine/tests/check-cells.txt) or the output of `FIZZ2.EXE /CHECKCELLS`
#                 (the `CELLROW` lines; one picture, named check.png)
#   <output-dir>  where the PNG files go: one for each `== name` case in a golden file, named from the case (01-i486-vga-sb16...png)
#   --font        a PSF2 console font, 8x16, with a Unicode table (the default is the first of the Linux console fonts
#                 `default8x16`, `lat2-16` found under /usr/share/kbd/consolefonts or /usr/share/consolefonts). The font file is not
#                 part of this repository: it is read from the machine that runs the tool
#   --scale       whole-number pixel scale (default 2: 1280x800, a 4:3 picture needs 1 x 1.2 non-square pixels, which this does not do)
#   --only        render only the golden cases whose name contains this text
#
# A cell is a character and an attribute: foreground in the low nibble, background in the next three bits, drawn with the 16 standard
# VGA text colours. The cells an adapter holds in video memory have the character first, then the attribute; the golden file has them
# as text lines then attribute lines. Python's standard library only (zlib writes the PNG).
import gzip
import os
import re
import struct
import sys
import zlib

COLS, ROWS = 80, 25
GLYPH_W, GLYPH_H = 8, 16

# The sixteen text colours as VGA's DAC holds them (6 bits each, scaled to 8).
PALETTE = [
    (0, 0, 0), (0, 0, 170), (0, 170, 0), (0, 170, 170), (170, 0, 0), (170, 0, 170), (170, 85, 0), (170, 170, 170),
    (85, 85, 85), (85, 85, 255), (85, 255, 85), (85, 255, 255), (255, 85, 85), (255, 85, 255), (255, 255, 85), (255, 255, 255),
]  # fmt: skip

# What code page 437 draws for bytes 0x00-0x1F and 0x7F (the others are the cp437 codec's).
LOW = "\u0000☺☻♥♦♣♠•◘○◙♂♀♪♫☼►◄↕‼¶§▬↨↑↓→←∟↔▲▼"
CANDIDATE_FONTS = [
    "/usr/share/kbd/consolefonts/default8x16.psfu.gz",
    "/usr/share/consolefonts/Lat15-VGA16.psf.gz",
    "/usr/share/kbd/consolefonts/lat2-16.psfu.gz",
]


def cp437_char(byte):
    if byte < 0x20:
        return LOW[byte]
    if byte == 0x7F:
        return "⌂"
    return bytes([byte]).decode("cp437")


def load_font(path):
    """Returns {character: 16 bytes} from a PSF2 font with a Unicode table."""
    data = open(path, "rb").read()
    if data[:2] == b"\x1f\x8b":
        data = gzip.decompress(data)
    if data[:4] != b"\x72\xb5\x4a\x86":
        raise SystemExit(f"render-cells: {path} is not a PSF2 font")
    _, header, flags, length, charsize, height, width = struct.unpack("<7I", data[4:32])
    if (width, height) != (GLYPH_W, GLYPH_H) or charsize != GLYPH_H:
        raise SystemExit(f"render-cells: {path} is {width}x{height}, not 8x16")
    glyphs = [data[header + i * charsize : header + (i + 1) * charsize] for i in range(length)]
    if not flags & 1:
        raise SystemExit(f"render-cells: {path} has no Unicode table")
    font = {}
    table = data[header + length * charsize :]
    for index, entry in enumerate(table.split(b"\xff")[:length]):
        # Sequences (after 0xFE) are combining forms; only the single code points before them map a character.
        for piece in entry.split(b"\xfe")[:1]:
            for ch in piece.decode("utf-8", errors="ignore"):
                font.setdefault(ch, glyphs[index])
    return font


def find_font(explicit):
    for path in [explicit] if explicit else CANDIDATE_FONTS:
        if path and os.path.exists(path):
            return load_font(path)
    raise SystemExit("render-cells: no VGA console font found; pass --font <psf2 file>")


def write_png(path, width, height, rgb):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + rgb[y * width * 3 : (y + 1) * width * 3] for y in range(height))
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(png)


def render(cells, font, scale):
    """cells: 25 rows of 80 (character, attribute). Returns (width, height, RGB bytes)."""
    width, height = COLS * GLYPH_W * scale, ROWS * GLYPH_H * scale
    out = bytearray(width * height * 3)
    blank = bytes(GLYPH_H)
    for row in range(ROWS):
        for col in range(COLS):
            ch, attr = cells[row][col]
            glyph = font.get(ch, font.get("?", blank))
            fg, bg = PALETTE[attr & 0x0F], PALETTE[(attr >> 4) & 0x07]
            for gy in range(GLYPH_H):
                line = bytearray()
                for gx in range(GLYPH_W):
                    colour = fg if glyph[gy] & (0x80 >> gx) else bg
                    line += bytes(colour) * scale
                for sy in range(scale):
                    y = (row * GLYPH_H + gy) * scale + sy
                    start = (y * width + col * GLYPH_W * scale) * 3
                    out[start : start + len(line)] = line
    return width, height, bytes(out)


def parse_golden(text):
    """Yields (name, cells) for each `== name` case: 25 text lines, then 25 lines of two hex digits for each cell."""
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        if lines[i].startswith("== "):
            name = lines[i][3:]
            body, attrs = lines[i + 1 : i + 26], lines[i + 26 : i + 51]
            cells = [[(body[r][c], int(attrs[r][2 * c : 2 * c + 2], 16)) for c in range(COLS)] for r in range(ROWS)]
            yield name, cells
            i += 51
        else:
            i += 1


def parse_cellrows(text):
    rows = {}
    for line in text.splitlines():
        m = re.match(r"CELLROW (\d\d) ([0-9A-Fa-f]{%d})\s*$" % (COLS * 4), line.replace("\r", ""))
        if m:
            raw = bytes.fromhex(m.group(2))
            rows[int(m.group(1))] = [(cp437_char(raw[2 * c]), raw[2 * c + 1]) for c in range(COLS)]
    if sorted(rows) != list(range(ROWS)):
        raise SystemExit("render-cells: the input has no complete set of 25 CELLROW lines")
    return [rows[r] for r in range(ROWS)]


def slug(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")[:60]


def main(argv):
    args, opts = [], {}
    it = iter(argv)
    for a in it:
        if a in ("--font", "--scale", "--only"):
            opts[a] = next(it, "")
        else:
            args.append(a)
    if len(args) != 2:
        sys.exit(__doc__ or "usage: render-cells.py <input> <output-dir>")
    scale = int(opts.get("--scale", "2"))
    font = find_font(opts.get("--font"))
    text = open(args[0], encoding="utf-8").read()
    os.makedirs(args[1], exist_ok=True)
    if "CELLROW " in text and "\n== " not in "\n" + text:
        jobs = [("check", parse_cellrows(text))]
    else:
        only = opts.get("--only")
        jobs = [(f"{n:02d}-{slug(name)}", cells) for n, (name, cells) in enumerate(parse_golden(text), 1) if not only or only in name]
    for name, cells in jobs:
        path = os.path.join(args[1], name + ".png")
        write_png(path, *render(cells, font, scale))
        print(path)


if __name__ == "__main__":
    main(sys.argv[1:])
