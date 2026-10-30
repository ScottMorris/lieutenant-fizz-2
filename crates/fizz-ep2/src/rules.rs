// The rule table: what a level's baked rule records mean to the sim. This package builds the lock phase only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A level's rules, in the form the sim runs them. [`Rule`] is the Rust form of a `fizz_pack::RuleRec` and
//! [`RuleTable`] the level's list of them in rule order, which is the order the sim evaluates them.
//!
//! Only the **lock** trigger (1) is built so far. A lock says "a key of this colour, spent on a closed door whose tile reads
//! this switch channel, opens the door": the sim's lock phase (in `World::step`, inside Ben's contacts) probes the tile in
//! front of Ben and, when it is a switched tile on the lock's channel, the channel is on and Ben holds the key, switches the
//! channel off, spends the key and emits `Event::DoorOpen`. The other triggers (start, enter, take, shoot, stomp, stun and time)
//! are refused with [`RuleError::Trigger`] until the rules phase that runs them exists.
//!
//! The table is read once, when a world is built; stepping it allocates nothing.

use alloc::vec::Vec;
use fizz_pack::RuleRec;
use fizz_pack::level::{MAX_RULES, OP_OFF, TRIGGER_LOCK};

/// Key colour numbers, Fizz 1's key order. They are the `subject` of a lock rule and the `colour` of `Event::Key`, and are listed
/// in `tiles::pack_content()`.
pub const KEY_RED: u8 = 0;
pub const KEY_BLUE: u8 = 1;
pub const KEY_GREEN: u8 = 2;

/// The meaning of the rule records, in `tiles::pack_content()`. The record layout is `fizz-pack`'s and is in the compatibility number
/// already; this byte is bumped when the same bytes come to mean something else (a changed phase order, say), so packs baked under the old
/// meaning are refused. 1: the lock phase described above.
pub const RULES_SEMANTICS: u8 = 1;

/// Why a rule record was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleError {
    /// A trigger the sim cannot run yet (2 to 8), or one that is not a trigger at all.
    Trigger,
    /// A lock whose flags, delay, guard, union or slot B are not empty, or whose op is not off.
    LockShape,
    /// A lock for a key colour the world has no key and flag for (only red and blue are built).
    LockColour,
    /// A lock whose channel is 32 or more.
    LockChannel,
    /// A lock whose event cell is outside the map.
    LockCell,
    /// More than 32 rules.
    TooMany,
}

/// A lock: `colour` is the key it takes, `channel` the switch channel of the door tiles it opens, and `cell` where the open is
/// reported (the door's cell).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lock {
    pub colour: u8,
    pub channel: u8,
    pub cell: (u16, u16),
}

/// One rule in the form the sim runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// Trigger 1: a key spent on a closed door.
    Lock(Lock),
}

impl Rule {
    /// A lock for key `colour` on `channel`, reporting at `cell`.
    pub const fn lock(colour: u8, channel: u8, cell: (u16, u16)) -> Rule {
        Rule::Lock(Lock {
            colour,
            channel,
            cell,
        })
    }

    /// Reads a record for a `w` by `h` map. Refuses triggers other than the lock and a lock that breaks the lock shape
    /// (`docs/FAP.md`: op off, no delay, no guard, no repeat, no cue, an empty union, a known key colour, a channel below 32 and
    /// a cell inside the map).
    pub fn from_rec(r: &RuleRec, w: u16, h: u16) -> Result<Rule, RuleError> {
        if r.trigger != TRIGGER_LOCK {
            return Err(RuleError::Trigger);
        }
        if r.flags != 0
            || r.guard_ch != 0
            || r.u0 != 0
            || r.u1 != 0
            || r.delay != 0
            || r.op != OP_OFF
            || r.has_cue()
            || r.cell_b != (0, 0)
        {
            return Err(RuleError::LockShape);
        }
        if r.subject > KEY_BLUE {
            return Err(RuleError::LockColour);
        }
        if r.channel >= 32 {
            return Err(RuleError::LockChannel);
        }
        if r.cell_a.0 >= w || r.cell_a.1 >= h {
            return Err(RuleError::LockCell);
        }
        Ok(Rule::lock(r.subject, r.channel, r.cell_a))
    }

    /// The record this rule bakes to.
    pub fn to_rec(&self) -> RuleRec {
        match *self {
            Rule::Lock(l) => RuleRec {
                trigger: TRIGGER_LOCK,
                subject: l.colour,
                op: OP_OFF,
                channel: l.channel,
                cell_a: l.cell,
                ..RuleRec::EMPTY
            },
        }
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

    /// The table for `recs` on a `w` by `h` map, in record order.
    pub fn from_recs(recs: &[RuleRec], w: u16, h: u16) -> Result<RuleTable, RuleError> {
        if recs.len() > MAX_RULES {
            return Err(RuleError::TooMany);
        }
        let mut rules = Vec::with_capacity(recs.len());
        for r in recs {
            rules.push(Rule::from_rec(r, w, h)?);
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

    /// The channel of the first lock for key `colour`, which is the door's channel for a presenter that wants to know whether
    /// that door is open.
    pub fn lock_channel(&self, colour: u8) -> Option<u8> {
        (0..self.rules.len())
            .filter_map(|i| self.lock_at(i))
            .find(|l| l.colour == colour)
            .map(|l| l.channel)
    }

    /// The records of the table, in order.
    pub fn to_recs(&self) -> Vec<RuleRec> {
        self.rules.iter().map(Rule::to_rec).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fizz_pack::level::{
        CUE_NONE, FLAG_GUARD, TRIGGER_ENTER, TRIGGER_START, TRIGGER_TAKE, TRIGGER_TIME,
    };

    fn lock() -> RuleRec {
        Rule::lock(KEY_BLUE, 2, (22, 39)).to_rec()
    }

    #[test]
    fn a_lock_record_round_trips() {
        let r = lock();
        assert_eq!(r.trigger, TRIGGER_LOCK);
        assert_eq!(
            (r.subject, r.op, r.channel, r.cell_a),
            (1, OP_OFF, 2, (22, 39))
        );
        assert_eq!((r.effect, r.caption), (CUE_NONE, CUE_NONE));
        assert_eq!(Rule::from_rec(&r, 32, 48), Ok(Rule::lock(1, 2, (22, 39))));
    }

    #[test]
    fn the_other_triggers_are_refused_for_now() {
        for trigger in [
            0,
            TRIGGER_START,
            TRIGGER_ENTER,
            TRIGGER_TAKE,
            TRIGGER_TIME,
            9,
        ] {
            let r = RuleRec { trigger, ..lock() };
            assert_eq!(Rule::from_rec(&r, 32, 48), Err(RuleError::Trigger));
        }
    }

    #[test]
    fn a_lock_must_have_the_lock_shape() {
        let bad = |r: RuleRec| Rule::from_rec(&r, 32, 48);
        assert_eq!(bad(RuleRec { op: 1, ..lock() }), Err(RuleError::LockShape));
        assert_eq!(
            bad(RuleRec { delay: 1, ..lock() }),
            Err(RuleError::LockShape)
        );
        assert_eq!(
            bad(RuleRec {
                flags: FLAG_GUARD,
                ..lock()
            }),
            Err(RuleError::LockShape)
        );
        assert_eq!(
            bad(RuleRec {
                effect: 3,
                ..lock()
            }),
            Err(RuleError::LockShape)
        );
        assert_eq!(bad(RuleRec { u0: 1, ..lock() }), Err(RuleError::LockShape));
        assert_eq!(
            bad(RuleRec {
                subject: 2,
                ..lock()
            }),
            Err(RuleError::LockColour)
        );
        assert_eq!(
            bad(RuleRec {
                channel: 32,
                ..lock()
            }),
            Err(RuleError::LockChannel)
        );
        assert_eq!(
            bad(RuleRec {
                cell_a: (32, 0),
                ..lock()
            }),
            Err(RuleError::LockCell)
        );
    }

    #[test]
    fn a_table_keeps_record_order_and_finds_a_lock_by_colour() {
        let recs = [Rule::lock(0, 1, (15, 21)).to_rec(), lock()];
        let t = RuleTable::from_recs(&recs, 32, 48).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.to_recs(), recs);
        assert_eq!(t.lock_channel(KEY_RED), Some(1));
        assert_eq!(t.lock_channel(KEY_BLUE), Some(2));
        assert_eq!(t.lock_channel(KEY_GREEN), None);
        let many = [lock(); 33];
        assert_eq!(RuleTable::from_recs(&many, 32, 48), Err(RuleError::TooMany));
    }
}
