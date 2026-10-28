// The text form of a probe result, and the hash of a screen: what `FIZZ2.EXE /PROBE` prints and what the host checks it against.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `/PROBE` prints one `PROBE <key> <value>` line for each `Probe` field, then `CELLS <hash>`, the hash of the screen the same values
// make. `write_probe` is the one writer of those lines and `parse_probe` reads them back, so the DOS program and the host tool that
// recomputes the hash (`examples/cells-hash.rs`) agree on the spelling by construction. Lines that do not start with `PROBE ` are
// ignored when parsing, so a whole program transcript can be passed in.

use crate::probe::{Blaster, CpuClass, FmKind, Probe, SoundBlaster, VideoClass, parse_blaster};
use crate::resolve::{FxPlan, MusicPlan, Resolved};
use crate::screen::CELLS;
use crate::settings::ConfigIssue;
use core::fmt::{self, Write};

/// The FNV-1a 64-bit hash of a screen's bytes in video-memory order: for each cell the character, then the attribute.
pub fn cells_hash(cells: &[u16; CELLS]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for cell in cells {
        for byte in cell.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// Writes the `PROBE <key> <value>` lines, one for each field, each ending in a newline.
pub fn write_probe(p: &Probe, out: &mut impl Write) -> fmt::Result {
    let video = match p.video {
        VideoClass::Vga => "vga",
        VideoClass::MonoVga => "monovga",
        VideoClass::Ega => "ega",
        VideoClass::Cga => "cga",
        VideoClass::MdaHercules => "mdahercules",
    };
    writeln!(out, "PROBE video {video}")?;
    writeln!(out, "PROBE mouse {}", yes_no(p.mouse))?;
    writeln!(out, "PROBE joystick {}", yes_no(p.joystick))?;
    let fm = match p.fm {
        FmKind::None => "none",
        FmKind::Opl2 => "opl2",
        FmKind::DualOpl2 => "dualopl2",
        FmKind::Opl3 => "opl3",
    };
    writeln!(out, "PROBE fm {fm}")?;
    match p.sb {
        Some(sb) => writeln!(
            out,
            "PROBE dsp {}.{:02} {:03X}",
            sb.dsp_major, sb.dsp_minor, sb.base
        )?,
        None => writeln!(out, "PROBE dsp none")?,
    }
    write!(out, "PROBE blaster")?;
    let b = &p.blaster;
    let mut any = false;
    if let Some(v) = b.base {
        write!(out, " A{v:X}")?;
        any = true;
    }
    if let Some(v) = b.irq {
        write!(out, " I{v}")?;
        any = true;
    }
    if let Some(v) = b.dma {
        write!(out, " D{v}")?;
        any = true;
    }
    if let Some(v) = b.hdma {
        write!(out, " H{v}")?;
        any = true;
    }
    if let Some(v) = b.mpu {
        write!(out, " P{v:X}")?;
        any = true;
    }
    if let Some(v) = b.card {
        write!(out, " T{v}")?;
        any = true;
    }
    if !any {
        write!(out, " none")?;
    }
    writeln!(out)?;
    writeln!(out, "PROBE x87 {}", yes_no(p.x87))?;
    let cpu = match p.cpu {
        CpuClass::I386 => "386",
        CpuClass::I486 => "486",
        CpuClass::I586Plus => "586",
    };
    writeln!(out, "PROBE cpu {cpu}")?;
    writeln!(out, "PROBE free_kb {}", p.free_kb)
}

fn parse_yes_no(v: &str) -> Result<bool, &'static str> {
    match v {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err("expected yes or no"),
    }
}

/// Reads the `PROBE` lines of `text` back into a `Probe`. Every key must appear; an unknown key, a bad value or a missing key is an
/// error naming the problem. A key that appears twice takes its last value.
pub fn parse_probe(text: &str) -> Result<Probe, &'static str> {
    let mut video = None;
    let mut mouse = None;
    let mut joystick = None;
    let mut fm = None;
    let mut sb: Option<Option<SoundBlaster>> = None;
    let mut blaster = None;
    let mut x87 = None;
    let mut cpu = None;
    let mut free_kb = None;
    for line in text.lines() {
        let Some(rest) = line.trim_end().strip_prefix("PROBE ") else {
            continue;
        };
        let (key, value) = rest.split_once(' ').ok_or("a PROBE line has no value")?;
        match key {
            "video" => {
                video = Some(match value {
                    "vga" => VideoClass::Vga,
                    "monovga" => VideoClass::MonoVga,
                    "ega" => VideoClass::Ega,
                    "cga" => VideoClass::Cga,
                    "mdahercules" => VideoClass::MdaHercules,
                    _ => return Err("unknown video class"),
                })
            }
            "mouse" => mouse = Some(parse_yes_no(value)?),
            "joystick" => joystick = Some(parse_yes_no(value)?),
            "fm" => {
                fm = Some(match value {
                    "none" => FmKind::None,
                    "opl2" => FmKind::Opl2,
                    "dualopl2" => FmKind::DualOpl2,
                    "opl3" => FmKind::Opl3,
                    _ => return Err("unknown fm kind"),
                })
            }
            "dsp" => {
                sb = Some(if value == "none" {
                    None
                } else {
                    let (version, base) = value
                        .split_once(' ')
                        .ok_or("dsp needs a version and a base")?;
                    let (major, minor) =
                        version.split_once('.').ok_or("dsp version needs a dot")?;
                    Some(SoundBlaster {
                        dsp_major: major.parse().map_err(|_| "bad dsp major version")?,
                        dsp_minor: minor.parse().map_err(|_| "bad dsp minor version")?,
                        base: u16::from_str_radix(base, 16).map_err(|_| "bad dsp base")?,
                    })
                })
            }
            "blaster" => {
                blaster = Some(if value == "none" {
                    Blaster::default()
                } else {
                    parse_blaster(value)
                })
            }
            "x87" => x87 = Some(parse_yes_no(value)?),
            "cpu" => {
                cpu = Some(match value {
                    "386" => CpuClass::I386,
                    "486" => CpuClass::I486,
                    "586" => CpuClass::I586Plus,
                    _ => return Err("unknown cpu class"),
                })
            }
            "free_kb" => free_kb = Some(value.parse().map_err(|_| "bad free_kb")?),
            _ => return Err("unknown PROBE key"),
        }
    }
    Ok(Probe {
        video: video.ok_or("no PROBE video line")?,
        mouse: mouse.ok_or("no PROBE mouse line")?,
        joystick: joystick.ok_or("no PROBE joystick line")?,
        fm: fm.ok_or("no PROBE fm line")?,
        sb: sb.ok_or("no PROBE dsp line")?,
        blaster: blaster.ok_or("no PROBE blaster line")?,
        x87: x87.ok_or("no PROBE x87 line")?,
        cpu: cpu.ok_or("no PROBE cpu line")?,
        free_kb: free_kb.ok_or("no PROBE free_kb line")?,
    })
}

/// Writes the `SOUND MUSIC <x> FX <y>` line (no newline): the resolved plan, then, each after a space, the reasons it is not what was
/// asked for. `file_issue` (the settings file was unusable) comes first, then the music reason, then the effects reason.
pub fn write_sound(
    resolved: &Resolved,
    file_issue: Option<ConfigIssue>,
    out: &mut impl Write,
) -> fmt::Result {
    let music = match resolved.music {
        MusicPlan::Off => "off",
        MusicPlan::Opl2 => "opl2",
        MusicPlan::Opl3 => "opl3",
    };
    let fx = match resolved.fx {
        FxPlan::Off => "off",
        FxPlan::Ch5 => "ch5",
        FxPlan::Ch14 => "ch14",
        FxPlan::Speaker => "speaker",
    };
    write!(out, "SOUND MUSIC {music} FX {fx}")?;
    if let Some(issue) = file_issue {
        write!(out, " {}", issue.code())?;
    }
    if let Some(r) = resolved.music_reason {
        write!(out, " {}", r.code())?;
    }
    if let Some(r) = resolved.fx_reason {
        write!(out, " {}", r.code())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::resolve;
    use crate::settings::{FxChoice, MusicChoice, Settings};
    use std::string::String;

    fn sound(settings: Settings, probe: &Probe, issue: Option<ConfigIssue>) -> String {
        let mut s = String::new();
        write_sound(&resolve(&settings, probe), issue, &mut s).unwrap();
        s
    }

    #[test]
    fn the_sound_line() {
        let mut sb16 = Probe::BARE;
        sb16.fm = FmKind::Opl3;
        let mut opl2 = Probe::BARE;
        opl2.fm = FmKind::Opl2;
        let auto = Settings::default();
        assert_eq!(sound(auto, &sb16, None), "SOUND MUSIC opl3 FX ch14");
        assert_eq!(sound(auto, &opl2, None), "SOUND MUSIC opl2 FX ch5");
        assert_eq!(
            sound(auto, &Probe::BARE, None),
            "SOUND MUSIC off FX speaker"
        );
        let sb = Settings {
            music: MusicChoice::Sb,
            fx: FxChoice::Sb,
        };
        assert_eq!(
            sound(sb, &opl2, None),
            "SOUND MUSIC opl2 FX ch5 music-sb-to-adlib fx-sb-to-adlib"
        );
        assert_eq!(
            sound(sb, &Probe::BARE, Some(ConfigIssue::BadMagic)),
            "SOUND MUSIC off FX speaker cfg-magic music-sb-to-off fx-sb-to-speaker"
        );
        let nosound = Settings {
            music: MusicChoice::Off,
            fx: FxChoice::Off,
        };
        assert_eq!(sound(nosound, &sb16, None), "SOUND MUSIC off FX off");
    }

    fn text(p: &Probe) -> String {
        let mut s = String::new();
        write_probe(p, &mut s).unwrap();
        s
    }

    #[test]
    fn bare_machine_lines() {
        assert_eq!(
            text(&Probe::BARE),
            "PROBE video vga\nPROBE mouse no\nPROBE joystick no\nPROBE fm none\nPROBE dsp none\nPROBE blaster none\nPROBE x87 yes\nPROBE cpu 486\nPROBE free_kb 8192\n"
        );
    }

    #[test]
    fn round_trip_of_a_full_machine() {
        let p = Probe {
            video: VideoClass::MonoVga,
            mouse: true,
            joystick: true,
            fm: FmKind::DualOpl2,
            sb: Some(SoundBlaster {
                dsp_major: 3,
                dsp_minor: 0,
                base: 0x240,
            }),
            blaster: parse_blaster("A240 I5 D3 H6 P330 T4"),
            x87: false,
            cpu: CpuClass::I586Plus,
            free_kb: 14_784,
        };
        let t = text(&p);
        assert!(t.contains("PROBE dsp 3.00 240\n"));
        assert!(t.contains("PROBE blaster A240 I5 D3 H6 P330 T4\n"));
        assert_eq!(parse_probe(&t), Ok(p));
    }

    #[test]
    fn other_lines_are_ignored_and_errors_are_named() {
        let mut t = String::from("HEAP 1\n");
        t.push_str(&text(&Probe::BARE));
        t.push_str("CELLS 00\nEXIT yes\n");
        assert_eq!(parse_probe(&t), Ok(Probe::BARE));
        assert_eq!(parse_probe("PROBE video vga\n"), Err("no PROBE mouse line"));
        assert_eq!(
            parse_probe("PROBE video cga2\n"),
            Err("unknown video class")
        );
        assert_eq!(parse_probe("PROBE colour red\n"), Err("unknown PROBE key"));
    }

    #[test]
    fn hash_is_fnv1a_of_the_bytes() {
        let empty = [0u16; CELLS];
        assert_eq!(cells_hash(&empty), cells_hash(&[0u16; CELLS]));
        let mut one = [0u16; CELLS];
        one[0] = 0x0741;
        assert_ne!(cells_hash(&one), cells_hash(&empty));
        // The bytes go in character first, then attribute: the same order as video memory.
        let mut swapped = [0u16; CELLS];
        swapped[0] = 0x4107;
        assert_ne!(cells_hash(&one), cells_hash(&swapped));
        // FNV-1a over 4,000 zero bytes, computed independently of the function above.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for _ in 0..CELLS * 2 {
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        assert_eq!(cells_hash(&empty), h);
    }
}
