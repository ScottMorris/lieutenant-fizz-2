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

use crate::tiles::{self, BLOCK, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};
use crate::world::World;
use fizz_phys::TileMap;

/// The level's width in tiles.
pub const W: usize = 32;
/// The level's height in tiles.
pub const H: usize = 48;

/// The level as text, top row first (the comment on each row is its `y`).
const ROWS: [&str; H] = [
    "................................", // 47
    "................................", // 46
    "................................", // 45
    "................................", // 44
    "................................", // 43
    "................................", // 42
    "................................", // 41
    "................................", // 40
    "....................E...........", // 39
    "....................E...........", // 38
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

/// A fresh world on the slice level: Ben at the spawn, the key and the chicken in place, the red door closed.
pub fn world() -> World {
    let mut w = World::new(map(), SPAWN);
    w.set_key(KEY.0, KEY.1);
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
        // The bottom text row is the solid floor, and the top one is open sky.
        assert!((0..32).all(|x| m.get(x, 0) == FILL));
        assert!((0..32).all(|x| m.get(x, 47) == EMPTY));
    }

    #[test]
    fn the_named_places_match_the_tiles() {
        let m = map();
        assert!(DOOR.iter().all(|&(x, y)| m.get(x, y) == DOOR_R));
        assert!(EXIT_TILES.iter().all(|&(x, y)| m.get(x, y) == EXIT));
        let count = |t: u8| m.data.iter().filter(|&&v| v == t).count();
        assert_eq!(count(DOOR_R), DOOR.len());
        assert_eq!(count(EXIT), EXIT_TILES.len());
        // Ben starts on the ground floor and the chicken on the long ledge, both on solid or one-way footing.
        assert_eq!(m.get(SPAWN.0 as i32, SPAWN.1 as i32 - 1), FILL);
        assert_eq!(m.get(CHICKEN.0 as i32, CHICKEN.1 as i32 - 1), PLAT);
        assert_eq!(m.get(KEY.0 as i32, KEY.1 as i32 - 1), PLAT);
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
