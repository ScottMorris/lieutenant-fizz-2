// LEVL version 2: the reader's checks on objects, rules and strings, each with the index of the record it names, and a deflate round
// trip of a level with rules through a pack.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use fizz_pack::level::*;
use fizz_pack::pack::{CODEC_DEFLATE, TAG_LEVL, TARGET_DOS};
use fizz_pack::write::PackWriter;
use fizz_pack::{
    ActionRec, CondRec, Expect, LevelView, ObjectRec, PackView, RuleRec, StringRec, compatibility,
};

/// `base` with `o` added as object 6.
fn with_object(o: ObjectRec) -> Lv {
    let mut lv = base();
    lv.objects.push(o);
    lv
}

fn pad(b: &[u8]) -> [u8; STRING_TEXT_MAX] {
    let mut t = [0; STRING_TEXT_MAX];
    t[..b.len()].copy_from_slice(b);
    t
}

#[test]
fn object_refusals() {
    let bad_obj = LevelError::BadObject(6);
    let bad_kind = LevelError::BadKind(6);
    let key = obj(KIND_KEY_RED, 0);
    assert!(with_object(key).ok());
    // Kinds: 0, over 10 and the reserved ones.
    for k in [
        0,
        KIND_GADGET,
        KIND_ENEMY,
        KIND_CUSTOM_ENEMY,
        KIND_CUSTOM_ITEM,
        11,
        255,
    ] {
        assert_eq!(
            with_object(ObjectRec { kind: k, ..key }).err(),
            bad_kind,
            "kind {k}"
        );
    }
    // Keys carry nothing.
    for o in [
        ObjectRec { sub: 1, ..key },
        ObjectRec { flags: 1, ..key },
        ObjectRec { aux: 1, ..key },
        ObjectRec {
            p_bits: 1.0f64.to_bits(),
            ..key
        },
        ObjectRec {
            kind: KIND_KEY_BLUE,
            aux: 1,
            ..key
        },
    ] {
        assert_eq!(with_object(o).err(), bad_obj, "{o:?}");
    }
    // Snacks: a kind 0 to 4 (the elixir is 4), nothing else.
    assert!(with_object(obj(KIND_SNACK, 4)).ok());
    assert_eq!(with_object(obj(KIND_SNACK, 5)).err(), bad_obj);
    assert_eq!(
        with_object(ObjectRec {
            aux: 1,
            ..obj(KIND_SNACK, 0)
        })
        .err(),
        bad_obj
    );
    // Chickens: a direction of 1 or -1, a group of 0 to 16 in byte 0 and nothing in the other bytes.
    assert!(with_object(chicken(0)).ok() && with_object(chicken(16)).ok());
    assert_eq!(with_object(chicken(17)).err(), bad_obj);
    assert_eq!(with_object(chicken(0x100)).err(), bad_obj);
    for p in [0.0f64, 0.5, f64::NAN] {
        let o = ObjectRec {
            p_bits: p.to_bits(),
            ..chicken(1)
        };
        assert_eq!(with_object(o).err(), bad_obj, "direction {p}");
    }
    let right = ObjectRec {
        p_bits: 1.0f64.to_bits(),
        ..chicken(1)
    };
    assert!(with_object(right).ok());
    assert_eq!(
        with_object(ObjectRec {
            sub: 1,
            ..chicken(1)
        })
        .err(),
        bad_obj
    );
    assert_eq!(
        with_object(ObjectRec {
            flags: 1,
            ..chicken(1)
        })
        .err(),
        bad_obj
    );
    // Switches: a style of 0 or 1, at least one input and no other flag, and nothing else.
    for f in [1, 2, 4, 7] {
        assert!(with_object(switch(f)).ok(), "{f}");
    }
    assert_eq!(with_object(switch(0)).err(), bad_obj);
    assert_eq!(with_object(switch(8)).err(), bad_obj);
    assert_eq!(
        with_object(ObjectRec {
            sub: 2,
            ..switch(1)
        })
        .err(),
        bad_obj
    );
    assert_eq!(
        with_object(ObjectRec {
            aux: 1,
            ..switch(1)
        })
        .err(),
        bad_obj
    );
    assert_eq!(
        with_object(ObjectRec {
            p_bits: 1,
            ..switch(1)
        })
        .err(),
        bad_obj
    );
    // Lights: a colour of 0 to 15, a radius above 0 and up to 16, falloff 0 or 1, flicker 0 to 2.
    let free = light(ATTACH_NONE, 0);
    assert!(with_object(free).ok());
    assert_eq!(with_object(ObjectRec { sub: 16, ..free }).err(), bad_obj);
    for r in [0.0f64, -1.0, 16.5, f64::NAN, f64::INFINITY] {
        let o = ObjectRec {
            p_bits: r.to_bits(),
            ..free
        };
        assert_eq!(with_object(o).err(), bad_obj, "radius {r}");
    }
    let widest = ObjectRec {
        p_bits: 16.0f64.to_bits(),
        ..free
    };
    assert!(with_object(widest).ok());
    let aux = |falloff: u32, flicker: u32| ObjectRec {
        aux: falloff | flicker << 8 | u32::from(ATTACH_NONE) << 16,
        ..free
    };
    assert!(with_object(aux(1, 2)).ok());
    assert_eq!(with_object(aux(2, 0)).err(), bad_obj, "falloff");
    assert_eq!(with_object(aux(0, 3)).err(), bad_obj, "flicker");
    assert_eq!(with_object(ObjectRec { flags: 2, ..free }).err(), bad_obj);
    // Attach: Ben, a snack or a chicken; ATTACHED exactly when there is a target.
    assert!(with_object(light(ATTACH_BEN, LIGHT_ATTACHED)).ok());
    assert!(with_object(light(1, LIGHT_ATTACHED)).ok());
    assert_eq!(
        with_object(light(ATTACH_BEN, 0)).err(),
        bad_obj,
        "a target without the flag"
    );
    assert_eq!(
        with_object(light(ATTACH_NONE, LIGHT_ATTACHED)).err(),
        bad_obj,
        "the flag without a target"
    );
    for (target, what) in [
        (2, "a key"),
        (3, "a switch"),
        (4, "a light"),
        (6, "itself"),
        (7, "past the objects"),
    ] {
        assert_eq!(
            with_object(light(target, LIGHT_ATTACHED)).err(),
            bad_obj,
            "{what}"
        );
    }
    // Positions: inside the map, and for an attached light an offset within 16 tiles.
    let at = |x: f64, y: f64| ObjectRec {
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
        ..key
    };
    assert!(with_object(at(0.0, 0.0)).ok() && with_object(at(39.9, 11.9)).ok());
    for (x, y) in [
        (-0.1, 1.0),
        (40.0, 1.0),
        (1.0, 12.0),
        (1.0, -0.5),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
    ] {
        assert_eq!(
            with_object(at(x, y)).err(),
            LevelError::BadPosition(6),
            "{x} {y}"
        );
    }
    let off = |x: f64, y: f64| ObjectRec {
        x_bits: x.to_bits(),
        y_bits: y.to_bits(),
        ..light(ATTACH_BEN, LIGHT_ATTACHED)
    };
    assert!(
        with_object(off(-16.0, 16.0)).ok(),
        "an offset may be negative"
    );
    assert_eq!(
        with_object(off(16.1, 0.0)).err(),
        LevelError::BadPosition(6)
    );
    assert_eq!(
        with_object(off(0.0, f64::NAN)).err(),
        LevelError::BadPosition(6)
    );
    // The index is the object's own, whichever object is wrong.
    let mut lv = base();
    lv.objects[2] = ObjectRec { sub: 1, ..key };
    assert_eq!(lv.err(), LevelError::BadObject(2));
    lv.objects[2] = ObjectRec { kind: 0, ..key };
    assert_eq!(lv.err(), LevelError::BadKind(2));
}

/// `base` with `r` as rule 1, after a valid rule 0.
fn rule_level(r: RuleRec) -> Lv {
    base().with_rules(&[rec(TRIGGER_START), r])
}

fn refuses(r: RuleRec) -> bool {
    rule_level(r).err() == LevelError::BadRule(1)
}

fn accepts(r: RuleRec) -> bool {
    rule_level(r).ok()
}

fn with_act(op: ActionRec) -> RuleRec {
    RuleRec {
        act: [op, ActionRec::EMPTY],
        ..rec(TRIGGER_START)
    }
}

fn with_cond(c: CondRec) -> RuleRec {
    RuleRec {
        cond: [c, CondRec::EMPTY],
        ..rec(TRIGGER_START)
    }
}

fn action(op: u8) -> ActionRec {
    ActionRec {
        op,
        ..ActionRec::EMPTY
    }
}

#[test]
fn rule_triggers_flags_and_reserved_bytes() {
    assert!(accepts(lock()));
    // Triggers 2 to 15 only: version 1's lock (1) is gone.
    for t in [0, 1, 16, 17, 255] {
        assert!(refuses(rec(t)), "trigger {t}");
    }
    // Only REPEAT is a flag.
    assert!(accepts(RuleRec {
        flags: FLAG_REPEAT,
        ..rec(TRIGGER_START)
    }));
    for f in [2, 4, 0x80] {
        assert!(
            refuses(RuleRec {
                flags: f,
                ..rec(TRIGGER_START)
            }),
            "flag {f}"
        );
    }
    // The reserved byte of a rule (offset 7) names the rule.
    let mut b = rule_level(rec(TRIGGER_START)).bytes();
    let rules_at = 32 + 8 + 8 * 2 + 16 * 3 + 32 * 6;
    b[rules_at + 48 + 7] = 1;
    assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadRule(1));
    // Rule 0's, too.
    let mut b = rule_level(rec(TRIGGER_START)).bytes();
    b[rules_at + 7] = 1;
    assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadRule(0));
}

#[test]
fn rule_subjects() {
    let subj = |t, s| RuleRec {
        subject: s,
        ..rec(t)
    };
    // Objects: 0 snack, 1 chicken (group 1), 2 red key, 3 switch, 4 light, 5 light.
    for ok in [
        subj(TRIGGER_TAKE, 0),
        subj(TRIGGER_TAKE, 2),
        subj(TRIGGER_STOMP, 1),
        subj(TRIGGER_STOMP, SUBJECT_ANY),
        subj(TRIGGER_STUN, 1),
        subj(TRIGGER_DEFEAT, SUBJECT_ANY),
        subj(TRIGGER_TOUCH_OBJECT, 5),
        subj(TRIGGER_USE, 3),
        subj(TRIGGER_HIT, 3),
        subj(TRIGGER_CLEARED, 1),
        subj(TRIGGER_BECOMES, 1),
    ] {
        assert!(accepts(ok), "{ok:?}");
    }
    for bad in [
        subj(TRIGGER_TAKE, 1),           // a chicken
        subj(TRIGGER_TAKE, 6),           // past the objects
        subj(TRIGGER_TAKE, SUBJECT_ANY), // no "any" for a take
        subj(TRIGGER_STOMP, 0),          // a snack
        subj(TRIGGER_STUN, 9),
        subj(TRIGGER_DEFEAT, 2),
        subj(TRIGGER_TOUCH_OBJECT, 6),
        subj(TRIGGER_TOUCH_OBJECT, SUBJECT_ANY),
        subj(TRIGGER_USE, 4), // a light
        subj(TRIGGER_USE, SUBJECT_ANY),
        subj(TRIGGER_HIT, 0),
        subj(TRIGGER_CLEARED, 0), // groups are 1 to 16
        subj(TRIGGER_CLEARED, 2), // no chicken carries group 2
        subj(TRIGGER_CLEARED, 17),
        subj(TRIGGER_BECOMES, 2), // only two states
        subj(TRIGGER_BECOMES, SUBJECT_ANY),
        subj(TRIGGER_START, 1), // "none" is 0xFFFF
        subj(TRIGGER_START, 0),
        subj(TRIGGER_TIME, 1),
        subj(TRIGGER_TOUCH, 0),
    ] {
        assert!(refuses(bad), "{bad:?}");
    }
}

#[test]
fn rule_args_rects_and_the_union() {
    // The arg: a value of the named state (or any change) on `becomes`, zero elsewhere.
    let becomes = |s, arg| RuleRec {
        subject: s,
        arg,
        ..rec(TRIGGER_BECOMES)
    };
    assert!(accepts(becomes(1, 0)) && accepts(becomes(1, 2)) && accepts(becomes(1, ARG_ANY)));
    assert!(refuses(becomes(1, 3)), "state 1 has three values");
    assert!(accepts(becomes(0, 1)) && refuses(becomes(0, 2)) && accepts(becomes(0, ARG_ANY)));
    assert!(refuses(RuleRec {
        arg: 1,
        ..rec(TRIGGER_START)
    }));
    // A rect: inside the map and not inverted, for the three triggers that name one. A touch may be of any size.
    for t in [TRIGGER_ENTER, TRIGGER_SHOOT, TRIGGER_TOUCH] {
        assert!(accepts(rect_rule(t, (0, 0, W - 1, H - 1))), "{t}");
        assert!(accepts(rect_rule(t, (7, 7, 7, 7))), "{t}");
        for r in [(0, 0, W, 0), (0, 0, 0, H), (5, 0, 4, 0), (0, 5, 0, 4)] {
            assert!(refuses(rect_rule(t, r)), "{t} {r:?}");
        }
    }
    // A union on a trigger that has none.
    for t in [
        TRIGGER_START,
        TRIGGER_STOMP,
        TRIGGER_DEFEAT,
        TRIGGER_BECOMES,
    ] {
        let (u0, u1) = RuleRec::union_rect(0, 0, 3, 3);
        let r = RuleRec {
            u0,
            u1,
            subject: if t == TRIGGER_BECOMES { 0 } else { SUBJECT_ANY },
            ..rec(t)
        };
        assert!(refuses(r), "trigger {t}");
    }
    // Time: at >= 1, every 0 or >= MIN_PERIOD, REPEAT exactly when every is not zero.
    let time = |at, every, flags| RuleRec {
        u0: at,
        u1: every,
        flags,
        ..rec(TRIGGER_TIME)
    };
    assert!(accepts(time(1, 0, 0)));
    assert!(accepts(time(1, MIN_PERIOD, FLAG_REPEAT)));
    assert!(accepts(time(u32::MAX, u32::MAX, FLAG_REPEAT)));
    assert!(refuses(time(0, 0, 0)));
    assert!(refuses(time(1, MIN_PERIOD - 1, FLAG_REPEAT)));
    assert!(refuses(time(1, 1, FLAG_REPEAT)));
    assert!(refuses(time(1, 0, FLAG_REPEAT)));
    assert!(refuses(time(1, 10, 0)));
    // The delay is any u16.
    assert!(accepts(RuleRec {
        delay: u16::MAX,
        ..rec(TRIGGER_START)
    }));
}

#[test]
fn condition_checks() {
    let c = |kind, a, b| CondRec { kind, a, b };
    // Empty, state in a set (state 0 has two values, state 1 three), holds, does not hold.
    for ok in [
        CondRec::EMPTY,
        c(COND_STATE_IN, 0, 0b01),
        c(COND_STATE_IN, 0, 0b11),
        c(COND_STATE_IN, 1, 0b111),
        c(COND_STATE_IN, 1, 0b100),
        c(COND_HOLDS, 0, 0),
        c(COND_HOLDS, 2, 0),
        c(COND_NOT_HOLDS, 1, 0),
    ] {
        assert!(accepts(with_cond(ok)), "{ok:?}");
    }
    for bad in [
        c(0, 1, 0), // an empty kind with a field
        c(0, 0, 1),
        c(COND_STATE_IN, 2, 0b01),  // no such state
        c(COND_STATE_IN, 0, 0),     // an empty set
        c(COND_STATE_IN, 0, 0b100), // a bit above the state's values
        c(COND_STATE_IN, 1, 0b1000),
        c(COND_HOLDS, 3, 0), // key colours are 0 to 2
        c(COND_HOLDS, 0, 1), // b is zero for a key
        c(COND_NOT_HOLDS, 3, 0),
        c(COND_AMMO_AT_LEAST, 0, 0), // numbered and refused
        c(COND_SCORE_AT_LEAST, 0, 0),
        c(COND_CLEARED, 0, 0),
        c(7, 0, 0),
        c(255, 0, 0),
    ] {
        assert!(refuses(with_cond(bad)), "{bad:?}");
    }
    // The second slot is checked as well, and the reserved byte (offset 3) of either.
    assert!(refuses(RuleRec {
        cond: [CondRec::EMPTY, c(COND_HOLDS, 3, 0)],
        ..rec(TRIGGER_START)
    }));
    let rules_at = 32 + 8 + 8 * 2 + 16 * 3 + 32 * 6;
    for slot in [16 + 3, 20 + 3] {
        let mut b = rule_level(rec(TRIGGER_START)).bytes();
        b[rules_at + 48 + slot] = 1;
        assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadRule(1));
    }
}

#[test]
fn action_checks() {
    let with = |op, a, b, n, x, y| ActionRec { op, a, b, n, x, y };
    for ok in [
        with(OP_SET, 0, 1, 0, 0, 0),
        with(OP_SET, 1, 2, 0, 0, 0),
        with(OP_TOGGLE, 0, 0, 0, 0, 0),
        with(OP_NEXT, 1, 0, 0, 0, 0),
        with(OP_SPEND_KEY, 2, 0, 0, 0, 0),
        with(OP_GIVE_KEY, 0, 0, 0, 0, 0),
        with(OP_SCORE, 0, 0, 1, 0, 0),
        with(OP_SCORE, 0, 0, 9999, 0, 0),
        with(OP_AMMO, 0, 0, 1, 0, 0),
        with(OP_AMMO, 0, 0, 99, 0, 0),
        // A word: a shared id, the level's strings 0 and 1, the world's strings, a style or the string's own, a cell in the map.
        with(OP_WORD, 0x3F, 0, 0, 0, 0),
        with(OP_WORD, CAP_LEVEL_BASE, 6, 0, 1, 1),
        with(OP_WORD, CAP_LEVEL_BASE + 1, CUE_NONE, 0, W - 1, H - 1),
        with(OP_WORD, CAP_WORLD_BASE, CUE_NONE, 0, 0, 0),
        with(OP_WORD, 0xBF, CUE_NONE, 0, 0, 0),
        // A cue: a shared effect, the world's library, up to its 256th entry.
        with(OP_CUE, 0, 0, 0, 0, 0),
        with(OP_CUE, 0, 0, 0x100, 3, 3),
        with(OP_CUE, 0, 0, CUE_ID_MAX, W - 1, H - 1),
    ] {
        assert!(accepts(with_act(ok)), "{ok:?}");
    }
    for bad in [
        action(10),
        action(255),
        with(OP_SET, 2, 0, 0, 0, 0),    // no such state
        with(OP_SET, 0, 2, 0, 0, 0),    // state 0 has two values
        with(OP_SET, 1, 3, 0, 0, 0),    // state 1 has three
        with(OP_SET, 0, 1, 1, 0, 0),    // n is zero for a set
        with(OP_SET, 0, 1, 0, 1, 0),    // so is the cell
        with(OP_TOGGLE, 1, 0, 0, 0, 0), // a toggle needs exactly two values
        with(OP_TOGGLE, 2, 0, 0, 0, 0),
        with(OP_TOGGLE, 0, 1, 0, 0, 0),
        with(OP_NEXT, 2, 0, 0, 0, 0),
        with(OP_SPEND_KEY, 3, 0, 0, 0, 0),
        with(OP_GIVE_KEY, 0, 1, 0, 0, 0),
        with(OP_SCORE, 0, 0, 0, 0, 0),
        with(OP_SCORE, 0, 0, 10000, 0, 0),
        with(OP_SCORE, 0, 0, -5, 0, 0),
        with(OP_AMMO, 0, 0, 0, 0, 0),
        with(OP_AMMO, 0, 0, 100, 0, 0),
        with(OP_WORD, CUE_NONE, 0, 0, 0, 0), // an unused action is empty instead
        with(OP_WORD, CAP_LEVEL_BASE + 2, 0, 0, 0, 0), // only two strings
        with(OP_WORD, 0xC0, 0, 0, 0, 0),
        with(OP_WORD, 0x7F, 0, 0, 0, 0),
        with(OP_WORD, 0, STYLE_COUNT, 0, 0, 0),
        with(OP_WORD, 0, 0, 1, 0, 0),
        with(OP_WORD, 0, 0, 0, W, 0),
        with(OP_WORD, 0, 0, 0, 0, H),
        with(OP_CUE, 1, 0, 0, 0, 0),
        with(OP_CUE, 0, 1, 0, 0, 0),
        with(OP_CUE, 0, 0, -1, 0, 0),
        with(OP_CUE, 0, 0, CUE_ID_MAX + 1, 0, 0),
        with(OP_CUE, 0, 0, 0, W, 0),
        with(0, 1, 0, 0, 0, 0), // an empty op with a field
        with(0, 0, 0, 0, 0, 1),
    ] {
        assert!(refuses(with_act(bad)), "{bad:?}");
    }
    // Slot 1 is checked as slot 0 is, and the reserved byte (offset 3) of either.
    assert!(refuses(RuleRec {
        act: [ActionRec::set(0, 1), with(OP_SET, 0, 2, 0, 0, 0)],
        ..rec(TRIGGER_START)
    }));
    let rules_at = 32 + 8 + 8 * 2 + 16 * 3 + 32 * 6;
    for slot in [24 + 3, 36 + 3] {
        let mut b = rule_level(rec(TRIGGER_START)).bytes();
        b[rules_at + 48 + slot] = 1;
        assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadRule(1));
    }
    // A rule with no action does nothing and is refused; one in the second slot only is fine.
    assert!(refuses(RuleRec {
        act: [ActionRec::EMPTY; 2],
        ..rec(TRIGGER_START)
    }));
    assert!(accepts(RuleRec {
        act: [ActionRec::EMPTY, ActionRec::set(0, 1)],
        ..rec(TRIGGER_START)
    }));
    // A rule's conditions may be absent: a lock without its key test is still a well-formed rule.
    assert!(accepts(RuleRec {
        cond: [CondRec::EMPTY; 2],
        ..lock()
    }));
}

#[test]
fn a_rule_without_level_strings_cannot_name_one() {
    let word = RuleRec {
        act: [
            ActionRec {
                op: OP_WORD,
                a: CAP_LEVEL_BASE,
                b: CUE_NONE,
                ..ActionRec::EMPTY
            },
            ActionRec::EMPTY,
        ],
        ..rec(TRIGGER_START)
    };
    let mut lv = base().with_rules(&[word]);
    assert!(lv.ok());
    lv.strings.clear();
    assert_eq!(lv.err(), LevelError::BadRule(0));
}

#[test]
fn string_refusals() {
    let with = |s: StringRec| Lv {
        strings: vec![StringRec::new(0, "ok"), s],
        ..base()
    };
    let ok = |s: StringRec| with(s).ok();
    let bad = |s: StringRec| with(s).err() == LevelError::BadString(1);
    // 30 bytes, 1 byte and each charset edge pass.
    assert!(ok(StringRec::new(0, &"a".repeat(30))));
    assert!(ok(StringRec::new(6, " ")));
    assert!(ok(StringRec::new(0, "~")));
    for c in STRING_EXTRAS {
        let s = char::from_u32(c).unwrap().to_string();
        assert!(ok(StringRec::new(0, &s)), "{c:#x}");
    }
    // The widest string is not checked here.
    let extras = "\u{2026}\u{2014}\u{2013}\u{00B7}\u{2018}\u{2019}\u{201C}\u{201D}";
    assert!(ok(StringRec::new(0, extras)));
    // Empty, over long, style, charset, UTF-8, padding.
    let raw = StringRec::new(0, "ab");
    assert!(bad(StringRec { len: 0, ..raw }));
    assert!(bad(StringRec { len: 31, ..raw }));
    assert!(bad(StringRec {
        style: STYLE_COUNT,
        ..raw
    }));
    assert!(ok(StringRec {
        style: STYLE_COUNT - 1,
        ..raw
    }));
    for b in [0x1F, 0x7F, 0] {
        assert!(
            bad(StringRec {
                len: 1,
                text: pad(&[b]),
                ..raw
            }),
            "{b:#x}"
        );
    }
    assert!(bad(StringRec::new(0, "\u{00E9}"))); // e acute is not in the font
    assert!(bad(StringRec::new(0, "\u{2022}"))); // a bullet is not in the extras
    assert!(bad(StringRec {
        len: 2,
        text: pad(&[0xC3, 0x28]),
        ..raw
    })); // invalid UTF-8
    assert!(bad(StringRec {
        len: 1,
        text: pad(&[0xC3]),
        ..raw
    })); // a cut sequence
    assert!(bad(StringRec {
        len: 1,
        text: pad(b"ab"),
        ..raw
    })); // padding that is not zero
}

#[test]
fn the_error_always_names_the_first_bad_record() {
    // Two bad rules: the first is named.
    let mut lv = base().with_rules(&[rec(TRIGGER_START), rec(1), rec(0)]);
    assert_eq!(lv.err(), LevelError::BadRule(1));
    // A bad state is found before a bad object.
    lv.rules.clear();
    lv.states[1] = StateRec::new(1, 0);
    lv.objects[3] = obj(0, 0);
    assert_eq!(lv.err(), LevelError::BadState(1));
}

#[test]
fn rules_in_a_pack_survive_deflate() {
    let lv = Lv {
        rules: every_trigger(),
        ..base()
    };
    let b = lv.bytes();
    let compat = compatibility(b"t");
    let mut w = PackWriter::new(TARGET_DOS, compat, "LIGHT");
    w.add_compressed(TAG_LEVL, LEVEL_VERSION, 6, &b);
    let f = w.finish();
    let want = Expect {
        target: TARGET_DOS,
        stem: "LIGHT",
        compat,
    };
    let p = PackView::parse(&f, &want).unwrap();
    let e = p.find(TAG_LEVL, 0).unwrap();
    assert_eq!(e.codec, CODEC_DEFLATE, "a level of zero tiles compresses");
    assert_eq!(e.version, LEVEL_VERSION);
    let mut out = vec![0u8; e.decoded_len as usize];
    let n = p.decode(&e, &mut out).unwrap();
    assert_eq!(&out[..n], &b[..]);
    let v = LevelView::with_directory(&out[..n], e.version, e.count).unwrap();
    assert_eq!(v.rules().collect::<Vec<_>>(), lv.rules);
    assert_eq!(v.strings().collect::<Vec<_>>(), lv.strings);
    assert_eq!(v.states().collect::<Vec<_>>(), lv.states);
    assert_eq!(v.bindings().collect::<Vec<_>>(), lv.bindings);
}
