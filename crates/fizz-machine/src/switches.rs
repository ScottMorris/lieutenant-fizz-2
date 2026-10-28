// The command-line switches of FIZZ2.EXE: one table that the argument parser and the help screen both read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// A switch is added by adding one line to `SWITCHES` (and, if it does something new, a `Kind` and the code that acts on it). The
// help text is written from the same lines, so a switch cannot be recognised without a help line or the reverse: the tests here
// check both ways, and `tests/help.txt` holds the exact text. Matching is case-insensitive, as DOS reads a command line.
use crate::settings::{Settings, apply_flags};
use core::fmt;

/// What a mode switch asks the program to do. The first one on the command line is used; `Live` is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Live,
    Replay,
    Bench,
    Check,
    CheckCells,
    Probe,
    Panic,
    CfgTest,
}

/// What a switch does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Chooses what the program does.
    Mode(Mode),
    /// Prints the help and exits.
    Help,
    /// The Modern key layout instead of Keen's.
    Modern,
    /// Captions on (`true`) or off (`false`); the last one given wins.
    Captions(bool),
    /// A sound choice, applied by `settings::apply_flags` (which also checks the value).
    Sound,
}

/// The shape of a switch's argument, for matching and for the help's name column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arg {
    /// The name alone.
    None,
    /// The name, a colon and a value: `/MUSIC:x`. The word is the value's placeholder in the help.
    Value(&'static str),
    /// The name, then an optional number as the next argument: `/BENCH [n]`.
    Count,
}

/// Where a switch is listed in the help.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Modes,
    Keys,
    Captions,
    Sound,
    /// For tests and developers; never listed.
    Developer,
}

impl Group {
    /// The heading line in the help, or `None` for a group the help does not list.
    pub const fn title(self) -> Option<&'static str> {
        match self {
            Group::Modes => Some("Modes (the first one given is used; /LIVE is the default)"),
            Group::Keys => Some("Keys"),
            Group::Captions => Some("Captions (on by default; the last one given wins)"),
            Group::Sound => Some("Sound (for this run only; FIZZ2.CFG keeps your saved choice)"),
            Group::Developer => None,
        }
    }
}

/// One line of the table.
#[derive(Clone, Copy, Debug)]
pub struct Switch {
    /// The switch as typed, upper case, with its slash: `/LIVE`.
    pub name: &'static str,
    /// Other spellings that mean the same.
    pub aliases: &'static [&'static str],
    pub arg: Arg,
    /// One line for the help, 62 characters at most.
    pub help: &'static str,
    pub group: Group,
    /// Recognised but not listed in the help (the developer switches).
    pub hidden: bool,
    pub kind: Kind,
}

const fn sw(name: &'static str, arg: Arg, group: Group, kind: Kind, help: &'static str) -> Switch {
    Switch {
        name,
        aliases: &[],
        arg,
        help,
        group,
        hidden: matches!(group, Group::Developer),
        kind,
    }
}

/// Every switch FIZZ2.EXE recognises, in the order the help lists them.
pub const SWITCHES: &[Switch] = &[
    sw("/LIVE", Arg::None, Group::Modes, Kind::Mode(Mode::Live), "Play the slice from the keyboard; Esc ends it"),
    sw("/REPLAY", Arg::None, Group::Modes, Kind::Mode(Mode::Replay), "Run the built-in script and print its transcript"),
    sw("/BENCH", Arg::Count, Group::Modes, Kind::Mode(Mode::Bench), "Time the script (first n ticks) and print the costs"),
    sw("/CHECK", Arg::None, Group::Modes, Kind::Mode(Mode::Check), "Show the machine check; Esc leaves, Enter plays"),
    sw("/CHECKCELLS", Arg::None, Group::Modes, Kind::Mode(Mode::CheckCells), "Draw the check screen and print its cells; no key"),
    sw("/PROBE", Arg::None, Group::Modes, Kind::Mode(Mode::Probe), "Print what the machine has: video, sound, processor"),
    Switch {
        aliases: &["/HELP", "-?"],
        ..sw("/?", Arg::None, Group::Modes, Kind::Help, "Show this help (also /HELP and -?); nothing else runs")
    },
    sw("/MODERN", Arg::None, Group::Keys, Kind::Modern, "Use the Modern keys (W A S D, Z X C) instead of Keen's"),
    sw("/CAPTIONS", Arg::None, Group::Captions, Kind::Captions(true), "Show the chicken's captions"),
    sw("/NOCAPTIONS", Arg::None, Group::Captions, Kind::Captions(false), "Hide the captions"),
    sw("/MUSIC", Arg::Value("x"), Group::Sound, Kind::Sound, "Music device: ADLIB, SB or OFF"),
    sw("/FX", Arg::Value("x"), Group::Sound, Kind::Sound, "Effects device: ADLIB, SB, SPEAKER or OFF"),
    sw("/ADLIB", Arg::None, Group::Sound, Kind::Sound, "Music and effects on the AdLib chip"),
    sw("/SB", Arg::None, Group::Sound, Kind::Sound, "Music and effects on the Sound Blaster"),
    sw("/SPEAKER", Arg::None, Group::Sound, Kind::Sound, "Effects on the PC speaker, no music"),
    sw("/NOSOUND", Arg::None, Group::Sound, Kind::Sound, "No music and no effects"),
    sw("/PANIC", Arg::None, Group::Developer, Kind::Mode(Mode::Panic), "Hook the timer and panic, to test the clean-up (exit 3)"),
    sw("/CFGTEST", Arg::None, Group::Developer, Kind::Mode(Mode::CfgTest), "Save FIZZ2.CFG from the sound switches and read it back"),
];

/// The width of the help's name column.
const NAME_COLUMN: usize = 14;

impl Switch {
    /// The text the help's name column shows: `/BENCH [n]`, `/MUSIC:x`, `/LIVE`.
    pub fn display(&self, out: &mut impl fmt::Write) -> fmt::Result {
        out.write_str(self.name)?;
        match self.arg {
            Arg::None => Ok(()),
            Arg::Value(v) => write!(out, ":{v}"),
            Arg::Count => out.write_str(" [n]"),
        }
    }

    /// Whether `arg` is this switch, by name or alias, with a value where the shape wants one.
    fn matches(&self, arg: &str) -> bool {
        let named = |n: &str| match self.arg {
            Arg::Value(_) => {
                arg.get(..n.len()).is_some_and(|h| h.eq_ignore_ascii_case(n))
                    && arg.len() > n.len() + 1
                    && arg.as_bytes()[n.len()] == b':'
            }
            _ => arg.eq_ignore_ascii_case(n),
        };
        named(self.name) || self.aliases.iter().any(|a| named(a))
    }
}

/// The table line `arg` names, if any. `/MUSIC:FOO` finds `/MUSIC`; whether the value is good is `parse`'s business.
pub fn lookup(arg: &str) -> Option<&'static Switch> {
    SWITCHES.iter().find(|s| s.matches(arg))
}

/// What the command line asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invocation {
    pub mode: Mode,
    pub modern: bool,
    pub captions: bool,
    /// `/BENCH`'s tick count, when given.
    pub bench_ticks: Option<u64>,
}

/// The outcome of reading a command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parsed<'a> {
    /// `/?`, `/HELP` or `-?` appeared: print the help and do nothing else, whatever else was given.
    Help,
    Run(Invocation),
    /// The first argument that is not a recognised switch, a bad sound value included.
    Unknown(&'a str),
}

/// Reads the arguments (without the program name). Help wins over everything; otherwise the first mode switch is the mode,
/// and a bare number is accepted only as `/BENCH`'s tick count.
pub fn parse<'a>(args: impl IntoIterator<Item = &'a str> + Clone) -> Parsed<'a> {
    if args.clone().into_iter().any(|a| lookup(a).is_some_and(|s| s.kind == Kind::Help)) {
        return Parsed::Help;
    }
    let mut run = Invocation {
        mode: Mode::Live,
        modern: false,
        captions: true,
        bench_ticks: None,
    };
    let mut mode_seen = false;
    let mut number: Option<(&str, u64)> = None;
    for arg in args {
        if let (None, Ok(n)) = (number, arg.parse::<u64>()) {
            number = Some((arg, n));
            continue;
        }
        let Some(s) = lookup(arg) else {
            return Parsed::Unknown(arg);
        };
        match s.kind {
            Kind::Mode(m) if !mode_seen => {
                run.mode = m;
                mode_seen = true;
            }
            Kind::Mode(_) | Kind::Help => {}
            Kind::Modern => run.modern = true,
            Kind::Captions(on) => run.captions = on,
            Kind::Sound => {
                if apply_flags(&mut Settings::default(), [arg]).applied != 1 {
                    return Parsed::Unknown(arg);
                }
            }
        }
    }
    match number {
        Some((_, n)) if run.mode == Mode::Bench => run.bench_ticks = Some(n),
        Some((text, _)) => return Parsed::Unknown(text),
        None => {}
    }
    Parsed::Run(run)
}

/// Writes the help screen: lines ending in `\n` (the caller turns them into the console's line ends), at most 25 lines of 79
/// characters, plain ASCII.
pub fn write_help(out: &mut impl fmt::Write) -> fmt::Result {
    out.write_str("Lieutenant Fizz 2 for DOS\n")?;
    out.write_str("Usage: FIZZ2.EXE [mode] [switches]. Letter case does not matter.\n")?;
    let mut group = None;
    for s in SWITCHES.iter().filter(|s| !s.hidden) {
        if group != Some(s.group) {
            group = Some(s.group);
            if let Some(t) = s.group.title() {
                writeln!(out, "{t}")?;
            }
        }
        let mut name = NameBuf { bytes: [0; NAME_COLUMN], len: 0 };
        s.display(&mut name)?;
        writeln!(out, "  {:<w$}{}", name.as_str(), s.help, w = NAME_COLUMN)?;
    }
    out.write_str("Exit status: 0 after help, 2 for a switch FIZZ2.EXE does not know.\n")
}

/// A small buffer for one name column, so formatting needs no allocation.
struct NameBuf {
    bytes: [u8; NAME_COLUMN],
    len: usize,
}

impl NameBuf {
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl fmt::Write for NameBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::{String, ToString};

    fn help() -> String {
        let mut s = String::new();
        write_help(&mut s).unwrap();
        s
    }

    fn run<'a>(args: &[&'a str]) -> Parsed<'a> {
        parse(args.iter().copied())
    }

    #[test]
    fn help_fits_a_dos_screen() {
        let text = help();
        assert!(text.lines().count() <= 25, "{} lines", text.lines().count());
        for l in text.lines() {
            assert!(l.len() <= 79, "too wide: {l}");
            assert!(l.bytes().all(|b| (0x20..0x7F).contains(&b)), "not plain ASCII: {l}");
        }
    }

    #[test]
    fn every_listed_switch_has_a_help_line_and_hidden_ones_have_none() {
        let text = help();
        for s in SWITCHES {
            assert!(!s.help.is_empty() && s.help.len() <= 62, "{}", s.name);
            let mut shown = String::new();
            s.display(&mut shown).unwrap();
            let line = std::format!("  {:<w$}{}", shown, s.help, w = NAME_COLUMN);
            assert_eq!(text.lines().any(|l| l == line), !s.hidden, "{}", s.name);
            assert_eq!(s.hidden, s.group == Group::Developer, "{}", s.name);
        }
    }

    #[test]
    fn every_help_line_names_a_recognised_switch() {
        for l in help().lines().filter(|l| l.starts_with("  ")) {
            let word = l.split_whitespace().next().unwrap();
            // `/MUSIC:x` is shown with its placeholder; try it with a value.
            let typed = word.replace(":x", ":ADLIB");
            let s = lookup(&typed).unwrap_or_else(|| panic!("help line for no switch: {l}"));
            assert!(!s.hidden, "{l}");
            assert_eq!(s.name, word.split(':').next().unwrap(), "{l}");
        }
    }

    #[test]
    fn names_are_unique_and_well_formed() {
        let mut seen = std::vec::Vec::new();
        for s in SWITCHES {
            for n in core::iter::once(&s.name).chain(s.aliases) {
                assert!(n.starts_with('/') || n.starts_with('-'), "{n}");
                assert_eq!(*n, n.to_ascii_uppercase());
                assert!(!seen.contains(n), "duplicate {n}");
                seen.push(*n);
            }
        }
    }

    #[test]
    fn sound_switches_agree_with_apply_flags() {
        for s in SWITCHES.iter().filter(|s| s.kind == Kind::Sound) {
            let typed = match s.arg {
                Arg::Value(_) => std::format!("{}:OFF", s.name),
                _ => s.name.to_string(),
            };
            assert_eq!(apply_flags(&mut Settings::default(), [typed.as_str()]).applied, 1, "{typed}");
        }
        // And no other table line is a sound flag.
        for s in SWITCHES.iter().filter(|s| s.kind != Kind::Sound) {
            assert_eq!(apply_flags(&mut Settings::default(), [s.name]).applied, 0, "{}", s.name);
        }
    }

    #[test]
    fn help_wins_over_everything() {
        for h in ["/?", "/help", "-?", "/HELP"] {
            assert_eq!(run(&[h]), Parsed::Help);
            assert_eq!(run(&["/LIVE", h]), Parsed::Help);
            assert_eq!(run(&[h, "/BOGUS"]), Parsed::Help);
        }
    }

    #[test]
    fn modes_and_options_combine() {
        let live = Invocation {
            mode: Mode::Live,
            modern: false,
            captions: true,
            bench_ticks: None,
        };
        let bench = Invocation { mode: Mode::Bench, ..live };
        assert_eq!(run(&[]), Parsed::Run(live));
        assert_eq!(
            run(&["/modern", "/nocaptions"]),
            Parsed::Run(Invocation { modern: true, captions: false, ..live })
        );
        assert_eq!(run(&["/NOCAPTIONS", "/CAPTIONS"]), Parsed::Run(live));
        assert_eq!(run(&["/REPLAY", "/LIVE"]), Parsed::Run(Invocation { mode: Mode::Replay, ..live }));
        assert_eq!(run(&["/PROBE", "/SB", "/FX:off"]), Parsed::Run(Invocation { mode: Mode::Probe, ..live }));
        assert_eq!(run(&["/PANIC"]), Parsed::Run(Invocation { mode: Mode::Panic, ..live }));
        assert_eq!(run(&["/BENCH"]), Parsed::Run(bench));
        assert_eq!(run(&["/BENCH", "120"]), Parsed::Run(Invocation { bench_ticks: Some(120), ..bench }));
        assert_eq!(run(&["120", "/BENCH"]), Parsed::Run(Invocation { bench_ticks: Some(120), ..bench }));
    }

    #[test]
    fn unknown_arguments_are_named() {
        assert_eq!(run(&["/LIVE", "/FOO"]), Parsed::Unknown("/FOO"));
        assert_eq!(run(&["FOO"]), Parsed::Unknown("FOO"));
        assert_eq!(run(&["/MUSIC:FOO"]), Parsed::Unknown("/MUSIC:FOO"));
        assert_eq!(run(&["/MUSIC:"]), Parsed::Unknown("/MUSIC:"));
        assert_eq!(run(&["/MUSIC"]), Parsed::Unknown("/MUSIC"));
        assert_eq!(run(&["/LIVE:1"]), Parsed::Unknown("/LIVE:1"));
        assert_eq!(run(&["/REPLAY", "12"]), Parsed::Unknown("12"));
        assert_eq!(run(&["12"]), Parsed::Unknown("12"));
    }
}
