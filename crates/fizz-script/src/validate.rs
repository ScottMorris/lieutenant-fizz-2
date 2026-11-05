// The validator and the lowering: checks a tree against the dialect's rules and, when it has no errors, builds the level.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::ast::{Ast, ObjectKind, Stmt};
use crate::diag::{Code, Diagnostic};
use crate::legend::{self, door_channel, key_colour, style_of, tile_of};
use fizz_ep2::level::{self, LevelSource, Object};
use fizz_ep2::rules::Rule;
use fizz_ep2::world::SnackKind;
use fizz_pack::level::{MAX_RULES, MAX_STRINGS, NAME_MAX, STRING_EXTRAS, STRING_TEXT_MAX};
use fizz_pack::{RuleRec, StringRec};
use std::collections::BTreeSet;

/// A level as the rules build it, owned. [`Lowered::source`] borrows it as a [`LevelSource`].
#[derive(Clone, Debug)]
pub struct Lowered {
    pub name: String,
    pub w: u16,
    pub h: u16,
    /// Tile ids, row-major, row 0 (the bottom) first.
    pub tiles: Vec<u8>,
    pub switches: u32,
    pub floor_solid: bool,
    pub spawn: (f64, f64),
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
            switches: self.switches,
            floor_solid: self.floor_solid,
            spawn: self.spawn,
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

struct Sink {
    diags: Vec<Diagnostic>,
}

impl Sink {
    fn add(&mut self, code: Code, line: usize, col: usize, msg: impl Into<String>) {
        self.diags.push(Diagnostic::new(code, line, col, msg));
    }

    fn add_fix(
        &mut self,
        code: Code,
        line: usize,
        col: usize,
        msg: impl Into<String>,
        fix: impl Into<String>,
    ) {
        self.diags
            .push(Diagnostic::new(code, line, col, msg).fix(fix));
    }
}

struct At {
    line: usize,
    col: usize,
}

/// Checks `ast` and, when the checks find no error, lowers it. The diagnostics are the validator's (errors and lints), sorted by
/// position; the syntax diagnostics of the parse are not repeated here.
pub fn analyse(ast: &Ast) -> (Vec<Diagnostic>, Option<Lowered>) {
    let mut s = Sink { diags: Vec::new() };

    // The header statements.
    let mut level = None;
    let mut start = None;
    let mut floor = None;
    let mut map_at = None;
    let mut channels = Vec::new();
    let mut rows = Vec::new();
    let mut objects = Vec::new();
    let mut locks = Vec::new();
    let mut strings = Vec::new();
    for (n, l, st) in ast.stmts() {
        let at = At {
            line: n,
            col: l.col(),
        };
        match st {
            Stmt::Level {
                name,
                w,
                h,
                id: _,
                ground,
            } => {
                if let Some(g) = ground {
                    s.add_fix(
                        Code::W112,
                        n,
                        at.col,
                        format!("`ground={g}` is a Fizz 1 field and is ignored"),
                        "remove it",
                    );
                }
                level = Some((at, name.as_str(), *w, *h));
            }
            Stmt::Theme { name } => s.add(
                Code::W109,
                n,
                at.col,
                format!("`theme {name}` is accepted and printed, but nothing bakes it yet"),
            ),
            Stmt::Start { x, y } => start = Some((at, *x, *y)),
            Stmt::Floor { word } => floor = Some((at, word.as_str())),
            Stmt::Channel { channels: c, on } => channels.push((at, c.as_slice(), *on)),
            Stmt::Map => map_at = Some(at),
            Stmt::Row { cells, label } => rows.push((at, cells.as_str(), *label)),
            Stmt::Object { kind, id, pos, dir } => {
                objects.push((at, *kind, id.as_str(), pos.point(), *dir))
            }
            Stmt::Lock {
                name,
                key,
                channel,
                cell,
            } => locks.push((at, name.as_str(), key.as_str(), *channel, *cell)),
            Stmt::Str { name, text, style } => {
                strings.push((at, name.as_str(), text.as_str(), style.as_deref()))
            }
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
                at.line,
                at.col,
                format!("a level is 1 to 65535 tiles each way, not {w}x{h}"),
                "write a size such as `32x48`",
            );
        } else {
            dims = Some((*w as usize, *h as usize));
        }
        if name.len() > NAME_MAX || !name.chars().all(|c| (' '..='~').contains(&c)) {
            s.add_fix(
                Code::E109,
                at.line,
                at.col,
                format!("a level's name is up to {NAME_MAX} bytes of printable ASCII"),
                "shorten it or use plain letters",
            );
        }
    }

    // The start.
    match &start {
        None => {
            // A `start` line that failed to parse has its own error; do not add a second one.
            let attempted = ast.lines.iter().any(|l| {
                l.body == crate::ast::Body::Bad && l.raw.trim_start().starts_with("start")
            });
            if level.is_some() && !attempted {
                s.add_fix(
                    Code::E010,
                    1,
                    1,
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
                    at.line,
                    at.col,
                    format!("the start {x},{y} is outside the map"),
                );
            }
        }
    }

    // The floor.
    let floor_solid = match &floor {
        None => false,
        Some((_, "open")) => false,
        Some((_, "solid")) => true,
        Some((at, w)) => {
            s.add_fix(
                Code::E002,
                at.line,
                at.col,
                format!("`floor {w}` is not `open` or `solid`"),
                "write `floor open` or `floor solid`",
            );
            false
        }
    };

    // The channels that start on.
    let mut switches = 0u32;
    for (at, list, on) in &channels {
        for &c in *list {
            if c >= 32 {
                s.add(
                    Code::E109,
                    at.line,
                    at.col,
                    format!("channel {c} is out of range: channels are 0 to 31"),
                );
            } else if *on {
                switches |= 1 << c;
            } else {
                switches &= !(1 << c);
            }
        }
    }

    // The map.
    let mut tiles_by_row: Vec<Vec<Option<u8>>> = Vec::new();
    for (idx, (at, cells, label)) in rows.iter().enumerate() {
        let mut row = Vec::new();
        let mut bad_letter = false;
        for (i, ch) in cells.chars().enumerate() {
            let t = tile_of(ch);
            if t.is_none() && !bad_letter {
                bad_letter = true;
                s.add_fix(
                    Code::E104,
                    at.line,
                    at.col + 1 + i,
                    format!("`{ch}` is not a letter of the legend"),
                    "the letters are . # B = ^ D b E w",
                );
            }
            row.push(t);
        }
        if let Some((w, h)) = dims {
            if row.len() != w {
                s.add(
                    Code::E103,
                    at.line,
                    at.col,
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
                    at.line,
                    at.col,
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
        let (line, col) = map_at.as_ref().map_or_else(
            || level.as_ref().map_or((1, 1), |(a, ..)| (a.line, a.col)),
            |a| (a.line, a.col),
        );
        s.add(
            Code::E103,
            line,
            col,
            format!("the map has {} rows, but the level is {h} high", rows.len()),
        );
    }
    let tile_at = |x: i64, y: i64| -> Option<u8> {
        let (_, h) = dims?;
        if x < 0 || y < 0 || y as usize >= h || rows.len() != h {
            return None;
        }
        let row = &tiles_by_row[h - 1 - y as usize];
        row.get(x as usize).copied().flatten()
    };

    // Names are shared by objects, locks and strings.
    let mut names: BTreeSet<&str> = BTreeSet::new();

    // The objects.
    let (mut red, mut blue) = (false, false);
    for (at, kind, id, (x, y), dir) in &objects {
        claim(&mut names, &mut s, at, id);
        if let Some((w, h)) = dims
            && !((0.0..w as f64).contains(x) && (0.0..h as f64).contains(y))
        {
            s.add(
                Code::E108,
                at.line,
                at.col,
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
                        at.line,
                        at.col,
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
                    at.line,
                    at.col,
                    "a chicken's `dir=` is 1 or -1",
                    "write `dir=-1` or `dir=1`",
                );
            }
            _ => {}
        }
    }

    // The locks.
    let mut lock_channels: Vec<(u32, usize)> = Vec::new();
    let mut lock_recs: Vec<RuleRec> = Vec::new();
    for (i, (at, name, key, channel, cell)) in locks.iter().enumerate() {
        claim(&mut names, &mut s, at, name);
        let colour = key_colour(key);
        if colour.is_none() {
            s.add_fix(
                Code::E002,
                at.line,
                at.col,
                format!("`key={key}` is not a key colour"),
                "write `key=red` or `key=blue`",
            );
        }
        if i >= MAX_RULES {
            s.add(
                Code::E111,
                at.line,
                at.col,
                format!("a level has at most {MAX_RULES} locks"),
            );
        }
        let mut cell_ok = true;
        if *channel >= 32 {
            cell_ok = false;
            s.add(
                Code::E109,
                at.line,
                at.col,
                format!("channel {channel} is out of range: channels are 0 to 31"),
            );
        }
        if let Some((w, h)) = dims
            && (cell.0 < 0 || cell.1 < 0 || cell.0 as usize >= w || cell.1 as usize >= h)
        {
            cell_ok = false;
            s.add(
                Code::E117,
                at.line,
                at.col,
                format!("the cell {},{} is outside the map", cell.0, cell.1),
            );
        }
        if cell_ok
            && let Some(t) = tile_at(cell.0, cell.1)
            && door_channel(t) != Some(*channel as u8)
        {
            s.add_fix(
                Code::E110,
                at.line,
                at.col,
                format!(
                    "the cell {},{} is not a door tile on channel {channel}",
                    cell.0, cell.1
                ),
                "point `at=` at a `D` (channel 1) or `b` (channel 2) cell",
            );
        }
        if *channel < 32
            && let Some(prev) = lock_channels.iter().find(|(c, _)| c == channel)
        {
            s.add_fix(
                Code::E119,
                at.line,
                at.col,
                format!("channel {channel} already has a lock (line {})", prev.1),
                "keep one lock for each door",
            );
        }
        if *channel < 32 {
            lock_channels.push((*channel, at.line));
        }
        if let (Some(c), true) = (colour, cell_ok) {
            lock_recs.push(Rule::lock(c, *channel as u8, (cell.0 as u16, cell.1 as u16)).to_rec());
        }
    }

    // The strings.
    let mut string_recs = Vec::new();
    for (i, (at, name, text, style)) in strings.iter().enumerate() {
        claim(&mut names, &mut s, at, name);
        let mut ok = true;
        if i >= MAX_STRINGS {
            ok = false;
            s.add(
                Code::E112,
                at.line,
                at.col,
                format!("a level has at most {MAX_STRINGS} strings"),
            );
        }
        if text.is_empty() || text.len() > STRING_TEXT_MAX {
            ok = false;
            s.add(
                Code::E112,
                at.line,
                at.col,
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
                at.line,
                at.col,
                format!("the game cannot draw `{bad}` in a string"),
            );
        }
        let style_n = match style {
            None => Some(0),
            Some(w) => style_of(w),
        };
        if style_n.is_none() {
            ok = false;
            s.add_fix(
                Code::E112,
                at.line,
                at.col,
                format!("`style={}` is not a style", style.unwrap_or("")),
                format!("the styles are {}", legend::STYLES.join(", ")),
            );
        }
        if let (true, Some(n)) = (ok, style_n) {
            string_recs.push(StringRec::new(n, text));
        }
    }

    // Lints.
    for (at, _, _, channel, _) in &locks {
        if *channel < 32 && switches & (1 << channel) == 0 {
            s.add(
                Code::W101,
                at.line,
                at.col,
                format!("channel {channel} starts off, so this lock's door starts open"),
            );
        }
    }
    let mut door_channels: BTreeSet<u8> = BTreeSet::new();
    let mut first_door: Vec<(u8, usize, usize)> = Vec::new();
    for (at, cells, _) in &rows {
        for (i, ch) in cells.chars().enumerate() {
            if let Some(c) = tile_of(ch).and_then(door_channel)
                && door_channels.insert(c)
            {
                first_door.push((c, at.line, at.col + 1 + i));
            }
        }
    }
    let locked: BTreeSet<u32> = lock_channels.iter().map(|(c, _)| *c).collect();
    for (c, line, col) in first_door {
        if !locked.contains(&u32::from(c)) {
            s.add(
                Code::W121,
                line,
                col,
                format!("this door is on channel {c}, which no lock opens"),
            );
        }
    }
    for (at, list, _) in &channels {
        let distinct: BTreeSet<u32> = list.iter().copied().collect();
        for c in distinct {
            if c < 32 && !locked.contains(&c) && !door_channels.contains(&(c as u8)) {
                s.add(
                    Code::W108,
                    at.line,
                    at.col,
                    format!("channel {c} is used by no lock and no door in the map"),
                );
            }
        }
    }
    for (at, _, key, _, _) in &locks {
        let provided = match *key {
            "red" => red,
            "blue" => blue,
            _ => true,
        };
        if !provided {
            s.add(
                Code::W120,
                at.line,
                at.col,
                format!("no `key-{key}` in the level, so this lock's door cannot open"),
            );
        }
    }

    crate::diag::sort(&mut s.diags);
    if s.diags.iter().any(Diagnostic::is_error) {
        return (s.diags, None);
    }
    let (Some((_, name, _, _)), Some((_, sx, sy)), Some((w, h))) = (level, start, dims) else {
        return (s.diags, None);
    };

    // Lowering.
    let mut tiles = vec![0u8; w * h];
    for (r, row) in tiles_by_row.iter().enumerate() {
        let y = h - 1 - r;
        for (x, t) in row.iter().enumerate() {
            tiles[y * w + x] = t.expect("every letter was checked");
        }
    }
    let mut objs: Vec<(u8, Object)> = objects
        .iter()
        .map(|(_, kind, _, (x, y), dir)| {
            let (x, y) = (*x, *y);
            let o = match kind {
                ObjectKind::KeyRed => Object::KeyRed { x, y },
                ObjectKind::KeyBlue => Object::KeyBlue { x, y },
                ObjectKind::Cheezie => Object::Snack {
                    kind: SnackKind::Cheezie,
                    x,
                    y,
                },
                ObjectKind::Choc => Object::Snack {
                    kind: SnackKind::Choc,
                    x,
                    y,
                },
                ObjectKind::Cookie => Object::Snack {
                    kind: SnackKind::Cookie,
                    x,
                    y,
                },
                ObjectKind::Soda => Object::Snack {
                    kind: SnackKind::Soda,
                    x,
                    y,
                },
                ObjectKind::Chicken => Object::Chicken {
                    x,
                    y,
                    dir: dir.unwrap_or(1.0),
                },
            };
            (kind.level_kind(), o)
        })
        .collect();
    objs.sort_by_key(|(k, _)| *k);
    let lowered = Lowered {
        name: name.to_string(),
        w: w as u16,
        h: h as u16,
        tiles,
        switches,
        floor_solid,
        spawn: (sx, sy),
        objects: objs.into_iter().map(|(_, o)| o).collect(),
        rules: lock_recs,
        strings: string_recs,
    };
    if level::validate(&lowered.source()).is_err() {
        s.add(
            Code::E199,
            1,
            1,
            "fizz-ep2 refused a level this validator accepted; this is a bug in fizz-script",
        );
        return (s.diags, None);
    }
    (s.diags, Some(lowered))
}

/// Records a name; a name seen before is E105.
fn claim<'a>(names: &mut BTreeSet<&'a str>, s: &mut Sink, at: &At, name: &'a str) {
    if !names.insert(name) {
        s.add_fix(
            Code::E105,
            at.line,
            at.col,
            format!("the name `{name}` is already used"),
            "give every object, lock and string its own name",
        );
    }
}
