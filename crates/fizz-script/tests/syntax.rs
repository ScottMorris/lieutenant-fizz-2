// The forms of the merged dialect: positions (`x= y=` for a cell, `at=` for an exact point), `state`, `door`, `when` and `lock`
// as sugar for exactly one state, one door binding and one touch rule.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_ep2::level::Object;
use fizz_ep2::rules::{KEY_BLUE, KEY_RED};
use fizz_pack::level::{
    ARG_ANY, COND_NOT_HOLDS, FLAG_REPEAT, OP_NEXT, OP_SCORE, OP_TOGGLE, OP_WORD, SUBJECT_ANY,
    TRIGGER_BECOMES, TRIGGER_STOMP, TRIGGER_TIME, TRIGGER_TOUCH,
};
use fizz_pack::{ActionRec, BindingRec, CondRec, RuleRec, StateRec};
use fizz_script::expand::{self, Caption};
use fizz_script::{Code, LowerError, check, format, lower};
use std::collections::BTreeSet;

const HEAD: &str = "script fizz2 0\nlevel \"T\" 4x4 id=t\nstart 1,1\n";
/// A red door at x 2, y 1 and 2; a blue door at x 1, y 0.
const MAP: &str = "map\n  |....|  3\n  |..D.|  2\n  |..D.|  1\n  |.b##|  0\n";
const KEYS: &str =
    "objects\n  key-red id=kr x=0 y=0\n  key-blue id=kb x=0 y=1\n  chicken id=hen x=3 y=3 dir=1\n";

fn level(body: &str) -> String {
    format!("{HEAD}{MAP}{body}")
}

/// A level with the given rules, both doors locked (so every switched tile is bound), both keys and a chicken.
fn ruled(rules: &str) -> String {
    format!(
        "{HEAD}lock red key=red at=2,1\nlock blue key=blue at=1,0\nstate lift values=down,mid,up\n{rules}{MAP}{KEYS}"
    )
}

fn codes(src: &str) -> BTreeSet<Code> {
    check(src).diagnostics.iter().map(|d| d.code).collect()
}

/// The `n`th rule of a valid script, expanded.
fn rule(src: &str, n: usize) -> expand::Rule {
    let r = check(src);
    assert!(!r.has_errors(), "{:?}", r.diagnostics);
    r.expansion.expect("no errors").rules[n].clone()
}

#[test]
fn x_y_name_a_cell_and_at_is_exact() {
    let src = level(
        "lock red key=red at=2,1\nlock blue key=blue at=1,0\nobjects\n  cheezie id=a x=3 y=2\n  cheezie id=b at=3,2\n  cheezie id=c at=2.5,1.25\n",
    );
    let l = lower(&src).unwrap();
    let pts: Vec<(f64, f64)> = l
        .objects
        .iter()
        .map(|o| match o {
            Object::Snack { x, y, .. } => (*x, *y),
            _ => unreachable!(),
        })
        .collect();
    // A cell is its centre; `at=` is the point itself; `y` counts up from the bottom row.
    assert_eq!(pts, [(3.5, 2.5), (3.0, 2.0), (2.5, 1.25)]);
}

#[test]
fn the_printer_writes_a_cell_form_only_for_an_exact_cell_centre() {
    let src = level(
        "lock red key=red at=2,1\nlock blue key=blue at=1,0\nobjects\n  cheezie id=a at=3.5,2.5\n  cheezie id=b x=1 y=0\n  cheezie id=c at=3,2\n  cheezie id=d at=2.5,1.25\n  cheezie id=e at=0.5,0.5\n",
    );
    let out = format(&src).unwrap();
    for line in [
        "  cheezie id=a x=3 y=2\n",
        "  cheezie id=b x=1 y=0\n",
        "  cheezie id=c at=3,2\n",
        "  cheezie id=d at=2.5,1.25\n",
        "  cheezie id=e x=0 y=0\n",
    ] {
        assert!(out.contains(line), "{line:?} missing from\n{out}");
    }
    assert_eq!(lower(&src).unwrap().digest(), lower(&out).unwrap().digest());
}

#[test]
fn a_lock_is_one_state_one_door_binding_and_one_touch_rule_over_the_whole_door() {
    // Any cell of the door's run gives the same records.
    for cell in ["2,1", "2,2"] {
        let l = lower(&ruled_locks(&format!("lock red key=red at={cell}\n"))).unwrap();
        assert_eq!(l.states[0], StateRec::new(2, 0));
        assert_eq!(l.bindings[0], BindingRec::door(0, 0b10, (2, 1, 2, 2)));
        let rect = RuleRec::union_rect(2, 1, 2, 2);
        assert_eq!(
            l.rules[0],
            RuleRec {
                trigger: TRIGGER_TOUCH,
                u0: rect.0,
                u1: rect.1,
                cond: [CondRec::state_in(0, 0b01), CondRec::holds(KEY_RED)],
                act: [ActionRec::set(0, 1), ActionRec::spend_key(KEY_RED)],
                ..RuleRec::EMPTY
            }
        );
    }
}

/// The level with a blue lock and the given red lock lines.
fn ruled_locks(red: &str) -> String {
    level(&format!("{red}lock blue key=blue at=1,0\n{KEYS}"))
}

#[test]
fn a_lock_and_its_three_lines_lower_to_one_level() {
    let sugar = ruled_locks("lock red key=red at=2,2\n");
    let written = ruled_locks(
        "state red values=closed,open\ndoor x=2 y=1..2 state=red open=open\nwhen touch x=2 y=1..2 if state red is closed and holding key-red then set red open, spend key-red\n",
    );
    let (a, b) = (lower(&sugar).unwrap(), lower(&written).unwrap());
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.rules, b.rules);
    // The printer keeps the sugar as one line.
    assert!(
        format(&sugar)
            .unwrap()
            .contains("lock red key=red at=2,2\n")
    );
}

#[test]
fn a_lock_off_a_door_or_on_a_door_that_is_not_a_rectangle_is_e110() {
    assert!(codes(&ruled_locks("lock red key=red at=0,0\n")).contains(&Code::E110));
    assert!(codes(&ruled_locks("lock red key=red at=3,0\n")).contains(&Code::E110));
    // An L of door tiles is one run, and not a rectangle.
    let l = "script fizz2 0\nlevel \"L\" 3x2 id=l\nstart 0,1\nlock a key=red at=0,0\nmap\n  |D..|  1\n  |DD.|  0\nobjects\n  key-red id=k x=2 y=1\n";
    let r = check(l);
    let e = r
        .diagnostics
        .iter()
        .find(|d| d.code == Code::E110)
        .expect("E110");
    assert_eq!(
        e.fix.as_deref(),
        Some("write `door` lines and a `when` rule")
    );
}

#[test]
fn channels_are_not_in_the_dialect() {
    assert!(codes(&level("channel 1 on\n")).contains(&Code::E102));
    let alias = codes(&ruled_locks("lock red key=red at=2,1 channel=1\n"));
    assert!(alias.contains(&Code::E102), "{alias:?}");
    let r = check(&level("channel 1 on\n"));
    let d = r.diagnostics.iter().find(|d| d.code == Code::E102).unwrap();
    assert_eq!(
        d.fix.as_deref(),
        Some("declare a `state` and bind it with `door`")
    );
}

#[test]
fn the_lock_options_are_reserved() {
    for opt in ["keep", "id=sw", "key=red,blue"] {
        let src = ruled_locks(&format!("lock red key=red at=2,1 {opt}\n"))
            .replace("key=red at=2,1 key=red,blue", "at=2,1 key=red,blue");
        assert!(
            codes(&src).contains(&Code::E102),
            "{opt}: {:?}",
            codes(&src)
        );
    }
    // A cell has no fraction.
    assert!(codes(&ruled_locks("lock red key=red at=2.5,1\n")).contains(&Code::E009));
}

#[test]
fn a_state_prints_short_and_a_door_lists_its_values_in_value_order() {
    let src = "script fizz2 0\nlevel \"T\" 2x1 id=t\nstart 0,0\nstate a values=off,on start=off\nstate b values=x,y,z start=x\nstate c values=x,y,z start=z\ndoor x=0..1 y=0 state=b open=z,x,z\nmap\n  |DD|  0\n";
    let out = format(src).unwrap();
    assert!(out.contains("\nstate a\n"), "{out}");
    assert!(out.contains("\nstate b values=x,y,z\n"), "{out}");
    assert!(out.contains("\nstate c values=x,y,z start=z\n"), "{out}");
    assert!(
        out.contains("\ndoor x=0..1 y=0 state=b open=x,z\n"),
        "{out}"
    );
    let l = lower(src).unwrap();
    assert_eq!(l.bindings[0], BindingRec::door(1, 0b101, (0, 0, 1, 0)));
    assert_eq!(l.states[2], StateRec::new(3, 2));
    assert_eq!(lower(&out).unwrap().digest(), l.digest());
}

#[test]
fn a_state_has_two_to_eight_distinct_values_and_starts_at_one_of_them() {
    let one = level("state a values=only\nlock red key=red at=2,1\nlock blue key=blue at=1,0\n");
    assert!(codes(&one).contains(&Code::E109));
    let twice = level("state a values=x,x\nlock red key=red at=2,1\nlock blue key=blue at=1,0\n");
    assert!(codes(&twice).contains(&Code::E105));
    let start = level("state a start=maybe\nlock red key=red at=2,1\nlock blue key=blue at=1,0\n");
    assert!(codes(&start).contains(&Code::E109));
}

#[test]
fn rules_resolve_names_to_numbers() {
    // Objects are numbered in construction order: the keys, then the chicken.
    let r = rule(
        &ruled("when stomp id=hen if not holding key-blue then score 100, next lift\n"),
        2,
    );
    let rec = r.to_rec().unwrap();
    assert_eq!((rec.trigger, rec.subject), (TRIGGER_STOMP, 2));
    assert_eq!(
        rec.cond[0],
        CondRec {
            kind: COND_NOT_HOLDS,
            a: KEY_BLUE,
            b: 0
        }
    );
    assert_eq!((rec.act[0].op, rec.act[0].n), (OP_SCORE, 100));
    assert_eq!((rec.act[1].op, rec.act[1].a), (OP_NEXT, 2));

    // `is not` is the complement within the state's values.
    let r = rule(
        &ruled("when state lift changes if state lift is not mid then toggle red\n"),
        2,
    );
    let rec = r.to_rec().unwrap();
    assert_eq!(
        (rec.trigger, rec.subject, rec.arg),
        (TRIGGER_BECOMES, 2, ARG_ANY)
    );
    assert_eq!(rec.cond[0], CondRec::state_in(2, 0b101));
    assert_eq!((rec.act[0].op, rec.act[0].a), (OP_TOGGLE, 0));

    // A time rule repeats exactly when it has `every=`.
    let r = rule(
        &ruled("when time at=1 every=60 after=10 then set lift up repeat\n"),
        2,
    );
    let rec = r.to_rec().unwrap();
    assert_eq!((rec.trigger, rec.u0, rec.u1), (TRIGGER_TIME, 1, 60));
    assert_eq!((rec.flags, rec.delay), (FLAG_REPEAT, 10));
    assert_eq!(rec.act[0], ActionRec::set(2, 2));
    assert!(codes(&ruled("when time at=1 every=60 then set lift up\n")).contains(&Code::E109));
    assert!(
        codes(&ruled("when time at=1 every=5 then set lift up repeat\n")).contains(&Code::E109)
    );

    // A word names a level string as caption `0x40 + i`.
    let src = ruled("string hi \"Hi\"\nwhen stomp any then word hi style=red at=1,2\n");
    let rec = rule(&src, 2).to_rec().unwrap();
    assert_eq!(rec.subject, SUBJECT_ANY);
    assert_eq!(
        (rec.act[0].op, rec.act[0].a, rec.act[0].b),
        (OP_WORD, 0x40, 2)
    );
    assert_eq!((rec.act[0].x, rec.act[0].y), (1, 2));
}

#[test]
fn the_validator_holds_rules_to_the_record() {
    let cases = [
        ("when start then set nothing on\n", Code::E002),
        ("when start then set lift sideways\n", Code::E002),
        ("when start then toggle lift\n", Code::E109),
        (
            "when start if state lift is not down,mid,up then next lift\n",
            Code::E109,
        ),
        ("when start then score 0\n", Code::E109),
        ("when start then ammo 100\n", Code::E109),
        (
            "when start then next lift, next lift, next lift\n",
            Code::E109,
        ),
        ("when take id=hen then next lift\n", Code::E109),
        ("when stun id=kr then next lift\n", Code::E109),
        ("when touch x=0..9 y=0 then next lift\n", Code::E117),
        ("when start then cue door at=9,9\n", Code::E117),
        ("when use id=lever then next lift\n", Code::E002),
        ("when hit id=lever then next lift\n", Code::E002),
        ("when cleared group=hens then next lift\n", Code::E002),
        (
            "when start if holding key-mauve then next lift\n",
            Code::E002,
        ),
        ("when hurt then next lift\n", Code::E102),
        ("when use at=1,1 then next lift\n", Code::E102),
        ("when stomp kind=bat then next lift\n", Code::E102),
        ("when start if ammo then next lift\n", Code::E102),
        ("when start then give key-green\n", Code::E102),
        ("when start then\n", Code::E003),
        ("when start next lift\n", Code::E009),
        ("when start then jump\n", Code::E009),
        ("when touch x=2..1 y=0 then next lift\n", Code::E004),
        ("when start then word \"a\u{1F600}\" at=1,1\n", Code::E112),
    ];
    for (rule, want) in cases {
        let got = codes(&ruled(rule));
        assert!(got.contains(&want), "{rule:?}: wanted {want}, got {got:?}");
    }
}

#[test]
fn a_repeating_touch_that_toggles_is_w114() {
    assert!(codes(&ruled("when touch x=0 y=3 then next lift repeat\n")).contains(&Code::W114));
    assert!(!codes(&ruled("when enter x=0 y=3 then next lift repeat\n")).contains(&Code::W114));
}

#[test]
fn a_rule_that_opens_a_locked_door_is_w123() {
    let src = ruled("when enter x=0 y=3 then set red open\n");
    assert!(codes(&src).contains(&Code::W123));
    // Two locks on one door bind its tiles twice, which the pack refuses.
    let two = ruled_locks("lock red key=red at=2,1\nlock red-2 key=red at=2,2\n");
    let c = codes(&two);
    assert!(c.contains(&Code::W123) && c.contains(&Code::E110), "{c:?}");
}

#[test]
fn a_valid_rule_the_simulation_cannot_run_yet_is_unbuilt_and_not_an_error() {
    let src = ruled("when enter x=0 y=3 then next lift\n");
    let r = check(&src);
    assert!(!r.has_errors(), "{:?}", r.diagnostics);
    assert!(r.lowered.is_none());
    let u = r.unbuilt.expect("unbuilt");
    assert_eq!(u.line, 7);
    match lower(&src) {
        Err(LowerError::Unbuilt(e)) => assert_eq!(e, u),
        other => panic!("{other:?}"),
    }
    // It still expands, so the dump can show it, and it formats.
    assert_eq!(r.expansion.unwrap().rules.len(), 3);
    assert!(format(&src).is_ok());

    // A cue or a word without a level string has no name table to lower through.
    for (text, caption) in [
        ("when start then cue door at=1,1\n", None),
        (
            "when start then word bonk at=1,1\n",
            Some(Caption::Unresolved("bonk".into())),
        ),
        (
            "when start then word \"Hi\" at=1,1\n",
            Some(Caption::Text("Hi".into())),
        ),
    ] {
        let r = check(&ruled(text));
        assert!(!r.has_errors(), "{text}: {:?}", r.diagnostics);
        let u = r.unbuilt.expect(text);
        assert_eq!(u.line, 7, "{text}");
        if let Some(c) = caption {
            let x = r.expansion.unwrap();
            assert!(
                matches!(&x.rules[2].actions[0], expand::Action::Word { caption, .. } if *caption == c)
            );
        }
    }
}

#[test]
fn a_rule_round_trips_through_the_printer() {
    let src = ruled(
        "when touch at=1, 3 if state lift is up,down and not holding key-red after=12 then set lift mid , give key-red repeat\n",
    );
    let out = format(&src).unwrap();
    assert!(
        out.contains("\nwhen touch at=1,3 if state lift is up,down and not holding key-red after=12 then set lift mid, give key-red repeat\n"),
        "{out}"
    );
    assert_eq!(format(&out).unwrap(), out);
    assert_eq!(rule(&src, 2), rule(&out, 2));
}

#[test]
fn records_carry_only_what_the_rule_says() {
    let r = rule(&ruled("when stomp id=hen then word \"Hi\" at=1,1\n"), 2);
    assert!(r.to_rec().is_err());
    let r = rule(&ruled("when start then toggle red\n"), 2);
    let rec = r.to_rec().unwrap();
    assert_eq!(rec.act[1], ActionRec::EMPTY);
    assert_eq!(rec.cond, [CondRec::EMPTY; 2]);
    assert_eq!(rec.act[0].op, OP_TOGGLE);
}
