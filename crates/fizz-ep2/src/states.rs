// The state table: the current value of each of a level's states, which only rule actions and the death reset change.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A level's states as the world runs them (`docs/RULES.md`, section 2): up to 32 states, each a small integer with 2 to 8 values
//! and a start value. The definitions are the level's `StateRec`s, checked when the doors are built (see [`crate::doors::Doors`]);
//! this table holds the current values. A state is changed only by rule actions ([`StateTable::set`], [`StateTable::toggle`] and
//! [`StateTable::next`]) and by the death reset ([`StateTable::reset`]). There are no switch channels: what a door shows is derived
//! from a state through its binding, and the table knows nothing of doors.
//!
//! Arithmetic is closed: the loader refuses a rule that sets a value out of range or toggles a state that does not have exactly
//! two values, so the operations here never clamp.

use alloc::vec::Vec;
use fizz_pack::StateRec;

/// The definitions and current values of a level's states.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateTable {
    defs: Vec<StateRec>,
    cur: Vec<u8>,
}

impl StateTable {
    /// A table with no states.
    pub fn none() -> StateTable {
        StateTable::default()
    }

    /// A table over `defs`, which the doors have checked (2 to 8 values, a start below them), each state at its start value.
    pub(crate) fn new(defs: &[StateRec]) -> StateTable {
        StateTable {
            defs: defs.to_vec(),
            cur: defs.iter().map(|d| d.start).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// The definition of `state`.
    pub fn def(&self, state: u8) -> Option<StateRec> {
        self.defs.get(usize::from(state)).copied()
    }

    /// The current value of `state`, or `None` for a state the table lacks.
    pub fn value(&self, state: u8) -> Option<u8> {
        self.cur.get(usize::from(state)).copied()
    }

    /// The current values in table order.
    pub fn values(&self) -> &[u8] {
        &self.cur
    }

    /// Sets `state` to `value` (the loader has checked it is below the value count). True if the value changed; a set that leaves
    /// the value as it was is not a change.
    pub fn set(&mut self, state: u8, value: u8) -> bool {
        let Some(slot) = self.cur.get_mut(usize::from(state)) else {
            return false;
        };
        let changed = *slot != value;
        *slot = value;
        changed
    }

    /// Flips a two-valued state. True (it always changes).
    pub fn toggle(&mut self, state: u8) -> bool {
        match self.value(state) {
            Some(v) => self.set(state, 1 - v),
            None => false,
        }
    }

    /// Advances `state` to its next value, wrapping from the last to 0. True (it always changes, since a state has 2 values or more).
    pub fn next(&mut self, state: u8) -> bool {
        match (self.def(state), self.value(state)) {
            (Some(d), Some(v)) => self.set(state, (v + 1) % d.values),
            _ => false,
        }
    }

    /// The death reset: every state back to its start value.
    pub fn reset(&mut self) {
        for (c, d) in self.cur.iter_mut().zip(&self.defs) {
            *c = d.start;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_starts_every_state_at_its_start_value() {
        let t = StateTable::new(&[StateRec::new(3, 2), StateRec::new(8, 0)]);
        assert_eq!(t.values(), &[2, 0]);
        assert_eq!(t.len(), 2);
        assert_eq!(t.value(2), None);
        assert!(StateTable::none().is_empty());
    }

    #[test]
    fn next_wraps_set_reports_change_toggle_flips_and_reset_restores_the_start() {
        let mut t = StateTable::new(&[StateRec::new(3, 1), StateRec::new(2, 0)]);
        assert!(t.next(0));
        assert!(t.next(0));
        assert_eq!(t.value(0), Some(0), "2 wraps to 0");
        assert!(!t.set(0, 0), "setting the same value is not a change");
        assert!(t.set(0, 2));
        assert!(t.toggle(1));
        assert_eq!(t.value(1), Some(1));
        assert!(t.toggle(1));
        assert_eq!(t.value(1), Some(0));
        assert!(
            !t.set(5, 1) && !t.next(5) && !t.toggle(5),
            "a missing state changes nothing"
        );
        t.set(1, 1);
        t.reset();
        assert_eq!(t.values(), &[1, 0]);
    }
}
