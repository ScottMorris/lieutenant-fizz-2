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
use alloc::string::String;
use core::fmt::Write;
use fizz_dos_rt::dos::{DosError, File, Mode};
use fizz_dos_rt::{dpmi, env, fpu, irq, println};
use fizz_machine::{Config, ConfigIssue, Prefs, Probe, Resolved, Settings, apply_flags, resolve, write_sound};

/// True when `arg` is one of the sound flags (`/MUSIC:x`, `/FX:x`, `/ADLIB`, `/SB`, `/SPEAKER`, `/NOSOUND`), so the mode search skips it.
pub fn is_flag(arg: &str) -> bool {
    apply_flags(&mut Settings::default(), [arg]).applied == 1
}

/// Reads `FIZZ2.CFG` from the current directory. A missing or unreadable file is Auto with no issue; a bad one is Auto with the first reason.
pub fn load_file() -> (Prefs, Option<ConfigIssue>) {
    // 32 bytes are more than any version of the file; a longer file fills the buffer and fails the length check.
    let mut buf = [0u8; 32];
    let n = match File::open(Config::FILE_NAME, Mode::Read) {
        Ok(mut f) => f.read(&mut buf).unwrap_or(0),
        Err(_) => 0,
    };
    Config::parse_prefs(&buf[..n])
}

/// Saves `prefs` as `FIZZ2.CFG` in the current directory, for the Options screen to call whenever a setting changes.
pub fn save(prefs: &Prefs) -> Result<(), DosError> {
    let mut f = File::open(Config::FILE_NAME, Mode::Create)?;
    f.write_all(&Config::encode_prefs(prefs))
}

/// The settings a run uses: the file, then the command-line flags.
pub fn load(args: &[String]) -> (Settings, Option<ConfigIssue>) {
    let (prefs, issue) = load_file();
    let mut settings = prefs.sound;
    apply_flags(&mut settings, args.iter().map(|a| a.as_str()));
    (settings, issue)
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

/// `/CFGTEST`, a test mode and not a player's switch: saves the settings the flags give (with the 60 Hz frame rate, to cover the
/// byte), reads the file back and prints the bytes and whether they came back equal. It overwrites `FIZZ2.CFG` in the current directory.
pub fn cfgtest_mode() -> i32 {
    let irq0 = dpmi::get_pm_vector(irq::vector_of(0)).ok();
    let mut settings = Settings::default();
    apply_flags(&mut settings, env::args().iter().map(|a| a.as_str()));
    let prefs = Prefs {
        sound: settings,
        frame_rate: fizz_machine::FrameRate::Hz60,
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
