// The `LEVL` section layout and its zero-copy reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Little-endian. Offsets are from the start of the decoded section. The layout is the one in `docs/FAP.md`.
//
//   0  u16 w            2  u16 h             4  u32 initial switches
//   8  u8 floor_solid   9  u8 name length   10  u16 object count
//  12  f64 spawn x     20  f64 spawn y      28  u8 rule count   29  u8 string count   30  u16 zero
//  32  the name (printable ASCII, at most 48 bytes, no terminator), zero-padded to a multiple of 8
//   .  object records, 32 bytes each, in construction order
//   .  rule records, 32 bytes each (at most 32), then string records, 32 bytes each (at most 32)
//   .  w * h tile bytes, in `TileMap` order (bottom row first), ready for `copy_from_slice`
//
// An object record: 0 u8 kind, 1 u8 sub (the snack kind for a snack, else zero), 2 u16 flags (zero), 4 u32 channel, 8 f64 x, 16 f64
// y, 24 f64 p (direction or parameter). Coordinates are `f64` bit patterns so the baked level has exactly the bits of the source
// numbers.
//
// A rule record: 0 u8 trigger, 1 u8 flags, 2 u8 guard channel, 3 u8 subject, 4 union (8 bytes: a rect x0 y0 x1 y1 as four u16, or a
// time `at` u32 and `every` u32), 12 u16 delay, 14 u8 op, 15 u8 channel, 16 u16 cell x, 18 u16 cell y (slot A, the channel action),
// 20 u8 effect, 21 u8 caption code, 22 u16 cell x, 24 u16 cell y (slot B, the cue), 26 six reserved zero bytes.
//
// A string record: 0 u8 length (1 to 30), 1 u8 style, 2 thirty bytes of UTF-8 text, zero-padded.

pub const LEVEL_HEADER_LEN: usize = 32;
pub const OBJECT_LEN: usize = 32;
pub const RULE_LEN: usize = 32;
pub const STRING_LEN: usize = 32;
/// The most rules and the most strings in one level (the sim keeps one bit per rule in a `u32`).
pub const MAX_RULES: usize = 32;
pub const MAX_STRINGS: usize = 32;
/// The most text bytes in one string record.
pub const STRING_TEXT_MAX: usize = 30;
/// The shortest repeat period of a `time` rule, in ticks.
pub const MIN_PERIOD: u32 = 6;
/// The number of caption styles: white, cyan, red, green, blue, yellow, grey.
pub const STYLE_COUNT: u8 = 7;
/// The characters a string may hold besides U+0020 to U+007E: middle dot, en dash, em dash, curly quotes and the ellipsis.
pub const STRING_EXTRAS: [u32; 8] = [
    0x00B7, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026,
];

pub const TRIGGER_LOCK: u8 = 1;
pub const TRIGGER_START: u8 = 2;
pub const TRIGGER_ENTER: u8 = 3;
pub const TRIGGER_TAKE: u8 = 4;
pub const TRIGGER_SHOOT: u8 = 5;
pub const TRIGGER_STOMP: u8 = 6;
pub const TRIGGER_STUN: u8 = 7;
pub const TRIGGER_TIME: u8 = 8;
// Triggers 9 (touch) and 10 (hit) are reserved for the gadget milestone and refused.

pub const OP_NONE: u8 = 0;
pub const OP_ON: u8 = 1;
pub const OP_OFF: u8 = 2;
pub const OP_TOGGLE: u8 = 3;

/// Rule flags.
pub const FLAG_REPEAT: u8 = 1;
pub const FLAG_GUARD: u8 = 2;
pub const FLAG_GUARD_ON: u8 = 4;

/// A rule `subject` of "any chicken" (stomp and stun).
pub const SUBJECT_ANY: u8 = 0xFF;
/// A slot B effect or caption code that means "none".
pub const CUE_NONE: u8 = 0xFF;
/// Caption codes below this are shared caption ids; `CUE_LEVEL_BASE + i` is the level's string `i`.
pub const CUE_LEVEL_BASE: u8 = 0x40;

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
    /// `floor_solid` is not 0 or 1, or the reserved word or an object's `flags` is not zero.
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
    /// A rule count over 32, or a rule record that breaks a rule (see `docs/FAP.md`).
    BadRule,
    /// A string count over 32, or a string record that breaks a rule (length, UTF-8, charset, padding, style).
    BadString,
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

/// One rule record, with the union kept as its two raw words (`rect()` and `time()` read it as either form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleRec {
    pub trigger: u8,
    pub flags: u8,
    pub guard_ch: u8,
    pub subject: u8,
    /// Bytes 4 to 7 of the union: `x0` and `y0` (low and high `u16`) for a rect, `at` for a time rule.
    pub u0: u32,
    /// Bytes 8 to 11 of the union: `x1` and `y1` for a rect, `every` for a time rule.
    pub u1: u32,
    pub delay: u16,
    pub op: u8,
    pub channel: u8,
    pub cell_a: (u16, u16),
    pub effect: u8,
    pub caption: u8,
    pub cell_b: (u16, u16),
}

impl RuleRec {
    /// A rule with no trigger and empty slots (slot B holds the none codes); callers fill in what they need.
    pub const EMPTY: RuleRec = RuleRec {
        trigger: 0,
        flags: 0,
        guard_ch: 0,
        subject: 0,
        u0: 0,
        u1: 0,
        delay: 0,
        op: OP_NONE,
        channel: 0,
        cell_a: (0, 0),
        effect: CUE_NONE,
        caption: CUE_NONE,
        cell_b: (0, 0),
    };

    /// The union as a rect `(x0, y0, x1, y1)`, both corners included.
    pub fn rect(&self) -> (u16, u16, u16, u16) {
        (
            self.u0 as u16,
            (self.u0 >> 16) as u16,
            self.u1 as u16,
            (self.u1 >> 16) as u16,
        )
    }
    /// The two union words for a rect.
    pub fn union_rect(x0: u16, y0: u16, x1: u16, y1: u16) -> (u32, u32) {
        (
            u32::from(x0) | u32::from(y0) << 16,
            u32::from(x1) | u32::from(y1) << 16,
        )
    }
    /// The union as `(at, every)` ticks.
    pub fn time(&self) -> (u32, u32) {
        (self.u0, self.u1)
    }
    pub fn repeat(&self) -> bool {
        self.flags & FLAG_REPEAT != 0
    }
    pub fn guard(&self) -> bool {
        self.flags & FLAG_GUARD != 0
    }
    pub fn guard_on(&self) -> bool {
        self.flags & FLAG_GUARD_ON != 0
    }
    /// Whether slot B holds a cue (an effect or a caption).
    pub fn has_cue(&self) -> bool {
        self.effect != CUE_NONE || self.caption != CUE_NONE
    }

    /// The 32-byte record.
    pub fn to_bytes(&self) -> [u8; RULE_LEN] {
        let mut b = [0u8; RULE_LEN];
        b[0] = self.trigger;
        b[1] = self.flags;
        b[2] = self.guard_ch;
        b[3] = self.subject;
        b[4..8].copy_from_slice(&self.u0.to_le_bytes());
        b[8..12].copy_from_slice(&self.u1.to_le_bytes());
        b[12..14].copy_from_slice(&self.delay.to_le_bytes());
        b[14] = self.op;
        b[15] = self.channel;
        b[16..18].copy_from_slice(&self.cell_a.0.to_le_bytes());
        b[18..20].copy_from_slice(&self.cell_a.1.to_le_bytes());
        b[20] = self.effect;
        b[21] = self.caption;
        b[22..24].copy_from_slice(&self.cell_b.0.to_le_bytes());
        b[24..26].copy_from_slice(&self.cell_b.1.to_le_bytes());
        b
    }

    /// Reads the first 32 bytes of `b` (panics if shorter). The reserved bytes 26 to 31 are not kept; `LevelView::new` checks them.
    pub fn from_bytes(b: &[u8]) -> RuleRec {
        RuleRec {
            trigger: b[0],
            flags: b[1],
            guard_ch: b[2],
            subject: b[3],
            u0: u32_at(b, 4),
            u1: u32_at(b, 8),
            delay: u16_at(b, 12),
            op: b[14],
            channel: b[15],
            cell_a: (u16_at(b, 16), u16_at(b, 18)),
            effect: b[20],
            caption: b[21],
            cell_b: (u16_at(b, 22), u16_at(b, 24)),
        }
    }
}

/// One string record: a caption a level carries for its rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringRec {
    /// Bytes of UTF-8 in `text`, 1 to 30.
    pub len: u8,
    /// A style number below `STYLE_COUNT`.
    pub style: u8,
    /// The text, zero-padded.
    pub text: [u8; STRING_TEXT_MAX],
}

impl StringRec {
    /// A record for `text` (panics if it is empty or over 30 bytes; the charset is checked by `LevelView::new`).
    pub fn new(style: u8, text: &str) -> StringRec {
        assert!(!text.is_empty() && text.len() <= STRING_TEXT_MAX);
        let mut t = [0u8; STRING_TEXT_MAX];
        t[..text.len()].copy_from_slice(text.as_bytes());
        StringRec {
            len: text.len() as u8,
            style,
            text: t,
        }
    }
    /// The text bytes (`len` of them).
    pub fn bytes(&self) -> &[u8] {
        &self.text[..usize::from(self.len).min(STRING_TEXT_MAX)]
    }
    /// The text, or an empty string if it is not UTF-8 (`LevelView::new` has already refused that).
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.bytes()).unwrap_or("")
    }
    /// The 32-byte record.
    pub fn to_bytes(&self) -> [u8; STRING_LEN] {
        let mut b = [0u8; STRING_LEN];
        b[0] = self.len;
        b[1] = self.style;
        b[2..].copy_from_slice(&self.text);
        b
    }
    /// Reads the first 32 bytes of `b` (panics if shorter).
    pub fn from_bytes(b: &[u8]) -> StringRec {
        let mut t = [0u8; STRING_TEXT_MAX];
        t.copy_from_slice(&b[2..STRING_LEN]);
        StringRec {
            len: b[0],
            style: b[1],
            text: t,
        }
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
    rules: usize,
    strings: usize,
    objects_at: usize,
    rules_at: usize,
    strings_at: usize,
    tiles_at: usize,
}

impl<'a> LevelView<'a> {
    /// Checks that `data` is exactly one well-formed `LEVL` section: the sizes in the header account for every byte, the name and
    /// reserved fields are clean, and every position is inside the map.
    pub fn new(data: &'a [u8]) -> Result<Self, LevelError> {
        if data.len() < LEVEL_HEADER_LEN {
            return Err(LevelError::Truncated);
        }
        if data[8] > 1 || u16_at(data, 30) != 0 {
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
        let (rules, strings) = (usize::from(data[28]), usize::from(data[29]));
        if rules > MAX_RULES {
            return Err(LevelError::BadRule);
        }
        if strings > MAX_STRINGS {
            return Err(LevelError::BadString);
        }
        let objects = usize::from(u16_at(data, 10));
        let objects_at = LEVEL_HEADER_LEN + padded_name_len(name_len);
        let rules_at = objects_at + objects * OBJECT_LEN;
        let strings_at = rules_at + rules * RULE_LEN;
        let tiles_at = strings_at + strings * STRING_LEN;
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
            rules,
            strings,
            objects_at,
            rules_at,
            strings_at,
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
        for i in 0..strings {
            view.check_string(i)?;
        }
        for i in 0..rules {
            view.check_rule(i)?;
        }
        Ok(view)
    }

    fn check_string(&self, i: usize) -> Result<(), LevelError> {
        let at = self.strings_at + i * STRING_LEN;
        let b = &self.data[at..at + STRING_LEN];
        let len = usize::from(b[0]);
        if len == 0 || len > STRING_TEXT_MAX || b[1] >= STYLE_COUNT {
            return Err(LevelError::BadString);
        }
        if b[2 + len..].iter().any(|&x| x != 0) {
            return Err(LevelError::BadString);
        }
        let text = core::str::from_utf8(&b[2..2 + len]).map_err(|_| LevelError::BadString)?;
        for c in text.chars() {
            let u = u32::from(c);
            if !(0x20..=0x7E).contains(&u) && !STRING_EXTRAS.contains(&u) {
                return Err(LevelError::BadString);
            }
        }
        Ok(())
    }

    fn check_rule(&self, i: usize) -> Result<(), LevelError> {
        let bad = Err(LevelError::BadRule);
        let at = self.rules_at + i * RULE_LEN;
        if self.data[at + 26..at + RULE_LEN].iter().any(|&x| x != 0) {
            return bad;
        }
        let r = self.rule(i);
        let (w, h) = (u32::from(self.w), u32::from(self.h));
        let in_map = |c: (u16, u16)| u32::from(c.0) < w && u32::from(c.1) < h;
        if r.trigger < TRIGGER_LOCK || r.trigger > TRIGGER_TIME {
            return bad;
        }
        if r.flags & !(FLAG_REPEAT | FLAG_GUARD | FLAG_GUARD_ON) != 0
            || (r.guard_on() && !r.guard())
        {
            return bad;
        }
        // The guard channel is a channel number when there is a guard and zero when there is not.
        if (r.guard() && r.guard_ch >= 32) || (!r.guard() && r.guard_ch != 0) {
            return bad;
        }
        let kind_of = |s: u8| {
            if usize::from(s) < self.objects {
                self.object(usize::from(s)).kind
            } else {
                0
            }
        };
        let subject_ok = match r.trigger {
            TRIGGER_LOCK => r.subject != SUBJECT_ANY,
            TRIGGER_TAKE => matches!(
                kind_of(r.subject),
                KIND_KEY_RED | KIND_KEY_BLUE | KIND_SNACK
            ),
            TRIGGER_STOMP | TRIGGER_STUN => {
                r.subject == SUBJECT_ANY || matches!(kind_of(r.subject), KIND_CHICKEN | KIND_ENEMY)
            }
            _ => r.subject == 0,
        };
        if !subject_ok {
            return bad;
        }
        match r.trigger {
            TRIGGER_ENTER | TRIGGER_SHOOT => {
                let (x0, y0, x1, y1) = r.rect();
                if x0 > x1 || y0 > y1 || !in_map((x1, y1)) {
                    return bad;
                }
            }
            TRIGGER_TIME => {
                let (at_tick, every) = r.time();
                if at_tick < 1 || (every != 0 && every < MIN_PERIOD) || r.repeat() != (every != 0) {
                    return bad;
                }
            }
            _ => {
                if r.u0 != 0 || r.u1 != 0 {
                    return bad;
                }
            }
        }
        // Slot A: the channel action.
        if r.op > OP_TOGGLE {
            return bad;
        }
        if r.op == OP_NONE {
            if r.channel != 0 || r.cell_a != (0, 0) {
                return bad;
            }
        } else if r.channel >= 32 || !in_map(r.cell_a) {
            return bad;
        }
        // Slot B: the cue. A caption code is none, a shared id, or one of this level's strings.
        let caption_ok = r.caption == CUE_NONE
            || r.caption < CUE_LEVEL_BASE
            || (r.caption >= CUE_LEVEL_BASE
                && usize::from(r.caption - CUE_LEVEL_BASE) < self.strings);
        if !caption_ok {
            return bad;
        }
        if r.has_cue() {
            if !in_map(r.cell_b) {
                return bad;
            }
        } else if r.cell_b != (0, 0) {
            return bad;
        }
        // A lock opens (switches off) its door channel at once, once; it has no guard and does not repeat.
        if r.trigger == TRIGGER_LOCK && (r.op != OP_OFF || r.delay != 0 || r.guard() || r.repeat())
        {
            return bad;
        }
        Ok(())
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
    pub fn rule_count(&self) -> usize {
        self.rules
    }
    /// Rule `i` in file order (panics if `i >= rule_count()`).
    pub fn rule(&self, i: usize) -> RuleRec {
        assert!(i < self.rules);
        RuleRec::from_bytes(&self.data[self.rules_at + i * RULE_LEN..])
    }
    pub fn rules(&self) -> impl Iterator<Item = RuleRec> + '_ {
        (0..self.rules).map(|i| self.rule(i))
    }
    pub fn string_count(&self) -> usize {
        self.strings
    }
    /// String `i` in file order (panics if `i >= string_count()`).
    pub fn string(&self, i: usize) -> StringRec {
        assert!(i < self.strings);
        StringRec::from_bytes(&self.data[self.strings_at + i * STRING_LEN..])
    }
    pub fn strings(&self) -> impl Iterator<Item = StringRec> + '_ {
        (0..self.strings).map(|i| self.string(i))
    }
    /// The `w * h` tile bytes.
    pub fn tiles(&self) -> &'a [u8] {
        &self.data[self.tiles_at..]
    }
}
