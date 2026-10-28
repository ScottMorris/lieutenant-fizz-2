// Draws the Options and Paused screens over a dimmed, frozen copy of the picture, in Fizz 1's menu look.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The look is Fizz 1's DOS port (`options_screen` and `plate_rows` in its shell): the whole picture dimmed, a white heading at
// twice size with a rule under it, and a column of rows. The chosen row is a plate in the accent colour (`fizz_view::theme`)
// with its corners clipped and black ink, and Fizz 1's soda can stands in the gutter beside it, bobbing one pixel every twenty
// ticks. The other rows are a white label and an accent-coloured value, and a grey footer names the keys. The pixel font has no
// arrows, so nothing is drawn for them. The geometry is for the 320 by 200 frame this crate draws; the Web's overlay is its own.
//
// The picture is dimmed once, when the screen opens ([`MenuView::open`]): every pixel `p` becomes `16 + (p & 15)`, the dimmed
// half of the palette the HUD box already uses. Each redraw copies that background and draws the screen over it, and only when
// something a viewer can see has changed (a row's words or selection, the note, or the bob), so a frame that nothing touched is
// neither redrawn nor presented again.

use fizz_menu::{
    Caps, Device, OVERRIDE_NOTE, OptionsScreen, Overrides, PausedScreen, Platform, RowId, RowView,
    Values,
};
use fizz_view::blit_sprite;
use fizz_view::pfont::{draw_text, draw_text_scaled, text_width, text_width_scaled};
use fizz_view::sprites_data::sprite;
use fizz_view::theme::FIZZ_INDEX;

use crate::frame::Frame;
use crate::palette::{BLACK, DIM_BASE, LIGHT_GREY, LIGHT_RED, WHITE};
use crate::{HEIGHT, WIDTH};

/// The heading's top edge, and the scale it is drawn at.
pub const TITLE_Y: i32 = 22;
pub const TITLE_SCALE: i32 = 2;
/// The one-pixel rule under the heading, in the accent colour.
pub const RULE_Y: i32 = 36;
/// The first row's top edge and the distance between rows.
pub const ROWS_Y: i32 = 56;
pub const PITCH: i32 = 14;
/// The left edge of the whole row column (the gutter) and its width: columns 40 to 279.
pub const LEFT: i32 = 40;
pub const COLUMN_W: i32 = 240;
/// The gutter the soda can stands in, so the plate starts at `LEFT + GUTTER`.
pub const GUTTER: i32 = 18;
/// The plate's height, and the space between its edge and the ink.
pub const PLATE_H: i32 = 12;
pub const INSET: i32 = 6;
/// The text's top edge inside a row.
const TEXT_DY: i32 = 2;
/// The override note's top edge, and the footer's.
pub const NOTE_Y: i32 = 172;
pub const FOOTER_Y: i32 = 186;
/// How many ticks the soda can stays at each of its two heights.
pub const BOB_TICKS: u32 = 20;

/// The Options footer.
pub const OPTIONS_FOOTER: &str = "Up/Down choose, Left/Right change, Esc back";
/// The Paused footer.
pub const PAUSED_FOOTER: &str = "Up and Down choose, Enter selects, Esc resumes";

/// What a menu screen shows: its heading, its rows, whether the override note shows, a message and its footer. A message takes
/// the note's line, in light red (Fizz 1's slot message), and an empty one shows nothing.
#[derive(Clone, Copy)]
pub struct Screen<'a> {
    pub title: &'a str,
    pub rows: &'a [RowView],
    pub note: bool,
    pub message: &'a str,
    pub footer: &'a str,
}

/// The dimmed picture a menu is drawn over, and what was last drawn on it.
pub struct MenuView {
    background: Frame,
    shown: Option<u64>,
    draws: u32,
    message: &'static str,
}

impl MenuView {
    pub const fn new() -> Self {
        Self {
            background: Frame::new(),
            shown: None,
            draws: 0,
            message: "",
        }
    }

    /// Sets the message the next draws show in light red below the rows (`""` clears it). It changes what is drawn, so the next
    /// draw happens.
    pub fn set_message(&mut self, message: &'static str) {
        self.message = message;
    }

    /// Freezes `picture` as the background: a dimmed copy of it, the whole frame. Call it once as the screen opens, and again
    /// when another screen (Paused to Options) opens over the same picture; the next draw then always draws.
    pub fn open(&mut self, picture: &Frame) {
        let dst = self.background.pixels_mut();
        for (d, s) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(picture.pixels().as_chunks::<4>().0)
        {
            let word = u32::from_le_bytes(*s);
            *d = ((word & 0x0F0F_0F0F) | (u32::from(DIM_BASE) * 0x0101_0101)).to_le_bytes();
        }
        self.shown = None;
    }

    /// Forgets what was drawn, so the next draw happens even if nothing changed (after something else wrote to the frame).
    pub fn invalidate(&mut self) {
        self.shown = None;
    }

    /// How many times a screen has been drawn (as opposed to found unchanged) since this view was made.
    pub fn draws(&self) -> u32 {
        self.draws
    }

    /// The background, for a caller that wants to look at it.
    pub fn background(&self) -> &Frame {
        &self.background
    }

    /// Draws the Options screen into `out` if anything shows differently from the last draw; returns whether it drew, which
    /// is whether the frame should be presented. `tick` is the game tick, for the can's bob.
    pub fn draw_options(
        &mut self,
        screen: &OptionsScreen,
        caps: &Caps,
        tick: u32,
        out: &mut Frame,
    ) -> bool {
        let rows = screen.rows(caps);
        self.draw(
            &Screen {
                title: "Options",
                rows: rows.as_slice(),
                note: screen.show_note(),
                message: self.message,
                footer: OPTIONS_FOOTER,
            },
            tick,
            out,
        )
    }

    /// Draws the Paused screen into `out`, as [`MenuView::draw_options`] does.
    pub fn draw_paused(&mut self, screen: &PausedScreen, tick: u32, out: &mut Frame) -> bool {
        let rows = screen.rows();
        self.draw(
            &Screen {
                title: "Paused",
                rows: rows.as_slice(),
                note: false,
                message: self.message,
                footer: PAUSED_FOOTER,
            },
            tick,
            out,
        )
    }

    /// Copies the background into `out` and draws `screen` over it, unless `screen` and the bob are what the last draw showed.
    pub fn draw(&mut self, screen: &Screen, tick: u32, out: &mut Frame) -> bool {
        let bob = bob(tick);
        let sig = signature(screen, bob);
        if self.shown == Some(sig) {
            return false;
        }
        out.pixels_mut().copy_from_slice(self.background.pixels());
        render(screen, bob, out);
        self.shown = Some(sig);
        self.draws += 1;
        true
    }
}

impl Default for MenuView {
    fn default() -> Self {
        Self::new()
    }
}

/// How many pixels the soda can is raised at `tick`: 0 or 1, changing every [`BOB_TICKS`] ticks.
pub const fn bob(tick: u32) -> i32 {
    ((tick / BOB_TICKS) & 1) as i32
}

/// A digest of everything that decides what [`render`] draws, to tell an unchanged screen from a changed one.
fn signature(screen: &Screen, bob: i32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // A separator no ASCII text holds, so "ab" then "c" differs from "a" then "bc".
        h ^= 0xFF;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    feed(screen.title.as_bytes());
    feed(screen.footer.as_bytes());
    feed(screen.message.as_bytes());
    feed(&[u8::from(screen.note), bob as u8]);
    for row in screen.rows {
        feed(row.label.as_bytes());
        feed(row.value.as_str().as_bytes());
        feed(&[u8::from(row.selected)]);
    }
    h
}

/// Draws `text` at (`x`, `y`) in `colour` over a black shadow one pixel down and right, as Fizz 1's `ui::text` does.
fn shadowed(out: &mut Frame, x: i32, y: i32, text: &str, colour: u8) {
    draw_text(out.pixels_mut(), WIDTH, HEIGHT, x + 1, y + 1, text, BLACK);
    draw_text(out.pixels_mut(), WIDTH, HEIGHT, x, y, text, colour);
}

fn centred(out: &mut Frame, y: i32, text: &str, colour: u8) {
    shadowed(out, (WIDTH as i32 - text_width(text)) / 2, y, text, colour);
}

/// Draws `screen` onto `out`, which already holds the background.
fn render(screen: &Screen, bob: i32, out: &mut Frame) {
    // The heading at twice size with a shadow two pixels down and right, then the rule. The 'p' of Options reaches the rule's row.
    let title_x = (WIDTH as i32 - text_width_scaled(screen.title, TITLE_SCALE)) / 2;
    for (d, c) in [(TITLE_SCALE, BLACK), (0, WHITE)] {
        draw_text_scaled(
            out.pixels_mut(),
            WIDTH,
            HEIGHT,
            title_x + d,
            TITLE_Y + d,
            screen.title,
            c,
            TITLE_SCALE,
        );
    }
    out.fill_rect(LEFT, RULE_Y, COLUMN_W, 1, FIZZ_INDEX);

    let plate_x = LEFT + GUTTER;
    let plate_w = COLUMN_W - GUTTER;
    for (i, row) in screen.rows.iter().enumerate() {
        let y = ROWS_Y + i as i32 * PITCH;
        let value = row.value.as_str();
        let value_w = text_width(value);
        let value_x = plate_x + plate_w - INSET - value_w;
        if row.selected {
            // The plate, with its four corner pixels left out.
            out.fill_rect(plate_x + 1, y, plate_w - 2, PLATE_H, FIZZ_INDEX);
            out.fill_rect(plate_x, y + 1, plate_w, PLATE_H - 2, FIZZ_INDEX);
            if let Some(can) = sprite("soda") {
                let top = y - 2 - bob;
                blit_sprite(
                    out.pixels_mut(),
                    WIDTH,
                    HEIGHT,
                    can,
                    LEFT + i32::from(can.ax),
                    top + i32::from(can.ay),
                    false,
                );
            }
            draw_text(
                out.pixels_mut(),
                WIDTH,
                HEIGHT,
                plate_x + INSET,
                y + TEXT_DY,
                row.label,
                BLACK,
            );
            draw_text(
                out.pixels_mut(),
                WIDTH,
                HEIGHT,
                value_x,
                y + TEXT_DY,
                value,
                BLACK,
            );
        } else {
            shadowed(out, plate_x + INSET, y + TEXT_DY, row.label, WHITE);
            shadowed(out, value_x, y + TEXT_DY, value, FIZZ_INDEX);
        }
    }
    if !screen.message.is_empty() {
        centred(out, NOTE_Y, screen.message, LIGHT_RED);
    } else if screen.note {
        centred(out, NOTE_Y, OVERRIDE_NOTE, LIGHT_GREY);
    }
    centred(out, FOOTER_Y, screen.footer, LIGHT_GREY);
}

/// The fixed machine the `/MENUSHOT` screens are drawn for: a Sound Blaster 16 with everything present, running at 60 Hz.
fn shot_caps() -> Caps {
    Caps {
        platform: Platform::Dos,
        sb: true,
        adlib: true,
        speaker: true,
        auto_music: Device::Sb,
        auto_fx: Device::Sb,
        rate: 60,
        fullscreen: false,
    }
}

/// Turns the picture in `frame` into the Options screen the `/MENUSHOT` mode and the host `menushot` example both draw: the
/// cursor on Effects, the Frame rate row marked as set by a switch (so the note shows) and the can at its first height. `view`
/// is opened on the picture, so on return it holds the dimmed copy and `frame` the finished screen.
pub fn shot_options(view: &mut MenuView, frame: &mut Frame) {
    view.open(frame);
    let mut screen = OptionsScreen::new(Values::default(), Overrides::NONE.with(RowId::FRAME_RATE));
    screen.cursor = 1;
    view.draw_options(&screen, &shot_caps(), 0, frame);
}

/// The same for the Paused screen, with the cursor on Resume.
pub fn shot_paused(view: &mut MenuView, frame: &mut Frame) {
    view.open(frame);
    view.draw_paused(&PausedScreen::new(Platform::Dos), 0, frame);
}

/// The room a row's label and value have between them: the plate's width less its inset on each side and a gap of one space.
pub const fn row_room() -> i32 {
    COLUMN_W - GUTTER - 2 * INSET - 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_digest;
    use fizz_machine::{
        FxChoice, FxPlan, MusicChoice, MusicPlan, OplMode, Resolved, SoundState, SoundStates,
    };
    use fizz_menu::MenuKey;

    fn resolved(sb: SoundState, adlib: SoundState, music: MusicPlan, fx: FxPlan) -> Resolved {
        Resolved {
            opl: OplMode::Opl2,
            music,
            fx,
            music_reason: None,
            fx_reason: None,
            states: SoundStates {
                speaker: SoundState::Available,
                adlib,
                sb,
            },
        }
    }

    fn sb16() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::InUse,
                SoundState::InUse,
                MusicPlan::Opl3,
                FxPlan::Ch14,
            ),
            60,
        )
    }

    fn adlib_only() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::Missing,
                SoundState::InUse,
                MusicPlan::Opl2,
                FxPlan::Ch5,
            ),
            60,
        )
    }

    fn no_card() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::Missing,
                SoundState::Missing,
                MusicPlan::Off,
                FxPlan::Speaker,
            ),
            30,
        )
    }

    /// A picture that uses all sixteen colours in a pattern with no flat areas, so a dimming mistake shows.
    fn picture() -> Frame {
        let mut f = Frame::new();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                f.pixels_mut()[y * WIDTH + x] = ((x * 7 + y * 3 + x * y / 5) & 15) as u8;
            }
        }
        f
    }

    fn options(cursor: usize, values: Values, over: Overrides, caps: &Caps) -> Frame {
        let mut screen = OptionsScreen::new(values, over);
        screen.cursor = cursor;
        let mut view = MenuView::new();
        view.open(&picture());
        let mut out = Frame::new();
        assert!(view.draw_options(&screen, caps, 0, &mut out));
        out
    }

    fn digest_options(cursor: usize, values: Values, over: Overrides, caps: &Caps) -> u64 {
        frame_digest(&options(cursor, values, over, caps))
    }

    #[test]
    fn a_message_takes_the_notes_line_in_light_red() {
        let screen = OptionsScreen::new(Values::default(), Overrides::NONE.with(RowId::FRAME_RATE));
        let mut view = MenuView::new();
        view.open(&picture());
        let mut plain = Frame::new();
        assert!(view.draw_options(&screen, &sb16(), 0, &mut plain));
        let red = |f: &Frame| f.pixels().iter().filter(|&&p| p == LIGHT_RED).count();
        // The soda can has a little light red of its own, so the message is judged by how much it adds.
        let base = red(&plain);
        // Setting a message changes the next draw, and the note's words are replaced.
        view.set_message("Could not write FIZZ2.CFG");
        let mut with = Frame::new();
        assert!(view.draw_options(&screen, &sb16(), 0, &mut with));
        assert!(red(&with) > base + 40, "{} over {}", red(&with), base);
        assert_ne!(frame_digest(&plain), frame_digest(&with));
        // Drawn again unchanged, it is not drawn.
        assert!(!view.draw_options(&screen, &sb16(), 0, &mut with));
        view.set_message("");
        let mut cleared = Frame::new();
        assert!(view.draw_options(&screen, &sb16(), 0, &mut cleared));
        assert_eq!(frame_digest(&plain), frame_digest(&cleared));
    }

    #[test]
    fn opening_dims_every_pixel_into_the_dimmed_half() {
        let pic = picture();
        let mut view = MenuView::new();
        view.open(&pic);
        for (b, p) in view.background().pixels().iter().zip(pic.pixels()) {
            assert_eq!(*b, 16 + (p & 15));
        }
        assert!(
            view.background()
                .pixels()
                .iter()
                .all(|b| (16..=31).contains(b))
        );
    }

    #[test]
    fn dimming_also_maps_pixels_already_in_the_dimmed_half() {
        // The HUD box leaves dimmed pixels in the picture; they stay in 16..=31 and keep their colour.
        let mut pic = Frame::new();
        pic.clear(0x1B);
        pic.pixels_mut()[0] = 0xFF;
        let mut view = MenuView::new();
        view.open(&pic);
        assert_eq!(view.background().pixels()[1], 0x1B);
        assert_eq!(view.background().pixels()[0], 0x1F);
    }

    #[test]
    fn the_screen_draws_over_the_dimmed_picture_and_only_there() {
        let out = options(2, Values::default(), Overrides::NONE, &sb16());
        let mut view = MenuView::new();
        view.open(&picture());
        let bg = view.background().pixels();
        // Far from the heading, rows and footer the frame is the background.
        for (y, x) in [
            (0usize, 0usize),
            (10, 300),
            (150, 5),
            (199, 319),
            (100, 310),
        ] {
            assert_eq!(out.pixels()[y * WIDTH + x], bg[y * WIDTH + x], "({x},{y})");
        }
        // Over it: the accent colour (plate and rule), white and black appear; the plate is solid inside.
        for colour in [FIZZ_INDEX, WHITE, BLACK] {
            assert!(out.pixels().contains(&colour), "{colour}");
        }
        let plate_y = ROWS_Y + 2 * PITCH;
        let (px, pw) = (LEFT + GUTTER, COLUMN_W - GUTTER);
        let at = |x: i32, y: i32| out.pixels()[y as usize * WIDTH + x as usize];
        // The corners are clipped; the edges between are solid.
        for (x, y) in [
            (px, plate_y),
            (px + pw - 1, plate_y),
            (px, plate_y + PLATE_H - 1),
            (px + pw - 1, plate_y + PLATE_H - 1),
        ] {
            assert_eq!(
                at(x, y),
                bg[y as usize * WIDTH + x as usize],
                "corner ({x},{y})"
            );
        }
        for x in [px + 1, px + pw - 2] {
            assert_eq!(at(x, plate_y), FIZZ_INDEX);
            assert_eq!(at(x, plate_y + PLATE_H - 1), FIZZ_INDEX);
        }
        assert_eq!(at(px, plate_y + 1), FIZZ_INDEX);
        assert_eq!(at(px + pw - 1, plate_y + PLATE_H - 2), FIZZ_INDEX);
        // The rule is one pixel of the accent colour across the column.
        for x in LEFT..LEFT + COLUMN_W {
            assert_eq!(at(x, RULE_Y), FIZZ_INDEX, "rule at {x}");
        }
        assert_eq!(
            at(LEFT - 1, RULE_Y),
            bg[RULE_Y as usize * WIDTH + (LEFT - 1) as usize]
        );
    }

    #[test]
    fn the_can_bobs_one_pixel_every_twenty_ticks() {
        let mut screen = OptionsScreen::new(Values::default(), Overrides::NONE);
        screen.cursor = 1;
        let caps = sb16();
        let mut view = MenuView::new();
        view.open(&picture());
        let mut out = Frame::new();
        let mut shots = [0u64; 3];
        for (i, tick) in [0u32, 19, 20].into_iter().enumerate() {
            view.draw_options(&screen, &caps, tick, &mut out);
            shots[i] = frame_digest(&out);
        }
        assert_eq!(shots[0], shots[1]);
        assert_ne!(shots[1], shots[2]);
        assert_eq!(
            (bob(0), bob(19), bob(20), bob(39), bob(40)),
            (0, 0, 1, 1, 0)
        );
    }

    #[test]
    fn nothing_redraws_until_something_changes() {
        let caps = sb16();
        let mut screen = OptionsScreen::new(Values::default(), Overrides::NONE);
        let mut view = MenuView::new();
        view.open(&picture());
        let mut out = Frame::new();
        assert!(view.draw_options(&screen, &caps, 0, &mut out));
        let first = frame_digest(&out);
        for tick in 1..20 {
            assert!(!view.draw_options(&screen, &caps, tick, &mut out), "{tick}");
        }
        assert_eq!(frame_digest(&out), first);
        assert_eq!(view.draws(), 1);
        // The bob changes the picture; a key does; a key that changes nothing does not.
        assert!(view.draw_options(&screen, &caps, 20, &mut out));
        assert!(!view.draw_options(&screen, &caps, 21, &mut out));
        screen.key(MenuKey::Down, &caps);
        assert!(view.draw_options(&screen, &caps, 21, &mut out));
        screen.key(MenuKey::Right, &caps);
        assert!(view.draw_options(&screen, &caps, 22, &mut out));
        assert_eq!(view.draws(), 4);
        // Opening again, or invalidating, forces the next draw.
        assert!(!view.draw_options(&screen, &caps, 23, &mut out));
        view.invalidate();
        assert!(view.draw_options(&screen, &caps, 23, &mut out));
        // The redraw is a pure function of what is shown: drawing the same state fresh gives the same pixels.
        let mut fresh_view = MenuView::new();
        fresh_view.open(&picture());
        let mut fresh = Frame::new();
        fresh_view.draw_options(&screen, &caps, 23, &mut fresh);
        assert_eq!(frame_digest(&out), frame_digest(&fresh));
    }

    #[test]
    fn a_change_only_redraws_where_it_shows() {
        // Changing one row's value changes pixels only in that row's band (the plate rows of the two rows involved when the
        // cursor moves).
        let caps = sb16();
        let mut screen = OptionsScreen::new(Values::default(), Overrides::NONE);
        let mut view = MenuView::new();
        view.open(&picture());
        let mut a = Frame::new();
        view.draw_options(&screen, &caps, 0, &mut a);
        let before = a.pixels().to_vec();
        screen.key(MenuKey::Right, &caps);
        view.draw_options(&screen, &caps, 0, &mut a);
        let band = ROWS_Y..ROWS_Y + PITCH;
        for (i, (b, p)) in before.iter().zip(a.pixels()).enumerate() {
            let y = (i / WIDTH) as i32;
            assert!(b == p || band.contains(&y), "pixel changed at y={y}");
        }
        assert_ne!(before.as_slice(), a.pixels());
    }

    #[test]
    fn pinned_digests_of_the_screens() {
        let caps = sb16();
        let values = Values::default();
        // Every selected row of the Options screen on a Sound Blaster 16 machine.
        let mut got = [0u64; 8];
        for (cursor, d) in got.iter_mut().enumerate() {
            *d = digest_options(cursor, values, Overrides::NONE, &caps);
        }
        assert_eq!(
            got,
            [
                0x9563_1aa6_e90a_86cc,
                0x456a_aebf_2a8c_c2b2,
                0xd11d_9ba0_81aa_bc5e,
                0xa104_5f1a_f276_4205,
                0x0a85_1dec_fc01_e01f,
                0xdb8b_b7f5_5c8a_d0e8,
                0xf473_5a24_dd61_7fab,
                0xd4c7_dc93_a01c_c479,
            ],
            "options rows"
        );
    }

    #[test]
    fn pinned_digest_of_paused() {
        let mut view = MenuView::new();
        view.open(&picture());
        let mut out = Frame::new();
        view.draw_paused(&PausedScreen::new(Platform::Dos), 0, &mut out);
        assert_eq!(frame_digest(&out), 0x2c77_9963_608c_2ef4);
    }

    #[test]
    fn pinned_digests_of_a_starred_row_and_a_missing_device() {
        let star = digest_options(
            3,
            Values::default(),
            Overrides::NONE.with(RowId::FRAME_RATE),
            &sb16(),
        );
        let missing = digest_options(
            0,
            Values {
                music: MusicChoice::Sb,
                fx: FxChoice::Sb,
                ..Values::default()
            },
            Overrides::NONE,
            &adlib_only(),
        );
        assert_eq!(
            (star, missing),
            (0x0740_4776_8265_6fc7, 0xb4b2_3065_14d6_21e1)
        );
    }

    #[test]
    fn every_label_and_value_fits_its_column() {
        let caps_all = [
            sb16(),
            adlib_only(),
            no_card(),
            Caps::web(true),
            Caps::web(false),
        ];
        let mut overrides = Overrides::NONE;
        for id in 0..10u8 {
            overrides = overrides.with(RowId(id));
        }
        let mut checked = 0;
        for caps in caps_all {
            for over in [Overrides::NONE, overrides] {
                for start in [
                    Values::default(),
                    Values {
                        music: MusicChoice::Sb,
                        fx: FxChoice::Sb,
                        ..Values::default()
                    },
                ] {
                    // Visit every choice of every row by stepping each row through a full turn.
                    let mut screen = OptionsScreen::new(start, over);
                    for row in 0..screen.rows(&caps).len() {
                        screen.cursor = row;
                        for _ in 0..12 {
                            screen.key(MenuKey::Right, &caps);
                            screen.cursor = row;
                            for r in screen.rows(&caps).as_slice() {
                                let used = text_width(r.label) + text_width(r.value.as_str());
                                assert!(
                                    used <= row_room(),
                                    "{:?} {} {} is {used} wide",
                                    caps.platform,
                                    r.label,
                                    r.value.as_str()
                                );
                                checked += 1;
                            }
                        }
                    }
                    for r in PausedScreen::new(caps.platform).rows().as_slice() {
                        assert!(text_width(r.label) <= row_room(), "{}", r.label);
                    }
                }
            }
        }
        assert!(checked > 1000, "{checked}");
        // The note and footers fit the frame.
        for text in [OVERRIDE_NOTE, OPTIONS_FOOTER, PAUSED_FOOTER] {
            assert!(text_width(text) <= WIDTH as i32 - 8, "{text}");
        }
        assert!(text_width_scaled("Options", TITLE_SCALE) < COLUMN_W);
        assert!(text_width_scaled("Paused", TITLE_SCALE) < COLUMN_W);
    }

    #[test]
    fn the_last_row_and_the_note_clear_each_other() {
        // Eight rows end at y = 56 + 7 * 14 + 12 = 166, above the note at 172 and the footer at 186.
        const {
            assert!(ROWS_Y + 7 * PITCH + PLATE_H < NOTE_Y);
            assert!(NOTE_Y + 9 < FOOTER_Y);
            assert!(FOOTER_Y + 9 <= HEIGHT as i32);
            // And the can's top, two above the first row, clears the rule.
            assert!(ROWS_Y - 2 - 1 > RULE_Y);
        }
    }

    #[test]
    fn the_note_shows_only_when_a_row_is_starred() {
        let caps = sb16();
        let plain = options(0, Values::default(), Overrides::NONE, &caps);
        let starred = options(
            0,
            Values::default(),
            Overrides::NONE.with(RowId::MUSIC),
            &caps,
        );
        let band = |f: &Frame| {
            f.pixels()[NOTE_Y as usize * WIDTH..(NOTE_Y as usize + 9) * WIDTH]
                .iter()
                .filter(|&&p| p == LIGHT_GREY)
                .count()
        };
        assert_eq!(band(&plain), 0);
        assert!(band(&starred) > 0);
    }
}
