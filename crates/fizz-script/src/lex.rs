// The line lexer: tokens, quoted strings and the `//` comment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A token of one line: its text (without the quotes of a string), its column and whether it was quoted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tok {
    pub text: String,
    /// The column of the first character, from 1.
    pub col: usize,
    pub quoted: bool,
}

#[derive(Debug, Default)]
pub(crate) struct Lexed {
    pub toks: Vec<Tok>,
    /// The text after `//`, trimmed.
    pub comment: Option<String>,
    /// An unterminated string: its column.
    pub unterminated: Option<usize>,
}

/// Splits `s` into tokens separated by spaces and tabs. `base` is the number of characters that came before `s` on its
/// line, so columns stay right when a line is lexed in two parts. A token that ends in a comma joins the token after it.
pub(crate) fn lex(s: &str, base: usize) -> Lexed {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Lexed::default();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == ' ' || c == '\t' {
            i += 1;
        } else if c == '/' && cs.get(i + 1) == Some(&'/') {
            let text: String = cs[i + 2..].iter().collect();
            out.comment = Some(text.trim().to_string());
            break;
        } else if c == '"' {
            let start = i;
            i += 1;
            let mut text = String::new();
            let mut closed = false;
            while i < cs.len() {
                if cs[i] == '"' {
                    closed = true;
                    i += 1;
                    break;
                }
                text.push(cs[i]);
                i += 1;
            }
            if !closed {
                out.unterminated = Some(base + start + 1);
                return out;
            }
            out.toks.push(Tok {
                text,
                col: base + start + 1,
                quoted: true,
            });
        } else {
            let start = i;
            while i < cs.len()
                && cs[i] != ' '
                && cs[i] != '\t'
                && cs[i] != '"'
                && !(cs[i] == '/' && cs.get(i + 1) == Some(&'/'))
            {
                i += 1;
            }
            let text: String = cs[start..i].iter().collect();
            join_comma(&mut out.toks, text, base + start + 1);
        }
    }
    out
}

/// Pushes an unquoted token, joining it to the one before when that one ends in a comma.
fn join_comma(toks: &mut Vec<Tok>, text: String, col: usize) {
    if let Some(last) = toks.last_mut()
        && !last.quoted
        && last.text.ends_with(',')
    {
        last.text.push_str(&text);
        return;
    }
    toks.push(Tok {
        text,
        col,
        quoted: false,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(l: &Lexed) -> Vec<&str> {
        l.toks.iter().map(|t| t.text.as_str()).collect()
    }

    #[test]
    fn tokens_columns_and_comments() {
        let l = lex("  start 2, 3 // here", 0);
        assert_eq!(texts(&l), ["start", "2,3"]);
        assert_eq!(l.toks[0].col, 3);
        assert_eq!(l.comment.as_deref(), Some("here"));
    }

    #[test]
    fn a_string_keeps_its_slashes_and_spaces() {
        let l = lex("string a \"x // y\" style=red", 0);
        assert_eq!(texts(&l), ["string", "a", "x // y", "style=red"]);
        assert!(l.toks[2].quoted);
        assert_eq!(l.comment, None);
    }

    #[test]
    fn an_unterminated_string_reports_its_column() {
        let l = lex("level \"oops", 0);
        assert_eq!(l.unterminated, Some(7));
    }

    #[test]
    fn a_comment_may_touch_a_token() {
        let l = lex("map// the grid", 0);
        assert_eq!(texts(&l), ["map"]);
        assert_eq!(l.comment.as_deref(), Some("the grid"));
    }

    #[test]
    fn base_shifts_the_columns() {
        let l = lex(" 12", 10);
        assert_eq!(l.toks[0].col, 12);
    }
}
