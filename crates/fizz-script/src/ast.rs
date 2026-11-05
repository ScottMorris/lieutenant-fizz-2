// The lossless syntax tree: every line of a script as read, with what it parsed to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A line ending as read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    /// The last line had no line ending.
    None,
    Lf,
    CrLf,
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::None => "",
            Eol::Lf => "\n",
            Eol::CrLf => "\r\n",
        }
    }
}

/// A position in tiles as written: an exact point, or the centre of a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pos {
    /// `at=X,Y`.
    At(f64, f64),
    /// `x=I y=J`, which is `at=I+0.5,J+0.5`.
    Cell(i64, i64),
}

impl Pos {
    /// The point in tiles.
    pub fn point(self) -> (f64, f64) {
        match self {
            Pos::At(x, y) => (x, y),
            Pos::Cell(i, j) => (i as f64 + 0.5, j as f64 + 0.5),
        }
    }
}

/// What an object line names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    KeyRed,
    KeyBlue,
    Cheezie,
    Choc,
    Cookie,
    Soda,
    /// The provisional Fizz elixir, a snack of kind `SnackKind::Elixir` that gives a new life.
    Elixir,
    Chicken,
}

impl ObjectKind {
    /// The words of the `objects` block, in lowering (kind) order.
    pub const ALL: [ObjectKind; 8] = [
        ObjectKind::KeyRed,
        ObjectKind::KeyBlue,
        ObjectKind::Cheezie,
        ObjectKind::Choc,
        ObjectKind::Cookie,
        ObjectKind::Soda,
        ObjectKind::Elixir,
        ObjectKind::Chicken,
    ];

    pub fn word(self) -> &'static str {
        match self {
            ObjectKind::KeyRed => "key-red",
            ObjectKind::KeyBlue => "key-blue",
            ObjectKind::Cheezie => "cheezie",
            ObjectKind::Choc => "choc",
            ObjectKind::Cookie => "cookie",
            ObjectKind::Soda => "soda",
            ObjectKind::Elixir => "elixir",
            ObjectKind::Chicken => "chicken",
        }
    }

    pub fn from_word(w: &str) -> Option<ObjectKind> {
        ObjectKind::ALL.into_iter().find(|k| k.word() == w)
    }

    /// The `LEVL` object kind number, which is the sort key of the lowering.
    pub fn level_kind(self) -> u8 {
        use fizz_pack::level::{KIND_CHICKEN, KIND_KEY_BLUE, KIND_KEY_RED, KIND_SNACK};
        match self {
            ObjectKind::KeyRed => KIND_KEY_RED,
            ObjectKind::KeyBlue => KIND_KEY_BLUE,
            ObjectKind::Chicken => KIND_CHICKEN,
            _ => KIND_SNACK,
        }
    }
}

/// A parsed statement. Values are typed; the line's text stays in [`Line::raw`].
#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Script {
        dialect: String,
        version: String,
    },
    Level {
        name: String,
        w: u32,
        h: u32,
        id: String,
        ground: Option<u32>,
    },
    Theme {
        name: String,
    },
    Start {
        x: f64,
        y: f64,
    },
    /// The word after `floor`, resolved by the validator.
    Floor {
        word: String,
    },
    Channel {
        channels: Vec<u32>,
        on: bool,
    },
    /// The key word is resolved by the validator.
    Lock {
        name: String,
        key: String,
        /// The provisional `channel=N` alias; the door tile decides the channel.
        channel: Option<u32>,
        cell: (i64, i64),
    },
    /// The style word is resolved by the validator.
    Str {
        name: String,
        text: String,
        style: Option<String>,
    },
    Map,
    Objects,
    Row {
        cells: String,
        label: Option<u32>,
    },
    Object {
        kind: ObjectKind,
        id: String,
        pos: Pos,
        dir: Option<f64>,
    },
}

/// What a line parsed to.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Blank,
    /// A comment on a line of its own; the text is in [`Line::comment`].
    Comment,
    Stmt(Stmt),
    /// A line that did not parse (or a reserved word). Its text is kept as read.
    Bad,
}

/// One line of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The line's text without its ending, exactly as read.
    pub raw: String,
    pub eol: Eol,
    pub body: Body,
    /// The text after `//`, trimmed: a trailing comment, or the whole of a comment line.
    pub comment: Option<String>,
}

impl Line {
    /// The column (from 1, in characters) of the first character that is not white space.
    pub fn col(&self) -> usize {
        self.raw
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count()
            + 1
    }

    pub fn stmt(&self) -> Option<&Stmt> {
        match &self.body {
            Body::Stmt(s) => Some(s),
            _ => None,
        }
    }
}

/// A script as lines. Line `n` of the file (from 1) is `lines[n - 1]`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Ast {
    pub lines: Vec<Line>,
}

impl Ast {
    /// The text exactly as it was read.
    pub fn to_source(&self) -> String {
        let mut out = String::new();
        for l in &self.lines {
            out.push_str(&l.raw);
            out.push_str(l.eol.as_str());
        }
        out
    }

    /// The statements with their 1-based line numbers.
    pub fn stmts(&self) -> impl Iterator<Item = (usize, &Line, &Stmt)> {
        self.lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| l.stmt().map(|s| (i + 1, l, s)))
    }
}
