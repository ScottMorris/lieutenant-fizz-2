// The slice's level: a cut-down Lighthouse Point interior, hand-written as `const` data in the shape `fizz-bake` will fill.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # The tile alphabet
//!
//! The level is drawn as text, top row first, and baked at compile time into the same row-major, bottom-row-first byte
//! array a [`TileMap`] holds, so nothing is parsed at run time.
//!
//! | Letter | Tile | Meaning |
//! | --- | --- | --- |
//! | `.` | [`EMPTY`] | sky, or open air |
//! | `w` | [`WALLBG`] | interior wall behind the rooms; not solid |
//! | `#` | [`FILL`] | solid fill |
//! | `B` | [`BLOCK`] | a solid block |
//! | `=` | [`PLAT`] | a one-way platform: solid only when landed on from above |
//! | `^` | [`SPIKE`] | spikes: lethal |
//! | `D` | [`DOOR_R`] | the red door: solid until Ben opens it with the key |
//! | `b` | [`DOOR_B`] | the blue door: solid until Ben opens it with the blue key |
//! | `E` | [`EXIT`] | the exit |
//!
//! # The route
//!
//! Ben starts on the floor of the left shaft at [`SPAWN`]. The shaft climbs by one-way ledges three tiles apart, which a
//! full jump (3.5 tiles) clears: alternate ledges sit on the left and the right so each jump crosses the shaft. The third
//! ledge is long and holds the chicken ([`CHICKEN`]) and, at its far left end, the red key ([`KEY`]). Above that, the
//! ledges climb to the red door ([`DOOR`]) in the wall that divides the two shafts. With the key, walking into the door
//! opens it; beyond it the right shaft climbs, again by ledges, to a gallery in the open air where the exit lies
//! ([`EXIT_TILES`]). A short run of spikes on the ground floor ends any careless drop from the first ledges.
//!
//! # The snack nook
//!
//! The gallery is also the playground. Ben climbs onto its right end, and the blue cookie door ([`BLUE_DOOR`]) stands
//! between him and the exit, under a lintel of blocks that runs to the top of the map, so nothing but the door lets him
//! pass. Right of the door sits the snack nook: a shelf of blocks ([`SHELF`]) four tiles above the floor.
//!
//! - Under the shelf hang two cheezies ([`SNACKS`]). A jump from the floor reaches them and bumps Ben's head on the shelf,
//!   which is the head-bump clunk.
//! - On the shelf lie a cheezie, a chocolate and a cookie, and at its left end the blue key ([`KEY_BLUE`]). The shelf is
//!   too high for a jump (3.5 tiles); the pogo with Jump held bounces 6.6 tiles, enough to land on it from the right, and
//!   every pogo landing is a bounce.
//! - Fizz Blaster ammo is five shots. Spending them against the right-hand wall and pressing Fire once more is the dud.
//! - Fizz 1's soda cans refill it, five shots a can. One lies on the gallery floor three tiles from the spot where the five
//!   shots go, between Ben and the wall he shoots at, so the player who has spent them walks into a refill ([`SNACKS`], the
//!   last two entries). The other lies at the right end of the shelf, where the pogo lands.

use crate::doors::{DoorError, Doors};
use crate::rules::{self, RuleError, RuleTable};
use crate::tiles::{self, BLOCK, DOOR_B, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};
use crate::world::{SnackKind, World};
use fizz_pack::level::{
    KIND_CHICKEN, KIND_CUSTOM_ENEMY, KIND_CUSTOM_ITEM, KIND_ENEMY, KIND_GADGET, KIND_KEY_BLUE,
    KIND_KEY_RED, KIND_LIGHT, KIND_SNACK, KIND_SWITCH, MAX_STRINGS, STRING_TEXT_MAX, STYLE_COUNT,
    TRIGGER_TOUCH,
};
use fizz_pack::{
    ActionRec, BindingRec, CondRec, LevelView, ObjectRec, RuleRec, StateRec, StringRec,
};
use fizz_phys::TileMap;
use fizz_sim::StateHash;

/// The level's width in tiles.
pub const W: usize = 32;
/// The level's height in tiles.
pub const H: usize = 48;

/// The level as text, top row first (the comment on each row is its `y`).
const ROWS: [&str; H] = [
    "......................B........B", // 47
    "......................B........B", // 46
    "......................B........B", // 45
    "......................B........B", // 44
    "......................B........B", // 43
    "......................B........B", // 42
    "......................B.BBBBB..B", // 41
    "......................b........B", // 40
    "....................E.b........B", // 39
    "....................E.b........B", // 38
    "#..............B===============#", // 37
    "###############Bwwwwwwwwwwwwwww#", // 36
    "###############Bwwwwwwwwwwwwwww#", // 35
    "###############Bwwwwwwww=======#", // 34
    "###############Bwwwwwwwwwwwwwww#", // 33
    "###############Bwwwwwwwwwwwwwww#", // 32
    "###############B=======wwwwwwww#", // 31
    "###############Bwwwwwwwwwwwwwww#", // 30
    "###############Bwwwwwwwwwwwwwww#", // 29
    "###############Bwwwwwwww=======#", // 28
    "###############Bwwwwwwwwwwwwwww#", // 27
    "###############Bwwwwwwwwwwwwwww#", // 26
    "###############B=======wwwwwwww#", // 25
    "###############Bwwwwwwwwwwwwwww#", // 24
    "###############Bwwwwwwwwwwwwwww#", // 23
    "#wwwwwwwwwwwwwwDwwwwwwww=======#", // 22
    "#wwwwwwwwwwwwwwDwwwwwwwwwwwwwww#", // 21
    "#wwwwwwwwwwwwwwDwwwwwwwwwwwwwww#", // 20
    "#wwwwwww=======BBBBBBBBBBBBBBBB#", // 19
    "#wwwwwwwwwwwwwwB################", // 18
    "#wwwwwwwwwwwwwwB################", // 17
    "#=======wwwwwwwB################", // 16
    "#wwwwwwwwwwwwwwB################", // 15
    "#wwwwwwwwwwwwwwB################", // 14
    "#wwwwwww=======B################", // 13
    "#wwwwwwwwwwwwwwB################", // 12
    "#wwwwwwwwwwwwwwB################", // 11
    "#w=============B################", // 10
    "#wwwwwwwwwwwwwwB################", // 9
    "#wwwwwwwwwwwwwwB################", // 8
    "#wwwwwww=======B################", // 7
    "#wwwwwwwwwwwwwwB################", // 6
    "#wwwwwwwwwwwwwwB################", // 5
    "#=======wwwwwwwB################", // 4
    "#wwwwwwwwwwwwwwB################", // 3
    "#wwwwwwwww^^^wwB################", // 2
    "################################", // 1
    "################################", // 0
];

/// Where Ben starts and respawns: the lower-left corner of his body, standing on the ground floor.
pub const SPAWN: (f64, f64) = (2.0, 2.0);
/// The red key's centre, at the far left end of the chicken's ledge.
pub const KEY: (f64, f64) = (2.5, 11.7);
/// The chicken's spawn point and direction, as Fizz 1's level builder takes them: it stands on the long ledge (`y = 11`).
pub const CHICKEN: (f64, f64, f64) = (8.0, 11.0, -1.0);
/// The red door's tiles: column 15, rows 20 to 22, in the wall between the shafts.
pub const DOOR: [(i32, i32); 3] = [(15, 20), (15, 21), (15, 22)];
/// The exit's tiles, on the gallery at the top of the right shaft.
pub const EXIT_TILES: [(i32, i32); 2] = [(20, 38), (20, 39)];
/// The blue door's tiles: column 22, rows 38 to 40, under the lintel of blocks that closes the gallery up to the map's top.
pub const BLUE_DOOR: [(i32, i32); 3] = [(22, 38), (22, 39), (22, 40)];
/// The shelf's tiles: row 41, columns 24 to 28. Its top is 42 tiles up, four above the gallery floor.
pub const SHELF: [(i32, i32); 5] = [(24, 41), (25, 41), (26, 41), (27, 41), (28, 41)];
/// The blue key's centre, at the left end of the shelf.
pub const KEY_BLUE: (f64, f64) = (24.5, 42.7);
/// The elixir's centre (PROVISIONAL, a placeholder for the owner's lives design): on the second ledge of the left shaft (row 7,
/// columns 8 to 14), at its right end by the wall. The route lands on the ledge's left part and climbs on from there, so the replay never
/// comes within 3 tiles of the elixir; a player who walks along the ledge to the wall takes it.
pub const ELIXIR: (SnackKind, f64, f64) = (SnackKind::Elixir, 13.5, 8.7);
/// The snacks and the soda cans, as `(kind, x, y)` with the centre in tiles, in the order they are added to the world (and so the
/// order of the hash). Two cheezies hang under the shelf, where a jump bumps the head to reach them; on the shelf, from its right
/// end to its left, lie a cheezie, a chocolate and a cookie. Then the cans: one on the gallery floor in the shooting stretch
/// by the right-hand wall, where Ben spends his five shots and needs more, and one on the right end of the shelf.
pub const SNACKS: [(SnackKind, f64, f64); 8] = [
    (SnackKind::Cheezie, 25.5, 40.2),
    (SnackKind::Cheezie, 27.5, 40.2),
    (SnackKind::Cheezie, 27.5, 42.7),
    (SnackKind::Choc, 26.5, 42.7),
    (SnackKind::Cookie, 25.5, 42.7),
    (SnackKind::Soda, 28.5, 38.7),
    (SnackKind::Soda, 28.5, 42.7),
    ELIXIR,
];

/// The baked level: the shape `fizz-bake` will fill from a map file.
pub struct LevelData {
    pub name: &'static str,
    pub w: i32,
    pub h: i32,
    /// Tile ids, row-major, row 0 (the bottom) first.
    pub tiles: [u8; W * H],
}

const fn tile_of(letter: u8) -> u8 {
    match letter {
        b'.' => EMPTY,
        b'w' => WALLBG,
        b'#' => FILL,
        b'B' => BLOCK,
        b'=' => PLAT,
        b'^' => SPIKE,
        b'D' => DOOR_R,
        b'b' => DOOR_B,
        b'E' => EXIT,
        _ => panic!("unknown tile letter"),
    }
}

const fn bake(rows: &[&str; H]) -> [u8; W * H] {
    let mut out = [0u8; W * H];
    let mut r = 0;
    while r < H {
        let bytes = rows[r].as_bytes();
        assert!(bytes.len() == W);
        let y = H - 1 - r;
        let mut x = 0;
        while x < W {
            out[y * W + x] = tile_of(bytes[x]);
            x += 1;
        }
        r += 1;
    }
    out
}

/// The slice level.
pub const SLICE: LevelData = LevelData {
    name: "Lighthouse Point",
    w: W as i32,
    h: H as i32,
    tiles: bake(&ROWS),
};

/// The slice level's map, with its tile properties and its doors bound to their switch bits.
pub fn map() -> TileMap {
    let mut m = TileMap::new(SLICE.w, SLICE.h, tiles::props());
    m.data.copy_from_slice(&SLICE.tiles);
    slice_doors().bind(&mut m);
    m
}

/// The slice's states and door bindings read against its tiles.
fn slice_doors() -> Doors {
    doors_of(&SLICE_SOURCE).expect("the slice level is valid")
}

/// A fresh world on the slice level: Ben at the spawn, the keys, the snacks and the chicken in place, both doors closed.
pub fn world() -> World {
    world_from(&SLICE_SOURCE).expect("the slice level is valid")
}

/// What the level can hold besides tiles. The variants are the object kinds of a `LEVL` section that the rules can build;
/// the pack's other kinds (the gadget, the switch, the light and the enemies) are refused here until the rules have a consumer
/// for them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Object {
    /// The red key, its centre in tiles.
    KeyRed { x: f64, y: f64 },
    /// The blue key, its centre in tiles.
    KeyBlue { x: f64, y: f64 },
    /// A snack or a soda can, its centre in tiles.
    Snack { kind: SnackKind, x: f64, y: f64 },
    /// A chicken's spawn point and direction (1 or -1).
    Chicken { x: f64, y: f64, dir: f64 },
}

impl Object {
    /// The `LEVL` object kind number.
    pub const fn kind(&self) -> u8 {
        match self {
            Object::KeyRed { .. } => KIND_KEY_RED,
            Object::KeyBlue { .. } => KIND_KEY_BLUE,
            Object::Snack { .. } => KIND_SNACK,
            Object::Chicken { .. } => KIND_CHICKEN,
        }
    }

    /// The object as a `LEVL` record: `sub` is the snack kind number, `p` the chicken's direction, `flags` and `aux` zero (a
    /// chicken's group is 0, none).
    pub fn to_rec(&self) -> ObjectRec {
        let (sub, x, y, p) = match *self {
            Object::KeyRed { x, y } | Object::KeyBlue { x, y } => (0, x, y, 0.0),
            Object::Snack { kind, x, y } => (kind.number(), x, y, 0.0),
            Object::Chicken { x, y, dir } => (0, x, y, dir),
        };
        ObjectRec {
            kind: self.kind(),
            sub,
            flags: 0,
            aux: 0,
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
            p_bits: p.to_bits(),
        }
    }

    /// Reads a record the rules can build. The kinds the rules have no consumer for, an unknown snack `sub`, a chicken direction
    /// other than 1 or -1 and any non-zero `flags`, `aux` or `p` (except a chicken's `p`) are refused.
    pub fn from_rec(r: &ObjectRec) -> Result<Object, SourceError> {
        let (x, y, p) = (r.x(), r.y(), r.p());
        if matches!(
            r.kind,
            KIND_GADGET
                | KIND_SWITCH
                | KIND_ENEMY
                | KIND_LIGHT
                | KIND_CUSTOM_ENEMY
                | KIND_CUSTOM_ITEM
        ) {
            return Err(SourceError::ReservedKind);
        }
        if r.flags != 0 || r.aux != 0 {
            return Err(SourceError::BadObject);
        }
        match r.kind {
            KIND_KEY_RED | KIND_KEY_BLUE | KIND_SNACK if r.p_bits != 0 => {
                Err(SourceError::BadObject)
            }
            KIND_KEY_RED | KIND_KEY_BLUE if r.sub != 0 => Err(SourceError::BadObject),
            KIND_KEY_RED => Ok(Object::KeyRed { x, y }),
            KIND_KEY_BLUE => Ok(Object::KeyBlue { x, y }),
            KIND_SNACK => Ok(Object::Snack {
                kind: SnackKind::from_number(r.sub).ok_or(SourceError::SnackKind)?,
                x,
                y,
            }),
            KIND_CHICKEN if r.sub != 0 => Err(SourceError::BadObject),
            KIND_CHICKEN if p == 1.0 || p == -1.0 => Ok(Object::Chicken { x, y, dir: p }),
            KIND_CHICKEN => Err(SourceError::ChickenDir),
            _ => Err(SourceError::BadObject),
        }
    }
}

/// Why a level was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceError {
    /// A width or height of 0, or a tile list that is not `w * h` bytes long.
    BadSize,
    /// The objects are not in ascending kind order (keys, then snacks, then chickens).
    KindOrder,
    /// A second key of one colour.
    SecondKey,
    /// A snack `sub` that is not one of the four snack kinds.
    SnackKind,
    /// A chicken direction other than 1 or -1.
    ChickenDir,
    /// An object kind the rules cannot build yet (the pack accepts the switch and the light; the rest are reserved).
    ReservedKind,
    /// Any other malformed object record (an unknown kind, or fields that must be zero).
    BadObject,
    /// A state or door binding the world cannot build.
    Door(DoorError),
    /// A rule the rule phase cannot run: an input, condition, action or delay that is not built yet, or a record that does not fit
    /// the level's states (see [`RuleError`]).
    Rule(RuleError),
    /// A string whose length is 0 or over 30, or whose style is not one of the seven, or more than 32 strings.
    BadString,
}

/// A level as the rules build a world from it: the same content a `LEVL` section holds, borrowed so a `const` can own it.
#[derive(Clone, Copy, Debug)]
pub struct LevelSource<'a> {
    /// For banners and the manifest; the rules and the digest never read it.
    pub name: &'a str,
    pub w: u16,
    pub h: u16,
    /// Tile ids, row-major, row 0 (the bottom) first.
    pub tiles: &'a [u8],
    pub floor_solid: bool,
    /// Ben's start: the lower-left corner of his body.
    pub spawn: (f64, f64),
    /// The level's states: how many values each has and the value it starts at.
    pub states: &'a [StateRec],
    /// The level's bindings. Only door bindings can be built so far.
    pub bindings: &'a [BindingRec],
    /// In construction order: keys, then snacks, then chickens, same-kind objects in source order.
    pub objects: &'a [Object],
    /// The level's rules in rule order (the order the sim evaluates them). Only locks can be built so far.
    pub rules: &'a [RuleRec],
    /// The level's own caption words. Carried and fingerprinted; nothing draws them yet.
    pub strings: &'a [StringRec],
}

/// The slice's objects in the order the world is built: both keys, the eight snacks, soda cans and the elixir of [`SNACKS`], and the
/// chicken. The elixir is the last snack, and the hash leaves the elixirs out of its snack list (see `world::hash`).
pub const SLICE_OBJECTS: [Object; 11] = [
    Object::KeyRed { x: KEY.0, y: KEY.1 },
    Object::KeyBlue {
        x: KEY_BLUE.0,
        y: KEY_BLUE.1,
    },
    Object::Snack {
        kind: SNACKS[0].0,
        x: SNACKS[0].1,
        y: SNACKS[0].2,
    },
    Object::Snack {
        kind: SNACKS[1].0,
        x: SNACKS[1].1,
        y: SNACKS[1].2,
    },
    Object::Snack {
        kind: SNACKS[2].0,
        x: SNACKS[2].1,
        y: SNACKS[2].2,
    },
    Object::Snack {
        kind: SNACKS[3].0,
        x: SNACKS[3].1,
        y: SNACKS[3].2,
    },
    Object::Snack {
        kind: SNACKS[4].0,
        x: SNACKS[4].1,
        y: SNACKS[4].2,
    },
    Object::Snack {
        kind: SNACKS[5].0,
        x: SNACKS[5].1,
        y: SNACKS[5].2,
    },
    Object::Snack {
        kind: SNACKS[6].0,
        x: SNACKS[6].1,
        y: SNACKS[6].2,
    },
    Object::Snack {
        kind: SNACKS[7].0,
        x: SNACKS[7].1,
        y: SNACKS[7].2,
    },
    Object::Chicken {
        x: CHICKEN.0,
        y: CHICKEN.1,
        dir: CHICKEN.2,
    },
];

/// The slice's states: `red-door` and `blue-door`, each of the values `closed` (0) and `open` (1), starting closed.
pub const SLICE_STATES: [StateRec; 2] = [StateRec::new(2, 0), StateRec::new(2, 0)];

/// The slice's door bindings: each door's tiles are passable while its state is in the set `{open}` (mask `0b10`), over the whole
/// rectangle of the door.
pub const SLICE_BINDINGS: [BindingRec; 2] = [
    BindingRec::door(0, 0b10, (15, 20, 15, 22)),
    BindingRec::door(1, 0b10, (22, 38, 22, 40)),
];

const RED_RECT: (u32, u32) = RuleRec::union_rect(15, 20, 15, 22);
const BLUE_RECT: (u32, u32) = RuleRec::union_rect(22, 38, 22, 40);

/// The slice's rules: the red door's lock, then the blue door's. The order is the order the sim evaluates them, which is the
/// order Fizz 1 tries the door colours. Each is what `lock red-door key=red` lowers to: a touch rule over the door's whole
/// rectangle that, while the door's state is in `{closed}` and Ben holds the key, sets the state to `open` and spends the key.
pub const SLICE_RULES: [RuleRec; 2] = [
    RuleRec {
        trigger: TRIGGER_TOUCH,
        u0: RED_RECT.0,
        u1: RED_RECT.1,
        cond: [CondRec::state_in(0, 0b01), CondRec::holds(rules::KEY_RED)],
        act: [ActionRec::set(0, 1), ActionRec::spend_key(rules::KEY_RED)],
        ..RuleRec::EMPTY
    },
    RuleRec {
        trigger: TRIGGER_TOUCH,
        u0: BLUE_RECT.0,
        u1: BLUE_RECT.1,
        cond: [CondRec::state_in(1, 0b01), CondRec::holds(rules::KEY_BLUE)],
        act: [ActionRec::set(1, 1), ActionRec::spend_key(rules::KEY_BLUE)],
        ..RuleRec::EMPTY
    },
];

/// The slice as a [`LevelSource`]: the data every world build goes through. Both doors start closed.
pub const SLICE_SOURCE: LevelSource<'static> = LevelSource {
    name: SLICE.name,
    w: W as u16,
    h: H as u16,
    tiles: &SLICE.tiles,
    floor_solid: false,
    spawn: SPAWN,
    states: &SLICE_STATES,
    bindings: &SLICE_BINDINGS,
    objects: &SLICE_OBJECTS,
    rules: &SLICE_RULES,
    strings: &[],
};

/// The source's states and door bindings read against its tiles.
fn doors_of(src: &LevelSource) -> Result<Doors, SourceError> {
    Doors::from_recs(
        src.states,
        src.bindings,
        src.tiles,
        src.w,
        src.h,
        &tiles::props(),
    )
    .map_err(SourceError::Door)
}

/// Checks what the pack cannot: the tile list has `w * h` bytes, the objects come in ascending kind order, there is at most
/// one key of each colour, every chicken direction is 1 or -1, every state and door binding can be built (see [`Doors`]), every
/// rule can be built (see [`rules::Rule::from_rec`]) and every string has a length and a style the pack accepts. (A snack's kind
/// and the object kinds the rules cannot build are excluded by the [`Object`] type.)
pub fn validate(src: &LevelSource) -> Result<(), SourceError> {
    if src.w == 0 || src.h == 0 || src.tiles.len() != usize::from(src.w) * usize::from(src.h) {
        return Err(SourceError::BadSize);
    }
    let mut last = 0;
    let (mut red, mut blue) = (false, false);
    for o in src.objects {
        let kind = o.kind();
        if kind < last {
            return Err(SourceError::KindOrder);
        }
        last = kind;
        match *o {
            Object::KeyRed { .. } if core::mem::replace(&mut red, true) => {
                return Err(SourceError::SecondKey);
            }
            Object::KeyBlue { .. } if core::mem::replace(&mut blue, true) => {
                return Err(SourceError::SecondKey);
            }
            Object::Chicken { dir, .. } if dir != 1.0 && dir != -1.0 => {
                return Err(SourceError::ChickenDir);
            }
            _ => {}
        }
    }
    let doors = doors_of(src)?;
    RuleTable::from_recs(src.rules, &doors).map_err(SourceError::Rule)?;
    check_strings(src.strings)
}

fn check_strings(strings: &[StringRec]) -> Result<(), SourceError> {
    if strings.len() > MAX_STRINGS {
        return Err(SourceError::BadString);
    }
    for s in strings {
        if s.len == 0 || usize::from(s.len) > STRING_TEXT_MAX || s.style >= STYLE_COUNT {
            return Err(SourceError::BadString);
        }
    }
    Ok(())
}

/// The digest of a level: FNV-1a 64 (the state hash's function) over, little-endian and in this order, `w` and `h` as `u16`,
/// the tile bytes, `floor_solid` as one byte, the bits of the spawn x and y, the state count (one byte) and each state's 8-byte
/// record, the binding count (one byte) and each binding's 16-byte record, then for each object its record fields `kind`, `sub`,
/// `flags` (`u16`), `aux` (`u32`) and the bits of `x`, `y` and `p`; then the rule count (one byte) and each rule's 48-byte
/// record, then the string count (one byte) and each string's 32-byte record. The name is not included. A baked level and the
/// `const` level are the same level when their digests agree.
pub fn digest(src: &LevelSource) -> u64 {
    let mut h = StateHash::new();
    h.write_u8(src.w as u8);
    h.write_u8((src.w >> 8) as u8);
    h.write_u8(src.h as u8);
    h.write_u8((src.h >> 8) as u8);
    for &t in src.tiles {
        h.write_u8(t);
    }
    h.write_u8(u8::from(src.floor_solid));
    h.write_f64(src.spawn.0);
    h.write_f64(src.spawn.1);
    h.write_u8(src.states.len() as u8);
    for s in src.states {
        for b in s.to_bytes() {
            h.write_u8(b);
        }
    }
    h.write_u8(src.bindings.len() as u8);
    for r in src.bindings {
        for b in r.to_bytes() {
            h.write_u8(b);
        }
    }
    for o in src.objects {
        let r = o.to_rec();
        h.write_u8(r.kind);
        h.write_u8(r.sub);
        h.write_u8(r.flags as u8);
        h.write_u8((r.flags >> 8) as u8);
        h.write_u32(r.aux);
        h.write_u64(r.x_bits);
        h.write_u64(r.y_bits);
        h.write_u64(r.p_bits);
    }
    h.write_u8(src.rules.len() as u8);
    for r in src.rules {
        for b in r.to_bytes() {
            h.write_u8(b);
        }
    }
    h.write_u8(src.strings.len() as u8);
    for s in src.strings {
        for b in s.to_bytes() {
            h.write_u8(b);
        }
    }
    h.finish()
}

/// The level's records, in the order `World::set_level` takes them.
struct Records<'a> {
    states: &'a [StateRec],
    bindings: &'a [BindingRec],
    rules: &'a [RuleRec],
}

fn build(
    (w, h): (u16, u16),
    tile_bytes: &[u8],
    floor_solid: bool,
    spawn: (f64, f64),
    objects: impl Iterator<Item = Object>,
    recs: Records,
) -> Result<World, SourceError> {
    let mut m = TileMap::new(i32::from(w), i32::from(h), tiles::props());
    m.data.copy_from_slice(tile_bytes);
    m.floor_solid = floor_solid;
    let mut world = World::new(m, spawn);
    for o in objects {
        match o {
            Object::KeyRed { x, y } => world.set_key(x, y),
            Object::KeyBlue { x, y } => world.set_key_blue(x, y),
            Object::Snack { kind, x, y } => world.add_snack(kind, x, y),
            Object::Chicken { x, y, dir } => world.add_chicken(x, y, dir),
        }
    }
    // Every state starts at its start value and the doors' bits follow; a death restores both.
    world.set_level(recs.states, recs.bindings, recs.rules)?;
    Ok(world)
}

/// A fresh world built from level data: Ben at the spawn, the objects in place in record order, every state at its start value
/// and each door's derived bit set from it.
pub fn world_from(src: &LevelSource) -> Result<World, SourceError> {
    validate(src)?;
    build(
        (src.w, src.h),
        src.tiles,
        src.floor_solid,
        src.spawn,
        src.objects.iter().copied(),
        Records {
            states: src.states,
            bindings: src.bindings,
            rules: src.rules,
        },
    )
}

/// A fresh world built from a `LEVL` section the pack reader has accepted. Refuses the object kinds the rules cannot build (the
/// switch and the light among them), an unknown snack kind, a bad chicken direction, a second key of a colour, a binding that is
/// not a door binding, switched tiles that no door binding covers, and every rule the rule phase cannot run: the inputs other
/// than `touch` over a rectangle and `enter` fail with `SourceError::Rule(RuleError::Unbuilt(trigger))`, and an unbuilt condition,
/// action or delay with the matching `RuleError` (see `rules.rs`). The level's strings are checked by the reader and not used yet.
pub fn world_from_view(view: &LevelView) -> Result<World, SourceError> {
    let mut objects = alloc::vec::Vec::with_capacity(view.object_count());
    let (mut red, mut blue) = (false, false);
    for r in view.objects() {
        let o = Object::from_rec(&r)?;
        match o {
            Object::KeyRed { .. } if core::mem::replace(&mut red, true) => {
                return Err(SourceError::SecondKey);
            }
            Object::KeyBlue { .. } if core::mem::replace(&mut blue, true) => {
                return Err(SourceError::SecondKey);
            }
            _ => {}
        }
        objects.push(o);
    }
    let (w, h) = (view.width() as u16, view.height() as u16);
    let states: alloc::vec::Vec<StateRec> = view.states().collect();
    let bindings: alloc::vec::Vec<BindingRec> = view.bindings().collect();
    let recs: alloc::vec::Vec<RuleRec> = view.rules().collect();
    build(
        (w, h),
        view.tiles(),
        view.floor_solid(),
        view.spawn(),
        objects.into_iter(),
        Records {
            states: &states,
            bindings: &bindings,
            rules: &recs,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_is_32_by_48_and_the_bake_flips_the_rows() {
        let m = map();
        assert_eq!((m.w, m.h), (32, 48));
        // The bottom text row is the solid floor, and the top one is open sky left of the door's lintel.
        assert!((0..32).all(|x| m.get(x, 0) == FILL));
        assert!((0..22).all(|x| m.get(x, 47) == EMPTY));
    }

    #[test]
    fn the_named_places_match_the_tiles() {
        let m = map();
        assert!(DOOR.iter().all(|&(x, y)| m.get(x, y) == DOOR_R));
        assert!(EXIT_TILES.iter().all(|&(x, y)| m.get(x, y) == EXIT));
        let count = |t: u8| m.data.iter().filter(|&&v| v == t).count();
        assert_eq!(count(DOOR_R), DOOR.len());
        assert_eq!(count(EXIT), EXIT_TILES.len());
        assert!(BLUE_DOOR.iter().all(|&(x, y)| m.get(x, y) == DOOR_B));
        assert_eq!(count(DOOR_B), BLUE_DOOR.len());
        assert!(SHELF.iter().all(|&(x, y)| m.get(x, y) == BLOCK));
        // Ben starts on the ground floor and the chicken on the long ledge, both on solid or one-way footing.
        assert_eq!(m.get(SPAWN.0 as i32, SPAWN.1 as i32 - 1), FILL);
        assert_eq!(m.get(CHICKEN.0 as i32, CHICKEN.1 as i32 - 1), PLAT);
        assert_eq!(m.get(KEY.0 as i32, KEY.1 as i32 - 1), PLAT);
    }

    #[test]
    fn the_cans_lie_on_solid_footing_in_the_nook_after_the_snacks() {
        let w = world();
        let cans: alloc::vec::Vec<_> = SNACKS.iter().filter(|s| s.0 == SnackKind::Soda).collect();
        assert_eq!(cans.len(), 2, "one on the gallery floor, one on the shelf");
        // The list order is the hash order: the snacks keep their places and the cans come last.
        assert!(SNACKS[..5].iter().all(|s| s.0 != SnackKind::Soda));
        assert!(SNACKS[5..7].iter().all(|s| s.0 == SnackKind::Soda));
        assert_eq!(SNACKS[7], ELIXIR, "the elixir is last");
        assert_eq!(w.snacks.len(), SNACKS.len());
        let m = map();
        for &&(_, x, y) in &cans {
            // A can's centre is 0.7 above the tile it lies on, as the keys' are.
            let (tx, ty) = (x as i32, (y - 0.7) as i32);
            assert!(
                m.solid(tx, ty - 1, false, 0.0) || m.get(tx, ty - 1) == PLAT,
                "{x}, {y}"
            );
            assert_eq!(m.get(tx, ty), EMPTY, "{x}, {y}");
        }
    }

    #[test]
    fn the_elixir_lies_on_the_second_ledge_off_the_route_and_a_walk_takes_it() {
        use crate::world::{Event, START_LIVES};
        use fizz_sim::{Buttons, InputFrame};
        let m = map();
        let (_, lx, ly) = ELIXIR;
        // On the second ledge of the left shaft (row 7, columns 8 to 14), at its right end by the wall, 0.7 above the planks.
        let (tx, ty) = (lx as i32, (ly - 0.7 + 0.001) as i32);
        assert_eq!((tx, ty), (13, 8));
        assert_eq!(m.get(tx, ty - 1), PLAT);
        assert!(!m.solid(tx, ty, false, 0.0), "open room above the ledge");
        // Ben standing on the ledge and walking right takes it, once, and gains a life.
        let mut w = world();
        w.ben.b.x = 9.0;
        w.ben.b.px = 9.0;
        w.ben.b.y = 8.0;
        w.ben.b.py = 8.0;
        w.ben.b.on_ground = true;
        let mut prev = Buttons::NONE;
        let mut lives = alloc::vec::Vec::new();
        for _ in 0..120 {
            w.step(InputFrame::from_transition(prev, Buttons::RIGHT));
            prev = Buttons::RIGHT;
            for e in w.events() {
                if let Event::Life { x, y } = *e {
                    lives.push((w.tick.0, x, y));
                }
            }
        }
        assert_eq!(lives.len(), 1, "{lives:?}");
        assert_eq!(w.lives, START_LIVES + 1);
        assert_eq!(w.score(), 0);
        // The replay's route never goes near it: the whole script takes no elixir (also checked in `replay.rs`).
        assert_eq!(world().lives, START_LIVES);
    }

    #[test]
    fn the_door_starts_closed_and_blocks_the_way() {
        let w = world();
        assert!(w.map.switch(tiles::DOOR_RED_CHANNEL));
        assert!(w.map.solid(DOOR[0].0, DOOR[0].1, false, 0.0));
        assert!(w.key_in_world());
        assert_eq!(w.chickens.len(), 1);
    }

    extern crate std;
    use fizz_pack::level::{KIND_LIGHT, SWITCH_USE, TRIGGER_ENTER, TRIGGER_START, TRIGGER_TIME};
    use fizz_pack::write::{LevelSpec, encode_level};
    use std::string::String;
    use std::vec::Vec;

    /// The world as the hand-written calls built it before levels were data: the oracle `world()` must equal.
    fn legacy_world() -> World {
        let mut w = World::new(map(), SPAWN);
        w.set_key(KEY.0, KEY.1);
        w.set_key_blue(KEY_BLUE.0, KEY_BLUE.1);
        for &(kind, x, y) in &SNACKS {
            w.add_snack(kind, x, y);
        }
        w.add_chicken(CHICKEN.0, CHICKEN.1, CHICKEN.2);
        w.set_level(&SLICE_STATES, &SLICE_BINDINGS, &SLICE_RULES)
            .unwrap();
        // Both doors start closed: each door's derived bit is set.
        assert_eq!(w.map.switches, tiles::DOOR_RED_BIT | tiles::DOOR_BLUE_BIT);
        w
    }

    fn assert_same_world(a: &World, b: &World) {
        assert_eq!(a.hash().finish(), b.hash().finish());
        assert_eq!(a.map.data, b.map.data);
        assert_eq!((a.map.w, a.map.h), (b.map.w, b.map.h));
        assert_eq!(a.map.switches, b.map.switches);
        assert_eq!(a.map.floor_solid, b.map.floor_solid);
        for (x, y) in (0..a.map.w).flat_map(|x| (0..a.map.h).map(move |y| (x, y))) {
            assert_eq!(a.map.switch_bit(x, y), b.map.switch_bit(x, y), "{x}, {y}");
        }
        assert_eq!(a.snacks, b.snacks);
        assert_eq!(a.chickens.len(), b.chickens.len());
        for (x, y) in a.chicken_views().zip(b.chicken_views()) {
            assert_eq!((x.x, x.y, x.dir), (y.x, y.y, y.dir));
        }
        assert_eq!(a.key_position(), b.key_position());
        assert_eq!(a.key_blue_position(), b.key_blue_position());
        assert_eq!(a.spawn(), b.spawn());
        assert_eq!(a.rules(), b.rules());
        assert_eq!(a.start_switches(), b.start_switches());
    }

    #[test]
    fn the_data_world_equals_the_hand_built_one() {
        let (a, b) = (world(), legacy_world());
        assert_same_world(&a, &b);
        assert_eq!(a.map.switches, 0b11);
        assert_eq!(a.spawn(), SPAWN);
    }

    #[test]
    fn the_doors_are_derived_from_the_bindings() {
        let doors = slice_doors();
        let d = doors.doors();
        assert_eq!(d.len(), 2);
        assert_eq!(
            (d[0].bit, d[0].rect, d[0].mask),
            (tiles::DOOR_RED_CHANNEL, (15, 20, 15, 22), 0b10)
        );
        assert_eq!(
            (d[1].bit, d[1].rect, d[1].mask),
            (tiles::DOOR_BLUE_CHANNEL, (22, 38, 22, 40), 0b10)
        );
        assert_eq!(doors.start_switches(), 0b11);
        // Each door's cells are the tiles the slice draws as its door.
        let m = map();
        for &(x, y) in &DOOR {
            assert_eq!(m.switch_bit(x, y), Some(tiles::DOOR_RED_CHANNEL));
        }
        for &(x, y) in &BLUE_DOOR {
            assert_eq!(m.switch_bit(x, y), Some(tiles::DOOR_BLUE_CHANNEL));
        }
        assert_eq!(m.switch_bit(14, 21), None);
        assert_eq!(m.switch_bit(15, 19), None);
        assert_eq!(m.switch_bit(15, 23), None);
    }

    #[test]
    fn the_slice_digest_is_pinned() {
        // Moved from 0xbd2e_72e6_f2ca_89fa on purpose (the digest with the elixir): the digest now covers the states and bindings in place of the switch word, the
        // objects' `aux` in place of their `channel`, and 48-byte rule records. Recomputed independently in Python
        // (`tools/levl2-slice.py`).
        assert_eq!(digest(&SLICE_SOURCE), 0xeb4c_6efa_20f3_875c);
    }

    fn one_string(style: u8) -> &'static [StringRec] {
        std::boxed::Box::leak(std::boxed::Box::new([StringRec::new(style, "hi")]))
    }

    const SWAPPED_RULES: [RuleRec; 2] = [SLICE_RULES[1], SLICE_RULES[0]];
    const MOVED_RECT_RULES: [RuleRec; 2] = [
        RuleRec {
            u0: RuleRec::union_rect(15, 21, 15, 22).0,
            ..SLICE_RULES[0]
        },
        SLICE_RULES[1],
    ];
    const OTHER_START: [StateRec; 2] = [StateRec::new(2, 1), StateRec::new(2, 0)];
    const THREE_VALUES: [StateRec; 2] = [StateRec::new(3, 0), StateRec::new(2, 0)];
    const OTHER_MASK: [BindingRec; 2] = [
        BindingRec::door(0, 0b110, (15, 20, 15, 22)),
        SLICE_BINDINGS[1],
    ];

    /// The slice source with `edit` applied to owned copies of its tiles and objects.
    fn changed(edit: impl FnOnce(&mut Vec<u8>, &mut Vec<Object>, &mut LevelSource)) -> u64 {
        let mut tiles = SLICE_SOURCE.tiles.to_vec();
        let mut objects = SLICE_SOURCE.objects.to_vec();
        let mut src = SLICE_SOURCE;
        edit(&mut tiles, &mut objects, &mut src);
        src.tiles = &tiles;
        src.objects = &objects;
        digest(&src)
    }

    #[test]
    fn every_field_of_the_level_moves_the_digest() {
        let base = digest(&SLICE_SOURCE);
        assert_eq!(changed(|_, _, _| {}), base);
        assert_ne!(changed(|t, _, _| t[100] ^= 1), base);
        assert_ne!(
            changed(|_, o, _| o[3] = Object::Snack {
                kind: SnackKind::Choc,
                x: 27.5,
                y: 40.2
            }),
            base
        );
        assert_ne!(
            changed(|_, o, _| o[2] = Object::Snack {
                kind: SnackKind::Cheezie,
                x: 25.5,
                y: 40.25
            }),
            base
        );
        assert_ne!(
            changed(|_, o, _| o.swap(2, 3)),
            base,
            "the order of two snacks is part of the level"
        );
        assert_ne!(changed(|_, _, s| s.states = &OTHER_START), base);
        assert_ne!(changed(|_, _, s| s.states = &THREE_VALUES), base);
        assert_ne!(changed(|_, _, s| s.states = &SLICE_STATES[..1]), base);
        assert_ne!(changed(|_, _, s| s.bindings = &OTHER_MASK), base);
        assert_ne!(changed(|_, _, s| s.bindings = &SLICE_BINDINGS[..1]), base);
        assert_ne!(changed(|_, _, s| s.rules = &SLICE_RULES[..1]), base);
        assert_ne!(
            changed(|_, _, s| s.rules = &SWAPPED_RULES),
            base,
            "the order of two rules is part of the level"
        );
        assert_ne!(changed(|_, _, s| s.rules = &MOVED_RECT_RULES), base);
        assert_ne!(changed(|_, _, s| s.strings = one_string(0)), base);
        assert_ne!(
            changed(|_, _, s| s.strings = one_string(2)),
            changed(|_, _, s| s.strings = one_string(0)),
        );
        assert_ne!(changed(|_, _, s| s.floor_solid = true), base);
        assert_ne!(changed(|_, _, s| s.spawn.0 = 2.5), base);
        assert_eq!(
            changed(|_, _, s| s.name = "Another name"),
            base,
            "the name is not part of the digest"
        );
    }

    #[test]
    fn validate_refuses_what_the_pack_cannot() {
        let ok = |objects: &[Object]| {
            validate(&LevelSource {
                objects,
                ..SLICE_SOURCE
            })
        };
        let chicken = Object::Chicken {
            x: 8.0,
            y: 11.0,
            dir: -1.0,
        };
        let red = Object::KeyRed { x: 2.5, y: 11.7 };
        assert_eq!(ok(&SLICE_OBJECTS), Ok(()));
        assert_eq!(ok(&[chicken, red]), Err(SourceError::KindOrder));
        assert_eq!(ok(&[red, red]), Err(SourceError::SecondKey));
        assert_eq!(
            ok(&[Object::Chicken {
                x: 8.0,
                y: 11.0,
                dir: 0.5
            }]),
            Err(SourceError::ChickenDir)
        );
        let short = LevelSource {
            tiles: &SLICE.tiles[1..],
            ..SLICE_SOURCE
        };
        assert_eq!(validate(&short), Err(SourceError::BadSize));
    }

    #[test]
    fn validate_refuses_states_and_bindings_the_world_cannot_build() {
        let with = |states: &[StateRec], bindings: &[BindingRec]| {
            validate(&LevelSource {
                states,
                bindings,
                ..SLICE_SOURCE
            })
        };
        let door = |state, mask, rect| BindingRec::door(state, mask, rect);
        assert_eq!(with(&SLICE_STATES, &SLICE_BINDINGS), Ok(()));
        let d = |e| Err(SourceError::Door(e));
        assert_eq!(
            with(&[StateRec::new(1, 0), SLICE_STATES[1]], &SLICE_BINDINGS),
            d(DoorError::State(0))
        );
        assert_eq!(
            with(&[StateRec::new(2, 2), SLICE_STATES[1]], &SLICE_BINDINGS),
            d(DoorError::State(0))
        );
        // A binding to a state that is not there, an empty or too wide mask, an inverted or out-of-map rect.
        assert_eq!(
            with(
                &SLICE_STATES,
                &[door(2, 0b10, (15, 20, 15, 22)), SLICE_BINDINGS[1]]
            ),
            d(DoorError::Binding(0))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[door(0, 0, (15, 20, 15, 22)), SLICE_BINDINGS[1]]
            ),
            d(DoorError::Binding(0))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[door(0, 0b100, (15, 20, 15, 22)), SLICE_BINDINGS[1]]
            ),
            d(DoorError::Binding(0))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[SLICE_BINDINGS[0], door(1, 0b10, (22, 40, 22, 38))]
            ),
            d(DoorError::Binding(1))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[SLICE_BINDINGS[0], door(1, 0b10, (22, 38, 32, 40))]
            ),
            d(DoorError::Binding(1))
        );
        // A binding that is not a door binding.
        let light = BindingRec {
            kind: fizz_pack::level::BIND_LIGHT,
            target: 0,
            rect: (0, 0, 0, 0),
            ..SLICE_BINDINGS[0]
        };
        assert_eq!(
            with(&SLICE_STATES, &[light, SLICE_BINDINGS[1]]),
            d(DoorError::Binding(0))
        );
        // A rect over a tile that is not switched, a door's cell left unbound, and two doors on one cell.
        assert_eq!(
            with(
                &SLICE_STATES,
                &[door(0, 0b10, (15, 20, 15, 23)), SLICE_BINDINGS[1]]
            ),
            d(DoorError::NotSwitched(0))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[door(0, 0b10, (15, 21, 15, 22)), SLICE_BINDINGS[1]]
            ),
            d(DoorError::Unbound(15, 20))
        );
        assert_eq!(
            with(&SLICE_STATES, &SLICE_BINDINGS[..1]),
            d(DoorError::Unbound(22, 38))
        );
        assert_eq!(
            with(
                &SLICE_STATES,
                &[SLICE_BINDINGS[0], door(1, 0b10, (15, 20, 15, 22))]
            ),
            d(DoorError::Overlap(1))
        );
    }

    fn encoded(src: &LevelSource) -> Vec<u8> {
        let recs: Vec<ObjectRec> = src.objects.iter().map(Object::to_rec).collect();
        encode_level(&LevelSpec {
            width: src.w,
            height: src.h,
            floor_solid: src.floor_solid,
            spawn_x_bits: src.spawn.0.to_bits(),
            spawn_y_bits: src.spawn.1.to_bits(),
            name: src.name,
            states: src.states,
            bindings: src.bindings,
            objects: &recs,
            rules: src.rules,
            strings: src.strings,
            tiles: src.tiles,
        })
        .unwrap()
    }

    #[test]
    fn the_slice_levl_is_2080_bytes_as_the_worked_example_says() {
        // The worked example in docs/FAP.md: the header 32, the name 16, the states 16, the bindings 32, the objects 352, the rules
        // 96 and the tiles 1,536.
        assert_eq!(
            encoded(&SLICE_SOURCE).len(),
            32 + 16 + 8 * 2 + 16 * 2 + 32 * 11 + 48 * 2 + 32 * 48
        );
        assert_eq!(encoded(&SLICE_SOURCE).len(), 2080);
        assert_eq!(SLICE.name.len(), 16);
        // The same bytes `tools/levl2-slice.py` writes from the document's description.
        assert_eq!(fizz_pack::fnv(&encoded(&SLICE_SOURCE)), 0x00AF_95F7);
        assert_eq!(
            encoded(&SLICE_SOURCE).len(),
            fizz_pack::level::level_len(16, 2, 2, 11, 2, 0, 32 * 48)
        );
    }

    #[test]
    fn the_slice_levl_bytes_are_the_documented_ones() {
        let b = encoded(&SLICE_SOURCE);
        let states_at = 32 + 16;
        assert_eq!(
            b[states_at..states_at + 16],
            [2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]
        );
        let bindings_at = states_at + 16;
        assert_eq!(
            b[bindings_at..bindings_at + 16],
            [1, 0, 2, 0, 0xFF, 0xFF, 0, 0, 15, 0, 20, 0, 15, 0, 22, 0]
        );
        assert_eq!(
            b[bindings_at + 16..bindings_at + 32],
            [1, 1, 2, 0, 0xFF, 0xFF, 0, 0, 22, 0, 38, 0, 22, 0, 40, 0]
        );
        let rules_at = bindings_at + 32 + 352;
        assert_eq!(
            b[rules_at..rules_at + 48],
            [
                0x09, 0, 0, 0, 0xFF, 0xFF, 0,
                0, // header: touch, no repeat, no delay, no subject, no arg
                15, 0, 20, 0, 15, 0, 22, 0, // the door's whole rect
                1, 0, 1, 0, // state 0 is in {closed}
                2, 0, 0, 0, // holds key colour 0
                1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, // set state 0 to 1
                4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // spend key colour 0
            ]
        );
        assert_eq!(
            b[rules_at + 48..rules_at + 56],
            [0x09, 0, 0, 0, 0xFF, 0xFF, 0, 0]
        );
        assert_eq!(
            b[rules_at + 56..rules_at + 64],
            [22, 0, 38, 0, 22, 0, 40, 0]
        );
        assert_eq!(b[rules_at + 64..rules_at + 72], [1, 1, 1, 0, 2, 1, 0, 0]);
        assert_eq!(b[rules_at + 72..rules_at + 76], [1, 1, 1, 0]);
        assert_eq!(b[rules_at + 84..rules_at + 88], [4, 1, 0, 0]);
    }

    #[test]
    fn the_encoded_level_builds_the_same_world_and_transcript() {
        let bytes = encoded(&SLICE_SOURCE);
        let view = LevelView::with_directory(
            &bytes,
            fizz_pack::level::LEVEL_VERSION,
            SLICE_OBJECTS.len() as u16,
        )
        .unwrap();
        // The view's digest: rebuild the source from the records and compare.
        let objects: Vec<Object> = view
            .objects()
            .map(|r| Object::from_rec(&r).unwrap())
            .collect();
        let states: Vec<StateRec> = view.states().collect();
        let bindings: Vec<BindingRec> = view.bindings().collect();
        let rules: Vec<RuleRec> = view.rules().collect();
        let strings: Vec<StringRec> = view.strings().collect();
        assert_eq!(states, SLICE_STATES);
        assert_eq!(bindings, SLICE_BINDINGS);
        assert_eq!(rules, SLICE_RULES);
        let src = LevelSource {
            name: SLICE.name,
            w: view.width() as u16,
            h: view.height() as u16,
            tiles: view.tiles(),
            floor_solid: view.floor_solid(),
            spawn: view.spawn(),
            states: &states,
            bindings: &bindings,
            objects: &objects,
            rules: &rules,
            strings: &strings,
        };
        assert_eq!(digest(&src), digest(&SLICE_SOURCE));
        let w = world_from_view(&view).unwrap();
        assert_same_world(&w, &world());
        let mut t = String::new();
        crate::replay::replay_from(&mut t, w).unwrap();
        assert_eq!(t, include_str!("../slice-replay.txt"));
    }

    #[test]
    fn the_golden_replay_opens_the_red_door_from_its_foot_and_the_blue_from_its_top() {
        // The lock is a touch over the door's whole rect, so any cell of the door's run takes the key. The golden run opens the red
        // door with Ben's front probe on its bottom cell (15, 20) and the blue door with the probe on its top cell (22, 40). The
        // `dooropen` event is at the door's centre cell, (15, 21) and (22, 39), in sixteenths of its centre.
        let transcript = include_str!("../slice-replay.txt");
        let mut w = world();
        let mut prev = fizz_sim::Buttons::NONE;
        let mut probes = Vec::new();
        for &(frames, now) in crate::replay::SLICE_SCRIPT {
            for _ in 0..frames {
                w.step(fizz_sim::InputFrame::from_transition(prev, now));
                prev = now;
                for e in w.events() {
                    if let crate::Event::DoorOpen { x, y } = *e {
                        let b = &w.ben.b;
                        let cx = if w.ben.face > 0.0 {
                            (b.x + b.w + 0.1).floor() as i32
                        } else {
                            (b.x - 0.1).floor() as i32
                        };
                        probes.push(((cx, (b.y + 0.2).floor() as i32), (x, y)));
                    }
                }
            }
        }
        assert_eq!(probes, [((15, 20), (248, 344)), ((22, 40), (360, 632))]);
        assert_eq!((DOOR[0], BLUE_DOOR[2]), ((15, 20), (22, 40)));
        assert!(
            transcript.contains("\nFINAL TICK 1800 HASH "),
            "and the run finishes"
        );
    }

    #[test]
    fn validate_refuses_rules_and_strings_the_rules_cannot_build() {
        let with = |rules: &[RuleRec], strings: &[StringRec]| {
            validate(&LevelSource {
                rules,
                strings,
                ..SLICE_SOURCE
            })
        };
        assert_eq!(with(&SLICE_RULES, &[]), Ok(()));
        let start = RuleRec {
            trigger: TRIGGER_START,
            act: [ActionRec::set(0, 1), ActionRec::EMPTY],
            ..RuleRec::EMPTY
        };
        assert_eq!(
            with(&[start], &[]),
            Err(SourceError::Rule(RuleError::Unbuilt(TRIGGER_START)))
        );
        let green = RuleRec {
            cond: [SLICE_RULES[0].cond[0], CondRec::holds(rules::KEY_GREEN)],
            act: [
                SLICE_RULES[0].act[0],
                ActionRec::spend_key(rules::KEY_GREEN),
            ],
            ..SLICE_RULES[0]
        };
        assert_eq!(
            with(&[green], &[]),
            Err(SourceError::Rule(RuleError::KeyColour))
        );
        // A touch rule that is not the lock sugar is still a rule: the red door's lock over the blue door's rect.
        let wrong_rect = RuleRec {
            u0: SLICE_RULES[1].u0,
            u1: SLICE_RULES[1].u1,
            ..SLICE_RULES[0]
        };
        assert_eq!(with(&[wrong_rect], &[]), Ok(()));
        let too_wide = RuleRec {
            u0: RuleRec::union_rect(0, 0, 32, 0).0,
            u1: RuleRec::union_rect(0, 0, 32, 0).1,
            ..SLICE_RULES[0]
        };
        assert_eq!(
            with(&[too_wide], &[]),
            Err(SourceError::Rule(RuleError::Shape))
        );
        let long = StringRec {
            len: 31,
            ..StringRec::new(0, "x")
        };
        let styled = StringRec {
            style: 7,
            ..StringRec::new(0, "x")
        };
        assert_eq!(with(&[], &[long]), Err(SourceError::BadString));
        assert_eq!(with(&[], &[styled]), Err(SourceError::BadString));
        assert_eq!(with(&[], &[StringRec::new(6, "ok")]), Ok(()));
    }

    /// The slice's bytes, with `rules` in place of its two locks and `objects` in place of its objects.
    fn view_world(objects: &[ObjectRec], rules: &[RuleRec]) -> Result<usize, SourceError> {
        let bytes = encode_level(&LevelSpec {
            width: SLICE_SOURCE.w,
            height: SLICE_SOURCE.h,
            floor_solid: false,
            spawn_x_bits: SPAWN.0.to_bits(),
            spawn_y_bits: SPAWN.1.to_bits(),
            name: "t",
            states: &SLICE_STATES,
            bindings: &SLICE_BINDINGS,
            objects,
            rules,
            strings: &[],
            tiles: SLICE_SOURCE.tiles,
        })
        .unwrap();
        let view = LevelView::with_directory(&bytes, 2, objects.len() as u16).unwrap();
        world_from_view(&view).map(|w| w.rules().len())
    }

    #[test]
    fn the_view_loader_builds_enter_rows_and_refuses_inputs_that_have_no_phase_yet() {
        let act = [ActionRec::set(0, 1), ActionRec::EMPTY];
        let (u0, u1) = RuleRec::union_rect(1, 1, 2, 2);
        assert_eq!(view_world(&[], &SLICE_RULES), Ok(2));
        let enter = RuleRec {
            trigger: TRIGGER_ENTER,
            u0,
            u1,
            act,
            ..RuleRec::EMPTY
        };
        assert_eq!(view_world(&[], &[enter]), Ok(1));
        for rec in [
            RuleRec {
                trigger: TRIGGER_START,
                act,
                ..RuleRec::EMPTY
            },
            RuleRec {
                trigger: TRIGGER_TIME,
                u0: 10,
                u1: 0,
                act,
                ..RuleRec::EMPTY
            },
        ] {
            assert_eq!(
                view_world(&[], &[rec]),
                Err(SourceError::Rule(RuleError::Unbuilt(rec.trigger)))
            );
        }
    }

    #[test]
    fn the_view_loader_refuses_what_the_rules_cannot_build() {
        let red = SLICE_OBJECTS[0].to_rec();
        assert_eq!(view_world(&[red], &[]), Ok(0));
        assert_eq!(view_world(&[red, red], &[]), Err(SourceError::SecondKey));
        // Kinds the pack accepts and the world has no consumer for: a switch and a light.
        let switch = ObjectRec {
            kind: fizz_pack::level::KIND_SWITCH,
            sub: 1,
            flags: SWITCH_USE,
            ..red
        };
        let light = ObjectRec {
            kind: KIND_LIGHT,
            sub: 3,
            flags: 0,
            aux: u32::from(fizz_pack::level::ATTACH_NONE) << 16,
            p_bits: 4.0f64.to_bits(),
            ..red
        };
        for o in [switch, light] {
            assert_eq!(view_world(&[o], &[]), Err(SourceError::ReservedKind));
        }
        // An unknown snack kind never reaches the loader through a view (the pack refuses it), but the record form is checked as well.
        let snack = SLICE_OBJECTS[2].to_rec();
        assert_eq!(
            Object::from_rec(&ObjectRec { sub: 9, ..snack }),
            Err(SourceError::SnackKind)
        );
    }

    #[test]
    fn a_chicken_with_a_group_is_refused_until_groups_exist() {
        let chicken = SLICE_OBJECTS[10].to_rec();
        assert_eq!(view_world(&[chicken], &[]), Ok(0));
        let grouped = ObjectRec { aux: 1, ..chicken };
        assert_eq!(view_world(&[grouped], &[]), Err(SourceError::BadObject));
    }

    #[test]
    fn a_bit_is_derived_for_each_door_up_to_the_pack_limit() {
        use crate::doors::MAX_DOORS;
        assert_eq!(MAX_DOORS, 32);
        // A strip of MAX_DOORS one-cell doors, then one more.
        let mut props = tiles::props();
        props.set_flags(DOOR_R, fizz_phys::tilemap::SWITCHED);
        let n = MAX_DOORS + 1;
        let tiles_row = alloc::vec![DOOR_R; n];
        let states = alloc::vec![StateRec::new(2, 0); n];
        let bindings: Vec<BindingRec> = (0..n)
            .map(|i| BindingRec::door(i as u8, 0b10, (i as u16, 0, i as u16, 0)))
            .collect();
        let ok = Doors::from_recs(
            &states[..MAX_DOORS],
            &bindings[..MAX_DOORS],
            &tiles_row[..MAX_DOORS],
            MAX_DOORS as u16,
            1,
            &props,
        );
        assert_eq!(ok.unwrap().doors().last().map(|d| d.bit), Some(31));
        // One more door, on a state of its own, is over both limits; the states are counted first.
        let over = Doors::from_recs(&states, &bindings, &tiles_row, n as u16, 1, &props);
        assert_eq!(over, Err(DoorError::TooManyStates));
        // The 33rd door on the 32nd state is over the door limit alone.
        let mut shared = bindings.clone();
        shared[MAX_DOORS].state = (MAX_DOORS - 1) as u8;
        let over = Doors::from_recs(
            &states[..MAX_DOORS],
            &shared,
            &tiles_row,
            n as u16,
            1,
            &props,
        );
        assert_eq!(over, Err(DoorError::TooMany));
    }
}
