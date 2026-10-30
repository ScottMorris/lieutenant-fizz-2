// The pack writer, the `LEVL` encoder and a small deterministic deflate compressor (feature `write`, needs `alloc`).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The compressor is deliberately simple: stored blocks, or one fixed-Huffman block found by a greedy hash-chain matcher with
// fixed parameters (32 KB window, 64 candidates per position, no lazy matching). It is enough to test the container and the decoder
// and to give small sections some compression. `fizz-bake` will carry the better encoder (dynamic Huffman, optimal parsing) later;
// the container does not care which encoder made a stream. Every function here is a pure function of its input, so the bytes are
// reproducible.

use crate::level::{
    LEVEL_HEADER_LEN, MAX_RULES, MAX_STRINGS, NAME_MAX, OBJECT_LEN, ObjectRec, RULE_LEN, RuleRec,
    STRING_LEN, StringRec, padded_name_len,
};
use crate::pack::{
    ALIGN, CODEC_DEFLATE, CODEC_STORED, ENTRY_LEN, FORMAT_VERSION, HEADER_LEN, MAGIC, checksum,
    fnv, stem_bytes,
};
use alloc::vec;
use alloc::vec::Vec;

// ---------------------------------------------------------------------------------------------------------------------------------
// Deflate

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, value: u32, bits: u32) {
        self.acc |= value << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Huffman codes go out most significant bit first, so they are bit-reversed into the least-significant-first stream.
    fn code(&mut self, code: u32, bits: u32) {
        self.put(code.reverse_bits() >> (32 - bits), bits);
    }

    fn lit(&mut self, sym: u32) {
        match sym {
            0..=143 => self.code(0x30 + sym, 8),
            144..=255 => self.code(0x190 + sym - 144, 9),
            256..=279 => self.code(sym - 256, 7),
            _ => self.code(0xC0 + sym - 280, 8),
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const WINDOW: usize = 32768;
const CHAIN: usize = 64;
const HASH_BITS: u32 = 15;

fn hash3(d: &[u8], i: usize) -> usize {
    let v = u32::from(d[i]) | u32::from(d[i + 1]) << 8 | u32::from(d[i + 2]) << 16;
    (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
}

/// Raw deflate as stored blocks only (at most 65535 bytes each). An empty input is one empty final stored block.
pub fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 5 * (data.len() / 65535 + 1));
    let mut chunks = data.chunks(65535).peekable();
    if chunks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(c) = chunks.next() {
        out.push(u8::from(chunks.peek().is_none()));
        out.extend_from_slice(&(c.len() as u16).to_le_bytes());
        out.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        out.extend_from_slice(c);
    }
    out
}

/// Raw deflate as one fixed-Huffman block with a greedy hash-chain matcher (see the module comment).
pub fn deflate_fixed(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter {
        out: Vec::new(),
        acc: 0,
        n: 0,
    };
    w.put(1, 1); // final block
    w.put(1, 2); // fixed Huffman
    let n = data.len();
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; n];
    let mut i = 0;
    let insert = |head: &mut Vec<usize>, prev: &mut Vec<usize>, at: usize| {
        if at + 3 <= n {
            let h = hash3(data, at);
            prev[at] = head[h];
            head[h] = at;
        }
    };
    while i < n {
        let (mut best_len, mut best_dist) = (0, 0);
        if i + 3 <= n {
            let mut cand = head[hash3(data, i)];
            let mut tries = 0;
            while cand != usize::MAX && i - cand <= WINDOW && tries < CHAIN {
                let max = (n - i).min(258);
                let mut l = 0;
                while l < max && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    (best_len, best_dist) = (l, i - cand);
                    if l == max {
                        break;
                    }
                }
                cand = prev[cand];
                tries += 1;
            }
        }
        if best_len >= 3 {
            let s = LEN_BASE
                .iter()
                .rposition(|&b| usize::from(b) <= best_len)
                .unwrap_or(0);
            w.lit(257 + s as u32);
            w.put(
                (best_len - usize::from(LEN_BASE[s])) as u32,
                u32::from(LEN_EXTRA[s]),
            );
            let d = DIST_BASE
                .iter()
                .rposition(|&b| usize::from(b) <= best_dist)
                .unwrap_or(0);
            w.code(d as u32, 5);
            w.put(
                (best_dist - usize::from(DIST_BASE[d])) as u32,
                u32::from(DIST_EXTRA[d]),
            );
            for k in i..i + best_len {
                insert(&mut head, &mut prev, k);
            }
            i += best_len;
        } else {
            w.lit(u32::from(data[i]));
            insert(&mut head, &mut prev, i);
            i += 1;
        }
    }
    w.lit(256);
    w.finish()
}

/// The smaller of `deflate_fixed` and `deflate_stored` (the stored one on a tie).
pub fn deflate(data: &[u8]) -> Vec<u8> {
    let stored = deflate_stored(data);
    let fixed = deflate_fixed(data);
    if fixed.len() < stored.len() {
        fixed
    } else {
        stored
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Pack writer

struct Section {
    tag: u32,
    codec: u8,
    version: u8,
    count: u16,
    stored: Vec<u8>,
    decoded_len: u32,
}

/// Builds a pack. Sections are written in a stable order sorted by tag (bytewise tag order; insertion order within a tag, which is the section's index),
/// each on a 16-byte boundary with zero padding, and the header carries the total length and checksum. Nothing depends on time,
/// addresses or hash order, so the same calls give the same bytes.
pub struct PackWriter {
    target: u8,
    flags: u8,
    compat: u32,
    stem: [u8; 8],
    sections: Vec<Section>,
}

impl PackWriter {
    /// A writer for `target` (`TARGET_WEB` or `TARGET_DOS`) whose header carries compatibility number `compat` and pack stem `stem`
    /// (for example `"SHARED"`).
    pub fn new(target: u8, compat: u32, stem: &str) -> Self {
        PackWriter {
            target,
            flags: 0,
            compat,
            stem: stem_bytes(stem),
            sections: Vec::new(),
        }
    }

    /// Adds a section stored as it is. `count` is the section's item count (see `Entry::count`).
    pub fn add_stored(&mut self, tag: u32, version: u8, count: u16, data: &[u8]) -> &mut Self {
        self.add_raw(
            tag,
            CODEC_STORED,
            version,
            count,
            data.to_vec(),
            data.len() as u32,
        )
    }

    /// Adds a placeholder: present in the directory and empty (decoded length 0, count 0).
    pub fn add_placeholder(&mut self, tag: u32, version: u8) -> &mut Self {
        self.add_stored(tag, version, 0, &[])
    }

    /// Adds a section compressed with `deflate` when that is smaller than the data, else stored.
    pub fn add_compressed(&mut self, tag: u32, version: u8, count: u16, data: &[u8]) -> &mut Self {
        let packed = deflate(data);
        if packed.len() < data.len() {
            self.add_raw(
                tag,
                CODEC_DEFLATE,
                version,
                count,
                packed,
                data.len() as u32,
            )
        } else {
            self.add_stored(tag, version, count, data)
        }
    }

    /// Adds a section from bytes already encoded for `codec`, trusting `decoded_len` (tests use this to build lies).
    pub fn add_raw(
        &mut self,
        tag: u32,
        codec: u8,
        version: u8,
        count: u16,
        stored: Vec<u8>,
        decoded_len: u32,
    ) -> &mut Self {
        self.sections.push(Section {
            tag,
            codec,
            version,
            count,
            stored,
            decoded_len,
        });
        self
    }

    /// Lays out and returns the pack file.
    pub fn finish(mut self) -> Vec<u8> {
        self.sections.sort_by_key(|s| s.tag.swap_bytes()); // bytewise tag order; stable
        let dir_end = HEADER_LEN + self.sections.len() * ENTRY_LEN;
        let mut offsets = Vec::new();
        let mut at = dir_end;
        for s in &self.sections {
            at = at.next_multiple_of(ALIGN);
            offsets.push(at);
            at += s.stored.len();
        }
        let at = at.next_multiple_of(ALIGN); // the file length is a multiple of 16
        let mut f = vec![0u8; at];
        f[..4].copy_from_slice(&MAGIC);
        f[4..6].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        f[6..8].copy_from_slice(&(HEADER_LEN as u16).to_le_bytes());
        f[8] = self.target;
        f[9] = self.flags;
        f[10..12].copy_from_slice(&(self.sections.len() as u16).to_le_bytes());
        f[12..16].copy_from_slice(&self.compat.to_le_bytes());
        f[16..20].copy_from_slice(&(at as u32).to_le_bytes());
        f[24..32].copy_from_slice(&self.stem);
        for (i, (s, &off)) in self.sections.iter().zip(&offsets).enumerate() {
            let e = HEADER_LEN + i * ENTRY_LEN;
            f[e..e + 4].copy_from_slice(&s.tag.to_le_bytes());
            f[e + 4..e + 8].copy_from_slice(&(off as u32).to_le_bytes());
            f[e + 8..e + 12].copy_from_slice(&(s.stored.len() as u32).to_le_bytes());
            f[e + 12..e + 16].copy_from_slice(&s.decoded_len.to_le_bytes());
            f[e + 16] = s.codec;
            f[e + 17] = s.version;
            f[e + 18..e + 20].copy_from_slice(&s.count.to_le_bytes());
            f[e + 20..e + 24].copy_from_slice(&fnv(&s.stored).to_le_bytes());
            f[off..off + s.stored.len()].copy_from_slice(&s.stored);
        }
        seal(&mut f);
        f
    }
}

/// Recomputes and stores the checksum of a whole pack file (after a test has edited its bytes).
pub fn seal(file: &mut [u8]) {
    let c = checksum(file);
    file[20..24].copy_from_slice(&c.to_le_bytes());
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Level encoder

/// The content of one `LEVL` section.
pub struct LevelSpec<'a> {
    pub width: u16,
    pub height: u16,
    pub switches: u32,
    pub floor_solid: bool,
    pub spawn_x_bits: u64,
    pub spawn_y_bits: u64,
    /// At most 48 bytes of printable ASCII.
    pub name: &'a str,
    pub objects: &'a [ObjectRec],
    /// At most 32 rules; empty for a level without rules.
    pub rules: &'a [RuleRec],
    /// At most 32 strings; empty for a level without strings.
    pub strings: &'a [StringRec],
    /// Exactly `width * height` bytes.
    pub tiles: &'a [u8],
}

/// Encodes a `LEVL` section (panics if the name is over 48 bytes, there are more than 32 rules or strings, or the tile count is not
/// `width * height`). It does not validate the records; `LevelView::new` does.
pub fn encode_level(l: &LevelSpec) -> Vec<u8> {
    assert!(l.name.len() <= NAME_MAX && l.objects.len() <= 65535);
    assert!(l.rules.len() <= MAX_RULES && l.strings.len() <= MAX_STRINGS);
    assert_eq!(l.tiles.len(), usize::from(l.width) * usize::from(l.height));
    let mut v = Vec::with_capacity(
        LEVEL_HEADER_LEN
            + 256
            + l.objects.len() * OBJECT_LEN
            + l.rules.len() * RULE_LEN
            + l.strings.len() * STRING_LEN
            + l.tiles.len(),
    );
    v.extend_from_slice(&l.width.to_le_bytes());
    v.extend_from_slice(&l.height.to_le_bytes());
    v.extend_from_slice(&l.switches.to_le_bytes());
    v.push(u8::from(l.floor_solid));
    v.push(l.name.len() as u8);
    v.extend_from_slice(&(l.objects.len() as u16).to_le_bytes());
    v.extend_from_slice(&l.spawn_x_bits.to_le_bytes());
    v.extend_from_slice(&l.spawn_y_bits.to_le_bytes());
    v.push(l.rules.len() as u8);
    v.push(l.strings.len() as u8);
    v.extend_from_slice(&[0; 2]);
    v.extend_from_slice(l.name.as_bytes());
    v.resize(LEVEL_HEADER_LEN + padded_name_len(l.name.len()), 0);
    for o in l.objects {
        v.push(o.kind);
        v.push(o.sub);
        v.extend_from_slice(&o.flags.to_le_bytes());
        v.extend_from_slice(&o.channel.to_le_bytes());
        v.extend_from_slice(&o.x_bits.to_le_bytes());
        v.extend_from_slice(&o.y_bits.to_le_bytes());
        v.extend_from_slice(&o.p_bits.to_le_bytes());
    }
    for r in l.rules {
        v.extend_from_slice(&r.to_bytes());
    }
    for s in l.strings {
        v.extend_from_slice(&s.to_bytes());
    }
    v.extend_from_slice(l.tiles);
    v
}
