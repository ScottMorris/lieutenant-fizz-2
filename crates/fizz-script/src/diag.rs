// Diagnostics: the stable error and lint codes of the Fizz 2 script dialect, and where they point.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fmt;

/// Whether a diagnostic stops a level from lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
}

macro_rules! codes {
    ($($name:ident, $sev:ident, $summary:literal;)*) => {
        /// A diagnostic code. The codes are stable and never reused; `docs/SCRIPT.md` is their catalogue.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Code {
            $($name,)*
        }

        impl Code {
            /// Every code, in catalogue order.
            pub const ALL: &'static [Code] = &[$(Code::$name,)*];

            /// The code as written, such as `E003`.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Code::$name => stringify!($name),)*
                }
            }

            /// Whether the code is an error or a lint.
            pub const fn severity(self) -> Severity {
                match self {
                    $(Code::$name => Severity::$sev,)*
                }
            }

            /// The catalogue's one-line meaning.
            pub const fn summary(self) -> &'static str {
                match self {
                    $(Code::$name => $summary,)*
                }
            }
        }
    };
}

codes! {
    E001, Error, "unknown statement word";
    E002, Error, "unknown name";
    E003, Error, "missing field or word";
    E004, Error, "malformed value";
    E006, Error, "a statement that may appear once appears again";
    E007, Error, "missing script line, or a statement before the level header";
    E008, Error, "a statement in the wrong place";
    E009, Error, "unknown field, stray word, repeated field or a fractional cell";
    E010, Error, "no start statement";
    E101, Error, "unsupported dialect or version";
    E102, Error, "a word reserved for a later stage, or a Fizz 1 word the dialect does not have";
    E103, Error, "map rows or widths differ from the level size, or a row label is wrong";
    E104, Error, "a map letter the legend does not hold";
    E105, Error, "a name used twice";
    E107, Error, "a second key of one colour";
    E108, Error, "the start or an object centre is outside the map";
    E109, Error, "a value the record cannot hold";
    E110, Error, "a door meaning that does not hold";
    E111, Error, "a table over its limit";
    E112, Error, "a string the level cannot carry";
    E117, Error, "a cell or rectangle outside the map";
    E199, Error, "internal: the validator and the rules disagree";
    W101, Warning, "a door starts open";
    W108, Warning, "a state nothing binds, reads or sets";
    W109, Warning, "a reserved word that is accepted but not baked";
    W112, Warning, "a Fizz 1 field that is ignored";
    W114, Warning, "a repeating touch rule that toggles or advances a state";
    W120, Warning, "a lock whose key colour no object provides";
    W123, Warning, "two rules that open one door";
}

/// Codes the catalogue has retired. They are never produced and never reused: W121 (a switched tile no binding covers) is E110,
/// and W122 (the provisional `channel=` alias) went with the channels.
pub const RETIRED: [&str; 2] = ["W121", "W122"];

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One finding, at a line and column (both counted from 1; the column counts characters).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: Code,
    pub line: usize,
    pub col: usize,
    pub message: String,
    /// A short instruction that fixes it, where there is one.
    pub fix: Option<String>,
}

impl Diagnostic {
    pub(crate) fn new(
        code: Code,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            code,
            line,
            col,
            message: message.into(),
            fix: None,
        }
    }

    pub(crate) fn fix(mut self, fix: impl Into<String>) -> Diagnostic {
        self.fix = Some(fix.into());
        self
    }

    pub fn severity(&self) -> Severity {
        self.code.severity()
    }

    pub fn is_error(&self) -> bool {
        self.severity() == Severity::Error
    }

    /// The report line the baker prints: `path:line:col: error[E003]: message (fix: ...)`.
    pub fn render(&self, path: &str) -> String {
        let kind = match self.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let fix = self
            .fix
            .as_ref()
            .map(|f| format!(" (fix: {f})"))
            .unwrap_or_default();
        format!(
            "{path}:{}:{}: {kind}[{}]: {}{fix}",
            self.line, self.col, self.code, self.message
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render("<input>"))
    }
}

/// Orders diagnostics by position, then code, so a report reads top to bottom.
pub(crate) fn sort(d: &mut [Diagnostic]) {
    d.sort_by_key(|x| (x.line, x.col, x.code));
}
