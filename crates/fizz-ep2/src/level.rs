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

use crate::tiles::{self, BLOCK, DOOR_B, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};
use crate::world::{SnackKind, World};
use fizz_pack::level::{
    KIND_CHICKEN, KIND_ENEMY, KIND_GADGET, KIND_KEY_BLUE, KIND_KEY_RED, KIND_SNACK, KIND_SWITCH,
};
use fizz_pack::{LevelView, ObjectRec};
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
/// The snacks and the soda cans, as `(kind, x, y)` with the centre in tiles, in the order they are added to the world (and so the
/// order of the hash). Two cheezies hang under the shelf, where a jump bumps the head to reach them; on the shelf, from its right
/// end to its left, lie a cheezie, a chocolate and a cookie. Then the cans: one on the gallery floor in the shooting stretch
/// by the right-hand wall, where Ben spends his five shots and needs more, and one on the right end of the shelf.
pub const SNACKS: [(SnackKind, f64, f64); 7] = [
    (SnackKind::Cheezie, 25.5, 40.2),
    (SnackKind::Cheezie, 27.5, 40.2),
    (SnackKind::Cheezie, 27.5, 42.7),
    (SnackKind::Choc, 26.5, 42.7),
    (SnackKind::Cookie, 25.5, 42.7),
    (SnackKind::Soda, 28.5, 38.7),
    (SnackKind::Soda, 28.5, 42.7),
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

/// The slice level's map, with its tile properties.
pub fn map() -> TileMap {
    let mut m = TileMap::new(SLICE.w, SLICE.h, tiles::props());
    m.data.copy_from_slice(&SLICE.tiles);
    m
}

/// A fresh world on the slice level: Ben at the spawn, the keys, the snacks and the chicken in place, both doors closed.
pub fn world() -> World {
    world_from(&SLICE_SOURCE).expect("the slice level is valid")
}

/// What the level can hold besides tiles. The variants are the object kinds of a `LEVL` section that the rules can build;
/// the pack's kinds 5 to 7 (gadget, switch, generic enemy) are reserved and refused.
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

    /// The object as a `LEVL` record: `sub` is the snack kind number, `p` the chicken's direction, `flags` and `channel` zero.
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
            channel: 0,
            x_bits: x.to_bits(),
            y_bits: y.to_bits(),
            p_bits: p.to_bits(),
        }
    }

    /// Reads a record the rules can build. Kinds 5 to 7, an unknown snack `sub`, a chicken direction other than 1 or -1 and
    /// any non-zero `flags`, `channel` or `p` (except a chicken's `p`) are refused.
    pub fn from_rec(r: &ObjectRec) -> Result<Object, SourceError> {
        let (x, y, p) = (r.x(), r.y(), r.p());
        if matches!(r.kind, KIND_GADGET | KIND_SWITCH | KIND_ENEMY) {
            return Err(SourceError::ReservedKind);
        }
        if r.flags != 0 || r.channel != 0 {
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
    /// An object kind of 5 to 7, which the rules cannot build yet.
    ReservedKind,
    /// Any other malformed object record (an unknown kind, or fields that must be zero).
    BadObject,
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
    /// The switch channels that start on.
    pub switches: u32,
    pub floor_solid: bool,
    /// Ben's start: the lower-left corner of his body.
    pub spawn: (f64, f64),
    /// In construction order: keys, then snacks, then chickens, same-kind objects in source order.
    pub objects: &'a [Object],
}

/// The slice's objects in the order the world is built (and so the order of the hash): both keys, the seven snacks and soda
/// cans of [`SNACKS`], and the chicken.
pub const SLICE_OBJECTS: [Object; 10] = [
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
    Object::Chicken {
        x: CHICKEN.0,
        y: CHICKEN.1,
        dir: CHICKEN.2,
    },
];

/// The slice as a [`LevelSource`]: the data every world build goes through. Channels 1 and 2 (the red and blue doors) start
/// on, which is closed.
pub const SLICE_SOURCE: LevelSource<'static> = LevelSource {
    name: SLICE.name,
    w: W as u16,
    h: H as u16,
    tiles: &SLICE.tiles,
    switches: tiles::DOOR_RED_BIT | tiles::DOOR_BLUE_BIT,
    floor_solid: false,
    spawn: SPAWN,
    objects: &SLICE_OBJECTS,
};

/// Checks what the pack cannot: the tile list has `w * h` bytes, the objects come in ascending kind order, there is at most
/// one key of each colour and every chicken direction is 1 or -1. (A snack's kind and the reserved object kinds are
/// excluded by the [`Object`] type.)
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
    Ok(())
}

/// The digest of a level: FNV-1a 64 (the state hash's function) over, little-endian and in this order, `w` and `h` as `u16`,
/// the tile bytes, `switches` as `u32`, `floor_solid` as one byte, the bits of the spawn x and y, then for each object its
/// record fields `kind`, `sub`, `flags` (`u16`), `channel` (`u32`) and the bits of `x`, `y` and `p`. The name is not
/// included. A baked level and the `const` level are the same level when their digests agree.
pub fn digest(src: &LevelSource) -> u64 {
    let mut h = StateHash::new();
    h.write_u8(src.w as u8);
    h.write_u8((src.w >> 8) as u8);
    h.write_u8(src.h as u8);
    h.write_u8((src.h >> 8) as u8);
    for &t in src.tiles {
        h.write_u8(t);
    }
    h.write_u32(src.switches);
    h.write_u8(u8::from(src.floor_solid));
    h.write_f64(src.spawn.0);
    h.write_f64(src.spawn.1);
    for o in src.objects {
        let r = o.to_rec();
        h.write_u8(r.kind);
        h.write_u8(r.sub);
        h.write_u8(r.flags as u8);
        h.write_u8((r.flags >> 8) as u8);
        h.write_u32(r.channel);
        h.write_u64(r.x_bits);
        h.write_u64(r.y_bits);
        h.write_u64(r.p_bits);
    }
    h.finish()
}

fn build(
    (w, h): (u16, u16),
    tile_bytes: &[u8],
    (switches, floor_solid): (u32, bool),
    spawn: (f64, f64),
    objects: impl Iterator<Item = Object>,
) -> World {
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
    // `World::new` and `set_key_blue` still set the door channels; the level has the last word.
    world.map.switches = switches;
    world
}

/// A fresh world built from level data: Ben at the spawn, the objects in place in record order, the level's switches set.
pub fn world_from(src: &LevelSource) -> Result<World, SourceError> {
    validate(src)?;
    Ok(build(
        (src.w, src.h),
        src.tiles,
        (src.switches, src.floor_solid),
        src.spawn,
        src.objects.iter().copied(),
    ))
}

/// A fresh world built from a `LEVL` section the pack reader has accepted. Refuses object kinds 5 to 7, an unknown snack
/// kind, a bad chicken direction and a second key of a colour.
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
    Ok(build(
        (view.width() as u16, view.height() as u16),
        view.tiles(),
        (view.switches(), view.floor_solid()),
        view.spawn(),
        objects.into_iter(),
    ))
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
        assert!(SNACKS[5..].iter().all(|s| s.0 == SnackKind::Soda));
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
    fn the_door_starts_closed_and_blocks_the_way() {
        let w = world();
        assert!(w.map.switch(tiles::DOOR_RED_CHANNEL));
        assert!(w.map.solid(DOOR[0].0, DOOR[0].1, false, 0.0));
        assert!(w.key_in_world());
        assert_eq!(w.chickens.len(), 1);
    }

    extern crate std;
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
        w
    }

    fn assert_same_world(a: &World, b: &World) {
        assert_eq!(a.hash().finish(), b.hash().finish());
        assert_eq!(a.map.data, b.map.data);
        assert_eq!((a.map.w, a.map.h), (b.map.w, b.map.h));
        assert_eq!(a.map.switches, b.map.switches);
        assert_eq!(a.map.floor_solid, b.map.floor_solid);
        assert_eq!(a.snacks, b.snacks);
        assert_eq!(a.chickens.len(), b.chickens.len());
        for (x, y) in a.chicken_views().zip(b.chicken_views()) {
            assert_eq!((x.x, x.y, x.dir), (y.x, y.y, y.dir));
        }
        assert_eq!(a.key_position(), b.key_position());
        assert_eq!(a.key_blue_position(), b.key_blue_position());
        assert_eq!(a.spawn(), b.spawn());
    }

    #[test]
    fn the_data_world_equals_the_hand_built_one() {
        let (a, b) = (world(), legacy_world());
        assert_same_world(&a, &b);
        assert_eq!(a.map.switches, 0b110);
        assert_eq!(a.spawn(), SPAWN);
    }

    #[test]
    fn the_slice_digest_is_pinned() {
        assert_eq!(digest(&SLICE_SOURCE), 0x4326_f908_3ff9_bde6);
    }

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
        assert_ne!(changed(|_, _, s| s.switches = 0b010), base);
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

    fn encoded(src: &LevelSource) -> Vec<u8> {
        let recs: Vec<ObjectRec> = src.objects.iter().map(Object::to_rec).collect();
        encode_level(&LevelSpec {
            width: src.w,
            height: src.h,
            switches: src.switches,
            floor_solid: src.floor_solid,
            spawn_x_bits: src.spawn.0.to_bits(),
            spawn_y_bits: src.spawn.1.to_bits(),
            name: src.name,
            objects: &recs,
            rules: &[],
            strings: &[],
            tiles: src.tiles,
        })
    }

    #[test]
    fn the_slice_levl_is_ten_objects_and_1904_bytes() {
        assert_eq!(encoded(&SLICE_SOURCE).len(), 32 + 16 + 32 * 10 + 32 * 48);
        assert_eq!(encoded(&SLICE_SOURCE).len(), 1904);
    }

    #[test]
    fn the_encoded_level_builds_the_same_world_and_transcript() {
        let bytes = encoded(&SLICE_SOURCE);
        let view = LevelView::with_count(&bytes, SLICE_OBJECTS.len() as u16).unwrap();
        // The view's digest: rebuild the source from the records and compare.
        let objects: Vec<Object> = view
            .objects()
            .map(|r| Object::from_rec(&r).unwrap())
            .collect();
        let src = LevelSource {
            name: SLICE.name,
            w: view.width() as u16,
            h: view.height() as u16,
            tiles: view.tiles(),
            switches: view.switches(),
            floor_solid: view.floor_solid(),
            spawn: view.spawn(),
            objects: &objects,
        };
        assert_eq!(digest(&src), digest(&SLICE_SOURCE));
        let w = world_from_view(&view).unwrap();
        assert_same_world(&w, &world());
        let mut t = String::new();
        crate::replay::replay_from(&mut t, w).unwrap();
        assert_eq!(t, include_str!("../slice-replay.txt"));
    }

    #[test]
    fn the_view_loader_refuses_what_the_rules_cannot_build() {
        let build = |objects: &[ObjectRec]| {
            let bytes = encode_level(&LevelSpec {
                width: SLICE_SOURCE.w,
                height: SLICE_SOURCE.h,
                switches: SLICE_SOURCE.switches,
                floor_solid: false,
                spawn_x_bits: SPAWN.0.to_bits(),
                spawn_y_bits: SPAWN.1.to_bits(),
                name: "t",
                objects,
                rules: &[],
                strings: &[],
                tiles: SLICE_SOURCE.tiles,
            });
            let view = LevelView::with_count(&bytes, objects.len() as u16).unwrap();
            world_from_view(&view).map(|_| ())
        };
        let red = SLICE_OBJECTS[0].to_rec();
        assert_eq!(build(&[red]), Ok(()));
        assert_eq!(build(&[red, red]), Err(SourceError::SecondKey));
        for kind in [KIND_GADGET, KIND_SWITCH, KIND_ENEMY] {
            assert_eq!(
                build(&[ObjectRec { kind, ..red }]),
                Err(SourceError::ReservedKind)
            );
        }
        let snack = SLICE_OBJECTS[2].to_rec();
        assert_eq!(
            build(&[ObjectRec { sub: 9, ..snack }]),
            Err(SourceError::SnackKind)
        );
    }
}
