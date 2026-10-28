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
//   FIZZ2.EXE /FPS30 /FPS60 /FPSAUTO  choose how often /LIVE and /REPLAY draw (frames.rs); the game always steps at 60 Hz
//   FIZZ2.EXE /?        prints the help (also /HELP and -?) and exits 0; an unknown switch prints one line and exits 2
//   (every switch is listed once, in fizz_machine::switches, which also writes the help)
//   FIZZ2.EXE /CFGTEST  a test mode: saves FIZZ2.CFG from the sound flags, reads it back and prints the result
//   FIZZ2.EXE /MODERN   selects the modern keys (arrows or WASD, Z jump, X pogo, C or Space fire) instead of Keen; /KEEN selects Keen's
//                       over a saved Modern. Captions, keys and the frame rate follow FIZZ2.CFG unless a switch says otherwise
//                       keys (arrows, Ctrl jump, Alt pogo, Space fire). It combines with /LIVE or stands alone for /LIVE
//   FIZZ2.EXE /NOCAPTIONS  turns off the words that float above a chicken when it clucks (/CAPTIONS, the default, turns them on; the
//                       last one given wins). The beak still moves. A settings-file field replaces this switch with the settings package
//   FIZZ2.EXE /MUSICTEST  a test mode: steps two loops of the music the sound plan names as fast as possible through the real chip
//                       writes (no timer) and prints the `MUSIC` line; `/MUSIC:OFF` prints `ARR off` and zeros
//   FIZZ2.EXE /BENCH /AUDIO  the bench with the 560 Hz audio service, first without and then with music, and what the service costs
//   FIZZ2.EXE /REPLANTEST  a test mode: runs the audio service, changes its music and effects devices live through a list of plans
//                       (`audio::replan`) and prints a `REPLAN` line for each (replan.rs)
//   FIZZ2.EXE /PANIC    starts the audio service (IRQ 0 hooked at 560 Hz, a cluck sounding) and panics, to show the clean-up list runs (exit 3)
//
// /LIVE and /REPLAY run the timer at 560 Hz through `audio::start`, hand each `Event::Cluck` to the audio service and print an `AUDIO`
// line with the register writes and speaker steps it made, and the music starts with the session: a `MUSIC` line next to it says what the
// music wrote (the player's own tick count, the writes, the loops and the FNV-1a of every write); /BENCH keeps the 60 Hz timer unless it is
// given /AUDIO.
//
// Output is printed only after the screen is back in text mode and the vectors are restored. Interrupt handlers (in the runtime)
// never touch the x87; the control word is set to 027Fh first so `f64` rounds as IEEE doubles do.
#![no_std]
#![no_main]

extern crate alloc;

mod bench;
mod cfgfile;
mod check;
mod frames;
mod hal;
mod machine;
mod replan;
mod sound;

use alloc::string::String;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicBool, Ordering};
use fizz_dos_rt::{audio, dpmi, entry, env, fpu, irq, kbd, println, timer};
use fizz_ep2::{Event, SLICE_SCRIPT, SLICE_TICKS, World, level};
use fizz_hal::{Display, InputSource, KeyCode, Pacer};
use fizz_machine::Resolved;
use fizz_machine::switches::{self, Mode, Parsed};
use fizz_machine::{Captions, Keys, Motion, PlacementMode};
use fizz_present_dos::{
    DrawOptions, Frame, Layout, Placement, Puffs, VIEW_H, VIEW_W, cluck_placement, dos_palette, key_to_buttons, present,
    render_slice_with,
};
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

/// Whether sound placement is Classic for this run (set once in `main` from `FIZZ2.CFG` and the switches; the program is single-threaded).
static CLASSIC_PLACEMENT: AtomicBool = AtomicBool::new(false);

/// Hands the clucks the last step emitted to the audio service, each with where its chicken is in the view the frame is drawn in
/// (Enhanced), or centred at full volume (Classic). Positions are read after the step, with the camera the picture uses; the view
/// and the placement are for sound only and have no effect on the rules.
fn play_events(world: &World, options: &DrawOptions) {
    for event in world.events() {
        match *event {
            Event::Cluck { variant, chicken } => {
                let place = if CLASSIC_PLACEMENT.load(Ordering::Relaxed) {
                    Placement::CENTRE
                } else {
                    cluck_placement(world, chicken, VIEW_W, VIEW_H, options)
                };
                audio::cluck(variant, &place);
            }
            // The puff is drawn, not heard; the shot sound comes with the other effects in a later milestone.
            Event::ShotEnd { .. } => {}
        }
    }
}

/// The `MUSIC` lines: what the music wrote (the player's own tick count, not wall time), and the chip order and busiest tick. Call it once
/// the service has stopped. The writes and hash are the ones `fizz_music::writes_for` gives for the same arrangement and ticks.
fn print_music() {
    let m = audio::music_report();
    let arr = match m.arr {
        1 => "opl2",
        2 => "opl3",
        _ => "off",
    };
    println!(
        "MUSIC SONG {} ARR {} TICKS {} WRITES {} LOOPS {} HASH {:08x}",
        fizz_music::level_song().id,
        arr,
        m.ticks,
        m.writes,
        m.loops,
        m.hash
    );
    println!(
        "MUSIC DETAIL ORDER {} MAX_PER_TICK {}",
        match m.order {
            1 => "ok",
            2 => "bad",
            _ => "none",
        },
        m.max_per_tick
    );
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
    display.set_palette(0, &dos_palette())
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
    // The switch table (`fizz_machine::switches`) is the one list of what this program accepts, and it writes the help too. Help and
    // an unknown switch end here, before the video mode or any hook is touched.
    let run = match switches::parse(args.iter().map(|a| a.as_str())) {
        Parsed::Help => return help(),
        Parsed::Unknown(arg) => {
            println!("FIZZ2.EXE: unknown switch {arg}, try FIZZ2.EXE /?");
            return 2;
        }
        Parsed::Run(run) => run,
    };
    // Per field, a switch beats `FIZZ2.CFG`, which beats the default; the switches are never saved (`sound::save` takes the file's own
    // `Prefs`).
    let prefs = run.over(&sound::load_file().0);
    let layout = match prefs.keys {
        Keys::Keen => Layout::Keen,
        Keys::Modern => Layout::Modern,
    };
    CLASSIC_PLACEMENT.store(prefs.placement == PlacementMode::Classic, Ordering::Relaxed);
    let options = DrawOptions {
        captions: prefs.captions == Captions::On,
        // Motion gates the death shake and the death spin; Reduced leaves them off.
        shake: prefs.motion == Motion::Full,
    };
    match run.mode {
        Mode::Live => live(layout, &options),
        Mode::Replay => replay(&options),
        Mode::Probe => machine::probe_mode(),
        Mode::Check => match check::check_mode() {
            check::Choice::Quit => 0,
            check::Choice::Play => live(layout, &options),
        },
        Mode::CheckCells => check::cells_mode(),
        Mode::CfgTest => sound::cfgtest_mode(),
        // `/BENCH <ticks>` times only the first `ticks` ticks of the script, for quick measurements while changing the drawing.
        Mode::Bench if run.bench_audio => bench_audio(run.bench_ticks.unwrap_or(SLICE_TICKS), &options),
        Mode::Bench => bench(run.bench_ticks.unwrap_or(SLICE_TICKS), &options),
        Mode::MusicTest => music_test(),
        Mode::ReplanTest => replan::replan_mode(),
        Mode::Panic => {
            // With the audio service running and a cluck sounding, so the panic handler's clean-up has the chip to silence.
            let plan = sound::Plan::new(&machine::probe_machine());
            if let Err(e) = audio::start(&plan.resolved) {
                println!("HOOK FAILED {:?}", e);
                return 1;
            }
            audio::music_start();
            audio::cluck(0, &Placement::CENTRE);
            // Let the music write for a while (about 0.4 s of 560 Hz ticks), so the clean-up has sounding channels to key off.
            let until = timer::ticks().wrapping_add(200);
            while timer::ticks().wrapping_sub(until) > u32::MAX / 2 {}
            println!("MUSIC PANIC WRITES {}", audio::music_report().writes);
            println!("HOOKED, PANICKING");
            panic!("requested by /PANIC");
        }
    }
}

/// `/?`: the help text from the switch table, a line at a time so each ends in the console's CR LF. It touches no hardware.
fn help() -> i32 {
    let mut text = String::new();
    let _ = switches::write_help(&mut text);
    for line in text.lines() {
        println!("{line}");
    }
    0
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
    // The presenter starts the music, never the rules: nothing in `fizz_ep2` reads the music clock.
    audio::music_start();

    let clock = DosClock;
    let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
    let mut drawing = frames::Drawing::new();
    let mut frame = Frame::new();
    let mut world = level::world();
    let mut out = Transcript::new();
    let mut puffs = Puffs::new();
    let mut prev = Buttons::NONE;
    let mut run = 0usize;
    let mut left = SLICE_SCRIPT.first().map_or(0, |r| u32::from(r.0));

    let _ = fizz_ep2::replay::write_start(&mut out, &world);
    drawing.first(|| draw_slice_frame(&mut display, &mut frame, &world, options, &puffs));

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
            play_events(&world, options);
            prev = now;
            let _ = fizz_ep2::replay::write_step(&mut out, &world);
            puffs.observe(&world);
        }
        if due > 0 {
            drawing.maybe(world.tick.0, || {
                draw_slice_frame(&mut display, &mut frame, &world, options, &puffs)
            });
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
    print_music();
    let exited = world.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED);
    println!("EXIT {}", if exited { "yes" } else { "no" });
    println!("FPU CW {:04X}", cw);
    println!("BIOS TICKS {}", bios_end.wrapping_sub(session.bios_start));
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    drawing.print(world.tick.0);
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
    let (world, r) = bench::run(&mut display, limit, options, false);
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
    audio::music_start();

    let clock = DosClock;
    let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
    let mut drawing = frames::Drawing::new();
    let mut input = DosInput::new();
    let mut latch = InputLatch::new();
    let mut frame = Frame::new();
    let mut world = level::world();
    let mut puffs = Puffs::new();
    let mut quit = false;

    drawing.first(|| draw_slice_frame_synced(&mut display, &mut frame, &world, options, &puffs));
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
            play_events(&world, options);
        }
        if due > 0 {
            drawing.maybe(world.tick.0, || {
                draw_slice_frame_synced(&mut display, &mut frame, &world, options, &puffs)
            });
        }
    }

    end_session(&mut display);
    let restored = vector_now(0) == session.irq0 && vector_now(1) == session.irq1;
    // Ben has moved when he is anywhere but the spawn he started at: a step changes `x`, a jump changes `y`.
    let moved = world.ben.b.x.to_bits() != level::SPAWN.0.to_bits() || world.ben.b.y.to_bits() != level::SPAWN.1.to_bits();
    println!("LIVE TICKS {} HASH {:016x}", world.tick.0, fizz_ep2::hash(&world).finish());
    sound.print();
    print_audio();
    print_music();
    println!("MOVED {}", if moved { "yes" } else { "no" });
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    drawing.print(world.tick.0);
    0
}

/// `/MUSICTEST`: the music's two loops through the real chip writes and no timer. The `MUSIC` line it prints is compared by the check with
/// `fizz_music::writes_for` for the same arrangement and ticks.
fn music_test() -> i32 {
    // Probed first: the probes' waits need the PIT at the BIOS rate (nothing here hooks the timer).
    let sound = sound::Plan::new(&machine::probe_machine());
    if let Err(e) = audio::music_test(&sound.resolved) {
        println!("HOOK FAILED {:?}", e);
        return 1;
    }
    sound.print();
    print_music();
    println!("FPU CW {:04X}", fpu::control_word());
    0
}

/// PIT clocks to microseconds (0.838 us each), in integers.
fn clocks_to_us(clocks: u64) -> u64 {
    clocks * 1_000_000 / 1_193_182
}

fn print_pass(name: &str, r: &bench::Report) {
    for (section, s) in [
        ("STEP", &r.step),
        ("DRAW", &r.draw),
        ("PRESENT", &r.present),
        ("TOTAL", &r.total),
    ] {
        println!(
            "BENCH AUDIO {} {} avg {} max {} units us",
            name,
            section,
            s.avg_us(),
            s.max_us()
        );
    }
}

/// `/BENCH /AUDIO`: the bench with the 560 Hz audio service running, first without music and then with it, each pass the whole script
/// with its clucks played. It reports the step, draw and copy averages of both passes, what the interrupt costs (PIT counts latched at
/// entry and exit), the most writes in one tick and the OPL status reads. DOSBox charges no time for ISA port I/O, so the microseconds
/// here are the emulator's; the exact counts are the evidence, and the time on a real 486 is unmeasured.
fn bench_audio(limit: u64, options: &DrawOptions) -> i32 {
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
    audio::set_profile(true);
    let (world_off, off) = bench::run(&mut display, limit, options, true);
    let irq_off = audio::profile();
    let music_off = audio::music_report();
    audio::set_profile(true);
    audio::music_start();
    let (world_on, on) = bench::run(&mut display, limit, options, true);
    let irq_on = audio::profile();
    let music_on = audio::music_report();
    let cw = fpu::control_word();
    end_session(&mut display);
    let restored = session.irq0.is_some() && vector_now(0) == session.irq0 && vector_now(1) == session.irq1;

    sound.print();
    println!(
        "BENCH AUDIO TIMER PIT divisor {} (559.9 Hz), ARR {}",
        timer::PIT_DIVISOR_560HZ,
        match music_on.arr {
            1 => "opl2",
            2 => "opl3",
            _ => "off",
        }
    );
    println!("BENCH AUDIO TICKS {} {}", off.ticks, on.ticks);
    print_pass("off", &off);
    print_pass("on", &on);
    for (name, p) in [("off", &irq_off), ("on", &irq_on)] {
        let n = u64::from(p.count).max(1);
        let tenths = clocks_to_us(u64::from(p.sum_clocks) * 10 / n);
        println!(
            "BENCH AUDIO {} IRQ count {} avg {}.{} max {} discarded {} units us",
            name,
            p.count,
            tenths / 10,
            tenths % 10,
            clocks_to_us(u64::from(p.max_clocks)),
            p.bad
        );
    }
    let on_writes = irq_on.total_writes.wrapping_sub(irq_off.total_writes);
    println!(
        "BENCH AUDIO off MAX_WRITES_PER_TICK {} OPL_WRITES {} STATUS_READS {}",
        irq_off.max_tick_writes,
        irq_off.total_writes,
        u64::from(irq_off.total_writes) * u64::from(audio::READS_PER_WRITE)
    );
    println!(
        "BENCH AUDIO on MAX_WRITES_PER_TICK {} OPL_WRITES {} STATUS_READS {}",
        irq_on.max_tick_writes,
        on_writes,
        u64::from(on_writes) * u64::from(audio::READS_PER_WRITE)
    );
    println!(
        "BENCH AUDIO on MUSIC TICKS {} WRITES {} MAX_PER_TICK {} (before the pass: {} writes)",
        music_on.ticks, music_on.writes, music_on.max_per_tick, music_off.writes
    );
    println!(
        "BENCH AUDIO HASH {:016x} {:016x}",
        fizz_ep2::hash(&world_off).finish(),
        fizz_ep2::hash(&world_on).finish()
    );
    println!("BENCH AUDIO NOTE DOSBox charges no time for ISA port I/O: the counts are exact, the 486 time is not measured");
    let exited = world_off.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED)
        && world_on.flags.contains(fizz_ep2::WorldFlags::EXIT_REACHED);
    println!("EXIT {}", if exited { "yes" } else { "no" });
    println!("FPU CW {:04X}", cw);
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}
