// The `LEVL` section layout and its zero-copy reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Little-endian. Offsets are from the start of the decoded section. The layout is the one in `docs/FAP.md`.
//
//   0  u16 w            2  u16 h             4  u32 initial switches
//   8  u8 floor_solid   9  u8 name length   10  u16 object count
//  12  f64 spawn x     20  f64 spawn y      28  u32 zero
//  32  the name (printable ASCII, at most 48 bytes, no terminator), zero-padded to a multiple of 8
//   .  object records, 32 bytes each, in construction order
//   .  w * h tile bytes, in `TileMap` order (bottom row first), ready for `copy_from_slice`
//
// An object record: 0 u8 kind, 1 u8 sub (the snack kind for a snack, else zero), 2 u16 flags (zero), 4 u32 channel, 8 f64 x, 16 f64
// y, 24 f64 p (direction or parameter). Coordinates are `f64` bit patterns so the baked level has exactly the bits of the source
// numbers.

pub const LEVEL_HEADER_LEN: usize = 32;
pub const OBJECT_LEN: usize = 32;

pub const KIND_KEY_RED: u8 = 1;
pub const KIND_KEY_BLUE: u8 = 2;
pub const KIND_SNACK: u8 = 3;
pub const KIND_CHICKEN: u8 = 4;
pub const KIND_GADGET: u8 = 5;
pub const KIND_SWITCH: u8 = 6;
pub const KIND_ENEMY: u8 = 7;

/// The longest level name, in bytes.
pub const NAME_MAX: usize = 48;

/// Why a `LEVL` section was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelError {
    /// Shorter than the header, or than the header, name, objects and tiles it describes.
    Truncated,
    /// Longer than the header, name, objects and tiles it describes.
    TrailingBytes,
    /// `floor_solid` is not 0 or 1, or a reserved word or an object's `flags` is not zero.
    BadField,
    /// A width or height of 0.
    BadSize,
    /// A name over 48 bytes or with a byte outside 0x20 to 0x7E, or padding that is not zero.
    BadName,
    /// An object `kind` of 0 or over 7, or a `sub` other than 0 on an object that is not a snack.
    BadKind,
    /// A spawn or object centre outside the map, or a non-finite `p`.
    BadPosition,
    /// The object count differs from the count in the directory entry.
    CountMismatch,
}

/// One object record. Coordinates are kept as `f64` bits; use `x()`, `y()` and `p()` for the numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectRec {
    pub kind: u8,
    pub sub: u8,
    pub flags: u16,
    pub channel: u32,
    pub x_bits: u64,
    pub y_bits: u64,
    pub p_bits: u64,
}

impl ObjectRec {
    pub fn x(&self) -> f64 {
        f64::from_bits(self.x_bits)
    }
    pub fn y(&self) -> f64 {
        f64::from_bits(self.y_bits)
    }
    pub fn p(&self) -> f64 {
        f64::from_bits(self.p_bits)
    }
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u64_at(b: &[u8], at: usize) -> u64 {
    let mut a = [0; 8];
    a.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(a)
}

/// Rounds a name length up to the stored, 8-byte-padded length.
pub const fn padded_name_len(n: usize) -> usize {
    (n + 7) & !7
}

/// Whether `(x, y)` is a centre inside a `w` by `h` map: finite, `0 <= x < w` and `0 <= y < h` (a NaN fails both tests).
fn inside(x: f64, y: f64, w: u16, h: u16) -> bool {
    x >= 0.0 && x < f64::from(w) && y >= 0.0 && y < f64::from(h)
}

/// A validated `LEVL` section (already decoded).
#[derive(Clone, Copy, Debug)]
pub struct LevelView<'a> {
    data: &'a [u8],
    w: u16,
    h: u16,
    objects: usize,
    objects_at: usize,
    tiles_at: usize,
}

impl<'a> LevelView<'a> {
    /// Checks that `data` is exactly one well-formed `LEVL` section: the sizes in the header account for every byte, the name and
    /// reserved fields are clean, and every position is inside the map.
    pub fn new(data: &'a [u8]) -> Result<Self, LevelError> {
        if data.len() < LEVEL_HEADER_LEN {
            return Err(LevelError::Truncated);
        }
        if data[8] > 1 || u32_at(data, 28) != 0 {
            return Err(LevelError::BadField);
        }
        let (w, h) = (u16_at(data, 0), u16_at(data, 2));
        if w == 0 || h == 0 {
            return Err(LevelError::BadSize);
        }
        let name_len = usize::from(data[9]);
        if name_len > NAME_MAX {
            return Err(LevelError::BadName);
        }
        let objects = usize::from(u16_at(data, 10));
        let objects_at = LEVEL_HEADER_LEN + padded_name_len(name_len);
        let tiles_at = objects_at + objects * OBJECT_LEN;
        // At most 48 + 7 + 32 + 65535 * 32 + 65535 * 65535 bytes, which fits in a u64 and is far past any real section.
        let end = tiles_at as u64 + u64::from(w) * u64::from(h);
        if end > data.len() as u64 {
            return Err(LevelError::Truncated);
        }
        if end < data.len() as u64 {
            return Err(LevelError::TrailingBytes);
        }
        let name = &data[LEVEL_HEADER_LEN..objects_at];
        if name[..name_len]
            .iter()
            .any(|&b| !(0x20..=0x7E).contains(&b))
            || name[name_len..].iter().any(|&b| b != 0)
        {
            return Err(LevelError::BadName);
        }
        let view = LevelView {
            data,
            w,
            h,
            objects,
            objects_at,
            tiles_at,
        };
        let (sx, sy) = view.spawn();
        if !inside(sx, sy, w, h) {
            return Err(LevelError::BadPosition);
        }
        for o in view.objects() {
            if o.kind == 0 || o.kind > KIND_ENEMY || (o.kind != KIND_SNACK && o.sub != 0) {
                return Err(LevelError::BadKind);
            }
            if o.flags != 0 {
                return Err(LevelError::BadField);
            }
            if !inside(o.x(), o.y(), w, h) || (o.p_bits >> 52) & 0x7FF == 0x7FF {
                return Err(LevelError::BadPosition);
            }
        }
        Ok(view)
    }

    /// `new`, and also checks the object count against the `count` of the section's directory entry.
    pub fn with_count(data: &'a [u8], count: u16) -> Result<Self, LevelError> {
        let view = Self::new(data)?;
        if view.objects != usize::from(count) {
            return Err(LevelError::CountMismatch);
        }
        Ok(view)
    }

    pub fn width(&self) -> usize {
        usize::from(self.w)
    }
    pub fn height(&self) -> usize {
        usize::from(self.h)
    }
    /// The initial switch bits.
    pub fn switches(&self) -> u32 {
        u32_at(self.data, 4)
    }
    pub fn floor_solid(&self) -> bool {
        self.data[8] == 1
    }
    pub fn spawn_bits(&self) -> (u64, u64) {
        (u64_at(self.data, 12), u64_at(self.data, 20))
    }
    pub fn spawn(&self) -> (f64, f64) {
        let (x, y) = self.spawn_bits();
        (f64::from_bits(x), f64::from_bits(y))
    }
    /// The level's name bytes (printable ASCII, checked by `new`).
    pub fn name(&self) -> &'a [u8] {
        &self.data[LEVEL_HEADER_LEN..LEVEL_HEADER_LEN + usize::from(self.data[9])]
    }
    pub fn object_count(&self) -> usize {
        self.objects
    }
    /// Object `i` in file order (panics if `i >= object_count()`).
    pub fn object(&self, i: usize) -> ObjectRec {
        assert!(i < self.objects);
        let at = self.objects_at + i * OBJECT_LEN;
        let d = self.data;
        ObjectRec {
            kind: d[at],
            sub: d[at + 1],
            flags: u16_at(d, at + 2),
            channel: u32_at(d, at + 4),
            x_bits: u64_at(d, at + 8),
            y_bits: u64_at(d, at + 16),
            p_bits: u64_at(d, at + 24),
        }
    }
    pub fn objects(&self) -> impl Iterator<Item = ObjectRec> + '_ {
        (0..self.objects).map(|i| self.object(i))
    }
    /// The `w * h` tile bytes.
    pub fn tiles(&self) -> &'a [u8] {
        &self.data[self.tiles_at..]
    }
}
