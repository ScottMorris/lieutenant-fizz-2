// Fills in the machine model's `Probe` from the runtime's hardware probes, and runs `FIZZ2.EXE /PROBE`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// `/PROBE` needs no key press and draws nothing: it runs the probes before any timer hook (the waits in them need the PIT at the BIOS
// rate), prints one `PROBE <key> <value>` line for every `Probe` field, the hash of the 2,000 cells the check screen would hold for those
// values with the default settings, how much memory the program holds, and the platform lines the other modes print. The host computes the
// same hash from the same lines (`cargo run -p fizz-machine --example cells-hash`), which is how `tools/check-dos.sh` proves the cells
// are built the same way here as on the host.
use alloc::string::String;
use core::fmt::Write;
use fizz_dos_rt::heap::HEAP;
use fizz_dos_rt::probe::{self, Cpu, Fm, Video};
use fizz_dos_rt::{dpmi, env, fpu, irq, println};
use fizz_machine::{
    CELLS, CpuClass, FmKind, Probe, Settings, SoundBlaster, VideoClass, cells, cells_hash, parse_blaster, resolve, write_probe,
};

/// Runs every probe, in the order the dependencies need: the `BLASTER` variable, then the DSP (whose base the two-chip test uses), then the FM chip.
pub fn probe_machine() -> Probe {
    let blaster = env::var("BLASTER").map(|v| parse_blaster(&v)).unwrap_or_default();
    let dsp = probe::dsp(blaster.base);
    let fm = probe::opl(dsp.map(|d| d.base));
    Probe {
        video: match probe::video() {
            Video::Vga => VideoClass::Vga,
            Video::MonoVga => VideoClass::MonoVga,
            Video::Ega => VideoClass::Ega,
            Video::Cga => VideoClass::Cga,
            Video::Mda => VideoClass::MdaHercules,
        },
        mouse: probe::mouse(),
        joystick: probe::joystick(),
        fm: match fm {
            Fm::None => FmKind::None,
            Fm::Opl2 => FmKind::Opl2,
            Fm::DualOpl2 => FmKind::DualOpl2,
            Fm::Opl3 => FmKind::Opl3,
        },
        sb: dsp.map(|d| SoundBlaster {
            dsp_major: d.major,
            dsp_minor: d.minor,
            base: d.base,
        }),
        blaster,
        x87: probe::fpu(),
        cpu: match probe::cpu() {
            Cpu::I386 => CpuClass::I386,
            Cpu::I486 => CpuClass::I486,
            Cpu::I586Plus => CpuClass::I586Plus,
        },
        free_kb: probe::free_memory().unwrap_or(0),
    }
}

/// `/PROBE`: see the module comment.
pub fn probe_mode() -> i32 {
    let irq0 = dpmi::get_pm_vector(irq::vector_of(0)).ok();
    let irq1 = dpmi::get_pm_vector(irq::vector_of(1)).ok();
    let machine = probe_machine();

    let resolved = resolve(&Settings::default(), &machine);
    let mut screen = [0u16; CELLS];
    cells(&machine, &resolved, &mut screen);

    let mut text = String::new();
    let _ = write_probe(&machine, &mut text);
    let _ = writeln!(text, "CELLS {:016x}", cells_hash(&screen));
    let _ = writeln!(text, "HEAP {}", HEAP.total_bytes());
    match dpmi::free_memory_info().ok().and_then(|m| m.free_pages) {
        Some(pages) => {
            let _ = writeln!(text, "PAGES FREE {pages}");
        }
        None => {
            let _ = writeln!(text, "PAGES FREE unknown");
        }
    }
    let restored = irq0.is_some()
        && dpmi::get_pm_vector(irq::vector_of(0)).ok() == irq0
        && dpmi::get_pm_vector(irq::vector_of(1)).ok() == irq1;
    let _ = writeln!(text, "EXIT yes");
    let _ = writeln!(text, "FPU CW {:04X}", fpu::control_word());
    let _ = writeln!(text, "VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    for line in text.lines() {
        println!("{}", line);
    }
    0
}
