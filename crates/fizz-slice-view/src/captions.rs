// Event captions for the Episode 2 slice: the event table and the live list.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! What each rules event looks and sounds like to a presenter: its effect id, and the caption it raises, if any; and the
//! [`EventCaptions`] list that keeps the live ones.
//!
//! Fizz 1 pairs every in-world sound with a floating word (`World::cap(x, y, Cap)` in its `world.rs`, the words and colours in
//! `text.rs`). Fizz 2's rules emit `fizz_ep2::Event`s instead; this module is the one place that says which event gets which
//! effect and word, so both presenters agree. Nothing here is rules state: it is not hashed and the rules never read it.
//!
//! ## Fizz 1's rules, and what they become here
//!
//! - **Same caption within 180 ms is dropped.** At 60 ticks a second that is 10.8, so [`DEDUPE_TICKS`] is 11. The id is the
//!   word, so the door and the head-bump (both "clunk") dedupe each other, as in Fizz 1.
//! - **More than 50 CSS pixels outside the view is dropped.** Fizz 1's viewport is about 1000 CSS pixels wide, so 50 is 5%; of
//!   this 320-pixel picture that is 16 pixels, [`CAPTION_REACH`], the same reach the cluck caption already uses. The test is
//!   made once, when the caption is raised, against the camera of that tick; after that the caption stays where it was raised
//!   in the world and the camera moves under it.
//! - **Life.** Fizz 1's Web text lives 950 ms (57 ticks) and rises 24 CSS pixels (12 here). Fizz 1's DOS port uses 45 ticks.
//!   Both platforms here use 57 ([`CAPTION_TICKS`]), the cluck caption's life, so that the Web and DOS show a caption for the
//!   same ticks and the two stay identical.
//! - **Six slots, no priority.** A full list reuses slot 0, as Fizz 1's DOS port does. This list first takes a slot whose caption
//!   is over, so only a seventh live caption replaces slot 0.
//! - **Sound ignores the Captions setting.** The setting gates the text only; the list is fed either way.

use crate::{
    BLUE, BROWN, CAPTION_REACH, CAPTION_TICKS, CYAN, CaptionView, DARK_GREY, DrawOptions,
    LIGHT_BLUE, LIGHT_CYAN, LIGHT_GREEN, LIGHT_GREY, LIGHT_RED, RED, WHITE, YELLOW, camera_shaken,
};
use fizz_ep2::{Event, World};
use fizz_view::TILE_PX;

const GREEN: u8 = 2;

// Effect ids, fixed by the milestone plan; the sound path (not here) plays by these.
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

/// A caption's word and colours: the bright one (Fizz 1's colour) and the dark one the DOS picture fades to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventCaption {
    pub text: &'static str,
    pub bright: u8,
    pub dark: u8,
}

/// Indices into [`EVENT_CAPTIONS`]; the index is the caption's id for dedupe.
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

const fn cap(text: &'static str, bright: u8, dark: u8) -> EventCaption {
    EventCaption { text, bright, dark }
}

/// Fizz 1's `CAPTIONS` words and colours (its 0xrrggbb values are the EGA light colours), in id order. The dash in the click
/// caption is the font's em dash.
pub const EVENT_CAPTIONS: [EventCaption; 12] = [
    cap("fzzt", LIGHT_CYAN, CYAN),
    cap("click \u{2014} no fizz", LIGHT_GREY, DARK_GREY),
    cap("fizzled", LIGHT_CYAN, CYAN),
    cap("bonk", WHITE, LIGHT_GREY),
    cap("whoa!", LIGHT_RED, RED),
    cap("ta-da!", LIGHT_GREEN, GREEN),
    cap("red gumdrop", LIGHT_RED, RED),
    cap("blue gumdrop", LIGHT_BLUE, BLUE),
    cap("green gumdrop", LIGHT_GREEN, GREEN),
    cap("clunk", WHITE, LIGHT_GREY),
    cap("boing", YELLOW, BROWN),
    cap("crunch", YELLOW, BROWN),
];

/// What an event is to a presenter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Presentation {
    /// The sound effect id (see the `EFFECT_` constants). A shot ending has no sound of its own; it is `None` there.
    pub effect: Option<u8>,
    /// The caption id (an index into [`EVENT_CAPTIONS`]), or `None`: a cluck keeps its caption on its chicken, and a jump has none.
    pub caption: Option<u8>,
}

const fn p(effect: u8, caption: Option<u8>) -> Presentation {
    Presentation {
        effect: Some(effect),
        caption,
    }
}

/// The effect and caption of `event`. Pure.
pub const fn presentation(event: &Event) -> Presentation {
    match *event {
        Event::Cluck { variant, .. } => p(EFFECT_CLUCK + variant, None),
        Event::ShotEnd { .. } => Presentation {
            effect: None,
            caption: None,
        },
        Event::Fire { .. } => p(EFFECT_FZZT, Some(CAP_FZZT)),
        Event::Dud { .. } => p(EFFECT_PLINK, Some(CAP_CLICK)),
        Event::Stun { .. } => p(EFFECT_STUN, Some(CAP_FIZZLED)),
        Event::Stomp { .. } => p(EFFECT_BONK, Some(CAP_BONK)),
        Event::Death { .. } => p(EFFECT_HURT, Some(CAP_WHOA)),
        Event::Exit { .. } => p(EFFECT_WIN, Some(CAP_TADA)),
        Event::Key { colour, .. } => p(
            EFFECT_KEY,
            Some(match colour {
                0 => CAP_RED,
                1 => CAP_BLUE,
                _ => CAP_GREEN,
            }),
        ),
        Event::DoorOpen { .. } | Event::HeadBump { .. } => p(EFFECT_CLUNK, Some(CAP_CLUNK)),
        Event::Bounce { .. } => p(EFFECT_BOING, Some(CAP_BOING)),
        Event::Jump { .. } => p(EFFECT_JUMP, None),
        Event::Snack { .. } => p(EFFECT_CRUNCH, Some(CAP_CRUNCH)),
    }
}

/// Where an event happened, in sixteenths of a tile, y up; `None` for events without a point (a cluck: its chicken has it).
pub const fn event_point(event: &Event) -> Option<(i32, i32)> {
    match *event {
        Event::Cluck { .. } => None,
        Event::ShotEnd { x, y }
        | Event::Fire { x, y }
        | Event::Dud { x, y }
        | Event::Stomp { x, y, .. }
        | Event::Stun { x, y, .. }
        | Event::Death { x, y }
        | Event::Key { x, y, .. }
        | Event::DoorOpen { x, y }
        | Event::Exit { x, y }
        | Event::Bounce { x, y }
        | Event::HeadBump { x, y }
        | Event::Jump { x, y }
        | Event::Snack { x, y } => Some((x, y)),
    }
}

/// Ticks within which a second caption of the same word is dropped: Fizz 1's 180 ms, 10.8 ticks, rounded to 11.
pub const DEDUPE_TICKS: u32 = 11;
/// The slots in the list, Fizz 1's DOS port's.
pub const MAX_EVENT_CAPTIONS: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slot {
    caption: u8,
    /// Where it was raised, in map pixels (x right, y down).
    x: i32,
    y: i32,
    born: u32,
}

/// The event captions on show: a fixed list of [`MAX_EVENT_CAPTIONS`], no allocation, fed like [`crate::Puffs`] by calling
/// [`EventCaptions::observe`] after every `fizz_ep2::step`. View state: never hashed, and the rules never see it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventCaptions {
    slots: [Option<Slot>; MAX_EVENT_CAPTIONS],
    /// The tick each caption id was last raised, for the dedupe. A dropped caption does not renew it.
    last: [Option<u32>; EVENT_CAPTIONS.len()],
}

impl Default for EventCaptions {
    fn default() -> Self {
        Self::new()
    }
}

impl EventCaptions {
    pub const fn new() -> Self {
        EventCaptions {
            slots: [None; MAX_EVENT_CAPTIONS],
            last: [None; EVENT_CAPTIONS.len()],
        }
    }

    /// Forgets everything (a presenter restarting the level).
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Takes the events of the step that just ran, for a `w` by `h` view drawn with `options`. Call it whether or not captions
    /// are shown; the Captions setting hides text only.
    pub fn observe(&mut self, world: &World, w: usize, h: usize, options: &DrawOptions) {
        let now = world.tick.0 as u32;
        let (cam_x, cam_y) = camera_shaken(world, w, h, options);
        let map_h_px = world.map.h.saturating_mul(TILE_PX);
        for e in world.events() {
            let (Some(id), Some((px, py))) = (presentation(e).caption, event_point(e)) else {
                continue;
            };
            if self.last[usize::from(id)].is_some_and(|t| now.wrapping_sub(t) < DEDUPE_TICKS) {
                continue;
            }
            let (x, y) = (px, map_h_px - py);
            let (sx, sy) = (x - cam_x, y - cam_y);
            if sx < -CAPTION_REACH
                || sx > w as i32 + CAPTION_REACH
                || sy < -CAPTION_REACH
                || sy > h as i32 + CAPTION_REACH
            {
                continue;
            }
            self.last[usize::from(id)] = Some(now);
            let at = self
                .slots
                .iter()
                .position(|s| s.is_none_or(|s| now.wrapping_sub(s.born) >= CAPTION_TICKS))
                .unwrap_or(0);
            self.slots[at] = Some(Slot {
                caption: id,
                x,
                y,
                born: now,
            });
        }
    }

    /// The captions still showing at `world.tick`, placed (unclamped, in frame pixels) for a `w` by `h` view drawn with
    /// `options`. Slot order, which is not age order; presenters need no order.
    pub fn views<'a>(
        &'a self,
        world: &World,
        w: usize,
        h: usize,
        options: &DrawOptions,
    ) -> impl Iterator<Item = CaptionView> + 'a {
        let now = world.tick.0 as u32;
        let (cam_x, cam_y) = camera_shaken(world, w, h, options);
        self.slots.iter().filter_map(move |s| {
            let s = (*s)?;
            let age = now.wrapping_sub(s.born);
            if age >= CAPTION_TICKS {
                return None;
            }
            let c = EVENT_CAPTIONS[usize::from(s.caption)];
            Some(CaptionView {
                text: c.text,
                bright: c.bright,
                dark: c.dark,
                age,
                x: s.x - cam_x,
                y: s.y - cam_y,
                event: true,
            })
        })
    }
}

/// Everything a presenter that draws the whole picture itself (the DOS one) feeds from the step events: the shot-end puffs and
/// the event captions. One `observe` after every `fizz_ep2::step`, one `&ViewFx` to the renderer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ViewFx {
    pub puffs: crate::Puffs,
    pub captions: EventCaptions,
}

impl ViewFx {
    pub const fn new() -> Self {
        ViewFx {
            puffs: crate::Puffs::new(),
            captions: EventCaptions::new(),
        }
    }

    /// Takes the events of the step that just ran, for a `w` by `h` picture drawn with `options`.
    pub fn observe(&mut self, world: &World, w: usize, h: usize, options: &DrawOptions) {
        self.puffs.observe(world);
        self.captions.observe(world, w, h, options);
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests;
