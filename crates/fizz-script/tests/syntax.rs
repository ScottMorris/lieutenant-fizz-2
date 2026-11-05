// The forms the Level Studio writes: positions (`x= y=` for a cell, `at=` for an exact point) and `lock NAME key=COLOUR at=X,Y`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_ep2::level::Object;
use fizz_ep2::rules::{KEY_BLUE, KEY_RED, Rule};
use fizz_script::{Code, check, format, lower};
use std::collections::BTreeSet;

const HEAD: &str = "script fizz2 0\nlevel \"T\" 4x4 id=t\nstart 1,1\n";
/// A red door at x 2, y 1 and 2; a blue door at x 1, y 0.
const MAP: &str = "map\n  |....|  3\n  |..D.|  2\n  |..D.|  1\n  |.b##|  0\n";
const KEYS: &str = "objects\n  key-red id=kr x=0 y=0\n  key-blue id=kb x=0 y=1\n";

fn level(body: &str) -> String {
    format!("{HEAD}{MAP}{body}")
}

/// A level with a lock section and both keys, so the key lints stay out of the way.
fn locked(locks: &str) -> String {
    level(&format!("{locks}{KEYS}"))
}

fn codes(src: &str) -> BTreeSet<Code> {
    check(src).diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn x_y_name_a_cell_and_at_is_exact() {
    let src = level(
        "objects\n  cheezie id=a x=3 y=2\n  cheezie id=b at=3,2\n  cheezie id=c at=2.5,1.25\n",
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
        "objects\n  cheezie id=a at=3.5,2.5\n  cheezie id=b x=1 y=0\n  cheezie id=c at=3,2\n  cheezie id=d at=2.5,1.25\n  cheezie id=e at=0.5,0.5\n",
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
fn a_lock_takes_its_channel_from_the_door_tile_at_any_cell_of_the_door() {
    let one = lower(&locked("lock a key=red at=2,1\n")).unwrap();
    let other = lower(&locked("lock a key=red at=2,2\n")).unwrap();
    // Both cells are the red door, channel 1; the record keeps the cell that was named.
    assert_eq!(one.rules[0], Rule::lock(KEY_RED, 1, (2, 1)).to_rec());
    assert_eq!(other.rules[0], Rule::lock(KEY_RED, 1, (2, 2)).to_rec());
    let blue = lower(&locked("lock b key=blue at=1,0\n")).unwrap();
    assert_eq!(blue.rules[0], Rule::lock(KEY_BLUE, 2, (1, 0)).to_rec());
}

#[test]
fn a_lock_off_a_door_is_e110() {
    let on_sky = codes(&locked("lock a key=red at=0,0\n"));
    assert!(on_sky.contains(&Code::E110), "{on_sky:?}");
    let on_stone = codes(&locked("lock a key=red at=2,0\n"));
    assert!(on_stone.contains(&Code::E110), "{on_stone:?}");
    assert!(lower(&locked("lock a key=red at=2,0\n")).is_err());
}

#[test]
fn the_channel_alias_is_read_dropped_by_the_printer_and_checked_against_the_tile() {
    let src = locked("lock a key=red at=2,1 channel=1\n");
    let found = codes(&src);
    assert!(found.contains(&Code::W122), "{found:?}");
    assert!(!found.contains(&Code::E110));
    let out = format(&src).unwrap();
    assert!(out.contains("lock a key=red at=2,1\n"), "{out}");
    assert!(!out.contains("channel="));
    assert_eq!(lower(&src).unwrap().digest(), lower(&out).unwrap().digest());

    let wrong = locked("lock a key=red at=2,1 channel=2\n");
    assert!(codes(&wrong).contains(&Code::E110));
    assert!(
        format(&wrong).is_err(),
        "fmt must not hide a disagreeing alias"
    );
}

#[test]
fn two_cells_of_one_door_are_a_warning_and_still_lower() {
    let src = locked("lock a key=red at=2,1\nlock b key=red at=2,2\n");
    assert!(codes(&src).contains(&Code::W123));
    let l = lower(&src).expect("the simulation allows two locks on a door");
    assert_eq!(l.rules.len(), 2);
    // The formatter still formats it.
    assert!(format(&src).is_ok());
}
