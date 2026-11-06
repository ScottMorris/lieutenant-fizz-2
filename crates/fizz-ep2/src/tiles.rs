// Tile ids and tile properties for Episode 2's levels, kept equal to Fizz 1's ids.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ids and properties follow Fizz 1 `episodes/episode-1/game/src/tiles.rs` at commit `4bce036`, restricted to
// the tiles the slice uses. Departure: the red and blue doors are switched tiles, solid while the switch bit their cell is bound to
// is set, rather than solid tiles that Fizz 1 rewrites to empty space when it opens. A tile id carries no channel: the level binds each
// door's cells to a bit (see `level.rs`).

use crate::cues;
use crate::rules::{KEY_BLUE, KEY_GREEN, KEY_RED, RULES_SEMANTICS};
use fizz_pack::level::{MAX_BINDINGS, MAX_DOOR_BINDINGS, MAX_STATE_VALUES, MAX_STATES};
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
/// The red door: solid while the switch bit its cells are bound to is set.
pub const DOOR_R: u8 = 12;
/// The blue door: solid while the switch bit its cells are bound to is set.
pub const DOOR_B: u8 = 13;
/// The exit: touching it ends the level.
pub const EXIT: u8 = 17;
/// Decorative interior wall behind a room. It is not solid.
pub const WALLBG: u8 = 30;

/// The switch bit of the slice's red door (door binding 0). A set bit means the door is closed.
pub const DOOR_RED_CHANNEL: u8 = 0;
/// The red door's bit in `TileMap::switches`.
pub const DOOR_RED_BIT: u32 = 1 << DOOR_RED_CHANNEL;

/// The switch bit of the slice's blue door (door binding 1). A set bit means the door is closed.
pub const DOOR_BLUE_CHANNEL: u8 = 1;
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
    p.set_flags(DOOR_B, SWITCHED);
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
/// the tile property table (flags and slope for each of the 256 tile ids, in that order), the named tile ids, and the
/// numbers of the snack kinds (the three snacks, the soda can and the elixir), the key colour numbers (red 0, blue 1, green 2), the state limits
/// (`MAX_STATES`, `MAX_STATE_VALUES`, `MAX_BINDINGS` and `MAX_DOOR_BINDINGS`), the sound effect ids (each named id, then `EFFECT_COUNT`), the shared caption ids (each, then `CAP_COUNT`), `STYLE_COUNT` and
/// the `RULES_SEMANTICS` byte, which is bumped when what a rule record means changes without its layout changing. The object kind
/// numbers belong to `fizz-pack` itself. Changing a flag, a slope, an id or a cue number changes the number on purpose,
/// so a pack baked before the change is refused.
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
    v.extend_from_slice(&[
        EMPTY, FILL, BLOCK, PLAT, SPIKE, CHOC, R45, L45, R22A, R22B, L22A, L22B, DOOR_R, DOOR_B,
        EXIT, WALLBG,
    ]);
    v.extend_from_slice(&[
        SnackKind::Cheezie.number(),
        SnackKind::Choc.number(),
        SnackKind::Cookie.number(),
        SnackKind::Soda.number(),
        SnackKind::Elixir.number(),
    ]);
    v.extend_from_slice(&[
        KEY_RED,
        KEY_BLUE,
        KEY_GREEN,
        MAX_STATES as u8,
        MAX_STATE_VALUES as u8,
        MAX_BINDINGS as u8,
        MAX_DOOR_BINDINGS as u8,
    ]);
    v.extend_from_slice(&[
        cues::EFFECT_CLUCK,
        cues::EFFECT_FZZT,
        cues::EFFECT_STUN,
        cues::EFFECT_BONK,
        cues::EFFECT_HURT,
        cues::EFFECT_WIN,
        cues::EFFECT_KEY,
        cues::EFFECT_CLUNK,
        cues::EFFECT_PLINK,
        cues::EFFECT_BOING,
        cues::EFFECT_JUMP,
        cues::EFFECT_CRUNCH,
        cues::EFFECT_BOING_FM,
        cues::EFFECT_JUMP_FM,
        cues::EFFECT_SODA,
        cues::EFFECT_FZZT_FM,
        cues::EFFECT_DOOR,
        cues::EFFECT_WHISTLE,
        cues::EFFECT_LIFE,
        cues::EFFECT_KICK,
        cues::EFFECT_COUNT,
    ]);
    v.extend_from_slice(&[
        cues::CAP_FZZT,
        cues::CAP_CLICK,
        cues::CAP_FIZZLED,
        cues::CAP_BONK,
        cues::CAP_WHOA,
        cues::CAP_TADA,
        cues::CAP_RED,
        cues::CAP_BLUE,
        cues::CAP_GREEN,
        cues::CAP_CLUNK,
        cues::CAP_BOING,
        cues::CAP_CRUNCH,
        cues::CAP_FSSSHT,
        cues::CAP_WHEEE,
        cues::CAP_LIFE,
        cues::CAP_KICK,
        cues::CAP_COUNT,
    ]);
    v.extend_from_slice(&[cues::STYLE_COUNT, RULES_SEMANTICS]);
    v
}

#[cfg(test)]
mod pack_content_tests {
    use super::*;

    #[test]
    fn the_content_bytes_follow_the_property_table() {
        let a = pack_content();
        // Prefix, two tables, the tile ids, the snack kinds, then the rule layer's numbers: the three key colours and the four state
        // limits (7), 20 effect ids and their count, 16 caption ids and their count, STYLE_COUNT and RULES_SEMANTICS.
        assert_eq!(a.len(), 16 + 2 * 256 + 16 + 5 + 7 + 21 + 17 + 2);
        assert_eq!(a.len(), 596);
        assert_eq!(a, pack_content());
        // The first flags byte after the prefix is tile 0's; FILL is solid, so its flags byte is not zero.
        assert_eq!(a[16], 0);
        assert_ne!(a[16 + usize::from(FILL)], 0);
        // The door tiles are switched, and a tile id carries no channel.
        assert_ne!(
            a[16 + usize::from(DOOR_R)] & fizz_phys::tilemap::SWITCHED,
            0
        );
        assert_ne!(
            a[16 + usize::from(DOOR_B)] & fizz_phys::tilemap::SWITCHED,
            0
        );
        let at = 16 + 2 * 256 + 16;
        assert_eq!(&a[at..at + 5], &[0, 1, 2, 3, 4], "snack kinds");
        assert_eq!(
            &a[at + 5..at + 12],
            &[0, 1, 2, 32, 8, 64, 32],
            "key colours, then MAX_STATES, MAX_STATE_VALUES, MAX_BINDINGS and MAX_DOOR_BINDINGS"
        );
        let fx = &a[at + 12..at + 33];
        assert_eq!(fx[0], 0);
        assert_eq!(fx[16], 19, "the door");
        assert_eq!(fx[17], 20, "the whistle");
        assert_eq!(fx[18], 21, "the new life");
        assert_eq!(fx[19], 22, "the last named effect is the wall-kick");
        assert_eq!(fx[20], 23, "EFFECT_COUNT");
        let caps = &a[at + 33..at + 50];
        assert_eq!(
            caps[..16],
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
        assert_eq!(caps[16], 16, "CAP_COUNT");
        assert_eq!(&a[a.len() - 2..], &[7, 2], "STYLE_COUNT, RULES_SEMANTICS");
    }

    #[test]
    fn the_compatibility_number_is_pinned() {
        // Moved from 0xc7f3_982b on purpose (842 content bytes under the schema number 0x25e8_1944): the content lost the channel table
        // and `MAX_CHANNELS` and gained the state limits and `RULES_SEMANTICS` 2, under the schema number of `LEVL` version 2. The new
        // value was also recomputed in Python from the dumped content bytes. The content is 596 bytes: the elixir's snack kind, the whistle, life and
        // kick effects and their captions are in it. The kick (effect 22, caption 15) moved it from 0x9161_5aaf (594 bytes); the value was recomputed in Python from the dumped bytes under the schema number 0xa1bb_8460.
        assert_eq!(fizz_pack::compatibility(&pack_content()), 0x6365_509c);
    }
}
