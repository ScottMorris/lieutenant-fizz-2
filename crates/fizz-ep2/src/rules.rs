// The rule table: what a level's baked rule records mean to the sim. This package builds the lock phase only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A level's rules, in the form the sim runs them. [`Rule`] is the Rust form of a `fizz_pack::RuleRec` and
//! [`RuleTable`] the level's list of them in rule order, which is the order the sim evaluates them.
//!
//! Only the **lock** is built so far. A lock is a plain touch rule (trigger 9) over the whole rectangle of a door: while Ben
//! touches it, the door's state is in the closed set and he holds the key, set the state to an open value and spend the key
//! (`docs/FAP.md`, the worked example). The sim's lock phase (in `World::step`, inside Ben's contacts) probes the tile in front of
//! Ben and, when it lies in the lock's door, the door is closed and Ben holds the key, clears the door's derived bit (see `doors.rs`),
//! spends the key and emits `Event::DoorOpen`. Any other rule is refused with [`RuleError::Trigger`] until the rule phase that runs
//! them exists, and a touch rule that is not the lock's shape with [`RuleError::LockShape`].
//!
//! The table is read once, when a world is built; stepping it allocates nothing.

use crate::doors::Doors;
use alloc::vec::Vec;
use fizz_pack::RuleRec;
use fizz_pack::level::{
    COND_HOLDS, COND_STATE_IN, MAX_RULES, OP_SET, OP_SPEND_KEY, SUBJECT_ANY, TRIGGER_TOUCH,
};

/// Key colour numbers, Fizz 1's key order. They are the key of a lock rule and the `colour` of `Event::Key`, and are listed
/// in `tiles::pack_content()`.
pub const KEY_RED: u8 = 0;
pub const KEY_BLUE: u8 = 1;
pub const KEY_GREEN: u8 = 2;

/// The meaning of the rule records, in `tiles::pack_content()`. The record layout is `fizz-pack`'s and is in the compatibility number
/// already; this byte is bumped when the same bytes come to mean something else (a changed phase order, say), so packs baked under the old
/// meaning are refused. 1: the lock as its own trigger on a switch channel. 2: the lock as a touch rule over a door's rectangle that
/// sets a state and spends a key, with the states and door bindings of `LEVL` version 2.
pub const RULES_SEMANTICS: u8 = 2;

/// Why a rule record was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleError {
    /// A trigger the sim cannot run yet (anything but the touch rule a lock is made of).
    Trigger,
    /// A touch rule that is not a lock: it must have no flag, delay, subject or arg, the conditions "state is in a set" and
    /// "holds", the actions "set" and "spend", the same state in the condition and the action, the same key in the condition and
    /// the action, the door's whole rectangle, and a closed set and an open value that agree with the door.
    LockShape,
    /// A lock for a key colour the world has no key and flag for (only red and blue are built).
    LockColour,
    /// A lock whose state has no door binding, or more than one.
    LockState,
    /// More than 32 rules.
    TooMany,
}

/// A lock: `colour` is the key it takes, `bit` the derived switch bit of the door it opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lock {
    pub colour: u8,
    pub bit: u8,
}

/// One rule in the form the sim runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// A key spent on a closed door.
    Lock(Lock),
}

impl Rule {
    /// A lock for key `colour` on the door whose derived bit is `bit`.
    pub const fn lock(colour: u8, bit: u8) -> Rule {
        Rule::Lock(Lock { colour, bit })
    }

    /// Reads a record for a level whose states and door bindings are `doors`. Refuses every rule but the lock, and a touch rule
    /// that is not a well-formed lock (see [`RuleError::LockShape`]).
    pub fn from_rec(r: &RuleRec, doors: &Doors) -> Result<Rule, RuleError> {
        if r.trigger != TRIGGER_TOUCH {
            return Err(RuleError::Trigger);
        }
        let [state_in, holds] = r.cond;
        let [set, spend] = r.act;
        if r.flags != 0
            || r.delay != 0
            || r.subject != SUBJECT_ANY
            || r.arg != 0
            || state_in.kind != COND_STATE_IN
            || holds.kind != COND_HOLDS
            || set.op != OP_SET
            || spend.op != OP_SPEND_KEY
            || state_in.a != set.a
            || holds.a != spend.a
        {
            return Err(RuleError::LockShape);
        }
        if holds.a > KEY_BLUE {
            return Err(RuleError::LockColour);
        }
        let door = doors.door_of(state_in.a).ok_or(RuleError::LockState)?;
        // The whole rectangle, the door's closed set as the condition and an open value as the action: a lock fires exactly when the
        // door is closed, and what it sets opens the door.
        if r.rect() != door.rect
            || state_in.b != doors.closed_set(door)
            || set.b >= 8
            || door.mask & (1 << set.b) == 0
        {
            return Err(RuleError::LockShape);
        }
        Ok(Rule::lock(holds.a, door.bit))
    }
}

/// A level's rules in rule order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleTable {
    rules: Vec<Rule>,
}

impl RuleTable {
    /// A table with no rules.
    pub fn new() -> RuleTable {
        RuleTable::default()
    }

    /// The table for `recs` on a level whose states and door bindings are `doors`, in record order.
    pub fn from_recs(recs: &[RuleRec], doors: &Doors) -> Result<RuleTable, RuleError> {
        if recs.len() > MAX_RULES {
            return Err(RuleError::TooMany);
        }
        let mut rules = Vec::with_capacity(recs.len());
        for r in recs {
            rules.push(Rule::from_rec(r, doors)?);
        }
        Ok(RuleTable { rules })
    }

    /// Adds a rule at the end. Panics past [`MAX_RULES`].
    pub fn push(&mut self, rule: Rule) {
        assert!(self.rules.len() < MAX_RULES, "a level has at most 32 rules");
        self.rules.push(rule);
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The rules in rule order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The `i`th rule if it is a lock.
    pub fn lock_at(&self, i: usize) -> Option<Lock> {
        match self.rules.get(i)? {
            Rule::Lock(l) => Some(*l),
        }
    }

    /// The derived bit of the first lock for key `colour`, which is the door's bit for a presenter that wants to know whether
    /// that door is open.
    pub fn lock_bit(&self, colour: u8) -> Option<u8> {
        (0..self.rules.len())
            .filter_map(|i| self.lock_at(i))
            .find(|l| l.colour == colour)
            .map(|l| l.bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{SLICE_BINDINGS, SLICE_RULES, SLICE_STATES};
    use crate::tiles;
    use fizz_pack::level::{
        ARG_ANY, COND_NOT_HOLDS, OP_TOGGLE, TRIGGER_ENTER, TRIGGER_START, TRIGGER_TAKE,
        TRIGGER_TIME,
    };
    use fizz_pack::{ActionRec, CondRec};

    /// The slice's two doors, derived from its tiles: bits 1 and 2, as before the states were data.
    fn doors() -> Doors {
        Doors::from_recs(
            &SLICE_STATES,
            &SLICE_BINDINGS,
            &crate::level::SLICE.tiles,
            32,
            48,
            &tiles::props(),
        )
        .unwrap()
    }

    fn red() -> RuleRec {
        SLICE_RULES[0]
    }

    fn blue() -> RuleRec {
        SLICE_RULES[1]
    }

    #[test]
    fn a_lock_record_reads_to_its_colour_and_door_bit() {
        let d = doors();
        assert_eq!(
            Rule::from_rec(&red(), &d),
            Ok(Rule::lock(KEY_RED, tiles::DOOR_RED_CHANNEL))
        );
        assert_eq!(
            Rule::from_rec(&blue(), &d),
            Ok(Rule::lock(KEY_BLUE, tiles::DOOR_BLUE_CHANNEL))
        );
    }

    #[test]
    fn the_other_rules_are_refused_for_now() {
        let d = doors();
        for trigger in [
            0,
            1,
            TRIGGER_START,
            TRIGGER_ENTER,
            TRIGGER_TAKE,
            TRIGGER_TIME,
            10,
        ] {
            let r = RuleRec { trigger, ..red() };
            assert_eq!(Rule::from_rec(&r, &d), Err(RuleError::Trigger));
        }
    }

    #[test]
    fn a_lock_must_have_the_lock_shape() {
        let d = doors();
        let bad = |r: RuleRec| Rule::from_rec(&r, &d);
        let shape = Err(RuleError::LockShape);
        assert_eq!(bad(RuleRec { delay: 1, ..red() }), shape);
        assert_eq!(bad(RuleRec { flags: 1, ..red() }), shape);
        assert_eq!(
            bad(RuleRec {
                subject: 3,
                ..red()
            }),
            shape
        );
        assert_eq!(
            bad(RuleRec {
                arg: ARG_ANY,
                ..red()
            }),
            shape
        );
        // The conditions and actions.
        let cond = |c0, c1| RuleRec {
            cond: [c0, c1],
            ..red()
        };
        assert_eq!(bad(cond(red().cond[1], red().cond[0])), shape, "swapped");
        assert_eq!(
            bad(cond(
                red().cond[0],
                CondRec {
                    kind: COND_NOT_HOLDS,
                    a: 0,
                    b: 0
                }
            )),
            shape
        );
        assert_eq!(
            bad(cond(CondRec::state_in(1, 0b01), red().cond[1])),
            shape,
            "another state than the action's"
        );
        assert_eq!(
            bad(cond(red().cond[0], CondRec::holds(1))),
            shape,
            "another key than the spend's"
        );
        assert_eq!(
            bad(cond(CondRec::state_in(0, 0b11), red().cond[1])),
            shape,
            "not the closed set"
        );
        assert_eq!(
            bad(cond(CondRec::state_in(0, 0b10), red().cond[1])),
            shape,
            "the open set"
        );
        let act = |a0, a1| RuleRec {
            act: [a0, a1],
            ..red()
        };
        assert_eq!(bad(act(red().act[1], red().act[0])), shape, "swapped");
        assert_eq!(
            bad(act(
                ActionRec {
                    op: OP_TOGGLE,
                    ..red().act[0]
                },
                red().act[1]
            )),
            shape
        );
        assert_eq!(
            bad(act(ActionRec::set(0, 0), red().act[1])),
            shape,
            "sets a closed value"
        );
        assert_eq!(bad(act(red().act[0], ActionRec::EMPTY)), shape);
        // The rectangle is the door's whole rectangle and nothing else.
        for rect in [
            (15, 21, 15, 21),
            (15, 20, 15, 21),
            (14, 20, 15, 22),
            (22, 38, 22, 40),
        ] {
            let (u0, u1) = RuleRec::union_rect(rect.0, rect.1, rect.2, rect.3);
            assert_eq!(bad(RuleRec { u0, u1, ..red() }), shape, "{rect:?}");
        }
    }

    #[test]
    fn a_lock_names_a_key_the_world_has_and_a_state_with_one_door() {
        let d = doors();
        let bad = |r: RuleRec| Rule::from_rec(&r, &d);
        let green = RuleRec {
            cond: [red().cond[0], CondRec::holds(KEY_GREEN)],
            act: [red().act[0], ActionRec::spend_key(KEY_GREEN)],
            ..red()
        };
        assert_eq!(bad(green), Err(RuleError::LockColour));
        // A state with no door, and a state with two.
        let none = RuleRec {
            cond: [CondRec::state_in(5, 0b01), red().cond[1]],
            act: [ActionRec::set(5, 1), red().act[1]],
            ..red()
        };
        assert_eq!(bad(none), Err(RuleError::LockState));
        let states = [fizz_pack::StateRec::new(2, 0)];
        let two = [
            fizz_pack::BindingRec::door(0, 0b10, (15, 20, 15, 22)),
            fizz_pack::BindingRec::door(0, 0b10, (22, 38, 22, 40)),
        ];
        let d2 = Doors::from_recs(
            &states,
            &two,
            &crate::level::SLICE.tiles,
            32,
            48,
            &tiles::props(),
        )
        .unwrap();
        assert_eq!(d2.door_of(0), None);
        assert_eq!(Rule::from_rec(&red(), &d2), Err(RuleError::LockState));
    }

    #[test]
    fn a_table_keeps_record_order_and_finds_a_lock_by_colour() {
        let d = doors();
        let t = RuleTable::from_recs(&SLICE_RULES, &d).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.lock_bit(KEY_RED), Some(tiles::DOOR_RED_CHANNEL));
        assert_eq!(t.lock_bit(KEY_BLUE), Some(tiles::DOOR_BLUE_CHANNEL));
        assert_eq!(t.lock_bit(KEY_GREEN), None);
        assert_eq!(t.lock_at(0).map(|l| l.colour), Some(KEY_RED));
        let many = [red(); 33];
        assert_eq!(RuleTable::from_recs(&many, &d), Err(RuleError::TooMany));
    }
}
