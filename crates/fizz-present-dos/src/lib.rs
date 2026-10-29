// Renders the slice into a 320x200 indexed frame and maps keys to buttons, for the DOS and host presenters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The frame is a pure function of the world. Nothing here touches hardware: `present` hands the finished frame to a
// `fizz_hal::Display`, and the platform layer owns the copy to the screen.
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod frame;
mod hud;
pub mod menu;
mod palette;

use fizz_hal::{Display, HalError, KeyCode};
use fizz_sim::Buttons;

pub use fizz_sfx::place::Placement;
pub use fizz_slice_view::{
    DrawOptions, EventCaptions, Puffs, VIEW_H, VIEW_W, ViewFx, cluck_placement, event_sound,
};
pub use fizz_view::{GLYPH, glyph};
pub use frame::Frame;
pub use hud::{Hud, HudValues};
pub use palette::*;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
pub const FRAME_BYTES: usize = WIDTH * HEIGHT;

/// The ticks at which `frame_digest` is taken by `FIZZ2.EXE /BENCH` and by the `frames` example. The last is before the exit
/// banner, which would add text over the picture. Ticks 110, 118, 122 and 290 show a shot-end puff each, in its four baked frames (the shots end at 106 and 281).
pub const DIGEST_TICKS: [u64; 11] = [110, 118, 122, 150, 270, 290, 400, 602, 700, 865, 1000];

/// An FNV-1a digest of the whole frame: the shared picture and the HUD box over it. The DOS bench and the host `frames`
/// example print it for the same ticks, so the DOS drawing path (its `memset`, `memcpy` and any faster routine) is proved to
/// draw the pixels the portable code does, HUD included.
pub fn frame_digest(frame: &Frame) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in frame.pixels() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Draws the slice: the shared picture from `fizz_slice_view`, then the HUD box in the top-left corner (Fizz, deaths and the
/// key chip; see [`hud`]; it is rendered again only when one of those changes), and a banner once Ben has reached the exit. The text is this
/// presenter's own; the picture beneath it is the one the Web draws too. Captions are on and no puffs show; see
/// [`render_slice_with`].
pub fn render_slice(world: &fizz_ep2::World, frame: &mut Frame) {
    render_slice_with(
        world,
        frame,
        &fizz_slice_view::DrawOptions::default(),
        &ViewFx::new(),
    );
}

/// [`render_slice`] with the shared picture's options (the DOS `/NOCAPTIONS` switch sets `captions` false) and the shot-end
/// puffs and captions the caller has gathered with [`ViewFx::observe`] after each step.
pub fn render_slice_with(
    world: &fizz_ep2::World,
    frame: &mut Frame,
    options: &fizz_slice_view::DrawOptions,
    fx: &ViewFx,
) {
    fizz_slice_view::draw_slice_captions(
        frame.pixels_mut(),
        WIDTH,
        HEIGHT,
        world,
        world.tick.0 as u32,
        options,
        &fx.puffs,
        &fx.captions,
    );
    frame.draw_hud(HudValues {
        ammo: world.ammo,
        key: world.flags.contains(fizz_ep2::WorldFlags::KEY_RED),
        deaths: world.deaths,
    });
    if world.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED) {
        let text = "LEVEL CLEAR";
        let x = (WIDTH as i32 - text.len() as i32 * GLYPH as i32) / 2;
        frame.fill_rect(x - 4, 146, text.len() as i32 * GLYPH as i32 + 8, 16, BLACK);
        frame.draw_text(x, 150, text, YELLOW);
    }
}

/// Which keys drive the buttons. The two sets are exclusive: a key of the other layout does nothing.
///
/// `Keen` is Fizz 1's classic layout and the default everywhere. `Modern` keeps the keys away from the Ctrl and Alt
/// shortcuts that browsers and desktops take.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layout {
    /// Arrows move, Ctrl jumps, Alt pogos, Space fires.
    #[default]
    Keen,
    /// Arrows or `W` `A` `S` `D` move, `Z` jumps, `X` pogos, `C` fires; Space fires too.
    Modern,
}

/// Maps a key to the game button it drives in `layout`, or `None` for a key the layout does not use.
///
/// Arrows move in both layouts and Space fires in both; `W` `A` `S` `D` move only in `Modern`, and Ctrl and Alt are only
/// jump and pogo in `Keen`.
pub fn key_to_buttons(code: KeyCode, layout: Layout) -> Option<Buttons> {
    let modern = layout == Layout::Modern;
    match code {
        KeyCode::Up => Some(Buttons::UP),
        KeyCode::Down => Some(Buttons::DOWN),
        KeyCode::Left => Some(Buttons::LEFT),
        KeyCode::Right => Some(Buttons::RIGHT),
        KeyCode::Space => Some(Buttons::FIRE),
        KeyCode::W if modern => Some(Buttons::UP),
        KeyCode::S if modern => Some(Buttons::DOWN),
        KeyCode::A if modern => Some(Buttons::LEFT),
        KeyCode::D if modern => Some(Buttons::RIGHT),
        KeyCode::Z if modern => Some(Buttons::JUMP),
        KeyCode::X if modern => Some(Buttons::POGO),
        KeyCode::C if modern => Some(Buttons::FIRE),
        KeyCode::Ctrl if !modern => Some(Buttons::JUMP),
        KeyCode::Alt if !modern => Some(Buttons::POGO),
        _ => None,
    }
}

/// Hands the finished frame to the display.
pub fn present<D: Display>(display: &mut D, frame: &Frame) -> Result<(), HalError> {
    display.present(frame.pixels(), WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slice stepped to `ticks` with its fx fed as the DOS loop feeds them.
    fn world_at(ticks: u64) -> (fizz_ep2::World, ViewFx) {
        let mut world = fizz_ep2::level::world();
        let mut fx = ViewFx::new();
        let mut prev = Buttons::NONE;
        'run: for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                if world.tick.0 >= ticks {
                    break 'run;
                }
                let input = fizz_sim::InputFrame::from_transition(prev, held);
                fizz_ep2::step(&mut world, input);
                prev = held;
                fx.observe(&world, WIDTH, HEIGHT, &DrawOptions::default());
            }
        }
        (world, fx)
    }

    #[test]
    fn an_event_caption_is_pixels_in_the_dos_frame_and_the_option_hides_them() {
        // Tick 40 is nine ticks after Ben's first shot, so "fzzt" is alive.
        let (world, fx) = world_at(40);
        let on_options = DrawOptions::default();
        assert!(
            fx.captions
                .views(&world, WIDTH, HEIGHT, &on_options)
                .count()
                >= 1
        );
        let draw = |options: &DrawOptions, fx: &ViewFx| {
            let mut f = Frame::new();
            render_slice_with(&world, &mut f, options, fx);
            f
        };
        let on = draw(&on_options, &fx);
        let bare = draw(&on_options, &ViewFx::new());
        assert_ne!(on.pixels(), bare.pixels(), "the word is drawn");
        let off = DrawOptions {
            captions: false,
            ..DrawOptions::default()
        };
        assert_eq!(
            draw(&off, &fx).pixels(),
            draw(&off, &ViewFx::new()).pixels(),
            "off hides the text"
        );
        // The word is light cyan (11) over a black (0) shadow.
        let (a, b) = (on.pixels(), bare.pixels());
        let differing = || (0..a.len()).filter(|&i| a[i] != b[i]);
        assert!(differing().any(|i| a[i] == 11));
        assert!(differing().all(|i| matches!(a[i], 0 | 11)));
    }

    const ALL_KEYS: [KeyCode; 14] = [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::W,
        KeyCode::A,
        KeyCode::S,
        KeyCode::D,
        KeyCode::Space,
        KeyCode::Z,
        KeyCode::X,
        KeyCode::C,
        KeyCode::Ctrl,
        KeyCode::Alt,
    ];

    fn check(layout: Layout, table: &[(KeyCode, Buttons)]) {
        for key in ALL_KEYS {
            let want = table.iter().find(|(k, _)| *k == key).map(|(_, b)| *b);
            assert_eq!(key_to_buttons(key, layout), want, "{layout:?} {key:?}");
        }
    }

    #[test]
    fn keen_layout_is_arrows_ctrl_alt_space() {
        check(
            Layout::Keen,
            &[
                (KeyCode::Up, Buttons::UP),
                (KeyCode::Down, Buttons::DOWN),
                (KeyCode::Left, Buttons::LEFT),
                (KeyCode::Right, Buttons::RIGHT),
                (KeyCode::Ctrl, Buttons::JUMP),
                (KeyCode::Alt, Buttons::POGO),
                (KeyCode::Space, Buttons::FIRE),
            ],
        );
    }

    #[test]
    fn modern_layout_is_wasd_or_arrows_zxc() {
        check(
            Layout::Modern,
            &[
                (KeyCode::Up, Buttons::UP),
                (KeyCode::W, Buttons::UP),
                (KeyCode::Down, Buttons::DOWN),
                (KeyCode::S, Buttons::DOWN),
                (KeyCode::Left, Buttons::LEFT),
                (KeyCode::A, Buttons::LEFT),
                (KeyCode::Right, Buttons::RIGHT),
                (KeyCode::D, Buttons::RIGHT),
                (KeyCode::Z, Buttons::JUMP),
                (KeyCode::X, Buttons::POGO),
                (KeyCode::C, Buttons::FIRE),
                (KeyCode::Space, Buttons::FIRE),
            ],
        );
    }

    #[test]
    fn the_default_layout_is_keen() {
        assert_eq!(Layout::default(), Layout::Keen);
    }

    #[test]
    fn unmapped_keys_drive_nothing() {
        for layout in [Layout::Keen, Layout::Modern] {
            assert_eq!(key_to_buttons(KeyCode::Escape, layout), None);
            let other = KeyCode::Unknown {
                extended: false,
                scancode: 0x1e,
            };
            assert_eq!(key_to_buttons(other, layout), None);
        }
    }

    #[test]
    fn the_slice_hud_is_a_box_over_the_whole_picture_with_no_tick() {
        let mut world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        render_slice(&world, &mut frame);
        // Outside the box the picture is the shared one, from the very first row (there is no bar along the top any more).
        let mut bare = Frame::new();
        fizz_slice_view::draw_slice(bare.pixels_mut(), WIDTH, HEIGHT, &world, 0);
        let same_outside = |a: &Frame, b: &Frame| {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let inside = (hud::X..hud::X + hud::W).contains(&x)
                        && (hud::Y..hud::Y + hud::H).contains(&y);
                    assert!(inside || a.pixels()[y * WIDTH + x] == b.pixels()[y * WIDTH + x]);
                }
            }
        };
        same_outside(&frame, &bare);
        // The box shows light cyan Fizz, no red, and the key as an outline; with the key it shows light red and no TICK anywhere.
        let in_box = |f: &Frame, colour: u8| {
            (hud::Y..hud::Y + hud::H)
                .flat_map(|y| (hud::X..hud::X + hud::W).map(move |x| (x, y)))
                .filter(|&(x, y)| f.pixels()[y * WIDTH + x] == colour)
                .count()
        };
        assert!(in_box(&frame, fizz_view::theme::FIZZ_INDEX) > 0);
        assert_eq!(in_box(&frame, LIGHT_RED), 0);
        world.flags.set(fizz_ep2::WorldFlags::KEY_RED);
        render_slice(&world, &mut frame);
        assert!(in_box(&frame, LIGHT_RED) > 0);
        world.ammo = 0;
        render_slice(&world, &mut frame);
        assert_eq!(in_box(&frame, fizz_view::theme::FIZZ_INDEX), 0);
        // The right-hand end of the old bar, where TICK was, is now picture.
        same_outside(&frame, &bare);
    }

    #[test]
    fn the_box_shows_the_picture_dimmed_and_only_it_uses_the_dimmed_half_of_the_palette() {
        let world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        render_slice(&world, &mut frame);
        let mut bare = Frame::new();
        fizz_slice_view::draw_slice(bare.pixels_mut(), WIDTH, HEIGHT, &world, 0);
        let mut dimmed_seen = 0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let (p, q) = (frame.pixels()[y * WIDTH + x], bare.pixels()[y * WIDTH + x]);
                assert!(q < 16, "the shared picture uses the sixteen colours");
                let inside = (hud::X..hud::X + hud::W).contains(&x)
                    && (hud::Y..hud::Y + hud::H).contains(&y);
                if p >= 16 {
                    assert!(inside, "({x},{y})");
                    assert_eq!(p, dimmed(q));
                    dimmed_seen += 1;
                }
            }
        }
        assert!(dimmed_seen > hud::W * hud::H / 2);
    }

    #[test]
    fn the_hud_is_rendered_only_when_its_values_change() {
        let mut world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        for _ in 0..5 {
            render_slice(&world, &mut frame);
        }
        assert_eq!(frame.hud_renders(), 1);
        world.tick.0 += 100;
        render_slice(&world, &mut frame);
        assert_eq!(frame.hud_renders(), 1, "the tick is not on the HUD");
        world.deaths += 1;
        render_slice(&world, &mut frame);
        assert_eq!(frame.hud_renders(), 2);
        world.deaths -= 1;
        render_slice(&world, &mut frame);
        assert_eq!(frame.hud_renders(), 3);
    }

    #[test]
    fn the_slice_banner_appears_at_the_exit() {
        let mut world = fizz_ep2::level::world();
        let mut frame = Frame::new();
        render_slice(&world, &mut frame);
        let before = frame.pixels().to_vec();
        world.flags.set(fizz_ep2::WorldFlags::EXIT_REACHED);
        render_slice(&world, &mut frame);
        assert_ne!(before, frame.pixels());
        assert!(frame.pixels().contains(&YELLOW));
    }
}
