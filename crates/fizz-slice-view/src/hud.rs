// What the slice's HUD shows, as pure data both presenters draw: the numbers, the key tray and the animation of a pickup.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The rules keep no HUD state and this file adds none to them. Everything is read from the world (`ammo`, `deaths`, the snacks
// taken, the key flags) or kept by the view in [`HudAnim`], which a presenter feeds with [`HudAnim::observe`] after each step, as
// it feeds the puffs and captions. The hash never sees it.
//
// The layout is the Two-Row Stack: `Fizz` and `Score` on the top row, `Deaths` and the key tray on the bottom row.

use fizz_ep2::{World, WorldFlags};

/// The most a Fizz or Deaths count shows.
pub const COUNT_CAP: u32 = 99;
/// How many digits the score field has, always drawn, with dim leading zeros.
pub const SCORE_DIGITS: usize = 5;
/// The largest score the field shows. The true score continues past it; only the display holds.
pub const SCORE_CAP: u32 = 99_999;
/// The score counts up one point every this many ticks after a snack (a cookie, 5 points, takes 10 ticks).
pub const COUNT_TICKS: u8 = 2;
/// How long the `+N` stays in the place of the word `Score` after a snack, in ticks (one second).
pub const POP_TICKS: u16 = 60;
/// How long a socket keeps its white ring after its key is picked up, in ticks. The design's six frames are of the DOS draw rate
/// (30 a second), so this is twelve ticks.
pub const RING_TICKS: u8 = 12;
/// The sockets a tray holds: one per kind of key.
pub const KEY_SLOTS: usize = 4;
/// The most pips shown under a socket; a larger count of one kind shows as a digit.
pub const MAX_PIPS: u8 = 4;

/// A kind of key. The tray has one socket for each kind the level has, in this order. The slice has the red and blue ones; the
/// green gumdrop and the gold USB stick are listed here for the levels that gain them (see [`key_state`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyKind {
    Red,
    Blue,
    Green,
    Gold,
}

/// The kinds, in the order the tray shows them.
pub const KEY_KINDS: [KeyKind; KEY_SLOTS] =
    [KeyKind::Red, KeyKind::Blue, KeyKind::Green, KeyKind::Gold];

/// The shape drawn on a key besides its colour, so keys are not told apart by hue alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyMark {
    Dot,
    Bar,
    Plus,
    /// The gold key is a USB stick, a different silhouette.
    Usb,
}

impl KeyKind {
    /// The kind's place in the tray, 0 to 3.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The name a screen reader and the manual use.
    pub const fn name(self) -> &'static str {
        match self {
            KeyKind::Red => "red",
            KeyKind::Blue => "blue",
            KeyKind::Green => "green",
            KeyKind::Gold => "gold",
        }
    }

    pub const fn mark(self) -> KeyMark {
        match self {
            KeyKind::Red => KeyMark::Dot,
            KeyKind::Blue => KeyMark::Bar,
            KeyKind::Green => KeyMark::Plus,
            KeyKind::Gold => KeyMark::Usb,
        }
    }

    /// The name of the 7 by 7 socket sprite in `assets/sprites/ep2-hud-keys.json`.
    pub const fn sprite(self) -> &'static str {
        match self {
            KeyKind::Red => "hudkey_red",
            KeyKind::Blue => "hudkey_blue",
            KeyKind::Green => "hudkey_green",
            KeyKind::Gold => "hudkey_gold",
        }
    }
}

/// One socket of the tray: whether the level has this kind of key, and how many of it Ben carries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyState {
    pub in_level: bool,
    pub held: u8,
}

/// The state of `kind` in `world`. The red and blue keys come from the level's key positions and the flags; the other kinds are
/// not in the slice, so their sockets are not shown. A level that gains one adds its arm here, from its level data and a flag,
/// and nothing else in either presenter changes.
pub fn key_state(world: &World, kind: KeyKind) -> KeyState {
    let (in_level, flag) = match kind {
        KeyKind::Red => (world.level_has_red_key(), WorldFlags::KEY_RED),
        KeyKind::Blue => (world.level_has_blue_key(), WorldFlags::KEY_BLUE),
        KeyKind::Green | KeyKind::Gold => (false, 0),
    };
    KeyState {
        in_level,
        held: u8::from(flag != 0 && world.flags.contains(flag)),
    }
}

/// What the HUD is made from, read straight from the world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HudModel {
    /// Fizz Blaster shots left.
    pub fizz: u32,
    /// The true score: Fizz 1's snack points (cheezie 1, chocolate 2, cookie 5) for the snacks taken, worked out from them.
    pub score: u32,
    pub deaths: u32,
    pub keys: [KeyState; KEY_SLOTS],
}

impl HudModel {
    pub fn from_world(world: &World) -> Self {
        HudModel {
            fizz: world.ammo,
            score: world.score(),
            deaths: world.deaths,
            keys: KEY_KINDS.map(|k| key_state(world, k)),
        }
    }
}

/// A count as the HUD prints it: at most [`COUNT_CAP`].
pub const fn count(n: u32) -> u32 {
    if n > COUNT_CAP { COUNT_CAP } else { n }
}

/// The score field: five digits, and how many of the leading ones are zeros that only pad (drawn dim). The display holds at
/// [`SCORE_CAP`]. A score of 0 pads four zeros and shows the last as a digit.
pub fn score_digits(score: u32) -> ([u8; SCORE_DIGITS], usize) {
    let n = if score > SCORE_CAP { SCORE_CAP } else { score };
    let mut digits = [0u8; SCORE_DIGITS];
    let mut rest = n;
    for d in digits.iter_mut().rev() {
        *d = (rest % 10) as u8;
        rest /= 10;
    }
    let mut pad = 0;
    while pad < SCORE_DIGITS - 1 && digits[pad] == 0 {
        pad += 1;
    }
    (digits, pad)
}

/// How a socket marks duplicates: pips under it for two to [`MAX_PIPS`] keys of the kind, a digit for more, nothing for one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Extra {
    None,
    Pips(u8),
    Digit(u8),
}

pub const fn extra(held: u8) -> Extra {
    if held <= 1 {
        Extra::None
    } else if held <= MAX_PIPS {
        Extra::Pips(held)
    } else if held >= 9 {
        Extra::Digit(9)
    } else {
        Extra::Digit(held)
    }
}

/// A socket as drawn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeySlot {
    pub in_level: bool,
    pub held: u8,
    /// The white ring round the key for the moment after it is picked up.
    pub ring: bool,
}

/// The HUD as a presenter draws it. Equal views draw equal pictures, so a presenter that caches its drawing renders again only
/// when the view changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HudView {
    pub fizz: u32,
    /// The score shown now, which counts up to the true score after a snack.
    pub score: u32,
    /// The points of the snack just taken, or 0: shown as `+N` in the place of the word `Score`.
    pub pop: u32,
    pub deaths: u32,
    pub keys: [KeySlot; KEY_SLOTS],
}

/// The view's own HUD animation state: the score counting up, the `+N` and the rings. It is fed after every step and never read
/// by the rules.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HudAnim {
    /// Whether a step has been observed. Until one has, the view shows the world as it is, with no animation.
    seen: bool,
    tick: u64,
    shown: u32,
    target: u32,
    clock: u8,
    pop: u32,
    pop_left: u16,
    ring: [u8; KEY_SLOTS],
    held: [u8; KEY_SLOTS],
}

impl HudAnim {
    pub const fn new() -> Self {
        HudAnim {
            seen: false,
            tick: 0,
            shown: 0,
            target: 0,
            clock: 0,
            pop: 0,
            pop_left: 0,
            ring: [0; KEY_SLOTS],
            held: [0; KEY_SLOTS],
        }
    }

    /// Takes the step that just ran. `reduced` is the Motion setting: the score then jumps to its new value and no ring shows,
    /// and the `+N` shows still. A death puts the snacks back, so the score falls to zero at once; a restart (the tick going
    /// back) starts over.
    pub fn observe(&mut self, world: &World, reduced: bool) {
        let m = HudModel::from_world(world);
        let held = m.keys.map(|k| k.held);
        if !self.seen || world.tick.0 < self.tick {
            *self = HudAnim {
                seen: true,
                tick: world.tick.0,
                shown: m.score,
                target: m.score,
                held,
                ..HudAnim::new()
            };
            return;
        }
        self.tick = world.tick.0;
        if m.score > self.target {
            // A second snack inside the second adds to the same `+N`.
            let base = if self.pop_left > 0 { self.pop } else { 0 };
            self.pop = base + (m.score - self.target);
            self.pop_left = POP_TICKS;
            self.target = m.score;
        } else if m.score < self.target {
            self.target = m.score;
            self.shown = m.score;
            self.pop = 0;
            self.pop_left = 0;
        }
        if reduced {
            self.shown = self.target;
            self.clock = 0;
        } else if self.shown < self.target {
            self.clock += 1;
            if self.clock >= COUNT_TICKS {
                self.clock = 0;
                self.shown += 1;
            }
        } else {
            self.clock = 0;
        }
        if self.pop_left > 0 {
            self.pop_left -= 1;
            if self.pop_left == 0 {
                self.pop = 0;
            }
        }
        for ((ring, was), now) in self.ring.iter_mut().zip(self.held).zip(held) {
            *ring = if reduced {
                0
            } else if now > was {
                RING_TICKS
            } else {
                ring.saturating_sub(1)
            };
        }
        self.held = held;
    }

    /// The HUD for `world` now.
    pub fn view(&self, world: &World) -> HudView {
        let m = HudModel::from_world(world);
        let animated = self.seen;
        let score = if animated { self.shown } else { m.score };
        HudView {
            fizz: count(m.fizz),
            score: score.min(SCORE_CAP),
            pop: if animated { self.pop } else { 0 },
            deaths: count(m.deaths),
            keys: core::array::from_fn(|i| KeySlot {
                in_level: m.keys[i].in_level,
                held: m.keys[i].held,
                ring: animated && self.ring[i] > 0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fizz_ep2::level;

    fn step_n(w: &mut World, a: &mut HudAnim, n: u32, reduced: bool) {
        for _ in 0..n {
            w.tick = w.tick.next();
            a.observe(w, reduced);
        }
    }

    fn take(w: &mut World, i: usize) {
        w.snacks[i].taken = true;
    }

    fn cookie(w: &World) -> usize {
        w.snacks.iter().position(|s| s.kind.points() == 5).unwrap()
    }

    #[test]
    fn the_score_is_worked_out_from_the_snacks_taken() {
        let mut w = level::world();
        assert_eq!(HudModel::from_world(&w).score, 0);
        // Cheezie 1, chocolate 2, cookie 5; the nook holds ten points in all.
        let total: u32 = w.snacks.iter().map(|s| s.kind.points()).sum();
        assert_eq!(total, 10);
        for i in 0..w.snacks.len() {
            take(&mut w, i);
        }
        assert_eq!(HudModel::from_world(&w).score, 10);
        // Reading the model changes nothing the hash covers.
        let before = fizz_ep2::hash(&w);
        let _ = HudModel::from_world(&w);
        assert_eq!(fizz_ep2::hash(&w), before);
    }

    #[test]
    fn the_score_field_is_five_digits_with_padding_and_holds_at_the_cap() {
        assert_eq!(score_digits(0), ([0, 0, 0, 0, 0], 4));
        assert_eq!(score_digits(5), ([0, 0, 0, 0, 5], 4));
        assert_eq!(score_digits(1240), ([0, 1, 2, 4, 0], 1));
        assert_eq!(score_digits(99_999), ([9, 9, 9, 9, 9], 0));
        assert_eq!(score_digits(100_000), ([9, 9, 9, 9, 9], 0));
        assert_eq!(score_digits(u32::MAX), ([9, 9, 9, 9, 9], 0));
    }

    #[test]
    fn the_view_clamps_the_display_and_the_true_score_continues() {
        let mut w = level::world();
        take(&mut w, 0);
        let mut a = HudAnim::new();
        a.observe(&w, true);
        a.target = SCORE_CAP + 500;
        a.shown = SCORE_CAP + 500;
        assert_eq!(a.view(&w).score, SCORE_CAP);
        // The model, and the world's own score, are not clamped.
        assert_eq!(HudModel::from_world(&w).score, w.score());
    }

    #[test]
    fn the_counts_cap_at_99() {
        assert_eq!(
            (count(0), count(99), count(100), count(u32::MAX)),
            (0, 99, 99, 99)
        );
    }

    #[test]
    fn a_fresh_view_shows_the_world_with_no_animation() {
        let mut w = level::world();
        take(&mut w, 1);
        let v = HudAnim::new().view(&w);
        assert_eq!((v.score, v.pop), (w.score(), 0));
        assert!(v.keys.iter().all(|k| !k.ring));
    }

    #[test]
    fn a_snack_pops_and_the_score_counts_up_a_point_every_two_ticks() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        let c = cookie(&w);
        take(&mut w, c);
        let mut seen = [0u32; 12];
        for s in seen.iter_mut() {
            step_n(&mut w, &mut a, 1, false);
            *s = a.view(&w).score;
        }
        assert_eq!(a.view(&w).pop, 5);
        // One point per COUNT_TICKS ticks, so a cookie's five points take ten ticks.
        assert_eq!(seen, [0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 5]);
        // The +N stays for a second from the pickup, then goes.
        step_n(&mut w, &mut a, 40, false);
        assert_eq!(a.view(&w).pop, 5);
        step_n(&mut w, &mut a, 10, false);
        assert_eq!(a.view(&w).pop, 0);
        assert_eq!(a.view(&w).score, 5);
    }

    #[test]
    fn two_snacks_in_a_second_add_to_one_pop() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        take(&mut w, 0);
        step_n(&mut w, &mut a, 1, false);
        take(&mut w, 1);
        step_n(&mut w, &mut a, 3, false);
        assert_eq!(
            a.view(&w).pop,
            w.snacks[0].kind.points() + w.snacks[1].kind.points()
        );
    }

    #[test]
    fn reduced_motion_makes_the_score_jump_with_no_ring() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, true);
        let c = cookie(&w);
        take(&mut w, c);
        w.flags.set(WorldFlags::KEY_RED);
        step_n(&mut w, &mut a, 1, true);
        let v = a.view(&w);
        assert_eq!(v.score, 5, "jumps at once");
        assert_eq!(v.pop, 5, "the +N shows still");
        assert!(v.keys.iter().all(|k| !k.ring));
        assert_eq!(v.keys[0].held, 1);
    }

    #[test]
    fn switching_to_reduced_mid_count_finishes_the_count() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        let c = cookie(&w);
        take(&mut w, c);
        step_n(&mut w, &mut a, 2, false);
        assert!(a.view(&w).score < 5);
        step_n(&mut w, &mut a, 1, true);
        assert_eq!(a.view(&w).score, 5);
    }

    #[test]
    fn a_death_puts_the_snacks_back_and_the_score_falls_at_once() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        take(&mut w, 0);
        step_n(&mut w, &mut a, 20, false);
        assert!(a.view(&w).score > 0);
        w.snacks[0].taken = false;
        step_n(&mut w, &mut a, 1, false);
        let v = a.view(&w);
        assert_eq!((v.score, v.pop), (0, 0));
    }

    #[test]
    fn a_restart_starts_the_animation_over() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        take(&mut w, 0);
        step_n(&mut w, &mut a, 3, false);
        let fresh = level::world();
        a.observe(&fresh, false);
        let v = a.view(&fresh);
        assert_eq!((v.score, v.pop), (0, 0));
    }

    #[test]
    fn the_tray_has_a_socket_for_each_kind_the_level_has() {
        let w = level::world();
        let m = HudModel::from_world(&w);
        assert_eq!(m.keys.map(|k| k.in_level), [true, true, false, false]);
        assert!(m.keys.iter().all(|k| k.held == 0), "empty from the start");
        // A world with no keys has no sockets.
        let bare = World::new(w.map.clone(), (1.0, 1.0));
        assert!(HudModel::from_world(&bare).keys.iter().all(|k| !k.in_level));
    }

    #[test]
    fn a_pickup_fills_its_socket_and_rings_it_for_a_moment() {
        let mut w = level::world();
        let mut a = HudAnim::new();
        a.observe(&w, false);
        step_n(&mut w, &mut a, 2, false);
        assert!(!a.view(&w).keys[0].ring);
        w.flags.set(WorldFlags::KEY_RED);
        step_n(&mut w, &mut a, 1, false);
        let v = a.view(&w);
        assert_eq!((v.keys[0].held, v.keys[0].ring), (1, true));
        assert_eq!((v.keys[1].held, v.keys[1].ring), (0, false));
        step_n(&mut w, &mut a, u32::from(RING_TICKS) - 1, false);
        assert!(a.view(&w).keys[0].ring);
        step_n(&mut w, &mut a, 1, false);
        assert!(!a.view(&w).keys[0].ring);
        assert_eq!(a.view(&w).keys[0].held, 1, "the key stays in its socket");
        // Spending the key on the door empties the socket, as the chip always did, with no ring.
        w.flags.clear(WorldFlags::KEY_RED);
        step_n(&mut w, &mut a, 1, false);
        let v = a.view(&w);
        assert_eq!((v.keys[0].held, v.keys[0].ring), (0, false));
    }

    #[test]
    fn duplicates_show_as_pips_then_a_digit() {
        assert_eq!(extra(0), Extra::None);
        assert_eq!(extra(1), Extra::None);
        assert_eq!(extra(2), Extra::Pips(2));
        assert_eq!(extra(4), Extra::Pips(4));
        assert_eq!(extra(5), Extra::Digit(5));
        assert_eq!(extra(200), Extra::Digit(9));
    }

    #[test]
    fn every_kind_has_its_own_mark_and_sprite() {
        use fizz_view::sprites_data::sprite;
        for (i, k) in KEY_KINDS.iter().enumerate() {
            assert_eq!(k.index(), i);
            let s = sprite(k.sprite()).unwrap();
            assert_eq!((s.w, s.h), (7, 7));
            for (j, o) in KEY_KINDS.iter().enumerate() {
                if i != j {
                    assert_ne!(k.mark(), o.mark());
                    assert_ne!(s.pixels, sprite(o.sprite()).unwrap().pixels);
                    assert_ne!(k.name(), o.name());
                }
            }
        }
    }
}
