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
//! - [`check`] adds the validator's errors and lints and, when there are no errors, the [`Lowered`] level, whose
//!   [`Lowered::source`] is a `LevelSource`.
//!
//! ```
//! let report = fizz_script::check("script fizz2 0\nlevel \"Tiny\" 2x1 id=tiny\nstart 0,0\nmap\n  |.#|\n");
//! assert!(report.diagnostics.is_empty());
//! assert_eq!(report.lowered.unwrap().tiles, [0, 1]);
//! ```

pub mod ast;
pub mod diag;
pub mod legend;
mod lex;
pub mod parse;
pub mod print;
pub mod validate;

pub use ast::{Ast, Body, Eol, Line, ObjectKind, Pos, Stmt};
pub use diag::{Code, Diagnostic, Severity};
pub use parse::{Parse, parse};
pub use print::{print_canonical, print_lossless};
pub use validate::Lowered;

/// The result of checking a script.
#[derive(Clone, Debug)]
pub struct Report {
    pub ast: Ast,
    /// The syntax diagnostics and the validator's, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
    /// The level, when the script has no errors.
    pub lowered: Option<Lowered>,
}

impl Report {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
}

/// Parses and validates `src`, and lowers it when there is no error.
pub fn check(src: &str) -> Report {
    let Parse {
        ast,
        mut diagnostics,
    } = parse(src);
    let syntax_errors = diagnostics.iter().any(Diagnostic::is_error);
    let (found, lowered) = validate::analyse(&ast);
    diagnostics.extend(found);
    diag::sort(&mut diagnostics);
    Report {
        ast,
        diagnostics,
        lowered: if syntax_errors { None } else { lowered },
    }
}

/// Lowers `src`, or returns every diagnostic when it has an error.
pub fn lower(src: &str) -> Result<Lowered, Vec<Diagnostic>> {
    let r = check(src);
    match r.lowered {
        Some(l) if !r.has_errors() => Ok(l),
        _ => Err(r.diagnostics),
    }
}

/// The canonical text of `src`, or the syntax errors that stop it from being formatted. (A file that parses can always be
/// formatted, even when the validator would refuse it.)
pub fn format(src: &str) -> Result<String, Vec<Diagnostic>> {
    let p = parse(src);
    if p.diagnostics.iter().any(Diagnostic::is_error) {
        return Err(p.diagnostics);
    }
    Ok(print_canonical(&p.ast))
}
