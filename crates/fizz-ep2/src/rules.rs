// The rule table: what a level's baked rule records mean to the sim. This package builds the touch and enter inputs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A level's rules, in the form the sim runs them (`docs/RULES.md`). [`Rule`] is the Rust form of a `fizz_pack::RuleRec`: one
//! input, up to two conditions (an AND) and up to two actions, run in slot order, and whether the row may fire again. [`RuleTable`]
//! is the level's list of them in rule order, which is the order the rule phase (in `World::step`, after the shots) evaluates them.
//!
//! There are no switch channels and no lock of its own: a rule writes a state and the door bindings follow (see `doors.rs`). A lock
//! is a plain touch rule over its door's whole rectangle, "while the door's state is in its closed set and Ben holds the key, set
//! the state to an open value and spend the key", and runs in the rule phase like any other row ([`Rule::lock_state`] recognises
//! the shape, which `World::lock_open` uses).
//!
//! Built so far: the `touch` input over a rectangle (Ben's front probe or his body, sampled in his contacts), the `enter` input
//! (an edge on Ben's body coming into a rectangle, sampled in the rule phase); the conditions "state is in a set", "holds a key"
//! and "does not hold a key"; the actions `set`, `toggle`, `next` and `spend`; and `repeat`. Every other input fails with
//! [`RuleError::Unbuilt`], every other condition with [`RuleError::UnbuiltCondition`], every other action with
//! [`RuleError::UnbuiltAction`], and a delay with [`RuleError::Delay`], until the phase or field that runs it exists.
//!
//! The table is read once, when a world is built; stepping it allocates nothing.

use crate::doors::Doors;
use alloc::vec::Vec;
use fizz_pack::level::{
    COND_AMMO_AT_LEAST, COND_CLEARED, COND_HOLDS, COND_NOT_HOLDS, COND_SCORE_AT_LEAST,
    COND_STATE_IN, FLAG_REPEAT, MAX_RULES, OP_AMMO, OP_CUE, OP_GIVE_KEY, OP_NEXT, OP_SCORE, OP_SET,
    OP_SPEND_KEY, OP_TOGGLE, OP_WORD, SUBJECT_ANY, TRIGGER_BECOMES, TRIGGER_ENTER, TRIGGER_START,
    TRIGGER_TOUCH,
};
use fizz_pack::{ActionRec, CondRec, RuleRec};

/// Key colour numbers, Fizz 1's key order. They are the key of a `holds` condition and a `spend` action and the `colour` of
/// `Event::Key`, and are listed in `tiles::pack_content()`.
pub const KEY_RED: u8 = 0;
pub const KEY_BLUE: u8 = 1;
pub const KEY_GREEN: u8 = 2;

/// The meaning of the rule records, in `tiles::pack_content()`. The record layout is `fizz-pack`'s and is in the compatibility number
/// already; this byte is bumped when the same bytes come to mean something else, so packs baked under the old meaning are refused.
/// 1: the lock as its own trigger on a switch channel. 2: the rules of `docs/RULES.md` with the states and door bindings of `LEVL`
/// version 2, where a lock is a touch rule over a door's rectangle that sets a state and spends a key, run in the rule phase.
pub const RULES_SEMANTICS: u8 = 2;

/// A rectangle of cells `(x0, y0, x1, y1)`, both corners included, row 0 at the bottom.
pub type Rect = (u16, u16, u16, u16);

/// Why a rule record was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleError {
    /// A value that is not a trigger at all (0, the retired lock trigger 1, or above 15).
    Trigger,
    /// A trigger the sim has no phase for yet (everything but `touch` over a rectangle and `enter`); the number is the trigger.
    Unbuilt(u8),
    /// A condition kind that is numbered but not built (ammo at least, score at least, group cleared); the number is the kind.
    UnbuiltCondition(u8),
    /// An action that is numbered but not built (`give`, `score`, `ammo`, `word`, `cue`); the number is the op.
    UnbuiltAction(u8),
    /// A rule with a delay, which no phase runs yet.
    Delay,
    /// A record whose fields do not fit its trigger, conditions or actions: a flag other than repeat, a subject or arg where the
    /// input takes none, an inverted rectangle or one outside the map, an unknown condition kind or op, or a field that must be
    /// zero and is not.
    Shape,
    /// A condition or action that names a state the level does not declare, a value or a set the state does not have, or a toggle
    /// on a state that does not have exactly two values.
    State,
    /// A key colour the world has no key and flag for (only red and blue are built).
    KeyColour,
    /// More than 32 rules.
    TooMany,
}

/// What a rule reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// Ben's front probe lands in the rectangle or his body overlaps it, this tick (a level, not an edge).
    Touch(Rect),
    /// Ben's body comes into the rectangle: not inside last tick, inside this tick.
    Enter(Rect),
}

/// A test that must hold for a rule to fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// The state's value is in the set (bit `v` for value `v`).
    StateIn { state: u8, set: u8 },
    /// Ben holds the key of the colour.
    Holds(u8),
    /// Ben does not hold the key of the colour.
    NotHolds(u8),
}

/// What a rule does when it fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// Set a state to a value.
    Set { state: u8, value: u8 },
    /// Flip a two-valued state.
    Toggle(u8),
    /// Advance a state to its next value, wrapping.
    Next(u8),
    /// Take the key of the colour from Ben (nothing if he does not hold it).
    Spend(u8),
}

/// One rule in the form the sim runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub input: Input,
    /// Read before any action of the rule runs; the rule fires only when every present condition holds.
    pub cond: [Option<Cond>; 2],
    /// Run in slot order; each takes effect at once.
    pub act: [Option<Act>; 2],
    /// Without it the row fires once per life; with it, on every re-entry (`enter`) or every tick of a contact (`touch`).
    pub repeat: bool,
}

impl Rule {
    /// Reads a record for a level whose states and door bindings are `doors`.
    pub fn from_rec(r: &RuleRec, doors: &Doors) -> Result<Rule, RuleError> {
        let rect = r.rect();
        let input = match r.trigger {
            TRIGGER_TOUCH => Input::Touch(rect),
            TRIGGER_ENTER => Input::Enter(rect),
            TRIGGER_START..=TRIGGER_BECOMES => return Err(RuleError::Unbuilt(r.trigger)),
            _ => return Err(RuleError::Trigger),
        };
        if r.delay != 0 {
            return Err(RuleError::Delay);
        }
        let (w, h) = doors.size();
        if r.flags & !FLAG_REPEAT != 0
            || r.subject != SUBJECT_ANY
            || r.arg != 0
            || rect.0 > rect.2
            || rect.1 > rect.3
            || rect.2 >= w
            || rect.3 >= h
        {
            return Err(RuleError::Shape);
        }
        let cond = [cond(&r.cond[0], doors)?, cond(&r.cond[1], doors)?];
        let act = [act(&r.act[0], doors)?, act(&r.act[1], doors)?];
        Ok(Rule {
            input,
            cond,
            act,
            repeat: r.repeat(),
        })
    }

    /// The state this rule opens if it is exactly what `lock` lowers to on `doors` (`docs/RULES.md`, section 9): a touch over the
    /// whole rectangle of the state's only door, without repeat, with the conditions "the state is in the door's closed set" and
    /// "holds key K" and the actions "set the state to a value in the door's active set" and "spend key K". `None` otherwise.
    pub fn lock_state(&self, doors: &Doors) -> Option<u8> {
        let Input::Touch(rect) = self.input else {
            return None;
        };
        let [Some(Cond::StateIn { state, set }), Some(Cond::Holds(key))] = self.cond else {
            return None;
        };
        let [Some(Act::Set { state: s, value }), Some(Act::Spend(k))] = self.act else {
            return None;
        };
        let door = doors.door_of(state)?;
        let lock = !self.repeat
            && s == state
            && k == key
            && rect == door.rect
            && set == doors.closed_set(door)
            && door.mask & (1 << value) != 0;
        lock.then_some(state)
    }
}

/// The key colours the world can hold.
fn key(colour: u8) -> Result<u8, RuleError> {
    if colour <= KEY_BLUE {
        Ok(colour)
    } else {
        Err(RuleError::KeyColour)
    }
}

fn cond(c: &CondRec, doors: &Doors) -> Result<Option<Cond>, RuleError> {
    Ok(Some(match c.kind {
        0 if c.is_empty() => return Ok(None),
        COND_STATE_IN => {
            let def = doors
                .states()
                .get(usize::from(c.a))
                .ok_or(RuleError::State)?;
            if c.b == 0 || c.b & !def.all_values() != 0 {
                return Err(RuleError::State);
            }
            Cond::StateIn {
                state: c.a,
                set: c.b,
            }
        }
        COND_HOLDS if c.b == 0 => Cond::Holds(key(c.a)?),
        COND_NOT_HOLDS if c.b == 0 => Cond::NotHolds(key(c.a)?),
        COND_AMMO_AT_LEAST | COND_SCORE_AT_LEAST | COND_CLEARED => {
            return Err(RuleError::UnbuiltCondition(c.kind));
        }
        _ => return Err(RuleError::Shape),
    }))
}

fn act(a: &ActionRec, doors: &Doors) -> Result<Option<Act>, RuleError> {
    if a.is_empty() {
        return Ok(None);
    }
    if matches!(a.op, OP_GIVE_KEY | OP_SCORE | OP_AMMO | OP_WORD | OP_CUE) {
        return Err(RuleError::UnbuiltAction(a.op));
    }
    // The built ops place no event and carry no amount; `set` alone has a value.
    if a.n != 0 || a.x != 0 || a.y != 0 || (a.op != OP_SET && a.b != 0) {
        return Err(RuleError::Shape);
    }
    let state = || doors.states().get(usize::from(a.a)).ok_or(RuleError::State);
    Ok(Some(match a.op {
        OP_SET => {
            if a.b >= state()?.values {
                return Err(RuleError::State);
            }
            Act::Set {
                state: a.a,
                value: a.b,
            }
        }
        OP_TOGGLE => {
            if state()?.values != 2 {
                return Err(RuleError::State);
            }
            Act::Toggle(a.a)
        }
        OP_NEXT => {
            state()?;
            Act::Next(a.a)
        }
        OP_SPEND_KEY => Act::Spend(key(a.a)?),
        _ => return Err(RuleError::Shape),
    }))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::{SLICE_BINDINGS, SLICE_RULES, SLICE_STATES};
    use crate::tiles;
    use fizz_pack::level::{
        ARG_ANY, OP_GIVE_KEY, TRIGGER_SHOOT, TRIGGER_STOMP, TRIGGER_STUN, TRIGGER_TAKE,
        TRIGGER_TIME,
    };
    use fizz_pack::{BindingRec, StateRec};

    /// The slice's two doors, derived from its tiles: bits 0 and 1.
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

    /// The slice's doors with a third state of `values` values that no door binds.
    fn doors_with(values: u8) -> Doors {
        let states = [SLICE_STATES[0], SLICE_STATES[1], StateRec::new(values, 0)];
        Doors::from_recs(
            &states,
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

    fn enter(act0: ActionRec) -> RuleRec {
        let (u0, u1) = RuleRec::union_rect(3, 4, 5, 6);
        RuleRec {
            trigger: TRIGGER_ENTER,
            u0,
            u1,
            act: [act0, ActionRec::EMPTY],
            ..RuleRec::EMPTY
        }
    }

    fn op(op: u8, a: u8) -> ActionRec {
        ActionRec {
            op,
            a,
            ..ActionRec::EMPTY
        }
    }

    #[test]
    fn a_lock_record_reads_to_a_plain_touch_rule() {
        let d = doors();
        assert_eq!(
            Rule::from_rec(&red(), &d),
            Ok(Rule {
                input: Input::Touch((15, 20, 15, 22)),
                cond: [
                    Some(Cond::StateIn {
                        state: 0,
                        set: 0b01
                    }),
                    Some(Cond::Holds(KEY_RED))
                ],
                act: [
                    Some(Act::Set { state: 0, value: 1 }),
                    Some(Act::Spend(KEY_RED))
                ],
                repeat: false,
            })
        );
        let t = RuleTable::from_recs(&SLICE_RULES, &d).unwrap();
        assert_eq!(t.rules()[0].lock_state(&d), Some(0));
        assert_eq!(t.rules()[1].lock_state(&d), Some(1));
    }

    #[test]
    fn the_inputs_without_a_phase_are_refused_by_number() {
        let d = doors();
        for trigger in [
            TRIGGER_START,
            TRIGGER_TAKE,
            TRIGGER_SHOOT,
            TRIGGER_STOMP,
            TRIGGER_STUN,
            TRIGGER_TIME,
            10,
            11,
            12,
            13,
            14,
            TRIGGER_BECOMES,
        ] {
            let r = RuleRec { trigger, ..red() };
            assert_eq!(Rule::from_rec(&r, &d), Err(RuleError::Unbuilt(trigger)));
        }
        for trigger in [0, 1, 16, 255] {
            let r = RuleRec { trigger, ..red() };
            assert_eq!(Rule::from_rec(&r, &d), Err(RuleError::Trigger), "{trigger}");
        }
        assert_eq!(
            Rule::from_rec(&RuleRec { delay: 1, ..red() }, &d),
            Err(RuleError::Delay)
        );
    }

    #[test]
    fn the_unbuilt_conditions_and_actions_are_refused_by_number() {
        let d = doors();
        for kind in [4, 5, 6] {
            let r = RuleRec {
                cond: [CondRec { kind, a: 0, b: 1 }, CondRec::EMPTY],
                ..red()
            };
            assert_eq!(
                Rule::from_rec(&r, &d),
                Err(RuleError::UnbuiltCondition(kind))
            );
        }
        for o in [OP_GIVE_KEY, OP_SCORE, OP_AMMO, OP_WORD, OP_CUE] {
            assert_eq!(
                Rule::from_rec(&enter(op(o, 0)), &d),
                Err(RuleError::UnbuiltAction(o))
            );
        }
    }

    #[test]
    fn an_enter_row_reads_its_rect_actions_and_repeat() {
        let d = doors_with(4);
        let r = RuleRec {
            flags: FLAG_REPEAT,
            act: [op(OP_NEXT, 2), op(OP_TOGGLE, 0)],
            cond: [
                CondRec {
                    kind: COND_NOT_HOLDS,
                    a: KEY_BLUE,
                    b: 0,
                },
                CondRec::state_in(2, 0b1010),
            ],
            ..enter(ActionRec::EMPTY)
        };
        assert_eq!(
            Rule::from_rec(&r, &d),
            Ok(Rule {
                input: Input::Enter((3, 4, 5, 6)),
                cond: [
                    Some(Cond::NotHolds(KEY_BLUE)),
                    Some(Cond::StateIn {
                        state: 2,
                        set: 0b1010
                    })
                ],
                act: [Some(Act::Next(2)), Some(Act::Toggle(0))],
                repeat: true,
            })
        );
        assert_eq!(
            Rule::from_rec(&r, &d).unwrap().lock_state(&d),
            None,
            "an enter row is not a lock"
        );
    }

    #[test]
    fn a_rule_must_fit_the_states_it_names() {
        let d = doors_with(3);
        let bad = |r: RuleRec| Rule::from_rec(&r, &d);
        let state = Err(RuleError::State);
        assert_eq!(
            bad(enter(ActionRec::set(2, 3))),
            state,
            "a value out of range"
        );
        assert_eq!(bad(enter(ActionRec::set(3, 0))), state, "no such state");
        assert_eq!(
            bad(enter(op(OP_TOGGLE, 2))),
            state,
            "toggle on three values"
        );
        assert_eq!(bad(enter(op(OP_NEXT, 9))), state);
        assert_eq!(bad(enter(ActionRec::set(2, 2))).map(|_| ()), Ok(()));
        let with_cond = |c| RuleRec {
            cond: [c, CondRec::EMPTY],
            ..enter(op(OP_NEXT, 2))
        };
        assert_eq!(
            bad(with_cond(CondRec::state_in(2, 0))),
            state,
            "an empty set"
        );
        assert_eq!(bad(with_cond(CondRec::state_in(2, 0b1000))), state);
        assert_eq!(bad(with_cond(CondRec::state_in(7, 1))), state);
        assert_eq!(
            bad(with_cond(CondRec::holds(KEY_GREEN))),
            Err(RuleError::KeyColour)
        );
        assert_eq!(
            bad(enter(ActionRec::spend_key(KEY_GREEN))),
            Err(RuleError::KeyColour)
        );
    }

    #[test]
    fn a_rule_must_have_the_shape_of_its_fields() {
        let d = doors();
        let shape = Err(RuleError::Shape);
        let bad = |r: RuleRec| Rule::from_rec(&r, &d);
        assert_eq!(bad(RuleRec { flags: 2, ..red() }), shape);
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
        for rect in [(5, 1, 4, 1), (1, 5, 1, 4), (0, 0, 32, 0), (0, 0, 0, 48)] {
            let (u0, u1) = RuleRec::union_rect(rect.0, rect.1, rect.2, rect.3);
            assert_eq!(bad(RuleRec { u0, u1, ..red() }), shape, "{rect:?}");
        }
        let cond = |c| RuleRec {
            cond: [c, CondRec::EMPTY],
            ..red()
        };
        assert_eq!(
            bad(cond(CondRec {
                kind: 0,
                a: 1,
                b: 0
            })),
            shape
        );
        assert_eq!(
            bad(cond(CondRec {
                kind: 7,
                a: 0,
                b: 0
            })),
            shape
        );
        assert_eq!(
            bad(cond(CondRec {
                kind: COND_HOLDS,
                a: 0,
                b: 1
            })),
            shape
        );
        assert_eq!(bad(enter(op(10, 0))), shape);
        assert_eq!(bad(enter(op(0, 1))), shape);
        assert_eq!(
            bad(enter(ActionRec {
                x: 1,
                ..ActionRec::set(0, 1)
            })),
            shape,
            "a set places no event"
        );
        assert_eq!(
            bad(enter(ActionRec {
                b: 1,
                ..op(OP_NEXT, 0)
            })),
            shape
        );
    }

    #[test]
    fn a_touch_rule_that_differs_from_the_lock_sugar_is_a_rule_and_not_a_lock() {
        let d = doors();
        let not_lock = |r: RuleRec| {
            let rule = Rule::from_rec(&r, &d).unwrap();
            assert_eq!(rule.lock_state(&d), None, "{r:?}");
        };
        not_lock(RuleRec {
            flags: FLAG_REPEAT,
            ..red()
        });
        // The blue door's rectangle, a part of the red door, the open set, an unopened value, the keys and slots swapped.
        not_lock(RuleRec {
            u0: SLICE_RULES[1].u0,
            u1: SLICE_RULES[1].u1,
            ..red()
        });
        let (u0, u1) = RuleRec::union_rect(15, 21, 15, 21);
        not_lock(RuleRec { u0, u1, ..red() });
        not_lock(RuleRec {
            cond: [CondRec::state_in(0, 0b10), red().cond[1]],
            ..red()
        });
        not_lock(RuleRec {
            act: [ActionRec::set(0, 0), red().act[1]],
            ..red()
        });
        not_lock(RuleRec {
            act: [red().act[0], ActionRec::spend_key(KEY_BLUE)],
            ..red()
        });
        not_lock(RuleRec {
            cond: [red().cond[1], red().cond[0]],
            ..red()
        });
        // A state with two doors has no lock.
        let two = [
            BindingRec::door(0, 0b10, (15, 20, 15, 22)),
            BindingRec::door(0, 0b10, (22, 38, 22, 40)),
        ];
        let d2 = Doors::from_recs(
            &SLICE_STATES,
            &two,
            &crate::level::SLICE.tiles,
            32,
            48,
            &tiles::props(),
        )
        .unwrap();
        assert_eq!(Rule::from_rec(&red(), &d2).unwrap().lock_state(&d2), None);
    }

    #[test]
    fn a_table_keeps_record_order_and_holds_32_rules() {
        let d = doors();
        let t = RuleTable::from_recs(&SLICE_RULES, &d).unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t.rules()[0].input, Input::Touch((15, 20, 15, 22)));
        assert_eq!(t.rules()[1].input, Input::Touch((22, 38, 22, 40)));
        assert_eq!(
            RuleTable::from_recs(&[red(); 32], &d).map(|t| t.len()),
            Ok(32)
        );
        assert_eq!(
            RuleTable::from_recs(&[red(); 33], &d),
            Err(RuleError::TooMany)
        );
    }
}
