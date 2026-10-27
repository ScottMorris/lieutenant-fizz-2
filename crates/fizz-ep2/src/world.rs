// The slice's world: Ben, the chickens, the Fizz Blaster shots, the red key and door, death, respawn and the exit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ported from Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: the `Mode::Level` branch of
// `step`, `tick_ben`, `tick_ben_contacts` (hazards, the exit, the red door, the red key and the enemy contact rules),
// `kill_by`, `on_death`, `tick_ents` and `tick_shots`. Left out on purpose: rooms, platforms, the other doors, keys and
// items, switches and mirrors, targets, bosses, captions, sound, the camera and the score. Departures: the red door is a
// switched tile (see `tiles.rs`), its open state and the key live in [`WorldFlags`], and a death respawns Ben in place
// instead of reloading the level, so the open door survives it.

use crate::ben::{Ben, FALL_DEATH_Y};
use crate::chicken::{Chicken, SHOT_STUN, STOMP_STUN};
use crate::shot::Shot;
use crate::tiles::{DOOR_R, DOOR_RED_CHANNEL, EXIT, SPIKE, is_liquid};
use alloc::vec::Vec;
#[cfg_attr(test, allow(unused_imports))]
use fizz_phys::fmath::FloatMath;
use fizz_phys::{STEP, TileMap};
use fizz_sim::{InputFrame, StateHash, Tick};

/// Fizz Blaster shots Ben carries at the start of the slice, and again after a respawn.
pub const START_AMMO: u32 = 5;
/// Seconds Ben lies dead before the world respawns him (Fizz 1's `dead > 1.4` in `tick_ben`).
pub const RESPAWN_AFTER: f64 = 1.4;

/// The progress bits that live outside any one body.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorldFlags(pub u32);

impl WorldFlags {
    /// Ben carries the red key (Fizz 1's `keys_red`). A death drops it.
    pub const KEY_RED: u32 = 1;
    /// The red door has been opened. It survives a death.
    pub const DOOR_RED_OPEN: u32 = 2;
    /// Ben has touched the exit (Fizz 1's `won`).
    pub const EXIT_REACHED: u32 = 4;

    pub const fn contains(self, bit: u32) -> bool {
        self.0 & bit == bit
    }

    pub fn set(&mut self, bit: u32) {
        self.0 |= bit;
    }

    pub fn clear(&mut self, bit: u32) {
        self.0 &= !bit;
    }
}

/// Everything the slice's rules remember between ticks.
#[derive(Clone, Debug)]
pub struct World {
    pub tick: Tick,
    pub map: TileMap,
    pub ben: Ben,
    pub flags: WorldFlags,
    /// Fizz Blaster shots left.
    pub ammo: u32,
    /// How many times Ben has died and respawned.
    pub deaths: u32,
    pub chickens: Vec<Chicken>,
    pub shots: Vec<Shot>,
    /// Where Ben starts and respawns.
    spawn: (f64, f64),
    /// Where the red key lies, as its centre, while it is in the world.
    key: Option<(f64, f64)>,
    /// Where each chicken starts: `(x, y, dir)` as passed to [`Chicken::new`].
    chicken_spawns: Vec<(f64, f64, f64)>,
}

impl World {
    /// A world on `map` with Ben at rest at `spawn`, the lower-left corner of his body. The red door starts closed.
    pub fn new(mut map: TileMap, spawn: (f64, f64)) -> Self {
        map.set_switch(DOOR_RED_CHANNEL, true);
        World {
            tick: Tick::ZERO,
            map,
            ben: Ben::new(spawn.0, spawn.1),
            flags: WorldFlags::default(),
            ammo: START_AMMO,
            deaths: 0,
            chickens: Vec::new(),
            shots: Vec::new(),
            spawn,
            key: None,
            chicken_spawns: Vec::new(),
        }
    }

    /// Adds a chicken at Fizz 1's spawn point `(x, y)`, walking in direction `dir` (1 or -1).
    pub fn add_chicken(&mut self, x: f64, y: f64, dir: f64) {
        self.chicken_spawns.push((x, y, dir));
        self.chickens.push(Chicken::new(x, y, dir));
    }

    /// Lays the red key with its centre at `(x, y)`.
    pub fn set_key(&mut self, x: f64, y: f64) {
        self.key = Some((x, y));
    }

    /// Whether the red key is lying in the world: not carried, and not spent on the door.
    pub fn key_in_world(&self) -> bool {
        self.key.is_some()
            && !self.flags.contains(WorldFlags::KEY_RED)
            && !self.flags.contains(WorldFlags::DOOR_RED_OPEN)
    }

    /// Advances the world by one tick (the level branch of Fizz 1's `step`).
    pub fn step(&mut self, input: InputFrame) {
        self.tick = self.tick.next();
        self.ben.b.px = self.ben.b.x;
        self.ben.b.py = self.ben.b.y;
        self.tick_ben(input);
        for c in &mut self.chickens {
            c.tick(&self.map);
        }
        self.tick_shots();
        if self.ben.dead > RESPAWN_AFTER {
            self.respawn();
        }
    }

    /// Fizz 1's `tick_ben`: the dead branch, the freeze after the exit, then movement and contacts.
    fn tick_ben(&mut self, input: InputFrame) {
        let dead = self.ben.dead > 0.0;
        if !dead && self.flags.contains(WorldFlags::EXIT_REACHED) {
            return;
        }
        self.ben
            .tick(&self.map, &[], input, &mut self.ammo, &mut self.shots);
        if !dead {
            self.tick_ben_contacts();
        }
    }

    /// Kills Ben unless he is already dead.
    fn kill(&mut self) {
        self.ben.kill();
    }

    /// Fizz 1's `tick_ben_contacts`: hazards and the exit, the red door, the red key, then the chickens.
    fn tick_ben_contacts(&mut self) {
        let (px, py, pw, ph) = (self.ben.b.x, self.ben.b.y, self.ben.b.w, self.ben.b.h);
        let mut die = py < FALL_DEATH_Y;
        for cx in (px.floor() as i32)..=((px + pw).floor() as i32) {
            for cy in (py.floor() as i32)..=((py + ph).floor() as i32) {
                let t = self.map.get(cx, cy);
                if t == SPIKE && py < f64::from(cy) + 0.55 {
                    die = true;
                }
                if is_liquid(t) {
                    let lim = if is_liquid(self.map.get(cx, cy + 1)) {
                        1.0
                    } else {
                        0.55
                    };
                    if py < f64::from(cy) + lim {
                        die = true;
                    }
                }
                if t == EXIT && !self.flags.contains(WorldFlags::EXIT_REACHED) {
                    self.flags.set(WorldFlags::EXIT_REACHED);
                    return;
                }
            }
        }
        if self.flags.contains(WorldFlags::KEY_RED) {
            let cx = if self.ben.face > 0.0 {
                (px + pw + 0.1).floor() as i32
            } else {
                (px - 0.1).floor() as i32
            };
            if self.map.get(cx, (py + 0.2).floor() as i32) == DOOR_R
                && self.map.switch(DOOR_RED_CHANNEL)
            {
                self.map.set_switch(DOOR_RED_CHANNEL, false);
                self.flags.clear(WorldFlags::KEY_RED);
                self.flags.set(WorldFlags::DOOR_RED_OPEN);
            }
        }
        if die {
            self.kill();
            return;
        }

        if let Some((kx, ky)) = self.key
            && self.key_in_world()
            && self.ben.b.overlaps(kx - 0.4, ky - 0.4, 0.8, 0.8)
        {
            self.flags.set(WorldFlags::KEY_RED);
        }

        for i in 0..self.chickens.len() {
            let en = self.chickens[i].clone();
            let b = &en.b;
            if en.dead || !self.ben.b.overlaps(b.x, b.y, b.w, b.h) {
                self.chickens[i].touch = false;
                continue;
            }
            self.chickens[i].touch = true;
            let stomp = self.ben.b.vy < 0.0 && self.ben.b.py >= b.y + b.h * 0.55;
            if en.stun > 0.0 {
                if stomp {
                    self.ben.b.vy = if self.ben.pogo { 20.0 } else { 12.0 };
                }
                continue;
            }
            if stomp {
                self.chickens[i].stun = STOMP_STUN;
                self.ben.b.vy = if self.ben.pogo { 20.0 } else { 12.0 };
                continue;
            }
            if self.ben.inv <= 0.0 {
                self.kill();
                return;
            }
        }
    }

    /// Fizz 1's `tick_shots`, for Ben's shots: they fly, die on walls and the map's edge, and stun the first chicken
    /// they enter. A shot passes through a chicken that is already stunned. Shots are visited from the last to the
    /// first and removed with `swap_remove`, so their order in the list, and in the hash, follows Fizz 1.
    fn tick_shots(&mut self) {
        let dt = STEP;
        let mut i = self.shots.len();
        while i > 0 {
            i -= 1;
            let mut b = self.shots[i];
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            b.life -= dt;
            let mut gone = b.life <= 0.0
                || self
                    .map
                    .solid(b.x.floor() as i32, b.y.floor() as i32, false, 0.0)
                || b.y < 0.0;
            if !gone {
                for j in 0..self.chickens.len() {
                    let e = &self.chickens[j];
                    if e.dead {
                        continue;
                    }
                    let (ex, ey, ew, eh) = (e.b.x, e.b.y, e.b.w, e.b.h);
                    let inside = b.x > ex - 0.15
                        && b.x < ex + ew + 0.15
                        && b.y > ey - 0.2
                        && b.y < ey + eh.max(1.1) + 0.2;
                    if inside {
                        if e.stun > 0.0 {
                            continue;
                        }
                        gone = true;
                        self.chickens[j].stun = SHOT_STUN;
                        break;
                    }
                }
            }
            if gone {
                self.shots.swap_remove(i);
            } else {
                self.shots[i] = b;
            }
        }
    }

    /// Puts Ben back at the spawn after a death (Fizz 1's `on_death`, which hands the reload to the shell). The key is
    /// dropped and lies where it began unless the door is already open; the open door stays open; the chickens and the
    /// ammo start over.
    fn respawn(&mut self) {
        self.deaths += 1;
        self.ben = Ben::new(self.spawn.0, self.spawn.1);
        self.flags.clear(WorldFlags::KEY_RED);
        self.ammo = START_AMMO;
        self.shots.clear();
        self.chickens = self
            .chicken_spawns
            .iter()
            .map(|&(x, y, dir)| Chicken::new(x, y, dir))
            .collect();
        self.map.set_switch(
            DOOR_RED_CHANNEL,
            !self.flags.contains(WorldFlags::DOOR_RED_OPEN),
        );
    }

    /// Hashes the world in the order [`hash`] documents.
    pub fn hash(&self) -> StateHash {
        hash(self)
    }
}

/// Hashes the world in a fixed field order, which is part of the Web and DOS parity contract:
///
/// 1. the tick (`u64`);
/// 2. Ben: `x y vx vy face squash dead inv shoot_t` as `f64` bits, then a flags byte (bit 0 pogo, bit 1 cut, bit 2 on
///    the ground, bit 3 bumped);
/// 3. the [`WorldFlags`] word (`u32`), `ammo` (`u32`) and `deaths` (`u32`);
/// 4. `map.switches` (`u32`);
/// 5. the chicken count (`u32`), then per chicken `x y vx vy dir stun` as `f64` bits and a flags byte (bit 0 on the
///    ground, bit 1 touching Ben, bit 2 dead);
/// 6. the shot count (`u32`), then per shot `x y vx vy life` as `f64` bits.
///
/// The camera is not hashed. The chicken's cluck timer and stream counter join the chicken record with the sound
/// package.
pub fn hash(w: &World) -> StateHash {
    let mut h = StateHash::new();
    h.write_u64(w.tick.0);
    let ben = &w.ben;
    for v in [
        ben.b.x,
        ben.b.y,
        ben.b.vx,
        ben.b.vy,
        ben.face,
        ben.squash,
        ben.dead,
        ben.inv,
        ben.shoot_t,
    ] {
        h.write_f64(v);
    }
    h.write_u8(
        u8::from(ben.pogo)
            | u8::from(ben.cut) << 1
            | u8::from(ben.b.on_ground) << 2
            | u8::from(ben.bumped) << 3,
    );
    h.write_u32(w.flags.0);
    h.write_u32(w.ammo);
    h.write_u32(w.deaths);
    h.write_u32(w.map.switches);
    h.write_u32(w.chickens.len() as u32);
    for c in &w.chickens {
        for v in [c.b.x, c.b.y, c.b.vx, c.b.vy, c.dir, c.stun] {
            h.write_f64(v);
        }
        h.write_u8(c.flags());
    }
    h.write_u32(w.shots.len() as u32);
    for s in &w.shots {
        for v in [s.x, s.y, s.vx, s.vy, s.life] {
            h.write_f64(v);
        }
    }
    h
}

/// Advances the world by one tick; the free-function form presenters call, as with `fizz_hello::step`.
pub fn step(w: &mut World, f: InputFrame) {
    w.step(f);
}
