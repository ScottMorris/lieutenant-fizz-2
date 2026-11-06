// The expansion: the level's states, door bindings and rules with every name resolved to a number and every `lock` written out
// as its state, binding and rule, each with the source line it came from. The dump view prints it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::legend::{STYLES, colour_word};
use fizz_pack::level::{
    ARG_ANY, CAP_LEVEL_BASE, COND_NOT_HOLDS, FLAG_REPEAT, MAX_BINDINGS, MAX_RULES, MAX_STATES,
    MAX_STRINGS, OP_AMMO, OP_GIVE_KEY, OP_NEXT, OP_SCORE, OP_TOGGLE, OP_WORD, SUBJECT_ANY,
    TRIGGER_BECOMES, TRIGGER_DEFEAT, TRIGGER_ENTER, TRIGGER_SHOOT, TRIGGER_START, TRIGGER_STOMP,
    TRIGGER_STUN, TRIGGER_TAKE, TRIGGER_TIME, TRIGGER_TOUCH, TRIGGER_TOUCH_OBJECT,
};
use fizz_pack::{ActionRec, BindingRec, CondRec, RuleRec, StateRec};

/// A rectangle of cells, `(x0, y0, x1, y1)`, both corners included.
pub type Cells = (u16, u16, u16, u16);

/// Where an expanded record came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The source line, from 1.
    pub line: usize,
    /// Whether a `lock` line wrote it.
    pub lock: bool,
}

/// One state, numbered by its place in [`Expansion::states`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub values: Vec<String>,
    /// The index of the start value.
    pub start: usize,
    pub origin: Origin,
}

impl State {
    pub fn to_rec(&self) -> StateRec {
        StateRec::new(self.values.len() as u8, self.start as u8)
    }
}

/// One door binding, numbered by its place in [`Expansion::doors`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Door {
    pub rect: Cells,
    pub state: u8,
    /// The active set: bit `i` is value `i`.
    pub mask: u8,
    pub origin: Origin,
}

impl Door {
    pub fn to_rec(&self) -> BindingRec {
        BindingRec::door(self.state, self.mask, self.rect)
    }
}

/// A rule's input, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Start,
    Enter(Cells),
    /// The index of a key or snack object.
    Take(u16),
    Shoot(Cells),
    /// A chicken's index, or `SUBJECT_ANY`.
    Stomp(u16),
    Stun(u16),
    Defeat(u16),
    Time {
        at: u32,
        every: u32,
    },
    /// A rectangle; a cell is a rectangle with equal corners.
    Touch(Cells),
    /// An object's index.
    TouchObject(u16),
    /// A state's index and a value, or `ARG_ANY` for any change.
    Becomes {
        state: u8,
        value: u8,
    },
}

/// A rule's condition, resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    /// The state is in the set `mask` (`is not` is already the complement).
    StateIn {
        state: u8,
        mask: u8,
    },
    Holds(u8),
    NotHolds(u8),
}

/// The caption of a `word` action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caption {
    /// The level's string `i`.
    Level(u8),
    /// A name that is not one of the level's strings: a shared caption or a world string, which have no names in the dialect yet.
    Unresolved(String),
    /// Quoted text, which no string record holds yet.
    Text(String),
}

/// A rule's action, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Set {
        state: u8,
        value: u8,
    },
    Toggle(u8),
    Next(u8),
    Spend(u8),
    Give(u8),
    Score(i32),
    Ammo(i32),
    Word {
        caption: Caption,
        /// A style number, or `None` for the string's own.
        style: Option<u8>,
        cell: (u16, u16),
    },
    /// A sound cue by name: shared effects and the world's sound library have no names in the dialect yet.
    Cue {
        name: String,
        cell: (u16, u16),
    },
}

/// One rule, numbered by its place in [`Expansion::rules`], which is the order the simulation evaluates them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub input: Input,
    pub conds: Vec<Cond>,
    pub delay: u16,
    pub actions: Vec<Action>,
    pub repeat: bool,
    pub origin: Origin,
}

fn union(r: Cells) -> (u32, u32) {
    RuleRec::union_rect(r.0, r.1, r.2, r.3)
}

impl Cond {
    fn to_rec(self) -> CondRec {
        match self {
            Cond::StateIn { state, mask } => CondRec::state_in(state, mask),
            Cond::Holds(c) => CondRec::holds(c),
            Cond::NotHolds(c) => CondRec {
                kind: COND_NOT_HOLDS,
                a: c,
                b: 0,
            },
        }
    }
}

impl Action {
    /// The action record, or why the dialect cannot lower it yet.
    fn to_rec(&self) -> Result<ActionRec, String> {
        let op = |op: u8, a: u8| ActionRec {
            op,
            a,
            ..ActionRec::EMPTY
        };
        Ok(match self {
            Action::Set { state, value } => ActionRec::set(*state, *value),
            Action::Toggle(s) => op(OP_TOGGLE, *s),
            Action::Next(s) => op(OP_NEXT, *s),
            Action::Spend(c) => ActionRec::spend_key(*c),
            Action::Give(c) => op(OP_GIVE_KEY, *c),
            Action::Score(n) => ActionRec {
                n: *n,
                ..op(OP_SCORE, 0)
            },
            Action::Ammo(n) => ActionRec {
                n: *n,
                ..op(OP_AMMO, 0)
            },
            Action::Word {
                caption: Caption::Level(i),
                style,
                cell,
            } => ActionRec {
                op: OP_WORD,
                a: CAP_LEVEL_BASE + i,
                b: style.unwrap_or(0xFF),
                n: 0,
                x: cell.0,
                y: cell.1,
            },
            Action::Word {
                caption: Caption::Unresolved(name),
                ..
            } => {
                return Err(format!(
                    "`word {name}` names no string of this level, and shared captions and world strings have no names in the dialect yet"
                ));
            }
            Action::Word {
                caption: Caption::Text(_),
                ..
            } => {
                return Err(
                    "a `word` with quoted text has no string record to lower to yet; declare a `string` and name it".into(),
                );
            }
            Action::Cue { name, .. } => {
                return Err(format!(
                    "`cue {name}`: shared sound effects and the world's sound library have no names in the dialect yet"
                ));
            }
        })
    }
}

impl Rule {
    /// The 48-byte record's fields, or why the dialect cannot lower the rule yet.
    pub fn to_rec(&self) -> Result<RuleRec, String> {
        let mut r = RuleRec::EMPTY;
        match self.input {
            Input::Start => r.trigger = TRIGGER_START,
            Input::Enter(c) => (r.trigger, (r.u0, r.u1)) = (TRIGGER_ENTER, union(c)),
            Input::Take(i) => (r.trigger, r.subject) = (TRIGGER_TAKE, i),
            Input::Shoot(c) => (r.trigger, (r.u0, r.u1)) = (TRIGGER_SHOOT, union(c)),
            Input::Stomp(i) => (r.trigger, r.subject) = (TRIGGER_STOMP, i),
            Input::Stun(i) => (r.trigger, r.subject) = (TRIGGER_STUN, i),
            Input::Defeat(i) => (r.trigger, r.subject) = (TRIGGER_DEFEAT, i),
            Input::Time { at, every } => (r.trigger, r.u0, r.u1) = (TRIGGER_TIME, at, every),
            Input::Touch(c) => (r.trigger, (r.u0, r.u1)) = (TRIGGER_TOUCH, union(c)),
            Input::TouchObject(i) => (r.trigger, r.subject) = (TRIGGER_TOUCH_OBJECT, i),
            Input::Becomes { state, value } => {
                (r.trigger, r.subject, r.arg) = (TRIGGER_BECOMES, u16::from(state), value)
            }
        }
        if self.repeat {
            r.flags = FLAG_REPEAT;
        }
        r.delay = self.delay;
        for (slot, c) in r.cond.iter_mut().zip(&self.conds) {
            *slot = c.to_rec();
        }
        for (slot, a) in r.act.iter_mut().zip(&self.actions) {
            *slot = a.to_rec()?;
        }
        Ok(r)
    }
}

/// The resolved, expanded level: what the `LEVL` section's state, binding and rule tables hold, with their sources.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expansion {
    pub states: Vec<State>,
    pub doors: Vec<Door>,
    pub rules: Vec<Rule>,
    /// How many strings the level carries.
    pub strings: usize,
}

fn mask_text(mask: u8) -> String {
    let v: Vec<String> = (0..8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i: u8| i.to_string())
        .collect();
    format!("{{{}}}", v.join(","))
}

fn cells_text(c: Cells) -> String {
    let r = |lo: u16, hi: u16| {
        if lo == hi {
            lo.to_string()
        } else {
            format!("{lo}..{hi}")
        }
    };
    format!("x={} y={}", r(c.0, c.2), r(c.1, c.3))
}

fn subject_text(i: u16) -> String {
    if i == SUBJECT_ANY {
        "any".into()
    } else {
        format!("object {i}")
    }
}

impl Input {
    fn dump(&self) -> String {
        match self {
            Input::Start => "start".into(),
            Input::Enter(c) => format!("enter {}", cells_text(*c)),
            Input::Take(i) => format!("take object {i}"),
            Input::Shoot(c) => format!("shoot {}", cells_text(*c)),
            Input::Stomp(i) => format!("stomp {}", subject_text(*i)),
            Input::Stun(i) => format!("stun {}", subject_text(*i)),
            Input::Defeat(i) => format!("defeat {}", subject_text(*i)),
            Input::Time { at, every: 0 } => format!("time at={at}"),
            Input::Time { at, every } => format!("time at={at} every={every}"),
            Input::Touch(c) => format!("touch {}", cells_text(*c)),
            Input::TouchObject(i) => format!("touch object {i}"),
            Input::Becomes { state, value } if *value == ARG_ANY => format!("s{state} changes"),
            Input::Becomes { state, value } => format!("s{state} becomes {value}"),
        }
    }
}

impl Cond {
    fn dump(&self) -> String {
        match self {
            Cond::StateIn { state, mask } => format!("s{state} in {}", mask_text(*mask)),
            Cond::Holds(c) => format!("holding {}", colour_word(*c)),
            Cond::NotHolds(c) => format!("not holding {}", colour_word(*c)),
        }
    }
}

impl Action {
    fn dump(&self) -> String {
        match self {
            Action::Set { state, value } => format!("set s{state} {value}"),
            Action::Toggle(s) => format!("toggle s{s}"),
            Action::Next(s) => format!("next s{s}"),
            Action::Spend(c) => format!("spend {}", colour_word(*c)),
            Action::Give(c) => format!("give {}", colour_word(*c)),
            Action::Score(n) => format!("score {n}"),
            Action::Ammo(n) => format!("ammo {n}"),
            Action::Word {
                caption,
                style,
                cell,
            } => {
                let c = match caption {
                    Caption::Level(i) => format!("string {i}"),
                    Caption::Unresolved(n) => format!("{n}?"),
                    Caption::Text(t) => format!("\"{t}\""),
                };
                let s = style
                    .map(|s| format!(" style={}", STYLES[usize::from(s)]))
                    .unwrap_or_default();
                format!("word {c}{s} at={},{}", cell.0, cell.1)
            }
            Action::Cue { name, cell } => format!("cue {name}? at={},{}", cell.0, cell.1),
        }
    }
}

impl Rule {
    fn dump(&self) -> String {
        let mut s = self.input.dump();
        for (i, c) in self.conds.iter().enumerate() {
            s.push_str(if i == 0 { " if " } else { " and " });
            s.push_str(&c.dump());
        }
        if self.delay != 0 {
            s.push_str(&format!(" after={}", self.delay));
        }
        s.push_str(" then ");
        let a: Vec<String> = self.actions.iter().map(Action::dump).collect();
        s.push_str(&a.join(", "));
        if self.repeat {
            s.push_str(" repeat");
        }
        s
    }
}

fn origin_text(o: Origin) -> String {
    if o.lock {
        format!("// lock, line {}", o.line)
    } else {
        format!("// line {}", o.line)
    }
}

/// The dump view of `docs/SCRIPT.md`: a header with the counts against the limits, then every state, door binding and rule,
/// numbered, each with the line it came from. Names and values are numbers here, as in the records; a name the dialect cannot
/// resolve yet is printed with a `?`.
pub fn dump(path: &str, x: &Expansion) -> String {
    let mut out = format!(
        "// dump: {path} — states {}/{MAX_STATES}, bindings {}/{MAX_BINDINGS} ({} door), rules {}/{MAX_RULES}, strings {}/{MAX_STRINGS}\n",
        x.states.len(),
        x.doors.len(),
        x.doors.len(),
        x.rules.len(),
        x.strings
    );
    let name_w = x.states.iter().map(|s| s.name.len()).max().unwrap_or(0);
    let mut lines: Vec<(String, Origin)> = Vec::new();
    for (i, s) in x.states.iter().enumerate() {
        let values: Vec<String> = s
            .values
            .iter()
            .enumerate()
            .map(|(n, v)| format!("{v}:{n}"))
            .collect();
        lines.push((
            format!(
                "state {i:<3} {:<name_w$}  values={} start={}",
                s.name,
                values.join(","),
                s.values[s.start]
            ),
            s.origin,
        ));
    }
    for (i, d) in x.doors.iter().enumerate() {
        lines.push((
            format!(
                "door  {i:<3} {} state={} open={}",
                cells_text(d.rect),
                d.state,
                mask_text(d.mask)
            ),
            d.origin,
        ));
    }
    for (i, r) in x.rules.iter().enumerate() {
        lines.push((format!("rule  {i:<3} {}", r.dump()), r.origin));
    }
    let width = lines.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (l, o) in lines {
        out.push_str(&format!("{l:<width$}  {}\n", origin_text(o)));
    }
    out
}
