// The one accent colour of Fizz 2, "grape fizz", that everything called Fizz is drawn in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Fizz 1 draws the Fizz number, Ben's shots and the puffs in cyan. Fizz 2 draws them, and the menu plate and titles, in EGA light
// magenta. The colour is defined here once: the DOS presenter draws with [`FIZZ_INDEX`], the Web reads [`FIZZ_CSS`] through the
// `--fizz` custom property in `web/style.css` (a test below keeps the two equal), and the sprite exporter recolours the shot and
// puff grids to the same colour (`FIZZ_RECOLOUR` in `tools/sprites/export-fizz1.ts`; a test below checks the baked shot uses it).

/// One of the sixteen EGA colours, by palette index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Ega {
    LightMagenta = 13,
}

impl Ega {
    /// The colour's EGA palette index, 0 to 15.
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// The colour as a lower-case CSS hex string, from the standard EGA values (`0x55` for a low channel, `0xff` for a high one).
    pub const fn css(self) -> &'static str {
        match self {
            Ega::LightMagenta => "#ff55ff",
        }
    }
}

/// The accent colour: light magenta.
pub const FIZZ: Ega = Ega::LightMagenta;
/// The accent colour's EGA palette index (13).
pub const FIZZ_INDEX: u8 = FIZZ.index();
/// The accent colour as CSS (`#ff55ff`), which `web/style.css` holds as `--fizz`.
pub const FIZZ_CSS: &str = FIZZ.css();
/// What the colour is called in the game and its documents.
pub const FIZZ_NAME: &str = "grape";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_and_css_agree() {
        assert_eq!(FIZZ_INDEX, 13);
        assert_eq!(FIZZ_CSS, "#ff55ff");
        assert_eq!(FIZZ_NAME, "grape");
    }

    #[test]
    fn the_stylesheet_defines_the_same_colour() {
        let css = include_str!("../../../web/style.css");
        let at = css.find("--fizz: ").expect("web/style.css defines --fizz");
        let value = &css[at + "--fizz: ".len()..];
        assert!(value.starts_with(FIZZ_CSS), "--fizz must be {FIZZ_CSS}");
        assert!(value[FIZZ_CSS.len()..].starts_with(';'));
    }

    #[test]
    fn the_shot_sprite_is_drawn_in_the_accent_colour() {
        let bubble = crate::sprites_data::sprite("bubble").expect("bubble");
        assert!(bubble.pixels.contains(&FIZZ_INDEX));
        assert!(
            !bubble.pixels.contains(&11),
            "no light cyan is left in the shot"
        );
    }
}
