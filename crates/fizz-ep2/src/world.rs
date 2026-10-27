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
use crate::chicken::{CLUCK_VARIANTS, Chicken, SHOT_STUN, STOMP_STUN};
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

/// Something the rules did that a presenter may want to show or play. Events are output only: they are not rules state,
/// and they are not hashed (the cluck timer and stream counter behind a [`Event::Cluck`] are).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// A chicken clucked. `variant` is below [`CLUCK_VARIANTS`](crate::chicken::CLUCK_VARIANTS) and `chicken` is the
    /// chicken's index in [`World::chickens`].
    Cluck { variant: u8, chicken: u8 },
}

/// What a presenter needs to draw a chicken and its cluck: where it is, which way it faces, and how far into its last
/// cluck it is. Read-only; see [`World::chicken_views`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChickenView {
    /// Left edge of the body, in tiles.
    pub x: f64,
    /// Bottom edge of the body (its feet), in tiles.
    pub y: f64,
    /// Walking direction: -1 or 1.
    pub dir: f64,
    /// Ticks since the last cluck; [`CLUCK_NEVER`](crate::chicken::CLUCK_NEVER) before the first. Not hashed.
    pub cluck_age: u32,
    /// The variant of the last cluck. Not hashed.
    pub cluck_variant: u8,
}

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
    /// What the last [`step`](World::step) emitted, in chicken order.
    events: Vec<Event>,
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
            events: Vec::new(),
            spawn,
            key: None,
            chicken_spawns: Vec::new(),
        }
    }

    /// Adds a chicken at Fizz 1's spawn point `(x, y)`, walking in direction `dir` (1 or -1).
    pub fn add_chicken(&mut self, x: f64, y: f64, dir: f64) {
        self.chicken_spawns.push((x, y, dir));
        let id = self.chickens.len() as u32;
        self.chickens.push(Chicken::new(x, y, dir, id, self.tick.0));
    }

    /// The events the last [`step`](World::step) emitted, in the order they happened (chickens in list order). All of
    /// them happened on [`World::tick`]. The next step clears the list, so a presenter reads it once per step; a presenter
    /// that runs several steps between frames must read after each one.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// The chickens as a presenter draws them: position, facing and cluck presentation state, in list order. The cluck
    /// age and variant are not part of the state hash.
    pub fn chicken_views(&self) -> impl Iterator<Item = ChickenView> + '_ {
        self.chickens.iter().map(|c| ChickenView {
            x: c.b.x,
            y: c.b.y,
            dir: c.dir,
            cluck_age: c.cluck_age,
            cluck_variant: c.cluck_variant,
        })
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

    /// The centre of the red key while it is lying in the world, for presenters that draw it. `None` once Ben carries it or
    /// the door has spent it.
    pub fn key_position(&self) -> Option<(f64, f64)> {
        if self.key_in_world() { self.key } else { None }
    }

    /// Advances the world by one tick (the level branch of Fizz 1's `step`).
    pub fn step(&mut self, input: InputFrame) {
        self.events.clear();
        self.tick = self.tick.next();
        self.ben.b.px = self.ben.b.x;
        self.ben.b.py = self.ben.b.y;
        self.tick_ben(input);
        for (i, c) in self.chickens.iter_mut().enumerate() {
            c.cluck_age = c.cluck_age.saturating_add(1);
            if let Some(variant) = c.tick(&self.map, self.tick.0) {
                debug_assert!(variant < CLUCK_VARIANTS);
                c.cluck_age = 0;
                c.cluck_variant = variant;
                self.events.push(Event::Cluck {
                    variant,
                    chicken: i as u8,
                });
            }
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
            .enumerate()
            .map(|(id, &(x, y, dir))| Chicken::new(x, y, dir, id as u32, self.tick.0))
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
///    the ground, bit 3 bumped), then `look_up look_down look_offset` as `f64` bits and `aim` as one byte (`i8` as `u8`);
/// 3. the [`WorldFlags`] word (`u32`), `ammo` (`u32`) and `deaths` (`u32`);
/// 4. `map.switches` (`u32`);
/// 5. the chicken count (`u32`), then per chicken `x y vx vy dir stun` as `f64` bits and a flags byte (bit 0 on the
///    ground, bit 1 touching Ben, bit 2 dead), then the cluck timer and the cluck stream's draw count (both `u32`);
/// 6. the shot count (`u32`), then per shot `x y vx vy life` as `f64` bits.
///
/// The camera and the [`Event`] list are not hashed.
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
    for v in [ben.look_up, ben.look_down, ben.look_offset] {
        h.write_f64(v);
    }
    h.write_u8(ben.aim as u8);
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
        h.write_u32(c.cluck_timer);
        h.write_u32(c.cluck_draws);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiles;
    use fizz_phys::TileMap;

    extern crate std;
    use std::vec::Vec;

    /// FNV-1a 64 over raw bytes, written out again here so the test does not lean on `StateHash`.
    fn fnv(bytes: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    fn f(out: &mut Vec<u8>, v: f64) {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }

    /// A world with every hashed field set to a distinct value.
    fn populated() -> World {
        let mut w = World::new(TileMap::new(4, 4, tiles::props()), (1.0, 2.0));
        w.tick = Tick(7);
        w.ben.b.x = 1.5;
        w.ben.b.y = 2.5;
        w.ben.b.vx = 0.25;
        w.ben.b.vy = -0.5;
        w.ben.face = -1.0;
        w.ben.squash = 0.0625;
        w.ben.dead = 0.125;
        w.ben.inv = 3.0;
        w.ben.shoot_t = 0.375;
        w.ben.pogo = true;
        w.ben.b.on_ground = true;
        w.ben.look_up = 0.5;
        w.ben.look_down = 0.75;
        w.ben.look_offset = 1.25;
        w.ben.aim = -1;
        w.flags = WorldFlags(WorldFlags::KEY_RED | WorldFlags::EXIT_REACHED);
        w.ammo = 3;
        w.deaths = 2;
        w.map.switches = 0x0a;
        w.add_chicken(5.0, 6.0, 1.0);
        w.add_chicken(8.0, 9.0, -1.0);
        w.chickens[0].b.vx = 1.5;
        w.chickens[0].b.vy = -2.0;
        w.chickens[0].stun = 4.0;
        w.chickens[0].touch = true;
        w.chickens[1].b.on_ground = true;
        w.chickens[0].cluck_timer = 123;
        w.chickens[0].cluck_draws = 6;
        w.chickens[1].cluck_timer = 200;
        w.chickens[1].cluck_draws = 4;
        w.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 16.0,
            vy: 0.0,
            life: 0.5,
        });
        w
    }

    #[test]
    fn the_hash_field_order_is_fixed() {
        let w = populated();
        let mut b = Vec::new();
        b.extend_from_slice(&7u64.to_le_bytes());
        for v in [1.5, 2.5, 0.25, -0.5, -1.0, 0.0625, 0.125, 3.0, 0.375] {
            f(&mut b, v);
        }
        b.push(0b0101); // Ben: pogo, on the ground
        for v in [0.5, 0.75, 1.25] {
            f(&mut b, v); // look up, look down, look offset
        }
        b.push(0xff); // aim -1 as a byte
        b.extend_from_slice(&5u32.to_le_bytes()); // flags: key and exit
        b.extend_from_slice(&3u32.to_le_bytes()); // ammo
        b.extend_from_slice(&2u32.to_le_bytes()); // deaths
        b.extend_from_slice(&0x0au32.to_le_bytes()); // switches
        b.extend_from_slice(&2u32.to_le_bytes()); // chicken count
        for (i, ch) in w.chickens.iter().enumerate() {
            for v in [ch.b.x, ch.b.y, ch.b.vx, ch.b.vy, ch.dir, ch.stun] {
                f(&mut b, v);
            }
            b.push([0b010, 0b001][i]); // touching; on the ground
            b.extend_from_slice(&[123u32, 200][i].to_le_bytes()); // cluck timer
            b.extend_from_slice(&[6u32, 4][i].to_le_bytes()); // cluck stream draws
        }
        b.extend_from_slice(&1u32.to_le_bytes()); // shot count
        for v in [10.0, 11.0, 16.0, 0.0, 0.5] {
            f(&mut b, v);
        }
        assert_eq!(hash(&w).finish(), fnv(&b));
    }

    #[test]
    fn the_hash_of_a_known_world_is_locked() {
        // This pins a world with every field set, so a reordering that happens to leave the slice's start unchanged is
        // still caught. `slice-replay.txt` pins the slice itself.
        assert_eq!(hash(&populated()).finish(), 0x4763_4dcf_4449_e880);
    }

    #[test]
    fn every_hashed_field_changes_the_digest() {
        let base = hash(&populated()).finish();
        let edits: [fn(&mut World); 28] = [
            |w| w.tick = Tick(8),
            |w| w.ben.b.x += 1.0,
            |w| w.ben.b.y += 1.0,
            |w| w.ben.b.vx += 1.0,
            |w| w.ben.b.vy += 1.0,
            |w| w.ben.face = 1.0,
            |w| w.ben.squash += 1.0,
            |w| w.ben.dead += 1.0,
            |w| w.ben.inv += 1.0,
            |w| w.ben.shoot_t += 1.0,
            |w| w.ben.cut = true,
            |w| w.ben.look_up += 1.0,
            |w| w.ben.look_down += 1.0,
            |w| w.ben.look_offset += 1.0,
            |w| w.ben.aim = 1,
            |w| w.flags.clear(WorldFlags::KEY_RED),
            |w| w.ammo += 1,
            |w| w.deaths += 1,
            |w| w.map.switches ^= 1,
            |w| w.chickens[0].dir = -1.0,
            |w| w.chickens[0].stun += 1.0,
            |w| w.chickens[1].b.x += 1.0,
            |w| w.chickens[0].cluck_timer += 1,
            |w| w.chickens[0].cluck_draws += 1,
            |w| w.chickens[1].cluck_timer += 1,
            |w| w.chickens[1].cluck_draws += 1,
            |w| w.shots[0].life += 1.0,
            |w| w.shots.clear(),
        ];
        for (i, edit) in edits.iter().enumerate() {
            let mut w = populated();
            edit(&mut w);
            assert_ne!(hash(&w).finish(), base, "edit {i} left the hash alone");
        }
    }

    fn idle() -> InputFrame {
        InputFrame::from_transition(fizz_sim::Buttons::NONE, fizz_sim::Buttons::NONE)
    }

    /// The `(tick, variant)` of every cluck over `ticks` idle steps of the slice level.
    fn clucks(w: &mut World, ticks: u32) -> Vec<(u64, u8)> {
        let mut out = Vec::new();
        for _ in 0..ticks {
            w.step(idle());
            for e in w.events() {
                let Event::Cluck { variant, .. } = *e;
                out.push((w.tick.0, variant));
            }
        }
        out
    }

    /// A slice world whose chicken is the only thing that happens: Ben stays dead-safe by being invulnerable.
    fn quiet() -> World {
        let mut w = crate::level::world();
        w.ben.inv = 1e9;
        w
    }

    #[test]
    fn the_same_world_clucks_the_same_clucks() {
        let a = clucks(&mut quiet(), 3000);
        let b = clucks(&mut quiet(), 3000);
        assert!(a.len() >= 8, "{} clucks", a.len());
        assert_eq!(a, b);
    }

    #[test]
    fn the_wait_between_clucks_stays_within_range_and_every_variant_occurs() {
        let mut w = quiet();
        let first = w.chickens[0].cluck_timer;
        assert!((90..=360).contains(&first));
        let c = clucks(&mut w, 60_000);
        assert!(c.len() > 100);
        assert_eq!(u64::from(first), c[0].0, "the spawn draw is the first wait");
        let mut seen = [false; CLUCK_VARIANTS as usize];
        for p in c.windows(2) {
            assert!((90..=360).contains(&(p[1].0 - p[0].0)), "{p:?}");
        }
        for &(_, v) in &c {
            seen[usize::from(v)] = true;
        }
        assert!(seen.iter().all(|&s| s), "variants seen: {seen:?}");
        // Each cluck takes two draws (the variant, then the next wait) after the spawn draw.
        assert_eq!(w.chickens[0].cluck_draws as usize, 1 + 2 * c.len());
    }

    #[test]
    fn a_stunned_chicken_neither_clucks_nor_counts_down() {
        let mut w = quiet();
        w.chickens[0].stun = 100.0;
        w.chickens[0].cluck_timer = 2;
        let timer = w.chickens[0].cluck_timer;
        assert!(clucks(&mut w, 600).is_empty());
        assert_eq!(w.chickens[0].cluck_timer, timer);
        assert_eq!(w.chickens[0].cluck_draws, 1);
        // Once the stun ends the timer runs on from where it stopped.
        w.chickens[0].stun = 0.0;
        assert_eq!(clucks(&mut w, 2).len(), 1);
    }

    #[test]
    fn a_dead_chicken_does_not_cluck() {
        let mut w = quiet();
        w.chickens[0].dead = true;
        w.chickens[0].cluck_timer = 1;
        assert!(clucks(&mut w, 600).is_empty());
        assert_eq!(w.chickens[0].cluck_timer, 1);
    }

    #[test]
    fn events_last_for_one_step() {
        let mut w = quiet();
        w.chickens[0].cluck_timer = 1;
        w.step(idle());
        assert_eq!(w.events().len(), 1);
        w.step(idle());
        assert!(w.events().is_empty());
    }

    #[test]
    fn cluck_age_and_variant_follow_the_clucks() {
        let mut w = quiet();
        assert_eq!(w.chickens[0].cluck_age, crate::chicken::CLUCK_NEVER);
        let first = u64::from(w.chickens[0].cluck_timer);
        for _ in 1..first {
            w.step(idle());
            assert_eq!(w.chickens[0].cluck_age, crate::chicken::CLUCK_NEVER);
        }
        w.step(idle());
        let Event::Cluck { variant, chicken } = w.events()[0];
        assert_eq!(chicken, 0);
        assert_eq!(w.chickens[0].cluck_age, 0);
        assert_eq!(w.chickens[0].cluck_variant, variant);
        for n in 1..=20 {
            w.step(idle());
            assert_eq!(w.chickens[0].cluck_age, n);
            assert_eq!(w.chickens[0].cluck_variant, variant);
        }
        let v = w.chicken_views().next().unwrap();
        assert_eq!((v.cluck_age, v.cluck_variant), (20, variant));
        assert_eq!(v.dir, w.chickens[0].dir);
    }

    #[test]
    fn cluck_age_saturates() {
        let mut w = quiet();
        w.chickens[0].cluck_age = u32::MAX - 1;
        w.chickens[0].stun = 100.0;
        w.step(idle());
        w.step(idle());
        assert_eq!(w.chickens[0].cluck_age, u32::MAX);
        w.step(idle());
        assert_eq!(w.chickens[0].cluck_age, u32::MAX);
    }

    #[test]
    fn presentation_state_is_not_hashed() {
        let mut a = quiet();
        let mut b = quiet();
        b.chickens[0].cluck_age = 7;
        b.chickens[0].cluck_variant = 3;
        assert_eq!(a.hash(), b.hash());
        // Two worlds that differ only in presentation state hash alike for as long as they run.
        for _ in 0..600 {
            a.step(idle());
            b.step(idle());
            b.chickens[0].cluck_age = 7;
            b.chickens[0].cluck_variant = 3;
            assert_eq!(a.hash(), b.hash());
        }
    }

    #[test]
    fn events_name_the_chicken_that_clucked() {
        let mut w = quiet();
        w.add_chicken(10.0, 9.0, 1.0);
        w.chickens[0].cluck_timer = 500;
        w.chickens[1].cluck_timer = 1;
        w.step(idle());
        let Event::Cluck { chicken, variant } = w.events()[0];
        assert_eq!(w.events().len(), 1);
        assert_eq!(chicken, 1);
        assert_eq!(w.chickens[1].cluck_age, 0);
        assert_eq!(w.chickens[1].cluck_variant, variant);
        assert_eq!(w.chickens[0].cluck_age, crate::chicken::CLUCK_NEVER);
    }

    #[test]
    fn a_respawn_forgets_the_last_cluck() {
        let mut w = quiet();
        w.chickens[0].cluck_age = 3;
        w.chickens[0].cluck_variant = 2;
        w.respawn();
        assert_eq!(w.chickens[0].cluck_age, crate::chicken::CLUCK_NEVER);
        assert_eq!(w.chickens[0].cluck_variant, 0);
    }

    #[test]
    fn chickens_draw_from_their_own_streams() {
        let mut w = World::new(TileMap::new(4, 4, tiles::props()), (1.0, 2.0));
        w.add_chicken(1.0, 1.0, 1.0);
        w.add_chicken(1.0, 1.0, 1.0);
        let timers: Vec<u32> = (0..2).map(|i| w.chickens[i].cluck_timer).collect();
        let more: Vec<u32> = (0..50)
            .map(|_| {
                w.add_chicken(1.0, 1.0, 1.0);
                w.chickens.last().unwrap().cluck_timer
            })
            .collect();
        assert!(
            more.iter().any(|&t| t != timers[0]),
            "ids give different streams"
        );
    }
}
