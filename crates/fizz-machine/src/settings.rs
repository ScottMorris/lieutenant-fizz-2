// The player's sound settings: the choices, the 8-byte `FIZZ2.CFG` layout and the command-line flags that override it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each field is Auto unless the file or a flag says otherwise. The file is saved whenever the player changes a setting; flags
// are one-run overrides and never reach `Config::encode`. Reading and writing the file itself belongs to the DOS runtime.

/// Which device plays music. The PC speaker cannot play it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MusicChoice {
    #[default]
    Auto,
    Off,
    /// An OPL2 in mono.
    AdLib,
    /// The Sound Blaster's OPL3 in stereo.
    Sb,
}

/// Which device plays sound effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FxChoice {
    #[default]
    Auto,
    Off,
    /// OPL2 channel 5, centred.
    AdLib,
    /// OPL3 channel 14 in bank 1.
    Sb,
    Speaker,
}

/// The player's sound choices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub music: MusicChoice,
    pub fx: FxChoice,
}

/// Why part of `FIZZ2.CFG` was not used. The field or fields concerned fall back to Auto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigIssue {
    /// The file is not 8 bytes long.
    BadLength,
    /// The first four bytes are not `FZ2C`.
    BadMagic,
    /// The version byte is not 1.
    BadVersion,
    /// The music byte is not 0 to 3.
    BadMusic,
    /// The effects byte is not 0 to 4.
    BadEffects,
}

impl ConfigIssue {
    /// A short fixed word for the `SOUND` line.
    pub fn code(self) -> &'static str {
        match self {
            ConfigIssue::BadLength => "cfg-length",
            ConfigIssue::BadMagic => "cfg-magic",
            ConfigIssue::BadVersion => "cfg-version",
            ConfigIssue::BadMusic => "cfg-music",
            ConfigIssue::BadEffects => "cfg-effects",
        }
    }
}

/// The `FIZZ2.CFG` file: 8 bytes. Bytes 0 to 3 are `FZ2C`, byte 4 the version (1), byte 5 the music choice (0 Auto, 1 Off, 2
/// AdLib, 3 Sound Blaster), byte 6 the effects choice (0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster, 4 Speaker), byte 7 reserved (0).
pub struct Config;

impl Config {
    /// The file's name, in the current directory.
    pub const FILE_NAME: &'static str = "FIZZ2.CFG";
    /// The file's length in bytes.
    pub const LEN: usize = 8;
    const MAGIC: [u8; 4] = *b"FZ2C";
    const VERSION: u8 = 1;

    /// Reads a file's bytes. An empty slice (no file, or an empty one) is Auto with no issue. A wrong length, magic or version
    /// makes both fields Auto; an unknown value makes only its own field Auto. The first issue found is returned. The reserved
    /// byte is ignored so a later version can use it.
    pub fn parse(bytes: &[u8]) -> (Settings, Option<ConfigIssue>) {
        if bytes.is_empty() {
            return (Settings::default(), None);
        }
        if bytes.len() != Self::LEN {
            return (Settings::default(), Some(ConfigIssue::BadLength));
        }
        if bytes[..4] != Self::MAGIC {
            return (Settings::default(), Some(ConfigIssue::BadMagic));
        }
        if bytes[4] != Self::VERSION {
            return (Settings::default(), Some(ConfigIssue::BadVersion));
        }
        let mut settings = Settings::default();
        let mut issue = None;
        match bytes[5] {
            0 => {}
            1 => settings.music = MusicChoice::Off,
            2 => settings.music = MusicChoice::AdLib,
            3 => settings.music = MusicChoice::Sb,
            _ => issue = Some(ConfigIssue::BadMusic),
        }
        match bytes[6] {
            0 => {}
            1 => settings.fx = FxChoice::Off,
            2 => settings.fx = FxChoice::AdLib,
            3 => settings.fx = FxChoice::Sb,
            4 => settings.fx = FxChoice::Speaker,
            _ => issue = issue.or(Some(ConfigIssue::BadEffects)),
        }
        (settings, issue)
    }

    /// The bytes to save for `settings`.
    pub fn encode(settings: &Settings) -> [u8; 8] {
        let music = match settings.music {
            MusicChoice::Auto => 0,
            MusicChoice::Off => 1,
            MusicChoice::AdLib => 2,
            MusicChoice::Sb => 3,
        };
        let fx = match settings.fx {
            FxChoice::Auto => 0,
            FxChoice::Off => 1,
            FxChoice::AdLib => 2,
            FxChoice::Sb => 3,
            FxChoice::Speaker => 4,
        };
        [
            Self::MAGIC[0],
            Self::MAGIC[1],
            Self::MAGIC[2],
            Self::MAGIC[3],
            Self::VERSION,
            music,
            fx,
            0,
        ]
    }
}

/// What `apply_flags` did with the arguments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlagReport {
    /// How many arguments changed or confirmed a sound choice.
    pub applied: usize,
    /// How many arguments were not a sound flag. They include the program's own mode flags (`/LIVE`, `/CHECK`), which the
    /// caller reads itself, and a sound flag with a bad value such as `/MUSIC:FOO`.
    pub unrecognised: usize,
    /// The index in `args` of the first such argument, so the caller can name it in a warning.
    pub first_unrecognised: Option<usize>,
}

fn after_prefix<'a>(arg: &'a str, prefix: &str) -> Option<&'a str> {
    let head = arg.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        arg.get(prefix.len()..)
    } else {
        None
    }
}

fn music_value(v: &str) -> Option<MusicChoice> {
    Some(match () {
        _ if v.eq_ignore_ascii_case("ADLIB") => MusicChoice::AdLib,
        _ if v.eq_ignore_ascii_case("SB") => MusicChoice::Sb,
        _ if v.eq_ignore_ascii_case("OFF") => MusicChoice::Off,
        _ => return None,
    })
}

fn fx_value(v: &str) -> Option<FxChoice> {
    Some(match () {
        _ if v.eq_ignore_ascii_case("ADLIB") => FxChoice::AdLib,
        _ if v.eq_ignore_ascii_case("SB") => FxChoice::Sb,
        _ if v.eq_ignore_ascii_case("SPEAKER") => FxChoice::Speaker,
        _ if v.eq_ignore_ascii_case("OFF") => FxChoice::Off,
        _ => return None,
    })
}

/// Applies the command-line flags to `settings`, left to right, so a flag wins over the file and a later flag over an earlier
/// one. The flags are `/MUSIC:ADLIB|SB|OFF`, `/FX:ADLIB|SB|SPEAKER|OFF` and the shortcuts `/ADLIB` (both AdLib), `/SB` (both
/// Sound Blaster), `/SPEAKER` (effects on the speaker, music off) and `/NOSOUND` (both off), all case-insensitive. Anything else
/// is left alone and counted in the report. Flags are never saved: pass only the file's `Settings` to `Config::encode`.
pub fn apply_flags<'a>(
    settings: &mut Settings,
    args: impl IntoIterator<Item = &'a str>,
) -> FlagReport {
    let mut report = FlagReport::default();
    for (i, arg) in args.into_iter().enumerate() {
        let applied = if let Some(v) = after_prefix(arg, "/MUSIC:") {
            music_value(v).map(|m| settings.music = m).is_some()
        } else if let Some(v) = after_prefix(arg, "/FX:") {
            fx_value(v).map(|f| settings.fx = f).is_some()
        } else if arg.eq_ignore_ascii_case("/ADLIB") {
            *settings = Settings {
                music: MusicChoice::AdLib,
                fx: FxChoice::AdLib,
            };
            true
        } else if arg.eq_ignore_ascii_case("/SB") {
            *settings = Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Sb,
            };
            true
        } else if arg.eq_ignore_ascii_case("/SPEAKER") {
            *settings = Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Speaker,
            };
            true
        } else if arg.eq_ignore_ascii_case("/NOSOUND") {
            *settings = Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Off,
            };
            true
        } else {
            false
        };
        if applied {
            report.applied += 1;
        } else {
            report.unrecognised += 1;
            report.first_unrecognised.get_or_insert(i);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const MUSIC: [MusicChoice; 4] = [
        MusicChoice::Auto,
        MusicChoice::Off,
        MusicChoice::AdLib,
        MusicChoice::Sb,
    ];
    const FX: [FxChoice; 5] = [
        FxChoice::Auto,
        FxChoice::Off,
        FxChoice::AdLib,
        FxChoice::Sb,
        FxChoice::Speaker,
    ];

    #[test]
    fn round_trip_every_pair() {
        for music in MUSIC {
            for fx in FX {
                let s = Settings { music, fx };
                let bytes = Config::encode(&s);
                assert_eq!(&bytes[..4], b"FZ2C");
                assert_eq!(bytes[4], 1);
                assert_eq!(bytes[7], 0);
                assert_eq!(Config::parse(&bytes), (s, None));
            }
        }
    }

    #[test]
    fn layout_is_exact() {
        let s = Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Speaker,
        };
        assert_eq!(Config::encode(&s), [b'F', b'Z', b'2', b'C', 1, 3, 4, 0]);
        assert_eq!(
            Config::encode(&Settings::default()),
            [b'F', b'Z', b'2', b'C', 1, 0, 0, 0]
        );
    }

    #[test]
    fn a_missing_or_empty_file_is_auto_without_a_reason() {
        assert_eq!(Config::parse(&[]), (Settings::default(), None));
    }

    #[test]
    fn wrong_length_is_auto_with_a_reason() {
        let good = Config::encode(&Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Sb,
        });
        for len in [1, 4, 7] {
            assert_eq!(
                Config::parse(&good[..len]),
                (Settings::default(), Some(ConfigIssue::BadLength))
            );
        }
        let mut long = [0u8; 9];
        long[..8].copy_from_slice(&good);
        assert_eq!(
            Config::parse(&long),
            (Settings::default(), Some(ConfigIssue::BadLength))
        );
    }

    #[test]
    fn wrong_magic_and_version_are_auto_with_a_reason() {
        let mut bytes = Config::encode(&Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Sb,
        });
        bytes[0] = b'X';
        assert_eq!(
            Config::parse(&bytes),
            (Settings::default(), Some(ConfigIssue::BadMagic))
        );
        let mut bytes = Config::encode(&Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Sb,
        });
        bytes[4] = 2;
        assert_eq!(
            Config::parse(&bytes),
            (Settings::default(), Some(ConfigIssue::BadVersion))
        );
        bytes[4] = 0;
        assert_eq!(
            Config::parse(&bytes),
            (Settings::default(), Some(ConfigIssue::BadVersion))
        );
    }

    #[test]
    fn an_unknown_value_resets_only_its_field() {
        let mut bytes = Config::encode(&Settings {
            music: MusicChoice::AdLib,
            fx: FxChoice::Speaker,
        });
        bytes[5] = 4;
        assert_eq!(
            Config::parse(&bytes),
            (
                Settings {
                    music: MusicChoice::Auto,
                    fx: FxChoice::Speaker
                },
                Some(ConfigIssue::BadMusic)
            )
        );
        let mut bytes = Config::encode(&Settings {
            music: MusicChoice::AdLib,
            fx: FxChoice::Speaker,
        });
        bytes[6] = 5;
        assert_eq!(
            Config::parse(&bytes),
            (
                Settings {
                    music: MusicChoice::AdLib,
                    fx: FxChoice::Auto
                },
                Some(ConfigIssue::BadEffects)
            )
        );
        bytes[5] = 200;
        bytes[6] = 255;
        assert_eq!(
            Config::parse(&bytes),
            (Settings::default(), Some(ConfigIssue::BadMusic))
        );
    }

    #[test]
    fn the_reserved_byte_is_ignored() {
        let mut bytes = Config::encode(&Settings {
            music: MusicChoice::Off,
            fx: FxChoice::Off,
        });
        bytes[7] = 9;
        assert_eq!(
            Config::parse(&bytes),
            (
                Settings {
                    music: MusicChoice::Off,
                    fx: FxChoice::Off
                },
                None
            )
        );
    }

    #[test]
    fn issue_codes_are_distinct() {
        let all = [
            ConfigIssue::BadLength,
            ConfigIssue::BadMagic,
            ConfigIssue::BadVersion,
            ConfigIssue::BadMusic,
            ConfigIssue::BadEffects,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
        }
    }

    fn flags(base: Settings, args: &[&str]) -> (Settings, FlagReport) {
        let mut s = base;
        let r = apply_flags(&mut s, args.iter().copied());
        (s, r)
    }

    #[test]
    fn each_flag_value() {
        let auto = Settings::default();
        assert_eq!(flags(auto, &["/MUSIC:ADLIB"]).0.music, MusicChoice::AdLib);
        assert_eq!(flags(auto, &["/MUSIC:SB"]).0.music, MusicChoice::Sb);
        assert_eq!(flags(auto, &["/MUSIC:OFF"]).0.music, MusicChoice::Off);
        assert_eq!(flags(auto, &["/FX:ADLIB"]).0.fx, FxChoice::AdLib);
        assert_eq!(flags(auto, &["/FX:SB"]).0.fx, FxChoice::Sb);
        assert_eq!(flags(auto, &["/FX:SPEAKER"]).0.fx, FxChoice::Speaker);
        assert_eq!(flags(auto, &["/FX:OFF"]).0.fx, FxChoice::Off);
    }

    #[test]
    fn shortcuts() {
        let auto = Settings::default();
        assert_eq!(
            flags(auto, &["/ADLIB"]).0,
            Settings {
                music: MusicChoice::AdLib,
                fx: FxChoice::AdLib
            }
        );
        assert_eq!(
            flags(auto, &["/SB"]).0,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Sb
            }
        );
        assert_eq!(
            flags(auto, &["/SPEAKER"]).0,
            Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Speaker
            }
        );
        assert_eq!(
            flags(auto, &["/NOSOUND"]).0,
            Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Off
            }
        );
    }

    #[test]
    fn case_does_not_matter() {
        let (s, r) = flags(Settings::default(), &["/music:Sb", "/Fx:speaker"]);
        assert_eq!(
            s,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker
            }
        );
        assert_eq!(
            r,
            FlagReport {
                applied: 2,
                unrecognised: 0,
                first_unrecognised: None
            }
        );
        assert_eq!(
            flags(Settings::default(), &["/nosound"]).0.fx,
            FxChoice::Off
        );
    }

    #[test]
    fn flags_win_over_the_file_per_field() {
        let file = Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Sb,
        };
        assert_eq!(
            flags(file, &["/FX:SPEAKER"]).0,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker
            }
        );
        assert_eq!(
            flags(file, &["/MUSIC:OFF"]).0,
            Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Sb
            }
        );
        assert_eq!(
            flags(file, &["/NOSOUND"]).0,
            Settings {
                music: MusicChoice::Off,
                fx: FxChoice::Off
            }
        );
    }

    #[test]
    fn a_later_flag_wins_over_an_earlier_one() {
        let (s, _) = flags(Settings::default(), &["/ADLIB", "/FX:SPEAKER"]);
        assert_eq!(
            s,
            Settings {
                music: MusicChoice::AdLib,
                fx: FxChoice::Speaker
            }
        );
        let (s, _) = flags(Settings::default(), &["/FX:SPEAKER", "/SB"]);
        assert_eq!(
            s,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Sb
            }
        );
    }

    #[test]
    fn unknown_flags_are_ignored_and_reported() {
        let file = Settings {
            music: MusicChoice::AdLib,
            fx: FxChoice::AdLib,
        };
        let (s, r) = flags(
            file,
            &["/LIVE", "/MUSIC:FOO", "/FX:", "/MUSIC", "/SB16", "plain"],
        );
        assert_eq!(s, file);
        assert_eq!(
            r,
            FlagReport {
                applied: 0,
                unrecognised: 6,
                first_unrecognised: Some(0)
            }
        );
        let (s, r) = flags(file, &["/CHECK", "/FX:OFF"]);
        assert_eq!(s.fx, FxChoice::Off);
        assert_eq!(
            r,
            FlagReport {
                applied: 1,
                unrecognised: 1,
                first_unrecognised: Some(0)
            }
        );
    }

    #[test]
    fn flags_never_reach_the_saved_bytes() {
        let file = Settings {
            music: MusicChoice::AdLib,
            fx: FxChoice::Auto,
        };
        let saved = Config::encode(&file);
        let (flagged, _) = flags(file, &["/NOSOUND"]);
        assert_ne!(flagged, file);
        // The caller saves the file's settings, not the flagged ones; encoding is a pure function of what it is given.
        assert_eq!(Config::encode(&file), saved);
    }

    #[test]
    fn non_ascii_arguments_do_not_panic() {
        let (s, r) = flags(Settings::default(), &["/MUSIC:é", "é", "/μ", "/FX:ÀDLIB"]);
        assert_eq!(s, Settings::default());
        assert_eq!(r.unrecognised, 4);
    }
}
