// The map legend, the key colours and the caption styles of the Fizz 2 dialect, built from `fizz-ep2`'s constants.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_ep2::cues;
use fizz_ep2::rules::{KEY_BLUE, KEY_RED};
use fizz_ep2::tiles::{self, BLOCK, DOOR_B, DOOR_R, EMPTY, EXIT, FILL, PLAT, SPIKE, WALLBG};
use fizz_phys::tilemap::SWITCHED;

/// One letter of the map legend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Letter {
    pub letter: char,
    pub tile: u8,
    pub name: &'static str,
}

/// The legend: the letters of the slice level, in the order `docs/SCRIPT.md` lists them.
pub const LEGEND: [Letter; 9] = [
    Letter {
        letter: '.',
        tile: EMPTY,
        name: "EMPTY",
    },
    Letter {
        letter: '#',
        tile: FILL,
        name: "FILL",
    },
    Letter {
        letter: 'B',
        tile: BLOCK,
        name: "BLOCK",
    },
    Letter {
        letter: '=',
        tile: PLAT,
        name: "PLAT",
    },
    Letter {
        letter: '^',
        tile: SPIKE,
        name: "SPIKE",
    },
    Letter {
        letter: 'D',
        tile: DOOR_R,
        name: "DOOR_R",
    },
    Letter {
        letter: 'b',
        tile: DOOR_B,
        name: "DOOR_B",
    },
    Letter {
        letter: 'E',
        tile: EXIT,
        name: "EXIT",
    },
    Letter {
        letter: 'w',
        tile: WALLBG,
        name: "WALLBG",
    },
];

/// The tile id a letter stands for.
pub fn tile_of(letter: char) -> Option<u8> {
    LEGEND.iter().find(|l| l.letter == letter).map(|l| l.tile)
}

/// The switch channel of a door tile, or `None` for a tile that is not switched.
pub fn door_channel(tile: u8) -> Option<u8> {
    let p = tiles::props();
    (p.flags(tile) & SWITCHED != 0).then(|| p.channel(tile))
}

/// The key colour a word names, as `fizz_ep2::rules` numbers it.
pub fn key_colour(word: &str) -> Option<u8> {
    match word {
        "red" => Some(KEY_RED),
        "blue" => Some(KEY_BLUE),
        _ => None,
    }
}

/// The caption style words, indexed by `fizz_ep2::cues` style number.
pub const STYLES: [&str; cues::STYLE_COUNT as usize] =
    ["white", "cyan", "red", "green", "blue", "yellow", "grey"];

/// The style number a word names.
pub fn style_of(word: &str) -> Option<u8> {
    STYLES.iter().position(|s| *s == word).map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_legend_holds_the_slice_letters_once() {
        let mut letters: Vec<char> = LEGEND.iter().map(|l| l.letter).collect();
        letters.sort();
        letters.dedup();
        assert_eq!(letters.len(), LEGEND.len());
        let mut ids: Vec<u8> = LEGEND.iter().map(|l| l.tile).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), LEGEND.len());
    }

    #[test]
    fn the_legend_matches_the_tile_ids_of_the_rules() {
        let ids: Vec<(char, u8)> = LEGEND.iter().map(|l| (l.letter, l.tile)).collect();
        assert_eq!(
            ids,
            [
                ('.', 0),
                ('#', 1),
                ('B', 2),
                ('=', 3),
                ('^', 4),
                ('D', 12),
                ('b', 13),
                ('E', 17),
                ('w', 30)
            ]
        );
    }

    #[test]
    fn doors_are_the_switched_tiles() {
        assert_eq!(door_channel(DOOR_R), Some(tiles::DOOR_RED_CHANNEL));
        assert_eq!(door_channel(DOOR_B), Some(tiles::DOOR_BLUE_CHANNEL));
        assert_eq!(door_channel(FILL), None);
        for l in LEGEND {
            let door = l.tile == DOOR_R || l.tile == DOOR_B;
            assert_eq!(door_channel(l.tile).is_some(), door, "{}", l.name);
        }
    }

    #[test]
    fn the_style_words_are_the_seven_styles_in_order() {
        assert_eq!(STYLES.len(), usize::from(cues::STYLE_COUNT));
        assert_eq!(style_of("white"), Some(cues::STYLE_WHITE));
        assert_eq!(style_of("grey"), Some(cues::STYLE_GREY));
        assert_eq!(style_of("purple"), None);
    }
}
