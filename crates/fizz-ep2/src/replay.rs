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
//! `FINAL TICK <n> HASH <digest>`.

use crate::level;
use crate::world::{hash, step};
use fizz_sim::{Buttons, InputFrame};

const IDLE: Buttons = Buttons::NONE;
const U: Buttons = Buttons::UP;
const L: Buttons = Buttons::LEFT;
const R: Buttons = Buttons::RIGHT;
const J: Buttons = Buttons::JUMP;
const F: Buttons = Buttons::FIRE;

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
    // Walk to the exit.
    (44, L),
    (8, L.union(J)),
    (4, IDLE),
    (4, R),
    (4, IDLE),
    // Stand at the exit; the world keeps running.
    (90, IDLE),
];

/// The number of ticks the transcript covers.
pub const SLICE_TICKS: u64 = 1268;

/// Writes transcript v1 of the slice script: a hash at tick 0, every 60th tick, and the final tick.
pub fn replay(out: &mut impl core::fmt::Write) -> core::fmt::Result {
    let mut w = level::world();
    let mut prev = Buttons::NONE;
    writeln!(out, "SLICE REPLAY 1")?;
    writeln!(out, "TICK 0 HASH {:016x}", hash(&w).finish())?;
    for &(frames, now) in SLICE_SCRIPT {
        for _ in 0..frames {
            step(&mut w, InputFrame::from_transition(prev, now));
            prev = now;
            if w.tick.0.is_multiple_of(60) {
                writeln!(out, "TICK {} HASH {:016x}", w.tick.0, hash(&w).finish())?;
            }
        }
    }
    writeln!(
        out,
        "FINAL TICK {} HASH {:016x}",
        w.tick.0,
        hash(&w).finish()
    )
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
        let lines: Vec<&str> = t.lines().collect();
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
