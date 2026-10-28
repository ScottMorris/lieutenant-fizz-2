// The player's settings: the choices, the 13-byte `FIZZ2.CFG` layout and the command-line flags that override it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Each field is its default (Auto for the sound and the frame rate) unless the file or a flag says otherwise. The file is saved whenever the player changes a setting; flags
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

/// The frame rate the player asked for. `FIZZ2.CFG` keeps it, and a `/FPS30`, `/FPS60` or `/FPSAUTO` switch overrides it for one run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameRate {
    #[default]
    Auto,
    /// 30 Hz.
    Hz30,
    /// 60 Hz.
    Hz60,
}

/// Whether the screen moves for effect: the death shake and the death spin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Full,
    Reduced,
}

/// How effects are placed in the stereo field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlacementMode {
    /// Panned and attenuated by where the source is in the view.
    #[default]
    Enhanced,
    /// Always centred at full volume, as the first game played them.
    Classic,
}

/// Whether the chicken's captions are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Captions {
    #[default]
    On,
    Off,
}

/// The key layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Keys {
    /// The layout Keen used.
    #[default]
    Keen,
    /// W A S D with Z, X and C.
    Modern,
}

/// Everything `FIZZ2.CFG` holds: the sound choices, the frame rate and the choices about motion, placement, captions and keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub sound: Settings,
    pub frame_rate: FrameRate,
    pub motion: Motion,
    pub placement: PlacementMode,
    pub captions: Captions,
    pub keys: Keys,
}

/// Why part of `FIZZ2.CFG` was not used. The field or fields concerned fall back to their defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigIssue {
    /// The file's length does not match its version (8 bytes for version 1, 9 for version 2, 13 for version 3).
    BadLength,
    /// The first four bytes are not `FZ2C`.
    BadMagic,
    /// The version byte is not 1, 2 or 3.
    BadVersion,
    /// The music byte is not 0 to 3.
    BadMusic,
    /// The effects byte is not 0 to 4.
    BadEffects,
    /// The frame-rate byte is not 0 to 2.
    BadFrameRate,
    /// The motion byte is not 0 or 1.
    BadMotion,
    /// The placement byte is not 0 or 1.
    BadPlacement,
    /// The captions byte is not 0 or 1.
    BadCaptions,
    /// The keys byte is not 0 or 1.
    BadKeys,
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
            ConfigIssue::BadMotion => "cfg-motion",
            ConfigIssue::BadPlacement => "cfg-placement",
            ConfigIssue::BadCaptions => "cfg-captions",
            ConfigIssue::BadKeys => "cfg-keys",
        }
    }
}

/// The `FIZZ2.CFG` file. Version 3 is 13 bytes: bytes 0 to 3 are `FZ2C`, byte 4 the version (3), byte 5 the music choice (0 Auto,
/// 1 Off, 2 AdLib, 3 Sound Blaster), byte 6 the effects choice (0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster, 4 Speaker), byte 7 the frame
/// rate (0 Auto, 1 30 Hz, 2 60 Hz), byte 8 the detail level (reserved, written as 0, ignored on reading), byte 9 the motion (0 Full,
/// 1 Reduced), byte 10 the sound placement (0 Enhanced, 1 Classic), byte 11 the captions (0 On, 1 Off) and byte 12 the key layout
/// (0 Keen, 1 Modern). Zero always means the default. Version 2 is the first nine bytes with version 2, and version 1 the first
/// eight (music, effects and a reserved byte); both are still read, with the fields they lack at their defaults, and saving writes
/// version 3.
pub struct Config;

impl Config {
    /// The file's name, in the current directory.
    pub const FILE_NAME: &'static str = "FIZZ2.CFG";
    /// The length of a version 3 file in bytes: the length `encode_prefs` writes.
    pub const LEN: usize = 13;
    /// The length of a version 2 file in bytes.
    pub const LEN_V2: usize = 9;
    /// The length of a version 1 file in bytes.
    pub const LEN_V1: usize = 8;
    const MAGIC: [u8; 4] = *b"FZ2C";
    const VERSION: u8 = 3;

    /// Reads a file's bytes as sound `Settings` only; see `parse_prefs`.
    pub fn parse(bytes: &[u8]) -> (Settings, Option<ConfigIssue>) {
        let (prefs, issue) = Self::parse_prefs(bytes);
        (prefs.sound, issue)
    }

    /// Reads a file's bytes. An empty slice (no file, or an empty one) is all defaults with no issue. A wrong length, magic or
    /// version makes every field its default; an unknown value makes only its own field its default. The first issue found is
    /// returned. An older version gives the fields it has and leaves the rest at their defaults. The reserved byte is ignored.
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
        let version = bytes[4];
        let want = match version {
            1 => Self::LEN_V1,
            2 => Self::LEN_V2,
            3 => Self::LEN,
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
        if version >= 2 {
            match bytes[7] {
                0 => {}
                1 => prefs.frame_rate = FrameRate::Hz30,
                2 => prefs.frame_rate = FrameRate::Hz60,
                _ => issue = issue.or(Some(ConfigIssue::BadFrameRate)),
            }
        }
        if version >= 3 {
            match bytes[9] {
                0 => {}
                1 => prefs.motion = Motion::Reduced,
                _ => issue = issue.or(Some(ConfigIssue::BadMotion)),
            }
            match bytes[10] {
                0 => {}
                1 => prefs.placement = PlacementMode::Classic,
                _ => issue = issue.or(Some(ConfigIssue::BadPlacement)),
            }
            match bytes[11] {
                0 => {}
                1 => prefs.captions = Captions::Off,
                _ => issue = issue.or(Some(ConfigIssue::BadCaptions)),
            }
            match bytes[12] {
                0 => {}
                1 => prefs.keys = Keys::Modern,
                _ => issue = issue.or(Some(ConfigIssue::BadKeys)),
            }
        }
        (prefs, issue)
    }

    /// The bytes to save for `prefs`: always a version 3 file.
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
            u8::from(prefs.motion == Motion::Reduced),
            u8::from(prefs.placement == PlacementMode::Classic),
            u8::from(prefs.captions == Captions::Off),
            u8::from(prefs.keys == Keys::Modern),
        ]
    }

    /// The bytes to save for sound `settings` with every other field at its default. A caller that holds the other fields uses
    /// `encode_prefs`, or the save would reset them.
    pub fn encode(settings: &Settings) -> [u8; Self::LEN] {
        Self::encode_prefs(&Prefs {
            sound: *settings,
            ..Prefs::default()
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
    const MOTIONS: [Motion; 2] = [Motion::Full, Motion::Reduced];
    const PLACEMENTS: [PlacementMode; 2] = [PlacementMode::Enhanced, PlacementMode::Classic];
    const CAPTIONS: [Captions; 2] = [Captions::On, Captions::Off];
    const KEYS: [Keys; 2] = [Keys::Keen, Keys::Modern];

    fn sample() -> Prefs {
        Prefs {
            sound: Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker,
            },
            frame_rate: FrameRate::Hz60,
            motion: Motion::Reduced,
            placement: PlacementMode::Classic,
            captions: Captions::Off,
            keys: Keys::Modern,
        }
    }

    #[test]
    fn round_trip_every_combination() {
        let mut count = 0;
        for music in MUSIC {
            for fx in FX {
                for frame_rate in RATES {
                    for motion in MOTIONS {
                        for placement in PLACEMENTS {
                            for captions in CAPTIONS {
                                for keys in KEYS {
                                    let p = Prefs {
                                        sound: Settings { music, fx },
                                        frame_rate,
                                        motion,
                                        placement,
                                        captions,
                                        keys,
                                    };
                                    let bytes = Config::encode_prefs(&p);
                                    assert_eq!(bytes.len(), 13);
                                    assert_eq!(&bytes[..4], b"FZ2C");
                                    assert_eq!(bytes[4], 3);
                                    assert_eq!(bytes[8], 0);
                                    assert_eq!(Config::parse_prefs(&bytes), (p, None));
                                    assert_eq!(Config::parse(&bytes), (p.sound, None));
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 4 * 5 * 3 * 2 * 2 * 2 * 2);
    }

    #[test]
    fn layout_is_exact() {
        assert_eq!(
            Config::encode_prefs(&sample()),
            [b'F', b'Z', b'2', b'C', 3, 3, 4, 2, 0, 1, 1, 1, 1]
        );
        assert_eq!(
            Config::encode(&Settings::default()),
            [b'F', b'Z', b'2', b'C', 3, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(&Config::encode(&sample().sound)[7..], [0u8; 6]);
    }

    #[test]
    fn zero_means_the_default_in_every_field() {
        let zeros = [b'F', b'Z', b'2', b'C', 3, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(Config::parse_prefs(&zeros), (Prefs::default(), None));
        let d = Prefs::default();
        assert_eq!(
            (d.motion, d.placement, d.captions, d.keys),
            (
                Motion::Full,
                PlacementMode::Enhanced,
                Captions::On,
                Keys::Keen
            )
        );
    }

    #[test]
    fn a_version_1_file_still_reads_with_the_new_fields_at_their_defaults() {
        for music in MUSIC {
            for fx in FX {
                let v3 = Config::encode(&Settings { music, fx });
                let mut v1 = [0u8; 8];
                v1.copy_from_slice(&v3[..8]);
                v1[4] = 1;
                v1[7] = 0;
                assert_eq!(
                    Config::parse_prefs(&v1),
                    (
                        Prefs {
                            sound: Settings { music, fx },
                            ..Prefs::default()
                        },
                        None
                    )
                );
            }
        }
        // The old layout exactly, as the first build wrote it.
        let old = [b'F', b'Z', b'2', b'C', 1, 3, 4, 0];
        assert_eq!(Config::parse(&old).0, sample().sound);
        // Saving it again upgrades it to version 3.
        assert_eq!(Config::encode(&Config::parse(&old).0)[4], 3);
    }

    #[test]
    fn a_version_2_file_still_reads_with_the_new_fields_at_their_defaults() {
        for music in MUSIC {
            for fx in FX {
                for frame_rate in RATES {
                    let rate = match frame_rate {
                        FrameRate::Auto => 0,
                        FrameRate::Hz30 => 1,
                        FrameRate::Hz60 => 2,
                    };
                    let m = MUSIC.iter().position(|x| *x == music).unwrap() as u8;
                    let f = FX.iter().position(|x| *x == fx).unwrap() as u8;
                    let v2 = [b'F', b'Z', b'2', b'C', 2, m, f, rate, 0];
                    assert_eq!(
                        Config::parse_prefs(&v2),
                        (
                            Prefs {
                                sound: Settings { music, fx },
                                frame_rate,
                                ..Prefs::default()
                            },
                            None
                        )
                    );
                }
            }
        }
        // Saving a version 2 file's prefs upgrades it to version 3 and keeps the frame rate.
        let (p, _) = Config::parse_prefs(&[b'F', b'Z', b'2', b'C', 2, 3, 4, 2, 0]);
        let saved = Config::encode_prefs(&p);
        assert_eq!(saved, [b'F', b'Z', b'2', b'C', 3, 3, 4, 2, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_missing_or_empty_file_is_auto_without_a_reason() {
        assert_eq!(Config::parse_prefs(&[]), (Prefs::default(), None));
        assert_eq!(Config::parse(&[]), (Settings::default(), None));
    }

    #[test]
    fn wrong_length_is_auto_with_a_reason() {
        let good = Config::encode_prefs(&sample());
        let bad = Some(ConfigIssue::BadLength);
        // Too short, and a version 3 header on an older body.
        for len in [1, 4, 5, 7, 8, 9, 10, 11, 12] {
            assert_eq!(
                Config::parse_prefs(&good[..len]),
                (Prefs::default(), bad),
                "{len}"
            );
        }
        let mut long = [3u8; 14];
        long[..13].copy_from_slice(&good);
        assert_eq!(Config::parse_prefs(&long), (Prefs::default(), bad));
        // A version 1 or 2 header on a longer body.
        for version in [1u8, 2] {
            let mut wrong = good;
            wrong[4] = version;
            assert_eq!(Config::parse_prefs(&wrong), (Prefs::default(), bad));
        }
        // A version 2 header on a version 1 body, and the reverse.
        let mut v2_short = [0u8; 8];
        v2_short.copy_from_slice(&good[..8]);
        v2_short[4] = 2;
        assert_eq!(Config::parse_prefs(&v2_short), (Prefs::default(), bad));
        let mut v1_long = [0u8; 9];
        v1_long.copy_from_slice(&good[..9]);
        v1_long[4] = 1;
        assert_eq!(Config::parse_prefs(&v1_long), (Prefs::default(), bad));
    }

    #[test]
    fn wrong_magic_and_version_are_auto_with_a_reason() {
        let mut bytes = Config::encode_prefs(&sample());
        bytes[0] = b'X';
        assert_eq!(
            Config::parse_prefs(&bytes),
            (Prefs::default(), Some(ConfigIssue::BadMagic))
        );
        for version in [0u8, 4, 255] {
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
        let base = sample();
        // (byte, bad value, the issue, the field it resets)
        type Reset = fn(&mut Prefs);
        let cases: [(usize, u8, ConfigIssue, Reset); 7] = [
            (5, 4, ConfigIssue::BadMusic, |p| {
                p.sound.music = MusicChoice::Auto
            }),
            (6, 5, ConfigIssue::BadEffects, |p| {
                p.sound.fx = FxChoice::Auto
            }),
            (7, 3, ConfigIssue::BadFrameRate, |p| {
                p.frame_rate = FrameRate::Auto
            }),
            (9, 2, ConfigIssue::BadMotion, |p| p.motion = Motion::Full),
            (10, 2, ConfigIssue::BadPlacement, |p| {
                p.placement = PlacementMode::Enhanced
            }),
            (11, 2, ConfigIssue::BadCaptions, |p| {
                p.captions = Captions::On
            }),
            (12, 255, ConfigIssue::BadKeys, |p| p.keys = Keys::Keen),
        ];
        for (at, value, issue, reset) in cases {
            let mut bytes = Config::encode_prefs(&base);
            bytes[at] = value;
            let mut want = base;
            reset(&mut want);
            assert_eq!(
                Config::parse_prefs(&bytes),
                (want, Some(issue)),
                "byte {at}"
            );
        }

        // Several bad bytes: the first issue is reported and every bad field is its default.
        let mut bytes = Config::encode_prefs(&base);
        bytes[5] = 200;
        bytes[6] = 255;
        bytes[11] = 9;
        let (p, issue) = Config::parse_prefs(&bytes);
        assert_eq!(issue, Some(ConfigIssue::BadMusic));
        assert_eq!(
            p,
            Prefs {
                sound: Settings::default(),
                captions: Captions::On,
                ..base
            }
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
            ConfigIssue::BadMotion,
            ConfigIssue::BadPlacement,
            ConfigIssue::BadCaptions,
            ConfigIssue::BadKeys,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
        }
        assert_eq!(ConfigIssue::BadMotion.code(), "cfg-motion");
        assert_eq!(ConfigIssue::BadPlacement.code(), "cfg-placement");
        assert_eq!(ConfigIssue::BadCaptions.code(), "cfg-captions");
        assert_eq!(ConfigIssue::BadKeys.code(), "cfg-keys");
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
            ..sample()
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
        // What a later save writes is the file's own prefs: effects stay Auto, the other choices stay.
        let saved = Config::encode_prefs(&read);
        assert_eq!(
            (
                saved[5], saved[6], saved[7], saved[9], saved[10], saved[11], saved[12]
            ),
            (3, 0, 1, 1, 1, 1, 1)
        );
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
