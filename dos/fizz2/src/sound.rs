// The player's sound settings on DOS: `FIZZ2.CFG` read and saved, the command-line flags applied, the plan resolved and its line printed.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// Precedence, per field: a flag wins over the file, the file over Auto. Flags are one-run overrides and are never saved: `save` takes the
// `Prefs` that `load_file` returned, never the flagged settings. The resolved plan is printed as the `SOUND` line and
// handed to the audio service (`fizz_dos_rt::audio`), which plays the cluck on it.
use crate::cfgfile;
use alloc::string::String;
use core::fmt::Write;
use fizz_dos_rt::dos::DosError;
use fizz_dos_rt::{dpmi, env, fpu, irq, println};
use fizz_machine::switches::{self, Parsed};
use fizz_machine::{
    Config, ConfigIssue, FrameRate, Motion, PlacementMode, Prefs, Probe, Resolved, Settings, apply_flags, resolve, write_sound,
};

/// Reads `FIZZ2.CFG` from the current directory. A missing or unreadable file is Auto with no issue; a bad one is Auto with the first reason.
pub fn load_file() -> (Prefs, Option<ConfigIssue>) {
    // 32 bytes are more than any version of the file; a longer file fills the buffer and fails the length check.
    let buf = cfgfile::read(Config::FILE_NAME, 32);
    Config::parse_prefs(&buf)
}

/// Saves `prefs` as `FIZZ2.CFG` in the current directory, for the Options screen to call whenever a setting changes.
pub fn save(prefs: &Prefs) -> Result<(), DosError> {
    cfgfile::save_bytes(Config::FILE_NAME, &Config::encode_prefs(prefs))
}

/// The settings a run uses: the file, then the command-line flags.
pub fn load(args: &[String]) -> (Settings, Option<ConfigIssue>) {
    let (prefs, issue) = load_file();
    let mut settings = prefs.sound;
    apply_flags(&mut settings, args.iter().map(|a| a.as_str()));
    (settings, issue)
}

/// The frame rate a run uses: the flag if one was given, else the file's byte (read with the same `load_file`), else Auto.
pub fn frame_rate(flag: Option<FrameRate>) -> FrameRate {
    flag.unwrap_or_else(|| load_file().0.frame_rate)
}

/// What a run will do with sound: the resolved plan and the line that reports it.
pub struct Plan {
    /// What the audio service plays on (`fizz_dos_rt::audio::start`).
    pub resolved: Resolved,
    line: String,
}

impl Plan {
    /// Resolves the file and the flags against `probe`.
    pub fn new(probe: &Probe) -> Plan {
        let (settings, issue) = load(&env::args());
        let resolved = resolve(&settings, probe);
        let mut line = String::new();
        let _ = write_sound(&resolved, issue, &mut line);
        Plan { resolved, line }
    }

    /// The `SOUND MUSIC <x> FX <y> [<reason>]` line.
    pub fn line(&self) -> &str {
        &self.line
    }

    /// Prints the `SOUND` line. Call it once the screen is back in text mode.
    pub fn print(&self) {
        println!("{}", self.line);
    }
}

/// `/CFGTEST`, a test mode and not a player's switch: saves the settings the flags give (with the 60 Hz frame rate, Reduced motion and Classic placement, to cover
/// those bytes; `/MODERN` and `/NOCAPTIONS` set theirs), reads the file back and prints the bytes and whether they came back equal. It overwrites `FIZZ2.CFG` in the current directory.
pub fn cfgtest_mode() -> i32 {
    let irq0 = dpmi::get_pm_vector(irq::vector_of(0)).ok();
    let int24 = dpmi::get_rm_vector(0x24).ok();
    let mut settings = Settings::default();
    apply_flags(&mut settings, env::args().iter().map(|a| a.as_str()));
    // The key layout and the captions come from the switches; the frame rate, motion and placement are fixed at non-default values so
    // that every byte of the version 3 layout is exercised.
    let from_switches = match switches::parse(env::args().iter().map(|a| a.as_str())) {
        Parsed::Run(run) => run.over(&Prefs::default()),
        _ => Prefs::default(),
    };
    let prefs = Prefs {
        sound: settings,
        frame_rate: FrameRate::Hz60,
        motion: Motion::Reduced,
        placement: PlacementMode::Classic,
        ..from_switches
    };
    let saved = save(&prefs).is_ok();
    let (back, issue) = load_file();
    println!("CFG SAVED {}", if saved { "yes" } else { "no" });
    let bytes = Config::encode_prefs(&back);
    let mut hex = String::new();
    for b in bytes {
        let _ = write!(hex, "{b:02X}");
    }
    println!("CFG BYTES {hex}");
    println!(
        "CFG ROUNDTRIP {}",
        if back == prefs && issue.is_none() { "yes" } else { "no" }
    );
    println!(
        "INT24 RESTORED {}",
        if int24.is_some() && dpmi::get_rm_vector(0x24).ok() == int24 {
            "yes"
        } else {
            "no"
        }
    );
    println!("FPU CW {:04X}", fpu::control_word());
    println!(
        "VECTORS RESTORED {}",
        if irq0.is_some() && dpmi::get_pm_vector(irq::vector_of(0)).ok() == irq0 {
            "yes"
        } else {
            "no"
        }
    );
    0
}
