// The slice's world: Ben, the chickens, the Fizz Blaster shots, the snacks and soda cans, the red and blue keys and doors, death, respawn and the exit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: ported from Fizz 1 `episodes/episode-1/game/src/world.rs` at commit `4bce036`: the `Mode::Level` branch of
// `step`, `tick_ben`, `tick_ben_contacts` (hazards, the exit, the doors, the keys, the snacks and the soda cans, and the enemy contact rules),
// `kill_by`, `on_death`, `tick_ents` and `tick_shots`. Left out on purpose: rooms, platforms, the green door and key,
// the USB stick, switches and mirrors, targets, bosses, captions, sound, the camera, extra lives and the HUD's score.
// Departures: the red and blue doors are switched tiles (see `tiles.rs`) and the key that opens each is a lock rule of the level
// (see `rules.rs`), the keys and the spent-key marks live in [`WorldFlags`], and a death respawns Ben in place instead of
// reloading the level. The respawn is nevertheless a reload in effect: it restores the level's start switches and puts the keys
// and the snacks back, as Fizz 1's reload does.

use crate::ben::{Ben, FALL_DEATH_Y};
use crate::chicken::{CLUCK_VARIANTS, Chicken, SHOT_STUN, STOMP_STUN};
use crate::rules::{KEY_BLUE as KEY_BLUE_COLOUR, KEY_RED as KEY_RED_COLOUR, RuleTable};
use crate::shot::Shot;
use crate::tiles::{EXIT, SPIKE, is_liquid};
use alloc::vec::Vec;
#[cfg_attr(test, allow(unused_imports))]
use fizz_phys::fmath::FloatMath;
use fizz_phys::tilemap::SWITCHED;
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
    /// One of Ben's shots ended: it hit a wall or the map's top, ran out of life, or stunned a chicken (Fizz 1's `gone` in
    /// `tick_shots`, where it pushes the puff). `x` and `y` are where the shot was, in sixteenths of a tile (the slice's
    /// logical pixels, y up from the map's bottom), rounded down. A respawn that clears the shots emits nothing. The slice
    /// has no enemy shots, so the event carries no owner.
    ShotEnd { x: i32, y: i32 },
    /// A shot left the Fizz Blaster (Fizz 1's "fzzt"). `x` and `y` are where Fizz 1 puts the caption: the shot's start, half
    /// a tile up. Positions of every event below are in sixteenths of a tile, y up, rounded down, like `ShotEnd`.
    Fire { x: i32, y: i32 },
    /// Fire was pressed with no ammo (Fizz 1's "click, no fizz").
    Dud { x: i32, y: i32 },
    /// Ben stomped an unstunned chicken (Fizz 1's "bonk"). `chicken` indexes [`World::chickens`]. Raised above the chicken.
    Stomp { chicken: u8, x: i32, y: i32 },
    /// A shot stunned an unstunned chicken (Fizz 1's "fizzled"). The shot also ends, so a [`Event::ShotEnd`] follows it.
    Stun { chicken: u8, x: i32, y: i32 },
    /// Ben died (Fizz 1's "whoa!"), at the point Fizz 1 raises it, 1.6 tiles above his feet.
    Death { x: i32, y: i32 },
    /// Ben picked up a key (Fizz 1's gumdrops). `colour` is Fizz 1's key order: 0 red, 1 blue, 2 green; the slice has the
    /// red and the blue one.
    Key { colour: u8, x: i32, y: i32 },
    /// A door opened with the matching key (Fizz 1's "clunk"), above the door's tile.
    DoorOpen { x: i32, y: i32 },
    /// Ben touched the exit (Fizz 1's "ta-da!"). Raised once; the world freezes Ben afterwards.
    Exit { x: i32, y: i32 },
    /// The pogo bounced off the ground (Fizz 1's "boing").
    Bounce { x: i32, y: i32 },
    /// Ben's head hit a ceiling on a rise that counts, once per rise (Fizz 1's "clunk").
    HeadBump { x: i32, y: i32 },
    /// A ground jump started (Fizz 1's jump sound, which has no caption).
    Jump { x: i32, y: i32 },
    /// Ben took a snack, a cheezie, a chocolate or a cookie (Fizz 1's "crunch"), half a tile above the item's centre.
    Snack { x: i32, y: i32 },
    /// Ben took a soda can, a refill for the Fizz Blaster (Fizz 1's "fsssht"), half a tile above the can's centre.
    Can { x: i32, y: i32 },
}

/// The items Fizz 1 scatters through its levels that Ben takes by touch, other than the keys: the snacks, with the points each
/// is worth in its `tick_ben_contacts`, and the soda can, which is worth none and refills the Fizz Blaster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnackKind {
    /// A cheezie: 1 point.
    Cheezie,
    /// A chocolate: 2 points.
    Choc,
    /// A cookie: 5 points.
    Cookie,
    /// A soda can (Fizz 1's `ItemKind::Soda`): no points, [`SODA_AMMO`] more shots.
    Soda,
}

/// The shots one soda can adds (Fizz 1: `self.game.ammo += 5`). Fizz 1 puts no cap on the ammo, and neither does this.
pub const SODA_AMMO: u32 = 5;

impl SnackKind {
    /// The number the asset pack schema uses for this kind: 0 cheezie, 1 chocolate, 2 cookie, 3 soda can (all four are listed in `tiles::pack_content()`).
    pub const fn number(self) -> u8 {
        match self {
            SnackKind::Cheezie => 0,
            SnackKind::Choc => 1,
            SnackKind::Cookie => 2,
            SnackKind::Soda => 3,
        }
    }

    /// The kind with this pack number, or `None` for a number no kind has.
    pub const fn from_number(n: u8) -> Option<SnackKind> {
        match n {
            0 => Some(SnackKind::Cheezie),
            1 => Some(SnackKind::Choc),
            2 => Some(SnackKind::Cookie),
            3 => Some(SnackKind::Soda),
            _ => None,
        }
    }

    /// The points Fizz 1 adds to the score for this snack.
    pub const fn points(self) -> u32 {
        match self {
            SnackKind::Cheezie => 1,
            SnackKind::Choc => 2,
            SnackKind::Cookie => 5,
            SnackKind::Soda => 0,
        }
    }
}

/// A snack lying in the level. The position is its centre in tiles; `taken` is the only part that changes, and it is hashed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snack {
    pub kind: SnackKind,
    pub x: f64,
    pub y: f64,
    pub taken: bool,
}

/// A point in tiles as an event position: sixteenths of a tile, rounded down.
fn sixteenths((x, y): (f64, f64)) -> (i32, i32) {
    ((x * 16.0).floor() as i32, (y * 16.0).floor() as i32)
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
    /// The red key has been spent on its lock. A death clears it, and the key lies in the world again.
    pub const KEY_RED_USED: u32 = 2;
    /// Ben has touched the exit (Fizz 1's `won`).
    pub const EXIT_REACHED: u32 = 4;
    /// Ben carries the blue key. A death drops it.
    pub const KEY_BLUE: u32 = 8;
    /// The blue key has been spent on its lock. A death clears it, and the key lies in the world again.
    pub const KEY_BLUE_USED: u32 = 16;

    /// The flag for Ben carrying the key of `colour` and the one for that key being spent, or `None` for a colour with neither.
    pub const fn key_bits(colour: u8) -> Option<(u32, u32)> {
        match colour {
            KEY_RED_COLOUR => Some((WorldFlags::KEY_RED, WorldFlags::KEY_RED_USED)),
            KEY_BLUE_COLOUR => Some((WorldFlags::KEY_BLUE, WorldFlags::KEY_BLUE_USED)),
            _ => None,
        }
    }

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
    /// The snacks lying in the level, in the order they were added. A death puts them all back (Fizz 1 reloads the level).
    pub snacks: Vec<Snack>,
    /// What the last [`step`](World::step) emitted: Ben's own events and his contacts first, then clucks in chicken order,
    /// then the shots from the last to the first (a stun, then its shot end).
    events: Vec<Event>,
    /// Where Ben starts and respawns.
    spawn: (f64, f64),
    /// The switch channels that are on when the level starts, and again after every death.
    start_switches: u32,
    /// The level's rules; only locks so far.
    rules: RuleTable,
    /// Where the red key lies, as its centre, while it is in the world.
    key: Option<(f64, f64)>,
    /// Where the blue key lies, as its centre, while it is in the world. A world with a blue key has a blue door.
    key_blue: Option<(f64, f64)>,
    /// Where each chicken starts: `(x, y, dir)` as passed to [`Chicken::new`].
    chicken_spawns: Vec<(f64, f64, f64)>,
}

impl World {
    /// A world on `map` with Ben at rest at `spawn`, the lower-left corner of his body, and no rules. The map's switch word is
    /// the level's start state (a death restores it); [`World::set_switches`] changes it.
    pub fn new(map: TileMap, spawn: (f64, f64)) -> Self {
        World {
            start_switches: map.switches,
            rules: RuleTable::new(),
            tick: Tick::ZERO,
            map,
            ben: Ben::new(spawn.0, spawn.1),
            flags: WorldFlags::default(),
            ammo: START_AMMO,
            deaths: 0,
            chickens: Vec::new(),
            shots: Vec::new(),
            snacks: Vec::new(),
            events: Vec::new(),
            spawn,
            key: None,
            key_blue: None,
            chicken_spawns: Vec::new(),
        }
    }

    /// Where Ben starts and respawns, the lower-left corner of his body.
    pub fn spawn(&self) -> (f64, f64) {
        self.spawn
    }

    /// Sets the start state of the switch channels, now and after every death: a set bit is a channel that starts on (for a
    /// door, closed).
    pub fn set_switches(&mut self, bits: u32) {
        self.start_switches = bits;
        self.map.switches = bits;
    }

    /// The switch channels that were on when the level started.
    pub fn start_switches(&self) -> u32 {
        self.start_switches
    }

    /// Gives the world its rules, replacing any it had.
    pub fn set_rules(&mut self, rules: RuleTable) {
        self.rules = rules;
    }

    /// The level's rules.
    pub fn rules(&self) -> &RuleTable {
        &self.rules
    }

    /// Whether the door that the lock for key `colour` opens is open now: the lock's channel is off. `false` for a level with no
    /// such lock.
    pub fn lock_open(&self, colour: u8) -> bool {
        self.rules
            .lock_channel(colour)
            .is_some_and(|c| !self.map.switch(c))
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
            && !self.flags.contains(WorldFlags::KEY_RED_USED)
    }

    /// The centre of the red key while it is lying in the world, for presenters that draw it. `None` once Ben carries it or
    /// the door has spent it.
    pub fn key_position(&self) -> Option<(f64, f64)> {
        if self.key_in_world() { self.key } else { None }
    }

    /// Lays the blue key with its centre at `(x, y)`.
    pub fn set_key_blue(&mut self, x: f64, y: f64) {
        self.key_blue = Some((x, y));
    }

    /// Whether the blue key is lying in the world: not carried, and not spent on the door.
    pub fn key_blue_in_world(&self) -> bool {
        self.key_blue.is_some()
            && !self.flags.contains(WorldFlags::KEY_BLUE)
            && !self.flags.contains(WorldFlags::KEY_BLUE_USED)
    }

    /// The centre of the blue key while it is lying in the world, as [`World::key_position`] is for the red one.
    pub fn key_blue_position(&self) -> Option<(f64, f64)> {
        if self.key_blue_in_world() {
            self.key_blue
        } else {
            None
        }
    }

    /// Whether the level has a red key at all, lying, carried or spent on its door. Level data, so it never changes after the
    /// level is built; a presenter's key tray has one socket for each kind the level has.
    pub fn level_has_red_key(&self) -> bool {
        self.key.is_some()
    }

    /// Whether the level has a blue key at all, as [`World::level_has_red_key`] is for the red one.
    pub fn level_has_blue_key(&self) -> bool {
        self.key_blue.is_some()
    }

    /// Lays a snack with its centre at `(x, y)`.
    pub fn add_snack(&mut self, kind: SnackKind, x: f64, y: f64) {
        self.snacks.push(Snack {
            kind,
            x,
            y,
            taken: false,
        });
    }

    /// Fizz 1's score for the snacks taken so far: 1 for a cheezie, 2 for a chocolate, 5 for a cookie. It is worked out from
    /// the snacks, so it is not hashed on its own.
    pub fn score(&self) -> u32 {
        self.snacks
            .iter()
            .filter(|s| s.taken)
            .map(|s| s.kind.points())
            .sum()
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
        let f = self
            .ben
            .tick(&self.map, &[], input, &mut self.ammo, &mut self.shots);
        // Fizz 1's order inside `tick_ben_move`: the jump or bounce, the shot, then the head bump.
        if let Some(p) = f.bounced {
            let (x, y) = sixteenths(p);
            self.events.push(Event::Bounce { x, y });
        }
        if let Some(p) = f.jumped {
            let (x, y) = sixteenths(p);
            self.events.push(Event::Jump { x, y });
        }
        if let Some(p) = f.fired {
            let (x, y) = sixteenths(p);
            self.events.push(Event::Fire { x, y });
        }
        if let Some(p) = f.dud {
            let (x, y) = sixteenths(p);
            self.events.push(Event::Dud { x, y });
        }
        if let Some(p) = f.bumped {
            let (x, y) = sixteenths(p);
            self.events.push(Event::HeadBump { x, y });
        }
        if !dead {
            self.tick_ben_contacts();
        }
    }

    /// Kills Ben unless he is already dead.
    fn kill(&mut self) {
        if self.ben.dead > 0.0 {
            return;
        }
        self.ben.kill();
        let (x, y) = sixteenths((self.ben.b.x, self.ben.b.y + 1.6));
        self.events.push(Event::Death { x, y });
    }

    /// The lock phase. Fizz 1 tries each door colour in turn (red, then blue) with the key that colour needs; here the level's
    /// locks run in rule order. A lock fires when Ben holds its key, the tile in front of him at `py + 0.2` is a switched tile on
    /// the lock's channel and that channel is on (the door is closed): the channel goes off, the key is spent and the door opens.
    fn tick_locks(&mut self) {
        let (px, py, pw) = (self.ben.b.x, self.ben.b.y, self.ben.b.w);
        for i in 0..self.rules.len() {
            let Some(lock) = self.rules.lock_at(i) else {
                continue;
            };
            let Some((key, used)) = WorldFlags::key_bits(lock.colour) else {
                continue;
            };
            if !self.flags.contains(key) {
                continue;
            }
            let cx = if self.ben.face > 0.0 {
                (px + pw + 0.1).floor() as i32
            } else {
                (px - 0.1).floor() as i32
            };
            let t = self.map.get(cx, (py + 0.2).floor() as i32);
            let props = &self.map.props;
            if props.flags(t) & SWITCHED != 0
                && props.channel(t) == lock.channel
                && self.map.switch(lock.channel)
            {
                self.map.set_switch(lock.channel, false);
                self.flags.clear(key);
                self.flags.set(used);
                let (x, y) = sixteenths((f64::from(cx) + 0.5, py + 2.0));
                self.events.push(Event::DoorOpen { x, y });
            }
        }
    }

    /// Fizz 1's `tick_ben_contacts`: hazards and the exit, the locks (the doors), the keys, then the snacks and the chickens.
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
                    let (x, y) = sixteenths((px, py + 2.0));
                    self.events.push(Event::Exit { x, y });
                    return;
                }
            }
        }
        self.tick_locks();
        if die {
            self.kill();
            return;
        }

        if let Some((kx, ky)) = self.key
            && self.key_in_world()
            && self.ben.b.overlaps(kx - 0.4, ky - 0.4, 0.8, 0.8)
        {
            self.flags.set(WorldFlags::KEY_RED);
            let (x, y) = sixteenths((kx, ky + 0.6));
            self.events.push(Event::Key { colour: 0, x, y });
        }

        if let Some((kx, ky)) = self.key_blue
            && self.key_blue_in_world()
            && self.ben.b.overlaps(kx - 0.4, ky - 0.4, 0.8, 0.8)
        {
            self.flags.set(WorldFlags::KEY_BLUE);
            let (x, y) = sixteenths((kx, ky + 0.6));
            self.events.push(Event::Key { colour: 1, x, y });
        }

        for i in 0..self.snacks.len() {
            let sn = self.snacks[i];
            if !sn.taken && self.ben.b.overlaps(sn.x - 0.4, sn.y - 0.4, 0.8, 0.8) {
                self.snacks[i].taken = true;
                let (x, y) = sixteenths((sn.x, sn.y + 0.6));
                if sn.kind == SnackKind::Soda {
                    self.ammo += SODA_AMMO;
                    self.events.push(Event::Can { x, y });
                } else {
                    self.events.push(Event::Snack { x, y });
                }
            }
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
                let (x, y) = sixteenths((b.x + b.w / 2.0, b.y + b.h + 0.4));
                self.events.push(Event::Stomp {
                    chicken: i as u8,
                    x,
                    y,
                });
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
                        let (x, y) = sixteenths((ex + ew / 2.0, ey + eh + 0.4));
                        self.events.push(Event::Stun {
                            chicken: j as u8,
                            x,
                            y,
                        });
                        break;
                    }
                }
            }
            if gone {
                self.events.push(Event::ShotEnd {
                    x: (b.x * 16.0).floor() as i32,
                    y: (b.y * 16.0).floor() as i32,
                });
                self.shots.swap_remove(i);
            } else {
                self.shots[i] = b;
            }
        }
    }

    /// Puts Ben back at the spawn after a death (Fizz 1's `on_death`, which hands the reload to the shell) and resets the level
    /// exactly as starting it the first time would: the switch channels go back to the level's start state (a door that was
    /// opened closes), the keys leave Ben's hands and the spent marks are cleared (every key lies where it began), the snacks
    /// come back, and the chickens and the ammo start over. Rules that act on the start of a level (timers, start rules) will
    /// re-arm here when they exist; the locks hold no state of their own.
    fn respawn(&mut self) {
        self.deaths += 1;
        self.ben = Ben::new(self.spawn.0, self.spawn.1);
        self.map.switches = self.start_switches;
        for bit in [
            WorldFlags::KEY_RED,
            WorldFlags::KEY_BLUE,
            WorldFlags::KEY_RED_USED,
            WorldFlags::KEY_BLUE_USED,
        ] {
            self.flags.clear(bit);
        }
        self.ammo = START_AMMO;
        self.shots.clear();
        for s in &mut self.snacks {
            s.taken = false;
        }
        self.chickens = self
            .chicken_spawns
            .iter()
            .enumerate()
            .map(|(id, &(x, y, dir))| Chicken::new(x, y, dir, id as u32, self.tick.0))
            .collect();
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
/// 6. the shot count (`u32`), then per shot `x y vx vy life` as `f64` bits;
/// 7. the snack count (`u32`), then per snack one byte, 1 if it has been taken and 0 if not. The soda cans are in this list too, in the order the level
///    added them, so a taken can is hashed like a taken snack (the ammo it gave is in `ammo`, step 3). A snack's kind and position
///    are level data, and the score is worked out from what is taken, so neither is hashed. The blue key and door live in
///    the [`WorldFlags`] word and in `map.switches` (bit 2).
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
    h.write_u32(w.snacks.len() as u32);
    for s in &w.snacks {
        h.write_u8(u8::from(s.taken));
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
    use fizz_sim::Buttons;

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
        w.add_snack(SnackKind::Cheezie, 2.0, 3.0);
        w.add_snack(SnackKind::Cookie, 3.0, 3.0);
        w.snacks[1].taken = true;
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
        b.extend_from_slice(&2u32.to_le_bytes()); // snack count
        b.extend_from_slice(&[0, 1]); // taken: the cheezie no, the cookie yes
        assert_eq!(hash(&w).finish(), fnv(&b));
    }

    #[test]
    fn the_hash_of_a_known_world_is_locked() {
        // This pins a world with every field set, so a reordering that happens to leave the slice's start unchanged is
        // still caught. `slice-replay.txt` pins the slice itself.
        assert_eq!(hash(&populated()).finish(), 0xff10_8a2f_9aab_52a5);
    }

    #[test]
    fn every_hashed_field_changes_the_digest() {
        let base = hash(&populated()).finish();
        let edits: [fn(&mut World); 31] = [
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
            |w| w.snacks[0].taken = true,
            |w| w.snacks[1].taken = false,
            |w| {
                w.flags
                    .set(WorldFlags::KEY_BLUE | WorldFlags::KEY_BLUE_USED)
            },
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
                if let Event::Cluck { variant, .. } = *e {
                    out.push((w.tick.0, variant));
                }
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
        let Event::Cluck { variant, chicken } = w.events()[0] else {
            panic!("a cluck")
        };
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
        let Event::Cluck { chicken, variant } = w.events()[0] else {
            panic!("a cluck")
        };
        assert_eq!(w.events().len(), 1);
        assert_eq!(chicken, 1);
        assert_eq!(w.chickens[1].cluck_age, 0);
        assert_eq!(w.chickens[1].cluck_variant, variant);
        assert_eq!(w.chickens[0].cluck_age, crate::chicken::CLUCK_NEVER);
    }

    /// The `ShotEnd` events of the last step.
    fn shot_ends(w: &World) -> Vec<(i32, i32)> {
        w.events()
            .iter()
            .filter_map(|e| match *e {
                Event::ShotEnd { x, y } => Some((x, y)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_shot_end_is_emitted_once_for_each_way_a_shot_ends() {
        // Out of life: one event at the shot's last position, in sixteenths of a tile, and the next step has none.
        let mut w = quiet();
        w.chickens[0].cluck_timer = 100_000;
        w.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.0,
        });
        w.step(idle());
        assert_eq!(shot_ends(&w), [(160, 176)]);
        assert!(w.shots.is_empty());
        w.step(idle());
        assert!(shot_ends(&w).is_empty());
        // A shot in flight emits nothing until it ends.
        w.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.5,
        });
        let mut steps = 0;
        while w.events().is_empty() || !w.shots.is_empty() {
            w.step(idle());
            steps += 1;
            assert!(steps <= 32, "a half-second shot ends within 32 ticks");
            if !w.shots.is_empty() {
                assert!(shot_ends(&w).is_empty());
            }
        }
        assert!(steps >= 29);
        assert_eq!(shot_ends(&w).len(), 1);
        // Off the bottom of the map.
        w.shots.push(Shot {
            x: 10.0,
            y: 0.1,
            vx: 0.0,
            vy: -16.0,
            life: 5.0,
        });
        w.step(idle());
        assert_eq!(shot_ends(&w).len(), 1);
        // Two shots ending on one tick give two events.
        w.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.0,
        });
        w.shots.push(Shot {
            x: 12.5,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.0,
        });
        w.step(idle());
        assert_eq!(shot_ends(&w), [(200, 176), (160, 176)]);
    }

    #[test]
    fn a_shot_that_stuns_a_chicken_ends_once_and_a_stunned_chicken_does_not_stop_it() {
        let mut w = quiet();
        w.chickens[0].cluck_timer = 100_000;
        let (cx, cy) = (w.chickens[0].b.x + 0.5, w.chickens[0].b.y + 0.5);
        w.shots.push(Shot {
            x: cx,
            y: cy,
            vx: 0.0,
            vy: 0.0,
            life: 5.0,
        });
        w.step(idle());
        assert_eq!(shot_ends(&w).len(), 1);
        assert!(w.chickens[0].stun > 0.0);
        w.shots.push(Shot {
            x: cx,
            y: cy,
            vx: 0.0,
            vy: 0.0,
            life: 0.05,
        });
        for _ in 0..6 {
            w.step(idle());
        }
        assert!(w.shots.is_empty());
    }

    #[test]
    fn shot_ends_do_not_touch_the_hash() {
        // Two worlds that differ in nothing but history hash alike: the event list is output only.
        let mut a = quiet();
        let mut b = quiet();
        a.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.0,
        });
        b.shots.push(Shot {
            x: 10.0,
            y: 11.0,
            vx: 0.0,
            vy: 0.0,
            life: 0.0,
        });
        a.step(idle());
        b.step(idle());
        b.events.clear();
        assert_eq!(a.hash(), b.hash());
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

    /// An empty arena with a solid floor under row 1 and, when `ceiling` is set, a solid ceiling at row 6, Ben at rest on
    /// the floor and no chickens, so only Ben's own events can happen.
    fn arena(ceiling: Option<i32>) -> World {
        let mut map = TileMap::new(24, 14, tiles::props());
        for x in 0..24 {
            map.set(x, 0, tiles::BLOCK);
            map.set(x, 1, tiles::BLOCK);
            if let Some(row) = ceiling {
                map.set(x, row, tiles::BLOCK);
            }
        }
        let mut w = World::new(map, (10.0, 2.0));
        for _ in 0..5 {
            w.step(idle());
        }
        w
    }

    fn press(w: &mut World, prev: Buttons, held: Buttons) {
        w.step(InputFrame::from_transition(prev, held));
    }

    fn kinds(w: &World) -> Vec<Event> {
        w.events().to_vec()
    }

    #[test]
    fn firing_raises_fire_and_an_empty_blaster_raises_dud() {
        let mut w = arena(None);
        press(&mut w, Buttons::NONE, Buttons::FIRE);
        assert!(
            matches!(kinds(&w)[..], [Event::Fire { .. }]),
            "{:?}",
            kinds(&w)
        );
        assert_eq!(w.ammo, START_AMMO - 1);
        let Event::Fire { x, y } = w.events()[0] else {
            panic!()
        };
        // Fizz 1: the shot starts 0.6 ahead of Ben's middle, 0.85 up, and the caption half a tile above that.
        assert_eq!(
            x,
            ((w.ben.b.x + w.ben.b.w / 2.0 + 0.6) * 16.0).floor() as i32
        );
        assert_eq!(y, ((2.0 + 0.85 + 0.5) * 16.0).floor() as i32);

        let mut w = arena(None);
        w.ammo = 0;
        press(&mut w, Buttons::NONE, Buttons::FIRE);
        let Event::Dud { x, y } = w.events()[0] else {
            panic!("a dud: {:?}", kinds(&w))
        };
        assert_eq!(w.events().len(), 1, "no Fire without ammo");
        assert!(w.shots.is_empty());
        assert_eq!(x, ((10.0 + 0.35) * 16.0).floor() as i32);
        assert_eq!(y, ((2.0 + 1.6) * 16.0).floor() as i32);
    }

    #[test]
    fn a_ground_jump_raises_jump_once_and_the_pogo_raises_bounce_instead() {
        let mut w = arena(None);
        press(&mut w, Buttons::NONE, Buttons::JUMP);
        assert!(
            matches!(kinds(&w)[..], [Event::Jump { .. }]),
            "{:?}",
            kinds(&w)
        );
        let Event::Jump { x, y } = w.events()[0] else {
            panic!()
        };
        assert_eq!((x, y), (160, 32));
        for _ in 0..3 {
            press(&mut w, Buttons::JUMP, Buttons::JUMP);
            assert!(w.events().is_empty(), "airborne: {:?}", kinds(&w));
        }

        let mut w = arena(None);
        press(&mut w, Buttons::NONE, Buttons::POGO);
        let ev = kinds(&w);
        assert!(matches!(ev[..], [Event::Bounce { .. }]), "{ev:?}");
        let Event::Bounce { x, y } = ev[0] else {
            panic!()
        };
        assert_eq!((x, y), (((10.0 + 0.35) * 16.0) as i32, 32));
        // Each landing bounces again, and no jump sound goes with it.
        let mut bounces = 0;
        for _ in 0..120 {
            press(&mut w, Buttons::POGO, Buttons::POGO);
            for e in w.events() {
                assert!(!matches!(e, Event::Jump { .. }));
                bounces += usize::from(matches!(e, Event::Bounce { .. }));
            }
        }
        assert!(bounces >= 2, "{bounces}");
    }

    #[test]
    fn a_head_bump_is_raised_once_per_rise_under_a_ceiling() {
        // Floor top at row 2; ceiling tile at row 6 leaves a 4-tile gap, which a 20.5 tiles/s jump (about 3.4 tiles) crosses
        // only by Ben's 1.4 tile body: his head meets the ceiling.
        let mut w = arena(Some(5));
        press(&mut w, Buttons::NONE, Buttons::JUMP);
        let mut bumps = Vec::new();
        for _ in 0..80 {
            press(&mut w, Buttons::JUMP, Buttons::JUMP);
            for e in w.events() {
                if let Event::HeadBump { x, y } = *e {
                    bumps.push((w.tick.0, x, y));
                }
            }
        }
        assert_eq!(bumps.len(), 1, "{bumps:?}");
        let (_, x, y) = bumps[0];
        assert_eq!(x, ((10.0 + w.ben.b.w / 2.0) * 16.0).floor() as i32);
        assert!((16 * 5 - 24..=16 * 5).contains(&y), "head height {y}");
        // Without a ceiling the same jump never bumps.
        let mut w = arena(None);
        press(&mut w, Buttons::NONE, Buttons::JUMP);
        for _ in 0..80 {
            press(&mut w, Buttons::JUMP, Buttons::JUMP);
            assert!(
                !w.events()
                    .iter()
                    .any(|e| matches!(e, Event::HeadBump { .. }))
            );
        }
    }

    #[test]
    fn contacts_raise_death_key_door_exit_stomp_and_stun() {
        // Death: a spike under Ben kills him once, 1.6 tiles above his feet.
        let mut w = arena(None);
        w.map.set(10, 2, tiles::SPIKE);
        w.step(idle());
        let ev = kinds(&w);
        assert!(matches!(ev[..], [Event::Death { .. }]), "{ev:?}");
        let Event::Death { x, y } = ev[0] else {
            panic!()
        };
        assert_eq!((x, y), (160, ((w.ben.b.y + 1.6) * 16.0).floor() as i32));
        w.step(idle());
        assert!(w.events().is_empty(), "dying is raised once");

        // Exit: raised once, then the world freezes Ben.
        let mut w = arena(None);
        w.map.set(10, 2, tiles::EXIT);
        w.step(idle());
        assert!(
            matches!(kinds(&w)[..], [Event::Exit { .. }]),
            "{:?}",
            kinds(&w)
        );
        w.step(idle());
        assert!(w.events().is_empty());

        // Key: red, at the key's centre and 0.6 up.
        let mut w = arena(None);
        w.set_key(10.4, 2.7);
        w.step(idle());
        assert_eq!(
            kinds(&w),
            [Event::Key {
                colour: 0,
                x: (10.4f64 * 16.0).floor() as i32,
                y: (3.3f64 * 16.0).floor() as i32
            }]
        );

        // Stun: a shot in a chicken raises Stun before its ShotEnd.
        let mut w = quiet();
        w.chickens[0].cluck_timer = 100_000;
        let (cx, cy) = (w.chickens[0].b.x + 0.5, w.chickens[0].b.y + 0.5);
        w.shots.push(Shot {
            x: cx,
            y: cy,
            vx: 0.0,
            vy: 0.0,
            life: 5.0,
        });
        w.step(idle());
        let ev = kinds(&w);
        assert!(
            matches!(
                ev[..],
                [Event::Stun { chicken: 0, .. }, Event::ShotEnd { .. }]
            ),
            "{ev:?}"
        );
        // A second shot through the stunned chicken stuns nothing.
        w.shots.push(Shot {
            x: cx,
            y: cy,
            vx: 0.0,
            vy: 0.0,
            life: 0.05,
        });
        w.step(idle());
        assert!(!kinds(&w).iter().any(|e| matches!(e, Event::Stun { .. })));
    }

    #[test]
    fn events_do_not_move_the_hash() {
        // The same inputs with the event list ignored give the same hash whether or not anyone reads the events.
        let (mut a, mut b) = (arena(Some(5)), arena(Some(5)));
        for t in 0..90 {
            let held = if t % 20 < 10 {
                Buttons::JUMP | Buttons::FIRE
            } else {
                Buttons::FIRE
            };
            let f = InputFrame::from_transition(Buttons::NONE, held);
            a.step(f);
            b.events.clear();
            b.step(f);
            assert_eq!(hash(&a).finish(), hash(&b).finish());
        }
    }

    #[test]
    fn a_soda_can_gives_five_shots_with_no_cap_and_raises_can_not_snack() {
        // Fizz 1: `ItemKind::Soda => { self.game.ammo += 5; cap(.., Fsssht) }`; there is no ceiling on the ammo.
        let mut w = arena(None);
        w.add_snack(SnackKind::Soda, 10.4, 2.7);
        w.ammo = 0;
        w.step(idle());
        assert_eq!(w.ammo, SODA_AMMO);
        assert_eq!(SODA_AMMO, 5);
        assert_eq!(
            kinds(&w),
            [Event::Can {
                x: (10.4f64 * 16.0).floor() as i32,
                y: (3.3f64 * 16.0).floor() as i32,
            }]
        );
        assert!(w.snacks[0].taken);
        assert_eq!(w.score(), 0, "a can is worth no points");
        // Taken once: standing in it again gives nothing.
        w.step(idle());
        assert_eq!(w.ammo, SODA_AMMO);
        assert!(w.events().is_empty());
        // A full blaster takes cans too, and goes past its starting five.
        let mut w = arena(None);
        w.add_snack(SnackKind::Soda, 10.4, 2.7);
        w.add_snack(SnackKind::Soda, 10.6, 2.7);
        w.step(idle());
        assert_eq!(w.ammo, START_AMMO + 2 * SODA_AMMO);
    }

    #[test]
    fn cans_and_snacks_raise_their_events_in_list_order_on_the_same_tick() {
        let mut w = arena(None);
        w.add_snack(SnackKind::Cookie, 10.4, 2.7);
        w.add_snack(SnackKind::Soda, 10.5, 2.7);
        w.add_snack(SnackKind::Cheezie, 10.6, 2.7);
        w.step(idle());
        let ev = kinds(&w);
        assert!(
            matches!(
                ev[..],
                [Event::Snack { .. }, Event::Can { .. }, Event::Snack { .. }]
            ),
            "{ev:?}"
        );
        assert_eq!(w.score(), 6);
        assert_eq!(w.ammo, START_AMMO + SODA_AMMO);
    }

    #[test]
    fn a_death_puts_the_cans_back_and_the_ammo_at_five() {
        // A death puts the cans back, like the snacks (Fizz 1 reloads the level and its items), and the slice starts the blaster over at five.
        let mut w = arena(None);
        w.add_snack(SnackKind::Soda, 20.4, 2.7);
        w.snacks[0].taken = true;
        w.ammo = 12;
        w.ben.kill();
        for _ in 0..100 {
            w.step(idle());
        }
        assert_eq!(w.deaths, 1);
        assert_eq!((w.ammo, w.snacks[0].taken), (START_AMMO, false));
    }

    #[test]
    fn a_taken_can_is_hashed_like_a_taken_snack() {
        let mut a = arena(None);
        a.add_snack(SnackKind::Soda, 20.4, 2.7);
        a.add_snack(SnackKind::Cheezie, 21.4, 2.7);
        let mut b = a.clone();
        assert_eq!(a.hash(), b.hash());
        b.snacks[0].taken = true;
        assert_ne!(a.hash(), b.hash(), "the can's taken byte is hashed");
        // The kind is level data: it is not hashed, only whether the item has been taken.
        let mut c = a.clone();
        c.snacks[0].kind = SnackKind::Cookie;
        assert_eq!(a.hash(), c.hash());
        // A can is one more snack-list byte, and the count.
        assert_ne!(arena(None).hash(), a.hash());
    }
}
