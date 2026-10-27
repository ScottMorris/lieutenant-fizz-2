// Compares the check screen's cells for thirteen machines with the golden file, and regenerates it with `BLESS=1`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The file follows Fizz 1's `expected-startup-cells.txt`: for each case a `== name` line, 25 lines of text (code page 437
// characters shown as their Unicode look-alikes) and 25 lines of attributes, two hexadecimal digits per cell (background
// nibble, then foreground). `BLESS=1 cargo test -p fizz-machine --test golden` rewrites the file; without it the test fails on
// any difference and prints the first differing case.

use fizz_machine::{
    Blaster, CELLS, COLS, CpuClass, FmKind, FxChoice, MusicChoice, Probe, ROWS, Settings,
    SoundBlaster, VideoClass, cells, resolve,
};
use std::fmt::Write;

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/check-cells.txt");

fn sb(major: u8, minor: u8) -> Option<SoundBlaster> {
    Some(SoundBlaster {
        dsp_major: major,
        dsp_minor: minor,
        base: 0x220,
    })
}

fn blaster() -> Blaster {
    fizz_machine::parse_blaster("A220 I7 D1 H5 T6")
}

fn machine(fm: FmKind, sb: Option<SoundBlaster>) -> Probe {
    Probe {
        fm,
        sb,
        ..Probe::BARE
    }
}

fn auto() -> Settings {
    Settings::default()
}

fn cases() -> Vec<(&'static str, Probe, Settings)> {
    let sb16 = Probe {
        mouse: true,
        free_kb: 13 * 1024,
        blaster: blaster(),
        ..machine(FmKind::Opl3, sb(4, 5))
    };
    vec![
        ("i486, VGA, SB16, mouse, 13 MB, Auto", sb16, auto()),
        (
            "i486, VGA, AdLib only, 8 MB, Auto",
            Probe {
                mouse: true,
                ..machine(FmKind::Opl2, None)
            },
            auto(),
        ),
        (
            "i486, VGA, Sound Blaster Pro 1 (2 x OPL2), joystick, Auto",
            Probe {
                joystick: true,
                blaster: blaster(),
                ..machine(FmKind::DualOpl2, sb(3, 0))
            },
            auto(),
        ),
        (
            "i486, VGA, Sound Blaster 2.0, Auto",
            Probe {
                blaster: blaster(),
                ..machine(FmKind::Opl2, sb(2, 1))
            },
            auto(),
        ),
        ("i486, VGA, PC speaker only, Auto", Probe::BARE, auto()),
        (
            "i486, EGA, SB16",
            Probe {
                video: VideoClass::Ega,
                ..sb16
            },
            auto(),
        ),
        (
            "i486, CGA, AdLib",
            Probe {
                video: VideoClass::Cga,
                ..machine(FmKind::Opl2, None)
            },
            auto(),
        ),
        (
            "i486, Hercules (monochrome attributes), AdLib",
            Probe {
                video: VideoClass::MdaHercules,
                ..machine(FmKind::Opl2, None)
            },
            auto(),
        ),
        (
            "i386, no x87, mono VGA, AdLib, 4 MB",
            Probe {
                video: VideoClass::MonoVga,
                cpu: CpuClass::I386,
                x87: false,
                free_kb: 4096,
                ..machine(FmKind::Opl2, None)
            },
            auto(),
        ),
        (
            "i486, VGA, SB16, low memory (2 MB)",
            Probe {
                free_kb: 2048,
                ..sb16
            },
            auto(),
        ),
        (
            "Pentium, VGA, SB16, mouse, joystick, 32 MB, Auto",
            Probe {
                cpu: CpuClass::I586Plus,
                joystick: true,
                free_kb: 32 * 1024,
                ..sb16
            },
            auto(),
        ),
        (
            "i486, VGA, SB16, config music AdLib and effects AdLib",
            sb16,
            Settings {
                music: MusicChoice::AdLib,
                fx: FxChoice::AdLib,
            },
        ),
        (
            "i486, VGA, SB16, effects Speaker and music Sound Blaster",
            sb16,
            Settings {
                music: MusicChoice::Sb,
                fx: FxChoice::Speaker,
            },
        ),
    ]
}

/// A code page 437 byte as a character for the text lines. Only the characters the screen uses are mapped.
fn cp437(b: u8) -> char {
    match b {
        0x20..=0x7E => b as char,
        0x10 => '\u{25BA}',
        0x1E => '\u{25B2}',
        0xB0 => '\u{2591}',
        0xB2 => '\u{2593}',
        0xBA => '\u{2551}',
        0xBB => '\u{2557}',
        0xBC => '\u{255D}',
        0xC8 => '\u{255A}',
        0xC9 => '\u{2554}',
        0xCD => '\u{2550}',
        0xFB => '\u{221A}',
        _ => panic!("code page 437 byte {b:#04x} is not mapped; add it to cp437()"),
    }
}

fn render() -> String {
    let mut text = String::new();
    for (name, probe, settings) in cases() {
        let mut out = [0u16; CELLS];
        cells(&probe, &resolve(&settings, &probe), &mut out);
        writeln!(text, "== {name}").unwrap();
        for y in 0..ROWS {
            let line: String = (0..COLS).map(|x| cp437(out[y * COLS + x] as u8)).collect();
            writeln!(text, "{line}").unwrap();
        }
        for y in 0..ROWS {
            for x in 0..COLS {
                write!(text, "{:02x}", out[y * COLS + x] >> 8).unwrap();
            }
            text.push('\n');
        }
    }
    text
}

#[test]
fn check_cells_match_the_golden_file() {
    let got = render();
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(FILE, &got).expect("write the golden file");
        return;
    }
    let want = std::fs::read_to_string(FILE)
        .expect("read tests/check-cells.txt (regenerate with BLESS=1)");
    if got != want {
        let (g, w): (Vec<&str>, Vec<&str>) = (got.lines().collect(), want.lines().collect());
        let case = g
            .iter()
            .zip(&w)
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, _)| i);
        let name = case.and_then(|i| g[..=i].iter().rev().find(|l| l.starts_with("== ")));
        panic!(
            "the check screen differs from tests/check-cells.txt (first difference at line {:?}, case {:?}); \
             if the change is intended, run BLESS=1 cargo test -p fizz-machine --test golden and review the diff",
            case.map(|i| i + 1),
            name
        );
    }
}

#[test]
fn every_case_has_a_distinct_name_and_the_expected_shape() {
    let text = render();
    let names: Vec<&str> = text.lines().filter(|l| l.starts_with("== ")).collect();
    assert_eq!(names.len(), 13);
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len());
    assert_eq!(text.lines().count(), 13 * (1 + 2 * ROWS));
    for line in text
        .lines()
        .filter(|l| !l.starts_with("== "))
        .skip(ROWS)
        .take(ROWS)
    {
        assert_eq!(line.len(), COLS * 2);
    }
}
