// The parser: text to a lossless tree and the syntax diagnostics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ast::{
    Action, Ast, Body, Cond, Eol, Input, Line, ObjectKind, Pos, Range, Rect, Stmt, Subject, When,
    WordText,
};
use crate::diag::{Code, Diagnostic};
use crate::lex::{Tok, lex};

/// The longest name (an `id`, a state or lock name, a string name).
pub const NAME_MAX: usize = 32;

/// Statement words that start a line of their own.
const TOP_WORDS: [&str; 12] = [
    "script", "level", "theme", "start", "floor", "state", "door", "when", "lock", "string", "map",
    "objects",
];
/// Statement words reserved for later stages (`docs/SCRIPT.md`, "Extension points").
const RESERVED_TOP: [&str; 3] = ["music", "allow", "world"];
/// Object words reserved for later stages: the objects the pack reserves, other key colours, and custom enemies and items.
const RESERVED_OBJECT: [&str; 9] = [
    "light",
    "switch",
    "platform",
    "key-green",
    "key-gold",
    "usb",
    "gadget",
    "enemy",
    "item",
];
/// The fix for a Fizz 1 channel, which the dialect does not have.
const CHANNEL_FIX: &str = "declare a `state` and bind it with `door`";

/// What a parse gives back: the tree and the syntax diagnostics. The tree holds every line, bad ones included.
#[derive(Clone, Debug, PartialEq)]
pub struct Parse {
    pub ast: Ast,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Map,
    Objects,
}

struct Fail {
    code: Code,
    col: usize,
    msg: String,
    fix: Option<String>,
}

type R<T> = Result<T, Fail>;

fn fail<T>(code: Code, col: usize, msg: impl Into<String>) -> R<T> {
    Err(Fail {
        code,
        col,
        msg: msg.into(),
        fix: None,
    })
}

fn fail_fix<T>(code: Code, col: usize, msg: impl Into<String>, fix: impl Into<String>) -> R<T> {
    Err(Fail {
        code,
        col,
        msg: msg.into(),
        fix: Some(fix.into()),
    })
}

struct State {
    started: bool,
    saw_level: bool,
    flagged_order: bool,
    seen: Vec<&'static str>,
    section: Section,
}

/// Parses a script. It never fails: a line that does not parse stays in the tree as a bad line, with a diagnostic.
pub fn parse(src: &str) -> Parse {
    let mut st = State {
        started: false,
        saw_level: false,
        flagged_order: false,
        seen: Vec::new(),
        section: Section::None,
    };
    let mut ast = Ast::default();
    let mut diags = Vec::new();
    for (i, (raw, eol)) in split_lines(src).into_iter().enumerate() {
        let line = parse_line(&mut st, &mut diags, i + 1, raw, eol);
        ast.lines.push(line);
    }
    if !st.saw_level && !st.flagged_order {
        diags.push(
            Diagnostic::new(Code::E007, 1, 1, "the script has no `level` header")
                .fix("add `level \"Name\" 32x48 id=name`"),
        );
    }
    crate::diag::sort(&mut diags);
    Parse {
        ast,
        diagnostics: diags,
    }
}

fn split_lines(src: &str) -> Vec<(&str, Eol)> {
    let mut out = Vec::new();
    let mut rest = src;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(i) => {
                if i > 0 && rest.as_bytes()[i - 1] == b'\r' {
                    out.push((&rest[..i - 1], Eol::CrLf));
                } else {
                    out.push((&rest[..i], Eol::Lf));
                }
                rest = &rest[i + 1..];
            }
            None => {
                out.push((rest, Eol::None));
                break;
            }
        }
    }
    out
}

fn bad(raw: &str, eol: Eol) -> Line {
    Line {
        raw: raw.to_string(),
        eol,
        body: Body::Bad,
        comment: None,
    }
}

fn report(diags: &mut Vec<Diagnostic>, line: usize, f: Fail) {
    let mut d = Diagnostic::new(f.code, line, f.col, f.msg);
    d.fix = f.fix;
    diags.push(d);
}

fn parse_line(st: &mut State, diags: &mut Vec<Diagnostic>, n: usize, raw: &str, eol: Eol) -> Line {
    if let Some(i) = raw.chars().position(|c| c.is_control() && c != '\t') {
        diags.push(Diagnostic::new(
            Code::E004,
            n,
            i + 1,
            "a control character is not allowed in a script",
        ));
        return bad(raw, eol);
    }
    let trimmed = raw.trim_start_matches([' ', '\t']);
    let indent = raw.chars().count() - trimmed.chars().count();
    if trimmed.trim_end().is_empty() {
        return Line {
            raw: raw.to_string(),
            eol,
            body: Body::Blank,
            comment: None,
        };
    }
    if let Some(text) = trimmed.strip_prefix("//") {
        return Line {
            raw: raw.to_string(),
            eol,
            body: Body::Comment,
            comment: Some(text.trim().to_string()),
        };
    }
    if trimmed.starts_with('|') {
        return match parse_row(st, trimmed, indent) {
            Ok((stmt, comment)) => Line {
                raw: raw.to_string(),
                eol,
                body: Body::Stmt(stmt),
                comment,
            },
            Err(f) => {
                report(diags, n, f);
                bad(raw, eol)
            }
        };
    }
    let lexed = lex(raw, 0);
    if let Some(col) = lexed.unterminated {
        diags.push(Diagnostic::new(
            Code::E004,
            n,
            col,
            "a string is not closed on its line",
        ));
        return bad(raw, eol);
    }
    let toks = lexed.toks;
    let first = &toks[0];
    let word = first.text.as_str();
    let col = first.col;
    if first.quoted {
        report(
            diags,
            n,
            Fail {
                code: Code::E001,
                col,
                msg: "a line cannot start with a string".into(),
                fix: None,
            },
        );
        return bad(raw, eol);
    }

    // The order of the file: the script line, then the level header.
    if !st.started {
        st.started = true;
        if word != "script" {
            st.flagged_order = true;
            diags.push(
                Diagnostic::new(
                    Code::E007,
                    n,
                    col,
                    "the file must start with the `script fizz2 0` line (a Fizz 1 script has none and is not read here)",
                )
                .fix("add `script fizz2 0` as the first line"),
            );
        }
    } else if !st.saw_level && !st.flagged_order && word != "script" && word != "level" {
        st.flagged_order = true;
        diags.push(
            Diagnostic::new(
                Code::E007,
                n,
                col,
                "a statement comes before the `level` header",
            )
            .fix("move the `level` line directly under `script fizz2 0`"),
        );
    }

    let result = if word == "channel" {
        st.section = Section::None;
        fail_fix(
            Code::E102,
            col,
            "`channel` is a Fizz 1 word: the dialect has no channels",
            CHANNEL_FIX,
        )
    } else if RESERVED_TOP.contains(&word) {
        st.section = Section::None;
        fail_fix(
            Code::E102,
            col,
            format!("`{word}` is reserved for a later stage of the dialect"),
            "remove it; version 0 does not define it yet",
        )
    } else if TOP_WORDS.contains(&word) {
        top_statement(st, diags, n, word, &toks)
    } else if let Some(kind) = ObjectKind::from_word(word) {
        if st.section == Section::Objects {
            parse_object(kind, &toks)
        } else {
            fail_fix(
                Code::E008,
                col,
                format!("`{word}` belongs under `objects`"),
                "put the line under an `objects` line",
            )
        }
    } else if RESERVED_OBJECT.contains(&word) {
        fail_fix(
            Code::E102,
            col,
            format!("`{word}` is reserved for a later stage of the dialect"),
            "remove it; version 0 has the red and blue keys, the snacks, the elixir and the chicken",
        )
    } else {
        fail_fix(
            Code::E001,
            col,
            format!("unknown statement `{word}`"),
            "the statements are script, level, theme, start, floor, state, door, when, lock, string, map and objects",
        )
    };
    match result {
        Ok(stmt) => Line {
            raw: raw.to_string(),
            eol,
            body: Body::Stmt(stmt),
            comment: lexed.comment,
        },
        Err(f) => {
            report(diags, n, f);
            bad(raw, eol)
        }
    }
}

/// Marks a once-only statement as seen; false when it was already there.
fn first_time(st: &mut State, word: &'static str) -> bool {
    if st.seen.contains(&word) {
        false
    } else {
        st.seen.push(word);
        true
    }
}

fn top_statement(
    st: &mut State,
    diags: &mut Vec<Diagnostic>,
    n: usize,
    word: &str,
    toks: &[Tok],
) -> R<Stmt> {
    let col = toks[0].col;
    st.section = Section::None;
    let once: Option<&'static str> = [
        "script", "level", "theme", "start", "floor", "map", "objects",
    ]
    .into_iter()
    .find(|w| *w == word);
    if let Some(w) = once
        && !first_time(st, w)
    {
        return fail_fix(
            Code::E006,
            col,
            format!("a second `{word}` line"),
            "keep one",
        );
    }
    match word {
        "script" => parse_script(diags, n, toks),
        "level" => {
            st.saw_level = true;
            parse_level(toks)
        }
        "theme" => {
            let name = one_word(toks, "theme")?;
            Ok(Stmt::Theme { name })
        }
        "start" => {
            let t = one_arg(toks, "start", "a pair such as `2,2`")?;
            let (x, y) = pair(&t.text, t.col)?;
            Ok(Stmt::Start { x, y })
        }
        "floor" => {
            let name = one_word(toks, "floor")?;
            Ok(Stmt::Floor { word: name })
        }
        "state" => parse_state(toks),
        "door" => parse_door(toks),
        "when" => parse_when(toks),
        "lock" => parse_lock(toks),
        "string" => parse_string(toks),
        "map" => {
            no_args(toks, "map")?;
            st.section = Section::Map;
            Ok(Stmt::Map)
        }
        "objects" => {
            no_args(toks, "objects")?;
            st.section = Section::Objects;
            Ok(Stmt::Objects)
        }
        _ => unreachable!("every top-level word is handled"),
    }
}

fn no_args(toks: &[Tok], word: &str) -> R<()> {
    match toks.get(1) {
        Some(t) => fail(
            Code::E009,
            t.col,
            format!("`{word}` takes nothing after it, found `{}`", t.text),
        ),
        None => Ok(()),
    }
}

/// The one argument of a statement, with nothing after it.
fn one_arg<'a>(toks: &'a [Tok], word: &str, what: &str) -> R<&'a Tok> {
    let Some(t) = toks.get(1) else {
        return fail(Code::E003, toks[0].col, format!("`{word}` needs {what}"));
    };
    if let Some(extra) = toks.get(2) {
        return fail(
            Code::E009,
            extra.col,
            format!("unexpected `{}` after the {word} value", extra.text),
        );
    }
    Ok(t)
}

fn one_word(toks: &[Tok], word: &str) -> R<String> {
    let t = one_arg(toks, word, "a word")?;
    check_word(&t.text, t.col, t.quoted)?;
    Ok(t.text.clone())
}

fn check_word(s: &str, col: usize, quoted: bool) -> R<()> {
    let mut cs = s.chars();
    let ok = !quoted
        && cs.next().is_some_and(|c| c.is_ascii_lowercase())
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(())
    } else {
        fail(
            Code::E004,
            col,
            format!("`{s}` is not a word: write lower-case letters, digits and hyphens"),
        )
    }
}

/// A name: `[a-z0-9][a-z0-9-]*`, at most [`NAME_MAX`] bytes.
fn check_name(s: &str, col: usize) -> R<String> {
    let mut cs = s.chars();
    let ok = cs
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok {
        return fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a name: write lower-case letters, digits and hyphens"),
            "for example `red-key`",
        );
    }
    if s.len() > NAME_MAX {
        return fail(
            Code::E109,
            col,
            format!("the name `{s}` is longer than {NAME_MAX} bytes"),
        );
    }
    Ok(s.to_string())
}

/// `-?digits` with an optional `.digits` when `frac` is set.
fn number_ok(s: &str, frac: bool) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let (int, fr) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    digits(int) && fr.is_none_or(|f| frac && digits(f))
}

fn num(s: &str, col: usize) -> R<f64> {
    if !number_ok(s, true) {
        return fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a number"),
            "write digits with an optional fraction, such as `7` or `-2.5`",
        );
    }
    s.parse::<f64>()
        .or_else(|_| fail(Code::E004, col, format!("`{s}` is not a number")))
}

fn int(s: &str, col: usize) -> R<i64> {
    if !number_ok(s, false) {
        return fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a whole number"),
            "write digits only",
        );
    }
    s.parse::<i64>()
        .or_else(|_| fail(Code::E004, col, format!("`{s}` is too large")))
}

fn uint(s: &str, col: usize) -> R<u32> {
    if s.starts_with('-') || !number_ok(s, false) {
        return fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a whole number of zero or more"),
            "write digits only",
        );
    }
    s.parse::<u32>()
        .or_else(|_| fail(Code::E004, col, format!("`{s}` is too large")))
}

fn pair(s: &str, col: usize) -> R<(f64, f64)> {
    match s.split_once(',') {
        Some((a, b)) if !b.contains(',') => Ok((num(a, col)?, num(b, col)?)),
        _ => fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a pair of numbers"),
            "write two numbers with a comma, such as `2.5,11.7`",
        ),
    }
}

/// A cell, `I,J` with whole numbers. A pair of numbers with a fraction is E009, since a cell has none.
fn ipair(s: &str, col: usize) -> R<(i64, i64)> {
    match s.split_once(',') {
        Some((a, b)) if !b.contains(',') => {
            if number_ok(a, true)
                && number_ok(b, true)
                && !(number_ok(a, false) && number_ok(b, false))
            {
                return fail_fix(
                    Code::E009,
                    col,
                    format!("`{s}` names a cell, which has no fraction"),
                    "write whole numbers, such as `15,21`",
                );
            }
            Ok((int(a, col)?, int(b, col)?))
        }
        _ => fail_fix(
            Code::E004,
            col,
            format!("`{s}` is not a pair of whole numbers"),
            "write two whole numbers with a comma, such as `15,21`",
        ),
    }
}

/// A range: an integer, or two joined by `..`, the low one first.
fn range(s: &str, col: usize) -> R<Range> {
    let (lo, hi) = match s.split_once("..") {
        Some((a, b)) => (int(a, col)?, int(b, col)?),
        None => {
            let v = int(s, col)?;
            (v, v)
        }
    };
    if lo > hi {
        return fail_fix(
            Code::E004,
            col,
            format!("the range `{s}` is backwards"),
            format!("write the low end first, `{hi}..{lo}`"),
        );
    }
    Ok(Range { lo, hi })
}

/// A list of words joined by commas.
fn word_list(s: &str, col: usize) -> R<Vec<String>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        check_word(part, col, false)?;
        out.push(part.to_string());
    }
    Ok(out)
}

/// E102 for the first token that is a reserved field (`key=` among `keys`) or a reserved bare word.
fn reserved(toks: &[Tok], keys: &[&str], words: &[&str], fix: &str) -> R<()> {
    for t in toks.iter().filter(|t| !t.quoted) {
        let key = t.text.split_once('=').map(|(k, _)| k);
        if key.is_some_and(|k| keys.contains(&k))
            || (key.is_none() && words.contains(&t.text.as_str()))
        {
            let what = key.map_or_else(|| format!("`{}`", t.text), |k| format!("`{k}=`"));
            return fail_fix(
                Code::E102,
                t.col,
                format!("{what} is reserved for a later stage of the dialect"),
                fix,
            );
        }
    }
    Ok(())
}
/// The `key=value` fields of a statement.
struct Fields {
    items: Vec<(String, String, usize)>,
}

impl Fields {
    fn parse(toks: &[Tok], allowed: &[&str]) -> R<Fields> {
        let mut items: Vec<(String, String, usize)> = Vec::new();
        for t in toks {
            let (k, v) = match (t.quoted, t.text.split_once('=')) {
                (false, Some(kv)) => kv,
                _ => {
                    return fail(
                        Code::E009,
                        t.col,
                        format!("unexpected `{}`: fields are written `key=value`", t.text),
                    );
                }
            };
            if !allowed.contains(&k) {
                return fail_fix(
                    Code::E009,
                    t.col,
                    format!("unknown field `{k}=`"),
                    format!("this statement takes {}", list(allowed)),
                );
            }
            if items.iter().any(|(ik, _, _)| ik == k) {
                return fail(
                    Code::E009,
                    t.col,
                    format!("the field `{k}=` is given twice"),
                );
            }
            items.push((k.to_string(), v.to_string(), t.col));
        }
        Ok(Fields { items })
    }

    fn get(&self, key: &str) -> Option<(&str, usize)> {
        self.items
            .iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, v, c)| (v.as_str(), *c))
    }

    fn need(&self, key: &str, col: usize, example: &str) -> R<(&str, usize)> {
        self.get(key).map_or_else(
            || {
                fail_fix(
                    Code::E003,
                    col,
                    format!("missing field `{key}=`"),
                    format!("add `{key}={example}`"),
                )
            },
            Ok,
        )
    }
}

fn list(allowed: &[&str]) -> String {
    allowed
        .iter()
        .map(|a| format!("`{a}=`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_script(diags: &mut Vec<Diagnostic>, n: usize, toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let (Some(d), Some(v)) = (toks.get(1), toks.get(2)) else {
        return fail_fix(
            Code::E003,
            col,
            "`script` needs a dialect and a version",
            "write `script fizz2 0`",
        );
    };
    if let Some(extra) = toks.get(3) {
        return fail(
            Code::E009,
            extra.col,
            format!("unexpected `{}` after the script line", extra.text),
        );
    }
    if d.text != "fizz2" || v.text != "0" {
        diags.push(
            Diagnostic::new(
                Code::E101,
                n,
                d.col,
                format!(
                    "this parser reads `script fizz2 0`, not `script {} {}`",
                    d.text, v.text
                ),
            )
            .fix("write `script fizz2 0`"),
        );
    }
    Ok(Stmt::Script {
        dialect: d.text.clone(),
        version: v.text.clone(),
    })
}

fn parse_level(toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let Some(name) = toks.get(1) else {
        return fail_fix(
            Code::E003,
            col,
            "`level` needs a quoted name",
            "write `level \"Name\" 32x48 id=name`",
        );
    };
    if !name.quoted {
        return fail_fix(
            Code::E004,
            name.col,
            "the level's name must be in double quotes",
            "write it as `\"Name\"`",
        );
    }
    let Some(size) = toks.get(2) else {
        return fail_fix(
            Code::E003,
            col,
            "`level` needs a size",
            "write it as `32x48`",
        );
    };
    let (w, h) = match (size.quoted, size.text.split_once('x')) {
        (false, Some((w, h))) => (uint(w, size.col)?, uint(h, size.col)?),
        _ => {
            return fail_fix(
                Code::E004,
                size.col,
                format!("`{}` is not a size", size.text),
                "write width and height as `32x48`",
            );
        }
    };
    let f = Fields::parse(&toks[3..], &["id", "ground"])?;
    let (id, idcol) = f.need("id", col, "name")?;
    let id = check_name(id, idcol)?;
    let ground = match f.get("ground") {
        Some((v, c)) => Some(uint(v, c)?),
        None => None,
    };
    Ok(Stmt::Level {
        name: name.text.clone(),
        w,
        h,
        id,
        ground,
    })
}

fn parse_state(toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let Some(name) = toks.get(1) else {
        return fail_fix(
            Code::E003,
            col,
            "`state` needs a name",
            "write `state lift values=down,up`",
        );
    };
    let name_s = check_name(&name.text, name.col)?;
    let f = Fields::parse(&toks[2..], &["values", "start"])?;
    let values = match f.get("values") {
        Some((v, c)) => Some(word_list(v, c)?),
        None => None,
    };
    let start = match f.get("start") {
        Some((v, c)) => {
            check_word(v, c, false)?;
            Some(v.to_string())
        }
        None => None,
    };
    Ok(Stmt::State {
        name: name_s,
        values,
        start,
    })
}

fn parse_door(toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let f = Fields::parse(&toks[1..], &["x", "y", "state", "open"])?;
    let (x, xc) = f.need("x", col, "15")?;
    let (y, yc) = f.need("y", col, "20..22")?;
    let (state, sc) = f.need("state", col, "name")?;
    let (open, oc) = f.need("open", col, "open")?;
    Ok(Stmt::Door {
        rect: Rect {
            x: range(x, xc)?,
            y: range(y, yc)?,
        },
        state: check_name(state, sc)?,
        open: word_list(open, oc)?,
    })
}

fn parse_lock(toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let Some(name) = toks.get(1) else {
        return fail_fix(
            Code::E003,
            col,
            "`lock` needs a name",
            "write `lock red-door key=red at=15,21`",
        );
    };
    let name_s = check_name(&name.text, name.col)?;
    for t in &toks[2..] {
        if !t.quoted && t.text.starts_with("channel=") {
            return fail_fix(
                Code::E102,
                t.col,
                "`channel=` is a Fizz 1 field: a lock's door is the run of door tiles at its cell",
                "remove it; for a channel, declare a `state` and bind it with `door`",
            );
        }
    }
    reserved(
        &toks[2..],
        &["id"],
        &["keep"],
        "remove it; kept keys and keyhole locks come with the switch objects",
    )?;
    let f = Fields::parse(&toks[2..], &["key", "at"])?;
    let (key, kc) = f.need("key", col, "red")?;
    if key.contains(',') {
        return fail_fix(
            Code::E102,
            kc,
            "a lock with two keys is reserved for a later stage of the dialect",
            "write one colour, `key=red` or `key=blue`",
        );
    }
    let (at, atcol) = f.need("at", col, "15,21")?;
    Ok(Stmt::Lock {
        name: name_s,
        key: key.to_string(),
        cell: ipair(at, atcol)?,
    })
}

/// One piece of a `when` line: a token split at its commas, with each comma a piece of its own.
#[derive(Clone, Debug)]
struct Atom {
    text: String,
    col: usize,
    quoted: bool,
}

fn atoms(toks: &[Tok]) -> Vec<Atom> {
    let mut out = Vec::new();
    for t in toks {
        if t.quoted {
            out.push(Atom {
                text: t.text.clone(),
                col: t.col,
                quoted: true,
            });
            continue;
        }
        let mut off = 0;
        for (i, part) in t.text.split(',').enumerate() {
            if i > 0 {
                out.push(Atom {
                    text: ",".into(),
                    col: t.col + off - 1,
                    quoted: false,
                });
            }
            if !part.is_empty() {
                out.push(Atom {
                    text: part.into(),
                    col: t.col + off,
                    quoted: false,
                });
            }
            off += part.chars().count() + 1;
        }
    }
    out
}

/// A cursor over the atoms of a `when` line.
struct Cur {
    a: Vec<Atom>,
    i: usize,
    /// The column just past the line's last atom, for a diagnostic about something missing at the end.
    end: usize,
}

impl Cur {
    fn peek(&self) -> Option<&Atom> {
        self.a.get(self.i)
    }

    fn peek_is(&self, word: &str) -> bool {
        self.peek().is_some_and(|a| !a.quoted && a.text == word)
    }

    fn next(&mut self) -> Option<Atom> {
        let a = self.a.get(self.i).cloned();
        self.i += 1;
        a
    }

    fn col(&self) -> usize {
        self.peek().map_or(self.end, |a| a.col)
    }

    /// The next atom, which must be there; `what` says what was expected.
    fn need(&mut self, what: &str) -> R<Atom> {
        let col = self.col();
        self.next()
            .map_or_else(|| fail(Code::E003, col, format!("missing {what}")), Ok)
    }

    /// The next atom as a `key=` field with the given key; its value and column.
    fn field(&mut self, key: &str, example: &str) -> R<(String, usize)> {
        let col = self.col();
        match self.peek() {
            Some(a) if !a.quoted && a.text.split_once('=').is_some_and(|(k, _)| k == key) => {
                let a = self.next().expect("peeked");
                let v = a.text.split_once('=').expect("checked").1.to_string();
                Ok((v, a.col + key.len() + 1))
            }
            Some(a) => fail_fix(
                Code::E009,
                a.col,
                format!("expected `{key}=`, found `{}`", a.text),
                format!("write `{key}={example}`"),
            ),
            None => fail_fix(
                Code::E003,
                col,
                format!("missing field `{key}=`"),
                format!("add `{key}={example}`"),
            ),
        }
    }

    /// The key of the next atom when it is a `key=` field.
    fn field_key(&self) -> Option<String> {
        self.peek()
            .filter(|a| !a.quoted)
            .and_then(|a| a.text.split_once('=').map(|(k, _)| k.to_string()))
    }

    /// `at=I,J`: the field's first number, a comma atom and the second.
    fn cell(&mut self) -> R<(i64, i64)> {
        let (first, col) = self.field("at", "15,21")?;
        if !self.peek_is(",") {
            return fail_fix(
                Code::E004,
                col,
                format!("`{first}` is not a pair of whole numbers"),
                "write two whole numbers with a comma, such as `15,21`",
            );
        }
        self.next();
        let second = self.need("the second number of `at=`")?;
        ipair(&format!("{first},{}", second.text), col)
    }

    /// `x=RANGE y=RANGE`, in either order.
    fn rect(&mut self) -> R<Rect> {
        let (mut x, mut y) = (None, None);
        while let Some(k) = self.field_key() {
            let slot = match k.as_str() {
                "x" => &mut x,
                "y" => &mut y,
                _ => break,
            };
            let a = self.next().expect("peeked");
            if slot.is_some() {
                return fail(
                    Code::E009,
                    a.col,
                    format!("the field `{k}=` is given twice"),
                );
            }
            let v = &a.text[k.len() + 1..];
            *slot = Some(range(v, a.col)?);
        }
        match (x, y) {
            (Some(x), Some(y)) => Ok(Rect { x, y }),
            (None, _) => fail_fix(Code::E003, self.col(), "missing field `x=`", "add `x=15`"),
            (_, None) => fail_fix(
                Code::E003,
                self.col(),
                "missing field `y=`",
                "add `y=20..22`",
            ),
        }
    }

    /// A name, given as `id=NAME`.
    fn id(&mut self) -> R<String> {
        let (v, c) = self.field("id", "name")?;
        check_name(&v, c)
    }

    fn name(&mut self, what: &str) -> R<String> {
        let a = self.need(what)?;
        if a.quoted {
            return fail(Code::E004, a.col, format!("{what} is a name, not a string"));
        }
        check_name(&a.text, a.col)
    }

    fn word(&mut self, what: &str) -> R<String> {
        let a = self.need(what)?;
        check_word(&a.text, a.col, a.quoted)?;
        Ok(a.text)
    }

    /// A key's object word, `key-red` or `key-blue`; another colour is reserved, and anything else is left to the validator.
    fn key(&mut self) -> R<String> {
        let a = self.need("a key, `key-red` or `key-blue`")?;
        check_word(&a.text, a.col, a.quoted)?;
        if a.text == "key-green" || a.text == "key-gold" {
            return fail_fix(
                Code::E102,
                a.col,
                format!("`{}` is reserved for a later stage of the dialect", a.text),
                "use `key-red` or `key-blue`",
            );
        }
        Ok(a.text)
    }

    fn int(&mut self, what: &str) -> R<i64> {
        let a = self.need(what)?;
        int(&a.text, a.col)
    }

    /// A word list after `is` or `is not`: words joined by commas.
    fn values(&mut self) -> R<Vec<String>> {
        let mut out = vec![self.word("a value")?];
        while self.peek_is(",") {
            self.next();
            out.push(self.word("a value after the comma")?);
        }
        Ok(out)
    }
}

/// The subject of a `stomp`, `stun` or `defeat`: `id=NAME` or `any`.
fn subject(c: &mut Cur) -> R<Subject> {
    if c.peek_is("any") {
        c.next();
        return Ok(Subject::Any);
    }
    if c.field_key().as_deref() == Some("kind") {
        return fail_fix(
            Code::E102,
            c.col(),
            "`kind=` is reserved until custom enemies",
            "write `id=NAME` or `any`",
        );
    }
    Ok(Subject::Id(c.id()?))
}

fn parse_input(c: &mut Cur) -> R<Input> {
    let col = c.col();
    let Some(a) = c.next() else {
        return fail_fix(
            Code::E003,
            col,
            "`when` needs an input",
            "write, for example, `when touch x=15 y=20..22 then ...`",
        );
    };
    if a.quoted {
        return fail(Code::E004, a.col, "an input is a word, not a string");
    }
    Ok(match a.text.as_str() {
        "start" => Input::Start,
        "enter" => Input::Enter(c.rect()?),
        "shoot" => Input::Shoot(c.rect()?),
        "take" => Input::Take(c.id()?),
        "stomp" => Input::Stomp(subject(c)?),
        "stun" => Input::Stun(subject(c)?),
        "defeat" => Input::Defeat(subject(c)?),
        "time" => {
            let (mut at, mut every) = (None, None);
            while let Some(k) = c.field_key() {
                let slot = match k.as_str() {
                    "at" => &mut at,
                    "every" => &mut every,
                    _ => break,
                };
                let f = c.next().expect("peeked");
                if slot.is_some() {
                    return fail(
                        Code::E009,
                        f.col,
                        format!("the field `{k}=` is given twice"),
                    );
                }
                *slot = Some(uint(&f.text[k.len() + 1..], f.col)?);
            }
            let Some(at) = at else {
                return fail_fix(Code::E003, c.col(), "missing field `at=`", "add `at=60`");
            };
            Input::Time { at, every }
        }
        "touch" => match c.field_key().as_deref() {
            Some("id") => Input::TouchId(c.id()?),
            Some("at") => {
                let (i, j) = c.cell()?;
                Input::TouchCell(i, j)
            }
            _ => Input::Touch(c.rect()?),
        },
        "use" => {
            if c.field_key().as_deref() == Some("at") {
                return fail_fix(
                    Code::E102,
                    c.col(),
                    "`use at=` (a cell that is used) is reserved for a later stage of the dialect",
                    "write `use id=SWITCH`",
                );
            }
            Input::Use(c.id()?)
        }
        "hit" => Input::Hit(c.id()?),
        "cleared" => {
            let (v, col) = c.field("group", "name")?;
            Input::Cleared(check_name(&v, col)?)
        }
        "state" => {
            let state = c.name("a state's name")?;
            let col = c.col();
            match c.next() {
                Some(w) if !w.quoted && w.text == "becomes" => Input::Becomes {
                    state,
                    value: c.word("a value after `becomes`")?,
                },
                Some(w) if !w.quoted && w.text == "changes" => Input::Changes { state },
                Some(w) => {
                    return fail_fix(
                        Code::E009,
                        w.col,
                        format!("expected `becomes` or `changes`, found `{}`", w.text),
                        "write `state NAME becomes VALUE` or `state NAME changes`",
                    );
                }
                None => {
                    return fail_fix(
                        Code::E003,
                        col,
                        "a state input needs `becomes VALUE` or `changes`",
                        "write `state NAME becomes VALUE` or `state NAME changes`",
                    );
                }
            }
        }
        "hurt" => {
            return fail_fix(
                Code::E102,
                a.col,
                "`hurt` is reserved for a later stage of the dialect",
                "remove the rule; `hit` is a switch struck",
            );
        }
        other => {
            return fail_fix(
                Code::E009,
                a.col,
                format!("`{other}` is not an input"),
                "the inputs are start, enter, take, shoot, stomp, stun, defeat, time, touch, use, hit, cleared and state",
            );
        }
    })
}

fn parse_cond(c: &mut Cur) -> R<Cond> {
    let a = c.need("a condition after `if` or `and`")?;
    if a.quoted {
        return fail(Code::E004, a.col, "a condition is a word, not a string");
    }
    match a.text.as_str() {
        "state" => {
            let state = c.name("a state's name")?;
            let col = c.col();
            match c.next() {
                Some(w) if !w.quoted && w.text == "is" => {}
                _ => {
                    return fail_fix(
                        Code::E003,
                        col,
                        "a state condition needs `is`",
                        "write `state NAME is VALUE`",
                    );
                }
            }
            let not = c.peek_is("not");
            if not {
                c.next();
            }
            Ok(Cond::StateIs {
                state,
                not,
                values: c.values()?,
            })
        }
        "holding" => Ok(Cond::Holding {
            not: false,
            key: c.key()?,
        }),
        "not" => {
            let col = c.col();
            match c.next() {
                Some(w) if !w.quoted && w.text == "holding" => Ok(Cond::Holding {
                    not: true,
                    key: c.key()?,
                }),
                _ => fail_fix(
                    Code::E003,
                    col,
                    "`not` is followed by `holding`",
                    "write `not holding key-red`",
                ),
            }
        }
        "ammo" | "score" | "cleared" => fail_fix(
            Code::E102,
            a.col,
            format!(
                "the `{}` condition is reserved for a later stage of the dialect",
                a.text
            ),
            "the conditions are `state NAME is ...` and `[not] holding KEY`",
        ),
        other => fail_fix(
            Code::E009,
            a.col,
            format!("`{other}` is not a condition"),
            "the conditions are `state NAME is ...` and `[not] holding KEY`",
        ),
    }
}

fn parse_action(c: &mut Cur) -> R<Action> {
    let a = c.need("an action")?;
    if a.quoted {
        return fail(Code::E004, a.col, "an action is a word, not a string");
    }
    Ok(match a.text.as_str() {
        "set" => Action::Set {
            state: c.name("a state's name")?,
            value: c.word("a value")?,
        },
        "toggle" => Action::Toggle(c.name("a state's name")?),
        "next" => Action::Next(c.name("a state's name")?),
        "spend" => Action::Spend(c.key()?),
        "give" => Action::Give(c.key()?),
        "score" => Action::Score(c.int("the points")?),
        "ammo" => Action::Ammo(c.int("the shots")?),
        "word" => {
            let t = c.need("a string's name or a quoted text")?;
            let text = if t.quoted {
                WordText::Text(t.text)
            } else {
                WordText::Name(check_name(&t.text, t.col)?)
            };
            let style = if c.field_key().as_deref() == Some("style") {
                let (v, col) = c.field("style", "white")?;
                check_word(&v, col, false)?;
                Some(v)
            } else {
                None
            };
            Action::Word {
                text,
                style,
                cell: c.cell()?,
            }
        }
        "cue" => Action::Cue {
            name: c.name("a cue's name")?,
            cell: c.cell()?,
        },
        other => {
            return fail_fix(
                Code::E009,
                a.col,
                format!("`{other}` is not an action"),
                "the actions are set, toggle, next, spend, give, score, ammo, word and cue",
            );
        }
    })
}

/// `when INPUT [if COND [and COND]] [after=TICKS] then ACTION[, ACTION] [repeat]`. Any number of conditions and actions parse;
/// the validator holds them to the record's two.
fn parse_when(toks: &[Tok]) -> R<Stmt> {
    let end = toks.last().map_or(1, |t| {
        t.col + t.text.chars().count() + if t.quoted { 2 } else { 0 }
    });
    let mut c = Cur {
        a: atoms(&toks[1..]),
        i: 0,
        end,
    };
    let input = parse_input(&mut c)?;
    let mut conds = Vec::new();
    if c.peek_is("if") {
        c.next();
        conds.push(parse_cond(&mut c)?);
        while c.peek_is("and") {
            c.next();
            conds.push(parse_cond(&mut c)?);
        }
    }
    let after = if c.field_key().as_deref() == Some("after") {
        let (v, col) = c.field("after", "60")?;
        Some(uint(&v, col)?)
    } else {
        None
    };
    match c.peek() {
        Some(a) if !a.quoted && a.text == "then" => {
            c.next();
        }
        Some(a) => {
            return fail_fix(
                Code::E009,
                a.col,
                format!(
                    "unexpected `{}`: the conditions and the delay come before `then`",
                    a.text
                ),
                "write `... then ACTION`",
            );
        }
        None => {
            return fail_fix(
                Code::E003,
                c.col(),
                "a rule needs `then` and at least one action",
                "add `then set NAME VALUE`",
            );
        }
    }
    if c.peek().is_none() || c.peek_is("repeat") {
        return fail_fix(
            Code::E003,
            c.col(),
            "a rule needs at least one action after `then`",
            "add an action, such as `set NAME VALUE`",
        );
    }
    let mut actions = vec![parse_action(&mut c)?];
    while c.peek_is(",") {
        c.next();
        actions.push(parse_action(&mut c)?);
    }
    let repeat = c.peek_is("repeat");
    if repeat {
        c.next();
    }
    if let Some(a) = c.peek() {
        return fail(
            Code::E009,
            a.col,
            format!("unexpected `{}` at the end of the rule", a.text),
        );
    }
    Ok(Stmt::When(When {
        input,
        conds,
        after,
        actions,
        repeat,
    }))
}

fn parse_string(toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let (Some(name), Some(text)) = (toks.get(1), toks.get(2)) else {
        return fail_fix(
            Code::E003,
            col,
            "`string` needs a name and a quoted text",
            "write `string hello \"Hello\" style=white`",
        );
    };
    let name_s = check_name(&name.text, name.col)?;
    if !text.quoted {
        return fail_fix(
            Code::E004,
            text.col,
            "the string's text must be in double quotes",
            "write it as `\"text\"`",
        );
    }
    let f = Fields::parse(&toks[3..], &["style"])?;
    Ok(Stmt::Str {
        name: name_s,
        text: text.text.clone(),
        style: f.get("style").map(|(v, _)| v.to_string()),
    })
}

fn parse_object(kind: ObjectKind, toks: &[Tok]) -> R<Stmt> {
    let col = toks[0].col;
    let allowed: &[&str] = if kind == ObjectKind::Chicken {
        &["id", "at", "x", "y", "dir"]
    } else {
        &["id", "at", "x", "y"]
    };
    reserved(
        &toks[1..],
        &["group"],
        &[],
        "remove it; a chicken's group comes with the `cleared` input's stage",
    )?;
    let f = Fields::parse(&toks[1..], allowed)?;
    let (id, idcol) = f.need("id", col, "name")?;
    let id = check_name(id, idcol)?;
    let pos = match (f.get("at"), f.get("x"), f.get("y")) {
        (Some((at, c)), None, None) => {
            let (x, y) = pair(at, c)?;
            Pos::At(x, y)
        }
        (None, Some((x, xc)), Some((y, yc))) => Pos::Cell(int(x, xc)?, int(y, yc)?),
        (None, None, None) => {
            return fail_fix(
                Code::E003,
                col,
                "an object needs a position",
                "add `at=2.5,11.7` or `x=3 y=4`",
            );
        }
        (None, Some(_), None) | (None, None, Some(_)) => {
            return fail_fix(
                Code::E003,
                col,
                "`x=` and `y=` come together",
                "add the other, or use `at=X,Y`",
            );
        }
        (Some(_), _, _) => {
            return fail(
                Code::E009,
                f.get("at").map_or(col, |(_, c)| c),
                "give the position as `at=` or as `x=` and `y=`, not both",
            );
        }
    };
    let dir = match (kind, f.get("dir")) {
        (ObjectKind::Chicken, Some((d, c))) => Some(num(d, c)?),
        (ObjectKind::Chicken, None) => {
            return fail_fix(
                Code::E003,
                col,
                "a chicken needs `dir=`",
                "add `dir=-1` or `dir=1`",
            );
        }
        _ => None,
    };
    Ok(Stmt::Object { kind, id, pos, dir })
}

fn parse_row(st: &State, trimmed: &str, indent: usize) -> R<(Stmt, Option<String>)> {
    let col = indent + 1;
    if st.section != Section::Map {
        return fail_fix(
            Code::E008,
            col,
            "a map row belongs under `map`",
            "put the rows under a `map` line",
        );
    }
    let body = &trimmed[1..];
    let Some(end) = body.find('|') else {
        return fail_fix(
            Code::E004,
            col,
            "a map row needs a closing `|`",
            "write the row between two `|` bars",
        );
    };
    let cells = &body[..end];
    let after = &body[end + 1..];
    let base = indent + 1 + cells.chars().count() + 1;
    let lexed = lex(after, base);
    if let Some(c) = lexed.unterminated {
        return fail(Code::E004, c, "a string is not closed on its line");
    }
    let mut label = None;
    if let Some(t) = lexed.toks.first() {
        label = Some(uint(&t.text, t.col)?);
    }
    if let Some(extra) = lexed.toks.get(1) {
        return fail(
            Code::E009,
            extra.col,
            format!("unexpected `{}` after the row label", extra.text),
        );
    }
    Ok((
        Stmt::Row {
            cells: cells.to_string(),
            label,
        },
        lexed.comment,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<Code> {
        parse(src).diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn splits_lines_keeping_their_endings() {
        let p = parse("script fizz2 0\r\nlevel \"A\" 1x1 id=a\n\nlast");
        let eols: Vec<Eol> = p.ast.lines.iter().map(|l| l.eol).collect();
        assert_eq!(eols, [Eol::CrLf, Eol::Lf, Eol::Lf, Eol::None]);
        assert_eq!(
            p.ast.to_source(),
            "script fizz2 0\r\nlevel \"A\" 1x1 id=a\n\nlast"
        );
    }

    #[test]
    fn a_header_parses_to_typed_values() {
        let p = parse("script fizz2 0\nlevel \"Lighthouse Point\" 32x48 id=light-1 // top\n");
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        match &p.ast.lines[1].body {
            Body::Stmt(Stmt::Level {
                name,
                w,
                h,
                id,
                ground,
            }) => {
                assert_eq!(
                    (name.as_str(), *w, *h, id.as_str(), *ground),
                    ("Lighthouse Point", 32, 48, "light-1", None)
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(p.ast.lines[1].comment.as_deref(), Some("top"));
    }

    #[test]
    fn a_missing_script_line_is_one_error() {
        assert_eq!(codes("level \"A\" 1x1 id=a\nstart 0,0\n"), [Code::E007]);
    }

    #[test]
    fn an_empty_file_has_no_level() {
        assert_eq!(codes(""), [Code::E007]);
    }

    #[test]
    fn a_bad_line_stays_in_the_tree() {
        let p = parse("script fizz2 0\nplot 1,2\n");
        assert_eq!(p.ast.lines[1].body, Body::Bad);
        assert_eq!(p.ast.lines[1].raw, "plot 1,2");
    }

    #[test]
    fn rows_and_objects_need_their_blocks() {
        let p = parse("script fizz2 0\nlevel \"A\" 1x1 id=a\n|.|\ncheezie id=c at=1,1\n");
        assert_eq!(
            p.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(),
            [Code::E008, Code::E008]
        );
    }

    #[test]
    fn a_row_may_hold_slashes_between_its_bars() {
        let p = parse("script fizz2 0\nlevel \"A\" 2x1 id=a\nmap\n  |//|  0 // note\n");
        match &p.ast.lines[3].body {
            Body::Stmt(Stmt::Row { cells, label }) => {
                assert_eq!((cells.as_str(), *label), ("//", Some(0)))
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(p.ast.lines[3].comment.as_deref(), Some("note"));
    }

    #[test]
    fn number_grammar_is_plain_decimals() {
        assert!(number_ok("-2.5", true));
        assert!(!number_ok(".5", true));
        assert!(!number_ok("1e3", true));
        assert!(!number_ok("+1", true));
        assert!(!number_ok("1.5", false));
    }
}
