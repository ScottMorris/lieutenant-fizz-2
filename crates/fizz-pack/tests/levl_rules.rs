// LEVL version 2: states, bindings, objects, rules and strings. Round trips, byte layout, every reader check with the index of the
// record it names, the limits at both edges, the writer's refusals and a deflate round trip through the pack.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use fizz_pack::level::*;
use fizz_pack::write::{EncodeError, LevelSpec, encode_level};
use fizz_pack::{ActionRec, BindingRec, LevelView, RuleRec, StateRec, StringRec};

#[test]
fn a_whole_level_round_trips() {
    let lv = Lv {
        rules: every_trigger(),
        ..base()
    };
    let b = lv.bytes();
    // header, name padded to 8, 2 states, 3 bindings, 6 objects and 2 strings, 16 rules, tiles.
    assert_eq!(
        b.len(),
        32 + 8 + 8 * 2 + 16 * 3 + 32 * (6 + 2) + 48 * 16 + 480
    );
    assert_eq!(b.len(), level_len(5, 2, 3, 6, 16, 2, 480));
    assert_eq!((b[24], b[25], b[26], b[27]), (2, 3, 16, 2));
    assert_eq!(b[28..32], [0; 4]);
    let v = LevelView::new(&b).unwrap();
    assert_eq!(v.states().collect::<Vec<_>>(), lv.states);
    assert_eq!(v.bindings().collect::<Vec<_>>(), lv.bindings);
    assert_eq!(v.objects().collect::<Vec<_>>(), lv.objects);
    assert_eq!(v.rules().collect::<Vec<_>>(), lv.rules);
    assert_eq!(v.strings().collect::<Vec<_>>(), lv.strings);
    assert_eq!(v.tiles().len(), 480);
    assert_eq!(v.string(1).as_str(), "Mind the gap \u{2026}");
    assert_eq!(v.rule(0).rect(), (5, 3, 5, 5));
    assert_eq!(v.rule(7).time(), (60, 90));
    assert!(v.rule(7).repeat());
    let r = every_trigger()[0];
    assert_eq!(RuleRec::from_bytes(&r.to_bytes()), r);
    let s = lv.strings[0];
    assert_eq!(StringRec::from_bytes(&s.to_bytes()), s);
    assert_eq!(v.state(1).all_values(), 0b111);
    assert_eq!(StateRec::new(8, 0).all_values(), 0xFF);
}

#[test]
fn record_byte_layout() {
    let lv = Lv {
        rules: vec![every_trigger()[2]],
        ..base()
    };
    let b = lv.bytes();
    // The states, bindings, objects, rules, then strings, in that order after the padded name.
    let s = 32 + 8;
    assert_eq!(
        b[s..s + 16],
        [2, 0, 0, 0, 0, 0, 0, 0, 3, 1, 0, 0, 0, 0, 0, 0]
    );
    let g = s + 16;
    assert_eq!(
        b[g..g + 16],
        [1, 0, 2, 0, 0xFF, 0xFF, 0, 0, 5, 0, 3, 0, 5, 0, 5, 0]
    );
    let l = g + 32; // the light binding is the third
    assert_eq!(b[l..l + 8], [BIND_LIGHT, 1, 1, 0, 4, 0, 0, 0]);
    let o = g + 48;
    assert_eq!(b[o..o + 8], [KIND_SNACK, 1, 0, 0, 0, 0, 0, 0]);
    let light_at = o + 4 * 32;
    assert_eq!(
        b[light_at..light_at + 8],
        [KIND_LIGHT, 3, 0, 0, 1, 1, 0xFF, 0xFF]
    );
    let r = o + 6 * 32;
    assert_eq!(b[r..r + 8], [TRIGGER_ENTER, 0, 30, 0, 0xFF, 0xFF, 0, 0]);
    assert_eq!(b[r + 8..r + 16], [1, 0, 2, 0, 14, 0, 2, 0]);
    assert_eq!(b[r + 16..r + 24], [0; 8]);
    assert_eq!(b[r + 24..r + 36], [OP_SET, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        b[r + 36..r + 48],
        [
            OP_WORD,
            CAP_LEVEL_BASE,
            CUE_NONE,
            0,
            0,
            0,
            0,
            0,
            15,
            0,
            7,
            0
        ]
    );
    let t = r + 48;
    assert_eq!(b[t..t + 8], [5, 2, b's', b'l', b'a', b'm', b'!', 0]);
    // A negative amount is a little-endian i32.
    let a = ActionRec {
        op: OP_SCORE,
        n: 258,
        ..ActionRec::EMPTY
    };
    assert_eq!(a.to_bytes()[4..8], [2, 1, 0, 0]);
    assert_eq!(ActionRec::from_bytes(&a.to_bytes()), a);
}

#[test]
fn the_documented_lock_bytes() {
    // The worked example in docs/FAP.md: rule 0, the red lock.
    let b = lock().to_bytes();
    let mut want = vec![0x09, 0, 0, 0, 0xFF, 0xFF, 0, 0];
    want.extend_from_slice(&[5, 0, 3, 0, 5, 0, 5, 0]);
    want.extend_from_slice(&[1, 0, 1, 0, 2, 0, 0, 0]);
    want.extend_from_slice(&[1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    want.extend_from_slice(&[4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(b.to_vec(), want);
}

#[test]
fn the_header_checks() {
    let lv = base();
    let b = lv.bytes();
    assert!(lv.ok());
    // The version is the directory's.
    assert_eq!(
        LevelView::with_directory(&b, 1, 6).unwrap_err(),
        LevelError::BadVersion(1)
    );
    assert_eq!(
        LevelView::with_directory(&b, 3, 6).unwrap_err(),
        LevelError::BadVersion(3)
    );
    assert!(LevelView::with_directory(&b, 2, 6).is_ok());
    assert_eq!(
        LevelView::with_directory(&b, 2, 7).unwrap_err(),
        LevelError::CountMismatch
    );
    let bad = |i: usize, v: u8| {
        let mut c = b.clone();
        c[i] = v;
        LevelView::new(&c).unwrap_err()
    };
    assert_eq!(bad(4, 2), LevelError::BadField);
    for at in 28..32 {
        assert_eq!(bad(at, 1), LevelError::BadField, "reserved byte {at}");
    }
    // Counts that disagree with the length are a size error, whichever way.
    assert_eq!(bad(24, 1), LevelError::TrailingBytes);
    assert_eq!(bad(25, 2), LevelError::TrailingBytes);
    assert_eq!(bad(26, 1), LevelError::Truncated);
    assert_eq!(bad(27, 3), LevelError::Truncated);
    assert_eq!(LevelView::new(&b[..31]).unwrap_err(), LevelError::Truncated);
}

#[test]
fn counts_over_the_limits_are_refused_before_the_length() {
    let b = base().bytes();
    let bad = |i: usize, v: u8| {
        let mut c = b.clone();
        c[i] = v;
        LevelView::new(&c).unwrap_err()
    };
    assert_eq!(bad(24, 33), LevelError::TooManyStates);
    assert_eq!(bad(25, 65), LevelError::TooManyBindings);
    assert_eq!(bad(26, 33), LevelError::TooManyRules);
    assert_eq!(bad(27, 33), LevelError::TooManyStrings);
    assert_eq!(MAX_STATES, 32);
    assert_eq!(MAX_BINDINGS, 64);
    assert_eq!(MAX_DOOR_BINDINGS, 32);
}

#[test]
fn the_limits_pass_at_their_edge() {
    // 32 states, 32 door bindings (one cell each) and 32 light bindings: 64 bindings, 32 rules and 32 strings.
    let states: Vec<StateRec> = (0..32)
        .map(|i| StateRec::new(2 + (i % 7) as u8, 0))
        .collect();
    let mut bindings: Vec<BindingRec> = (0..32u16)
        .map(|i| BindingRec::door(i as u8, 0b10, (i, 0, i, 0)))
        .collect();
    let mut objects = vec![];
    for i in 0..32u16 {
        objects.push(light(ATTACH_NONE, 0));
        bindings.push(BindingRec {
            kind: BIND_LIGHT,
            state: 0,
            mask: 0b01,
            target: i,
            rect: (0, 0, 0, 0),
        });
    }
    let rules: Vec<RuleRec> = (0..32).map(|_| rec(TRIGGER_START)).collect();
    let strings: Vec<StringRec> = (0..32).map(|i| StringRec::new(i % 7, "x")).collect();
    let lv = Lv {
        states,
        bindings,
        objects,
        rules,
        strings,
    };
    let v = LevelView::new(&lv.bytes()).map(|v| {
        (
            v.state_count(),
            v.binding_count(),
            v.rule_count(),
            v.string_count(),
        )
    });
    assert_eq!(v, Ok((32, 64, 32, 32)));
    // The 33rd door binding is refused with its own index, though the total is within 64. (The writer would not encode it, so the
    // bytes are edited: the first light binding becomes a door.)
    let mut b = lv.bytes();
    let at = 32 + 8 + 8 * 32 + 16 * 32;
    let door = BindingRec::door(0, 0b10, (33, 0, 33, 0)).to_bytes();
    b[at..at + 16].copy_from_slice(&door);
    assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadBinding(32));
}

#[test]
fn the_writer_refuses_input_over_a_limit() {
    let lv = base();
    let st = |n: usize| Lv {
        states: vec![StateRec::new(2, 0); n],
        ..lv.clone()
    };
    assert!(st(32).spec_bytes().is_ok());
    assert_eq!(st(33).spec_bytes(), Err(EncodeError::TooManyStates));
    let bd = |n: usize| Lv {
        bindings: vec![BindingRec::door(0, 1, (0, 0, 0, 0)); n],
        ..lv.clone()
    };
    assert!(bd(32).spec_bytes().is_ok());
    assert_eq!(bd(64).spec_bytes(), Err(EncodeError::TooManyDoorBindings));
    assert_eq!(bd(65).spec_bytes(), Err(EncodeError::TooManyBindings));
    assert_eq!(bd(33).spec_bytes(), Err(EncodeError::TooManyDoorBindings));
    let ru = |n: usize| lv.with_rules(&vec![rec(TRIGGER_START); n]);
    assert!(ru(32).spec_bytes().is_ok());
    assert_eq!(ru(33).spec_bytes(), Err(EncodeError::TooManyRules));
    let sr = |n: usize| Lv {
        strings: vec![StringRec::new(0, "x"); n],
        ..lv.clone()
    };
    assert!(sr(32).spec_bytes().is_ok());
    assert_eq!(sr(33).spec_bytes(), Err(EncodeError::TooManyStrings));
    let spec = |name: &str, tiles: &[u8]| {
        encode_level(&LevelSpec {
            width: 2,
            height: 2,
            floor_solid: false,
            spawn_x_bits: 0,
            spawn_y_bits: 0,
            name,
            states: &[],
            bindings: &[],
            objects: &[],
            rules: &[],
            strings: &[],
            tiles,
        })
    };
    assert!(spec(&"n".repeat(48), &[0; 4]).is_ok());
    assert_eq!(
        spec(&"n".repeat(49), &[0; 4]),
        Err(EncodeError::NameTooLong)
    );
    assert_eq!(spec("n", &[0; 3]), Err(EncodeError::TileCount));
    assert_eq!(spec("n", &[0; 5]), Err(EncodeError::TileCount));
}

#[test]
fn state_refusals() {
    let with = |s: StateRec| Lv {
        states: vec![StateRec::new(2, 0), s],
        // The bindings and rules of `base` name state 1, so a plain level is used here.
        bindings: vec![],
        objects: vec![],
        rules: vec![],
        strings: vec![],
    };
    assert!(with(StateRec::new(2, 1)).ok());
    assert!(with(StateRec::new(8, 7)).ok());
    for bad in [
        StateRec::new(0, 0),
        StateRec::new(1, 0),
        StateRec::new(9, 0),
        StateRec::new(2, 2),
        StateRec::new(8, 8),
        StateRec {
            flags: STATE_WORLD,
            ..StateRec::new(2, 0)
        },
        StateRec {
            flags: 2,
            ..StateRec::new(2, 0)
        },
    ] {
        assert_eq!(with(bad).err(), LevelError::BadState(1), "{bad:?}");
    }
    // A non-zero reserved byte names its state.
    let lv = with(StateRec::new(3, 0));
    let mut b = lv.bytes();
    b[32 + 8 + 8 + 5] = 1;
    assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadState(1));
}

#[test]
fn binding_refusals() {
    let lv = base();
    let with = |i: usize, f: &dyn Fn(&mut BindingRec)| {
        let mut c = lv.clone();
        f(&mut c.bindings[i]);
        c.err()
    };
    let door = |f: &dyn Fn(&mut BindingRec)| with(1, f);
    let bad = |i| LevelError::BadBinding(i);
    // The kind: 0, the reserved platform and panel, and the unassigned.
    for k in [0, BIND_PLATFORM, BIND_PANEL, 5, 255] {
        assert_eq!(door(&|b| b.kind = k), bad(1), "kind {k}");
    }
    // The state, and the mask against that state's values (state 1 has three).
    assert_eq!(door(&|b| b.state = 2), bad(1));
    assert_eq!(door(&|b| b.mask = 0), bad(1));
    assert_eq!(door(&|b| b.mask = 0b1000), bad(1));
    assert_eq!(door(&|b| b.mask = 0xFF), bad(1));
    assert_eq!(with(0, &|b| b.mask = 0b100), bad(0)); // state 0 has two values
    // The active set may be every value of the state.
    assert!(lv.ok());
    let mut all = lv.clone();
    all.bindings[0].mask = 0b11;
    assert!(all.ok());
    // A door's target and rect.
    assert_eq!(door(&|b| b.target = 4), bad(1));
    assert_eq!(door(&|b| b.rect = (9, 3, 8, 3)), bad(1)); // x inverted
    assert_eq!(door(&|b| b.rect = (9, 4, 9, 3)), bad(1)); // y inverted
    assert_eq!(door(&|b| b.rect = (9, 3, W, 3)), bad(1)); // past the width
    assert_eq!(door(&|b| b.rect = (9, 3, 9, H)), bad(1)); // past the height
    // A light binding: an object of kind 8, no rect.
    assert_eq!(with(2, &|b| b.target = 0xFFFF), bad(2));
    assert_eq!(with(2, &|b| b.target = 0), bad(2)); // a snack
    assert_eq!(with(2, &|b| b.target = 6), bad(2)); // past the objects
    assert_eq!(with(2, &|b| b.rect = (1, 0, 1, 0)), bad(2));
    // Reserved bytes.
    for at in [3, 6, 7] {
        let mut b = lv.bytes();
        b[32 + 8 + 16 + 16 + at] = 1;
        assert_eq!(LevelView::new(&b).unwrap_err(), bad(1), "reserved {at}");
    }
    // Nothing is bound twice: overlapping door rects, two bindings on one light.
    let mut twice = lv.clone();
    twice.bindings[1].rect = (5, 5, 6, 6);
    assert_eq!(twice.err(), bad(1));
    let mut twice = lv.clone();
    twice.bindings.push(BindingRec {
        kind: BIND_LIGHT,
        state: 0,
        mask: 1,
        target: 4,
        rect: (0, 0, 0, 0),
    });
    assert_eq!(twice.err(), bad(3));
    // Doors that only touch are fine.
    let mut next = lv.clone();
    next.bindings[1].rect = (6, 3, 6, 5);
    assert!(next.ok());
}
