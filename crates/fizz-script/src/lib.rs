// The Fizz 2 script dialect: a lossless parser, canonical printer and validator, and the lowering to `fizz_ep2::level::LevelSource`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! # `fizz-script`
//!
//! The reusable library behind Fizz 2's level files, `script fizz2 0`, specified in `docs/SCRIPT.md`. The baker (`fizz-bake`)
//! and the Level Studio both link it, so there is one parser. It has no platform types and no I/O: text in, a tree, diagnostics
//! and a lowered level out.
//!
//! - [`parse`] never fails: it returns a lossless [`Ast`] (every line as read, bad lines included) and the syntax diagnostics.
//! - [`print_lossless`] writes the tree back byte for byte; [`print_canonical`] writes the canonical form.
//! - [`check`] adds the validator's errors and lints and, when there are no errors, the [`Expansion`] (every `lock` written out
//!   as its state, door binding and rule, with names resolved; [`dump`] prints it) and the [`Lowered`] level, whose
//!   [`Lowered::source`] is a `LevelSource`.
//!
//! A valid script can still be one this version of the game cannot run: a `when` rule other than a lock's shape has no phase in
//! the simulation yet, and a `word` or `cue` that names a shared caption or sound has no name table. Such a script parses, prints
//! and validates, and lowering returns [`Unbuilt`] with the line and the reason, not a diagnostic.
//!
//! ```
//! let report = fizz_script::check("script fizz2 0\nlevel \"Tiny\" 2x1 id=tiny\nstart 0,0\nmap\n  |.#|\n");
//! assert!(report.diagnostics.is_empty());
//! assert_eq!(report.lowered.unwrap().tiles, [0, 1]);
//! ```

pub mod ast;
pub mod diag;
pub mod expand;
pub mod legend;
mod lex;
pub mod parse;
pub mod print;
pub mod validate;

pub use ast::{Ast, Body, Eol, Line, ObjectKind, Pos, Stmt};
pub use diag::{Code, Diagnostic, Severity};
pub use expand::{Expansion, dump};
pub use parse::{Parse, parse};
pub use print::{print_canonical, print_lossless};
pub use validate::{Lowered, Unbuilt};

/// The result of checking a script.
#[derive(Clone, Debug)]
pub struct Report {
    pub ast: Ast,
    /// The syntax diagnostics and the validator's, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
    /// The resolved and expanded states, door bindings and rules, when the script has no errors.
    pub expansion: Option<Expansion>,
    /// The level, when the script has no errors and the game can build it.
    pub lowered: Option<Lowered>,
    /// Why a script with no errors does not lower yet.
    pub unbuilt: Option<Unbuilt>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

/// Why [`lower`] gave no level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LowerError {
    /// The script has errors; these are all its diagnostics.
    Diagnostics(Vec<Diagnostic>),
    /// The script is valid, and the game cannot build what it says yet.
    Unbuilt(Unbuilt),
}

/// Parses and validates `src`, and expands and lowers it when there is no error.
pub fn check(src: &str) -> Report {
    let Parse {
        ast,
        mut diagnostics,
    } = parse(src);
    let syntax_errors = diagnostics.iter().any(Diagnostic::is_error);
    let a = validate::analyse(&ast);
    diagnostics.extend(a.diagnostics);
    diag::sort(&mut diagnostics);
    let ok = !syntax_errors;
    Report {
        ast,
        diagnostics,
        expansion: a.expansion.filter(|_| ok),
        lowered: a.lowered.filter(|_| ok),
        unbuilt: a.unbuilt.filter(|_| ok),
    }
}

/// Lowers `src`: the level, or every diagnostic when it has an error, or why the game cannot build it yet.
pub fn lower(src: &str) -> Result<Lowered, LowerError> {
    let r = check(src);
    if r.has_errors() {
        return Err(LowerError::Diagnostics(r.diagnostics));
    }
    match (r.lowered, r.unbuilt) {
        (Some(l), _) => Ok(l),
        (None, Some(u)) => Err(LowerError::Unbuilt(u)),
        (None, None) => Err(LowerError::Diagnostics(r.diagnostics)),
    }
}

/// The canonical text of `src`, or the syntax errors that stop it from being formatted. A file that parses is formatted even when
/// the validator would refuse it.
pub fn format(src: &str) -> Result<String, Vec<Diagnostic>> {
    let p = parse(src);
    if p.diagnostics.iter().any(Diagnostic::is_error) {
        return Err(p.diagnostics);
    }
    Ok(print_canonical(&p.ast))
}
