#!/usr/bin/env python3
# Encodes the slice level as LEVL version 2 and computes its digest, independently of the Rust code that pins both.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I tools/levl2-slice.py [--bytes out.levl]
# It reads the level's rows from crates/fizz-ep2/src/level.rs and holds the rest of the slice (objects, states, bindings, rules) as
# literals written from docs/FAP.md. It prints the decoded length (the worked example says 2,080), the FNV-1a 32 of the section bytes
# and the level digest, which `fizz_ep2::level::digest` and the tests in `crates/fizz-ep2/src/level.rs` pin.
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TILES = {".": 0, "w": 30, "#": 1, "B": 2, "=": 3, "^": 4, "D": 12, "b": 13, "E": 17}
NAME = b"Lighthouse Point"
SPAWN = (2.0, 2.0)
W, H = 32, 48


def rows():
    src = (ROOT / "crates/fizz-ep2/src/level.rs").read_text()
    found = re.findall(r'^    "([.wB#=^DbE]{32})",\s*// \d+$', src, re.M)
    assert len(found) == H, len(found)
    return found


def tile_bytes():
    out = bytearray(W * H)
    for r, row in enumerate(rows()):
        y = H - 1 - r
        for x, c in enumerate(row):
            out[y * W + x] = TILES[c]
    return bytes(out)


def f64(x):
    return struct.pack("<d", x)


def obj(kind, sub, x, y, p=0.0, flags=0, aux=0):
    return struct.pack("<BBHI", kind, sub, flags, aux) + f64(x) + f64(y) + f64(p)


# Red key, blue key, eight snacks (cheezie 0, chocolate 1, cookie 2, soda can 3, the provisional elixir 4, last), the chicken.
OBJECTS = [
    obj(1, 0, 2.5, 11.7),
    obj(2, 0, 24.5, 42.7),
    obj(3, 0, 25.5, 40.2),
    obj(3, 0, 27.5, 40.2),
    obj(3, 0, 27.5, 42.7),
    obj(3, 1, 26.5, 42.7),
    obj(3, 2, 25.5, 42.7),
    obj(3, 3, 28.5, 38.7),
    obj(3, 3, 28.5, 42.7),
    obj(3, 4, 13.5, 8.7),
    obj(4, 0, 8.0, 11.0, p=-1.0),
]
STATES = [bytes([2, 0, 0, 0, 0, 0, 0, 0])] * 2


def binding(state, mask, rect):
    return struct.pack("<BBBBHH4H", 1, state, mask, 0, 0xFFFF, 0, *rect)


BINDINGS = [binding(0, 0b10, (15, 20, 15, 22)), binding(1, 0b10, (22, 38, 22, 40))]


def lock(rect, state, key):
    head = struct.pack("<BBHHBB", 9, 0, 0, 0xFFFF, 0, 0) + struct.pack("<4H", *rect)
    conds = bytes([1, state, 0b01, 0]) + bytes([2, key, 0, 0])
    set_state = bytes([1, state, 1, 0]) + bytes(8)
    spend = bytes([4, key, 0, 0]) + bytes(8)
    return head + conds + set_state + spend


RULES = [lock((15, 20, 15, 22), 0, 0), lock((22, 38, 22, 40), 1, 1)]


def levl():
    name = NAME + bytes((-len(NAME)) % 8)
    head = struct.pack("<HHBBH", W, H, 0, len(NAME), len(OBJECTS)) + f64(SPAWN[0]) + f64(SPAWN[1])
    head += bytes([len(STATES), len(BINDINGS), len(RULES), 0]) + bytes(4)
    assert len(head) == 32
    body = b"".join(STATES) + b"".join(BINDINGS) + b"".join(OBJECTS) + b"".join(RULES)
    return head + name + body + tile_bytes()


def fnv32(data):
    h = 0x811C9DC5
    for b in data:
        h = ((h ^ b) * 0x01000193) & 0xFFFFFFFF
    return h


def digest():
    # w, h as u16; tiles; floor_solid; spawn; states (count, records); bindings (count, records); objects as kind, sub, flags, aux,
    # x, y, p (the record bytes are exactly that); rules (count, records); strings (count, records). The name is not included.
    d = struct.pack("<HH", W, H) + tile_bytes() + bytes([0]) + f64(SPAWN[0]) + f64(SPAWN[1])
    d += bytes([len(STATES)]) + b"".join(STATES)
    d += bytes([len(BINDINGS)]) + b"".join(BINDINGS)
    d += b"".join(OBJECTS)
    d += bytes([len(RULES)]) + b"".join(RULES)
    d += bytes([0])
    h = 0xCBF29CE484222325
    for b in d:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


if __name__ == "__main__":
    data = levl()
    print("LEVL bytes   %d" % len(data))
    print("LEVL fnv32   0x%08X" % fnv32(data))
    print("digest       0x%016x" % digest())
    if len(sys.argv) == 3 and sys.argv[1] == "--bytes":
        Path(sys.argv[2]).write_bytes(data)
