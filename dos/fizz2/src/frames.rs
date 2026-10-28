// The drawing cadence of /LIVE and /REPLAY: which ticks get a frame, and the `FRAMES` line.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository; the policy is `fizz_machine::FrameGovernor`, adapted from Fizz 1's DOS port.
//
// The simulation always runs at 60 Hz through the `Pacer`; this only decides whether a pass of the loop draws. The rate is the
// `/FPS30`, `/FPS60` or `/FPSAUTO` flag, else the frame-rate byte of `FIZZ2.CFG`, else Auto (the sound settings' precedence); flags
// are never written to the file. A drawn frame's cost is read from the audio service's 560 Hz tick counter, wait for the retrace
// included when the caller's closure waits.
use crate::sound;
use fizz_dos_rt::{env, println, timer};
use fizz_machine::switches::{self, Parsed};
use fizz_machine::{FrameGovernor, FrameRate};

pub struct Drawing {
    governor: FrameGovernor,
    drawn: u32,
}

impl Drawing {
    /// The governor for this run: the flag over the file over Auto. Reads `FIZZ2.CFG`, so call it before the video mode is set.
    pub fn new() -> Drawing {
        let flag = match switches::parse(env::args().iter().map(|a| a.as_str())) {
            Parsed::Run(run) => run.frame_rate,
            _ => None,
        };
        Drawing {
            governor: FrameGovernor::new(sound::frame_rate(flag)),
            drawn: 0,
        }
    }

    /// Switches to `rate` while running, as the Options screen does. A fixed rate takes effect at once; Auto starts again at 60 Hz
    /// with an empty measuring window. The draw cadence only: the simulation is not told.
    pub fn set_mode(&mut self, rate: FrameRate) {
        self.governor = FrameGovernor::new(rate);
    }

    /// The rate the governor is drawing at now: 60 or 30.
    pub fn rate(&self) -> u32 {
        self.governor.rate()
    }

    /// Draws the first frame, which is counted but not measured.
    pub fn first(&mut self, draw: impl FnOnce()) {
        draw();
        self.drawn += 1;
    }

    /// Draws a frame with `draw` when the governor says the tick calls for one, timing it for Auto. Call it after a pass that stepped.
    pub fn maybe(&mut self, tick: u64, draw: impl FnOnce()) {
        if !self.governor.draw_this_tick(tick) {
            return;
        }
        let start = timer::ticks();
        draw();
        self.drawn += 1;
        let _ = self.governor.record_frame(timer::ticks().wrapping_sub(start));
    }

    /// Prints `FRAMES <n> TICKS <t> RATE <hz> MODE <auto|30|60>`. Call it once the screen is back in text mode.
    pub fn print(&self, ticks: u64) {
        let mode = match self.governor.mode() {
            FrameRate::Auto => "auto",
            FrameRate::Hz30 => "30",
            FrameRate::Hz60 => "60",
        };
        println!(
            "FRAMES {} TICKS {} RATE {} MODE {}",
            self.drawn,
            ticks,
            self.governor.rate(),
            mode
        );
    }
}
