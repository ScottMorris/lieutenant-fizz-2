// The sound resolver: the player's choices and the probed machine become one deterministic plan, and each device row a state.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// An impossible choice never fails: it falls back along a fixed chain and records why as a code. The chip is global: it runs in
// OPL3 mode exactly when an OPL3 is present and music or effects resolved to the Sound Blaster, and then the audio service
// applies `route` to every OPL2-format register write.

use crate::probe::{FmKind, Probe};
use crate::route::OplMode;
use crate::settings::{FxChoice, MusicChoice, Settings};

/// How music will play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicPlan {
    Off,
    /// OPL2 voices, mono, on channels 0-4 and 6-8.
    Opl2,
    /// OPL3 voices, stereo, on channels 0-4, 9-13 and 15-17.
    Opl3,
}

/// How sound effects will play. Channels 5 and 14 stay reserved for them whatever music does. Music never uses the speaker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FxPlan {
    Off,
    /// OPL2 channel 5, centred.
    Ch5,
    /// OPL3 channel 14 in bank 1.
    Ch14,
    /// The PC speaker, through PIT channel 2.
    Speaker,
}

/// Why a choice fell back. A small fixed set, for the `SOUND` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// Music asked for the Sound Blaster's OPL3 and got the OPL2.
    MusicSbToAdlib,
    /// Music asked for the Sound Blaster and there is no FM chip, so music is off.
    MusicSbToOff,
    /// Music asked for AdLib and there is no FM chip, so music is off.
    MusicAdlibToOff,
    /// Effects asked for the Sound Blaster's OPL3 and got the OPL2.
    FxSbToAdlib,
    /// Effects asked for the Sound Blaster and there is no FM chip, so they use the speaker.
    FxSbToSpeaker,
    /// Effects asked for AdLib and there is no FM chip, so they use the speaker.
    FxAdlibToSpeaker,
}

impl Reason {
    /// A short fixed word for the `SOUND` line.
    pub fn code(self) -> &'static str {
        match self {
            Reason::MusicSbToAdlib => "music-sb-to-adlib",
            Reason::MusicSbToOff => "music-sb-to-off",
            Reason::MusicAdlibToOff => "music-adlib-to-off",
            Reason::FxSbToAdlib => "fx-sb-to-adlib",
            Reason::FxSbToSpeaker => "fx-sb-to-speaker",
            Reason::FxAdlibToSpeaker => "fx-adlib-to-speaker",
        }
    }
}

/// What a device row on the check screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundState {
    /// Not on this machine.
    Missing,
    /// On this machine and not used by the plan.
    Available,
    /// On this machine and used by the plan for music, effects or both.
    InUse,
}

/// The state of each sound row on the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundStates {
    /// The PC speaker, assumed present.
    pub speaker: SoundState,
    /// The AdLib (OPL2) row: any FM chip answering at 0x388.
    pub adlib: SoundState,
    /// The Sound Blaster (OPL3) row: a DSP answered, or an OPL3 is present.
    pub sb: SoundState,
}

/// The sound plan for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// How the FM chip is used: `Opl3` exactly when an OPL3 is present and music or effects resolved to the Sound Blaster.
    pub opl: OplMode,
    pub music: MusicPlan,
    pub fx: FxPlan,
    pub music_reason: Option<Reason>,
    pub fx_reason: Option<Reason>,
    pub states: SoundStates,
}

/// Resolves the player's `settings` against what the machine has.
///
/// Effects: Sound Blaster falls back to AdLib then the speaker; AdLib to the speaker; Auto is channel 14 on an OPL3, else
/// channel 5 on any FM chip, else the speaker. Music: Sound Blaster falls back to AdLib then off; AdLib to off; Auto is OPL3
/// on an OPL3, else OPL2 on any FM chip, else off. A Sound Blaster Pro 1 (two OPL2 chips) counts as one OPL2.
pub fn resolve(settings: &Settings, probe: &Probe) -> Resolved {
    let has_fm = probe.fm != FmKind::None;
    let has_opl3 = probe.fm == FmKind::Opl3;

    let mut music_reason = None;
    let music = match settings.music {
        MusicChoice::Off => MusicPlan::Off,
        MusicChoice::Auto if has_opl3 => MusicPlan::Opl3,
        MusicChoice::Auto if has_fm => MusicPlan::Opl2,
        MusicChoice::Auto => MusicPlan::Off,
        MusicChoice::AdLib if has_fm => MusicPlan::Opl2,
        MusicChoice::AdLib => {
            music_reason = Some(Reason::MusicAdlibToOff);
            MusicPlan::Off
        }
        MusicChoice::Sb if has_opl3 => MusicPlan::Opl3,
        MusicChoice::Sb if has_fm => {
            music_reason = Some(Reason::MusicSbToAdlib);
            MusicPlan::Opl2
        }
        MusicChoice::Sb => {
            music_reason = Some(Reason::MusicSbToOff);
            MusicPlan::Off
        }
    };

    let mut fx_reason = None;
    let fx = match settings.fx {
        FxChoice::Off => FxPlan::Off,
        FxChoice::Speaker => FxPlan::Speaker,
        FxChoice::Auto if has_opl3 => FxPlan::Ch14,
        FxChoice::Auto if has_fm => FxPlan::Ch5,
        FxChoice::Auto => FxPlan::Speaker,
        FxChoice::AdLib if has_fm => FxPlan::Ch5,
        FxChoice::AdLib => {
            fx_reason = Some(Reason::FxAdlibToSpeaker);
            FxPlan::Speaker
        }
        FxChoice::Sb if has_opl3 => FxPlan::Ch14,
        FxChoice::Sb if has_fm => {
            fx_reason = Some(Reason::FxSbToAdlib);
            FxPlan::Ch5
        }
        FxChoice::Sb => {
            fx_reason = Some(Reason::FxSbToSpeaker);
            FxPlan::Speaker
        }
    };

    let uses_opl3 = music == MusicPlan::Opl3 || fx == FxPlan::Ch14;
    let uses_opl2 = music == MusicPlan::Opl2 || fx == FxPlan::Ch5;
    let opl = if uses_opl3 {
        OplMode::Opl3
    } else if uses_opl2 {
        OplMode::Opl2
    } else {
        OplMode::None
    };

    let state = |present: bool, in_use: bool| match (present, in_use) {
        (false, _) => SoundState::Missing,
        (true, false) => SoundState::Available,
        (true, true) => SoundState::InUse,
    };
    let states = SoundStates {
        speaker: state(true, fx == FxPlan::Speaker),
        adlib: state(has_fm, uses_opl2),
        sb: state(probe.sb.is_some() || has_opl3, uses_opl3),
    };

    Resolved {
        opl,
        music,
        fx,
        music_reason,
        fx_reason,
        states,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::SoundBlaster;
    use std::vec::Vec;

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
    const FM: [FmKind; 4] = [FmKind::None, FmKind::Opl2, FmKind::DualOpl2, FmKind::Opl3];

    fn sb(major: u8, minor: u8) -> Option<SoundBlaster> {
        Some(SoundBlaster {
            dsp_major: major,
            dsp_minor: minor,
            base: 0x220,
        })
    }

    /// The machines of the table test: speaker only, AdLib card, SB 2.0, SB Pro 1, SB Pro 2 and SB16, and an OPL3 with no DSP.
    pub(crate) fn machines() -> Vec<Probe> {
        let m = |fm, sb| Probe {
            fm,
            sb,
            ..Probe::BARE
        };
        std::vec![
            m(FmKind::None, None),
            m(FmKind::Opl2, None),
            m(FmKind::Opl2, sb(2, 1)),
            m(FmKind::DualOpl2, sb(3, 0)),
            m(FmKind::Opl3, sb(3, 2)),
            m(FmKind::Opl3, sb(4, 5)),
            m(FmKind::Opl3, None),
            m(FmKind::None, sb(2, 1)),
        ]
    }

    fn plan(music: MusicChoice, fx: FxChoice, probe: &Probe) -> Resolved {
        resolve(&Settings { music, fx }, probe)
    }

    #[test]
    fn table_invariants() {
        let mut count = 0;
        for probe in machines() {
            for music in MUSIC {
                for fx in FX {
                    count += 1;
                    let r = plan(music, fx, &probe);
                    let has_fm = probe.fm != FmKind::None;
                    let has_opl3 = probe.fm == FmKind::Opl3;
                    let ctx = std::format!("{probe:?} {music:?} {fx:?} -> {r:?}");
                    // Never names hardware that is absent.
                    assert!(
                        has_fm
                            || (r.music == MusicPlan::Off
                                && r.opl == OplMode::None
                                && r.fx != FxPlan::Ch5),
                        "{ctx}"
                    );
                    assert!(
                        has_opl3
                            || (r.music != MusicPlan::Opl3
                                && r.fx != FxPlan::Ch14
                                && r.opl != OplMode::Opl3),
                        "{ctx}"
                    );
                    // The chip mode is global: Opl3 exactly when an OPL3 serves music or effects as the Sound Blaster.
                    let uses_opl3 = r.music == MusicPlan::Opl3 || r.fx == FxPlan::Ch14;
                    assert_eq!(r.opl == OplMode::Opl3, uses_opl3, "{ctx}");
                    assert_eq!(
                        r.opl == OplMode::None,
                        r.music == MusicPlan::Off && !matches!(r.fx, FxPlan::Ch5 | FxPlan::Ch14),
                        "{ctx}"
                    );
                    // An explicit Off or Speaker is honoured; Auto never gives silence while a device exists.
                    if music == MusicChoice::Off {
                        assert_eq!(r.music, MusicPlan::Off, "{ctx}");
                    }
                    if fx == FxChoice::Off {
                        assert_eq!(r.fx, FxPlan::Off, "{ctx}");
                    }
                    if fx == FxChoice::Speaker {
                        assert_eq!(r.fx, FxPlan::Speaker, "{ctx}");
                    }
                    if fx == FxChoice::Auto {
                        assert_ne!(r.fx, FxPlan::Off, "{ctx}");
                    }
                    if music == MusicChoice::Auto && has_fm {
                        assert_ne!(r.music, MusicPlan::Off, "{ctx}");
                    }
                    // A reason is recorded exactly when the request could not be met.
                    let music_met = match music {
                        MusicChoice::Auto | MusicChoice::Off => true,
                        MusicChoice::AdLib => has_fm,
                        MusicChoice::Sb => has_opl3,
                    };
                    let fx_met = match fx {
                        FxChoice::Auto | FxChoice::Off | FxChoice::Speaker => true,
                        FxChoice::AdLib => has_fm,
                        FxChoice::Sb => has_opl3,
                    };
                    assert_eq!(r.music_reason.is_none(), music_met, "{ctx}");
                    assert_eq!(r.fx_reason.is_none(), fx_met, "{ctx}");
                    // States agree with the plan and the machine.
                    assert_eq!(
                        r.states.speaker,
                        if r.fx == FxPlan::Speaker {
                            SoundState::InUse
                        } else {
                            SoundState::Available
                        },
                        "{ctx}"
                    );
                    assert_eq!(r.states.adlib == SoundState::Missing, !has_fm, "{ctx}");
                    assert_eq!(
                        r.states.adlib == SoundState::InUse,
                        r.music == MusicPlan::Opl2 || r.fx == FxPlan::Ch5,
                        "{ctx}"
                    );
                    assert_eq!(r.states.sb == SoundState::InUse, uses_opl3, "{ctx}");
                    assert_eq!(
                        r.states.sb == SoundState::Missing,
                        probe.sb.is_none() && !has_opl3,
                        "{ctx}"
                    );
                    // Whatever is in use is present.
                    if uses_opl3 {
                        assert_ne!(r.states.sb, SoundState::Missing, "{ctx}");
                    }
                }
            }
        }
        assert_eq!(count, 8 * 4 * 5);
    }

    #[test]
    fn every_fm_kind_is_covered_by_the_machines() {
        for fm in FM {
            assert!(machines().iter().any(|p| p.fm == fm));
        }
    }

    #[test]
    fn auto_on_each_machine() {
        let auto = Settings::default();
        let r = resolve(&auto, &Probe::BARE);
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::None, MusicPlan::Off, FxPlan::Speaker)
        );
        let r = resolve(
            &auto,
            &Probe {
                fm: FmKind::Opl2,
                ..Probe::BARE
            },
        );
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl2, MusicPlan::Opl2, FxPlan::Ch5)
        );
        let r = resolve(
            &auto,
            &Probe {
                fm: FmKind::DualOpl2,
                sb: sb(3, 0),
                ..Probe::BARE
            },
        );
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl2, MusicPlan::Opl2, FxPlan::Ch5)
        );
        let r = resolve(
            &auto,
            &Probe {
                fm: FmKind::Opl3,
                sb: sb(4, 5),
                ..Probe::BARE
            },
        );
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl3, MusicPlan::Opl3, FxPlan::Ch14)
        );
        assert_eq!((r.music_reason, r.fx_reason), (None, None));
    }

    #[test]
    fn sb_choice_without_an_opl3_falls_back_with_a_reason() {
        let pro1 = Probe {
            fm: FmKind::DualOpl2,
            sb: sb(3, 0),
            ..Probe::BARE
        };
        let r = plan(MusicChoice::Sb, FxChoice::Sb, &pro1);
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl2, MusicPlan::Opl2, FxPlan::Ch5)
        );
        assert_eq!(
            (r.music_reason, r.fx_reason),
            (Some(Reason::MusicSbToAdlib), Some(Reason::FxSbToAdlib))
        );
        let none = plan(MusicChoice::Sb, FxChoice::Sb, &Probe::BARE);
        assert_eq!(
            (none.opl, none.music, none.fx),
            (OplMode::None, MusicPlan::Off, FxPlan::Speaker)
        );
        assert_eq!(
            (none.music_reason, none.fx_reason),
            (Some(Reason::MusicSbToOff), Some(Reason::FxSbToSpeaker))
        );
        let none = plan(MusicChoice::AdLib, FxChoice::AdLib, &Probe::BARE);
        assert_eq!(
            (none.music_reason, none.fx_reason),
            (
                Some(Reason::MusicAdlibToOff),
                Some(Reason::FxAdlibToSpeaker)
            )
        );
    }

    #[test]
    fn reason_codes_are_distinct() {
        let all = [
            Reason::MusicSbToAdlib,
            Reason::MusicSbToOff,
            Reason::MusicAdlibToOff,
            Reason::FxSbToAdlib,
            Reason::FxSbToSpeaker,
            Reason::FxAdlibToSpeaker,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a.code(), b.code());
            }
        }
    }

    #[test]
    fn mixed_choices_on_an_opl3_share_one_chip_in_opl3_mode() {
        let sb16 = Probe {
            fm: FmKind::Opl3,
            sb: sb(4, 5),
            ..Probe::BARE
        };
        // Music on the AdLib role and effects on the Sound Blaster role: the chip is in OPL3 mode, so the OR rule centres music.
        let r = plan(MusicChoice::AdLib, FxChoice::Sb, &sb16);
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl3, MusicPlan::Opl2, FxPlan::Ch14)
        );
        assert_eq!(
            (r.states.adlib, r.states.sb),
            (SoundState::InUse, SoundState::InUse)
        );
        // Music on the Sound Blaster and effects on the AdLib channel.
        let r = plan(MusicChoice::Sb, FxChoice::AdLib, &sb16);
        assert_eq!(
            (r.opl, r.music, r.fx),
            (OplMode::Opl3, MusicPlan::Opl3, FxPlan::Ch5)
        );
        // Only OPL2 roles: the chip stays in its power-up mode.
        let r = plan(MusicChoice::AdLib, FxChoice::AdLib, &sb16);
        assert_eq!(r.opl, OplMode::Opl2);
    }

    #[test]
    fn sound_states_for_the_named_cases() {
        let sb16 = Probe {
            fm: FmKind::Opl3,
            sb: sb(4, 5),
            ..Probe::BARE
        };
        // AdLib chosen on an SB16: the Sound Blaster row is available, not in use.
        let s = plan(MusicChoice::AdLib, FxChoice::AdLib, &sb16).states;
        assert_eq!(
            (s.speaker, s.adlib, s.sb),
            (
                SoundState::Available,
                SoundState::InUse,
                SoundState::Available
            )
        );
        // Auto on an SB16.
        let s = plan(MusicChoice::Auto, FxChoice::Auto, &sb16).states;
        assert_eq!(
            (s.speaker, s.adlib, s.sb),
            (
                SoundState::Available,
                SoundState::Available,
                SoundState::InUse
            )
        );
        // Effects on the speaker, music on the Sound Blaster.
        let s = plan(MusicChoice::Sb, FxChoice::Speaker, &sb16).states;
        assert_eq!(
            (s.speaker, s.adlib, s.sb),
            (SoundState::InUse, SoundState::Available, SoundState::InUse)
        );
        // Speaker only.
        let s = resolve(&Settings::default(), &Probe::BARE).states;
        assert_eq!(
            (s.speaker, s.adlib, s.sb),
            (SoundState::InUse, SoundState::Missing, SoundState::Missing)
        );
        // Everything off: all present rows are available.
        let s = plan(MusicChoice::Off, FxChoice::Off, &sb16).states;
        assert_eq!(
            (s.speaker, s.adlib, s.sb),
            (
                SoundState::Available,
                SoundState::Available,
                SoundState::Available
            )
        );
        // A Pro 1 and an SB 2.0 use their OPL2 as AdLib, and the Sound Blaster row stays available.
        for probe in [
            Probe {
                fm: FmKind::DualOpl2,
                sb: sb(3, 0),
                ..Probe::BARE
            },
            Probe {
                fm: FmKind::Opl2,
                sb: sb(2, 1),
                ..Probe::BARE
            },
        ] {
            let s = resolve(&Settings::default(), &probe).states;
            assert_eq!(
                (s.speaker, s.adlib, s.sb),
                (
                    SoundState::Available,
                    SoundState::InUse,
                    SoundState::Available
                )
            );
        }
        // An AdLib card alone.
        let s = resolve(
            &Settings::default(),
            &Probe {
                fm: FmKind::Opl2,
                ..Probe::BARE
            },
        )
        .states;
        assert_eq!((s.adlib, s.sb), (SoundState::InUse, SoundState::Missing));
    }
}
