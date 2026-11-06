// Round trips: the lossless printer returns every input, and the canonical printer is stable and keeps the meaning.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_script::{check, format, parse, print_canonical, print_lossless};
use std::fs;
use std::path::PathBuf;

fn corpus_texts() -> Vec<String> {
    let mut out = Vec::new();
    for d in ["levels", "errors", "lints", "fmt"] {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus")
            .join(d);
        for e in fs::read_dir(p).unwrap() {
            out.push(fs::read_to_string(e.unwrap().path()).unwrap());
        }
    }
    out
}

/// A small deterministic generator, so the soup of tokens is the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() as usize) % xs.len()]
    }
}

const PIECES: [&str; 58] = [
    "script",
    "fizz2",
    "0",
    "level",
    "\"T\"",
    "4x3",
    "id=t",
    "start",
    "1,1",
    "2.5,3",
    "floor",
    "open",
    "map",
    "objects",
    "|..D.|",
    "|####|",
    "|",
    "lock",
    "key=red",
    "channel=1",
    "at=2,1",
    "channel",
    "state",
    "values=a,b,c",
    "start=b",
    "door",
    "x=2",
    "y=1..2",
    "open=on",
    "when",
    "touch",
    "then",
    "if",
    "and",
    "set",
    "toggle",
    "repeat",
    "after=5",
    "holding",
    "key-red",
    "cheezie",
    "chicken",
    "dir=-1",
    "x=1",
    "y=0",
    "at=3.5,0.5",
    "string",
    "\"hi // there\"",
    "style=cyan",
    "// note",
    "theme",
    "ground=4",
    "\t",
    "  ",
    "\"",
    ",",
    "e5",
    "id=a",
];

/// A file's slots in order, each with the lines that can fill it. The soup picks one line for most slots, so a good share of
/// the files parse; the rest get a loose piece, a repeated line or a missing one.
const SLOTS: [&[&str]; 19] = [
    &["script fizz2 0", "script   fizz2 0 // dialect"],
    &[
        "level \"T\" 4x3 id=t",
        "level  \"Odd  name\"   4x3   id=odd-1 // note",
    ],
    &["start 1,1", "start 2.50, 0.7", "start 0,0"],
    &["floor open", "floor solid", ""],
    &[
        "state lift values=down, up, top start=up",
        "state lift values=down,up,top",
        "// a comment",
    ],
    &[
        "lock gate key=red at=2,1",
        "lock g2 key=blue at=2,1",
        "door x=2 y=1 state=lift open=top,up,up",
        "door y=1 x=2 state=lift open=up",
    ],
    &[
        "when touch x=0..1 y=1 if state lift is not down and holding key-red then set lift up, spend key-red",
        "when enter x=3 y=2 after=30 then next lift repeat",
        "when time every=60 at=1 then next lift repeat",
        "when state lift becomes up then word hi style=red at=1,2",
        "when stomp id=hen then score 100,  ammo 5",
        "when touch id=k then cue door at=2,1",
        "",
    ],
    &[
        "string hi \"Hi // there\" style=red",
        "string yo \"Yo\"",
        "",
    ],
    &["theme night", "", "// about"],
    &["map"],
    &["  |....|", "|....|  2"],
    &["\t|..D.|  1", "|..D.|", "  |..D.|  1 // door"],
    &["  |####|  0 // base", "|####|"],
    &["", "// a comment"],
    &["objects"],
    &["key-red id=k at=0.5,0.5", "key-red id=k x=0 y=0"],
    &["cheezie id=c1 x=1 y=1", "choc id=c1 at=1.25,1.5", ""],
    &[
        "chicken id=hen at=3,2 dir=-1",
        "chicken id=hen x=3 y=2 dir=1",
    ],
    &["soda id=can at=2.25,0.5", "cookie id=c1 at=3.5,0.5", ""],
];

fn soup(seed: u64) -> String {
    let mut g = Lcg(seed);
    let mut s = String::new();
    for slot in SLOTS {
        let r = g.next() % 20;
        if r == 0 {
            continue;
        }
        if r == 1 {
            for _ in 0..(g.next() % 6) {
                s.push_str(g.pick(&PIECES));
                s.push(if g.next().is_multiple_of(5) {
                    '\t'
                } else {
                    ' '
                });
            }
            s.push('\n');
        }
        s.push_str(g.pick(slot));
        if g.next().is_multiple_of(9) {
            s.push(' ');
        }
        s.push_str(g.pick(&["\n", "\n", "\n", "\r\n", "\n\n"]));
    }
    if g.next().is_multiple_of(3) {
        s.pop();
    }
    s
}

#[test]
fn the_lossless_printer_returns_every_input_byte_for_byte() {
    let mut inputs = corpus_texts();
    let more: Vec<String> = inputs
        .iter()
        .flat_map(|t| {
            [
                t.replace('\n', "\r\n"),
                t.trim_end().to_string(),
                t.replace("  ", "\t"),
                format!("\n\n{t}\n\n"),
            ]
        })
        .collect();
    inputs.extend(more);
    inputs.extend((0..400).map(soup));
    inputs.extend(["", "\n", "\r", "x", "|", "\"", "//", "  \t \n"].map(String::from));
    for t in &inputs {
        let p = parse(t);
        assert_eq!(print_lossless(&p.ast), *t, "lossless failed for {t:?}");
    }
}

#[test]
fn the_canonical_form_is_stable() {
    let mut inputs = corpus_texts();
    inputs.extend((0..400).map(soup));
    let mut formatted = 0;
    for t in &inputs {
        let Ok(once) = format(t) else { continue };
        formatted += 1;
        // Printing the canonical text again gives the same text.
        assert_eq!(
            format(&once).as_deref(),
            Ok(once.as_str()),
            "not stable for {t:?}"
        );
        // So does printing the tree of the canonical text without going through `format`.
        assert_eq!(print_canonical(&parse(&once).ast), once);
        // Canonical text ends in one newline and has no trailing white space or doubled blank lines.
        assert!(once.is_empty() || once.ends_with('\n') && !once.ends_with("\n\n"));
        assert!(once.lines().all(|l| l == l.trim_end()));
    }
    assert!(formatted > 100, "only {formatted} inputs formatted");
}

#[test]
fn canonical_text_keeps_the_meaning() {
    let mut lowered = 0;
    for t in corpus_texts().into_iter().chain((0..400).map(soup)) {
        let before = check(&t);
        let Ok(once) = format(&t) else { continue };
        let after = check(&once);
        let codes =
            |r: &fizz_script::Report| r.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>();
        let mut a = codes(&before);
        // Formatting writes the right row labels, so a wrong label is the one finding it can remove (the code stays if another
        // row is wrong).
        let only_labels = before
            .diagnostics
            .iter()
            .filter(|d| d.code == fizz_script::Code::E103)
            .all(|d| d.message.starts_with("this row is y"));
        if only_labels {
            a.retain(|c| *c != fizz_script::Code::E103);
        }
        let mut b = codes(&after);
        a.sort();
        a.dedup();
        b.sort();
        b.dedup();
        assert_eq!(a, b, "diagnostics changed for {t:?}");
        assert_eq!(
            before.lowered.as_ref().map(|l| l.digest()),
            after.lowered.as_ref().map(|l| l.digest()),
            "digest changed for {t:?}"
        );
        assert_eq!(
            before.unbuilt.as_ref().map(|u| &u.reason),
            after.unbuilt.as_ref().map(|u| &u.reason),
            "lowering changed for {t:?}"
        );
        // A valid script either lowers or is a valid script the game cannot build yet; both went through the lowering.
        lowered += usize::from(!after.has_errors());
    }
    assert!(lowered > 20, "only {lowered} inputs reached the lowering");
}

#[test]
fn the_parser_never_panics_on_odd_input() {
    for seed in 1000..3000 {
        let t = soup(seed);
        let r = check(&t);
        // A script with an error never lowers.
        if r.has_errors() {
            assert!(r.lowered.is_none(), "{t:?}");
        }
    }
}

#[test]
fn crlf_input_formats_to_lf() {
    let t = "script fizz2 0\r\nlevel \"T\" 1x1 id=t\r\nstart 0,0\r\nmap\r\n  |.|\r\n";
    let out = format(t).unwrap();
    assert!(!out.contains('\r'));
    assert_eq!(
        out,
        "script fizz2 0\nlevel \"T\" 1x1 id=t\nstart 0,0\nmap\n  |.|  0\n"
    );
}
