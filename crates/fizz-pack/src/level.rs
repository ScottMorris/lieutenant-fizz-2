// The `LEVL` section (version 2) layout and its zero-copy reader.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Little-endian. Offsets are from the start of the decoded section. The layout is the one in `docs/FAP.md`, which is the source of
// truth; this comment is only a map of it. Version 1 is dropped: no pack shipped, so there is no reader for it.
//
//   0  u16 w            2  u16 h             4  u8 floor_solid    5  u8 name length (at most 48)    6  u16 object count
//   8  f64 spawn x     16  f64 spawn y      24  u8 state count   25  u8 binding count   26  u8 rule count   27  u8 string count
//  28  four reserved zero bytes
//  32  the name (printable ASCII, no terminator), zero-padded to a multiple of 8
//   .  state records (8 bytes), binding records (16), object records (32), rule records (48), string records (32)
//   .  w * h tile bytes, in `TileMap` order (bottom row first), ready for `copy_from_slice`
//
// A state: 0 u8 values (2 to 8), 1 u8 start, 2 u8 flags (zero), 3..8 reserved.
// A binding: 0 u8 kind, 1 u8 state, 2 u8 mask (the active set of values), 3 reserved, 4 u16 target (a light's object, else 0xFFFF),
// 6..8 reserved, 8 the rect as four u16 (a door), else zero.
// An object: 0 u8 kind, 1 u8 sub, 2 u16 flags, 4 u32 aux, 8 f64 x, 16 f64 y, 24 f64 p. Coordinates are `f64` bit patterns so the
// baked level has exactly the bits of the source numbers.
// A rule: 0 u8 trigger, 1 u8 flags, 2 u16 delay, 4 u16 subject, 6 u8 arg, 7 reserved, 8 the union (a rect as four u16, or a time
// `at` and `every` as two u32), 16 and 20 two conditions (4 bytes), 24 and 36 two actions (12 bytes).
// A condition: 0 u8 kind, 1 u8 a, 2 u8 b, 3 reserved. An action: 0 u8 op, 1 u8 a, 2 u8 b, 3 reserved, 4 i32 n, 8 u16 x, 10 u16 y.
// A string: 0 u8 length (1 to 30), 1 u8 style, 2 thirty bytes of UTF-8 text, zero-padded.

pub const LEVEL_HEADER_LEN: usize = 32;
pub const STATE_LEN: usize = 8;
pub const BINDING_LEN: usize = 16;
pub const OBJECT_LEN: usize = 32;
pub const RULE_LEN: usize = 48;
pub const CONDITION_LEN: usize = 4;
pub const ACTION_LEN: usize = 12;
pub const STRING_LEN: usize = 32;

/// The `LEVL` section version a reader accepts (the directory's section-version byte).
pub const LEVEL_VERSION: u8 = 2;

/// The most states in one level.
pub const MAX_STATES: usize = 32;
/// The most values one state has (the width of a binding's or a condition's mask).
pub const MAX_STATE_VALUES: usize = 8;
/// The most bindings in one level, of every kind.
pub const MAX_BINDINGS: usize = 64;
/// The most door bindings (kind 1), so that the simulation may keep one derived bit for each in a `u32`.
pub const MAX_DOOR_BINDINGS: usize = 32;
/// The most rules in one level (the sim keeps one bit per rule in a `u32`).
pub const MAX_RULES: usize = 32;
pub const MAX_STRINGS: usize = 32;
/// The most groups: a chicken's group number is 0 (none) or 1 to 16.
pub const MAX_GROUPS: usize = 16;
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
/// The longest radius of a light, in tiles, and the furthest an attached light's offset may reach.
pub const LIGHT_RADIUS_MAX: f64 = 16.0;
/// The most key colours: red 0, blue 1, green 2.
pub const KEY_COLOUR_MAX: u8 = 2;

pub const KIND_KEY_RED: u8 = 1;
pub const KIND_KEY_BLUE: u8 = 2;
pub const KIND_SNACK: u8 = 3;
pub const KIND_CHICKEN: u8 = 4;
pub const KIND_GADGET: u8 = 5;
pub const KIND_SWITCH: u8 = 6;
pub const KIND_ENEMY: u8 = 7;
pub const KIND_LIGHT: u8 = 8;
pub const KIND_CUSTOM_ENEMY: u8 = 9;
pub const KIND_CUSTOM_ITEM: u8 = 10;

/// Switch flags: which inputs set a switch off.
pub const SWITCH_HIT_SHOT: u16 = 1;
pub const SWITCH_HIT_STOMP: u16 = 2;
pub const SWITCH_USE: u16 = 4;
/// Light flag: the light follows an object (or Ben); its `x` and `y` are offsets.
pub const LIGHT_ATTACHED: u16 = 1;
/// A light's `attach` for none, and for Ben.
pub const ATTACH_NONE: u16 = 0xFFFF;
pub const ATTACH_BEN: u16 = 0xFFFE;

pub const BIND_DOOR: u8 = 1;
pub const BIND_LIGHT: u8 = 2;
/// Reserved, and refused until the game has level platforms.
pub const BIND_PLATFORM: u8 = 3;
/// Reserved, and refused until `BDRP` is specified.
pub const BIND_PANEL: u8 = 4;
/// A binding's `target` when it names no object.
pub const NO_TARGET: u16 = 0xFFFF;

pub const TRIGGER_START: u8 = 2;
pub const TRIGGER_ENTER: u8 = 3;
pub const TRIGGER_TAKE: u8 = 4;
pub const TRIGGER_SHOOT: u8 = 5;
pub const TRIGGER_STOMP: u8 = 6;
pub const TRIGGER_STUN: u8 = 7;
pub const TRIGGER_TIME: u8 = 8;
pub const TRIGGER_TOUCH: u8 = 9;
pub const TRIGGER_TOUCH_OBJECT: u8 = 10;
pub const TRIGGER_USE: u8 = 11;
pub const TRIGGER_HIT: u8 = 12;
pub const TRIGGER_DEFEAT: u8 = 13;
pub const TRIGGER_CLEARED: u8 = 14;
pub const TRIGGER_BECOMES: u8 = 15;

pub const COND_STATE_IN: u8 = 1;
pub const COND_HOLDS: u8 = 2;
pub const COND_NOT_HOLDS: u8 = 3;
/// Numbered and refused: ammo at least, score at least, a group is cleared.
pub const COND_AMMO_AT_LEAST: u8 = 4;
pub const COND_SCORE_AT_LEAST: u8 = 5;
pub const COND_CLEARED: u8 = 6;

pub const OP_SET: u8 = 1;
pub const OP_TOGGLE: u8 = 2;
pub const OP_NEXT: u8 = 3;
pub const OP_SPEND_KEY: u8 = 4;
pub const OP_GIVE_KEY: u8 = 5;
pub const OP_SCORE: u8 = 6;
pub const OP_AMMO: u8 = 7;
pub const OP_WORD: u8 = 8;
pub const OP_CUE: u8 = 9;

/// Rule flag: the rule may fire again after it has fired.
pub const FLAG_REPEAT: u8 = 1;
/// State flag: reserved for a world-scoped state; it must be zero.
pub const STATE_WORLD: u8 = 1;

/// A rule `subject` of "any" (stomp, stun and defeat), and of "none" on every other trigger.
pub const SUBJECT_ANY: u16 = 0xFFFF;
/// A `becomes` rule's `arg` of "any change".
pub const ARG_ANY: u8 = 0xFF;
/// A word action's `b` of "the string's own style", and the caption code that is never valid.
pub const CUE_NONE: u8 = 0xFF;
/// Caption codes below this are shared caption ids; `CAP_LEVEL_BASE + i` is the level's string `i`.
pub const CAP_LEVEL_BASE: u8 = 0x40;
/// `CAP_WORLD_BASE + i` is the world's string `i`.
pub const CAP_WORLD_BASE: u8 = 0x80;
/// Cue ids below this are shared sound effects; `CUE_WORLD_BASE + i` is entry `i` of the world's sound library.
pub const CUE_WORLD_BASE: i32 = 0x100;
/// The largest cue id: the world's sound library holds at most 256 entries.
pub const CUE_ID_MAX: i32 = 0x1FF;
/// The largest point value of a score action, and the largest shot count of an ammo action.
pub const SCORE_MAX: i32 = 9999;
pub const AMMO_MAX: i32 = 99;

/// The longest level name, in bytes.
pub const NAME_MAX: usize = 48;

/// Why a `LEVL` section was refused. A variant that names a record carries the record's index, so the baker can point at the
/// source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelError {
    /// Shorter than the header, or than the records and tiles it describes.
    Truncated,
    /// Longer than the records and tiles the header describes.
    TrailingBytes,
    /// A section version other than 2.
    BadVersion(u8),
    /// `floor_solid` is not 0 or 1, or a reserved header byte is not zero.
    BadField,
    /// A width or height of 0.
    BadSize,
    /// A name over 48 bytes or with a byte outside 0x20 to 0x7E, or padding that is not zero.
    BadName,
    /// A spawn outside the map or not finite.
    BadSpawn,
    /// A state count over 32.
    TooManyStates,
    /// A binding count over 64.
    TooManyBindings,
    /// A rule count over 32.
    TooManyRules,
    /// A string count over 32.
    TooManyStrings,
    /// The object count differs from the count in the directory entry.
    CountMismatch,
    /// State `i` has fewer than 2 or more than 8 values, a start not below its values, a non-zero `flags` or a non-zero reserved byte.
    BadState(u8),
    /// Binding `i` breaks the binding rules (kind, state, mask, target, rect, reserved bytes, the door limit, a double binding).
    BadBinding(u8),
    /// Object `i` has a `kind` of 0 or over 10, or a reserved kind.
    BadKind(u16),
    /// Object `i` has a field the object table says is zero but is not, or a value outside its table (a snack kind, a direction, a
    /// group, a radius, an attach target, a switch with no input flag, a non-finite `p`).
    BadObject(u16),
    /// Object `i`'s centre is outside the map or not finite.
    BadPosition(u16),
    /// Rule `i` breaks the rule checks in `docs/FAP.md`.
    BadRule(u8),
    /// String `i` breaks a string rule (length, UTF-8, charset, padding, style).
    BadString(u8),
}

/// One state record: how many values a state has and which one the level begins with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRec {
    /// 2 to 8.
    pub values: u8,
    /// The value the level begins with and a death restores, below `values`.
    pub start: u8,
    /// Bit 0 `STATE_WORLD` is reserved and must be zero.
    pub flags: u8,
}

impl StateRec {
    /// A level-scoped state of `values` values that starts at `start`.
    pub const fn new(values: u8, start: u8) -> StateRec {
        StateRec {
            values,
            start,
            flags: 0,
        }
    }

    /// The mask with one bit for each of the state's values.
    pub const fn all_values(&self) -> u8 {
        if self.values >= 8 {
            0xFF
        } else {
            ((1u16 << self.values) - 1) as u8
        }
    }

    /// The 8-byte record.
    pub fn to_bytes(&self) -> [u8; STATE_LEN] {
        let mut b = [0u8; STATE_LEN];
        b[0] = self.values;
        b[1] = self.start;
        b[2] = self.flags;
        b
    }

    /// Reads the first 8 bytes of `b` (panics if shorter). The reserved bytes are not kept; `LevelView::new` checks them.
    pub fn from_bytes(b: &[u8]) -> StateRec {
        StateRec {
            values: b[0],
            start: b[1],
            flags: b[2],
        }
    }
}

/// One binding record: a thing attached to a state and the set of values at which it is active (passable, lit, shown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingRec {
    pub kind: u8,
    pub state: u8,
    /// The active set: bit `i` is value `i`. Not zero, and no bit at or above the state's `values`.
    pub mask: u8,
    /// A light binding's object index; `NO_TARGET` otherwise.
    pub target: u16,
    /// A door binding's rect `(x0, y0, x1, y1)`, both corners included; zero otherwise.
    pub rect: (u16, u16, u16, u16),
}

impl BindingRec {
    /// A door binding: the switched tiles in `rect` are passable while `state`'s value is in `mask`.
    pub const fn door(state: u8, mask: u8, rect: (u16, u16, u16, u16)) -> BindingRec {
        BindingRec {
            kind: BIND_DOOR,
            state,
            mask,
            target: NO_TARGET,
            rect,
        }
    }

    /// The 16-byte record.
    pub fn to_bytes(&self) -> [u8; BINDING_LEN] {
        let mut b = [0u8; BINDING_LEN];
        b[0] = self.kind;
        b[1] = self.state;
        b[2] = self.mask;
        b[4..6].copy_from_slice(&self.target.to_le_bytes());
        b[8..10].copy_from_slice(&self.rect.0.to_le_bytes());
        b[10..12].copy_from_slice(&self.rect.1.to_le_bytes());
        b[12..14].copy_from_slice(&self.rect.2.to_le_bytes());
        b[14..16].copy_from_slice(&self.rect.3.to_le_bytes());
        b
    }

    /// Reads the first 16 bytes of `b` (panics if shorter). The reserved bytes are not kept; `LevelView::new` checks them.
    pub fn from_bytes(b: &[u8]) -> BindingRec {
        BindingRec {
            kind: b[0],
            state: b[1],
            mask: b[2],
            target: u16_at(b, 4),
            rect: (u16_at(b, 8), u16_at(b, 10), u16_at(b, 12), u16_at(b, 14)),
        }
    }
}

/// One object record. Coordinates are kept as `f64` bits; use `x()`, `y()` and `p()` for the numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectRec {
    pub kind: u8,
    pub sub: u8,
    pub flags: u16,
    /// A per-kind word: a chicken's group, a light's falloff, flicker and attach target; zero for the rest.
    pub aux: u32,
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

    /// The 32-byte record.
    pub fn to_bytes(&self) -> [u8; OBJECT_LEN] {
        let mut b = [0u8; OBJECT_LEN];
        b[0] = self.kind;
        b[1] = self.sub;
        b[2..4].copy_from_slice(&self.flags.to_le_bytes());
        b[4..8].copy_from_slice(&self.aux.to_le_bytes());
        b[8..16].copy_from_slice(&self.x_bits.to_le_bytes());
        b[16..24].copy_from_slice(&self.y_bits.to_le_bytes());
        b[24..32].copy_from_slice(&self.p_bits.to_le_bytes());
        b
    }

    /// Reads the first 32 bytes of `b` (panics if shorter).
    pub fn from_bytes(b: &[u8]) -> ObjectRec {
        ObjectRec {
            kind: b[0],
            sub: b[1],
            flags: u16_at(b, 2),
            aux: u32_at(b, 4),
            x_bits: u64_at(b, 8),
            y_bits: u64_at(b, 16),
            p_bits: u64_at(b, 24),
        }
    }
}

/// A rule's condition: empty, or one test that must hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CondRec {
    /// 0 none, 1 state is in a set, 2 Ben holds a key, 3 Ben does not hold a key (4 to 6 are numbered and refused).
    pub kind: u8,
    /// The state index (kind 1) or the key colour (kinds 2 and 3).
    pub a: u8,
    /// The set of values as a mask (kind 1); zero otherwise.
    pub b: u8,
}

impl CondRec {
    pub const EMPTY: CondRec = CondRec {
        kind: 0,
        a: 0,
        b: 0,
    };

    /// "State `state` is in the set `mask`."
    pub const fn state_in(state: u8, mask: u8) -> CondRec {
        CondRec {
            kind: COND_STATE_IN,
            a: state,
            b: mask,
        }
    }

    /// "Ben holds key `colour`."
    pub const fn holds(colour: u8) -> CondRec {
        CondRec {
            kind: COND_HOLDS,
            a: colour,
            b: 0,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.kind == 0 && self.a == 0 && self.b == 0
    }

    pub fn to_bytes(&self) -> [u8; CONDITION_LEN] {
        [self.kind, self.a, self.b, 0]
    }

    /// Reads the first 4 bytes of `b` (panics if shorter). The reserved byte is not kept.
    pub fn from_bytes(b: &[u8]) -> CondRec {
        CondRec {
            kind: b[0],
            a: b[1],
            b: b[2],
        }
    }
}

/// A rule's action: empty, or one op, run in slot order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionRec {
    pub op: u8,
    /// The state index, key colour or caption code, by `op`.
    pub a: u8,
    /// The value (`set`) or the style override (`word`), by `op`.
    pub b: u8,
    /// The amount (`score`, `ammo`) or the cue id (`cue`); zero otherwise.
    pub n: i32,
    /// The cell where a raised event is placed; zero for the other ops.
    pub x: u16,
    pub y: u16,
}

impl ActionRec {
    pub const EMPTY: ActionRec = ActionRec {
        op: 0,
        a: 0,
        b: 0,
        n: 0,
        x: 0,
        y: 0,
    };

    /// "Set state `state` to `value`."
    pub const fn set(state: u8, value: u8) -> ActionRec {
        ActionRec {
            op: OP_SET,
            a: state,
            b: value,
            ..ActionRec::EMPTY
        }
    }

    /// "Ben no longer holds key `colour`."
    pub const fn spend_key(colour: u8) -> ActionRec {
        ActionRec {
            op: OP_SPEND_KEY,
            a: colour,
            ..ActionRec::EMPTY
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.op == 0 && self.a == 0 && self.b == 0 && self.n == 0 && self.x == 0 && self.y == 0
    }

    pub fn to_bytes(&self) -> [u8; ACTION_LEN] {
        let mut b = [0u8; ACTION_LEN];
        b[0] = self.op;
        b[1] = self.a;
        b[2] = self.b;
        b[4..8].copy_from_slice(&self.n.to_le_bytes());
        b[8..10].copy_from_slice(&self.x.to_le_bytes());
        b[10..12].copy_from_slice(&self.y.to_le_bytes());
        b
    }

    /// Reads the first 12 bytes of `b` (panics if shorter). The reserved byte is not kept.
    pub fn from_bytes(b: &[u8]) -> ActionRec {
        ActionRec {
            op: b[0],
            a: b[1],
            b: b[2],
            n: u32_at(b, 4) as i32,
            x: u16_at(b, 8),
            y: u16_at(b, 10),
        }
    }
}

/// One rule record, with the union kept as its two raw words (`rect()` and `time()` read it as either form).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleRec {
    pub trigger: u8,
    pub flags: u8,
    /// Ticks between the input and the actions.
    pub delay: u16,
    /// What the input names; `SUBJECT_ANY` is "any" where the trigger allows it and "none" elsewhere.
    pub subject: u16,
    /// For `becomes`, the value or `ARG_ANY`; zero otherwise.
    pub arg: u8,
    /// Bytes 0 to 3 of the union: `x0` and `y0` (low and high `u16`) for a rect, `at` for a time rule.
    pub u0: u32,
    /// Bytes 4 to 7 of the union: `x1` and `y1` for a rect, `every` for a time rule.
    pub u1: u32,
    pub cond: [CondRec; 2],
    pub act: [ActionRec; 2],
}

impl RuleRec {
    /// A rule with no trigger, no conditions and no actions; callers fill in what they need.
    pub const EMPTY: RuleRec = RuleRec {
        trigger: 0,
        flags: 0,
        delay: 0,
        subject: SUBJECT_ANY,
        arg: 0,
        u0: 0,
        u1: 0,
        cond: [CondRec::EMPTY; 2],
        act: [ActionRec::EMPTY; 2],
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
    pub const fn union_rect(x0: u16, y0: u16, x1: u16, y1: u16) -> (u32, u32) {
        (x0 as u32 | (y0 as u32) << 16, x1 as u32 | (y1 as u32) << 16)
    }
    /// The union as `(at, every)` ticks.
    pub fn time(&self) -> (u32, u32) {
        (self.u0, self.u1)
    }
    pub fn repeat(&self) -> bool {
        self.flags & FLAG_REPEAT != 0
    }

    /// The 48-byte record.
    pub fn to_bytes(&self) -> [u8; RULE_LEN] {
        let mut b = [0u8; RULE_LEN];
        b[0] = self.trigger;
        b[1] = self.flags;
        b[2..4].copy_from_slice(&self.delay.to_le_bytes());
        b[4..6].copy_from_slice(&self.subject.to_le_bytes());
        b[6] = self.arg;
        b[8..12].copy_from_slice(&self.u0.to_le_bytes());
        b[12..16].copy_from_slice(&self.u1.to_le_bytes());
        b[16..20].copy_from_slice(&self.cond[0].to_bytes());
        b[20..24].copy_from_slice(&self.cond[1].to_bytes());
        b[24..36].copy_from_slice(&self.act[0].to_bytes());
        b[36..48].copy_from_slice(&self.act[1].to_bytes());
        b
    }

    /// Reads the first 48 bytes of `b` (panics if shorter). The reserved bytes are not kept; `LevelView::new` checks them.
    pub fn from_bytes(b: &[u8]) -> RuleRec {
        RuleRec {
            trigger: b[0],
            flags: b[1],
            delay: u16_at(b, 2),
            subject: u16_at(b, 4),
            arg: b[6],
            u0: u32_at(b, 8),
            u1: u32_at(b, 12),
            cond: [CondRec::from_bytes(&b[16..]), CondRec::from_bytes(&b[20..])],
            act: [
                ActionRec::from_bytes(&b[24..]),
                ActionRec::from_bytes(&b[36..]),
            ],
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

/// The decoded length of a `LEVL` section with these counts: `32 + pad8(name_len) + 8 * states + 16 * bindings +
/// 32 * (objects + strings) + 48 * rules + w * h`.
pub const fn level_len(
    name_len: usize,
    states: usize,
    bindings: usize,
    objects: usize,
    rules: usize,
    strings: usize,
    tiles: usize,
) -> usize {
    LEVEL_HEADER_LEN
        + padded_name_len(name_len)
        + STATE_LEN * states
        + BINDING_LEN * bindings
        + OBJECT_LEN * (objects + strings)
        + RULE_LEN * rules
        + tiles
}

/// Whether `(x, y)` is a centre inside a `w` by `h` map: finite, `0 <= x < w` and `0 <= y < h` (a NaN fails both tests).
fn inside(x: f64, y: f64, w: u16, h: u16) -> bool {
    x >= 0.0 && x < f64::from(w) && y >= 0.0 && y < f64::from(h)
}

/// Whether `v` is finite and `-16 <= v <= 16`.
fn within_reach(v: f64) -> bool {
    (-LIGHT_RADIUS_MAX..=LIGHT_RADIUS_MAX).contains(&v)
}

/// Whether two rects `(x0, y0, x1, y1)` share a cell.
fn rects_overlap(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

/// A validated `LEVL` section (already decoded).
#[derive(Clone, Copy, Debug)]
pub struct LevelView<'a> {
    data: &'a [u8],
    w: u16,
    h: u16,
    states: usize,
    bindings: usize,
    objects: usize,
    rules: usize,
    strings: usize,
    states_at: usize,
    bindings_at: usize,
    objects_at: usize,
    rules_at: usize,
    strings_at: usize,
    tiles_at: usize,
}

impl<'a> LevelView<'a> {
    /// Checks that `data` is exactly one well-formed `LEVL` section: the sizes in the header account for every byte, the name and
    /// reserved fields are clean, every record obeys its table and every position is inside the map. The section version is the
    /// directory's, so `with_directory` checks it.
    pub fn new(data: &'a [u8]) -> Result<Self, LevelError> {
        if data.len() < LEVEL_HEADER_LEN {
            return Err(LevelError::Truncated);
        }
        if data[4] > 1 || data[28..32].iter().any(|&b| b != 0) {
            return Err(LevelError::BadField);
        }
        let (w, h) = (u16_at(data, 0), u16_at(data, 2));
        if w == 0 || h == 0 {
            return Err(LevelError::BadSize);
        }
        let name_len = usize::from(data[5]);
        if name_len > NAME_MAX {
            return Err(LevelError::BadName);
        }
        let (states, bindings) = (usize::from(data[24]), usize::from(data[25]));
        let (rules, strings) = (usize::from(data[26]), usize::from(data[27]));
        if states > MAX_STATES {
            return Err(LevelError::TooManyStates);
        }
        if bindings > MAX_BINDINGS {
            return Err(LevelError::TooManyBindings);
        }
        if rules > MAX_RULES {
            return Err(LevelError::TooManyRules);
        }
        if strings > MAX_STRINGS {
            return Err(LevelError::TooManyStrings);
        }
        let objects = usize::from(u16_at(data, 6));
        let states_at = LEVEL_HEADER_LEN + padded_name_len(name_len);
        let bindings_at = states_at + states * STATE_LEN;
        let objects_at = bindings_at + bindings * BINDING_LEN;
        let rules_at = objects_at + objects * OBJECT_LEN;
        let strings_at = rules_at + rules * RULE_LEN;
        let tiles_at = strings_at + strings * STRING_LEN;
        // At most 48 + 7 + 256 + 1024 + 65535 * 32 + 1536 + 1024 + 65535 * 65535 bytes, which fits in a u64 and is far past any real
        // section.
        let end = tiles_at as u64 + u64::from(w) * u64::from(h);
        if end > data.len() as u64 {
            return Err(LevelError::Truncated);
        }
        if end < data.len() as u64 {
            return Err(LevelError::TrailingBytes);
        }
        let name = &data[LEVEL_HEADER_LEN..states_at];
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
            states,
            bindings,
            objects,
            rules,
            strings,
            states_at,
            bindings_at,
            objects_at,
            rules_at,
            strings_at,
            tiles_at,
        };
        let (sx, sy) = view.spawn();
        if !inside(sx, sy, w, h) {
            return Err(LevelError::BadSpawn);
        }
        for i in 0..states {
            view.check_state(i)?;
        }
        for i in 0..objects {
            view.check_object(i)?;
        }
        view.check_bindings()?;
        for i in 0..strings {
            view.check_string(i)?;
        }
        for i in 0..rules {
            view.check_rule(i)?;
        }
        Ok(view)
    }

    fn check_state(&self, i: usize) -> Result<(), LevelError> {
        let bad = Err(LevelError::BadState(i as u8));
        let at = self.states_at + i * STATE_LEN;
        let b = &self.data[at..at + STATE_LEN];
        let s = self.state(i);
        if !(2..=MAX_STATE_VALUES as u8).contains(&s.values)
            || s.start >= s.values
            || s.flags != 0
            || b[3..].iter().any(|&x| x != 0)
        {
            return bad;
        }
        Ok(())
    }

    fn check_object(&self, i: usize) -> Result<(), LevelError> {
        let bad = Err(LevelError::BadObject(i as u16));
        let o = self.object(i);
        if o.kind == 0 || o.kind > KIND_CUSTOM_ITEM {
            return Err(LevelError::BadKind(i as u16));
        }
        if matches!(
            o.kind,
            KIND_GADGET | KIND_ENEMY | KIND_CUSTOM_ENEMY | KIND_CUSTOM_ITEM
        ) {
            return Err(LevelError::BadKind(i as u16));
        }
        // The fields the kind leaves at zero.
        let zero_sub = o.sub == 0;
        let zero_flags = o.flags == 0;
        let zero_aux = o.aux == 0;
        let zero_p = o.p_bits == 0;
        let attached = o.kind == KIND_LIGHT && o.flags & LIGHT_ATTACHED != 0;
        match o.kind {
            KIND_KEY_RED | KIND_KEY_BLUE => {
                if !(zero_sub && zero_flags && zero_aux && zero_p) {
                    return bad;
                }
            }
            KIND_SNACK => {
                if o.sub > 4 || !(zero_flags && zero_aux && zero_p) {
                    return bad;
                }
            }
            KIND_CHICKEN => {
                let dir = o.p();
                if !(zero_sub && zero_flags)
                    || o.aux >> 8 != 0
                    || o.aux & 0xFF > MAX_GROUPS as u32
                    || (dir != 1.0 && dir != -1.0)
                {
                    return bad;
                }
            }
            KIND_SWITCH => {
                let inputs = SWITCH_HIT_SHOT | SWITCH_HIT_STOMP | SWITCH_USE;
                if o.sub > 1 || o.flags & !inputs != 0 || o.flags == 0 || !(zero_aux && zero_p) {
                    return bad;
                }
            }
            _ => {
                // A light.
                let radius = o.p();
                let attach = (o.aux >> 16) as u16;
                let target_ok = match attach {
                    ATTACH_NONE | ATTACH_BEN => true,
                    t => {
                        usize::from(t) < self.objects
                            && matches!(self.object(usize::from(t)).kind, KIND_SNACK | KIND_CHICKEN)
                    }
                };
                if o.sub > 15
                    || o.flags & !LIGHT_ATTACHED != 0
                    || (o.aux & 0xFF) > 1
                    || ((o.aux >> 8) & 0xFF) > 2
                    || !target_ok
                    || attached != (attach != ATTACH_NONE)
                    || !(radius > 0.0 && radius <= LIGHT_RADIUS_MAX)
                {
                    return bad;
                }
            }
        }
        let position_ok = if attached {
            within_reach(o.x()) && within_reach(o.y())
        } else {
            inside(o.x(), o.y(), self.w, self.h)
        };
        if !position_ok {
            return Err(LevelError::BadPosition(i as u16));
        }
        Ok(())
    }

    fn check_bindings(&self) -> Result<(), LevelError> {
        let mut doors = 0;
        for i in 0..self.bindings {
            let bad = Err(LevelError::BadBinding(i as u8));
            let at = self.bindings_at + i * BINDING_LEN;
            let raw = &self.data[at..at + BINDING_LEN];
            let b = self.binding(i);
            if raw[3] != 0 || raw[6] != 0 || raw[7] != 0 {
                return bad;
            }
            if usize::from(b.state) >= self.states {
                return bad;
            }
            let state = self.state(usize::from(b.state));
            if b.mask == 0 || b.mask & !state.all_values() != 0 {
                return bad;
            }
            match b.kind {
                BIND_DOOR => {
                    doors += 1;
                    let (x0, y0, x1, y1) = b.rect;
                    if doors > MAX_DOOR_BINDINGS
                        || b.target != NO_TARGET
                        || x0 > x1
                        || y0 > y1
                        || x1 >= self.w
                        || y1 >= self.h
                    {
                        return bad;
                    }
                }
                BIND_LIGHT => {
                    if b.rect != (0, 0, 0, 0)
                        || b.target == NO_TARGET
                        || usize::from(b.target) >= self.objects
                        || self.object(usize::from(b.target)).kind != KIND_LIGHT
                    {
                        return bad;
                    }
                }
                // 0, the reserved platform and panel kinds, and everything unassigned.
                _ => return bad,
            }
            for j in 0..i {
                let o = self.binding(j);
                if o.kind != b.kind {
                    continue;
                }
                let twice = if b.kind == BIND_DOOR {
                    rects_overlap(o.rect, b.rect)
                } else {
                    o.target == b.target
                };
                if twice {
                    return bad;
                }
            }
        }
        Ok(())
    }

    fn check_string(&self, i: usize) -> Result<(), LevelError> {
        let bad = Err(LevelError::BadString(i as u8));
        let at = self.strings_at + i * STRING_LEN;
        let b = &self.data[at..at + STRING_LEN];
        let len = usize::from(b[0]);
        if len == 0 || len > STRING_TEXT_MAX || b[1] >= STYLE_COUNT {
            return bad;
        }
        if b[2 + len..].iter().any(|&x| x != 0) {
            return bad;
        }
        let Ok(text) = core::str::from_utf8(&b[2..2 + len]) else {
            return bad;
        };
        for c in text.chars() {
            let u = u32::from(c);
            if !(0x20..=0x7E).contains(&u) && !STRING_EXTRAS.contains(&u) {
                return bad;
            }
        }
        Ok(())
    }

    /// The number of values of state `s`, or `None` when there is no such state.
    fn values_of(&self, s: u8) -> Option<u8> {
        (usize::from(s) < self.states).then(|| self.state(usize::from(s)).values)
    }

    fn check_condition(&self, raw: &[u8], c: CondRec) -> bool {
        if raw[3] != 0 {
            return false;
        }
        match c.kind {
            0 => c.is_empty(),
            COND_STATE_IN => match self.values_of(c.a) {
                Some(v) => {
                    let all = StateRec::new(v, 0).all_values();
                    c.b != 0 && c.b & !all == 0
                }
                None => false,
            },
            COND_HOLDS | COND_NOT_HOLDS => c.a <= KEY_COLOUR_MAX && c.b == 0,
            // 4 to 6 are numbered and refused, and so is everything unassigned.
            _ => false,
        }
    }

    fn check_action(&self, raw: &[u8], a: ActionRec) -> bool {
        if raw[3] != 0 {
            return false;
        }
        let in_map = |x: u16, y: u16| x < self.w && y < self.h;
        let no_cell = a.x == 0 && a.y == 0;
        match a.op {
            0 => a.is_empty(),
            OP_SET => match self.values_of(a.a) {
                Some(v) => a.b < v && a.n == 0 && no_cell,
                None => false,
            },
            OP_TOGGLE => self.values_of(a.a) == Some(2) && a.b == 0 && a.n == 0 && no_cell,
            OP_NEXT => self.values_of(a.a).is_some() && a.b == 0 && a.n == 0 && no_cell,
            OP_SPEND_KEY | OP_GIVE_KEY => a.a <= KEY_COLOUR_MAX && a.b == 0 && a.n == 0 && no_cell,
            OP_SCORE => a.a == 0 && a.b == 0 && (1..=SCORE_MAX).contains(&a.n) && no_cell,
            OP_AMMO => a.a == 0 && a.b == 0 && (1..=AMMO_MAX).contains(&a.n) && no_cell,
            OP_WORD => {
                let code_ok = match a.a {
                    CUE_NONE => false,
                    c if c < CAP_LEVEL_BASE => true,
                    c if c < CAP_WORLD_BASE => usize::from(c - CAP_LEVEL_BASE) < self.strings,
                    c => c < 0xC0,
                };
                code_ok && (a.b < STYLE_COUNT || a.b == CUE_NONE) && a.n == 0 && in_map(a.x, a.y)
            }
            OP_CUE => a.a == 0 && a.b == 0 && (0..=CUE_ID_MAX).contains(&a.n) && in_map(a.x, a.y),
            _ => false,
        }
    }

    fn check_rule(&self, i: usize) -> Result<(), LevelError> {
        let bad = Err(LevelError::BadRule(i as u8));
        let at = self.rules_at + i * RULE_LEN;
        let raw = &self.data[at..at + RULE_LEN];
        if raw[7] != 0 {
            return bad;
        }
        let r = self.rule(i);
        let (w, h) = (self.w, self.h);
        if !(TRIGGER_START..=TRIGGER_BECOMES).contains(&r.trigger) || r.flags & !FLAG_REPEAT != 0 {
            return bad;
        }
        // The object a subject names, if there is one.
        let object_kind = |s: u16| {
            if usize::from(s) < self.objects {
                Some(self.object(usize::from(s)).kind)
            } else {
                None
            }
        };
        let subject_ok = match r.trigger {
            TRIGGER_TAKE => matches!(
                object_kind(r.subject),
                Some(KIND_KEY_RED | KIND_KEY_BLUE | KIND_SNACK)
            ),
            TRIGGER_STOMP | TRIGGER_STUN | TRIGGER_DEFEAT => {
                r.subject == SUBJECT_ANY || matches!(object_kind(r.subject), Some(KIND_CHICKEN))
            }
            TRIGGER_TOUCH_OBJECT => object_kind(r.subject).is_some(),
            TRIGGER_USE | TRIGGER_HIT => object_kind(r.subject) == Some(KIND_SWITCH),
            TRIGGER_CLEARED => {
                (1..=MAX_GROUPS as u16).contains(&r.subject)
                    && self
                        .objects()
                        .any(|o| o.kind == KIND_CHICKEN && u32::from(r.subject) == (o.aux & 0xFF))
            }
            TRIGGER_BECOMES => {
                r.subject < MAX_STATES as u16 && self.values_of(r.subject as u8).is_some()
            }
            _ => r.subject == SUBJECT_ANY,
        };
        if !subject_ok {
            return bad;
        }
        // The arg is a state value (or any change) on `becomes` and zero everywhere else.
        if r.trigger == TRIGGER_BECOMES {
            let values = self.values_of(r.subject as u8).unwrap_or(0);
            if r.arg != ARG_ANY && r.arg >= values {
                return bad;
            }
        } else if r.arg != 0 {
            return bad;
        }
        match r.trigger {
            TRIGGER_ENTER | TRIGGER_SHOOT | TRIGGER_TOUCH => {
                let (x0, y0, x1, y1) = r.rect();
                if x0 > x1 || y0 > y1 || x1 >= w || y1 >= h {
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
        for k in 0..2 {
            let c_at = 16 + CONDITION_LEN * k;
            let a_at = 24 + ACTION_LEN * k;
            if !self.check_condition(&raw[c_at..c_at + CONDITION_LEN], r.cond[k])
                || !self.check_action(&raw[a_at..a_at + ACTION_LEN], r.act[k])
            {
                return bad;
            }
        }
        // A rule does something.
        if r.act.iter().all(ActionRec::is_empty) {
            return bad;
        }
        Ok(())
    }

    /// `new`, after checking the directory entry's section version and object count against the section.
    pub fn with_directory(data: &'a [u8], version: u8, count: u16) -> Result<Self, LevelError> {
        if version != LEVEL_VERSION {
            return Err(LevelError::BadVersion(version));
        }
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
    pub fn floor_solid(&self) -> bool {
        self.data[4] == 1
    }
    pub fn spawn_bits(&self) -> (u64, u64) {
        (u64_at(self.data, 8), u64_at(self.data, 16))
    }
    pub fn spawn(&self) -> (f64, f64) {
        let (x, y) = self.spawn_bits();
        (f64::from_bits(x), f64::from_bits(y))
    }
    /// The level's name bytes (printable ASCII, checked by `new`).
    pub fn name(&self) -> &'a [u8] {
        &self.data[LEVEL_HEADER_LEN..LEVEL_HEADER_LEN + usize::from(self.data[5])]
    }
    pub fn state_count(&self) -> usize {
        self.states
    }
    /// State `i` in file order (panics if `i >= state_count()`).
    pub fn state(&self, i: usize) -> StateRec {
        assert!(i < self.states);
        StateRec::from_bytes(&self.data[self.states_at + i * STATE_LEN..])
    }
    pub fn states(&self) -> impl Iterator<Item = StateRec> + '_ {
        (0..self.states).map(|i| self.state(i))
    }
    pub fn binding_count(&self) -> usize {
        self.bindings
    }
    /// Binding `i` in file order (panics if `i >= binding_count()`).
    pub fn binding(&self, i: usize) -> BindingRec {
        assert!(i < self.bindings);
        BindingRec::from_bytes(&self.data[self.bindings_at + i * BINDING_LEN..])
    }
    pub fn bindings(&self) -> impl Iterator<Item = BindingRec> + '_ {
        (0..self.bindings).map(|i| self.binding(i))
    }
    pub fn object_count(&self) -> usize {
        self.objects
    }
    /// Object `i` in file order (panics if `i >= object_count()`).
    pub fn object(&self, i: usize) -> ObjectRec {
        assert!(i < self.objects);
        ObjectRec::from_bytes(&self.data[self.objects_at + i * OBJECT_LEN..])
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
