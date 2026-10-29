// The pack container: header, directory, checksum, schema-derived compatibility number and section lookup.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inflate::{InflateError, inflate};

/// The first four bytes of every pack: `FAP` and 0x1A (which stops `TYPE` on DOS).
pub const MAGIC: [u8; 4] = *b"FAP\x1A";
/// The container version this crate reads and writes.
pub const FORMAT_VERSION: u16 = 1;
/// Header size in bytes.
pub const HEADER_LEN: usize = 32;
/// Directory entry size in bytes.
pub const ENTRY_LEN: usize = 20;
/// Sections start on multiples of this many bytes.
pub const ALIGN: usize = 16;

/// Header `target` byte: a pack baked for the Web build.
pub const TARGET_WEB: u8 = 1;
/// Header `target` byte: a pack baked for the DOS build.
pub const TARGET_DOS: u8 = 2;

/// Codec 0: the section is stored as it is decoded.
pub const CODEC_STORED: u8 = 0;
/// Codec 1: the section is a raw deflate (RFC 1951) stream.
pub const CODEC_DEFLATE: u8 = 1;

const fn tag(b: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*b)
}

/// Section tag: one level (`LevelView`).
pub const TAG_LEVL: u32 = tag(b"LEVL");
/// Section tag: sprites.
pub const TAG_SPRT: u32 = tag(b"SPRT");
/// Section tag: OPL3 register stream (effects or music).
pub const TAG_OPL3: u32 = tag(b"OPL3");
/// Section tag: OPL2 register stream.
pub const TAG_OPL2: u32 = tag(b"OPL2");
/// Section tag: PC speaker stream.
pub const TAG_SPKR: u32 = tag(b"SPKR");
/// Section tag: music metadata.
pub const TAG_MUSC: u32 = tag(b"MUSC");
/// Section tag: backdrops.
pub const TAG_BDRP: u32 = tag(b"BDRP");

/// FNV-1a 32 offset basis and prime.
const FNV_BASIS: u32 = 0x811C_9DC5;
const FNV_PRIME: u32 = 0x0100_0193;

/// Continues an FNV-1a 32 hash over `bytes`. `const` so the compatibility number is computed at compile time.
pub const fn fnv1a(mut h: u32, bytes: &[u8]) -> u32 {
    let mut i = 0;
    while i < bytes.len() {
        h = (h ^ bytes[i] as u32).wrapping_mul(FNV_PRIME);
        i += 1;
    }
    h
}

/// The FNV-1a 32 hash of `bytes`.
pub const fn fnv(bytes: &[u8]) -> u32 {
    fnv1a(FNV_BASIS, bytes)
}

const fn feed_u32(h: u32, v: u32) -> u32 {
    fnv1a(h, &v.to_le_bytes())
}

/// The hash of this crate's own schema. It feeds, in this order and as little-endian `u32` unless noted: the ASCII text
/// `fizz-pack-schema`, `FORMAT_VERSION`, `HEADER_LEN`, `ENTRY_LEN`, `ALIGN`, `LEVEL_HEADER_LEN`, `OBJECT_LEN`, the two codec numbers,
/// the seven section tags (`LEVL`, `SPRT`, `OPL3`, `OPL2`, `SPKR`, `MUSC`, `BDRP`) as their `u32` values, and the object kind
/// numbers `KIND_KEY_RED` to `KIND_ENEMY`. Anything that changes the bytes a reader must understand changes this number.
pub const SCHEMA_HASH: u32 = {
    let mut h = fnv(b"fizz-pack-schema");
    h = feed_u32(h, FORMAT_VERSION as u32);
    h = feed_u32(h, HEADER_LEN as u32);
    h = feed_u32(h, ENTRY_LEN as u32);
    h = feed_u32(h, ALIGN as u32);
    h = feed_u32(h, crate::level::LEVEL_HEADER_LEN as u32);
    h = feed_u32(h, crate::level::OBJECT_LEN as u32);
    h = feed_u32(h, CODEC_STORED as u32);
    h = feed_u32(h, CODEC_DEFLATE as u32);
    h = feed_u32(h, TAG_LEVL);
    h = feed_u32(h, TAG_SPRT);
    h = feed_u32(h, TAG_OPL3);
    h = feed_u32(h, TAG_OPL2);
    h = feed_u32(h, TAG_SPKR);
    h = feed_u32(h, TAG_MUSC);
    h = feed_u32(h, TAG_BDRP);
    h = feed_u32(h, crate::level::KIND_KEY_RED as u32);
    h = feed_u32(h, crate::level::KIND_KEY_BLUE as u32);
    h = feed_u32(h, crate::level::KIND_SNACK as u32);
    h = feed_u32(h, crate::level::KIND_CHICKEN as u32);
    h = feed_u32(h, crate::level::KIND_GADGET as u32);
    h = feed_u32(h, crate::level::KIND_SWITCH as u32);
    h = feed_u32(h, crate::level::KIND_ENEMY as u32);
    h
};

/// The compatibility number a pack carries in its header: `SCHEMA_HASH` continued over `content`, the bytes the game crate owns
/// (`fizz-ep2` feeds its tile ids and object enums as a fixed byte table). `compatibility(&[])` is the bare container schema.
pub const fn compatibility(content: &[u8]) -> u32 {
    fnv1a(SCHEMA_HASH, content)
}

/// Everything the container reader can refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Fewer bytes than a header, or than the header and directory need.
    Truncated,
    BadMagic,
    BadVersion,
    BadHeaderLen,
    /// The reserved header bytes are not zero.
    Reserved,
    /// The recorded total length differs from the length of the bytes given.
    LengthMismatch,
    BadChecksum,
    /// The pack's compatibility number is not the one the caller expects.
    Incompatible,
    /// A section ends past the end of the file, or starts inside the header and directory.
    OutOfRange,
    /// A section starts before the previous one ends (sections must be in ascending offset order).
    Overlap,
    /// A section offset that is not a multiple of 16.
    Misaligned,
    /// Directory entries are not in ascending tag order.
    Unsorted,
    /// A codec number other than 0 or 1, or a stored section whose two lengths differ.
    BadCodec,
    /// The output buffer is shorter than the section's decoded length.
    OutputTooSmall,
    /// The section's deflate stream is bad.
    Inflate(InflateError),
}

/// One directory entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub tag: u32,
    /// Byte offset of the stored bytes from the start of the file.
    pub offset: u32,
    pub stored_len: u32,
    pub decoded_len: u32,
    pub codec: u8,
    /// The section's own layout version (for example the `LEVL` layout).
    pub version: u8,
    /// Items in the section, for the reader's convenience (a `LEVL` section: its object count). Not verified by the container.
    pub count: u16,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The FNV-1a 32 checksum of a whole pack with the four checksum bytes (header offset 20) read as zero.
pub fn checksum(file: &[u8]) -> u32 {
    let head = file.get(..20).unwrap_or(file);
    let mut h = fnv1a(FNV_BASIS, head);
    if file.len() > 20 {
        h = fnv1a(h, &[0; 4]);
        h = fnv1a(h, file.get(24..).unwrap_or(&[]));
    }
    h
}

/// A validated pack held as bytes. `parse` checks everything the container can check on its own, so the accessors cannot panic.
#[derive(Clone, Copy, Debug)]
pub struct PackView<'a> {
    data: &'a [u8],
    target: u8,
    flags: u8,
    count: usize,
}

impl<'a> PackView<'a> {
    /// Validates `data` as a whole pack file: magic, version, header size, zero reserved bytes, total length equal to
    /// `data.len()`, checksum, `compat` equal to the header's compatibility number, then every directory entry (codec known, stored
    /// sections equal in both lengths, tags ascending, offsets 16-aligned, ascending and non-overlapping, inside the file and past the
    /// directory).
    pub fn parse(data: &'a [u8], compat: u32) -> Result<Self, Error> {
        if data.len() < HEADER_LEN {
            return Err(Error::Truncated);
        }
        if data[..4] != MAGIC {
            return Err(Error::BadMagic);
        }
        if u16_at(data, 4) != FORMAT_VERSION {
            return Err(Error::BadVersion);
        }
        if usize::from(u16_at(data, 6)) != HEADER_LEN {
            return Err(Error::BadHeaderLen);
        }
        if data[24..32].iter().any(|&b| b != 0) {
            return Err(Error::Reserved);
        }
        let total = u32_at(data, 16) as usize;
        if total > data.len() {
            return Err(Error::Truncated);
        }
        if total != data.len() {
            return Err(Error::LengthMismatch);
        }
        if checksum(data) != u32_at(data, 20) {
            return Err(Error::BadChecksum);
        }
        if u32_at(data, 12) != compat {
            return Err(Error::Incompatible);
        }
        let count = usize::from(u16_at(data, 10));
        let dir_end = HEADER_LEN + count * ENTRY_LEN;
        if dir_end > data.len() {
            return Err(Error::Truncated);
        }
        let pack = PackView {
            data,
            target: data[8],
            flags: data[9],
            count,
        };
        let (mut prev_end, mut prev_tag) = (dir_end, 0);
        for i in 0..count {
            let e = pack.entry(i);
            if e.tag < prev_tag {
                return Err(Error::Unsorted);
            }
            prev_tag = e.tag;
            if e.codec > CODEC_DEFLATE || (e.codec == CODEC_STORED && e.stored_len != e.decoded_len)
            {
                return Err(Error::BadCodec);
            }
            let start = e.offset as usize;
            if !start.is_multiple_of(ALIGN) {
                return Err(Error::Misaligned);
            }
            if start < dir_end {
                return Err(Error::OutOfRange);
            }
            if start < prev_end {
                return Err(Error::Overlap);
            }
            // Both are below 2^32, so the sum cannot wrap on a 64-bit host; on 32-bit targets `checked_add` guards it.
            let end = start
                .checked_add(e.stored_len as usize)
                .ok_or(Error::OutOfRange)?;
            if end > total {
                return Err(Error::OutOfRange);
            }
            prev_end = end;
        }
        Ok(pack)
    }

    /// The header's target byte (`TARGET_WEB` or `TARGET_DOS`).
    pub fn target(&self) -> u8 {
        self.target
    }

    /// The header's flags byte (no flag is defined in version 1).
    pub fn flags(&self) -> u8 {
        self.flags
    }

    /// The number of sections.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Directory entry `i` (panics if `i >= len()`).
    pub fn entry(&self, i: usize) -> Entry {
        let at = HEADER_LEN + i * ENTRY_LEN;
        let d = self.data;
        Entry {
            tag: u32_at(d, at),
            offset: u32_at(d, at + 4),
            stored_len: u32_at(d, at + 8),
            decoded_len: u32_at(d, at + 12),
            codec: d[at + 16],
            version: d[at + 17],
            count: u16_at(d, at + 18),
        }
    }

    /// All entries in directory order.
    pub fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        (0..self.count).map(|i| self.entry(i))
    }

    /// The `index`th (counting from 0, in directory order) section with `tag`.
    pub fn find(&self, tag: u32, index: usize) -> Option<Entry> {
        self.entries().filter(|e| e.tag == tag).nth(index)
    }

    /// How many sections carry `tag`.
    pub fn count_of(&self, tag: u32) -> usize {
        self.entries().filter(|e| e.tag == tag).count()
    }

    /// The section's stored (possibly compressed) bytes.
    pub fn stored(&self, e: &Entry) -> &'a [u8] {
        let start = e.offset as usize;
        &self.data[start..start + e.stored_len as usize]
    }

    /// For a stored section, its bytes in place with no copy; `None` for a compressed one.
    pub fn stored_in_place(&self, e: &Entry) -> Option<&'a [u8]> {
        (e.codec == CODEC_STORED).then(|| self.stored(e))
    }

    /// Decodes the section into the front of `out` and returns the decoded length. `out` must be at least
    /// `e.decoded_len` long; a deflate stream that disagrees with the recorded length (either way) is an error.
    pub fn decode(&self, e: &Entry, out: &mut [u8]) -> Result<usize, Error> {
        let n = e.decoded_len as usize;
        let dst = out.get_mut(..n).ok_or(Error::OutputTooSmall)?;
        if e.codec == CODEC_STORED {
            dst.copy_from_slice(self.stored(e));
        } else {
            inflate(self.stored(e), dst).map_err(Error::Inflate)?;
        }
        Ok(n)
    }
}
