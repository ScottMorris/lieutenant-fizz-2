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
    assert_eq!(src.switches, SLICE_SOURCE.switches);
    assert_eq!(src.floor_solid, SLICE_SOURCE.floor_solid);
    assert_eq!(src.spawn, SLICE_SOURCE.spawn);
    assert_eq!(src.objects, SLICE_SOURCE.objects);
    assert_eq!(src.rules, SLICE_SOURCE.rules);
    assert_eq!(src.strings.len(), 0);

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
        assert!(r.lowered.is_some(), "{name}: a lint does not stop lowering");
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
        let before = lower(&input).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let after = lower(&got).unwrap();
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
}
