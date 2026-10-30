// Tile ids and tile properties for Episode 2's levels, kept equal to Fizz 1's ids.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ids and properties follow Fizz 1 `episodes/episode-1/game/src/tiles.rs` at commit `4bce036`, restricted to
// the tiles the slice uses. Departure: the red and blue doors are switched tiles, each on its own channel, rather than solid tiles
// that Fizz 1 rewrites to empty space when it opens.

use fizz_phys::TileProps;
use fizz_phys::tilemap::{
    ONEWAY, SLOPE_L22A, SLOPE_L22B, SLOPE_L45, SLOPE_R22A, SLOPE_R22B, SLOPE_R45, SOLID, SWITCHED,
};

/// Empty space: sky, or air inside a room.
pub const EMPTY: u8 = 0;
/// Solid fill.
pub const FILL: u8 = 1;
/// A solid block.
pub const BLOCK: u8 = 2;
/// A one-way platform: solid only when landed on from above.
pub const PLAT: u8 = 3;
/// Spikes: lethal to the part of Ben below the tile's 0.55 line.
pub const SPIKE: u8 = 4;
/// Lethal chocolate.
pub const CHOC: u8 = 5;
/// A rising 45 degree slope.
pub const R45: u8 = 6;
/// A falling 45 degree slope.
pub const L45: u8 = 7;
/// The first half of a rising 22.5 degree slope.
pub const R22A: u8 = 8;
/// The second half of a rising 22.5 degree slope.
pub const R22B: u8 = 9;
/// The first half of a falling 22.5 degree slope.
pub const L22A: u8 = 10;
/// The second half of a falling 22.5 degree slope.
pub const L22B: u8 = 11;
/// The red door: solid while [`DOOR_RED_CHANNEL`] is on.
pub const DOOR_R: u8 = 12;
/// The blue door: solid while [`DOOR_BLUE_CHANNEL`] is on.
pub const DOOR_B: u8 = 13;
/// The exit: touching it ends the level.
pub const EXIT: u8 = 17;
/// Decorative interior wall behind a room. It is not solid.
pub const WALLBG: u8 = 30;

/// The switch channel of the red door. A set bit means the door is closed.
pub const DOOR_RED_CHANNEL: u8 = 1;
/// The red door's bit in `TileMap::switches`.
pub const DOOR_RED_BIT: u32 = 1 << DOOR_RED_CHANNEL;

/// The switch channel of the blue door. A set bit means the door is closed.
pub const DOOR_BLUE_CHANNEL: u8 = 2;
/// The blue door's bit in `TileMap::switches`.
pub const DOOR_BLUE_BIT: u32 = 1 << DOOR_BLUE_CHANNEL;

/// Whether a tile is lethal liquid. Fizz 1 also counts molten metal, which Episode 2 does not have yet.
pub fn is_liquid(t: u8) -> bool {
    t == CHOC
}

/// Collision behaviour for the tiles above.
pub fn props() -> TileProps {
    let mut p = TileProps::default();
    p.set_flags(FILL, SOLID);
    p.set_flags(BLOCK, SOLID);
    p.set_flags(PLAT, ONEWAY);
    p.set_flags(DOOR_R, SWITCHED);
    p.set_channel(DOOR_R, DOOR_RED_CHANNEL);
    p.set_flags(DOOR_B, SWITCHED);
    p.set_channel(DOOR_B, DOOR_BLUE_CHANNEL);
    for (t, s) in [
        (R45, SLOPE_R45),
        (L45, SLOPE_L45),
        (R22A, SLOPE_R22A),
        (R22B, SLOPE_R22B),
        (L22A, SLOPE_L22A),
        (L22B, SLOPE_L22B),
    ] {
        p.set_slope(t, s);
    }
    p
}

/// The bytes `fizz-pack` continues its schema hash over to make the compatibility number of a world or shared pack
/// (`fizz_pack::compatibility(&pack_content())`). Everything a baked level's meaning depends on and the pack does not hold is here:
/// the tile property table (flags, slope and switch channel for each of the 256 tile ids, in that order), the named tile ids, and the
/// numbers of the snack kinds (the three snacks and the soda can). The object kind numbers belong to `fizz-pack` itself. Changing a flag, a slope, a channel or an id
/// changes the number on purpose, so a pack baked before the change is refused.
pub fn pack_content() -> alloc::vec::Vec<u8> {
    use crate::world::SnackKind;
    let mut v = alloc::vec::Vec::new();
    v.extend_from_slice(b"fizz-ep2-content");
    let p = props();
    for id in 0..=255u8 {
        v.push(p.flags(id));
    }
    for id in 0..=255u8 {
        v.push(p.slope(id));
    }
    for id in 0..=255u8 {
        v.push(p.channel(id));
    }
    v.extend_from_slice(&[
        EMPTY, FILL, BLOCK, PLAT, SPIKE, CHOC, R45, L45, R22A, R22B, L22A, L22B, DOOR_R, DOOR_B,
        EXIT, WALLBG,
    ]);
    v.extend_from_slice(&[
        SnackKind::Cheezie.number(),
        SnackKind::Choc.number(),
        SnackKind::Cookie.number(),
        SnackKind::Soda.number(),
    ]);
    v
}

#[cfg(test)]
mod pack_content_tests {
    use super::*;

    #[test]
    fn the_content_bytes_follow_the_property_table() {
        let a = pack_content();
        assert_eq!(a.len(), 16 + 3 * 256 + 16 + 4);
        assert_eq!(a, pack_content());
        // The first flags byte after the prefix is tile 0's; FILL is solid, so its flags byte is not zero.
        assert_eq!(a[16], 0);
        assert_ne!(a[16 + usize::from(FILL)], 0);
        // The door channels are in the third table.
        assert_eq!(a[16 + 512 + usize::from(DOOR_R)], DOOR_RED_CHANNEL);
        assert_eq!(a[16 + 512 + usize::from(DOOR_B)], DOOR_BLUE_CHANNEL);
        assert_eq!(&a[a.len() - 4..], &[0, 1, 2, 3]);
    }
}
