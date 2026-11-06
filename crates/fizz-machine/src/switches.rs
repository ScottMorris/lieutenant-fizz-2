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
use crate::settings::{Captions, FrameRate, Keys, Night, Prefs, Settings, SkyDetail, apply_flags};
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
    MusicTest,
    MixTest,
    ReplanTest,
    MenuShot,
    Options,
}

/// What a switch does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Chooses what the program does.
    Mode(Mode),
    /// Prints the help and exits.
    Help,
    /// A key layout for this run, over `FIZZ2.CFG`'s; the last one given wins.
    Keys(Keys),
    /// Captions on or off for this run, over `FIZZ2.CFG`'s; the last one given wins.
    Captions(Captions),
    /// `/BENCH` also runs the 560 Hz audio service and the music (a developer switch).
    BenchAudio,
    /// `/LIVE` runs without its 30-second wall-clock limit (a developer switch, for a person at an emulator).
    NoLimit,
    /// The frame rate for this run, over `FIZZ2.CFG`'s; the last one given wins.
    FrameRate(FrameRate),
    /// The sky for this run, `/SKY:AUTO`, `/SKY:FULL` or `/SKY:LIGHT`, over `FIZZ2.CFG`'s; the last one given wins. The value is read by
    /// [`sky_value`].
    Sky,
    /// Night on (`/NIGHT`) or off (`/DAY`) for this run, over `FIZZ2.CFG`'s; the last one given wins.
    Night(Night),
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
    /// Keys, captions and frame rate: the choices that are not about sound.
    Options,
    Sound,
    /// For tests and developers; never listed.
    Developer,
}

impl Group {
    /// The heading line in the help, or `None` for a group the help does not list.
    pub const fn title(self) -> Option<&'static str> {
        match self {
            Group::Modes => Some("Modes (the first one given is used; /LIVE is the default)"),
            Group::Options => {
                Some("Options (for this run; where one has an opposite, the last one given wins)")
            }
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
    /// Recognised but not given a line of its own in the help: the developer switches, and a switch another one's line names
    /// as its opposite (see `pair`).
    pub hidden: bool,
    /// The opposite this switch's help line names after its own name (`/MODERN /KEEN`), so a pair takes one line of the 25. The
    /// opposite is in the same group and `hidden`, and its own `help` is what its line would say.
    pub pair: Option<&'static str>,
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
        pair: None,
        kind,
    }
}

/// Every switch FIZZ2.EXE recognises, in the order the help lists them.
pub const SWITCHES: &[Switch] = &[
    sw(
        "/LIVE",
        Arg::None,
        Group::Modes,
        Kind::Mode(Mode::Live),
        "Play the slice from the keyboard; Esc pauses",
    ),
    sw(
        "/REPLAY",
        Arg::None,
        Group::Modes,
        Kind::Mode(Mode::Replay),
        "Run the built-in script and print its transcript",
    ),
    sw(
        "/BENCH",
        Arg::Count,
        Group::Modes,
        Kind::Mode(Mode::Bench),
        "Time the script (first n ticks) and print the costs",
    ),
    sw(
        "/CHECK",
        Arg::None,
        Group::Modes,
        Kind::Mode(Mode::Check),
        "Show the machine check; Esc leaves, Enter plays",
    ),
    sw(
        "/CHECKCELLS",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::CheckCells),
        "Draw the check screen and print its cells; no key",
    ),
    sw(
        "/PROBE",
        Arg::None,
        Group::Modes,
        Kind::Mode(Mode::Probe),
        "Print what the machine has: video, sound, processor",
    ),
    Switch {
        aliases: &["/HELP", "-?"],
        ..sw(
            "/?",
            Arg::None,
            Group::Modes,
            Kind::Help,
            "Show this help (also /HELP and -?); nothing else runs",
        )
    },
    Switch {
        pair: Some("/KEEN"),
        ..sw(
            "/MODERN",
            Arg::None,
            Group::Options,
            Kind::Keys(Keys::Modern),
            "Modern keys (W A S D, Z X C), or Keen's, the default",
        )
    },
    Switch {
        hidden: true,
        ..sw(
            "/KEEN",
            Arg::None,
            Group::Options,
            Kind::Keys(Keys::Keen),
            "Use Keen's keys, even if FIZZ2.CFG saved the Modern ones",
        )
    },
    sw(
        "/CAPTIONS",
        Arg::None,
        Group::Options,
        Kind::Captions(Captions::On),
        "Show the chicken's captions",
    ),
    sw(
        "/NOCAPTIONS",
        Arg::None,
        Group::Options,
        Kind::Captions(Captions::Off),
        "Hide the captions",
    ),
    sw(
        "/FPS30",
        Arg::None,
        Group::Options,
        Kind::FrameRate(FrameRate::Hz30),
        "Draw 30 frames a second (the game itself still runs at 60)",
    ),
    sw(
        "/FPS60",
        Arg::None,
        Group::Options,
        Kind::FrameRate(FrameRate::Hz60),
        "Draw 60 frames a second",
    ),
    sw(
        "/FPSAUTO",
        Arg::None,
        Group::Options,
        Kind::FrameRate(FrameRate::Auto),
        "Draw 60 or 30 a second, whichever the machine keeps up with",
    ),
    sw(
        "/SKY",
        Arg::Value("x"),
        Group::Options,
        Kind::Sky,
        "Sky: AUTO (the default), FULL night, or LIGHT (stars and sea)",
    ),
    Switch {
        pair: Some("/DAY"),
        ..sw(
            "/NIGHT",
            Arg::None,
            Group::Options,
            Kind::Night(Night::On),
            "Night, lit by lanterns; or day, the default",
        )
    },
    Switch {
        hidden: true,
        ..sw(
            "/DAY",
            Arg::None,
            Group::Options,
            Kind::Night(Night::Off),
            "Play by day, even if FIZZ2.CFG saved the night",
        )
    },
    sw(
        "/MUSIC",
        Arg::Value("x"),
        Group::Sound,
        Kind::Sound,
        "Music device: ADLIB, SB or OFF",
    ),
    sw(
        "/FX",
        Arg::Value("x"),
        Group::Sound,
        Kind::Sound,
        "Effects device: ADLIB, SB, SPEAKER or OFF",
    ),
    sw(
        "/ADLIB",
        Arg::None,
        Group::Sound,
        Kind::Sound,
        "Music and effects on the AdLib chip",
    ),
    sw(
        "/SB",
        Arg::None,
        Group::Sound,
        Kind::Sound,
        "Music and effects on the Sound Blaster",
    ),
    sw(
        "/SPEAKER",
        Arg::None,
        Group::Sound,
        Kind::Sound,
        "Effects on the PC speaker, no music",
    ),
    sw(
        "/NOSOUND",
        Arg::None,
        Group::Sound,
        Kind::Sound,
        "No music and no effects",
    ),
    sw(
        "/PANIC",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::Panic),
        "Hook the timer and panic, to test the clean-up (exit 3)",
    ),
    sw(
        "/CFGTEST",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::CfgTest),
        "Save FIZZ2.CFG from the sound switches and read it back",
    ),
    sw(
        "/MUSICTEST",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::MusicTest),
        "Step two loops of the music as fast as possible and hash",
    ),
    sw(
        "/MIXTEST",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::MixTest),
        "Play the music, switch to the menu mix and back, and audit it",
    ),
    sw(
        "/REPLANTEST",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::ReplanTest),
        "Run the audio service and change its devices live",
    ),
    sw(
        "/MENUSHOT",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::MenuShot),
        "Draw the Options and Paused screens and print their hashes",
    ),
    sw(
        "/OPTIONS",
        Arg::None,
        Group::Developer,
        Kind::Mode(Mode::Options),
        "Open the Options screen at once; Esc leaves",
    ),
    sw(
        "/NOLIMIT",
        Arg::None,
        Group::Developer,
        Kind::NoLimit,
        "With /LIVE: do not end the run after 30 seconds",
    ),
    sw(
        "/AUDIO",
        Arg::None,
        Group::Developer,
        Kind::BenchAudio,
        "With /BENCH: run the 560 Hz audio service and the music",
    ),
];

/// The width of the help's name column.
const NAME_COLUMN: usize = 14;

impl Switch {
    /// The text the help's name column shows: `/BENCH [n]`, `/MUSIC:x`, `/LIVE`.
    pub fn display(&self, out: &mut impl fmt::Write) -> fmt::Result {
        out.write_str(self.name)?;
        match self.arg {
            Arg::None => {}
            Arg::Value(v) => write!(out, ":{v}")?,
            Arg::Count => out.write_str(" [n]")?,
        }
        if let Some(p) = self.pair {
            write!(out, " {p}")?;
        }
        Ok(())
    }

    /// Whether `arg` is this switch, by name or alias, with a value where the shape wants one.
    fn matches(&self, arg: &str) -> bool {
        let named = |n: &str| match self.arg {
            Arg::Value(_) => {
                arg.get(..n.len())
                    .is_some_and(|h| h.eq_ignore_ascii_case(n))
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
    /// The key layout a switch chose, if one was given (the last wins); `None` leaves `FIZZ2.CFG`'s choice, or Keen, in force.
    pub keys: Option<Keys>,
    /// The captions a switch chose, if one was given (the last wins); `None` leaves `FIZZ2.CFG`'s choice, or On, in force.
    pub captions: Option<Captions>,
    /// The frame rate a flag chose, if one was given (the last wins); `None` leaves `FIZZ2.CFG`'s choice, or Auto, in force.
    pub frame_rate: Option<FrameRate>,
    /// The sky a `/SKY:` switch chose, if one was given (the last wins); `None` leaves `FIZZ2.CFG`'s choice, or Auto, in force.
    pub sky: Option<SkyDetail>,
    /// The night `/NIGHT` or `/DAY` chose, if one was given (the last wins); `None` leaves `FIZZ2.CFG`'s choice, or Off, in force.
    pub night: Option<Night>,
    /// `/BENCH`'s tick count, when given.
    pub bench_ticks: Option<u64>,
    /// `/AUDIO`: `/BENCH` runs with the 560 Hz service and the music.
    pub bench_audio: bool,
    /// `/NOLIMIT`: `/LIVE` runs until Quit to DOS, not for 30 seconds at most.
    pub no_limit: bool,
}

impl Invocation {
    /// The preferences this run uses: `file`'s, with each field a switch chose replaced by the switch's value (a switch beats the
    /// file, the file beats the default). The sound switches are applied separately by `settings::apply_flags`. The result is for
    /// the run only: the caller saves the file's own `Prefs`, never this.
    pub fn over(&self, file: &Prefs) -> Prefs {
        let mut prefs = *file;
        if let Some(k) = self.keys {
            prefs.keys = k;
        }
        if let Some(c) = self.captions {
            prefs.captions = c;
        }
        if let Some(r) = self.frame_rate {
            prefs.frame_rate = r;
        }
        if let Some(d) = self.sky {
            prefs.sky = d;
        }
        if let Some(n) = self.night {
            prefs.night = n;
        }
        prefs
    }
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
    if args
        .clone()
        .into_iter()
        .any(|a| lookup(a).is_some_and(|s| s.kind == Kind::Help))
    {
        return Parsed::Help;
    }
    let mut run = Invocation {
        mode: Mode::Live,
        keys: None,
        captions: None,
        frame_rate: None,
        sky: None,
        night: None,
        bench_ticks: None,
        bench_audio: false,
        no_limit: false,
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
            Kind::BenchAudio => run.bench_audio = true,
            Kind::NoLimit => run.no_limit = true,
            Kind::Keys(k) => run.keys = Some(k),
            Kind::Captions(c) => run.captions = Some(c),
            Kind::FrameRate(r) => run.frame_rate = Some(r),
            Kind::Sky => match sky_value(arg) {
                Some(d) => run.sky = Some(d),
                None => return Parsed::Unknown(arg),
            },
            Kind::Night(n) => run.night = Some(n),
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

/// The sky a `/SKY:` argument names: `AUTO`, `FULL` or `LIGHT`, in any case; `None` for any other value.
pub fn sky_value(arg: &str) -> Option<SkyDetail> {
    let value = arg.split_once(':')?.1;
    if value.eq_ignore_ascii_case("AUTO") {
        Some(SkyDetail::Auto)
    } else if value.eq_ignore_ascii_case("FULL") {
        Some(SkyDetail::Full)
    } else if value.eq_ignore_ascii_case("LIGHT") {
        Some(SkyDetail::Light)
    } else {
        None
    }
}

/// Writes the help screen: lines ending in `\n` (the caller turns them into the console's line ends), at most 25 lines of 79
/// characters, plain ASCII.
pub fn write_help(out: &mut impl fmt::Write) -> fmt::Result {
    out.write_str(
        "Lieutenant Fizz 2 for DOS. Usage: FIZZ2.EXE [mode] [switches]; case is ignored.\n",
    )?;
    let mut group = None;
    for s in SWITCHES.iter().filter(|s| !s.hidden) {
        if group != Some(s.group) {
            group = Some(s.group);
            if let Some(t) = s.group.title() {
                writeln!(out, "{t}")?;
            }
        }
        let mut name = NameBuf {
            bytes: [0; NAME_COLUMN],
            len: 0,
        };
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
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
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
            assert!(
                l.bytes().all(|b| (0x20..0x7F).contains(&b)),
                "not plain ASCII: {l}"
            );
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
            // A switch is hidden when it is a developer's, or when another's line names it as the opposite of a pair, which is
            // then in the same group, has no pair of its own and appears on that line.
            let paired_by = SWITCHES.iter().find(|o| o.pair == Some(s.name));
            assert_eq!(
                s.hidden,
                s.group == Group::Developer || paired_by.is_some(),
                "{}",
                s.name
            );
            if let Some(o) = paired_by {
                assert_eq!(
                    (o.group, s.pair, o.hidden),
                    (s.group, None, false),
                    "{}",
                    s.name
                );
                assert!(
                    text.lines()
                        .any(|l| l.split_whitespace().nth(1) == Some(s.name))
                );
            }
            if let Some(p) = s.pair {
                assert!(lookup(p).is_some_and(|o| o.hidden), "{p}");
            }
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
            // A pair's line names its opposite second, and that is a switch too.
            if let Some(p) = s.pair {
                assert_eq!(l.split_whitespace().nth(1), Some(p), "{l}");
                assert!(lookup(p).is_some(), "{l}");
            }
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
            assert_eq!(
                apply_flags(&mut Settings::default(), [typed.as_str()]).applied,
                1,
                "{typed}"
            );
        }
        // And no other table line is a sound flag.
        for s in SWITCHES.iter().filter(|s| s.kind != Kind::Sound) {
            assert_eq!(
                apply_flags(&mut Settings::default(), [s.name]).applied,
                0,
                "{}",
                s.name
            );
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
            keys: None,
            captions: None,
            frame_rate: None,
            sky: None,
            night: None,
            bench_ticks: None,
            bench_audio: false,
            no_limit: false,
        };
        let bench = Invocation {
            mode: Mode::Bench,
            ..live
        };
        assert_eq!(run(&[]), Parsed::Run(live));
        assert_eq!(
            run(&["/modern", "/nocaptions"]),
            Parsed::Run(Invocation {
                keys: Some(Keys::Modern),
                captions: Some(Captions::Off),
                ..live
            })
        );
        assert_eq!(
            run(&["/NOCAPTIONS", "/CAPTIONS"]),
            Parsed::Run(Invocation {
                captions: Some(Captions::On),
                ..live
            })
        );
        assert_eq!(
            run(&["/MODERN", "/keen"]),
            Parsed::Run(Invocation {
                keys: Some(Keys::Keen),
                ..live
            })
        );
        // The frame-rate flags combine with any mode and the last one wins.
        assert_eq!(
            run(&["/FPS30", "/REPLAY"]),
            Parsed::Run(Invocation {
                mode: Mode::Replay,
                frame_rate: Some(FrameRate::Hz30),
                ..live
            })
        );
        assert_eq!(
            run(&["/fps30", "/FPS60", "/modern"]),
            Parsed::Run(Invocation {
                keys: Some(Keys::Modern),
                frame_rate: Some(FrameRate::Hz60),
                ..live
            })
        );
        assert_eq!(
            run(&["/FPS60", "/FPSAUTO"]),
            Parsed::Run(Invocation {
                frame_rate: Some(FrameRate::Auto),
                ..live
            })
        );
        assert_eq!(run(&["/FPS30:1"]), Parsed::Unknown("/FPS30:1"));
        // The sky takes FULL or LIGHT in any case, combines with any mode, and the last one wins.
        assert_eq!(
            run(&["/sky:full", "/BENCH"]),
            Parsed::Run(Invocation {
                mode: Mode::Bench,
                sky: Some(SkyDetail::Full),
                ..live
            })
        );
        assert_eq!(
            run(&["/SKY:FULL", "/Sky:Light"]),
            Parsed::Run(Invocation {
                sky: Some(SkyDetail::Light),
                ..live
            })
        );
        for bad in ["/SKY:", "/SKY", "/SKY:DIM", "/SKY:FULLER", "/SKYFULL"] {
            assert_eq!(run(&[bad]), Parsed::Unknown(bad), "{bad}");
        }
        assert_eq!(
            run(&["/REPLAY", "/LIVE"]),
            Parsed::Run(Invocation {
                mode: Mode::Replay,
                ..live
            })
        );
        assert_eq!(
            run(&["/PROBE", "/SB", "/FX:off"]),
            Parsed::Run(Invocation {
                mode: Mode::Probe,
                ..live
            })
        );
        assert_eq!(
            run(&["/PANIC"]),
            Parsed::Run(Invocation {
                mode: Mode::Panic,
                ..live
            })
        );
        assert_eq!(run(&["/BENCH"]), Parsed::Run(bench));
        assert_eq!(
            run(&["/LIVE", "/NOLIMIT"]),
            Parsed::Run(Invocation {
                no_limit: true,
                ..live
            })
        );
        assert_eq!(
            run(&["/BENCH", "/AUDIO"]),
            Parsed::Run(Invocation {
                bench_audio: true,
                ..bench
            })
        );
        assert_eq!(
            run(&["/musictest", "/ADLIB"]),
            Parsed::Run(Invocation {
                mode: Mode::MusicTest,
                ..live
            })
        );
        assert_eq!(
            run(&["/mixtest"]),
            Parsed::Run(Invocation {
                mode: Mode::MixTest,
                ..live
            })
        );
        assert_eq!(
            run(&["/replantest"]),
            Parsed::Run(Invocation {
                mode: Mode::ReplanTest,
                ..live
            })
        );
        assert_eq!(
            run(&["/menushot"]),
            Parsed::Run(Invocation {
                mode: Mode::MenuShot,
                ..live
            })
        );
        assert_eq!(
            run(&["/options"]),
            Parsed::Run(Invocation {
                mode: Mode::Options,
                ..live
            })
        );
        assert_eq!(
            run(&["/BENCH", "120"]),
            Parsed::Run(Invocation {
                bench_ticks: Some(120),
                ..bench
            })
        );
        assert_eq!(
            run(&["120", "/BENCH"]),
            Parsed::Run(Invocation {
                bench_ticks: Some(120),
                ..bench
            })
        );
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

    fn invocation(args: &[&str]) -> Invocation {
        match run(args) {
            Parsed::Run(i) => i,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn night_and_day_set_the_night_for_the_run_and_the_last_wins() {
        assert_eq!(invocation(&["/night"]).night, Some(Night::On));
        assert_eq!(invocation(&["/NIGHT", "/Day"]).night, Some(Night::Off));
        assert_eq!(
            invocation(&["/DAY", "/NIGHT", "/BENCH"]).night,
            Some(Night::On)
        );
        assert_eq!(invocation(&["/BENCH"]).night, None);
        assert_eq!(invocation(&["/BENCH"]).mode, Mode::Bench);
        let file = Prefs {
            night: Night::On,
            ..Prefs::default()
        };
        assert_eq!(invocation(&[]).over(&file), file);
        assert_eq!(invocation(&["/DAY"]).over(&file), Prefs::default());
        assert_eq!(invocation(&["/NIGHT"]).over(&Prefs::default()), file);
        for bad in ["/NIGHT:ON", "/NIGHTS", "/DAYS", "/DAY:1"] {
            assert_eq!(run(&[bad]), Parsed::Unknown(bad), "{bad}");
        }
    }

    #[test]
    fn a_switch_beats_the_file_and_the_file_beats_the_default() {
        let file = Prefs {
            keys: Keys::Modern,
            captions: Captions::Off,
            frame_rate: FrameRate::Hz30,
            ..Prefs::default()
        };
        // No switches: the file stands.
        assert_eq!(invocation(&[]).over(&file), file);
        // Each switch overrides only its own field.
        assert_eq!(
            invocation(&["/KEEN"]).over(&file),
            Prefs {
                keys: Keys::Keen,
                ..file
            }
        );
        assert_eq!(
            invocation(&["/CAPTIONS"]).over(&file),
            Prefs {
                captions: Captions::On,
                ..file
            }
        );
        assert_eq!(
            invocation(&["/FPS60"]).over(&file),
            Prefs {
                frame_rate: FrameRate::Hz60,
                ..file
            }
        );
        assert_eq!(
            invocation(&["/SKY:FULL"]).over(&file),
            Prefs {
                sky: SkyDetail::Full,
                ..file
            }
        );
        assert_eq!(
            invocation(&["/SKY:LIGHT"]).over(&file),
            Prefs {
                sky: SkyDetail::Light,
                ..file
            }
        );
        assert_eq!(
            invocation(&["/sky:auto"]).over(&Prefs {
                sky: SkyDetail::Full,
                ..file
            }),
            file
        );
        // Over a default file, /MODERN and /NOCAPTIONS apply; the last of a pair wins.
        let none = Prefs::default();
        assert_eq!(
            invocation(&["/MODERN", "/NOCAPTIONS"]).over(&none),
            Prefs {
                keys: Keys::Modern,
                captions: Captions::Off,
                ..none
            }
        );
        assert_eq!(
            invocation(&["/MODERN", "/KEEN"]).over(&file).keys,
            Keys::Keen
        );
        assert_eq!(
            invocation(&["/KEEN", "/MODERN"]).over(&none).keys,
            Keys::Modern
        );
        // The file itself is untouched, so saving it never writes a switch.
        let _ = invocation(&["/KEEN", "/CAPTIONS"]).over(&file);
        assert_eq!(file.keys, Keys::Modern);
    }
}
