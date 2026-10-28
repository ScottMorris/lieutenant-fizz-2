// Platform adapters: the `fizz-hal` traits over the DOS runtime's plain functions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The runtime (`fizz-dos-rt`) owns every `unsafe` block, so nothing here needs one. The traits are the shape a second platform
// (the web presenter) implements too, which keeps the game loop independent of DOS.
use fizz_dos_rt::{kbd, timer, vga};
use fizz_hal::{Clock, Display, HalError, InputSource, KeyEvent, ModeInfo, Rgb6, Set1Decoder, Ticks};

/// The PIT at divisor 2131 (the audio service's 560 Hz clock) runs at 559.9165 Hz, which rounds to 559,916 millihertz.
pub const PIT_MILLIHERTZ: u32 = 559_916;

/// The 560 Hz IRQ 0 tick counter as a `Clock`; the `Pacer` turns it into 60 Hz simulation steps with an integer remainder.
pub struct DosClock;

impl Clock for DosClock {
    fn now(&self) -> Ticks {
        Ticks(timer::ticks())
    }

    fn millihertz(&self) -> u32 {
        PIT_MILLIHERTZ
    }
}

/// Keyboard events: raw Set 1 scancodes from the IRQ 1 ring, decoded.
pub struct DosInput {
    decoder: Set1Decoder,
}

impl DosInput {
    pub const fn new() -> Self {
        DosInput {
            decoder: Set1Decoder::new(),
        }
    }
}

impl Default for DosInput {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSource for DosInput {
    fn poll_key(&mut self) -> Option<KeyEvent> {
        while let Some(byte) = kbd::pop_scancode() {
            if let Some(event) = self.decoder.feed(byte) {
                return Some(event);
            }
        }
        None
    }
}

/// Mode 13h only: 320x200, 256 colours.
pub const MODE_13H: ModeInfo = ModeInfo {
    id: 0x13,
    width: vga::WIDTH as u16,
    height: vga::HEIGHT as u16,
};

const MODES: [ModeInfo; 1] = [MODE_13H];

/// The VGA display in mode 13h.
pub struct DosDisplay;

impl Display for DosDisplay {
    fn modes(&self) -> &[ModeInfo] {
        &MODES
    }

    fn set_mode(&mut self, id: u16) -> Result<ModeInfo, HalError> {
        if id != MODE_13H.id {
            return Err(HalError::Unsupported);
        }
        vga::enter_mode13();
        Ok(MODE_13H)
    }

    fn restore_text_mode(&mut self) {
        vga::leave();
    }

    fn set_palette(&mut self, first: u8, colours: &[Rgb6]) -> Result<(), HalError> {
        if usize::from(first) + colours.len() > 256 || colours.iter().any(|c| c.r > 63 || c.g > 63 || c.b > 63) {
            return Err(HalError::InvalidArgument);
        }
        for (i, c) in colours.iter().enumerate() {
            vga::set_palette(first + i as u8, &[[c.r, c.g, c.b]]);
        }
        Ok(())
    }

    fn wait_vretrace(&self) -> Result<(), HalError> {
        vga::wait_vretrace();
        Ok(())
    }

    fn present(&mut self, pixels: &[u8], pitch: usize) -> Result<(), HalError> {
        if pitch != vga::WIDTH || pixels.len() < vga::PIXELS {
            return Err(HalError::InvalidArgument);
        }
        let frame: &[u8; vga::PIXELS] = pixels[..vga::PIXELS].try_into().map_err(|_| HalError::InvalidArgument)?;
        vga::present(frame);
        Ok(())
    }
}
