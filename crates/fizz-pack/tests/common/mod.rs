// Shared builders for the LEVL version 2 tests: an editable level and the records it holds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(dead_code)]

use fizz_pack::level::*;
use fizz_pack::write::{EncodeError, LevelSpec, encode_level};
use fizz_pack::{
    ActionRec, BindingRec, CondRec, LevelView, ObjectRec, RuleRec, StateRec, StringRec,
};

pub const W: u16 = 40;
pub const H: u16 = 12;

pub fn obj(kind: u8, sub: u8) -> ObjectRec {
    ObjectRec {
        kind,
        sub,
        flags: 0,
        aux: 0,
        x_bits: 3.5f64.to_bits(),
        y_bits: 4.5f64.to_bits(),
        p_bits: 0,
    }
}

pub fn chicken(group: u32) -> ObjectRec {
    ObjectRec {
        aux: group,
        p_bits: (-1.0f64).to_bits(),
        ..obj(KIND_CHICKEN, 0)
    }
}

pub fn switch(flags: u16) -> ObjectRec {
    ObjectRec {
        flags,
        ..obj(KIND_SWITCH, 1)
    }
}

pub fn light(attach: u16, flags: u16) -> ObjectRec {
    ObjectRec {
        flags,
        aux: u32::from(attach) << 16 | 1 << 8 | 1,
        p_bits: 5.0f64.to_bits(),
        ..obj(KIND_LIGHT, 3)
    }
}

/// A whole level in editable pieces.
#[derive(Clone)]
pub struct Lv {
    pub states: Vec<StateRec>,
    pub bindings: Vec<BindingRec>,
    pub objects: Vec<ObjectRec>,
    pub rules: Vec<RuleRec>,
    pub strings: Vec<StringRec>,
}

/// Objects: 0 snack, 1 chicken (group 1), 2 red key, 3 switch, 4 light, 5 light attached to the snack.
/// States: 0 has two values (starts at 0), 1 has three (starts at 1). Doors: 0 on state 0, 1 on state 1.
pub fn base() -> Lv {
    Lv {
        states: vec![StateRec::new(2, 0), StateRec::new(3, 1)],
        bindings: vec![
            BindingRec::door(0, 0b10, (5, 3, 5, 5)),
            BindingRec::door(1, 0b110, (9, 3, 9, 3)),
            BindingRec {
                kind: BIND_LIGHT,
                state: 1,
                mask: 0b001,
                target: 4,
                rect: (0, 0, 0, 0),
            },
        ],
        objects: vec![
            obj(KIND_SNACK, 1),
            chicken(1),
            obj(KIND_KEY_RED, 0),
            switch(SWITCH_USE | SWITCH_HIT_SHOT),
            light(ATTACH_NONE, 0),
            ObjectRec {
                x_bits: 0.5f64.to_bits(),
                y_bits: 1.0f64.to_bits(),
                ..light(0, LIGHT_ATTACHED)
            },
        ],
        rules: vec![],
        strings: vec![
            StringRec::new(2, "slam!"),
            StringRec::new(6, "Mind the gap \u{2026}"),
        ],
    }
}

impl Lv {
    pub fn spec_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        let tiles = vec![0u8; usize::from(W) * usize::from(H)];
        encode_level(&LevelSpec {
            width: W,
            height: H,
            floor_solid: false,
            spawn_x_bits: 1.5f64.to_bits(),
            spawn_y_bits: 1.5f64.to_bits(),
            name: "Rules",
            states: &self.states,
            bindings: &self.bindings,
            objects: &self.objects,
            rules: &self.rules,
            strings: &self.strings,
            tiles: &tiles,
        })
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.spec_bytes().unwrap()
    }
    pub fn err(&self) -> LevelError {
        LevelView::new(&self.bytes()).unwrap_err()
    }
    pub fn ok(&self) -> bool {
        LevelView::new(&self.bytes()).is_ok()
    }
    pub fn with_rules(&self, rules: &[RuleRec]) -> Lv {
        Lv {
            rules: rules.to_vec(),
            ..self.clone()
        }
    }
}

pub fn rec(trigger: u8) -> RuleRec {
    RuleRec {
        trigger,
        act: [ActionRec::set(0, 1), ActionRec::EMPTY],
        ..RuleRec::EMPTY
    }
}

pub fn rect_rule(trigger: u8, r: (u16, u16, u16, u16)) -> RuleRec {
    let (u0, u1) = RuleRec::union_rect(r.0, r.1, r.2, r.3);
    RuleRec {
        u0,
        u1,
        ..rec(trigger)
    }
}

/// Rule 0 of the slice, in the shape the baker lowers `lock` to: a touch over the door's whole rect.
pub fn lock() -> RuleRec {
    RuleRec {
        cond: [CondRec::state_in(0, 0b01), CondRec::holds(0)],
        act: [ActionRec::set(0, 1), ActionRec::spend_key(0)],
        ..rect_rule(TRIGGER_TOUCH, (5, 3, 5, 5))
    }
}

pub fn every_trigger() -> Vec<RuleRec> {
    vec![
        lock(),
        rec(TRIGGER_START),
        RuleRec {
            delay: 30,
            act: [
                ActionRec::set(0, 1),
                ActionRec {
                    op: OP_WORD,
                    a: CAP_LEVEL_BASE,
                    b: CUE_NONE,
                    x: 15,
                    y: 7,
                    ..ActionRec::EMPTY
                },
            ],
            ..rect_rule(TRIGGER_ENTER, (1, 2, 14, 2))
        },
        RuleRec {
            subject: 2,
            ..rec(TRIGGER_TAKE)
        },
        rect_rule(TRIGGER_SHOOT, (0, 0, 39, 11)),
        RuleRec {
            subject: SUBJECT_ANY,
            ..rec(TRIGGER_STOMP)
        },
        RuleRec {
            subject: 1,
            ..rec(TRIGGER_STUN)
        },
        RuleRec {
            u0: 60,
            u1: 90,
            flags: FLAG_REPEAT,
            cond: [CondRec::state_in(1, 0b101), CondRec::EMPTY],
            act: [
                ActionRec {
                    op: OP_TOGGLE,
                    a: 0,
                    ..ActionRec::EMPTY
                },
                ActionRec {
                    op: OP_CUE,
                    n: 0x1FF,
                    x: 1,
                    y: 1,
                    ..ActionRec::EMPTY
                },
            ],
            ..rec(TRIGGER_TIME)
        },
        rect_rule(TRIGGER_TOUCH, (0, 0, 39, 11)),
        RuleRec {
            subject: 4,
            ..rec(TRIGGER_TOUCH_OBJECT)
        },
        RuleRec {
            subject: 3,
            ..rec(TRIGGER_USE)
        },
        RuleRec {
            subject: 3,
            ..rec(TRIGGER_HIT)
        },
        RuleRec {
            subject: 1,
            ..rec(TRIGGER_DEFEAT)
        },
        RuleRec {
            subject: 1,
            act: [
                ActionRec {
                    op: OP_NEXT,
                    a: 1,
                    ..ActionRec::EMPTY
                },
                ActionRec {
                    op: OP_SCORE,
                    n: 9999,
                    ..ActionRec::EMPTY
                },
            ],
            ..rec(TRIGGER_CLEARED)
        },
        RuleRec {
            subject: 1,
            arg: 2,
            cond: [
                CondRec::EMPTY,
                CondRec {
                    kind: COND_NOT_HOLDS,
                    a: 2,
                    b: 0,
                },
            ],
            act: [
                ActionRec {
                    op: OP_GIVE_KEY,
                    a: 1,
                    ..ActionRec::EMPTY
                },
                ActionRec {
                    op: OP_AMMO,
                    n: 99,
                    ..ActionRec::EMPTY
                },
            ],
            ..rec(TRIGGER_BECOMES)
        },
        RuleRec {
            subject: 0,
            arg: ARG_ANY,
            ..rec(TRIGGER_BECOMES)
        },
    ]
}
