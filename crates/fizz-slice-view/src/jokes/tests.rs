// Tests for the jokes: Fizz 1's thresholds, resets and texts, on real worlds stepped with real input.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use fizz_ep2::tiles::{self, BLOCK};
use fizz_ep2::{Chicken, level, step};
use fizz_phys::TileMap;

extern crate std;
use std::vec::Vec;

const R: Buttons = Buttons::RIGHT;
const L: Buttons = Buttons::LEFT;
const J: Buttons = Buttons::JUMP;

/// A floor two rows thick under a 58-row sky with a solid wall column at each of `walls`, Ben standing at `x`.
fn arena(walls: &[i32], x: f64) -> World {
    let mut map = TileMap::new(24, 60, tiles::props());
    for tx in 0..24 {
        map.set(tx, 0, BLOCK);
        map.set(tx, 1, BLOCK);
    }
    for &wx in walls {
        for ty in 2..60 {
            map.set(wx, ty, BLOCK);
        }
    }
    World::new(map, (x, 2.0))
}

/// Steps with `now` after `prev` and observes, returning the jokes.
fn go(w: &mut World, watch: &mut JokeWatch, prev: &mut Buttons, now: Buttons) -> Jokes {
    let input = InputFrame::from_transition(*prev, now);
    step(w, input);
    *prev = now;
    watch.observe(w, input)
}

fn kinds(j: Jokes) -> Vec<JokeKind> {
    j.iter().map(|j| j.kind).collect()
}

fn count(w: &World, f: fn(&Event) -> bool) -> u32 {
    w.events().iter().filter(|e| f(e)).count() as u32
}

#[test]
fn the_four_texts_are_fizz_1s_words() {
    assert_eq!(
        JokeKind::Idle.text(),
        "Ben whistles a little tune. The level waits politely"
    );
    assert_eq!(
        JokeKind::Pogo.text(),
        "Thirty bounces. Ben is not a kangaroo, but he is doing a very good impression"
    );
    assert_eq!(
        JokeKind::Kick.text(),
        "Ben's knees file a formal complaint about all this wall-kicking"
    );
    assert_eq!(
        JokeKind::Stomp.text(),
        "Ben has been told that marshmallows are not trampolines. He disagrees"
    );
    for (i, k) in JokeKind::ALL.into_iter().enumerate() {
        assert_eq!(k as usize, i);
        assert_eq!(Joke::new(k).text, JOKE_TEXTS[i]);
    }
    assert_eq!(
        (POGO_BOUNCES, STOMPS, KICKS, IDLE_SECONDS),
        (30, 8, 12, 20.0)
    );
}

#[test]
fn standing_still_tells_on_the_tick_the_sum_passes_twenty_and_only_once() {
    let mut w = arena(&[], 10.0);
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    // The sum as Fizz 1 makes it: a tick that begins with Ben on the ground and no button held adds one step.
    let mut sum = 0.0f64;
    let mut told_at = None;
    let mut standing = false;
    for tick in 1..=1400u32 {
        let began_standing = standing;
        let j = go(&mut w, &mut watch, &mut prev, Buttons::NONE);
        standing = w.ben.b.on_ground;
        if began_standing {
            sum += STEP;
        } else {
            sum = 0.0;
        }
        if !j.is_empty() {
            assert_eq!(kinds(j), [JokeKind::Idle]);
            assert!(told_at.is_none(), "told twice");
            told_at = Some(tick);
            assert!(sum > IDLE_SECONDS, "told at {sum}");
        } else if told_at.is_none() {
            assert!(sum <= IDLE_SECONDS, "not told at {sum}");
        }
    }
    // About twenty seconds after the landing.
    let t = told_at.expect("told");
    assert!((1200..1215).contains(&t), "told at tick {t}");
}

#[test]
fn any_button_held_or_leaving_the_ground_puts_the_idle_time_back() {
    let mut w = arena(&[], 10.0);
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    for _ in 0..1100 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    }
    assert!(watch.idle > 18.0);
    // One tick with Up held is a button: the sum goes back to nothing, and the joke needs another twenty seconds.
    go(&mut w, &mut watch, &mut prev, Buttons::UP);
    assert_eq!(watch.idle, 0.0);
    let mut told = 0;
    for _ in 0..1190 {
        told += usize::from(!go(&mut w, &mut watch, &mut prev, Buttons::NONE).is_empty());
    }
    assert_eq!(told, 0, "1190 ticks is under twenty seconds");
    for _ in 0..30 {
        told += usize::from(!go(&mut w, &mut watch, &mut prev, Buttons::NONE).is_empty());
    }
    assert_eq!(told, 1);
    // A jump puts it back too: Ben leaves the ground.
    go(&mut w, &mut watch, &mut prev, J);
    go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    assert_eq!(watch.idle, 0.0);
}

#[test]
fn the_idle_sum_is_held_while_ben_is_dead_and_a_respawn_starts_a_new_visit() {
    let mut w = arena(&[], 10.0);
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    for _ in 0..600 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    }
    w.ben.kill();
    go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    let held = watch.idle;
    assert!(held > 9.0);
    go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    assert_eq!(watch.idle, held, "a step that began dead leaves it alone");
    let mut respawned = false;
    for _ in 0..600 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
        if w.ben.dead == 0.0 {
            respawned = true;
            break;
        }
    }
    assert!(respawned);
    assert_eq!(watch, JokeWatch::new(), "a respawn is a new level visit");
}

/// Ben on the pogo stick in an arena with no chickens.
fn pogo_world() -> (World, JokeWatch, Buttons) {
    let mut w = arena(&[], 10.0);
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    for _ in 0..5 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    }
    assert!(go(&mut w, &mut watch, &mut prev, Buttons::POGO).is_empty());
    assert!(w.ben.pogo);
    (w, watch, prev)
}

/// Steps until `n` more bounces have happened, returning the jokes told on the way.
fn bounce(w: &mut World, watch: &mut JokeWatch, prev: &mut Buttons, n: u32) -> Vec<JokeKind> {
    let (mut seen, mut told) = (0, Vec::new());
    while seen < n {
        let j = go(w, watch, prev, Buttons::NONE);
        seen += count(w, |e| matches!(e, Event::Bounce { .. }));
        told.extend(kinds(j));
    }
    told
}

#[test]
fn thirty_pogo_bounces_in_a_row_tell_on_the_thirtieth_and_only_once() {
    let (mut w, mut watch, mut prev) = pogo_world();
    // The press that turned the stick on may have bounced it off the floor on the same step.
    let first = watch.pogos;
    assert!(first <= 1);
    let told = bounce(&mut w, &mut watch, &mut prev, POGO_BOUNCES - first - 1);
    assert!(told.is_empty(), "29 bounces: {told:?}");
    assert_eq!(watch.pogos, POGO_BOUNCES - 1);
    let told = bounce(&mut w, &mut watch, &mut prev, 1);
    assert_eq!(told, [JokeKind::Pogo]);
    assert!(bounce(&mut w, &mut watch, &mut prev, 40).is_empty(), "once");
}

#[test]
fn switching_the_stick_off_breaks_the_run_and_landing_does_not() {
    let (mut w, mut watch, mut prev) = pogo_world();
    assert!(bounce(&mut w, &mut watch, &mut prev, 20).is_empty());
    assert!(watch.pogos >= 20);
    // Off, then on again: the run starts from nothing, so thirty more are needed.
    go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    assert!(go(&mut w, &mut watch, &mut prev, Buttons::POGO).is_empty());
    assert!(!w.ben.pogo);
    assert_eq!(watch.pogos, 0, "the press that turns the stick off");
    for _ in 0..30 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    }
    assert!(go(&mut w, &mut watch, &mut prev, Buttons::POGO).is_empty());
    assert!(w.ben.pogo);
    assert!(
        bounce(&mut w, &mut watch, &mut prev, 25).is_empty(),
        "25 since the break"
    );
    let told = bounce(&mut w, &mut watch, &mut prev, 6);
    assert_eq!(told, [JokeKind::Pogo]);
}

#[test]
fn a_press_that_switches_the_stick_on_does_not_break_a_run() {
    let (mut w, mut watch, mut prev) = (arena(&[], 10.0), JokeWatch::new(), Buttons::NONE);
    for _ in 0..5 {
        go(&mut w, &mut watch, &mut prev, Buttons::NONE);
    }
    watch.pogos = 7;
    go(&mut w, &mut watch, &mut prev, Buttons::POGO);
    assert!(w.ben.pogo);
    assert!(watch.pogos >= 7, "only an off press resets");
}

#[test]
fn twelve_wall_kicks_tell_on_the_twelfth_and_only_once() {
    // Two walls three tiles apart: kick off one, cross, kick off the other.
    let mut w = arena(&[8, 12], 11.0);
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    let (mut kicks, mut told_at) = (0u32, Vec::new());
    let mut tick = 0u32;
    while kicks < 14 && tick < 3000 {
        let towards = if w.ben.face > 0.0 { R } else { L };
        let held = if tick.is_multiple_of(2) {
            towards | J
        } else {
            towards
        };
        let j = go(&mut w, &mut watch, &mut prev, held);
        kicks += count(&w, |e| matches!(e, Event::Kick { .. }));
        if !j.is_empty() {
            assert_eq!(kinds(j), [JokeKind::Kick]);
            told_at.push(kicks);
        }
        tick += 1;
    }
    assert!(
        kicks >= 13,
        "the chimney gave {kicks} kicks in {tick} ticks"
    );
    assert_eq!(told_at, [KICKS], "told once, on the twelfth");
    assert_eq!(watch.kicks, kicks);
}

/// Ben dropped on a fresh chicken `n` times. Returns the jokes told on each stomp.
fn stomps(n: u32) -> Vec<Vec<JokeKind>> {
    let mut w = arena(&[], 10.0);
    w.ben.inv = 1.0e9;
    w.chickens.push(Chicken::new(10.0, 2.0, 1.0, 0, 0));
    let (mut watch, mut prev) = (JokeWatch::new(), Buttons::NONE);
    let mut told = Vec::new();
    for _ in 0..n {
        w.chickens[0].stun = 0.0;
        w.chickens[0].b.x = 10.0;
        w.chickens[0].b.y = 2.0;
        w.ben.b.x = 10.0;
        w.ben.b.y = 5.0;
        w.ben.b.vy = 0.0;
        w.ben.b.vx = 0.0;
        for _ in 0..120 {
            let j = go(&mut w, &mut watch, &mut prev, Buttons::NONE);
            if count(&w, |e| matches!(e, Event::Stomp { .. })) > 0 {
                told.push(kinds(j));
                break;
            }
        }
    }
    assert_eq!(told.len() as u32, n, "every drop stomped");
    told
}

#[test]
fn eight_stomps_on_chickens_tell_on_the_eighth_and_only_once() {
    let told = stomps(10);
    for (i, t) in told.iter().enumerate() {
        if i == 7 {
            assert_eq!(t, &[JokeKind::Stomp], "the eighth");
        } else {
            assert!(t.is_empty(), "stomp {}: {t:?}", i + 1);
        }
    }
}

#[test]
fn a_restart_starts_the_counters_and_the_telling_again() {
    let (mut w, mut watch, mut prev) = pogo_world();
    bounce(&mut w, &mut watch, &mut prev, POGO_BOUNCES);
    assert_eq!(watch.told, JokeKind::Pogo.bit());
    watch.clear();
    assert_eq!(watch, JokeWatch::new());
}

#[test]
fn the_golden_replay_tells_no_joke() {
    use fizz_ep2::{SLICE_SCRIPT, SLICE_TICKS};
    let (mut w, mut watch, mut prev) = (level::world(), JokeWatch::new(), Buttons::NONE);
    let mut told = Vec::new();
    'run: for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            if w.tick.0 >= SLICE_TICKS {
                break 'run;
            }
            told.extend(kinds(go(&mut w, &mut watch, &mut prev, now)));
        }
    }
    assert_eq!(w.tick.0, SLICE_TICKS);
    assert!(told.is_empty(), "the replay told {told:?}");
    // What the replay counts, so a change to the script that nears a threshold shows here: 0 kicks, a few bounces and stomps, and
    // only the short pauses of the script for the idle sum.
    std::eprintln!(
        "replay counters: pogos {} stomps {} kicks {} idle {}",
        watch.pogos,
        watch.stomps,
        watch.kicks,
        watch.idle
    );
}
