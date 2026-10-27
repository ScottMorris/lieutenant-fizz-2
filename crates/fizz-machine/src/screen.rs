// The system-check screen as 80x25 text cells: what the machine has and how the sound plan uses it, read-only.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Ported from Fizz 1's DOS port, `m2/dos-rt/src/preview.rs` in `ScottMorris/lieutenant-fizz-dos` at commit 221dd6c: the `Cells`
// builder, `plate` and `rows`, the code page 437 double-line boxes and light-shade shadows, and the four-panel layout with
// the same positions. What changed: the cell buffer is a fixed `[u16; 2000]` and an attribute is written whole instead of as
// separate colours; the title is `LIEUTENANT FIZZ 2 - SYSTEM CHECK`; the speed test and the "14 lighting shades" row are gone;
// the input is a `Probe` and a `Resolved` instead of a list of integers; the Sound panel shows three states (missing, available,
// in use) from the probe and the resolved plan, with the Sound Blaster's device and chip on its row (the DSP version and base text is kept behind `SHOW_SB_DETAIL`); the memory floor is
// `MEMORY_FLOOR_KB` and a low value warns without blocking; and a monochrome adapter gets its own attribute set.
//
// A cell is the character (code page 437) in the low byte and the attribute in the high byte: foreground in bits 0-3,
// background in bits 4-6, bit 7 clear so nothing blinks whatever the adapter makes of it.
//
// Attributes by state. Colour adapters: missing is dark grey on black (0x08), available light grey on black (0x07), in use
// bright white on blue (0x1F) across the whole row. A monochrome adapter (MDA or Hercules, mode 7) turns every attribute into
// normal (0x07), bright (0x0F) or inverse (0x70) and shows a dark grey foreground on black as black, so there the three states
// are normal, bright and inverse: missing 0x07, available 0x0F, in use 0x70. The order dim, normal, highlighted is the same on
// both. The marker in front of each row (`x`, a tick, a pointer) repeats the state in the character, so no state depends on an
// attribute alone.

use crate::probe::{CpuClass, FmKind, Probe, SoundBlaster, VideoClass};
use crate::resolve::{FxPlan, MusicPlan, Resolved, SoundState};
use core::fmt::Write;

/// The text screen's size in character cells.
pub const COLS: usize = 80;
pub const ROWS: usize = 25;
pub const CELLS: usize = COLS * ROWS;

/// Free memory, in kilobytes, below which the Machine panel warns. Fizz 1's 10 MB does not apply: Fizz 2 loads a level's pack
/// and little else. This is a placeholder until the DOS memory ladder (DOSBox at 4, 2 and 1 MB) measures what the slice needs;
/// a value below it only warns and never stops the game.
pub const MEMORY_FLOOR_KB: u32 = 4096;

// Code page 437 characters.
const CP_TICK: u8 = 0xFB;
const CP_WARN: u8 = 0x1E;
const CP_POINTER: u8 = 0x10;
const CP_LIGHT_SHADE: u8 = 0xB0;
const CP_DARK_SHADE: u8 = 0xB2;
const CP_H: u8 = 0xCD;
const CP_V: u8 = 0xBA;
const CP_TOP_LEFT: u8 = 0xC9;
const CP_TOP_RIGHT: u8 = 0xBB;
const CP_BOTTOM_LEFT: u8 = 0xC8;
const CP_BOTTOM_RIGHT: u8 = 0xBC;

/// Every attribute the screen uses (background in the high nibble, foreground in the low).
struct Theme {
    /// The dark-shade field behind everything, and the light-shade shadows.
    field: u8,
    /// The title plate's edge and fill.
    title_plate: u8,
    title_text: u8,
    /// A panel's edge and black fill.
    panel: u8,
    legend: u8,
    found: u8,
    dim: u8,
    in_use: u8,
    warn: u8,
    needed: u8,
    /// The text of the footer lines.
    foot: u8,
    call: u8,
}

const COLOUR: Theme = Theme {
    field: 0x06,
    title_plate: 0x6E,
    title_text: 0x6F,
    panel: 0x0E,
    legend: 0x6F,
    found: 0x07,
    dim: 0x08,
    in_use: 0x1F,
    warn: 0x6E,
    needed: 0x4F,
    foot: 0x07,
    call: 0x0E,
};

const MONO: Theme = Theme {
    field: 0x07,
    title_plate: 0x0F,
    title_text: 0x0F,
    panel: 0x0F,
    legend: 0x70,
    found: 0x0F,
    dim: 0x07,
    in_use: 0x70,
    warn: 0x0F,
    needed: 0x70,
    foot: 0x07,
    call: 0x0F,
};

/// How a row is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Found,
    /// Not on this machine.
    Dim,
    /// In use by the sound plan.
    InUse,
    /// There, but short of what the game wants: the game still runs.
    Warn,
    /// Missing, and the game cannot start without it.
    Needed,
}

impl From<SoundState> for Row {
    fn from(s: SoundState) -> Row {
        match s {
            SoundState::Missing => Row::Dim,
            SoundState::Available => Row::Found,
            SoundState::InUse => Row::InUse,
        }
    }
}

/// A short line built without allocating.
struct Line {
    bytes: [u8; 64],
    len: usize,
}

impl Line {
    fn new() -> Line {
        Line {
            bytes: [0; 64],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let room = self.bytes.len() - self.len;
        let n = s.len().min(room);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Whether the game can start: a VGA and an x87. Low memory only warns.
pub fn ready(probe: &Probe) -> bool {
    probe.video.has_vga() && probe.x87
}

/// Whether the Sound Blaster row carries the detailed text from `sound_blaster_detail` (DSP version, base and FM words). It is off
/// because the check screen shows only the device and its chip; a later detailed or verbose view can turn it on.
pub const SHOW_SB_DETAIL: bool = false;

/// The Sound Blaster row's short words: the device and its FM chip, such as `Sound Blaster (OPL3)`, `Sound Blaster (OPL2)` or
/// `Sound Blaster (2 x OPL2)` for a Pro 1. No DSP version and no base. Without a Sound Blaster (or an OPL3 chip standing for
/// one) the row is just `Sound Blaster`, and a card with no FM chip also shows no brackets.
fn sound_blaster_short(probe: &Probe, line: &mut Line) {
    let chip = match probe.fm {
        FmKind::None => None,
        FmKind::Opl2 => Some("OPL2"),
        FmKind::DualOpl2 => Some("2 x OPL2"),
        FmKind::Opl3 => Some("OPL3"),
    };
    let present = probe.sb.is_some() || probe.fm == FmKind::Opl3;
    match chip {
        Some(chip) if present => {
            let _ = write!(line, "Sound Blaster ({chip})");
        }
        _ => {
            let _ = write!(line, "Sound Blaster");
        }
    }
}

/// The Sound Blaster row's detailed words: the DSP version and base as plain text, and what the FM chip is. Used by the row only
/// while `SHOW_SB_DETAIL` is true.
fn sound_blaster_detail(probe: &Probe, line: &mut Line) {
    let fm = match probe.fm {
        FmKind::None => "no FM",
        FmKind::Opl2 => "OPL2",
        FmKind::DualOpl2 => "2 x OPL2 (mono)",
        FmKind::Opl3 => "OPL3",
    };
    match probe.sb {
        Some(SoundBlaster {
            dsp_major,
            dsp_minor,
            base,
        }) => {
            // The long name fits unless the FM words are long; a Pro 1 gets the short name.
            let name = if probe.fm == FmKind::DualOpl2 {
                "SB"
            } else {
                "Sound Blaster"
            };
            let _ = write!(line, "{name} {dsp_major}.{dsp_minor:02} at {base:X}, {fm}");
        }
        None if probe.fm == FmKind::Opl3 => {
            let _ = write!(line, "OPL3 chip, no Sound Blaster DSP");
        }
        None => {
            let _ = write!(line, "Sound Blaster");
        }
    }
}

/// Calls `f` with each row of the four panels: the panel, the row in it, the words and how to draw it.
fn rows(probe: &Probe, resolved: &Resolved, mut f: impl FnMut(usize, usize, &[u8], Row)) {
    let on = probe.video.has_vga();
    let (adapter, mode) = match probe.video {
        VideoClass::Vga => ("VGA adapter", "VGA 320x200x256"),
        VideoClass::MonoVga => ("Mono VGA adapter", "Mono VGA 320x200x256"),
        VideoClass::Ega => ("EGA adapter", "EGA 640x350x16"),
        VideoClass::Cga => ("CGA adapter", "CGA 640x200x2"),
        VideoClass::MdaHercules => ("MDA or Hercules adapter", "80x25 text, mono"),
    };
    f(0, 0, adapter.as_bytes(), Row::Found);
    f(
        0,
        1,
        mode.as_bytes(),
        if on { Row::Found } else { Row::Needed },
    );

    let has = |on: bool| if on { Row::Found } else { Row::Dim };
    f(1, 0, b"Keyboard", Row::Found);
    f(1, 1, b"Mouse", has(probe.mouse));
    f(1, 2, b"Joystick", has(probe.joystick));

    let states = resolved.states;
    f(2, 0, b"PC speaker", states.speaker.into());
    f(2, 1, b"AdLib (OPL2)", states.adlib.into());
    let mut line = Line::new();
    if SHOW_SB_DETAIL {
        sound_blaster_detail(probe, &mut line);
    } else {
        sound_blaster_short(probe, &mut line);
    }
    f(2, 2, line.as_bytes(), states.sb.into());

    let cpu = match probe.cpu {
        CpuClass::I386 => "Processor i386",
        CpuClass::I486 => "Processor i486",
        CpuClass::I586Plus => "Processor Pentium or later",
    };
    f(3, 0, cpu.as_bytes(), Row::Found);
    f(
        3,
        1,
        b"x87 maths (needed)",
        if probe.x87 { Row::Found } else { Row::Needed },
    );
    let mut line = Line::new();
    let kb = probe.free_kb;
    let row = if kb >= MEMORY_FLOOR_KB {
        let _ = write!(line, "{} MB memory free", kb / 1024);
        Row::Found
    } else {
        if kb >= 1024 {
            let _ = write!(
                line,
                "{} MB free, {} MB wanted",
                kb / 1024,
                MEMORY_FLOOR_KB / 1024
            );
        } else {
            let _ = write!(line, "{kb} KB free, {} MB wanted", MEMORY_FLOOR_KB / 1024);
        }
        Row::Warn
    };
    f(3, 2, line.as_bytes(), row);
}

/// The text screen being built.
struct Cells<'a> {
    out: &'a mut [u16; CELLS],
    theme: &'static Theme,
}

impl Cells<'_> {
    fn set(&mut self, x: usize, y: usize, ch: u8, attr: u8) {
        if x < COLS && y < ROWS {
            self.out[y * COLS + x] = u16::from(ch) | u16::from(attr & 0x7F) << 8;
        }
    }

    /// Writes `s` from column `x`; returns the column after it.
    fn put(&mut self, x: usize, y: usize, s: &[u8], attr: u8) -> usize {
        for (i, &ch) in s.iter().enumerate() {
            self.set(x + i, y, ch, attr);
        }
        x + s.len()
    }

    fn fill(&mut self, x: usize, y: usize, n: usize, ch: u8, attr: u8) {
        for i in 0..n {
            self.set(x + i, y, ch, attr);
        }
    }

    fn centre(&mut self, y: usize, s: &[u8], attr: u8) {
        self.put(COLS.saturating_sub(s.len()) / 2, y, s, attr);
    }

    /// A plate: a filled box with a double-line edge in `attr`, a legend on the top edge if `title` is given, and a shadow of
    /// light shade two columns wide down the right (a cell is about twice as tall as it is wide) and one row below, offset two
    /// columns.
    fn plate(&mut self, x: usize, y: usize, w: usize, h: usize, attr: u8, title: Option<&str>) {
        for r in 0..h {
            self.fill(x, y + r, w, b' ', attr);
        }
        self.set(x, y, CP_TOP_LEFT, attr);
        self.fill(x + 1, y, w - 2, CP_H, attr);
        self.set(x + w - 1, y, CP_TOP_RIGHT, attr);
        for r in 1..h - 1 {
            self.set(x, y + r, CP_V, attr);
            self.set(x + w - 1, y + r, CP_V, attr);
        }
        self.set(x, y + h - 1, CP_BOTTOM_LEFT, attr);
        self.fill(x + 1, y + h - 1, w - 2, CP_H, attr);
        self.set(x + w - 1, y + h - 1, CP_BOTTOM_RIGHT, attr);
        if let Some(title) = title {
            let legend = self.theme.legend;
            let end = self.put(x + 2, y, b" ", legend);
            let end = self.put(end, y, title.as_bytes(), legend);
            self.put(end, y, b" ", legend);
        }
        let shadow = self.theme.field;
        for r in 1..=h {
            self.fill(x + w, y + r, 2, CP_LIGHT_SHADE, shadow);
        }
        self.fill(x + 2, y + h, w, CP_LIGHT_SHADE, shadow);
    }
}

/// What the game needs and the machine lacks, joined as in a sentence ("VGA and x87").
fn needs(probe: &Probe, line: &mut Line) {
    let mut parts: [&str; 2] = [""; 2];
    let mut n = 0;
    for (missing, name) in [(!probe.video.has_vga(), "VGA"), (!probe.x87, "x87")] {
        if missing {
            parts[n] = name;
            n += 1;
        }
    }
    for (i, part) in parts[..n].iter().enumerate() {
        let _ = write!(line, "{}{part}", if i == 0 { "" } else { " and " });
    }
}

/// Builds the check screen as 80x25 text cells into `out`: the title, the four panels, and the footer with the resolved plan
/// and the call to play (or what is missing). A monochrome adapter gets attributes that survive its three-level rendering.
pub fn cells(probe: &Probe, resolved: &Resolved, out: &mut [u16; CELLS]) {
    let theme: &'static Theme = if probe.video.is_text_mono() {
        &MONO
    } else {
        &COLOUR
    };
    let mut s = Cells { out, theme };
    for cell in s.out.iter_mut() {
        *cell = u16::from(CP_DARK_SHADE) | u16::from(theme.field) << 8;
    }
    s.plate(6, 2, 68, 3, theme.title_plate, None);
    s.centre(3, b"LIEUTENANT FIZZ 2 - SYSTEM CHECK", theme.title_text);

    const NAMES: [&str; 4] = ["Video", "Input", "Sound", "Machine"];
    const PANEL_X: [usize; 2] = [2, 41];
    const PANEL_Y: [usize; 2] = [6, 12];
    const PANEL_W: usize = 37;
    const FOOT_Y: usize = 18;
    for (i, name) in NAMES.iter().enumerate() {
        s.plate(
            PANEL_X[i % 2],
            PANEL_Y[i / 2],
            PANEL_W,
            5,
            theme.panel,
            Some(name),
        );
    }
    rows(probe, resolved, |panel, n, label, row| {
        let (x, y) = (PANEL_X[panel % 2], PANEL_Y[panel / 2] + 1 + n);
        let (mark, attr) = match row {
            Row::Found => (CP_TICK, theme.found),
            Row::Dim => (b'x', theme.dim),
            Row::InUse => (CP_POINTER, theme.in_use),
            Row::Warn => (CP_WARN, theme.warn),
            Row::Needed => (b'!', theme.needed),
        };
        // A highlighted row is a bar across the panel's inside; the others only colour their own cells.
        if matches!(row, Row::InUse | Row::Warn | Row::Needed) {
            s.fill(x + 1, y, PANEL_W - 2, b' ', attr);
        }
        s.set(x + 2, y, mark, attr);
        s.put(x + 4, y, label, attr);
    });

    s.plate(6, FOOT_Y, 68, 5, theme.panel, None);
    let mut plan = Line::new();
    let music = match resolved.music {
        MusicPlan::Off => "off",
        MusicPlan::Opl2 => "AdLib (OPL2)",
        MusicPlan::Opl3 => "Sound Blaster (OPL3)",
    };
    let fx = match resolved.fx {
        FxPlan::Off => "off",
        FxPlan::Ch5 => "AdLib",
        FxPlan::Ch14 => "Sound Blaster",
        FxPlan::Speaker => "PC speaker",
    };
    let _ = write!(plan, "Music: {music}   Effects: {fx}");
    s.centre(FOOT_Y + 1, plan.as_bytes(), theme.foot);
    if ready(probe) {
        s.centre(FOOT_Y + 3, b"Press Enter to play, Esc to quit", theme.call);
    } else {
        s.fill(7, FOOT_Y + 3, 66, b' ', theme.needed);
        let mut line = Line::new();
        let _ = line.write_str("! Needs ");
        needs(probe, &mut line);
        let _ = line.write_str(": Esc to quit");
        s.centre(FOOT_Y + 3, line.as_bytes(), theme.needed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::SoundBlaster;
    use crate::resolve::resolve;
    use crate::settings::Settings;
    use std::string::String;

    fn screen(probe: &Probe) -> [u16; CELLS] {
        let mut out = [0u16; CELLS];
        cells(probe, &resolve(&Settings::default(), probe), &mut out);
        out
    }

    fn row_text(out: &[u16; CELLS], y: usize) -> String {
        (0..COLS)
            .map(|x| (out[y * COLS + x] & 0xFF) as u8 as char)
            .collect()
    }

    fn sb16() -> Probe {
        Probe {
            fm: FmKind::Opl3,
            sb: Some(SoundBlaster {
                dsp_major: 4,
                dsp_minor: 5,
                base: 0x220,
            }),
            ..Probe::BARE
        }
    }

    #[test]
    fn no_attribute_blinks_and_every_cell_is_written() {
        for probe in [
            Probe::BARE,
            sb16(),
            Probe {
                video: VideoClass::MdaHercules,
                ..sb16()
            },
        ] {
            for cell in screen(&probe) {
                assert_eq!(cell & 0x8000, 0);
                assert_ne!(cell, 0);
            }
        }
    }

    #[test]
    fn title_is_centred() {
        let out = screen(&Probe::BARE);
        let line = row_text(&out, 3);
        let text: std::vec::Vec<char> = line.chars().collect();
        let word: std::vec::Vec<char> = "LIEUTENANT FIZZ 2 - SYSTEM CHECK".chars().collect();
        let start = (COLS - word.len()) / 2;
        assert_eq!(text[start..start + word.len()], word[..]);
    }

    #[test]
    fn sound_labels_fit_their_panel() {
        let machines = [
            Probe::BARE,
            sb16(),
            Probe {
                fm: FmKind::DualOpl2,
                sb: Some(SoundBlaster {
                    dsp_major: 3,
                    dsp_minor: 0,
                    base: 0x240,
                }),
                ..Probe::BARE
            },
            Probe {
                fm: FmKind::Opl2,
                sb: Some(SoundBlaster {
                    dsp_major: 2,
                    dsp_minor: 1,
                    base: 0x220,
                }),
                ..Probe::BARE
            },
            Probe {
                fm: FmKind::Opl3,
                ..Probe::BARE
            },
            Probe {
                fm: FmKind::None,
                sb: Some(SoundBlaster {
                    dsp_major: 2,
                    dsp_minor: 1,
                    base: 0x220,
                }),
                ..Probe::BARE
            },
        ];
        for probe in machines {
            let r = resolve(&Settings::default(), &probe);
            rows(&probe, &r, |panel, _, label, _| {
                // The label starts at column 4 of a 37-wide panel and must leave the right edge alone.
                assert!(
                    4 + label.len() <= 36,
                    "panel {panel}: {}",
                    core::str::from_utf8(label).unwrap()
                );
            });
        }
    }

    fn short(probe: &Probe) -> String {
        let mut line = Line::new();
        sound_blaster_short(probe, &mut line);
        core::str::from_utf8(line.as_bytes()).unwrap().into()
    }

    fn detail(probe: &Probe) -> String {
        let mut line = Line::new();
        sound_blaster_detail(probe, &mut line);
        core::str::from_utf8(line.as_bytes()).unwrap().into()
    }

    fn sb_with(fm: FmKind, major: u8, minor: u8) -> Probe {
        Probe {
            fm,
            sb: Some(SoundBlaster {
                dsp_major: major,
                dsp_minor: minor,
                base: 0x220,
            }),
            ..Probe::BARE
        }
    }

    #[test]
    fn sound_blaster_row_shows_only_device_and_chip() {
        let out = screen(&sb16());
        assert!(row_text(&out, 12 + 3).contains("Sound Blaster (OPL3)"));
        assert_eq!(short(&sb16()), "Sound Blaster (OPL3)");
        assert_eq!(short(&sb_with(FmKind::Opl2, 2, 1)), "Sound Blaster (OPL2)");
        assert_eq!(
            short(&sb_with(FmKind::DualOpl2, 3, 0)),
            "Sound Blaster (2 x OPL2)"
        );
        assert_eq!(short(&Probe::BARE), "Sound Blaster");
        let pro1 = row_text(&screen(&sb_with(FmKind::DualOpl2, 3, 0)), 15);
        assert!(pro1.contains("Sound Blaster (2 x OPL2)"), "{pro1}");
        assert!(!pro1.contains("3.00") && !pro1.contains("at 220"), "{pro1}");
    }

    #[test]
    fn detail_text_keeps_dsp_version_and_base() {
        assert_eq!(detail(&sb16()), "Sound Blaster 4.05 at 220, OPL3");
        assert_eq!(
            detail(&sb_with(FmKind::DualOpl2, 3, 0)),
            "SB 3.00 at 220, 2 x OPL2 (mono)"
        );
    }

    #[test]
    fn three_states_have_three_attributes() {
        // Colour: speaker available, AdLib available, Sound Blaster in use (an SB16 on Auto).
        let out = screen(&sb16());
        let attr = |y: usize| (out[y * COLS + 10] >> 8) as u8;
        assert_eq!(
            (attr(13), attr(14), attr(15)),
            (COLOUR.found, COLOUR.found, COLOUR.in_use)
        );
        // Speaker only: AdLib and Sound Blaster missing, speaker in use.
        let out = screen(&Probe::BARE);
        let attr = |y: usize| (out[y * COLS + 10] >> 8) as u8;
        assert_eq!(
            (attr(13), attr(14), attr(15)),
            (COLOUR.in_use, COLOUR.dim, COLOUR.dim)
        );
    }

    #[test]
    fn a_monochrome_adapter_uses_only_normal_bright_and_inverse() {
        let probe = Probe {
            video: VideoClass::MdaHercules,
            ..sb16()
        };
        for cell in screen(&probe) {
            assert!(
                matches!(cell >> 8, 0x07 | 0x0F | 0x70),
                "attribute {:#x}",
                cell >> 8
            );
        }
        // Missing, available and in use are three different attributes on it.
        const { assert!(MONO.dim != MONO.found && MONO.found != MONO.in_use && MONO.dim != MONO.in_use) };
    }

    #[test]
    fn low_memory_warns_without_blocking() {
        let low = Probe {
            free_kb: MEMORY_FLOOR_KB - 1,
            ..Probe::BARE
        };
        assert!(ready(&low));
        assert!(row_text(&screen(&low), 15).contains("MB free, 4 MB wanted"));
        let tiny = Probe {
            free_kb: 700,
            ..Probe::BARE
        };
        assert!(row_text(&screen(&tiny), 15).contains("700 KB free"));
        let enough = Probe {
            free_kb: MEMORY_FLOOR_KB,
            ..Probe::BARE
        };
        assert!(row_text(&screen(&enough), 15).contains("4 MB memory free"));
    }

    #[test]
    fn missing_vga_or_x87_is_named_and_blocks() {
        let ega = Probe {
            video: VideoClass::Ega,
            x87: false,
            ..Probe::BARE
        };
        assert!(!ready(&ega));
        let out = screen(&ega);
        assert!(row_text(&out, 21).contains("! Needs VGA and x87: Esc to quit"));
        assert!(!row_text(&out, 21).contains("Enter"));
        assert!(row_text(&screen(&Probe::BARE), 21).contains("Press Enter to play, Esc to quit"));
    }

    #[test]
    fn a_probe_that_changes_nothing_draws_the_same_screen() {
        assert_eq!(screen(&sb16()), screen(&sb16()));
    }
}
