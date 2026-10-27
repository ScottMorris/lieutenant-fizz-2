// FIZZ2.EXE, the Milestone 0 hello-tick program: the shared rules, presenter and HAL on a 486-class DOS target.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository; the loop shape follows rtsmoke and the transcript is `fizz_hello::replay`'s.
//
//   FIZZ2.EXE /REPLAY   runs the built-in M0 script for 600 ticks at 60 Hz and prints transcript v1 plus platform lines
//   FIZZ2.EXE /LIVE     keyboard play (arrows or WASD, Space); Esc ends it. The default when no argument is given
//   FIZZ2.EXE /PANIC    hooks IRQ 0 and panics, to show the clean-up list runs (exit status 3)
//
// Output is printed only after the screen is back in text mode and the vectors are restored. Interrupt handlers (in the runtime)
// never touch the x87; the control word is set to 027Fh first so `f64` rounds as IEEE doubles do.
#![no_std]
#![no_main]

extern crate alloc;

mod hal;

use core::fmt::{self, Write};
use fizz_dos_rt::{dpmi, entry, env, fpu, irq, kbd, println, timer};
use fizz_hal::{Display, InputSource, KeyCode, Pacer};
use fizz_hello::{M0_SCRIPT, M0_TICKS, World, hash, step};
use fizz_present_dos::{Frame, ega_palette, key_to_buttons, present, render};
use fizz_sim::{Buttons, InputFrame, InputLatch, TICK_HZ};
use hal::{DosClock, DosDisplay, DosInput};

entry!(main);

/// Steps per `Pacer::due` call at most; a stall longer than this drops time instead of bursting.
const MAX_CATCH_UP: u32 = 5;
/// `/LIVE` ends by itself after this many ticks (30 s) so an emulator run cannot hang.
const LIVE_TICK_CAP: u64 = 1800;

/// A fixed buffer for the transcript, so nothing is printed (and no DOS call made) while the screen is in graphics mode.
struct Transcript {
    bytes: [u8; 2048],
    len: usize,
}

impl Transcript {
    const fn new() -> Self {
        Transcript {
            bytes: [0; 2048],
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

fn start_session(keyboard: bool) -> Result<Session, irq::Error> {
    let irq0 = vector_now(0);
    let irq1 = vector_now(1);
    timer::hook_60hz()?;
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
    timer::unhook();
}

fn open_display(display: &mut DosDisplay) -> Result<(), fizz_hal::HalError> {
    display.set_mode(0x13)?;
    display.set_palette(0, &ega_palette())
}

fn draw(display: &mut DosDisplay, frame: &mut Frame, world: &World) {
    render(world, frame);
    let _ = present(display, frame);
}

fn main() -> i32 {
    fpu::set_double_precision();
    let args = env::args();
    let mode = args.first().map(|a| a.as_str()).unwrap_or("/LIVE");
    if mode.eq_ignore_ascii_case("/REPLAY") {
        replay()
    } else if mode.eq_ignore_ascii_case("/PANIC") {
        if let Err(e) = timer::hook_60hz() {
            println!("HOOK FAILED {:?}", e);
            return 1;
        }
        println!("HOOKED, PANICKING");
        panic!("requested by /PANIC");
    } else {
        live()
    }
}

fn replay() -> i32 {
    let session = match start_session(false) {
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
    let mut world = World::new();
    let mut out = Transcript::new();
    let mut prev = Buttons::NONE;
    let mut run = 0usize;
    let mut left = M0_SCRIPT.first().map_or(0, |r| r.0);

    let _ = writeln!(out, "M0 REPLAY 1");
    let _ = writeln!(out, "TICK 0 HASH {:016x}", hash(&world).finish());
    draw(&mut display, &mut frame, &world);

    while world.tick.0 < M0_TICKS {
        for _ in 0..pacer.due(&clock) {
            if world.tick.0 >= M0_TICKS {
                break;
            }
            while left == 0 && run + 1 < M0_SCRIPT.len() {
                run += 1;
                left = M0_SCRIPT[run].0;
            }
            let now = M0_SCRIPT[run].1;
            left = left.saturating_sub(1);
            step(&mut world, InputFrame::from_transition(prev, now));
            prev = now;
            if world.tick.0.is_multiple_of(60) {
                let _ = writeln!(out, "TICK {} HASH {:016x}", world.tick.0, hash(&world).finish());
            }
            draw(&mut display, &mut frame, &world);
        }
    }
    let _ = writeln!(out, "FINAL TICK {} HASH {:016x}", world.tick.0, hash(&world).finish());

    let cw = fpu::control_word();
    let bios_end = timer::bios_ticks();
    end_session(&mut display);
    let restored = session.irq0.is_some() && vector_now(0) == session.irq0 && vector_now(1) == session.irq1;

    for line in out.text().lines() {
        println!("{}", line);
    }
    println!("FPU CW {:04X}", cw);
    println!("BIOS TICKS {}", bios_end.wrapping_sub(session.bios_start));
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}

fn live() -> i32 {
    let session = match start_session(true) {
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
    let mut world = World::new();
    let mut quit = false;

    draw(&mut display, &mut frame, &world);
    while !quit && world.tick.0 < LIVE_TICK_CAP {
        for _ in 0..pacer.due(&clock) {
            while let Some(event) = input.poll_key() {
                if event.code == KeyCode::Escape && event.pressed {
                    quit = true;
                } else if let Some(b) = key_to_buttons(event.code) {
                    latch.set(b, event.pressed);
                }
            }
            if quit || world.tick.0 >= LIVE_TICK_CAP {
                break;
            }
            step(&mut world, latch.frame());
            draw(&mut display, &mut frame, &world);
        }
    }

    end_session(&mut display);
    let restored = vector_now(0) == session.irq0 && vector_now(1) == session.irq1;
    let moved = world.x.to_bits() != 160.0_f64.to_bits() || world.y.to_bits() != 100.0_f64.to_bits();
    println!("LIVE TICKS {} HASH {:016x}", world.tick.0, hash(&world).finish());
    println!("MOVED {}", if moved { "yes" } else { "no" });
    println!("VECTORS RESTORED {}", if restored { "yes" } else { "no" });
    0
}
