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
    assert_eq!(at(Event::Can { x, y }).effect, Some(17));
    assert_eq!(cap(Event::Can { x, y }), Some("fsssht"));
    assert_eq!(at(Event::ShotEnd { x, y }).effect, None);
}

#[test]
fn the_replay_raises_these_captions_on_these_ticks() {
    let got = timeline();
    // The fire events at 1129 and 1149 are dropped: the same word came 10 ticks before each. After the can at 1190 the blaster fires five more times, and the ones at 1205 and 1225 are dropped the same way.
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
        (1190, "fsssht"),
        (1195, "fzzt"),
        (1215, "fzzt"),
        (1235, "fzzt"),
        (1274, "crunch"),
        (1278, "clunk"),
        (1336, "crunch"),
        (1340, "clunk"),
        (1427, "boing"),
        (1455, "boing"),
        (1483, "boing"),
        (1522, "fsssht"),
        (1523, "crunch"),
        (1528, "boing"),
        (1548, "crunch"),
        (1556, "boing"),
        (1576, "crunch"),
        (1579, "blue gumdrop"),
        (1584, "boing"),
        (1615, "clunk"),
        (1624, "boing"),
        (1629, "clunk"),
        (1641, "boing"),
        (1646, "clunk"),
        (1658, "boing"),
        (1664, "ta-da!"),
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

/// Every sound the replay makes, as `(tick, effect id, placement)` in event order, with the Captions option as given.
fn replay_sounds(captions: bool) -> Vec<(u32, u8, fizz_sfx::place::Placement)> {
    let options = DrawOptions { captions, ..opts() };
    let mut w = level::world();
    let mut out = Vec::new();
    let mut prev = Buttons::NONE;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            for e in w.events() {
                if let Some((id, place)) = event_sound(&w, e, VIEW_W, VIEW_H, &options) {
                    out.push((w.tick.0 as u32, id, place));
                }
            }
        }
    }
    out
}

#[test]
fn the_replay_fills_all_six_caption_slots_at_its_busiest() {
    let mut most = 0;
    play(|w, caps| {
        most = most.max(caps.views(w, VIEW_W, VIEW_H, &opts()).count());
    });
    // Snacks, bounces, head-bumps and a key follow one another in the nook, so the list is full at least once. `/BENCH` draws the
    // worst case (six of the widest words) at a fixed picture to time it.
    assert_eq!(most, MAX_EVENT_CAPTIONS);
}

#[test]
fn show_six_fills_every_slot_inside_the_view_with_the_widest_words() {
    let w = world_with_fire();
    let o = opts();
    // The six chosen are the six widest of the twelve words (a tie at the bottom may be broken either way).
    let width = |id: u8| fizz_view::pfont::text_width(EVENT_CAPTIONS[usize::from(id)].text);
    let mut all: Vec<u8> = (0..EVENT_CAPTIONS.len() as u8).collect();
    all.sort_by_key(|&id| core::cmp::Reverse(width(id)));
    let sixth = width(all[5]);
    for id in SIX_WIDEST {
        assert!(
            width(id) >= sixth,
            "{} is narrower than the sixth widest",
            EVENT_CAPTIONS[usize::from(id)].text
        );
    }
    let mut caps = EventCaptions::new();
    caps.show_six(&w, VIEW_W, VIEW_H, &o);
    let views: Vec<CaptionView> = caps.views(&w, VIEW_W, VIEW_H, &o).collect();
    assert_eq!(views.len(), MAX_EVENT_CAPTIONS);
    for v in &views {
        assert!(v.event && v.age <= 6);
        assert!(
            v.x >= 0 && v.x < VIEW_W as i32 && v.y >= 0 && v.y < VIEW_H as i32,
            "{v:?}"
        );
    }
    let mut words: Vec<&str> = views.iter().map(|v| v.text).collect();
    words.sort_unstable();
    words.dedup();
    assert_eq!(words.len(), 6, "six different words");
    // They are drawn: the picture differs from the same world with none on show.
    let (mut a, mut b) = (vec![0u8; VIEW_W * VIEW_H], vec![0u8; VIEW_W * VIEW_H]);
    let t = w.tick.0 as u32;
    draw_slice_captions(
        &mut a,
        VIEW_W,
        VIEW_H,
        &w,
        t,
        &o,
        &Puffs::new(),
        &EventCaptions::new(),
    );
    draw_slice_captions(&mut b, VIEW_W, VIEW_H, &w, t, &o, &Puffs::new(), &caps);
    assert_ne!(a, b);
}

#[test]
fn the_effect_ids_are_the_sound_tables_order() {
    // Eleven effects follow the cluck's four variants, then the alternates (boing (FM), jump (FM)), soda and the alternate fzzt (FM), which
    // no event but the soda plays, and every id the table hands out is one the sound data has.
    assert_eq!(EFFECT_FZZT_FM as usize + 1, fizz_sfx::EFFECT_COUNT);
    assert_eq!(EFFECT_BOING_FM, EFFECT_CRUNCH + 1);
    assert_eq!(EFFECT_JUMP_FM, EFFECT_BOING_FM + 1);
    assert_eq!(EFFECT_SODA, EFFECT_JUMP_FM + 1);
    assert_eq!(EFFECT_FZZT_FM, EFFECT_SODA + 1);
    assert_eq!(EFFECT_CLUCK, 0);
    assert_eq!(fizz_ep2::CLUCK_VARIANTS, EFFECT_FZZT);
    let ids = [
        EFFECT_FZZT,
        EFFECT_STUN,
        EFFECT_BONK,
        EFFECT_HURT,
        EFFECT_WIN,
        EFFECT_KEY,
        EFFECT_CLUNK,
        EFFECT_PLINK,
        EFFECT_BOING,
        EFFECT_JUMP,
        EFFECT_CRUNCH,
    ];
    assert_eq!(ids, core::array::from_fn(|i| EFFECT_FZZT + i as u8));
}

#[test]
fn every_event_in_the_replay_but_a_shot_ending_plays_one_effect() {
    // The golden transcript's EVENT lines are the contract: each line but `shotend` is one sound, in order, and nothing else is.
    let golden = include_str!("../../../fizz-ep2/slice-replay.txt");
    let want: Vec<(u32, &str)> = golden
        .lines()
        .filter_map(|l| l.strip_prefix("EVENT "))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let tick = it.next()?.parse().ok()?;
            let name = it.next()?;
            (name != "shotend").then_some((tick, name))
        })
        .collect();
    let got = replay_sounds(true);
    assert_eq!(got.len(), want.len(), "one effect per sounding event");
    let name_of = |id: u8| match id {
        0..=3 => "cluck",
        EFFECT_FZZT => "fire",
        EFFECT_STUN => "stun",
        EFFECT_BONK => "stomp",
        EFFECT_HURT => "death",
        EFFECT_WIN => "exit",
        EFFECT_KEY => "key",
        EFFECT_CLUNK => "clunk",
        EFFECT_PLINK => "dud",
        EFFECT_BOING => "bounce",
        EFFECT_JUMP => "jump",
        EFFECT_CRUNCH => "snack",
        EFFECT_SODA => "can",
        _ => "?",
    };
    for ((tick, id, _), (want_tick, want_name)) in got.iter().zip(&want) {
        assert_eq!(tick, want_tick);
        let name = name_of(*id);
        let ok = name == *want_name
            || (name == "clunk" && matches!(*want_name, "dooropen" | "headbump"));
        assert!(
            ok,
            "tick {tick}: effect {id} ({name}) for a {want_name} event"
        );
        assert!(usize::from(*id) < fizz_sfx::EFFECT_COUNT);
    }
}

#[test]
fn the_replay_plays_these_effects_this_often() {
    let mut count = [0u32; fizz_sfx::EFFECT_COUNT];
    for (_, id, _) in replay_sounds(true) {
        count[usize::from(id)] += 1;
    }
    // The five clucks (variant 0 twice, the others once each), then fzzt, fizzled, bonk, whoa!, ta-da!, the keys, clunks (two doors and four
    // head-bumps), click, boings, jumps and crunches.
    assert_eq!(count[..4], [2, 1, 1, 1]);
    assert_eq!(
        count[4..],
        [13, 1, 1, 1, 1, 3, 6, 1, 9, count[13], 5, 0, 0, 2, 0],
        "fzzt, stun, bonk, hurt, win, key, clunk, plink, boing, jump, crunch, then boing (FM), jump (FM), soda and fzzt (FM), which no event but the soda plays"
    );
    assert!(count[13] >= 20, "the route jumps a lot: {}", count[13]);
}

#[test]
fn sound_is_the_same_whatever_the_captions_setting_and_is_never_deduped() {
    let on = replay_sounds(true);
    let off = replay_sounds(false);
    assert_eq!(on, off, "the Captions setting is text only");
    // Fizz 1's 180 ms caption dedupe drops text, not sound: the same event twice in a row plays twice.
    let w = world_with_fire();
    let e = Event::Fire { x: 100, y: 100 };
    let a = event_sound(&w, &e, VIEW_W, VIEW_H, &opts());
    let b = event_sound(&w, &e, VIEW_W, VIEW_H, &opts());
    assert!(a.is_some() && a == b);
    // And the replay has two different sounds in adjacent ticks (a jump, then a snack), both of them played.
    assert!(
        on.windows(2)
            .any(|p| p[0].0 + 1 >= p[1].0 && p[0].1 != p[1].1),
        "the replay has two different sounds within a tick of each other"
    );
}

#[test]
fn a_sound_is_placed_where_its_event_happened_and_a_shot_ending_is_silent() {
    let w = world_with_fire();
    let o = opts();
    let (cam_x, cam_y) = camera_shaken(&w, VIEW_W, VIEW_H, &o);
    let map_h_px = w.map.h * TILE_PX;
    let jump = |x: i32| Event::Jump {
        x,
        y: map_h_px - cam_y - 100,
    };
    let at = |e: Event| event_sound(&w, &e, VIEW_W, VIEW_H, &o).unwrap();
    let (id, l) = at(jump(cam_x));
    assert_eq!(id, EFFECT_JUMP);
    let (_, r) = at(jump(cam_x + VIEW_W as i32));
    assert!(
        l.pan < 0 && r.pan > 0,
        "left of the view pans left, right of it right"
    );
    let (_, f) = at(jump(cam_x + VIEW_W as i32 * 4));
    assert!(f.distance < r.distance, "farther is quieter");
    assert_eq!(
        event_sound(&w, &Event::ShotEnd { x: 0, y: 0 }, VIEW_W, VIEW_H, &o),
        None
    );
    // A cluck comes from its chicken, exactly where `cluck_placement` puts it.
    let cluck = Event::Cluck {
        variant: 2,
        chicken: 0,
    };
    assert_eq!(
        event_sound(&w, &cluck, VIEW_W, VIEW_H, &o),
        Some((2, crate::cluck_placement(&w, 0, VIEW_W, VIEW_H, &o)))
    );
}
