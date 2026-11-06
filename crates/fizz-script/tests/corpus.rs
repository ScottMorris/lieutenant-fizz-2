// The conformance corpus: valid levels, one file for every error and lint code, and the formatter's pairs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_ep2::level::{self, SLICE_SOURCE};
use fizz_script::{Code, Severity, check, format, lower, print_canonical, print_lossless};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("corpus")
        .join(name)
}

/// The files of a corpus directory, sorted, as `(file name, text)`.
fn files(name: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(dir(name))
        .unwrap_or_else(|e| panic!("corpus/{name}: {e}"))
        .map(|e| e.unwrap())
        .filter(|e| e.path().extension().is_some_and(|x| x == "fizz"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

fn code_of(file: &str) -> Code {
    let stem = &file[..4];
    *Code::ALL
        .iter()
        .find(|c| c.as_str() == stem)
        .unwrap_or_else(|| panic!("{file} does not start with a code"))
}

#[test]
fn the_valid_levels_are_clean_canonical_and_lower() {
    let levels = files("levels");
    assert!(levels.len() >= 2);
    for (name, text) in levels {
        let r = check(&text);
        assert!(r.diagnostics.is_empty(), "{name}: {:?}", r.diagnostics);
        assert_eq!(print_lossless(&r.ast), text, "{name}: lossless");
        assert_eq!(
            print_canonical(&r.ast),
            text,
            "{name} is not in canonical form"
        );
        assert!(r.lowered.is_some(), "{name}");
    }
}

#[test]
fn the_slice_script_has_the_digest_of_the_const_slice() {
    let text = fs::read_to_string(dir("levels").join("slice.fizz")).unwrap();
    let lowered = lower(&text).expect("the slice script lowers");
    let src = lowered.source();
    assert_eq!(level::digest(&src), level::digest(&SLICE_SOURCE));

    // The digest is the proof; these say which field would differ.
    assert_eq!(src.name, SLICE_SOURCE.name);
    assert_eq!((src.w, src.h), (SLICE_SOURCE.w, SLICE_SOURCE.h));
    assert_eq!(src.tiles, SLICE_SOURCE.tiles);
    assert_eq!(src.floor_solid, SLICE_SOURCE.floor_solid);
    assert_eq!(src.spawn, SLICE_SOURCE.spawn);
    assert_eq!(src.states, SLICE_SOURCE.states);
    assert_eq!(src.bindings, SLICE_SOURCE.bindings);
    assert_eq!(src.objects, SLICE_SOURCE.objects);
    assert_eq!(src.rules, SLICE_SOURCE.rules);
    assert_eq!(src.strings, SLICE_SOURCE.strings);

    // The same starting world.
    let from_script = level::world_from(&src).expect("the world builds");
    assert_eq!(
        from_script.hash().finish(),
        level::world().hash().finish(),
        "the world built from the script hashes as the const slice does"
    );
}

#[test]
fn every_error_file_gives_its_code_and_the_codes_it_lists() {
    let mut covered = BTreeSet::new();
    for (name, text) in files("errors") {
        let mut want: BTreeSet<Code> = BTreeSet::from([code_of(&name)]);
        if let Some(list) = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("// expect:"))
        {
            for w in list.split_whitespace() {
                want.insert(*Code::ALL.iter().find(|c| c.as_str() == w).expect(w));
            }
        }
        let r = check(&text);
        let got: BTreeSet<Code> = r
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.code)
            .collect();
        assert_eq!(got, want, "{name}: {:?}", r.diagnostics);
        assert!(r.lowered.is_none(), "{name} must not lower");
        assert_eq!(print_lossless(&r.ast), text, "{name}: lossless");
        covered.insert(code_of(&name));
    }
    for c in Code::ALL {
        if c.severity() == Severity::Error && *c != Code::E199 {
            assert!(covered.contains(c), "no corpus file for {c}");
        }
    }
}

#[test]
fn every_lint_file_gives_only_its_warning() {
    let mut covered = BTreeSet::new();
    for (name, text) in files("lints") {
        let code = code_of(&name);
        let r = check(&text);
        let got: BTreeSet<Code> = r.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(got, BTreeSet::from([code]), "{name}: {:?}", r.diagnostics);
        // A lint does not stop lowering; a rule the simulation cannot run yet does, without a diagnostic.
        assert!(
            r.lowered.is_some() || r.unbuilt.is_some(),
            "{name}: neither lowered nor unbuilt"
        );
        covered.insert(code);
    }
    for c in Code::ALL {
        if c.severity() == Severity::Warning {
            assert!(covered.contains(c), "no corpus file for {c}");
        }
    }
}

#[test]
fn the_formatter_pairs_print_canonically_and_keep_their_meaning() {
    let mut n = 0;
    for (name, input) in files("fmt") {
        let Some(stem) = name.strip_suffix(".in.fizz") else {
            continue;
        };
        let want = fs::read_to_string(dir("fmt").join(format!("{stem}.out.fizz"))).unwrap();
        let got = format(&input).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        assert_eq!(got, want, "{name}");
        assert_eq!(format(&got).unwrap(), got, "{name}: not stable");
        let before = lower(&input).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let after = lower(&got).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(before.digest(), after.digest(), "{name}: meaning changed");
        n += 1;
    }
    assert!(n >= 1);
}

#[test]
fn the_catalogue_in_the_doc_lists_every_code() {
    let doc =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/SCRIPT.md"))
            .unwrap();
    for c in Code::ALL {
        assert!(
            doc.contains(&format!("| {c} |")),
            "docs/SCRIPT.md does not list {c}"
        );
    }
    // A retired code stays in the catalogue, marked retired, and the crate never produces it.
    for c in fizz_script::diag::RETIRED {
        assert!(
            doc.contains(&format!("| {c} | Retired")),
            "docs/SCRIPT.md does not mark {c} retired"
        );
        assert!(Code::ALL.iter().all(|k| k.as_str() != c));
    }
}

#[test]
fn the_slice_dump_shows_each_lock_as_its_three_records() {
    let text = fs::read_to_string(dir("levels").join("slice.fizz")).unwrap();
    let r = check(&text);
    let x = r.expansion.expect("the slice expands");
    let d = fizz_script::dump("slice.fizz", &x);
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(
        lines[0],
        "// dump: slice.fizz — states 2/32, bindings 2/64 (2 door), rules 2/32, strings 0/32"
    );
    assert!(lines[1].starts_with("state 0   red-door   values=closed:0,open:1 start=closed"));
    assert!(lines[2].starts_with("state 1   blue-door  values=closed:0,open:1 start=closed"));
    assert!(lines[3].starts_with("door  0   x=15 y=20..22 state=0 open={1}"));
    assert!(lines[4].starts_with("door  1   x=22 y=38..40 state=1 open={1}"));
    assert!(lines[5].starts_with(
        "rule  0   touch x=15 y=20..22 if s0 in {0} and holding red then set s0 1, spend red "
    ));
    assert!(lines[6].starts_with(
        "rule  1   touch x=22 y=38..40 if s1 in {0} and holding blue then set s1 1, spend blue "
    ));
    // Every line names the lock it came from.
    let lock_line = |name: &str| {
        text.lines()
            .position(|l| l.starts_with(&format!("lock {name} ")))
            .unwrap()
            + 1
    };
    for (i, name) in [
        (1, "red-door"),
        (3, "red-door"),
        (5, "red-door"),
        (2, "blue-door"),
        (6, "blue-door"),
    ] {
        assert!(
            lines[i].ends_with(&format!("// lock, line {}", lock_line(name))),
            "{}",
            lines[i]
        );
    }
}
