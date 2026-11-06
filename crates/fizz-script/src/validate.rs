// The validator and the lowering: checks a tree against the dialect's rules, expands it and, when it has no errors, builds the
// level.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ast::{self, Ast, ObjectKind, Rect, Stmt, Subject, WordText};
use crate::diag::{Code, Diagnostic};
use crate::expand::{self, Caption, Cells, Expansion, Origin};
use crate::legend::{self, is_switched, key_colour, key_object_colour, style_of, tile_of};
use fizz_ep2::level::{self, LevelSource, Object, SourceError};
use fizz_ep2::world::SnackKind;
use fizz_pack::level::{
    AMMO_MAX, ARG_ANY, MAX_DOOR_BINDINGS, MAX_RULES, MAX_STATE_VALUES, MAX_STATES, MAX_STRINGS,
    MIN_PERIOD, NAME_MAX, SCORE_MAX, STRING_EXTRAS, STRING_TEXT_MAX, SUBJECT_ANY,
};
use fizz_pack::write::{LevelSpec, encode_level};
use fizz_pack::{BindingRec, LevelView, RuleRec, StateRec, StringRec};
use std::collections::{BTreeMap, BTreeSet};

/// A level as the rules build it, owned. [`Lowered::source`] borrows it as a [`LevelSource`].
#[derive(Clone, Debug)]
pub struct Lowered {
    pub name: String,
    pub w: u16,
    pub h: u16,
    /// Tile ids, row-major, row 0 (the bottom) first.
    pub tiles: Vec<u8>,
    pub floor_solid: bool,
    pub spawn: (f64, f64),
    pub states: Vec<StateRec>,
    pub bindings: Vec<BindingRec>,
    /// In construction order: keys, then snacks, then chickens, same-kind objects in file order.
    pub objects: Vec<Object>,
    pub rules: Vec<RuleRec>,
    pub strings: Vec<StringRec>,
}

impl Lowered {
    /// The level as the `LevelSource` every world build goes through.
    pub fn source(&self) -> LevelSource<'_> {
        LevelSource {
            name: &self.name,
            w: self.w,
            h: self.h,
            tiles: &self.tiles,
            floor_solid: self.floor_solid,
            spawn: self.spawn,
            states: &self.states,
            bindings: &self.bindings,
            objects: &self.objects,
            rules: &self.rules,
            strings: &self.strings,
        }
    }

    /// The level's digest, `fizz_ep2::level::digest`.
    pub fn digest(&self) -> u64 {
        level::digest(&self.source())
    }
}

/// A script that is valid and that the dialect cannot lower yet, because the records or the simulation it would lower to do not
/// exist on this version of the game. It is not a diagnostic: nothing in the file is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unbuilt {
    /// The line of the statement that cannot be lowered, from 1.
    pub line: usize,
    pub reason: String,
}

/// What the validator found and built.
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    /// The validator's errors and lints, sorted by position; the parse's syntax diagnostics are not repeated here.
    pub diagnostics: Vec<Diagnostic>,
    /// The resolved states, door bindings and rules, when the file has no errors.
    pub expansion: Option<Expansion>,
    pub lowered: Option<Lowered>,
    pub unbuilt: Option<Unbuilt>,
}

struct Sink {
    diags: Vec<Diagnostic>,
}

impl Sink {
    fn add(&mut self, code: Code, at: At, msg: impl Into<String>) {
        self.diags.push(Diagnostic::new(code, at.line, at.col, msg));
    }

    fn add_fix(&mut self, code: Code, at: At, msg: impl Into<String>, fix: impl Into<String>) {
        self.diags
            .push(Diagnostic::new(code, at.line, at.col, msg).fix(fix));
    }
}

#[derive(Clone, Copy, Debug)]
struct At {
    line: usize,
    col: usize,
}

/// A state as the validator knows it.
struct StateInfo {
    values: Vec<String>,
    index: u8,
}

/// The parts of the file the checks need, gathered in one pass.
struct Ctx<'a> {
    dims: Option<(usize, usize)>,
    /// The tiles, bottom row first, when the map is complete and every letter is known.
    tiles: Option<Vec<u8>>,
    states: BTreeMap<&'a str, StateInfo>,
    /// An object's name to its index in construction order and its kind.
    objects: BTreeMap<&'a str, (u16, ObjectKind)>,
    strings: BTreeMap<&'a str, u8>,
}

impl Ctx<'_> {
    fn tile(&self, x: i64, y: i64) -> Option<u8> {
        let (w, h) = self.dims?;
        let t = self.tiles.as_ref()?;
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            return None;
        }
        Some(t[y as usize * w + x as usize])
    }

    fn in_map(&self, x: i64, y: i64) -> bool {
        self.dims
            .is_none_or(|(w, h)| x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h)
    }

    /// The rectangle as cells, or E117 when it reaches outside the map.
    fn rect(&self, s: &mut Sink, at: At, r: Rect, what: &str) -> Option<Cells> {
        if !self.in_map(r.x.lo, r.y.lo) || !self.in_map(r.x.hi, r.y.hi) {
            s.add(
                Code::E117,
                at,
                format!("the {what} {} reaches outside the map", r.text()),
            );
            return None;
        }
        Some((r.x.lo as u16, r.y.lo as u16, r.x.hi as u16, r.y.hi as u16))
    }

    fn cell(&self, s: &mut Sink, at: At, c: (i64, i64), what: &str) -> Option<(u16, u16)> {
        if !self.in_map(c.0, c.1) {
            s.add(
                Code::E117,
                at,
                format!("the {what} {},{} is outside the map", c.0, c.1),
            );
            return None;
        }
        Some((c.0 as u16, c.1 as u16))
    }

    fn state(&self, s: &mut Sink, at: At, name: &str) -> Option<&StateInfo> {
        let st = self.states.get(name);
        if st.is_none() {
            s.add_fix(
                Code::E002,
                at,
                format!("no state is named `{name}`"),
                "declare it with `state` or `lock`",
            );
        }
        st
    }

    fn value(&self, s: &mut Sink, at: At, state: &str, info: &StateInfo, v: &str) -> Option<u8> {
        let i = info.values.iter().position(|x| x == v);
        if i.is_none() {
            s.add_fix(
                Code::E002,
                at,
                format!("`{v}` is not a value of the state `{state}`"),
                format!("its values are {}", info.values.join(", ")),
            );
        }
        i.map(|i| i as u8)
    }

    fn object(&self, s: &mut Sink, at: At, id: &str) -> Option<(u16, ObjectKind)> {
        let o = self.objects.get(id).copied();
        if o.is_none() {
            s.add(Code::E002, at, format!("no object is named `{id}`"));
        }
        o
    }

    fn key(&self, s: &mut Sink, at: At, word: &str) -> Option<u8> {
        let c = key_object_colour(word);
        if c.is_none() {
            s.add_fix(
                Code::E002,
                at,
                format!("`{word}` is not a key"),
                "write `key-red` or `key-blue`",
            );
        }
        c
    }
}

fn string_ok(s: &mut Sink, at: At, text: &str) -> bool {
    let mut ok = true;
    if text.is_empty() || text.len() > STRING_TEXT_MAX {
        ok = false;
        s.add(
            Code::E112,
            at,
            format!(
                "a string is 1 to {STRING_TEXT_MAX} bytes, not {}",
                text.len()
            ),
        );
    }
    if let Some(bad) = text
        .chars()
        .find(|c| !((' '..='~').contains(c) || STRING_EXTRAS.contains(&(*c as u32))))
    {
        ok = false;
        s.add(
            Code::E112,
            at,
            format!("the game cannot draw `{bad}` in a string"),
        );
    }
    ok
}

fn style(s: &mut Sink, at: At, word: &str) -> Option<u8> {
    let n = style_of(word);
    if n.is_none() {
        s.add_fix(
            Code::E112,
            at,
            format!("`style={word}` is not a style"),
            format!("the styles are {}", legend::STYLES.join(", ")),
        );
    }
    n
}

/// The 4-connected run of cells holding the same tile as `(x, y)`, as its bounding rectangle and whether the run fills it.
fn run(ctx: &Ctx, x: i64, y: i64) -> (Cells, bool) {
    let t = ctx.tile(x, y);
    let mut seen = BTreeSet::from([(x, y)]);
    let mut todo = vec![(x, y)];
    while let Some((cx, cy)) = todo.pop() {
        for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
            if ctx.tile(nx, ny) == t && seen.insert((nx, ny)) {
                todo.push((nx, ny));
            }
        }
    }
    let x0 = seen.iter().map(|c| c.0).min().unwrap_or(x);
    let x1 = seen.iter().map(|c| c.0).max().unwrap_or(x);
    let y0 = seen.iter().map(|c| c.1).min().unwrap_or(y);
    let y1 = seen.iter().map(|c| c.1).max().unwrap_or(y);
    let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as usize;
    (
        (x0 as u16, y0 as u16, x1 as u16, y1 as u16),
        area == seen.len(),
    )
}

/// What a `when` or `lock` line contributes to the lints.
#[derive(Default)]
struct Uses<'a> {
    /// States a rule reads (conditions, `becomes`, `changes`) or writes (`set`, `toggle`, `next`).
    touched: BTreeSet<&'a str>,
    /// `(state, value, line, col)` for each `set`.
    sets: Vec<(&'a str, &'a str, At)>,
}

/// Checks `ast`, expands it and, when the checks find no error, lowers it.
pub fn analyse(ast: &Ast) -> Analysis {
    let mut s = Sink { diags: Vec::new() };

    // One pass over the statements.
    let mut level = None;
    let mut start = None;
    let mut floor = None;
    let mut map_at = None;
    let mut rows = Vec::new();
    let mut objects = Vec::new();
    let mut strings = Vec::new();
    // The states, doors, rules and locks, in file order.
    let mut items: Vec<(At, &Stmt)> = Vec::new();
    // Every name a statement declares, in file order.
    let mut names: Vec<(At, &str)> = Vec::new();
    for (n, l, st) in ast.stmts() {
        let at = At {
            line: n,
            col: l.col(),
        };
        match st {
            Stmt::Level {
                name, w, h, ground, ..
            } => {
                if let Some(g) = ground {
                    s.add_fix(
                        Code::W112,
                        at,
                        format!("`ground={g}` is a Fizz 1 field and is ignored"),
                        "remove it",
                    );
                }
                level = Some((at, name.as_str(), *w, *h));
            }
            Stmt::Theme { name } => s.add(
                Code::W109,
                at,
                format!("`theme {name}` is accepted and printed, but nothing bakes it yet"),
            ),
            Stmt::Start { x, y } => start = Some((at, *x, *y)),
            Stmt::Floor { word } => floor = Some((at, word.as_str())),
            Stmt::Map => map_at = Some(at),
            Stmt::Row { cells, label } => rows.push((at, cells.as_str(), *label)),
            Stmt::Object { kind, id, pos, dir } => {
                names.push((at, id));
                objects.push((at, *kind, id.as_str(), pos.point(), *dir));
            }
            Stmt::Str { name, text, style } => {
                names.push((at, name));
                strings.push((at, name.as_str(), text.as_str(), style.as_deref()));
            }
            Stmt::State { name, .. } | Stmt::Lock { name, .. } => {
                names.push((at, name));
                items.push((at, st));
            }
            Stmt::Door { .. } | Stmt::When(_) => items.push((at, st)),
            Stmt::Script { .. } | Stmt::Objects => {}
        }
    }

    // The level: its size and its name.
    let mut dims = None;
    if let Some((at, name, w, h)) = &level {
        let ok = |v: u32| (1..=65535).contains(&v);
        if !ok(*w) || !ok(*h) {
            s.add_fix(
                Code::E109,
                *at,
                format!("a level is 1 to 65535 tiles each way, not {w}x{h}"),
                "write a size such as `32x48`",
            );
        } else {
            dims = Some((*w as usize, *h as usize));
        }
        if name.len() > NAME_MAX || !name.chars().all(|c| (' '..='~').contains(&c)) {
            s.add_fix(
                Code::E109,
                *at,
                format!("a level's name is up to {NAME_MAX} bytes of printable ASCII"),
                "shorten it or use plain letters",
            );
        }
    }

    // The start.
    match &start {
        None => {
            // A `start` line that failed to parse has its own error; do not add a second one.
            let attempted = ast
                .lines
                .iter()
                .any(|l| l.body == ast::Body::Bad && l.raw.trim_start().starts_with("start"));
            if level.is_some() && !attempted {
                s.add_fix(
                    Code::E010,
                    At { line: 1, col: 1 },
                    "the script has no `start` line",
                    "add `start 2,2`",
                );
            }
        }
        Some((at, x, y)) => {
            if let Some((w, h)) = dims
                && !((0.0..w as f64).contains(x) && (0.0..h as f64).contains(y))
            {
                s.add(
                    Code::E108,
                    *at,
                    format!("the start {x},{y} is outside the map"),
                );
            }
        }
    }

    // The floor.
    let floor_solid = match &floor {
        None | Some((_, "open")) => false,
        Some((_, "solid")) => true,
        Some((at, w)) => {
            s.add_fix(
                Code::E002,
                *at,
                format!("`floor {w}` is not `open` or `solid`"),
                "write `floor open` or `floor solid`",
            );
            false
        }
    };

    // The map.
    let mut grid_ok = true;
    let mut tiles_by_row: Vec<Vec<Option<u8>>> = Vec::new();
    for (idx, (at, cells, label)) in rows.iter().enumerate() {
        let mut row = Vec::new();
        let mut bad_letter = false;
        for (i, ch) in cells.chars().enumerate() {
            let t = tile_of(ch);
            if t.is_none() && !bad_letter {
                bad_letter = true;
                grid_ok = false;
                s.add_fix(
                    Code::E104,
                    At {
                        line: at.line,
                        col: at.col + 1 + i,
                    },
                    format!("`{ch}` is not a letter of the legend"),
                    "the letters are . # B = ^ D b E w",
                );
            }
            row.push(t);
        }
        if let Some((w, h)) = dims {
            if row.len() != w {
                grid_ok = false;
                s.add(
                    Code::E103,
                    *at,
                    format!(
                        "the row has {} letters, but the level is {w} wide",
                        row.len()
                    ),
                );
            }
            if let Some(l) = label
                && idx < h
                && *l as usize != h - 1 - idx
            {
                s.add_fix(
                    Code::E103,
                    *at,
                    format!("this row is y {}, not {l}", h - 1 - idx),
                    format!("write the label {}", h - 1 - idx),
                );
            }
        }
        tiles_by_row.push(row);
    }
    if let Some((_, h)) = dims
        && rows.len() != h
    {
        grid_ok = false;
        let at = map_at.unwrap_or_else(|| level.as_ref().map_or(At { line: 1, col: 1 }, |l| l.0));
        s.add(
            Code::E103,
            at,
            format!("the map has {} rows, but the level is {h} high", rows.len()),
        );
    }
    let tiles = match dims {
        Some((w, h)) if grid_ok => {
            let mut t = vec![0u8; w * h];
            for (r, row) in tiles_by_row.iter().enumerate() {
                let y = h - 1 - r;
                for (x, c) in row.iter().enumerate() {
                    t[y * w + x] = c.expect("every letter was checked");
                }
            }
            Some(t)
        }
        _ => None,
    };

    // Names: objects, states (a lock's name is its state's) and strings share one namespace.
    let mut claimed = BTreeSet::new();
    for (at, name) in &names {
        if !claimed.insert(*name) {
            s.add_fix(
                Code::E105,
                *at,
                format!("the name `{name}` is already used"),
                "give every object, state, lock and string its own name",
            );
        }
    }

    // The objects, and their indices in construction order.
    let (mut red, mut blue) = (false, false);
    for (at, kind, id, (x, y), dir) in &objects {
        if let Some((w, h)) = dims
            && !((0.0..w as f64).contains(x) && (0.0..h as f64).contains(y))
        {
            s.add(
                Code::E108,
                *at,
                format!("`{id}` is at {x},{y}, outside the map"),
            );
        }
        match kind {
            ObjectKind::KeyRed | ObjectKind::KeyBlue => {
                let seen = if *kind == ObjectKind::KeyRed {
                    &mut red
                } else {
                    &mut blue
                };
                if std::mem::replace(seen, true) {
                    s.add_fix(
                        Code::E107,
                        *at,
                        format!(
                            "a second `{}`: a level has one key of each colour",
                            kind.word()
                        ),
                        "remove it",
                    );
                }
            }
            ObjectKind::Chicken if !matches!(dir, Some(d) if *d == 1.0 || *d == -1.0) => {
                s.add_fix(
                    Code::E109,
                    *at,
                    "a chicken's `dir=` is 1 or -1",
                    "write `dir=-1` or `dir=1`",
                );
            }
            _ => {}
        }
    }
    let mut order: Vec<usize> = (0..objects.len()).collect();
    order.sort_by_key(|&i| objects[i].1.level_kind());
    let mut object_index = BTreeMap::new();
    for (n, &i) in order.iter().enumerate() {
        object_index
            .entry(objects[i].2)
            .or_insert((n as u16, objects[i].1));
    }

    // The strings.
    let mut string_recs = Vec::new();
    let mut string_index = BTreeMap::new();
    for (i, (at, name, text, st)) in strings.iter().enumerate() {
        string_index.entry(*name).or_insert(i as u8);
        let mut ok = true;
        if i >= MAX_STRINGS {
            ok = false;
            s.add(
                Code::E112,
                *at,
                format!("a level has at most {MAX_STRINGS} strings"),
            );
        }
        ok &= string_ok(&mut s, *at, text);
        let style_n = match st {
            None => Some(0),
            Some(w) => style(&mut s, *at, w),
        };
        if let (true, Some(n)) = (ok, style_n) {
            string_recs.push(StringRec::new(n, text));
        }
    }

    let mut ctx = Ctx {
        dims,
        tiles,
        states: BTreeMap::new(),
        objects: object_index,
        strings: string_index,
    };
    let mut x = Expansion {
        strings: strings.len(),
        ..Expansion::default()
    };

    // The states, counting the one each lock declares at its line.
    for (at, st) in &items {
        let (name, values, start, lock) = match st {
            Stmt::State {
                name,
                values,
                start,
            } => (
                name.as_str(),
                values
                    .clone()
                    .unwrap_or_else(|| vec!["off".into(), "on".into()]),
                start.as_deref(),
                false,
            ),
            Stmt::Lock { name, .. } => (
                name.as_str(),
                vec!["closed".into(), "open".into()],
                None,
                true,
            ),
            _ => continue,
        };
        if !(2..=MAX_STATE_VALUES).contains(&values.len()) {
            s.add_fix(
                Code::E109,
                *at,
                format!(
                    "a state has 2 to {MAX_STATE_VALUES} values, not {}",
                    values.len()
                ),
                "write the values as `values=off,on`",
            );
        }
        let mut seen = BTreeSet::new();
        for v in &values {
            if !seen.insert(v.as_str()) {
                s.add(
                    Code::E105,
                    *at,
                    format!("the value `{v}` is named twice in the state `{name}`"),
                );
            }
        }
        let start_i = match start {
            None => 0,
            Some(v) => values.iter().position(|x| x == v).unwrap_or_else(|| {
                s.add_fix(
                    Code::E109,
                    *at,
                    format!("`start={v}` is not one of the state's values"),
                    format!("start at one of {}", values.join(", ")),
                );
                0
            }),
        };
        if x.states.len() == MAX_STATES {
            s.add(
                Code::E111,
                *at,
                format!("a level has at most {MAX_STATES} states, counting one for each lock"),
            );
        }
        let index = x.states.len() as u8;
        ctx.states.entry(name).or_insert(StateInfo {
            values: values.clone(),
            index,
        });
        x.states.push(expand::State {
            name: name.to_string(),
            values,
            start: start_i,
            origin: Origin {
                line: at.line,
                lock,
            },
        });
    }

    // The door bindings, counting the one each lock declares at its line. Each switched tile is covered by exactly one.
    let mut covered: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    // For each lock: its line, its state, its door's rectangle and its key colour.
    let mut locks: Vec<(At, &str, Cells, u8)> = Vec::new();
    for (at, st) in &items {
        let door = match st {
            Stmt::Door { rect, state, open } => {
                let info = ctx.state(&mut s, *at, state);
                let mut mask = Some(0u8);
                if let Some(info) = info {
                    for v in open {
                        match ctx.value(&mut s, *at, state, info, v) {
                            Some(i) => mask = mask.map(|m| m | 1 << i),
                            None => mask = None,
                        }
                    }
                }
                let index = info.map(|i| i.index);
                let cells = ctx.rect(&mut s, *at, *rect, "door");
                if let Some(c) = cells
                    && ctx.tiles.is_some()
                {
                    let bad = (c.1..=c.3)
                        .flat_map(|y| (c.0..=c.2).map(move |x| (x, y)))
                        .find(|&(x, y)| {
                            !ctx.tile(i64::from(x), i64::from(y))
                                .is_some_and(is_switched)
                        });
                    if let Some((bx, by)) = bad {
                        s.add_fix(
                            Code::E110,
                            *at,
                            format!("the cell {bx},{by} of this door is not a door tile"),
                            "bind only `D` and `b` cells",
                        );
                        None
                    } else {
                        index.zip(mask).map(|(i, m)| (c, i, m, false))
                    }
                } else {
                    None
                }
            }
            Stmt::Lock { name, key, cell } => {
                let colour = key_colour(key);
                if colour.is_none() {
                    s.add_fix(
                        Code::E002,
                        *at,
                        format!("`key={key}` is not a key colour"),
                        "write `key=red` or `key=blue`",
                    );
                }
                let Some((cx, cy)) = ctx.cell(&mut s, *at, *cell, "lock's cell") else {
                    continue;
                };
                let (cx, cy) = (i64::from(cx), i64::from(cy));
                let Some(t) = ctx.tile(cx, cy) else {
                    continue;
                };
                if !is_switched(t) {
                    s.add_fix(
                        Code::E110,
                        *at,
                        format!("the cell {cx},{cy} is not a door tile"),
                        "point `at=` at a `D` or `b` cell",
                    );
                    continue;
                }
                let (cells, filled) = run(&ctx, cx, cy);
                if !filled {
                    s.add_fix(
                        Code::E110,
                        *at,
                        format!("the door at {cx},{cy} is not a rectangle of door tiles"),
                        "write `door` lines and a `when` rule",
                    );
                    continue;
                }
                let index = ctx.states.get(name.as_str()).map(|i| i.index);
                if let (Some(_), Some(c)) = (index, colour) {
                    locks.push((*at, name, cells, c));
                }
                index.map(|i| (cells, i, 0b10, true))
            }
            _ => continue,
        };
        let Some((cells, state, mask, lock)) = door else {
            continue;
        };
        if x.doors.len() == MAX_DOOR_BINDINGS {
            s.add(
                Code::E111,
                *at,
                format!(
                    "a level has at most {MAX_DOOR_BINDINGS} door bindings, counting one for each lock"
                ),
            );
        }
        let n = x.doors.len();
        let mut overlap = None;
        for yy in cells.1..=cells.3 {
            for xx in cells.0..=cells.2 {
                if let Some(prev) = covered.insert((xx, yy), n) {
                    overlap.get_or_insert((xx, yy, prev));
                }
            }
        }
        if let Some((ox, oy, prev)) = overlap {
            s.add_fix(
                Code::E110,
                *at,
                format!(
                    "the door tile at {ox},{oy} is already bound by line {}",
                    x.doors[prev].origin.line
                ),
                "bind each door tile once",
            );
        }
        x.doors.push(expand::Door {
            rect: cells,
            state,
            mask,
            origin: Origin {
                line: at.line,
                lock,
            },
        });
    }
    if let (Some((w, h)), Some(_)) = (dims, &ctx.tiles) {
        // One error for each run of door tiles that no binding covers, at its first cell in reading order.
        let mut reported: BTreeSet<(i64, i64)> = BTreeSet::new();
        for (r, (at, ..)) in rows.iter().enumerate().take(h) {
            let y = (h - 1 - r) as i64;
            for xx in 0..w as i64 {
                let t = ctx.tile(xx, y).expect("in the map");
                if !is_switched(t)
                    || covered.contains_key(&(xx as u16, y as u16))
                    || reported.contains(&(xx, y))
                {
                    continue;
                }
                let (c, _) = run(&ctx, xx, y);
                for yy in c.1..=c.3 {
                    for xc in c.0..=c.2 {
                        reported.insert((i64::from(xc), i64::from(yy)));
                    }
                }
                s.add_fix(
                    Code::E110,
                    At {
                        line: at.line,
                        col: at.col + 1 + xx as usize,
                    },
                    format!("the door tile at {xx},{y} has no `door` binding or `lock`"),
                    "add a `lock` or a `door` line for it",
                );
            }
        }
    }

    // The rules, counting the one each lock declares at its line.
    let mut uses = Uses::default();
    let mut lock_iter = locks.iter();
    for (at, st) in &items {
        let rule = match st {
            Stmt::Lock { name, .. } => {
                uses.touched.insert(name);
                // A lock that failed its door checks has an error already and adds no rule.
                let Some((_, _, cells, colour)) =
                    lock_iter.clone().next().filter(|l| l.0.line == at.line)
                else {
                    continue;
                };
                lock_iter.next();
                let state = ctx.states[name.as_str()].index;
                Some(expand::Rule {
                    input: expand::Input::Touch(*cells),
                    conds: vec![
                        expand::Cond::StateIn { state, mask: 0b01 },
                        expand::Cond::Holds(*colour),
                    ],
                    delay: 0,
                    actions: vec![
                        expand::Action::Set { state, value: 1 },
                        expand::Action::Spend(*colour),
                    ],
                    repeat: false,
                    origin: Origin {
                        line: at.line,
                        lock: true,
                    },
                })
            }
            Stmt::When(w) => resolve(&ctx, &mut s, *at, w, &mut uses),
            _ => continue,
        };
        if x.rules.len() == MAX_RULES {
            s.add(
                Code::E111,
                *at,
                format!("a level has at most {MAX_RULES} rules, counting one for each lock"),
            );
        }
        if let Some(r) = rule {
            x.rules.push(r);
        }
    }

    // Lints.
    for d in &x.doors {
        let st = &x.states[usize::from(d.state)];
        if d.mask & (1 << st.start) != 0 {
            s.add(
                Code::W101,
                At {
                    line: d.origin.line,
                    col: 1,
                },
                format!(
                    "the state `{}` starts at `{}`, an open value of this door, so the door starts open",
                    st.name, st.values[st.start]
                ),
            );
        }
    }
    let bound: BTreeSet<u8> = x.doors.iter().map(|d| d.state).collect();
    for st in &x.states {
        let i = ctx.states.get(st.name.as_str()).map(|i| i.index);
        if !st.origin.lock
            && !i.is_some_and(|i| bound.contains(&i))
            && !uses.touched.contains(st.name.as_str())
        {
            s.add(
                Code::W108,
                At {
                    line: st.origin.line,
                    col: 1,
                },
                format!(
                    "nothing binds the state `{}`, and no rule reads or sets it",
                    st.name
                ),
            );
        }
    }
    for (at, st) in &items {
        if let Stmt::Lock { key, .. } = st {
            let provided = match key.as_str() {
                "red" => red,
                "blue" => blue,
                _ => true,
            };
            if !provided {
                s.add(
                    Code::W120,
                    *at,
                    format!("no `key-{key}` in the level, so this lock's door cannot open"),
                );
            }
        }
    }
    for (i, (at, name, cells, _)) in locks.iter().enumerate() {
        if let Some(prev) = locks[..i].iter().find(|l| l.2 == *cells) {
            s.add_fix(
                Code::W123,
                *at,
                format!("this door already has a lock (line {})", prev.0.line),
                "keep one lock for each door",
            );
        }
        for (state, value, set_at) in &uses.sets {
            if state == name && *value == "open" {
                s.add_fix(
                    Code::W123,
                    *set_at,
                    format!(
                        "the lock `{name}` (line {}) already opens this door",
                        at.line
                    ),
                    "keep one rule that opens each door",
                );
            }
        }
    }

    crate::diag::sort(&mut s.diags);
    let mut out = Analysis {
        diagnostics: s.diags,
        ..Analysis::default()
    };
    if out.diagnostics.iter().any(Diagnostic::is_error) {
        return out;
    }
    let (Some((_, name, _, _)), Some((_, sx, sy)), Some((w, h)), Some(tiles)) =
        (level, start, dims, ctx.tiles.clone())
    else {
        return out;
    };

    // Lowering.
    let objs: Vec<Object> = order
        .iter()
        .map(|&i| {
            let (_, kind, _, (x, y), dir) = objects[i];
            let snack = |kind| Object::Snack { kind, x, y };
            match kind {
                ObjectKind::KeyRed => Object::KeyRed { x, y },
                ObjectKind::KeyBlue => Object::KeyBlue { x, y },
                ObjectKind::Cheezie => snack(SnackKind::Cheezie),
                ObjectKind::Choc => snack(SnackKind::Choc),
                ObjectKind::Cookie => snack(SnackKind::Cookie),
                ObjectKind::Soda => snack(SnackKind::Soda),
                ObjectKind::Elixir => snack(SnackKind::Elixir),
                ObjectKind::Chicken => Object::Chicken {
                    x,
                    y,
                    dir: dir.unwrap_or(1.0),
                },
            }
        })
        .collect();
    let mut rules = Vec::new();
    for r in &x.rules {
        match r.to_rec() {
            Ok(rec) => rules.push(rec),
            Err(reason) => {
                out.unbuilt.get_or_insert(Unbuilt {
                    line: r.origin.line,
                    reason,
                });
            }
        }
    }
    let lowered = Lowered {
        name: name.to_string(),
        w: w as u16,
        h: h as u16,
        tiles,
        floor_solid,
        spawn: (sx, sy),
        states: x.states.iter().map(expand::State::to_rec).collect(),
        bindings: x.doors.iter().map(expand::Door::to_rec).collect(),
        objects: objs,
        rules,
        strings: string_recs,
    };
    out.expansion = Some(x);
    if out.unbuilt.is_some() {
        return out;
    }
    let internal = |out: &mut Analysis, why: String| {
        out.diagnostics.push(Diagnostic::new(
            Code::E199,
            1,
            1,
            format!("{why} in a level this validator accepted; this is a bug in fizz-script"),
        ));
    };
    // The records must be a `LEVL` section the pack reader accepts, whatever the simulation can run.
    let src = lowered.source();
    let objects_recs: Vec<_> = src.objects.iter().map(Object::to_rec).collect();
    let spec = LevelSpec {
        width: src.w,
        height: src.h,
        floor_solid: src.floor_solid,
        spawn_x_bits: src.spawn.0.to_bits(),
        spawn_y_bits: src.spawn.1.to_bits(),
        name: src.name,
        states: src.states,
        bindings: src.bindings,
        objects: &objects_recs,
        rules: src.rules,
        strings: src.strings,
        tiles: src.tiles,
    };
    match encode_level(&spec).map(|b| LevelView::new(&b).map(|_| ())) {
        Err(e) => {
            internal(
                &mut out,
                format!("the level encoder refused the records ({e:?})"),
            );
            return out;
        }
        Ok(Err(e)) => {
            internal(
                &mut out,
                format!("the pack reader refused the records ({e:?})"),
            );
            return out;
        }
        Ok(Ok(())) => {}
    }
    match level::validate(&src) {
        Ok(()) => out.lowered = Some(lowered),
        Err(SourceError::Rule(_)) => {
            // The rules are read in order, so the first prefix that is refused ends at the rule the simulation cannot run.
            let n = (1..=src.rules.len())
                .find(|&n| {
                    let part = LevelSource {
                        rules: &src.rules[..n],
                        ..src
                    };
                    level::validate(&part).is_err()
                })
                .unwrap_or(src.rules.len());
            let line = out
                .expansion
                .as_ref()
                .and_then(|x| x.rules.get(n.saturating_sub(1)))
                .map_or(1, |r| r.origin.line);
            out.unbuilt = Some(Unbuilt {
                line,
                reason: "the simulation runs only touch and enter rules with the set, toggle, next and spend actions so far; this rule has no phase in fizz-ep2 yet".into(),
            });
        }
        Err(e) => internal(&mut out, format!("fizz-ep2 refused the level ({e:?})")),
    }
    out
}

/// Resolves a `when` rule against the level, or reports why it cannot be.
fn resolve<'a>(
    ctx: &Ctx,
    s: &mut Sink,
    at: At,
    w: &'a ast::When,
    uses: &mut Uses<'a>,
) -> Option<expand::Rule> {
    use expand::{Action as A, Cond as C, Input as I};
    let before = s.diags.iter().filter(|d| d.is_error()).count();
    let subject = |s: &mut Sink, sub: &Subject, what: &str| match sub {
        Subject::Any => Some(SUBJECT_ANY),
        Subject::Id(id) => {
            let (i, kind) = ctx.object(s, at, id)?;
            if kind != ObjectKind::Chicken {
                s.add(
                    Code::E109,
                    at,
                    format!("a {what} names a chicken, and `{id}` is a {}", kind.word()),
                );
            }
            Some(i)
        }
    };
    let input = match &w.input {
        ast::Input::Start => Some(I::Start),
        ast::Input::Enter(r) => ctx.rect(s, at, *r, "rectangle").map(I::Enter),
        ast::Input::Shoot(r) => ctx.rect(s, at, *r, "rectangle").map(I::Shoot),
        ast::Input::Touch(r) => ctx.rect(s, at, *r, "rectangle").map(I::Touch),
        ast::Input::TouchCell(i, j) => ctx
            .cell(s, at, (*i, *j), "cell")
            .map(|(x, y)| I::Touch((x, y, x, y))),
        ast::Input::TouchId(id) => ctx.object(s, at, id).map(|(i, _)| I::TouchObject(i)),
        ast::Input::Take(id) => {
            let o = ctx.object(s, at, id);
            if let Some((_, kind)) = o
                && kind == ObjectKind::Chicken
            {
                s.add(
                    Code::E109,
                    at,
                    format!("`take` names a key or a snack, and `{id}` is a chicken"),
                );
            }
            o.map(|(i, _)| I::Take(i))
        }
        ast::Input::Stomp(sub) => subject(s, sub, "stomp").map(I::Stomp),
        ast::Input::Stun(sub) => subject(s, sub, "stun").map(I::Stun),
        ast::Input::Defeat(sub) => subject(s, sub, "defeat").map(I::Defeat),
        ast::Input::Time { at: t, every } => {
            if *t == 0 {
                s.add(Code::E109, at, "a `time` rule's `at=` is at least 1");
            }
            if let Some(e) = every
                && (*e as usize) < MIN_PERIOD as usize
            {
                s.add(
                    Code::E109,
                    at,
                    format!("a `time` rule's `every=` is at least {MIN_PERIOD} ticks"),
                );
            }
            if every.is_some() != w.repeat {
                s.add_fix(
                    Code::E109,
                    at,
                    "a `time` rule repeats exactly when it has `every=`",
                    if w.repeat {
                        "add `every=TICKS`, or remove `repeat`"
                    } else {
                        "add `repeat`, or remove `every=`"
                    },
                );
            }
            Some(I::Time {
                at: *t,
                every: every.unwrap_or(0),
            })
        }
        ast::Input::Use(id) | ast::Input::Hit(id) => {
            s.add_fix(
                Code::E002,
                at,
                format!("no switch is named `{id}`: version 0 has no switch objects"),
                "switches arrive with a later stage of the dialect",
            );
            None
        }
        ast::Input::Cleared(g) => {
            s.add_fix(
                Code::E002,
                at,
                format!("no object carries the group `{g}`: version 0 has no `group=`"),
                "groups arrive with a later stage of the dialect",
            );
            None
        }
        ast::Input::Becomes { state, value } => {
            uses.touched.insert(state);
            let info = ctx.state(s, at, state)?;
            let v = ctx.value(s, at, state, info, value)?;
            Some(I::Becomes {
                state: info.index,
                value: v,
            })
        }
        ast::Input::Changes { state } => {
            uses.touched.insert(state);
            ctx.state(s, at, state).map(|info| I::Becomes {
                state: info.index,
                value: ARG_ANY,
            })
        }
    };

    if w.conds.len() > 2 {
        s.add_fix(
            Code::E109,
            at,
            format!("a rule holds 2 conditions, not {}", w.conds.len()),
            "split it into rules chained through a state",
        );
    }
    let mut conds = Vec::new();
    for c in &w.conds {
        match c {
            ast::Cond::StateIs { state, not, values } => {
                uses.touched.insert(state);
                let Some(info) = ctx.state(s, at, state) else {
                    continue;
                };
                let mut mask = 0u8;
                let mut ok = true;
                for v in values {
                    match ctx.value(s, at, state, info, v) {
                        Some(i) => mask |= 1 << i,
                        None => ok = false,
                    }
                }
                if !ok {
                    continue;
                }
                if *not {
                    let all = ((1u16 << info.values.len()) - 1) as u8;
                    mask = all & !mask;
                    if mask == 0 {
                        s.add(
                            Code::E109,
                            at,
                            format!("`is not` leaves the state `{state}` no value"),
                        );
                        continue;
                    }
                }
                conds.push(C::StateIn {
                    state: info.index,
                    mask,
                });
            }
            ast::Cond::Holding { not, key } => {
                if let Some(c) = ctx.key(s, at, key) {
                    conds.push(if *not { C::NotHolds(c) } else { C::Holds(c) });
                }
            }
        }
    }

    let delay = match w.after {
        Some(t) if t > u32::from(u16::MAX) => {
            s.add(
                Code::E109,
                at,
                format!("a delay is at most {} ticks, not {t}", u16::MAX),
            );
            0
        }
        t => t.unwrap_or(0) as u16,
    };

    if w.actions.len() > 2 {
        s.add_fix(
            Code::E109,
            at,
            format!("a rule holds 2 actions, not {}", w.actions.len()),
            "split it into rules chained through a state",
        );
    }
    let mut actions = Vec::new();
    for a in &w.actions {
        let r = match a {
            ast::Action::Set { state, value } => {
                uses.touched.insert(state);
                uses.sets.push((state, value, at));
                ctx.state(s, at, state).and_then(|info| {
                    let v = ctx.value(s, at, state, info, value)?;
                    Some(A::Set {
                        state: info.index,
                        value: v,
                    })
                })
            }
            ast::Action::Toggle(state) => {
                uses.touched.insert(state);
                ctx.state(s, at, state).and_then(|info| {
                    if info.values.len() != 2 {
                        s.add_fix(
                            Code::E109,
                            at,
                            format!(
                                "`toggle` flips a state of 2 values, and `{state}` has {}",
                                info.values.len()
                            ),
                            "use `next`",
                        );
                        return None;
                    }
                    Some(A::Toggle(info.index))
                })
            }
            ast::Action::Next(state) => {
                uses.touched.insert(state);
                ctx.state(s, at, state).map(|info| A::Next(info.index))
            }
            ast::Action::Spend(k) => ctx.key(s, at, k).map(A::Spend),
            ast::Action::Give(k) => ctx.key(s, at, k).map(A::Give),
            ast::Action::Score(n) | ast::Action::Ammo(n) => {
                let (max, what) = match a {
                    ast::Action::Score(_) => (SCORE_MAX, "score"),
                    _ => (AMMO_MAX, "ammo"),
                };
                if !(1..=i64::from(max)).contains(n) {
                    s.add(Code::E109, at, format!("`{what}` adds 1 to {max}, not {n}"));
                    None
                } else if what == "score" {
                    Some(A::Score(*n as i32))
                } else {
                    Some(A::Ammo(*n as i32))
                }
            }
            ast::Action::Word {
                text,
                style: st,
                cell,
            } => {
                let caption = match text {
                    WordText::Name(n) => Some(
                        ctx.strings
                            .get(n.as_str())
                            .map_or_else(|| Caption::Unresolved(n.clone()), |i| Caption::Level(*i)),
                    ),
                    WordText::Text(t) => string_ok(s, at, t).then(|| Caption::Text(t.clone())),
                };
                let style_n = match st {
                    None => Some(None),
                    Some(w) => style(s, at, w).map(Some),
                };
                let c = ctx.cell(s, at, *cell, "word's cell");
                match (caption, style_n, c) {
                    (Some(caption), Some(style), Some(cell)) => Some(A::Word {
                        caption,
                        style,
                        cell,
                    }),
                    _ => None,
                }
            }
            ast::Action::Cue { name, cell } => {
                ctx.cell(s, at, *cell, "cue's cell").map(|cell| A::Cue {
                    name: name.clone(),
                    cell,
                })
            }
        };
        if let Some(r) = r {
            actions.push(r);
        }
    }

    let touch = matches!(
        w.input,
        ast::Input::Touch(_) | ast::Input::TouchCell(..) | ast::Input::TouchId(_)
    );
    if w.repeat
        && touch
        && w.actions
            .iter()
            .any(|a| matches!(a, ast::Action::Toggle(_) | ast::Action::Next(_)))
    {
        s.add_fix(
            Code::W114,
            at,
            "a touch holds on every tick Ben is in contact, so this rule flips the state every tick",
            "remove `repeat`, or use `enter`, which is an edge",
        );
    }

    let after = s.diags.iter().filter(|d| d.is_error()).count();
    (after == before).then(|| expand::Rule {
        input: input.expect("no error, so the input resolved"),
        conds,
        delay,
        actions,
        repeat: w.repeat,
        origin: Origin {
            line: at.line,
            lock: false,
        },
    })
}
