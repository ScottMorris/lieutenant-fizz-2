// The cue tables a level's rules point at: sound effect ids, shared caption ids and caption styles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The numbers a baked rule refers to when it plays a cue. A rule record holds an effect id and a caption id as bytes, so the
//! numbers are part of the pack format and live with the rules, not with a presenter: the baker checks a rule against them,
//! and `tiles::pack_content()` puts them in the compatibility number.
//!
//! What stays in the presenters is everything a number is turned into: the words, the colours and the placement of a caption
//! (`fizz-slice-view`'s `captions.rs`), and the sound each effect id plays (`fizz-sfx`).

/// The cluck variants use effects 0 to 3 (effect = this plus the variant).
pub const EFFECT_CLUCK: u8 = 0;
pub const EFFECT_FZZT: u8 = 4;
pub const EFFECT_STUN: u8 = 5;
pub const EFFECT_BONK: u8 = 6;
pub const EFFECT_HURT: u8 = 7;
pub const EFFECT_WIN: u8 = 8;
pub const EFFECT_KEY: u8 = 9;
pub const EFFECT_CLUNK: u8 = 10;
pub const EFFECT_PLINK: u8 = 11;
pub const EFFECT_BOING: u8 = 12;
pub const EFFECT_JUMP: u8 = 13;
pub const EFFECT_CRUNCH: u8 = 14;
/// Three alternates, kept in the table and played by no event: the first Fizz 2 boing, jump and fzzt (Undertone FM and pitch steps), which
/// Fizz 1's own voices replaced at 12, 13 and 4.
pub const EFFECT_BOING_FM: u8 = 15;
pub const EFFECT_JUMP_FM: u8 = 16;
pub const EFFECT_SODA: u8 = 17;
pub const EFFECT_FZZT_FM: u8 = 18;
/// A door opening with its key (`Event::DoorOpen`): lower and longer than the clunk, which the head-bump keeps. The caption stays "clunk".
pub const EFFECT_DOOR: u8 = 19;
/// The number of effect ids: valid ids are `0..EFFECT_COUNT`.
pub const EFFECT_COUNT: u8 = 20;

/// The shared captions, as indices into the presenter's table of words; the index is the caption's id for dedupe, and the byte
/// a rule's caption code holds below `fizz_pack::level::CUE_LEVEL_BASE`.
pub const CAP_FZZT: u8 = 0;
pub const CAP_CLICK: u8 = 1;
pub const CAP_FIZZLED: u8 = 2;
pub const CAP_BONK: u8 = 3;
pub const CAP_WHOA: u8 = 4;
pub const CAP_TADA: u8 = 5;
pub const CAP_RED: u8 = 6;
pub const CAP_BLUE: u8 = 7;
pub const CAP_GREEN: u8 = 8;
pub const CAP_CLUNK: u8 = 9;
pub const CAP_BOING: u8 = 10;
pub const CAP_CRUNCH: u8 = 11;
pub const CAP_FSSSHT: u8 = 12;
/// The number of shared captions: valid ids are `0..CAP_COUNT`.
pub const CAP_COUNT: u8 = 13;

/// The caption styles a level's own strings choose from, in the order of a string record's style byte. A presenter maps each to
/// a bright and a dark colour.
pub const STYLE_WHITE: u8 = 0;
pub const STYLE_CYAN: u8 = 1;
pub const STYLE_RED: u8 = 2;
pub const STYLE_GREEN: u8 = 3;
pub const STYLE_BLUE: u8 = 4;
pub const STYLE_YELLOW: u8 = 5;
pub const STYLE_GREY: u8 = 6;
/// The number of styles; equals `fizz_pack::level::STYLE_COUNT`, which the reader checks a string against.
pub const STYLE_COUNT: u8 = 7;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_counts_cover_the_last_id_and_the_style_count_is_the_readers() {
        assert_eq!(EFFECT_COUNT, EFFECT_DOOR + 1);
        assert_eq!(CAP_COUNT, CAP_FSSSHT + 1);
        assert_eq!(STYLE_COUNT, STYLE_GREY + 1);
        assert_eq!(STYLE_COUNT, fizz_pack::level::STYLE_COUNT);
        const { assert!(CAP_COUNT < fizz_pack::level::CUE_LEVEL_BASE) };
    }
}
