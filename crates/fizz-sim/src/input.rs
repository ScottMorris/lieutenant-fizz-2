// Defines the platform-neutral button state that presenters feed into the rules each tick.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::{BitAnd, BitOr, BitOrAssign, Not};

/// A set of logical buttons packed into one byte.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Buttons(pub u8);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const UP: Self = Self(1);
    pub const DOWN: Self = Self(2);
    pub const LEFT: Self = Self(4);
    pub const RIGHT: Self = Self(8);
    pub const JUMP: Self = Self(16);
    pub const POGO: Self = Self(32);
    pub const FIRE: Self = Self(64);

    /// Returns true when no button is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true when every button in `other` is also set here.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the buttons set in either set.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the buttons set in both sets.
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns the buttons set here and not in `other`.
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for Buttons {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl BitAnd for Buttons {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        self.intersection(rhs)
    }
}

impl Not for Buttons {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

/// The input the rules see for one tick.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputFrame {
    /// Buttons that are down at the end of the frame.
    pub held: Buttons,
    /// Buttons that went down at least once since the previous frame, even if already released.
    pub pressed: Buttons,
}

impl InputFrame {
    /// Builds a frame from two consecutive held states, where `pressed` is the newly set bits.
    pub const fn from_transition(prev: Buttons, now: Buttons) -> Self {
        Self {
            held: now,
            pressed: now.without(prev),
        }
    }
}

/// Collects key edges between frames so that a quick tap is never lost.
///
/// A platform calls `set` as keys change, then `frame` once per tick.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputLatch {
    held: Buttons,
    pending: Buttons,
}

impl InputLatch {
    pub const fn new() -> Self {
        Self {
            held: Buttons::NONE,
            pending: Buttons::NONE,
        }
    }

    /// Records a key edge. Repeated down events for an already-held button are not new presses.
    pub fn set(&mut self, b: Buttons, down: bool) {
        if down {
            self.pending |= b.without(self.held);
            self.held |= b;
        } else {
            self.held = self.held.without(b);
        }
    }

    /// Reports the held state and every down edge since the previous call, then clears the edges.
    pub fn frame(&mut self) -> InputFrame {
        let frame = InputFrame {
            held: self.held,
            pressed: self.pending,
        };
        self.pending = Buttons::NONE;
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_values_are_fixed() {
        assert_eq!(Buttons::UP.0, 1);
        assert_eq!(Buttons::DOWN.0, 2);
        assert_eq!(Buttons::LEFT.0, 4);
        assert_eq!(Buttons::RIGHT.0, 8);
        assert_eq!(Buttons::JUMP.0, 16);
        assert_eq!(Buttons::POGO.0, 32);
        assert_eq!(Buttons::FIRE.0, 64);
    }

    #[test]
    fn helpers_combine_and_test_bits() {
        let both = Buttons::UP | Buttons::JUMP;
        assert!(both.contains(Buttons::UP));
        assert!(!both.contains(Buttons::DOWN));
        assert!(!both.contains(Buttons::UP | Buttons::DOWN));
        assert_eq!(both.without(Buttons::UP), Buttons::JUMP);
        assert_eq!(both & Buttons::JUMP, Buttons::JUMP);
        assert!(Buttons::NONE.is_empty());
        assert!(!both.is_empty());
    }

    #[test]
    fn the_three_action_buttons_are_distinct_and_combine() {
        let all = Buttons::JUMP | Buttons::POGO | Buttons::FIRE;
        assert_eq!(all.0, 16 | 32 | 64);
        for b in [Buttons::JUMP, Buttons::POGO, Buttons::FIRE] {
            assert!(all.contains(b));
            assert!(!all.without(b).contains(b));
            assert_eq!(
                b & (Buttons::UP | Buttons::DOWN | Buttons::LEFT | Buttons::RIGHT),
                Buttons::NONE
            );
        }
        assert_eq!(Buttons::JUMP & Buttons::POGO, Buttons::NONE);
    }

    #[test]
    fn the_new_buttons_latch_independently() {
        let mut latch = InputLatch::new();
        latch.set(Buttons::POGO, true);
        latch.set(Buttons::FIRE, true);
        latch.set(Buttons::FIRE, false);
        let f = latch.frame();
        assert_eq!(f.held, Buttons::POGO);
        assert_eq!(f.pressed, Buttons::POGO | Buttons::FIRE);
    }

    #[test]
    fn from_transition_marks_only_new_bits() {
        let prev = Buttons::RIGHT;
        let now = Buttons::RIGHT | Buttons::JUMP;
        let f = InputFrame::from_transition(prev, now);
        assert_eq!(f.held, now);
        assert_eq!(f.pressed, Buttons::JUMP);
        let released = InputFrame::from_transition(now, Buttons::NONE);
        assert_eq!(released, InputFrame::default());
    }

    #[test]
    fn a_press_and_release_inside_one_frame_still_reports_pressed() {
        let mut latch = InputLatch::new();
        latch.set(Buttons::JUMP, true);
        latch.set(Buttons::JUMP, false);
        let f = latch.frame();
        assert_eq!(f.held, Buttons::NONE);
        assert_eq!(f.pressed, Buttons::JUMP);
        assert_eq!(latch.frame(), InputFrame::default());
    }

    #[test]
    fn a_held_button_is_pressed_only_once() {
        let mut latch = InputLatch::new();
        latch.set(Buttons::RIGHT, true);
        let first = latch.frame();
        assert_eq!(
            (first.held, first.pressed),
            (Buttons::RIGHT, Buttons::RIGHT)
        );
        // Key auto-repeat sends further down events while the key stays down.
        latch.set(Buttons::RIGHT, true);
        let second = latch.frame();
        assert_eq!(
            (second.held, second.pressed),
            (Buttons::RIGHT, Buttons::NONE)
        );
    }

    #[test]
    fn simultaneous_buttons_are_tracked_independently() {
        let mut latch = InputLatch::new();
        latch.set(Buttons::LEFT | Buttons::UP, true);
        latch.set(Buttons::JUMP, true);
        latch.set(Buttons::JUMP, false);
        let f = latch.frame();
        assert_eq!(f.held, Buttons::LEFT | Buttons::UP);
        assert_eq!(f.pressed, Buttons::LEFT | Buttons::UP | Buttons::JUMP);
        latch.set(Buttons::UP, false);
        let g = latch.frame();
        assert_eq!(g.held, Buttons::LEFT);
        assert_eq!(g.pressed, Buttons::NONE);
    }

    #[test]
    fn a_second_tap_after_release_is_a_new_press() {
        let mut latch = InputLatch::new();
        latch.set(Buttons::JUMP, true);
        latch.frame();
        latch.set(Buttons::JUMP, false);
        latch.frame();
        latch.set(Buttons::JUMP, true);
        assert_eq!(latch.frame().pressed, Buttons::JUMP);
    }
}
