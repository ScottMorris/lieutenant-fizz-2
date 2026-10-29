// The slice's scripted replay: the input every platform feeds the rules, and the transcript they must reproduce.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The replay is the parity contract for the slice. Native, Web and DOS builds feed [`SLICE_SCRIPT`] into a fresh
//! [`level::world`](crate::level::world) one [`InputFrame`] per tick and print the transcript [`replay`] writes; the
//! digests must agree line for line, and with `slice-replay.txt` next to this crate's manifest.
//!
//! The shape mirrors `fizz_hello::replay` so a presenter swaps one for the other: [`replay`] writes transcript v1, whose
//! header is `SLICE REPLAY 1`, then `TICK <n> HASH <digest>` at tick 0 and every 60th tick, then
//! `FINAL TICK <n> HASH <digest>`. Between them sit `EVENT <n> cluck <variant> <chicken>` lines, one per
//! [`Event::Cluck`] the rules emitted on tick `n`, and one line for every other [`Event`], all with `<x> <y>` in sixteenths
//! of a tile as integers (y up): `shotend <x> <y>`, `fire <x> <y>`, `dud <x> <y>`, `stomp <chicken> <x> <y>`,
//! `stun <chicken> <x> <y>`, `death <x> <y>`, `key <colour> <x> <y>` (0 red, 1 blue, 2 green), `dooropen <x> <y>`,
//! `exit <x> <y>`, `bounce <x> <y>`, `headbump <x> <y>`, `jump <x> <y>` and `snack <x> <y>`. `<chicken>` is the chicken's index in the
//! world's list. An `EVENT` line comes before the `TICK` or `FINAL` line of the same tick: the line reads as what
//! happened during tick `n`, and the hash after it as the state that tick left behind. Within a tick, events follow the
//! order of [`World::events`]. A parser that only knows `TICK` and `FINAL` lines still reads the transcript; the new line
//! kind is why the gates compare `EVENT` lines too. The header stays `1` because no existing line changed its shape.
//!
//! A presenter that paces the script itself (the DOS build does, one tick per pacer tick) calls [`write_step`] after each
//! step and [`write_final`] at the end, and gets the same bytes [`replay`] writes.

use crate::level;
use crate::world::{Event, World, hash, step};
use fizz_sim::{Buttons, InputFrame};

const IDLE: Buttons = Buttons::NONE;
const U: Buttons = Buttons::UP;
const L: Buttons = Buttons::LEFT;
const R: Buttons = Buttons::RIGHT;
const J: Buttons = Buttons::JUMP;
const F: Buttons = Buttons::FIRE;
const P: Buttons = Buttons::POGO;

/// The slice script as `(ticks, held buttons)` runs. It sums to [`SLICE_TICKS`].
///
/// A run that starts with a button newly held is that button's press edge. The script plays the level twice over its
/// first half so the replay proves more than a clean run. The first life shoots two shots into the air and a wall,
/// climbs to the long ledge, shoots the chicken, takes the red key past it and carries it onto the spikes: Ben dies, the
/// key drops and the world respawns him. The second life climbs again and stomps the chicken instead, takes the key,
/// opens the red door with it, climbs the right shaft and walks into the exit.
pub const SLICE_SCRIPT: &[(u16, Buttons)] = &[
    // Stand still, fire straight up, then level into the wall, and wait for both shots to end.
    (30, IDLE),
    (4, U.union(F)),
    (26, IDLE),
    (4, F),
    (60, IDLE),
    // Jump to the first ledge.
    (12, J),
    (4, R.union(J)),
    (4, R),
    (4, L),
    // Cross the shaft to the second ledge.
    (20, R.union(J)),
    (8, R),
    (8, R.union(J)),
    (4, R),
    (8, R.union(J)),
    (8, L.union(J)),
    (4, IDLE),
    (4, L),
    // Land on the chicken's ledge.
    (8, J),
    (8, R.union(J)),
    (4, IDLE),
    (4, L),
    // Turn to face the chicken and shoot it.
    (44, L),
    (4, L.union(F)),
    // Walk through the stunned chicken to the key.
    (12, L),
    // Carry the key out to the spikes and die on them.
    (12, L),
    (84, R),
    (4, L),
    // Wait for the respawn.
    (82, IDLE),
    // Jump to the first ledge.
    (4, IDLE),
    (16, J),
    (8, L),
    // Cross the shaft to the second ledge.
    (20, R.union(J)),
    (8, R),
    (12, R.union(J)),
    (4, R),
    (16, R.union(J)),
    (12, R),
    // Land on the chicken's ledge.
    (8, J),
    (4, R.union(J)),
    (4, J),
    (4, L),
    (4, L.union(J)),
    // Stomp the chicken instead.
    (4, L),
    (4, L.union(J)),
    (24, L),
    (4, L.union(J)),
    (16, L),
    (4, L.union(J)),
    // Fetch the key again.
    (20, L),
    // Back to the right end of the ledge.
    (8, R.union(J)),
    (60, R),
    (4, IDLE),
    // Third ledge.
    (8, J),
    (4, L.union(J)),
    (4, J),
    (4, L),
    (4, L.union(J)),
    // Fourth ledge.
    (4, L),
    (8, L.union(J)),
    (4, R.union(J)),
    (4, J),
    (4, R),
    (4, L),
    // Last ledge before the door.
    (20, R.union(J)),
    (12, R),
    (4, IDLE),
    // Walk into the red door with the key.
    (52, R),
    // Step through the open door.
    (20, R),
    // Climb the right shaft.
    (20, R.union(J)),
    (20, R),
    (16, R.union(J)),
    (4, R),
    (4, IDLE),
    (20, L.union(J)),
    (8, L),
    (4, IDLE),
    (16, R.union(J)),
    (4, R),
    (4, IDLE),
    (20, L.union(J)),
    (12, L),
    (4, R),
    (16, R.union(J)),
    (4, R),
    (4, IDLE),
    // Up to the gallery.
    (8, J),
    (4, R.union(J)),
    (4, J),
    (4, R),
    (4, IDLE),
    // The snack nook. Spend the five shots on the wall to the right, then press Fire once more: the empty blaster clicks.
    (4, IDLE),
    (2, F),
    (8, IDLE),
    (2, F),
    (8, IDLE),
    (2, F),
    (8, IDLE),
    (2, F),
    (8, IDLE),
    (2, F),
    (8, IDLE),
    (2, F),
    (8, IDLE),
    // Jump under the shelf: the cheezie hanging there crunches and the head bumps the shelf. Then the same at the other one.
    (8, R),
    (4, IDLE),
    (20, J),
    (16, IDLE),
    (28, L),
    (4, IDLE),
    (20, J),
    (16, IDLE),
    // Walk to the right-hand wall, take the pogo stick and bounce, low, low, then high.
    (52, R),
    (4, IDLE),
    (2, P),
    (50, IDLE),
    (6, J),
    (9, IDLE),
    // Drift left at the top of the high bounce and land on the shelf, then bounce along it to the left end, taking the snacks
    // and the blue key.
    (14, L),
    (60, IDLE),
    // Drop off the shelf facing the blue door with the key: it opens, and the exit is behind it.
    (60, L),
    (40, IDLE),
    // Stand at the exit; the world keeps running.
    (90, IDLE),
];

/// The number of ticks the transcript covers.
pub const SLICE_TICKS: u64 = 1681;

/// Writes the header and the tick 0 line.
pub fn write_start(out: &mut impl core::fmt::Write, w: &World) -> core::fmt::Result {
    writeln!(out, "SLICE REPLAY 1")?;
    writeln!(out, "TICK 0 HASH {:016x}", hash(w).finish())
}

/// Writes what the step that just ran contributes: an `EVENT` line for each event, then a `TICK` line when the tick is a
/// multiple of 60.
pub fn write_step(out: &mut impl core::fmt::Write, w: &World) -> core::fmt::Result {
    for e in w.events() {
        match *e {
            Event::Cluck { variant, chicken } => {
                writeln!(out, "EVENT {} cluck {variant} {chicken}", w.tick.0)?
            }
            Event::ShotEnd { x, y } => writeln!(out, "EVENT {} shotend {x} {y}", w.tick.0)?,
            Event::Fire { x, y } => writeln!(out, "EVENT {} fire {x} {y}", w.tick.0)?,
            Event::Dud { x, y } => writeln!(out, "EVENT {} dud {x} {y}", w.tick.0)?,
            Event::Stomp { chicken, x, y } => {
                writeln!(out, "EVENT {} stomp {chicken} {x} {y}", w.tick.0)?
            }
            Event::Stun { chicken, x, y } => {
                writeln!(out, "EVENT {} stun {chicken} {x} {y}", w.tick.0)?
            }
            Event::Death { x, y } => writeln!(out, "EVENT {} death {x} {y}", w.tick.0)?,
            Event::Key { colour, x, y } => {
                writeln!(out, "EVENT {} key {colour} {x} {y}", w.tick.0)?
            }
            Event::DoorOpen { x, y } => writeln!(out, "EVENT {} dooropen {x} {y}", w.tick.0)?,
            Event::Exit { x, y } => writeln!(out, "EVENT {} exit {x} {y}", w.tick.0)?,
            Event::Bounce { x, y } => writeln!(out, "EVENT {} bounce {x} {y}", w.tick.0)?,
            Event::HeadBump { x, y } => writeln!(out, "EVENT {} headbump {x} {y}", w.tick.0)?,
            Event::Jump { x, y } => writeln!(out, "EVENT {} jump {x} {y}", w.tick.0)?,
            Event::Snack { x, y } => writeln!(out, "EVENT {} snack {x} {y}", w.tick.0)?,
        }
    }
    if w.tick.0.is_multiple_of(60) {
        writeln!(out, "TICK {} HASH {:016x}", w.tick.0, hash(w).finish())?;
    }
    Ok(())
}

/// Writes the `FINAL` line.
pub fn write_final(out: &mut impl core::fmt::Write, w: &World) -> core::fmt::Result {
    writeln!(
        out,
        "FINAL TICK {} HASH {:016x}",
        w.tick.0,
        hash(w).finish()
    )
}

/// Writes transcript v1 of the slice script: a hash at tick 0, every 60th tick, and the final tick, with an `EVENT` line
/// for every event in between.
pub fn replay(out: &mut impl core::fmt::Write) -> core::fmt::Result {
    let mut w = level::world();
    let mut prev = Buttons::NONE;
    write_start(out, &w)?;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            write_step(out, &w)?;
        }
    }
    write_final(out, &w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{World, WorldFlags};

    extern crate std;
    use std::string::String;
    use std::vec::Vec;

    fn transcript() -> String {
        let mut s = String::new();
        replay(&mut s).unwrap();
        s
    }

    #[test]
    fn the_script_sums_to_the_declared_ticks() {
        let total: u64 = SLICE_SCRIPT.iter().map(|&(n, _)| u64::from(n)).sum();
        assert_eq!(total, SLICE_TICKS);
    }

    #[test]
    fn the_transcript_has_the_v1_shape() {
        let t = transcript();
        assert!(t.ends_with('\n'));
        let all: Vec<&str> = t.lines().collect();
        let lines: Vec<&str> = all
            .iter()
            .copied()
            .filter(|l| !l.starts_with("EVENT "))
            .collect();
        let last_tick = SLICE_TICKS / 60 * 60;
        // Header, tick 0, every 60th tick, the final line.
        assert_eq!(lines.len() as u64, 3 + last_tick / 60);
        assert_eq!(lines[0], "SLICE REPLAY 1");
        assert!(lines[1].starts_with("TICK 0 HASH "));
        let last = lines[lines.len() - 2];
        assert!(last.starts_with(&std::format!("TICK {last_tick} HASH ")));
        let final_line = lines[lines.len() - 1];
        assert!(final_line.starts_with(&std::format!("FINAL TICK {SLICE_TICKS} HASH ")));
        assert_eq!(final_line.rsplit(' ').next().unwrap().len(), 16);
    }

    /// The `(tick, variant)` of every `EVENT` line; the chicken index must be a valid index.
    fn events() -> Vec<(u64, u8)> {
        transcript()
            .lines()
            .filter_map(|l| l.strip_prefix("EVENT "))
            .filter(|l| l.split(' ').nth(1) == Some("cluck"))
            .map(|l| {
                let f: Vec<&str> = l.split(' ').collect();
                assert_eq!(f[1], "cluck");
                assert_eq!(f.len(), 4);
                assert!(f[3].parse::<usize>().unwrap() < level::world().chickens.len());
                (f[0].parse().unwrap(), f[2].parse().unwrap())
            })
            .collect()
    }

    #[test]
    fn the_transcript_has_a_handful_of_clucks_in_range() {
        let e = events();
        assert!(
            (3..=40).contains(&e.len()),
            "{} clucks in {SLICE_TICKS} ticks",
            e.len()
        );
        assert!(e.windows(2).all(|p| p[0].0 <= p[1].0), "ticks ascend");
        assert!(
            e.iter()
                .all(|&(t, v)| (1..=SLICE_TICKS).contains(&t) && v < crate::CLUCK_VARIANTS)
        );
    }

    #[test]
    fn the_script_ends_some_shots() {
        let ends = transcript()
            .lines()
            .filter(|l| l.contains(" shotend "))
            .count();
        assert!((2..=20).contains(&ends), "{ends} shot ends");
    }

    #[test]
    fn the_transcript_lines_of_a_shot_end_are_integers() {
        for l in transcript().lines().filter(|l| l.contains(" shotend ")) {
            let f: Vec<&str> = l.split(' ').collect();
            assert_eq!(f.len(), 5, "{l}");
            assert_eq!(f[0], "EVENT");
            assert!(
                f[1].parse::<u64>().is_ok()
                    && f[3].parse::<i32>().is_ok()
                    && f[4].parse::<i32>().is_ok()
            );
        }
    }

    #[test]
    fn an_event_line_precedes_the_tick_line_of_its_tick() {
        let mut w = level::world();
        w.tick = fizz_sim::Tick(59);
        w.chickens[0].cluck_timer = 1;
        step(&mut w, InputFrame::from_transition(IDLE, IDLE));
        assert_eq!(w.events().len(), 1);
        let mut s = String::new();
        write_step(&mut s, &w).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("EVENT 60 cluck "));
        assert!(lines[0].ends_with(" 0"), "{}", lines[0]);
        assert!(lines[1].starts_with("TICK 60 HASH "));
    }

    #[test]
    fn the_final_hash_is_the_playground_golden() {
        // The final hash moved once, when the playground's snacks, blue key and blue door joined the hashed state and the
        // level (the hash order is in `world::hash`). Presentation state and events must never move it again.
        let t = transcript();
        assert!(
            t.ends_with("FINAL TICK 1681 HASH e3d629fda94149ab\n"),
            "{t}"
        );
    }

    #[test]
    fn every_tick_and_final_line_is_pinned_apart_from_the_events() {
        // The non-EVENT lines (header, TICK and FINAL) are the state hashes; adding or changing events can never be the
        // cause of one moving. The digest is FNV-1a 64 over those lines.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut lines = 0;
        for l in transcript().lines().filter(|l| !l.starts_with("EVENT ")) {
            for &b in l.as_bytes().iter().chain(b"\n") {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            lines += 1;
        }
        assert_eq!(lines, 31);
        assert_eq!(h, 0xcb57_844e_4cd2_6b11);
    }

    /// The kind of every `EVENT` line, in order.
    fn kinds() -> Vec<String> {
        transcript()
            .lines()
            .filter_map(|l| l.strip_prefix("EVENT "))
            .map(|l| String::from(l.split(' ').nth(1).unwrap()))
            .collect()
    }

    #[test]
    fn the_playground_fires_every_event_the_slice_can_show() {
        let k = kinds();
        let count = |name: &str| k.iter().filter(|x| x.as_str() == name).count();
        for name in [
            "fire", "dud", "stomp", "stun", "death", "key", "dooropen", "exit", "bounce",
            "headbump", "jump", "snack", "shotend", "cluck",
        ] {
            assert!(count(name) >= 1, "no {name} event in the replay");
        }
        assert_eq!(count("key"), 3, "red twice (a death between), blue once");
        assert_eq!(count("dooropen"), 2);
        assert_eq!(count("dud"), 1);
        assert_eq!(count("snack"), 5);
        assert!(count("bounce") >= 5);
        assert!(count("headbump") >= 3);
    }

    #[test]
    fn the_playground_pays_fizz_1s_points_and_opens_the_blue_door() {
        let mut w = level::world();
        let mut prev = Buttons::NONE;
        for &(frames, now) in SLICE_SCRIPT {
            for _ in 0..frames {
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
            }
        }
        // Three cheezies (1 each), a chocolate (2) and a cookie (5).
        assert_eq!(w.score(), 10);
        assert!(w.snacks.iter().all(|s| s.taken));
        assert!(w.flags.contains(WorldFlags::DOOR_BLUE_OPEN));
        assert!(!w.flags.contains(WorldFlags::KEY_BLUE), "the door spent it");
        assert!(!w.map.switch(crate::tiles::DOOR_BLUE_CHANNEL));
        assert!(!w.key_blue_in_world());
        assert_eq!(w.ammo, 0, "the shots were spent on the wall");
    }

    #[test]
    fn the_blue_door_stays_shut_without_the_blue_key() {
        let mut w = level::world();
        // Stand at the door's right side with only the red key and walk into it.
        w.flags.set(WorldFlags::KEY_RED);
        w.ben.b.x = 23.2;
        w.ben.b.y = 38.0;
        w.ben.face = -1.0;
        let mut prev = Buttons::NONE;
        for _ in 0..90 {
            step(&mut w, InputFrame::from_transition(prev, L));
            prev = L;
        }
        assert!(w.ben.b.x >= 23.0);
        assert!(!w.flags.contains(WorldFlags::DOOR_BLUE_OPEN));
        assert!(w.map.switch(crate::tiles::DOOR_BLUE_CHANNEL));
    }

    #[test]
    fn the_nook_is_sealed_so_the_exit_needs_the_blue_door() {
        // The lintel above the door and the wall right of the nook run to the top of the map.
        let m = level::map();
        for y in 41..48 {
            assert!(m.solid(22, y, false, 0.0), "lintel block at y={y}");
        }
        for y in 38..48 {
            assert!(m.solid(31, y, false, 0.0), "right wall at y={y}");
        }
    }

    #[test]
    fn a_jump_into_the_shelf_bumps_the_head_once() {
        let mut w = level::world();
        w.ben.b.x = 26.5;
        w.ben.b.y = 38.0;
        w.ben.b.on_ground = true;
        let mut prev = Buttons::NONE;
        let mut bumps = 0;
        for i in 0..40 {
            let now = if i < 24 { J } else { IDLE };
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            bumps += w
                .events()
                .iter()
                .filter(|e| matches!(e, Event::HeadBump { .. }))
                .count();
        }
        assert_eq!(bumps, 1);
    }

    #[test]
    fn a_death_puts_the_snacks_back_and_keeps_the_open_blue_door() {
        let mut w = level::world();
        w.snacks[0].taken = true;
        w.flags.set(WorldFlags::DOOR_BLUE_OPEN);
        w.map.set_switch(crate::tiles::DOOR_BLUE_CHANNEL, false);
        w.ben.kill();
        for _ in 0..100 {
            step(&mut w, InputFrame::from_transition(IDLE, IDLE));
        }
        assert_eq!(w.deaths, 1);
        assert!(w.snacks.iter().all(|s| !s.taken));
        assert!(!w.map.switch(crate::tiles::DOOR_BLUE_CHANNEL));
        assert!(w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
    }

    #[test]
    fn the_transcript_matches_the_golden_file() {
        assert_eq!(transcript(), include_str!("../slice-replay.txt"));
    }

    /// Plays the script and notes what happened on the way, so the golden hashes are known to cover each rule.
    #[test]
    fn the_script_exercises_every_rule_of_the_slice() {
        let mut w = level::world();
        let mut prev = Buttons::NONE;
        let (mut max_shots, mut shot_stun, mut stomp_stun) = (0, false, false);
        let mut key_when_died = false;
        let mut door_open_before_death = false;
        let mut deaths = 0;
        for &(frames, now) in SLICE_SCRIPT {
            for _ in 0..frames {
                let had_key = w.flags.contains(WorldFlags::KEY_RED);
                let was_open = w.flags.contains(WorldFlags::DOOR_RED_OPEN);
                step(&mut w, InputFrame::from_transition(prev, now));
                prev = now;
                max_shots = max_shots.max(w.shots.len());
                let stun = w.chickens[0].stun;
                shot_stun |= stun > 5.9;
                stomp_stun |= stun > 2.9 && stun < 3.0 && w.deaths == 1;
                if w.deaths > deaths {
                    deaths = w.deaths;
                    key_when_died = had_key;
                    door_open_before_death = was_open;
                    assert!(!w.flags.contains(WorldFlags::KEY_RED));
                    assert!(w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
                    assert_eq!(w.ammo, crate::world::START_AMMO);
                }
            }
        }
        assert!(max_shots >= 1, "a shot was in flight");
        assert!(shot_stun, "a shot stunned the chicken");
        assert!(stomp_stun, "a stomp stunned the chicken");
        assert_eq!(deaths, 1);
        assert!(key_when_died, "Ben died carrying the key");
        assert!(!door_open_before_death);
        assert!(w.flags.contains(WorldFlags::DOOR_RED_OPEN));
        assert!(
            !w.flags.contains(WorldFlags::KEY_RED),
            "the door spent the key"
        );
        assert!(w.flags.contains(WorldFlags::EXIT_REACHED));
        assert!(!w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
        assert!(w.ben.dead == 0.0);
    }

    #[test]
    fn the_door_stays_shut_without_the_key() {
        let mut w: World = level::world();
        let (dx, dy) = (level::DOOR[0].0 as f64 - 1.0, level::DOOR[0].1 as f64);
        w.ben.b.x = dx - 0.2;
        w.ben.b.y = dy;
        let mut prev = Buttons::NONE;
        for _ in 0..120 {
            step(&mut w, InputFrame::from_transition(prev, R));
            prev = R;
        }
        assert!(w.ben.b.x < level::DOOR[0].0 as f64);
        assert!(!w.flags.contains(WorldFlags::DOOR_RED_OPEN));
        assert!(w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
    }

    #[test]
    fn an_open_door_survives_a_death_and_the_key_does_not() {
        let mut w = level::world();
        w.flags.set(WorldFlags::DOOR_RED_OPEN);
        w.flags.set(WorldFlags::KEY_RED);
        w.map.set_switch(crate::tiles::DOOR_RED_CHANNEL, false);
        w.ben.kill();
        for _ in 0..100 {
            step(&mut w, InputFrame::from_transition(IDLE, IDLE));
        }
        assert_eq!(w.deaths, 1);
        assert!(w.flags.contains(WorldFlags::DOOR_RED_OPEN));
        assert!(!w.flags.contains(WorldFlags::KEY_RED));
        assert!(!w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
        assert!(!w.key_in_world(), "a spent key does not come back");
    }

    #[test]
    fn a_lost_key_comes_back_with_the_respawn() {
        let mut w = level::world();
        w.flags.set(WorldFlags::KEY_RED);
        assert!(!w.key_in_world());
        w.ben.kill();
        for _ in 0..100 {
            step(&mut w, InputFrame::from_transition(IDLE, IDLE));
        }
        assert!(w.key_in_world());
        assert!(w.map.switch(crate::tiles::DOOR_RED_CHANNEL));
    }
}
