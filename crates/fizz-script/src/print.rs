// The printers: the tree back as it was read, and the tree in the one canonical form.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ast::{
    Action, Ast, Body, Cond, Input, Line, ObjectKind, Pos, Stmt, Subject, When, WordText,
};

/// The text exactly as it was read, for every input.
pub fn print_lossless(ast: &Ast) -> String {
    ast.to_source()
}

enum Item {
    Blank,
    Comment(String),
    Line { indent: usize, text: String },
}

/// The shortest decimal that reads back to the same `f64`, with no exponent and no trailing `.0`.
pub fn number(v: f64) -> String {
    format!("{v}")
}

/// `x=I y=J` when the point is a cell centre, `at=X,Y` otherwise, so one position has one spelling.
fn position(p: Pos) -> String {
    let (x, y) = p.point();
    let (cx, cy) = (x - 0.5, y - 0.5);
    let cell = |c: f64| c.fract() == 0.0 && (0.0..1e9).contains(&c);
    if cell(cx) && cell(cy) {
        format!("x={} y={}", cx as i64, cy as i64)
    } else {
        format!("at={},{}", number(x), number(y))
    }
}

fn digits(n: usize) -> usize {
    n.to_string().len()
}

fn cell(c: (i64, i64)) -> String {
    format!("at={},{}", c.0, c.1)
}

fn subject(s: &Subject) -> String {
    match s {
        Subject::Id(id) => format!("id={id}"),
        Subject::Any => "any".to_string(),
    }
}

fn input(i: &Input) -> String {
    match i {
        Input::Start => "start".into(),
        Input::Enter(r) => format!("enter {}", r.text()),
        Input::Take(id) => format!("take id={id}"),
        Input::Shoot(r) => format!("shoot {}", r.text()),
        Input::Stomp(s) => format!("stomp {}", subject(s)),
        Input::Stun(s) => format!("stun {}", subject(s)),
        Input::Defeat(s) => format!("defeat {}", subject(s)),
        Input::Time { at, every } => match every {
            Some(e) => format!("time at={at} every={e}"),
            None => format!("time at={at}"),
        },
        Input::Touch(r) => format!("touch {}", r.text()),
        Input::TouchCell(i, j) => format!("touch {}", cell((*i, *j))),
        Input::TouchId(id) => format!("touch id={id}"),
        Input::Use(id) => format!("use id={id}"),
        Input::Hit(id) => format!("hit id={id}"),
        Input::Cleared(g) => format!("cleared group={g}"),
        Input::Becomes { state, value } => format!("state {state} becomes {value}"),
        Input::Changes { state } => format!("state {state} changes"),
    }
}

fn cond(c: &Cond) -> String {
    match c {
        Cond::StateIs { state, not, values } => format!(
            "state {state} is {}{}",
            if *not { "not " } else { "" },
            values.join(",")
        ),
        Cond::Holding { not, key } => format!("{}holding {key}", if *not { "not " } else { "" }),
    }
}

fn action(a: &Action) -> String {
    match a {
        Action::Set { state, value } => format!("set {state} {value}"),
        Action::Toggle(s) => format!("toggle {s}"),
        Action::Next(s) => format!("next {s}"),
        Action::Spend(k) => format!("spend {k}"),
        Action::Give(k) => format!("give {k}"),
        Action::Score(n) => format!("score {n}"),
        Action::Ammo(n) => format!("ammo {n}"),
        Action::Word {
            text,
            style,
            cell: c,
        } => {
            let t = match text {
                WordText::Name(n) => n.clone(),
                WordText::Text(t) => format!("\"{t}\""),
            };
            let s = style
                .as_ref()
                .map(|s| format!(" style={s}"))
                .unwrap_or_default();
            format!("word {t}{s} {}", cell(*c))
        }
        Action::Cue { name, cell: c } => format!("cue {name} {}", cell(*c)),
    }
}

/// `when INPUT [if COND [and COND]] [after=TICKS] then ACTION[, ACTION] [repeat]`.
pub(crate) fn when(w: &When) -> String {
    let mut s = format!("when {}", input(&w.input));
    for (i, c) in w.conds.iter().enumerate() {
        s.push_str(if i == 0 { " if " } else { " and " });
        s.push_str(&cond(c));
    }
    if let Some(a) = w.after {
        s.push_str(&format!(" after={a}"));
    }
    s.push_str(" then ");
    s.push_str(&w.actions.iter().map(action).collect::<Vec<_>>().join(", "));
    if w.repeat {
        s.push_str(" repeat");
    }
    s
}

/// A `door`'s open values in the order of the state's values, without repeats. When the state, or one of the values, is not
/// declared, the list keeps the order it was written in, without repeats.
fn open_values(ast: &Ast, state: &str, open: &[String]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for v in open {
        if !seen.contains(v) {
            seen.push(v.clone());
        }
    }
    if let Some(values) = ast.state_values(state)
        && seen.iter().all(|v| values.contains(v))
    {
        seen.sort_by_key(|v| values.iter().position(|x| x == v));
    }
    seen
}

fn statement(ast: &Ast, stmt: &Stmt, row: Option<(usize, usize)>) -> (usize, String) {
    match stmt {
        Stmt::Script { dialect, version } => (0, format!("script {dialect} {version}")),
        Stmt::Level {
            name,
            w,
            h,
            id,
            ground,
        } => {
            let g = ground.map(|g| format!(" ground={g}")).unwrap_or_default();
            (0, format!("level \"{name}\" {w}x{h} id={id}{g}"))
        }
        Stmt::Theme { name } => (0, format!("theme {name}")),
        Stmt::Start { x, y } => (0, format!("start {},{}", number(*x), number(*y))),
        Stmt::Floor { word } => (0, format!("floor {word}")),
        Stmt::State {
            name,
            values,
            start,
        } => {
            // The common state, `values=off,on` starting at its first value, is one short line.
            let list = values
                .clone()
                .unwrap_or_else(|| vec!["off".to_string(), "on".to_string()]);
            let mut s = format!("state {name}");
            if list != ["off", "on"] {
                s.push_str(&format!(" values={}", list.join(",")));
            }
            if let Some(st) = start
                && list.first() != Some(st)
            {
                s.push_str(&format!(" start={st}"));
            }
            (0, s)
        }
        Stmt::Door { rect, state, open } => (
            0,
            format!(
                "door {} state={state} open={}",
                rect.text(),
                open_values(ast, state, open).join(",")
            ),
        ),
        Stmt::When(w) => (0, when(w)),
        Stmt::Lock { name, key, cell: c } => (0, format!("lock {name} key={key} {}", cell(*c))),
        Stmt::Str { name, text, style } => {
            let s = style
                .as_ref()
                .map(|s| format!(" style={s}"))
                .unwrap_or_default();
            (0, format!("string {name} \"{text}\"{s}"))
        }
        Stmt::Map => (0, "map".to_string()),
        Stmt::Objects => (0, "objects".to_string()),
        Stmt::Row { cells, .. } => {
            let (idx, height) = row.expect("a row knows its place");
            if idx < height {
                let width = digits(height - 1);
                (2, format!("|{cells}|  {:>width$}", height - 1 - idx))
            } else {
                // A row below the bottom of the level has no `y` to label it with.
                (2, format!("|{cells}|"))
            }
        }
        Stmt::Object { kind, id, pos, dir } => {
            let d = dir
                .map(|d| format!(" dir={}", number(d)))
                .unwrap_or_default();
            (2, format!("{} id={id} {}{d}", kind.word(), position(*pos)))
        }
    }
}

fn with_comment(text: String, comment: &Option<String>) -> String {
    match comment {
        Some(c) if c.is_empty() => format!("{text}  //"),
        Some(c) => format!("{text}  // {c}"),
        None => text,
    }
}

fn flush(out: &mut Vec<Item>, group: &mut Vec<(u8, Vec<Item>)>) {
    group.sort_by_key(|(k, _)| *k);
    for (_, items) in group.drain(..) {
        out.extend(items);
    }
}

/// The tree in canonical form, the one `fizz-bake fmt` writes (see `docs/SCRIPT.md`). Printing it again gives the same text.
pub fn print_canonical(ast: &Ast) -> String {
    // The labels count down from the level's height; without a level, from the number of rows.
    let height = ast
        .stmts()
        .find_map(|(_, _, s)| match s {
            Stmt::Level { h, .. } => Some(*h as usize),
            _ => None,
        })
        .unwrap_or_else(|| {
            ast.lines
                .iter()
                .filter(|l| matches!(l.body, Body::Stmt(Stmt::Row { .. })))
                .count()
        });
    let mut row_idx = 0;
    let mut out: Vec<Item> = Vec::new();
    let mut pending: Vec<Item> = Vec::new();
    let mut group: Vec<(u8, Vec<Item>)> = Vec::new();
    for line in &ast.lines {
        match &line.body {
            Body::Blank => pending.push(Item::Blank),
            Body::Comment => pending.push(Item::Comment(line.comment.clone().unwrap_or_default())),
            Body::Stmt(stmt) => {
                let place = matches!(stmt, Stmt::Row { .. }).then(|| {
                    row_idx += 1;
                    (row_idx - 1, height)
                });
                let (indent, text) = statement(ast, stmt, place);
                let item = Item::Line {
                    indent,
                    text: with_comment(text, &line.comment),
                };
                if let Stmt::Object { kind, .. } = stmt {
                    let mut items: Vec<Item> = pending
                        .drain(..)
                        .filter(|i| matches!(i, Item::Comment(_)))
                        .collect();
                    items.push(item);
                    group.push((ObjectKind::level_kind(*kind), items));
                } else {
                    flush(&mut out, &mut group);
                    out.append(&mut pending);
                    out.push(item);
                }
            }
            Body::Bad => {
                flush(&mut out, &mut group);
                out.append(&mut pending);
                out.push(bad_item(line));
            }
        }
    }
    flush(&mut out, &mut group);
    out.append(&mut pending);
    render(&out)
}

fn bad_item(line: &Line) -> Item {
    Item::Line {
        indent: 0,
        text: line.raw.trim().to_string(),
    }
}

fn render(items: &[Item]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut last_blank = true;
    for (i, item) in items.iter().enumerate() {
        match item {
            Item::Blank => {
                if !last_blank {
                    lines.push(String::new());
                }
                last_blank = true;
            }
            Item::Comment(text) => {
                let indent = items[i..]
                    .iter()
                    .find_map(|n| match n {
                        Item::Line { indent, .. } => Some(*indent),
                        _ => None,
                    })
                    .unwrap_or(0);
                let body = if text.is_empty() {
                    "//".to_string()
                } else {
                    format!("// {text}")
                };
                lines.push(format!("{}{body}", " ".repeat(indent)));
                last_blank = false;
            }
            Item::Line { indent, text } => {
                lines.push(format!("{}{text}", " ".repeat(*indent)));
                last_blank = false;
            }
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let mut s = lines.join("\n");
    if !s.is_empty() {
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn numbers_print_shortest_without_a_point_for_whole_values() {
        assert_eq!(number(2.0), "2");
        assert_eq!(number(11.7), "11.7");
        assert_eq!(number(-0.5), "-0.5");
    }

    #[test]
    fn a_cell_centre_prints_as_a_cell_and_the_rest_as_a_point() {
        assert_eq!(position(Pos::At(3.5, 4.5)), "x=3 y=4");
        assert_eq!(position(Pos::Cell(3, 4)), "x=3 y=4");
        assert_eq!(position(Pos::At(2.5, 11.7)), "at=2.5,11.7");
        assert_eq!(position(Pos::At(8.0, 11.0)), "at=8,11");
        assert_eq!(position(Pos::Cell(-1, 0)), "at=-0.5,0.5");
    }

    #[test]
    fn blank_lines_collapse_and_the_ends_are_trimmed() {
        let p = parse("\n\nscript fizz2 0\n\n\n\nlevel \"A\" 1x1 id=a\n\n\n");
        assert_eq!(
            print_canonical(&p.ast),
            "script fizz2 0\n\nlevel \"A\" 1x1 id=a\n"
        );
    }

    #[test]
    fn a_comment_takes_the_indent_of_the_line_after_it() {
        let p = parse("script fizz2 0\nlevel \"A\" 1x1 id=a\nmap\n// the row\n|.|\n");
        assert_eq!(
            print_canonical(&p.ast),
            "script fizz2 0\nlevel \"A\" 1x1 id=a\nmap\n  // the row\n  |.|  0\n"
        );
    }

    #[test]
    fn objects_regroup_by_kind_with_their_comments() {
        let p = parse(
            "script fizz2 0\nlevel \"A\" 9x9 id=a\nobjects\n// a hen\nchicken id=h at=1,1 dir=-1\n\ncheezie id=c x=1 y=1\nkey-red id=k at=2,2\n",
        );
        assert_eq!(
            print_canonical(&p.ast),
            "script fizz2 0\nlevel \"A\" 9x9 id=a\nobjects\n  key-red id=k at=2,2\n  cheezie id=c x=1 y=1\n  // a hen\n  chicken id=h at=1,1 dir=-1\n"
        );
    }
}
