#!/usr/bin/env python3
# Recomputes LEVEL_HASH and SCHEMA_HASH independently of the Rust consts, from the list in docs/FAP.md ("Compatibility numbers and hashes").
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: python3 -I crates/fizz-pack/tests/levl_hash.py
# The pins in tests/pack.rs (PIN_LEVEL, PIN_SCHEMA) must equal the two numbers printed here. CONTAINER_HASH and the three audio hashes
# are the pinned values in tests/pack.rs; the LEVL 2 change does not move them.
import struct

BASIS, PRIME = 0x811C9DC5, 0x01000193


def fnv(h, data):
    for b in data:
        h = ((h ^ b) * PRIME) & 0xFFFFFFFF
    return h


def u32s(h, *values):
    for v in values:
        h = fnv(h, struct.pack("<I", v))
    return h


def tag(t):
    return struct.unpack("<I", t)[0]


def level_hash():
    h = fnv(BASIS, b"fizz-pack-level")
    h = u32s(h, 2)  # LEVL section version
    h = u32s(h, 32, 8, 16, 32, 48, 4, 12, 32)  # header, state, binding, object, rule, condition, action, string
    h = u32s(h, tag(b"LEVL"), tag(b"SPRT"), tag(b"MUSC"), tag(b"BDRP"))
    # MAX_STATES, MAX_STATE_VALUES, MAX_BINDINGS, MAX_DOOR_BINDINGS, MAX_RULES, MAX_STRINGS, MAX_GROUPS, STRING_TEXT_MAX, MIN_PERIOD
    h = u32s(h, 32, 8, 64, 32, 32, 32, 16, 30, 6)
    h = u32s(h, 7)  # STYLE_COUNT
    h = u32s(h, *range(1, 11))  # object kinds 1 to 10
    h = u32s(h, 1, 2, 4, 1)  # switch HIT_SHOT, HIT_STOMP, USE; light ATTACHED
    h = u32s(h, 1, 2, 3, 4)  # binding kinds
    h = u32s(h, *range(2, 16))  # triggers 2 to 15
    h = u32s(h, *range(1, 7))  # condition kinds 1 to 6
    h = u32s(h, *range(1, 10))  # action ops 1 to 9
    h = u32s(h, 1, 1)  # REPEAT, state WORLD
    h = u32s(h, 0x40, 0x80, 0x100, 0xFFFF, 0xFF)  # caption bases, cue base, any subject, any change
    h = u32s(h, 0x00B7, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026)
    return h


def schema_hash(level, container=0x6F16BA34, audio=(0xB8991606, 0x8D24DCC8, 0x5A8096D0)):
    return u32s(fnv(BASIS, b"fizz-pack-schema"), container, level, *audio)


if __name__ == "__main__":
    # A self-check of this script's arithmetic: the schema number before LEVL 2 follows from the level number before it.
    assert schema_hash(0x76AA52BB) == 0x25E81944
    lh = level_hash()
    print("LEVEL_HASH  0x%08X" % lh)
    print("SCHEMA_HASH 0x%08X" % schema_hash(lh))
