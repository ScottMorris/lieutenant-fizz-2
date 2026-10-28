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

/// The frame rate the player asked for. Nothing reads it yet; the file keeps it so the Options screen can save it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameRate {
    #[default]
    Auto,
    /// 30 Hz.
    Hz30,
    /// 60 Hz.
    Hz60,
}

/// Everything `FIZZ2.CFG` holds: the sound choices and the frame rate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub sound: Settings,
    pub frame_rate: FrameRate,
}

/// Why part of `FIZZ2.CFG` was not used. The field or fields concerned fall back to Auto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigIssue {
    /// The file's length does not match its version (8 bytes for version 1, 9 for version 2).
    BadLength,
    /// The first four bytes are not `FZ2C`.
    BadMagic,
    /// The version byte is not 1 or 2.
    BadVersion,
    /// The music byte is not 0 to 3.
    BadMusic,
    /// The effects byte is not 0 to 4.
    BadEffects,
    /// The frame-rate byte is not 0 to 2.
    BadFrameRate,
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
            ConfigIssue::BadFrameRate => "cfg-framerate",
        }
    }
}

/// The `FIZZ2.CFG` file. Version 2 is 9 bytes: bytes 0 to 3 are `FZ2C`, byte 4 the version (2), byte 5 the music choice (0 Auto,
/// 1 Off, 2 AdLib, 3 Sound Blaster), byte 6 the effects choice (0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster, 4 Speaker), byte 7 the frame
/// rate (0 Auto, 1 30 Hz, 2 60 Hz) and byte 8 the detail level (reserved, written as 0, ignored on reading). Version 1, which an
/// earlier build wrote, is 8 bytes (version 1, music, effects, one reserved byte); it is still read, with the frame rate Auto.
pub struct Config;

impl Config {
    /// The file's name, in the current directory.
    pub const FILE_NAME: &'static str = "FIZZ2.CFG";
    /// The length of a version 2 file in bytes: the length `encode` writes.
    pub const LEN: usize = 9;
    /// The length of a version 1 file in bytes.
    pub const LEN_V1: usize = 8;
    const MAGIC: [u8; 4] = *b"FZ2C";
    const VERSION: u8 = 2;

    /// Reads a file's bytes as sound `Settings` only; see `parse_prefs`.
    pub fn parse(bytes: &[u8]) -> (Settings, Option<ConfigIssue>) {
        let (prefs, issue) = Self::parse_prefs(bytes);
        (prefs.sound, issue)
    }

    /// Reads a file's bytes. An empty slice (no file, or an empty one) is Auto with no issue. A wrong length, magic or version
    /// makes every field Auto; an unknown value makes only its own field Auto. The first issue found is returned. A version 1 file
    /// gives its music and effects and leaves the frame rate Auto. The reserved bytes are ignored so a later version can use them.
    pub fn parse_prefs(bytes: &[u8]) -> (Prefs, Option<ConfigIssue>) {
        if bytes.is_empty() {
            return (Prefs::default(), None);
        }
        if bytes.len() < 5 {
            return (Prefs::default(), Some(ConfigIssue::BadLength));
        }
        if bytes[..4] != Self::MAGIC {
            return (Prefs::default(), Some(ConfigIssue::BadMagic));
        }
        let want = match bytes[4] {
            1 => Self::LEN_V1,
            2 => Self::LEN,
            _ => return (Prefs::default(), Some(ConfigIssue::BadVersion)),
        };
        if bytes.len() != want {
            return (Prefs::default(), Some(ConfigIssue::BadLength));
        }
        let mut prefs = Prefs::default();
        let mut issue = None;
        match bytes[5] {
            0 => {}
            1 => prefs.sound.music = MusicChoice::Off,
            2 => prefs.sound.music = MusicChoice::AdLib,
            3 => prefs.sound.music = MusicChoice::Sb,
            _ => issue = Some(ConfigIssue::BadMusic),
        }
        match bytes[6] {
            0 => {}
            1 => prefs.sound.fx = FxChoice::Off,
            2 => prefs.sound.fx = FxChoice::AdLib,
            3 => prefs.sound.fx = FxChoice::Sb,
            4 => prefs.sound.fx = FxChoice::Speaker,
            _ => issue = issue.or(Some(ConfigIssue::BadEffects)),
        }
        if bytes[4] == 2 {
            match bytes[7] {
                0 => {}
                1 => prefs.frame_rate = FrameRate::Hz30,
                2 => prefs.frame_rate = FrameRate::Hz60,
                _ => issue = issue.or(Some(ConfigIssue::BadFrameRate)),
            }
        }
        (prefs, issue)
    }

    /// The bytes to save for `prefs`: always a version 2 file.
    pub fn encode_prefs(prefs: &Prefs) -> [u8; Self::LEN] {
        let music = match prefs.sound.music {
            MusicChoice::Auto => 0,
            MusicChoice::Off => 1,
            MusicChoice::AdLib => 2,
            MusicChoice::Sb => 3,
        };
        let fx = match prefs.sound.fx {
            FxChoice::Auto => 0,
            FxChoice::Off => 1,
            FxChoice::AdLib => 2,
            FxChoice::Sb => 3,
            FxChoice::Speaker => 4,
        };
        let frame_rate = match prefs.frame_rate {
            FrameRate::Auto => 0,
            FrameRate::Hz30 => 1,
            FrameRate::Hz60 => 2,
        };
        [
            Self::MAGIC[0],
            Self::MAGIC[1],
            Self::MAGIC[2],
            Self::MAGIC[3],
            Self::VERSION,
            music,
            fx,
            frame_rate,
            0,
        ]
    }

    /// The bytes to save for sound `settings` with the frame rate Auto. A caller that holds a frame rate uses `encode_prefs`, or the
    /// save would reset it.
    pub fn encode(settings: &Settings) -> [u8; Self::LEN] {
        Self::encode_prefs(&Prefs {
            sound: *settings,
            frame_rate: FrameRate::Auto,
        })
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

    const RATES: [FrameRate; 3] = [FrameRate::Auto, FrameRate::Hz30, FrameRate::Hz60];

    fn sample() -> Prefs {
        Prefs {
            sound: Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker,
            },
            frame_rate: FrameRate::Hz60,
        }
    }

    #[test]
    fn round_trip_every_combination() {
        for music in MUSIC {
            for fx in FX {
                for frame_rate in RATES {
                    let p = Prefs {
                        sound: Settings { music, fx },
                        frame_rate,
                    };
                    let bytes = Config::encode_prefs(&p);
                    assert_eq!(bytes.len(), 9);
                    assert_eq!(&bytes[..4], b"FZ2C");
                    assert_eq!(bytes[4], 2);
                    assert_eq!(bytes[8], 0);
                    assert_eq!(Config::parse_prefs(&bytes), (p, None));
                    assert_eq!(Config::parse(&bytes), (p.sound, None));
                }
            }
        }
    }

    #[test]
    fn layout_is_exact() {
        assert_eq!(
            Config::encode_prefs(&sample()),
            [b'F', b'Z', b'2', b'C', 2, 3, 4, 2, 0]
        );
        assert_eq!(
            Config::encode(&Settings::default()),
            [b'F', b'Z', b'2', b'C', 2, 0, 0, 0, 0]
        );
        assert_eq!(Config::encode(&sample().sound)[7], 0);
    }

    #[test]
    fn a_version_1_file_still_reads_with_the_new_fields_auto() {
        for music in MUSIC {
            for fx in FX {
                let v2 = Config::encode(&Settings { music, fx });
                let mut v1 = [0u8; 8];
                v1.copy_from_slice(&v2[..8]);
                v1[4] = 1;
                v1[7] = 0;
                assert_eq!(
                    Config::parse_prefs(&v1),
                    (
                        Prefs {
                            sound: Settings { music, fx },
                            frame_rate: FrameRate::Auto
                        },
                        None
                    )
                );
            }
        }
        // The old layout exactly, as the first build wrote it.
        let old = [b'F', b'Z', b'2', b'C', 1, 3, 4, 0];
        assert_eq!(Config::parse(&old).0, sample().sound);
        // Saving it again upgrades it to version 2.
        assert_eq!(Config::encode(&Config::parse(&old).0)[4], 2);
    }

    #[test]
    fn a_missing_or_empty_file_is_auto_without_a_reason() {
        assert_eq!(Config::parse_prefs(&[]), (Prefs::default(), None));
        assert_eq!(Config::parse(&[]), (Settings::default(), None));
    }

    #[test]
    fn wrong_length_is_auto_with_a_reason() {
        let good = Config::encode_prefs(&sample());
        // Too short, and a version 2 header on a version 1 body.
        for len in [1, 4, 5, 7, 8] {
            assert_eq!(
                Config::parse_prefs(&good[..len]),
                (Prefs::default(), Some(ConfigIssue::BadLength)),
                "{len}"
            );
        }
        let mut long = [2u8; 10];
        long[..9].copy_from_slice(&good);
        assert_eq!(
            Config::parse_prefs(&long),
            (Prefs::default(), Some(ConfigIssue::BadLength))
        );
        // A version 1 header on a 9-byte body.
        let mut v1_long = good;
        v1_long[4] = 1;
        assert_eq!(
            Config::parse_prefs(&v1_long),
            (Prefs::default(), Some(ConfigIssue::BadLength))
        );
        // A 7-byte version 1 file.
        let mut v1_short = [0u8; 7];
        v1_short.copy_from_slice(&good[..7]);
        v1_short[4] = 1;
        assert_eq!(
            Config::parse_prefs(&v1_short),
            (Prefs::default(), Some(ConfigIssue::BadLength))
        );
    }

    #[test]
    fn wrong_magic_and_version_are_auto_with_a_reason() {
        let mut bytes = Config::encode_prefs(&sample());
        bytes[0] = b'X';
        assert_eq!(
            Config::parse_prefs(&bytes),
            (Prefs::default(), Some(ConfigIssue::BadMagic))
        );
        for version in [0u8, 3, 255] {
            let mut bytes = Config::encode_prefs(&sample());
            bytes[4] = version;
            assert_eq!(
                Config::parse_prefs(&bytes),
                (Prefs::default(), Some(ConfigIssue::BadVersion)),
                "{version}"
            );
        }
    }

    #[test]
    fn an_unknown_value_resets_only_its_field() {
        let base = Prefs {
            sound: Settings {
                music: MusicChoice::AdLib,
                fx: FxChoice::Speaker,
            },
            frame_rate: FrameRate::Hz30,
        };
        let mut bytes = Config::encode_prefs(&base);
        bytes[5] = 4;
        let (p, issue) = Config::parse_prefs(&bytes);
        assert_eq!(issue, Some(ConfigIssue::BadMusic));
        assert_eq!(p.sound.music, MusicChoice::Auto);
        assert_eq!(
            (p.sound.fx, p.frame_rate),
            (FxChoice::Speaker, FrameRate::Hz30)
        );

        let mut bytes = Config::encode_prefs(&base);
        bytes[6] = 5;
        let (p, issue) = Config::parse_prefs(&bytes);
        assert_eq!(issue, Some(ConfigIssue::BadEffects));
        assert_eq!(p.sound.fx, FxChoice::Auto);
        assert_eq!(
            (p.sound.music, p.frame_rate),
            (MusicChoice::AdLib, FrameRate::Hz30)
        );

        let mut bytes = Config::encode_prefs(&base);
        bytes[7] = 3;
        let (p, issue) = Config::parse_prefs(&bytes);
        assert_eq!(issue, Some(ConfigIssue::BadFrameRate));
        assert_eq!(p.frame_rate, FrameRate::Auto);
        assert_eq!(p.sound, base.sound);

        // Several bad bytes: the first issue is reported and every bad field is Auto.
        bytes[5] = 200;
        bytes[6] = 255;
        assert_eq!(
            Config::parse_prefs(&bytes),
            (Prefs::default(), Some(ConfigIssue::BadMusic))
        );
    }

    #[test]
    fn the_reserved_byte_is_ignored() {
        let mut bytes = Config::encode_prefs(&sample());
        bytes[8] = 9;
        assert_eq!(Config::parse_prefs(&bytes), (sample(), None));
        let v1 = [b'F', b'Z', b'2', b'C', 1, 1, 1, 9];
        assert_eq!(
            Config::parse(&v1),
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
            ConfigIssue::BadFrameRate,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
        }
    }

    #[test]
    fn file_then_flags_precedence_and_flags_are_not_saved() {
        // Per field: a flag beats the file, the file beats Auto; sound flags leave the frame rate alone.
        let file = Prefs {
            sound: Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Auto,
            },
            frame_rate: FrameRate::Hz30,
        };
        let (read, _) = Config::parse_prefs(&Config::encode_prefs(&file));
        let mut effective = read.sound;
        apply_flags(&mut effective, ["/FX:SPEAKER"]);
        assert_eq!(
            effective,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker
            }
        );
        // What a later save writes is the file's own prefs: effects stay Auto, the frame rate stays.
        let saved = Config::encode_prefs(&read);
        assert_eq!((saved[5], saved[6], saved[7]), (3, 0, 1));
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
