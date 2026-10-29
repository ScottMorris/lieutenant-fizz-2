// Tests for the event captions: per-tick timelines over the slice script.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::{Puffs, VIEW_H, VIEW_W, draw_slice_captions, draw_slice_with};
use fizz_ep2::world::step;
use fizz_ep2::{SLICE_SCRIPT, level};
use fizz_sim::{Buttons, InputFrame};

extern crate std;
use std::vec;
use std::vec::Vec;

fn opts() -> DrawOptions {
    DrawOptions::default()
}

/// Plays the slice script, calling `each` after every step and observe.
fn play(mut each: impl FnMut(&World, &EventCaptions)) {
    let mut w = level::world();
    let mut caps = EventCaptions::new();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            caps.observe(&w, VIEW_W, VIEW_H, &opts());
            each(&w, &caps);
        }
    }
}

/// The replay's captions as `(tick, word)`, at the tick each is raised.
fn timeline() -> Vec<(u32, &'static str)> {
    let mut out = Vec::new();
    play(|w, caps| {
        for v in caps.views(w, VIEW_W, VIEW_H, &opts()) {
            if v.age == 0 {
                out.push((w.tick.0 as u32, v.text));
            }
        }
    });
    out
}

/// A world one step in, whose step emitted a `Fire` at Ben (fire held from the start).
fn world_with_fire() -> World {
    let mut w = level::world();
    step(
        &mut w,
        InputFrame::from_transition(Buttons::NONE, Buttons::FIRE),
    );
    assert!(w.events().iter().any(|e| matches!(e, Event::Fire { .. })));
    w
}

fn slot(caption: u8, born: u32) -> Option<Slot> {
    Some(Slot {
        caption,
        x: 100,
        y: 100,
        born,
    })
}

#[test]
fn the_table_maps_every_event_as_the_plan_says() {
    let at = |e: Event| presentation(&e);
    let cap = |e: Event| at(e).caption.map(|i| EVENT_CAPTIONS[usize::from(i)].text);
    let (x, y) = (0, 0);
    let cluck = Event::Cluck {
        variant: 3,
        chicken: 0,
    };
    assert_eq!(at(cluck).effect, Some(3));
    assert_eq!(cap(cluck), None);
    assert_eq!(at(Event::Fire { x, y }).effect, Some(4));
    assert_eq!(cap(Event::Fire { x, y }), Some("fzzt"));
    assert_eq!(cap(Event::Dud { x, y }), Some("click \u{2014} no fizz"));
    assert_eq!(at(Event::Dud { x, y }).effect, Some(11));
    let stun = Event::Stun { chicken: 0, x, y };
    assert_eq!(at(stun).effect, Some(5));
    assert_eq!(cap(stun), Some("fizzled"));
    let stomp = Event::Stomp { chicken: 0, x, y };
    assert_eq!(at(stomp).effect, Some(6));
    assert_eq!(cap(stomp), Some("bonk"));
    assert_eq!(at(Event::Death { x, y }).effect, Some(7));
    assert_eq!(cap(Event::Death { x, y }), Some("whoa!"));
    assert_eq!(at(Event::Exit { x, y }).effect, Some(8));
    assert_eq!(cap(Event::Exit { x, y }), Some("ta-da!"));
    for (colour, word) in [
        (0, "red gumdrop"),
        (1, "blue gumdrop"),
        (2, "green gumdrop"),
    ] {
        let key = Event::Key { colour, x, y };
        assert_eq!(at(key).effect, Some(9));
        assert_eq!(cap(key), Some(word));
    }
    assert_eq!(at(Event::DoorOpen { x, y }).effect, Some(10));
    assert_eq!(cap(Event::DoorOpen { x, y }), Some("clunk"));
    assert_eq!(cap(Event::HeadBump { x, y }), Some("clunk"));
    assert_eq!(at(Event::Bounce { x, y }).effect, Some(12));
    assert_eq!(cap(Event::Bounce { x, y }), Some("boing"));
    assert_eq!(at(Event::Jump { x, y }).effect, Some(13));
    assert_eq!(cap(Event::Jump { x, y }), None);
    assert_eq!(at(Event::Snack { x, y }).effect, Some(14));
    assert_eq!(cap(Event::Snack { x, y }), Some("crunch"));
    assert_eq!(at(Event::ShotEnd { x, y }).effect, None);
}

#[test]
fn the_replay_raises_these_captions_on_these_ticks() {
    let got = timeline();
    // The fire events at 1129 and 1149 are dropped: the same word came 10 ticks before each.
    let want: &[(u32, &str)] = &[
        (31, "fzzt"),
        (61, "fzzt"),
        (281, "fzzt"),
        (281, "fizzled"),
        (295, "red gumdrop"),
        (394, "whoa!"),
        (658, "bonk"),
        (675, "red gumdrop"),
        (890, "clunk"),
        (1119, "fzzt"),
        (1139, "fzzt"),
        (1159, "fzzt"),
        (1169, "click \u{2014} no fizz"),
        (1192, "crunch"),
        (1196, "clunk"),
        (1260, "crunch"),
        (1264, "clunk"),
        (1351, "boing"),
        (1379, "boing"),
        (1407, "boing"),
        (1447, "crunch"),
        (1452, "boing"),
        (1472, "crunch"),
        (1480, "boing"),
        (1500, "crunch"),
        (1503, "blue gumdrop"),
        (1508, "boing"),
        (1539, "clunk"),
        (1548, "boing"),
        (1553, "clunk"),
        (1565, "boing"),
        (1570, "clunk"),
        (1582, "boing"),
        (1588, "ta-da!"),
    ];
    assert_eq!(got, want);
}

#[test]
fn a_caption_lives_exactly_its_life() {
    // Every live caption's age is its tick minus the tick it was raised, and none shows at or past CAPTION_TICKS.
    let mut births: Vec<(u32, &str)> = Vec::new();
    let mut longest = 0;
    play(|w, caps| {
        for v in caps.views(w, VIEW_W, VIEW_H, &opts()) {
            assert!(v.age < CAPTION_TICKS);
            longest = longest.max(v.age);
            if v.age == 0 {
                births.push((w.tick.0 as u32, v.text));
            }
        }
    });
    assert_eq!(longest, CAPTION_TICKS - 1);
    assert!(!births.is_empty());
}

#[test]
fn a_full_list_replaces_slot_zero_but_first_takes_an_expired_slot() {
    let w = world_with_fire();
    let tick = w.tick.0 as u32;
    let mut caps = EventCaptions::new();
    caps.slots = [
        slot(CAP_BONK, tick),
        slot(CAP_WHOA, tick),
        slot(CAP_TADA, tick),
        slot(CAP_RED, tick),
        slot(CAP_BLUE, tick),
        slot(CAP_CLUNK, tick),
    ];
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(
        caps.slots[0].map(|s| s.caption),
        Some(CAP_FZZT),
        "full: slot 0 is reused"
    );
    assert_eq!(caps.slots[1].map(|s| s.caption), Some(CAP_WHOA));

    let mut caps = EventCaptions::new();
    caps.slots = [
        slot(CAP_BONK, tick),
        slot(CAP_WHOA, tick),
        slot(CAP_TADA, tick.wrapping_sub(CAPTION_TICKS)),
        slot(CAP_RED, tick),
        slot(CAP_BLUE, tick),
        slot(CAP_CLUNK, tick),
    ];
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.slots[0].map(|s| s.caption), Some(CAP_BONK));
    assert_eq!(
        caps.slots[2].map(|s| s.caption),
        Some(CAP_FZZT),
        "expired slot first"
    );
}

#[test]
fn the_same_word_within_eleven_ticks_is_dropped() {
    let w = world_with_fire();
    let tick = w.tick.0 as u32;
    // The same word raised `DEDUPE_TICKS - 1` ticks ago blocks it; `DEDUPE_TICKS` ago does not.
    let mut caps = EventCaptions::new();
    caps.last[usize::from(CAP_FZZT)] = Some(tick.wrapping_sub(DEDUPE_TICKS - 1));
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.views(&w, VIEW_W, VIEW_H, &opts()).count(), 0);
    let mut caps = EventCaptions::new();
    caps.last[usize::from(CAP_FZZT)] = Some(tick.wrapping_sub(DEDUPE_TICKS));
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.views(&w, VIEW_W, VIEW_H, &opts()).count(), 1);
    // A different word is not blocked by it.
    let mut caps = EventCaptions::new();
    caps.last[usize::from(CAP_BONK)] = Some(tick);
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.views(&w, VIEW_W, VIEW_H, &opts()).count(), 1);
}

#[test]
fn clunk_from_a_door_and_from_a_head_bump_dedupe_each_other() {
    assert_eq!(
        presentation(&Event::DoorOpen { x: 0, y: 0 }).caption,
        presentation(&Event::HeadBump { x: 0, y: 0 }).caption
    );
}

#[test]
fn the_replay_never_shows_the_same_word_twice_within_eleven_ticks() {
    let t = timeline();
    for (i, a) in t.iter().enumerate() {
        for b in &t[i + 1..] {
            if a.1 == b.1 {
                assert!(b.0 - a.0 >= DEDUPE_TICKS, "{a:?} {b:?}");
            }
        }
    }
}

#[test]
fn a_caption_outside_the_reach_is_dropped_at_birth() {
    let w = world_with_fire();
    let mut caps = EventCaptions::new();
    caps.observe(&w, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.views(&w, VIEW_W, VIEW_H, &opts()).count(), 1);
    // The same event seen by a camera 40 tiles away is off screen: nothing is raised.
    let mut far = w.clone();
    far.ben.b.x += 40.0;
    let mut caps = EventCaptions::new();
    caps.observe(&far, VIEW_W, VIEW_H, &opts());
    assert_eq!(caps.views(&far, VIEW_W, VIEW_H, &opts()).count(), 0);
}

#[test]
fn the_reach_is_sixteen_pixels() {
    // 50 CSS pixels of a roughly 1000 pixel wide viewport, in a 320 pixel picture.
    assert_eq!(50 * VIEW_W as i32 / 1000, 16);
    assert_eq!(CAPTION_REACH, 16);
}

#[test]
fn captions_off_hides_only_the_text() {
    let off = DrawOptions {
        captions: false,
        ..opts()
    };
    let mut seen = false;
    play(|w, caps| {
        if !seen && w.tick.0 > 1000 && caps.views(w, VIEW_W, VIEW_H, &opts()).count() > 0 {
            seen = true;
            let t = w.tick.0 as u32;
            let mut with = vec![0u8; VIEW_W * VIEW_H];
            let mut without = vec![0u8; VIEW_W * VIEW_H];
            let mut bare = vec![0u8; VIEW_W * VIEW_H];
            draw_slice_captions(
                &mut with,
                VIEW_W,
                VIEW_H,
                w,
                t,
                &opts(),
                &Puffs::new(),
                caps,
            );
            draw_slice_captions(
                &mut without,
                VIEW_W,
                VIEW_H,
                w,
                t,
                &off,
                &Puffs::new(),
                caps,
            );
            draw_slice_with(&mut bare, VIEW_W, VIEW_H, w, t, &off);
            assert_eq!(without, bare, "off draws the picture without the words");
            assert_ne!(with, without, "on draws the words");
        }
    });
    assert!(seen);
}

#[test]
fn the_list_is_fed_whatever_the_captions_option() {
    // `observe` takes the options only for the camera: a list fed with captions off holds the same captions.
    let off = DrawOptions {
        captions: false,
        ..opts()
    };
    let w = world_with_fire();
    let (mut a, mut b) = (EventCaptions::new(), EventCaptions::new());
    a.observe(&w, VIEW_W, VIEW_H, &opts());
    b.observe(&w, VIEW_W, VIEW_H, &off);
    assert_eq!(a, b);
}

#[test]
fn the_word_is_drawn_inside_its_cell_around_the_event_point() {
    let mut found: Option<(Vec<u8>, Vec<u8>, CaptionView)> = None;
    play(|w, caps| {
        if found.is_none()
            && let Some(v) = caps.views(w, VIEW_W, VIEW_H, &opts()).find(|v| v.age == 20)
        {
            let t = w.tick.0 as u32;
            let mut a = vec![0u8; VIEW_W * VIEW_H];
            let mut b = vec![0u8; VIEW_W * VIEW_H];
            draw_slice_captions(&mut a, VIEW_W, VIEW_H, w, t, &opts(), &Puffs::new(), caps);
            let none = EventCaptions::new();
            draw_slice_captions(&mut b, VIEW_W, VIEW_H, w, t, &opts(), &Puffs::new(), &none);
            found = Some((a, b, v));
        }
    });
    let (a, b, v) = found.expect("a caption 20 ticks old");
    let rise = (v.age * CAPTION_RISE_PX / CAPTION_TICKS) as i32;
    let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for i in 0..a.len() {
        if a[i] != b[i] {
            let (x, y) = ((i % VIEW_W) as i32, (i / VIEW_W) as i32);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
    }
    let width = fizz_view::pfont::text_width(v.text);
    let left = v.x - width / 2;
    let top = v.y - 4 - rise;
    assert!(
        x0 >= left && x1 <= left + width + 1,
        "x {x0}..{x1} vs {left}+{width}"
    );
    assert!(y0 >= top && y1 <= top + 9, "y {y0}..{y1} vs {top}");
}

const CAPTION_RISE_PX: u32 = crate::CAPTION_RISE as u32;
