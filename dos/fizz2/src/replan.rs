// `FIZZ2.EXE /REPLANTEST`: runs the audio service and changes its music and effects devices live, as the Options screen will.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// The mode walks a fixed list of sound settings, resolves each against the machine the probes found (so a machine without a card falls
// back the way a player's choice would) and, after the first, hands the plan to `audio::replan`. Each stage plays a cluck and runs for
// `DWELL` ticks of the 560 Hz clock (half a second); its `REPLAN` line says what the chip and the speaker gate looked like the moment the
// replan returned, and what the service did during the dwell:
//
//   REPLAN <from> -> <to> GATE0 <g> OPL3 <on|off> GATE <g> CHANGED <yes|no> MUSIC0 <ticks> WRITES <n> STEPS <n> TICKS <n> MUSIC <ticks>
//   (<g> is open or closed)
//
// `from` and `to` are `<music>/<fx>` plan names (`start` before the first). `GATE0` is port 0x61 just before the replan,
// `OPL3` the service's record of register 0x105 and `GATE` port 0x61 read back as the replan returned.
// `MUSIC0` is the music player's tick count when `replan` returned (0 when the music started again), `WRITES` every OPL write made
// during the dwell (music and effects), `STEPS` the speaker steps, `TICKS` the handler ticks and `MUSIC` the music player's ticks at the
// end. Lines print once the hooks are off and the vectors are back, then `REPLAN DONE <n>` and the platform lines.
use alloc::string::String;
use core::fmt::Write;
use fizz_dos_rt::{audio, dpmi, fpu, irq, println, timer};
use fizz_machine::{FxChoice, FxPlan, MusicChoice, MusicPlan, Resolved, Settings, resolve};
use fizz_present_dos::Placement;

use crate::machine;

/// How long each stage runs, in ticks of the 560 Hz clock.
const DWELL: u32 = 280;
/// Ticks before the end of a stage at which its cluck starts. A speaker cluck takes 192 ticks, so it is still sounding when the next
/// stage replans (the speaker gate is open then), and a music-only chip has its effect voice keyed on.
const CLUCK_LEAD: u32 = 100;

/// The settings the mode walks, in order. The list leaves the Sound Blaster for the AdLib, goes to the speaker and away from it,
/// turns everything off and comes back.
const STAGES: [(MusicChoice, FxChoice); 8] = [
    (MusicChoice::Auto, FxChoice::Auto),
    (MusicChoice::AdLib, FxChoice::AdLib),
    (MusicChoice::Sb, FxChoice::Speaker),
    (MusicChoice::Off, FxChoice::Speaker),
    (MusicChoice::Sb, FxChoice::Sb),
    (MusicChoice::Off, FxChoice::Off),
    (MusicChoice::AdLib, FxChoice::Speaker),
    (MusicChoice::Auto, FxChoice::Auto),
];

fn name(plan: &Resolved) -> String {
    let music = match plan.music {
        MusicPlan::Off => "off",
        MusicPlan::Opl2 => "opl2",
        MusicPlan::Opl3 => "opl3",
    };
    let fx = match plan.fx {
        FxPlan::Off => "off",
        FxPlan::Ch5 => "ch5",
        FxPlan::Ch14 => "ch14",
        FxPlan::Speaker => "speaker",
    };
    let mut s = String::new();
    let _ = write!(s, "{music}/{fx}");
    s
}

fn wait(ticks: u32) {
    let until = timer::ticks().wrapping_add(ticks);
    while timer::ticks().wrapping_sub(until) > u32::MAX / 2 {}
}

pub fn replan_mode() -> i32 {
    // Probed before the timer is hooked: the probes' waits need the PIT at the BIOS rate.
    let probe = machine::probe_machine();
    let irq0 = dpmi::get_pm_vector(irq::vector_of(0)).ok();
    let plans = STAGES.map(|(music, fx)| resolve(&Settings { music, fx }, &probe));
    if let Err(e) = audio::start(&plans[0]) {
        println!("HOOK FAILED {:?}", e);
        return 1;
    }
    audio::music_start();
    let mut report = String::new();
    let mut from = String::from("start");
    for (i, plan) in plans.iter().enumerate() {
        // The gate as the last stage left it: open when its cluck is still sounding on the speaker.
        let gate0 = audio::speaker_gate_open();
        let changed = if i == 0 { true } else { audio::replan(plan) };
        let (opl3, gate, music0) = (audio::opl3_mode(), audio::speaker_gate_open(), audio::music_report().ticks);
        let (w0, s0, t0) = (audio::profile().total_writes, audio::speaker_steps(), audio::service_ticks());
        // The cluck starts late in the dwell and lasts past its end, so the next replan comes while it is still sounding.
        wait(DWELL - CLUCK_LEAD);
        audio::cluck(0, &Placement::CENTRE);
        wait(CLUCK_LEAD);
        let to = name(plan);
        let _ = writeln!(
            report,
            "REPLAN {} -> {} GATE0 {} OPL3 {} GATE {} CHANGED {} MUSIC0 {} WRITES {} STEPS {} TICKS {} MUSIC {}",
            from,
            to,
            if gate0 { "open" } else { "closed" },
            if opl3 { "on" } else { "off" },
            if gate { "open" } else { "closed" },
            if changed { "yes" } else { "no" },
            music0,
            audio::profile().total_writes.wrapping_sub(w0),
            audio::speaker_steps().wrapping_sub(s0),
            audio::service_ticks().wrapping_sub(t0),
            audio::music_report().ticks,
        );
        from = to;
    }
    let cw = fpu::control_word();
    audio::stop();
    timer::unhook();
    let restored = irq0.is_some() && dpmi::get_pm_vector(irq::vector_of(0)).ok() == irq0;
    for line in report.lines() {
        println!("{line}");
    }
    println!("REPLAN DONE {}", plans.len());
    println!("FPU CW {:04X}", cw);
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}
