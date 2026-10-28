// FIZZ2.EXE: the shared rules, presenter and HAL on a 486-class DOS target, playing the Milestone 1 slice.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository; the loop shape follows rtsmoke and the transcript is `fizz_ep2::replay`'s.
//
//   FIZZ2.EXE /LIVE     plays the slice from the keyboard; Esc ends it. The default when no mode is given
//   FIZZ2.EXE /REPLAY   runs the slice's built-in script (1,268 ticks at 60 Hz), drawing it as it goes, and prints its transcript
//                       v1 plus the platform lines: the parity gate
//   FIZZ2.EXE /BENCH [n] runs the script once as fast as the machine allows and prints what each tick costs, in microseconds, for the
//                       simulation step, the drawing and the copy to the screen, against the 16,667 us a 60 Hz tick allows
//   FIZZ2.EXE /PROBE    probes the machine (video, mouse, joystick, FM chip, Sound Blaster, x87, processor, memory) and prints what it found
//                       as `PROBE` lines with the hash of the check screen those values make; it needs no key and draws nothing
//   FIZZ2.EXE /CHECK    shows the machine check (an 80x25 text screen, read-only): Esc leaves, Enter goes on to /LIVE. It waits for a
//                       key, so no script or test runs it
//   FIZZ2.EXE /CHECKCELLS draws the same screen, reads the 4,000 bytes back from video memory and prints them as hex (no key needed)
//   FIZZ2.EXE /MUSIC:x /FX:x /ADLIB /SB /SPEAKER /NOSOUND  choose the sound for this run over FIZZ2.CFG (sound.rs) and print a `SOUND` line
//   FIZZ2.EXE /CFGTEST  a test mode: saves FIZZ2.CFG from the sound flags, reads it back and prints the result
//   FIZZ2.EXE /MODERN   selects the modern keys (arrows or WASD, Z jump, X pogo, C or Space fire) instead of the default Keen
//                       keys (arrows, Ctrl jump, Alt pogo, Space fire). It combines with /LIVE or stands alone for /LIVE
//   FIZZ2.EXE /NOCAPTIONS  turns off the words that float above a chicken when it clucks (/CAPTIONS, the default, turns them on; the
//                       last one given wins). The beak still moves. A settings-file field replaces this switch with the settings package
//   FIZZ2.EXE /PANIC    starts the audio service (IRQ 0 hooked at 560 Hz, a cluck sounding) and panics, to show the clean-up list runs (exit 3)
//
// /LIVE and /REPLAY run the timer at 560 Hz through `audio::start`, hand each `Event::Cluck` to the audio service and print an `AUDIO`
// line with the register writes and speaker steps it made; /BENCH keeps the 60 Hz timer its stopwatch is built for.
//
// Output is printed only after the screen is back in text mode and the vectors are restored. Interrupt handlers (in the runtime)
// never touch the x87; the control word is set to 027Fh first so `f64` rounds as IEEE doubles do.
#![no_std]
#![no_main]

extern crate alloc;

mod bench;
mod check;
mod hal;
mod machine;
mod sound;

use core::fmt::{self, Write};
use fizz_dos_rt::{audio, dpmi, entry, env, fpu, irq, kbd, println, timer};
use fizz_ep2::{Event, SLICE_SCRIPT, SLICE_TICKS, World, level};
use fizz_hal::{Display, InputSource, KeyCode, Pacer};
use fizz_machine::Resolved;
use fizz_present_dos::{DrawOptions, Frame, Layout, Puffs, ega_palette, key_to_buttons, present, render_slice_with};
use fizz_sim::{Buttons, InputFrame, InputLatch, TICK_HZ};
use hal::{DosClock, DosDisplay, DosInput};

entry!(main);

/// Steps per `Pacer::due` call at most; a stall longer than this drops time instead of bursting.
const MAX_CATCH_UP: u32 = 5;
/// `/LIVE` ends by itself after this many ticks (30 s) so an emulator run cannot hang.
const LIVE_TICK_CAP: u64 = 1800;

/// Bytes the transcript buffer holds. The slice's transcript, with its `EVENT` lines, is 872 bytes (about 33 bytes for each
/// 60 ticks, 20 for each cluck and 24 for each shot end), so 8,192 leaves ample room for the lines later milestones add to it.
const TRANSCRIPT_BYTES: usize = 8192;

/// A fixed buffer for the transcript, so nothing is printed (and no DOS call made) while the screen is in graphics mode.
struct Transcript {
    bytes: [u8; TRANSCRIPT_BYTES],
    len: usize,
}

impl Transcript {
    const fn new() -> Self {
        Transcript {
            bytes: [0; TRANSCRIPT_BYTES],
            len: 0,
        }
    }

    fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl Write for Transcript {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

type Vector = Option<(u16, u32)>;

/// The vectors as found before hooking, and the BIOS tick counter at the start.
struct Session {
    irq0: Vector,
    irq1: Vector,
    bios_start: u32,
}

fn vector_now(line: u8) -> Vector {
    dpmi::get_pm_vector(irq::vector_of(line)).ok()
}

/// Hooks the timer: at 560 Hz with the audio service for `sound` when there is a plan, at 60 Hz when there is not (`/BENCH`, whose
/// stopwatch is built for the 60 Hz period). The simulation is paced from either clock by the integer `Pacer`.
fn start_session(keyboard: bool, sound: Option<&Resolved>) -> Result<Session, irq::Error> {
    let irq0 = vector_now(0);
    let irq1 = vector_now(1);
    match sound {
        Some(plan) => audio::start(plan)?,
        None => timer::hook_60hz()?,
    }
    if keyboard {
        kbd::hook()?;
    }
    Ok(Session {
        irq0,
        irq1,
        bios_start: timer::bios_ticks(),
    })
}

/// Text mode back first, then the hooks off.
fn end_session(display: &mut DosDisplay) {
    display.restore_text_mode();
    kbd::unhook();
    audio::stop();
    timer::unhook();
}

/// Hands the clucks the last step emitted to the audio service. It has no effect on the rules.
fn play_events(world: &World) {
    for event in world.events() {
        match *event {
            Event::Cluck { variant, .. } => audio::cluck(variant),
        }
    }
}

/// The `AUDIO` line: how many OPL register writes and speaker steps the run made. Call it once the service has stopped.
fn print_audio() {
    println!(
        "AUDIO WRITES {} SPEAKER STEPS {}",
        audio::register_writes(),
        audio::speaker_steps()
    );
}

fn open_display(display: &mut DosDisplay) -> Result<(), fizz_hal::HalError> {
    display.set_mode(0x13)?;
    display.set_palette(0, &ega_palette())
}

fn draw_slice_frame(display: &mut DosDisplay, frame: &mut Frame, world: &fizz_ep2::World, options: &DrawOptions, puffs: &Puffs) {
    render_slice_with(world, frame, options, puffs);
    let _ = present(display, frame);
}

/// The live loop's draw: waits for the vertical retrace, then copies, so the copy does not land mid-scan and tear. The wait
/// is bounded inside `wait_vretrace` (a million status reads per phase), and it only delays the copy: the simulation is paced
/// by the timer through the `Pacer` and never sees the retrace. `/REPLAY` does not wait.
fn draw_slice_frame_synced(
    display: &mut DosDisplay,
    frame: &mut Frame,
    world: &fizz_ep2::World,
    options: &DrawOptions,
    puffs: &Puffs,
) {
    render_slice_with(world, frame, options, puffs);
    let _ = display.wait_vretrace();
    let _ = present(display, frame);
}

fn main() -> i32 {
    fpu::set_double_precision();
    let args = env::args();
    let layout = if args.iter().any(|a| a.eq_ignore_ascii_case("/MODERN")) {
        Layout::Modern
    } else {
        Layout::Keen
    };
    // Captions are on unless the last of `/CAPTIONS` and `/NOCAPTIONS` on the command line says otherwise. The settings file
    // that will hold this choice comes with the settings package; until then it is a switch.
    let captions = !args
        .iter()
        .rfind(|a| a.eq_ignore_ascii_case("/CAPTIONS") || a.eq_ignore_ascii_case("/NOCAPTIONS"))
        .is_some_and(|a| a.eq_ignore_ascii_case("/NOCAPTIONS"));
    let options = DrawOptions {
        captions,
        // On: the Options screen (Milestone 1.5) will own this setting; FIZZ2.CFG v2's reserved detail byte is not used for it.
        shake: true,
    };
    let mode = args
        .iter()
        .map(|a| a.as_str())
        .find(|a| {
            !["/MODERN", "/CAPTIONS", "/NOCAPTIONS"]
                .iter()
                .any(|flag| a.eq_ignore_ascii_case(flag))
                && !sound::is_flag(a)
        })
        .unwrap_or("/LIVE");
    // `/BENCH <ticks>` times only the first `ticks` ticks of the script, for quick measurements while changing the drawing.
    let bench_limit = args
        .iter()
        .filter_map(|a| a.parse::<u64>().ok())
        .next()
        .unwrap_or(SLICE_TICKS);
    if mode.eq_ignore_ascii_case("/REPLAY") {
        replay(&options)
    } else if mode.eq_ignore_ascii_case("/PROBE") {
        machine::probe_mode()
    } else if mode.eq_ignore_ascii_case("/CHECK") {
        match check::check_mode() {
            check::Choice::Quit => 0,
            check::Choice::Play => live(layout, &options),
        }
    } else if mode.eq_ignore_ascii_case("/CHECKCELLS") {
        check::cells_mode()
    } else if mode.eq_ignore_ascii_case("/CFGTEST") {
        sound::cfgtest_mode()
    } else if mode.eq_ignore_ascii_case("/BENCH") {
        bench(bench_limit, &options)
    } else if mode.eq_ignore_ascii_case("/PANIC") {
        // With the audio service running and a cluck sounding, so the panic handler's clean-up has the chip to silence.
        let plan = sound::Plan::new(&machine::probe_machine());
        if let Err(e) = audio::start(&plan.resolved) {
            println!("HOOK FAILED {:?}", e);
            return 1;
        }
        audio::cluck(0);
        println!("HOOKED, PANICKING");
        panic!("requested by /PANIC");
    } else {
        live(layout, &options)
    }
}

/// The slice's scripted run: the transcript, then the platform lines (whether Ben reached the exit, the x87 control word, the
/// BIOS tick count and whether the vectors were restored). The script is played into a fresh level one tick per pacer tick, so the run takes about 21 seconds; the screen shows it as it goes.
fn replay(options: &DrawOptions) -> i32 {
    // Probed before the timer is hooked: the probes' waits need the PIT at the BIOS rate.
    let sound = sound::Plan::new(&machine::probe_machine());
    let session = match start_session(false, Some(&sound.resolved)) {
        Ok(s) => s,
        Err(e) => {
            println!("HOOK FAILED {:?}", e);
            return 1;
        }
    };
    let mut display = DosDisplay;
    if open_display(&mut display).is_err() {
        end_session(&mut display);
        println!("DISPLAY FAILED");
        return 1;
    }

    let clock = DosClock;
    let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
    let mut frame = Frame::new();
    let mut world = level::world();
    let mut out = Transcript::new();
    let mut puffs = Puffs::new();
    let mut prev = Buttons::NONE;
    let mut run = 0usize;
    let mut left = SLICE_SCRIPT.first().map_or(0, |r| u32::from(r.0));

    let _ = fizz_ep2::replay::write_start(&mut out, &world);
    draw_slice_frame(&mut display, &mut frame, &world, options, &puffs);

    while world.tick.0 < SLICE_TICKS {
        let due = pacer.due(&clock);
        for _ in 0..due {
            if world.tick.0 >= SLICE_TICKS {
                break;
            }
            while left == 0 && run + 1 < SLICE_SCRIPT.len() {
                run += 1;
                left = u32::from(SLICE_SCRIPT[run].0);
            }
            let now = SLICE_SCRIPT[run].1;
            left = left.saturating_sub(1);
            fizz_ep2::step(&mut world, InputFrame::from_transition(prev, now));
            play_events(&world);
            prev = now;
            let _ = fizz_ep2::replay::write_step(&mut out, &world);
            puffs.observe(&world);
        }
        if due > 0 {
            draw_slice_frame(&mut display, &mut frame, &world, options, &puffs);
        }
    }
    let _ = fizz_ep2::replay::write_final(&mut out, &world);

    let cw = fpu::control_word();
    let bios_end = timer::bios_ticks();
    end_session(&mut display);
    let restored = session.irq0.is_some() && vector_now(0) == session.irq0 && vector_now(1) == session.irq1;

    for line in out.text().lines() {
        println!("{}", line);
    }
    sound.print();
    print_audio();
    let exited = world.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED);
    println!("EXIT {}", if exited { "yes" } else { "no" });
    println!("FPU CW {:04X}", cw);
    println!("BIOS TICKS {}", bios_end.wrapping_sub(session.bios_start));
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}

/// Times the slice (see `bench`): the figures, then the platform lines, printed once the screen is back in text mode.
fn bench(limit: u64, options: &DrawOptions) -> i32 {
    let session = match start_session(false, None) {
        Ok(s) => s,
        Err(e) => {
            println!("HOOK FAILED {:?}", e);
            return 1;
        }
    };
    let mut display = DosDisplay;
    if open_display(&mut display).is_err() {
        end_session(&mut display);
        println!("DISPLAY FAILED");
        return 1;
    }
    let (cal_loops, cal_us) = bench::calibrate();
    let (world, r) = bench::run(&mut display, limit, options);
    let cw = fpu::control_word();
    end_session(&mut display);
    let restored = session.irq0.is_some() && vector_now(0) == session.irq0 && vector_now(1) == session.irq1;

    println!("BENCH TICKS {}", r.ticks);
    println!(
        "BENCH TIMER PIT 1193182 Hz, 0.84 us per count, reading cost {} counts",
        r.overhead_clocks
    );
    println!(
        "BENCH CALIBRATE {} loops in {} us ({} instructions per loop)",
        cal_loops,
        cal_us,
        bench::CALIBRATE_INSTRUCTIONS_PER_LOOP
    );
    for (name, s) in [
        ("STEP", &r.step),
        ("DRAW", &r.draw),
        ("PRESENT", &r.present),
        ("TOTAL", &r.total),
    ] {
        println!("BENCH {} avg {} max {} units us", name, s.avg_us(), s.max_us());
    }
    for &(tick, digest) in &r.frames[..r.frame_count] {
        println!("BENCH FRAME {} {:016x}", tick, digest);
    }
    println!("BENCH BUDGET {} us per tick at 60 Hz", bench::BUDGET_US);
    println!("BENCH HASH {:016x}", fizz_ep2::hash(&world).finish());
    let exited = world.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED);
    println!("EXIT {}", if exited { "yes" } else { "no" });
    println!("FPU CW {:04X}", cw);
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}

fn live(layout: Layout, options: &DrawOptions) -> i32 {
    let sound = sound::Plan::new(&machine::probe_machine());
    let session = match start_session(true, Some(&sound.resolved)) {
        Ok(s) => s,
        Err(e) => {
            println!("HOOK FAILED {:?}", e);
            return 1;
        }
    };
    let mut display = DosDisplay;
    if open_display(&mut display).is_err() {
        end_session(&mut display);
        println!("DISPLAY FAILED");
        return 1;
    }

    let clock = DosClock;
    let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
    let mut input = DosInput::new();
    let mut latch = InputLatch::new();
    let mut frame = Frame::new();
    let mut world = level::world();
    let mut puffs = Puffs::new();
    let mut quit = false;

    draw_slice_frame_synced(&mut display, &mut frame, &world, options, &puffs);
    while !quit && world.tick.0 < LIVE_TICK_CAP {
        let due = pacer.due(&clock);
        for _ in 0..due {
            while let Some(event) = input.poll_key() {
                if event.code == KeyCode::Escape && event.pressed {
                    quit = true;
                } else if let Some(b) = key_to_buttons(event.code, layout) {
                    latch.set(b, event.pressed);
                }
            }
            if quit || world.tick.0 >= LIVE_TICK_CAP {
                break;
            }
            fizz_ep2::step(&mut world, latch.frame());
            puffs.observe(&world);
            play_events(&world);
        }
        if due > 0 {
            draw_slice_frame_synced(&mut display, &mut frame, &world, options, &puffs);
        }
    }

    end_session(&mut display);
    let restored = vector_now(0) == session.irq0 && vector_now(1) == session.irq1;
    // Ben has moved when he is anywhere but the spawn he started at: a step changes `x`, a jump changes `y`.
    let moved = world.ben.b.x.to_bits() != level::SPAWN.0.to_bits() || world.ben.b.y.to_bits() != level::SPAWN.1.to_bits();
    println!("LIVE TICKS {} HASH {:016x}", world.tick.0, fizz_ep2::hash(&world).finish());
    sound.print();
    print_audio();
    println!("MOVED {}", if moved { "yes" } else { "no" });
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}
