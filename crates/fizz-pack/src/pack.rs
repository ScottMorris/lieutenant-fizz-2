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
pub const ENTRY_LEN: usize = 24;
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
/// Section tag: OPL3 register stream (one song).
pub const TAG_OPL3: u32 = tag(b"OPL3");
/// Section tag: OPL2 register stream (one song).
pub const TAG_OPL2: u32 = tag(b"OPL2");
/// Section tag: music metadata.
pub const TAG_MUSC: u32 = tag(b"MUSC");
/// Section tag: backdrops.
pub const TAG_BDRP: u32 = tag(b"BDRP");
/// Section tag: the effect set of one device (an index, then one stream per effect). One section per device, in the order
/// OPL3, OPL2, speaker.
pub const TAG_FXPK: u32 = tag(b"FXPK");

/// The audio section header (all device tags), in bytes.
pub const AUDIO_HEADER_LEN: usize = 16;
/// `loop_start` of a stream that plays once.
pub const NO_LOOP: u32 = 0xFFFF_FFFF;
/// The effect-set header, in bytes.
pub const FX_HEADER_LEN: usize = 8;
/// One effect-set index entry, in bytes.
pub const FX_ENTRY_LEN: usize = 16;
/// The default per-section decoded-size limit the baker enforces, in bytes.
pub const SECTION_LIMIT: u32 = 96 * 1024;

/// The three audio devices. Each has its own effect set (`FXPK`) and tick rate; the two OPL devices also have music files with a
/// section tag and a compatibility number of their own. The PC speaker plays effects only: there is no speaker music.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Opl3,
    Opl2,
    Speaker,
}

impl Device {
    /// The device number stored in an effect-set header: 0 OPL3, 1 OPL2, 2 speaker.
    pub const fn number(self) -> u8 {
        match self {
            Device::Opl3 => 0,
            Device::Opl2 => 1,
            Device::Speaker => 2,
        }
    }

    /// The tag of this device's song sections; `None` for the speaker, which has no music.
    pub const fn song_tag(self) -> Option<u32> {
        match self {
            Device::Opl3 => Some(TAG_OPL3),
            Device::Opl2 => Some(TAG_OPL2),
            Device::Speaker => None,
        }
    }

    /// The device's stream tick rate in hertz.
    pub const fn rate(self) -> u16 {
        match self {
            Device::Opl3 | Device::Opl2 => 560,
            Device::Speaker => 140,
        }
    }
}

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

/// The hash of the container: format version, header, directory and alignment sizes, and the codec numbers.
pub const CONTAINER_HASH: u32 = {
    let mut h = fnv(b"fizz-pack-container");
    h = feed_u32(h, FORMAT_VERSION as u32);
    h = feed_u32(h, HEADER_LEN as u32);
    h = feed_u32(h, ENTRY_LEN as u32);
    h = feed_u32(h, ALIGN as u32);
    h = feed_u32(h, CODEC_STORED as u32);
    h = feed_u32(h, CODEC_DEFLATE as u32);
    h
};

/// The hash of the non-audio sections: the `LEVL` header, object, rule and string sizes, the tags `LEVL`, `SPRT`, `MUSC` and `BDRP`,
/// the object kind numbers, and then the rule and string limits, the cue codes, the trigger, op and flag numbers and the string
/// charset extras (in the order of the `feed_u32` calls below).
pub const LEVEL_HASH: u32 = {
    let mut h = fnv(b"fizz-pack-level");
    h = feed_u32(h, crate::level::LEVEL_HEADER_LEN as u32);
    h = feed_u32(h, crate::level::OBJECT_LEN as u32);
    h = feed_u32(h, TAG_LEVL);
    h = feed_u32(h, TAG_SPRT);
    h = feed_u32(h, TAG_MUSC);
    h = feed_u32(h, TAG_BDRP);
    h = feed_u32(h, crate::level::KIND_KEY_RED as u32);
    h = feed_u32(h, crate::level::KIND_KEY_BLUE as u32);
    h = feed_u32(h, crate::level::KIND_SNACK as u32);
    h = feed_u32(h, crate::level::KIND_CHICKEN as u32);
    h = feed_u32(h, crate::level::KIND_GADGET as u32);
    h = feed_u32(h, crate::level::KIND_SWITCH as u32);
    h = feed_u32(h, crate::level::KIND_ENEMY as u32);
    // The rule and string records (amended into LEVL version 1 before any pack shipped).
    h = feed_u32(h, crate::level::RULE_LEN as u32);
    h = feed_u32(h, crate::level::STRING_LEN as u32);
    h = feed_u32(h, crate::level::MAX_RULES as u32);
    h = feed_u32(h, crate::level::MAX_STRINGS as u32);
    h = feed_u32(h, crate::level::STRING_TEXT_MAX as u32);
    h = feed_u32(h, crate::level::MIN_PERIOD);
    h = feed_u32(h, crate::level::STYLE_COUNT as u32);
    h = feed_u32(h, crate::level::CUE_NONE as u32);
    h = feed_u32(h, crate::level::CUE_LEVEL_BASE as u32);
    h = feed_u32(h, crate::level::TRIGGER_LOCK as u32);
    h = feed_u32(h, crate::level::TRIGGER_START as u32);
    h = feed_u32(h, crate::level::TRIGGER_ENTER as u32);
    h = feed_u32(h, crate::level::TRIGGER_TAKE as u32);
    h = feed_u32(h, crate::level::TRIGGER_SHOOT as u32);
    h = feed_u32(h, crate::level::TRIGGER_STOMP as u32);
    h = feed_u32(h, crate::level::TRIGGER_STUN as u32);
    h = feed_u32(h, crate::level::TRIGGER_TIME as u32);
    h = feed_u32(h, crate::level::OP_NONE as u32);
    h = feed_u32(h, crate::level::OP_ON as u32);
    h = feed_u32(h, crate::level::OP_OFF as u32);
    h = feed_u32(h, crate::level::OP_TOGGLE as u32);
    h = feed_u32(h, crate::level::FLAG_REPEAT as u32);
    h = feed_u32(h, crate::level::FLAG_GUARD as u32);
    h = feed_u32(h, crate::level::FLAG_GUARD_ON as u32);
    h = feed_u32(h, crate::level::SUBJECT_ANY as u32);
    let mut i = 0;
    while i < crate::level::STRING_EXTRAS.len() {
        h = feed_u32(h, crate::level::STRING_EXTRAS[i]);
        i += 1;
    }
    h
};

/// The hash of one device's audio layout: the audio header size, the no-loop marker, the device's song tag (0 for the speaker), number and tick rate, and
/// the effect-set tag, header and entry sizes. A change to one device's layout changes only that device's number.
pub const fn audio_hash(d: Device) -> u32 {
    let mut h = fnv(b"fizz-pack-audio");
    h = feed_u32(h, AUDIO_HEADER_LEN as u32);
    h = feed_u32(h, NO_LOOP);
    h = feed_u32(
        h,
        match d.song_tag() {
            Some(t) => t,
            None => 0,
        },
    );
    h = feed_u32(h, d.number() as u32);
    h = feed_u32(h, d.rate() as u32);
    h = feed_u32(h, TAG_FXPK);
    h = feed_u32(h, FX_HEADER_LEN as u32);
    h = feed_u32(h, FX_ENTRY_LEN as u32);
    h
}

/// The hash of the whole schema: `CONTAINER_HASH`, `LEVEL_HASH` and the three audio hashes, each as a little-endian `u32`, after the
/// ASCII text `fizz-pack-schema`. Anything that changes the bytes a reader of a world or shared pack must understand changes this
/// number.
pub const SCHEMA_HASH: u32 = {
    let mut h = fnv(b"fizz-pack-schema");
    h = feed_u32(h, CONTAINER_HASH);
    h = feed_u32(h, LEVEL_HASH);
    h = feed_u32(h, audio_hash(Device::Opl3));
    h = feed_u32(h, audio_hash(Device::Opl2));
    h = feed_u32(h, audio_hash(Device::Speaker));
    h
};

/// The compatibility number of a world or shared pack (`SHARED.FAP`, `LIGHT.FAP`): `SCHEMA_HASH` continued over `content`, the bytes
/// the game crate owns. `fizz-ep2` feeds, in order, its tile property table (flags, slope and switch channel for each of the 256 tile
/// ids), its named tile ids and its snack kind numbers (see `docs/FAP.md`). `compatibility(&[])` is the bare container schema.
pub const fn compatibility(content: &[u8]) -> u32 {
    fnv1a(SCHEMA_HASH, content)
}

/// The compatibility number of a music file (`MUSIC.OP3`, `MUSIC.OP2`) for device `d`: `CONTAINER_HASH` continued over that
/// device's `audio_hash`. It does not depend on levels, tiles or the other device, so a level change does not invalidate music.
/// `None` for the speaker, which has no music file.
pub const fn audio_compatibility(d: Device) -> Option<u32> {
    match d.song_tag() {
        Some(_) => Some(feed_u32(CONTAINER_HASH, audio_hash(d))),
        None => None,
    }
}

/// Everything the container reader can refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Fewer bytes than a header, or than the header and directory need, or a section's bytes are not all present.
    Truncated,
    BadMagic,
    BadVersion,
    BadHeaderLen,
    /// An unknown flag bit is set.
    BadFlags,
    /// The pack was baked for another target than the caller expects.
    WrongTarget,
    /// The header's stem is not valid, or is not the one the caller asked for (a renamed or misplaced file).
    Stem,
    /// The recorded total length differs from the length of the bytes given, or is not a multiple of 16.
    LengthMismatch,
    /// The whole-file checksum is wrong.
    BadChecksum,
    /// The pack's compatibility number is not the one the caller expects.
    Incompatible,
    /// A section ends past the end of the file, or starts inside the header and directory.
    OutOfRange,
    /// A section starts before the previous one ends (sections are in ascending offset order).
    Overlap,
    /// A section offset that is not a multiple of 16.
    Misaligned,
    /// Directory entries are not in ascending tag order (bytewise).
    Unsorted,
    /// A codec number other than 0 or 1, or a stored section whose two lengths differ.
    BadCodec,
    /// A section's stored bytes do not match the checksum in its directory entry.
    SectionChecksum,
    /// A section's decoded length is over the limit the caller set.
    TooLarge,
    /// The output buffer is shorter than the section's decoded length.
    OutputTooSmall,
    /// The section's deflate stream is bad.
    Inflate(InflateError),
}

/// What a caller expects of a pack file; anything else is refused by `PackView::parse`.
#[derive(Clone, Copy, Debug)]
pub struct Expect<'a> {
    /// `TARGET_WEB` or `TARGET_DOS`.
    pub target: u8,
    /// The pack stem, for example `"SHARED"` for `SHARED.FAP` (upper case, 1 to 8 bytes).
    pub stem: &'a str,
    /// The compatibility number the caller was built with (`compatibility(..)` or `audio_compatibility(..)`).
    pub compat: u32,
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
    /// The records in the section: objects in a level, records in an audio stream, effects in an effect set, sprites in a sprite
    /// section; 0 for a section without records. The container does not verify it; each section reader does.
    pub count: u16,
    /// FNV-1a 32 of the section's stored bytes.
    pub checksum: u32,
}

impl Entry {
    /// A placeholder is present in the directory and empty (decoded length 0, count 0): an asset with no form for this target.
    pub fn is_placeholder(&self) -> bool {
        self.decoded_len == 0 && self.count == 0
    }

    /// Checks `stored` against this entry's stored length and checksum. A loader that read one section from a large file calls this
    /// on the bytes it read, without reading the rest of the file.
    pub fn verify_stored(&self, stored: &[u8]) -> Result<(), Error> {
        if stored.len() != self.stored_len as usize {
            return Err(Error::Truncated);
        }
        if fnv(stored) != self.checksum {
            return Err(Error::SectionChecksum);
        }
        Ok(())
    }

    /// Verifies `stored` (see `verify_stored`) and decodes it into the front of `out`, returning the decoded length. `out` must be at
    /// least `decoded_len` long; a deflate stream that disagrees with the recorded length (either way) is an error.
    pub fn decode_stored(&self, stored: &[u8], out: &mut [u8]) -> Result<usize, Error> {
        self.verify_stored(stored)?;
        let n = self.decoded_len as usize;
        let dst = out.get_mut(..n).ok_or(Error::OutputTooSmall)?;
        if self.codec == CODEC_STORED {
            dst.copy_from_slice(stored);
        } else {
            inflate(stored, dst).map_err(Error::Inflate)?;
        }
        Ok(n)
    }
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

/// Encodes a pack stem for the header: ASCII, space-padded to 8 bytes. Panics if the stem is empty, over 8 bytes or not made of
/// printable ASCII without spaces (callers pass literals; a reader never calls this).
pub const fn stem_bytes(stem: &str) -> [u8; 8] {
    let b = stem.as_bytes();
    assert!(!b.is_empty() && b.len() <= 8);
    let mut out = [b' '; 8];
    let mut i = 0;
    while i < b.len() {
        assert!(b[i] > 0x20 && b[i] < 0x7F);
        out[i] = b[i];
        i += 1;
    }
    out
}

/// Whether the header's 8 stem bytes spell `want`: the stem, then spaces or zero bytes to the end, with nothing else after it.
fn stem_matches(field: &[u8], want: &str) -> bool {
    let w = want.as_bytes();
    if w.is_empty() || w.len() > 8 || field.len() != 8 {
        return false;
    }
    let is_pad = |b: u8| b == b' ' || b == 0;
    field[..w.len()] == *w && field[w.len()..].iter().all(|&b| is_pad(b))
}

/// A validated pack held as bytes. `parse` checks everything the container can check on its own, so the accessors cannot panic.
/// A view made by `parse_head` holds only the header and directory, so the section accessors then report `Truncated`.
#[derive(Clone, Copy, Debug)]
pub struct PackView<'a> {
    data: &'a [u8],
    target: u8,
    count: usize,
}

impl<'a> PackView<'a> {
    /// Validates `data` as a whole pack file. In this order: magic, version, header size, flags, target, stem, compatibility number,
    /// total length (equal to `data.len()` and a multiple of 16), the whole-file checksum, then every directory entry (codec known,
    /// stored sections equal in both lengths, tags ascending, offsets 16-aligned, ascending, non-overlapping, inside the file and past
    /// the directory). Section checksums are checked when a section is read, not here.
    pub fn parse(data: &'a [u8], want: &Expect) -> Result<Self, Error> {
        let pack = Self::head(data, data.len() as u64, want, true)?;
        if checksum(data) != u32_at(data, 20) {
            return Err(Error::BadChecksum);
        }
        pack.check_directory()?;
        Ok(pack)
    }

    /// Validates the header and directory only, for a loader that reads one section of a big file (a song from `MUSIC.OP3`). `head`
    /// is the start of the file (at least the header and directory) and `file_len` its real length. The whole-file checksum is not
    /// checked; the loader reads a section's stored bytes and calls `Entry::decode_stored`, which checks that section's checksum.
    pub fn parse_head(head: &'a [u8], file_len: u64, want: &Expect) -> Result<Self, Error> {
        let pack = Self::head(head, file_len, want, false)?;
        pack.check_directory()?;
        Ok(pack)
    }

    fn head(data: &'a [u8], file_len: u64, want: &Expect, whole: bool) -> Result<Self, Error> {
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
        if data[9] != 0 {
            return Err(Error::BadFlags);
        }
        if data[8] != want.target {
            return Err(Error::WrongTarget);
        }
        if !stem_matches(&data[24..32], want.stem) {
            return Err(Error::Stem);
        }
        if u32_at(data, 12) != want.compat {
            return Err(Error::Incompatible);
        }
        let total = u64::from(u32_at(data, 16));
        if whole && total > file_len {
            return Err(Error::Truncated);
        }
        if total != file_len || !total.is_multiple_of(ALIGN as u64) {
            return Err(Error::LengthMismatch);
        }
        let count = usize::from(u16_at(data, 10));
        if HEADER_LEN + count * ENTRY_LEN > data.len() {
            return Err(Error::Truncated);
        }
        Ok(PackView {
            data,
            target: data[8],
            count,
        })
    }

    fn check_directory(&self) -> Result<(), Error> {
        let total = u64::from(u32_at(self.data, 16));
        let dir_end = HEADER_LEN + self.count * ENTRY_LEN;
        let (mut prev_end, mut prev_tag) = (dir_end as u64, 0u32);
        for i in 0..self.count {
            let e = self.entry(i);
            // Tags sort bytewise, which is the big-endian order of the little-endian `u32` the tag is held as.
            let key = e.tag.swap_bytes();
            if key < prev_tag {
                return Err(Error::Unsorted);
            }
            prev_tag = key;
            if e.codec > CODEC_DEFLATE || (e.codec == CODEC_STORED && e.stored_len != e.decoded_len)
            {
                return Err(Error::BadCodec);
            }
            let start = u64::from(e.offset);
            if !start.is_multiple_of(ALIGN as u64) {
                return Err(Error::Misaligned);
            }
            if start < dir_end as u64 {
                return Err(Error::OutOfRange);
            }
            if start < prev_end {
                return Err(Error::Overlap);
            }
            let end = start + u64::from(e.stored_len);
            if end > total {
                return Err(Error::OutOfRange);
            }
            prev_end = end;
        }
        Ok(())
    }

    /// The header's target byte (`TARGET_WEB` or `TARGET_DOS`).
    pub fn target(&self) -> u8 {
        self.target
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
            checksum: u32_at(d, at + 20),
        }
    }

    /// All entries in directory order.
    pub fn entries(&self) -> impl Iterator<Item = Entry> + '_ {
        (0..self.count).map(|i| self.entry(i))
    }

    /// The `index`th (counting from 0, in directory order) section with `tag`. A placeholder is returned like any other entry; test
    /// it with `Entry::is_placeholder`.
    pub fn find(&self, tag: u32, index: usize) -> Option<Entry> {
        self.entries().filter(|e| e.tag == tag).nth(index)
    }

    /// How many sections carry `tag`.
    pub fn count_of(&self, tag: u32) -> usize {
        self.entries().filter(|e| e.tag == tag).count()
    }

    /// Refuses a pack with a section whose decoded length is over `limit` (the per-section limit, `SECTION_LIMIT` by default).
    pub fn check_limit(&self, limit: u32) -> Result<(), Error> {
        match self.entries().any(|e| e.decoded_len > limit) {
            true => Err(Error::TooLarge),
            false => Ok(()),
        }
    }

    /// The section's stored (possibly compressed) bytes, after checking them against the entry's checksum. `Truncated` when this
    /// view holds only the head of the file.
    pub fn stored(&self, e: &Entry) -> Result<&'a [u8], Error> {
        let start = e.offset as usize;
        let bytes = start
            .checked_add(e.stored_len as usize)
            .and_then(|end| self.data.get(start..end))
            .ok_or(Error::Truncated)?;
        e.verify_stored(bytes)?;
        Ok(bytes)
    }

    /// For a stored section, its bytes in place with no copy (checksum checked); `Ok(None)` for a compressed one.
    pub fn stored_in_place(&self, e: &Entry) -> Result<Option<&'a [u8]>, Error> {
        match e.codec {
            CODEC_STORED => self.stored(e).map(Some),
            _ => Ok(None),
        }
    }

    /// Decodes the section into the front of `out` and returns the decoded length, after checking its checksum.
    pub fn decode(&self, e: &Entry, out: &mut [u8]) -> Result<usize, Error> {
        e.decode_stored(self.stored(e)?, out)
    }
}
