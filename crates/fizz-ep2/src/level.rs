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
use fizz_phys::TileMap;

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
    let mut w = World::new(map(), SPAWN);
    w.set_key(KEY.0, KEY.1);
    w.set_key_blue(KEY_BLUE.0, KEY_BLUE.1);
    for &(kind, x, y) in &SNACKS {
        w.add_snack(kind, x, y);
    }
    w.add_chicken(CHICKEN.0, CHICKEN.1, CHICKEN.2);
    w
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
}
