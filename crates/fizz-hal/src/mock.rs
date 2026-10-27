// Mocks: a settable clock, a queued input source and a recording display for host tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Behind the `mock` feature. These use `alloc` only, so the crate stays `no_std`.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::Cell;

use crate::input::{InputSource, KeyEvent};
use crate::time::{Clock, Ticks};
use crate::video::{Display, HalError, ModeInfo, Rgb6};

/// A clock that only moves when told to. Uses interior mutability so a `Pacer` can borrow it while a test advances it.
#[derive(Debug)]
pub struct MockClock {
    now: Cell<u32>,
    millihertz: u32,
}

impl MockClock {
    /// A clock at tick zero running at `millihertz`.
    pub const fn new(millihertz: u32) -> Self {
        MockClock {
            now: Cell::new(0),
            millihertz,
        }
    }

    /// Sets the reading outright, for example just below the wrap.
    pub fn set(&self, ticks: u32) {
        self.now.set(ticks);
    }

    /// Moves forward by `ticks`, wrapping at 32 bits.
    pub fn advance(&self, ticks: u32) {
        self.now.set(self.now.get().wrapping_add(ticks));
    }
}

impl Clock for MockClock {
    fn now(&self) -> Ticks {
        Ticks(self.now.get())
    }

    fn millihertz(&self) -> u32 {
        self.millihertz
    }
}

/// An input source fed from a queue the test fills.
#[derive(Debug, Default)]
pub struct MockInput {
    queue: VecDeque<KeyEvent>,
}

impl MockInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues an event behind the ones already waiting.
    pub fn push(&mut self, event: KeyEvent) {
        self.queue.push_back(event);
    }

    /// Events not yet polled.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

impl InputSource for MockInput {
    fn poll_key(&mut self) -> Option<KeyEvent> {
        self.queue.pop_front()
    }
}

/// The only mode the mock offers: BIOS mode 0x13.
pub const MODE_13H: ModeInfo = ModeInfo {
    id: 0x13,
    width: 320,
    height: 200,
};

const MODES: [ModeInfo; 1] = [MODE_13H];

/// A display that records what it is given. It offers one mode, 0x13 (320x200).
#[derive(Debug)]
pub struct MockDisplay {
    mode: Option<ModeInfo>,
    palette: [Rgb6; 256],
    frame: Vec<u8>,
    presents: u32,
    text_restores: u32,
}

impl Default for MockDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDisplay {
    pub fn new() -> Self {
        MockDisplay {
            mode: None,
            palette: [Rgb6::default(); 256],
            frame: Vec::new(),
            presents: 0,
            text_restores: 0,
        }
    }

    /// The mode currently set, or `None` before `set_mode` and after `restore_text_mode`.
    pub fn mode(&self) -> Option<ModeInfo> {
        self.mode
    }

    /// The 256-entry palette as last set (black until written).
    pub fn palette(&self) -> &[Rgb6; 256] {
        &self.palette
    }

    /// The last presented frame with the row padding removed: `width * height` bytes, or empty before the first `present`.
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    /// How many frames have been presented.
    pub fn presents(&self) -> u32 {
        self.presents
    }

    /// How many times `restore_text_mode` was called.
    pub fn text_restores(&self) -> u32 {
        self.text_restores
    }

    /// The 64-bit FNV-1a hash of the last presented frame, for pinning a frame in a test. An empty frame hashes to the FNV offset
    /// basis.
    pub fn checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in &self.frame {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

impl Display for MockDisplay {
    fn modes(&self) -> &[ModeInfo] {
        &MODES
    }

    fn set_mode(&mut self, id: u16) -> Result<ModeInfo, HalError> {
        let mode = MODES
            .iter()
            .copied()
            .find(|m| m.id == id)
            .ok_or(HalError::Unsupported)?;
        self.mode = Some(mode);
        Ok(mode)
    }

    fn restore_text_mode(&mut self) {
        self.mode = None;
        self.text_restores += 1;
    }

    fn set_palette(&mut self, first: u8, colours: &[Rgb6]) -> Result<(), HalError> {
        let first = usize::from(first);
        if first + colours.len() > self.palette.len()
            || colours.iter().any(|c| c.r > 63 || c.g > 63 || c.b > 63)
        {
            return Err(HalError::InvalidArgument);
        }
        self.palette[first..first + colours.len()].copy_from_slice(colours);
        Ok(())
    }

    fn wait_vretrace(&self) -> Result<(), HalError> {
        Ok(())
    }

    fn present(&mut self, pixels: &[u8], pitch: usize) -> Result<(), HalError> {
        let mode = self.mode.ok_or(HalError::Device)?;
        let (w, h) = (usize::from(mode.width), usize::from(mode.height));
        if pitch < w || pixels.len() < pitch * (h - 1) + w {
            return Err(HalError::InvalidArgument);
        }
        self.frame.clear();
        for row in 0..h {
            self.frame
                .extend_from_slice(&pixels[row * pitch..row * pitch + w]);
        }
        self.presents += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyCode;
    use crate::time::Pacer;
    use alloc::vec;

    #[test]
    fn the_clock_sets_and_advances_with_wrap() {
        let c = MockClock::new(60_000);
        assert_eq!(c.now(), Ticks(0));
        assert_eq!(c.millihertz(), 60_000);
        c.set(u32::MAX);
        c.advance(2);
        assert_eq!(c.now(), Ticks(1));
    }

    #[test]
    fn the_clock_drives_a_pacer() {
        let c = MockClock::new(1_000_000);
        let mut p = Pacer::new(&c, 60, 4);
        c.advance(100);
        assert_eq!(p.due(&c), 4);
    }

    #[test]
    fn input_replays_events_in_order() {
        let mut i = MockInput::new();
        assert!(i.is_empty());
        assert_eq!(i.poll_key(), None);
        let a = KeyEvent {
            code: KeyCode::W,
            pressed: true,
        };
        let b = KeyEvent {
            code: KeyCode::W,
            pressed: false,
        };
        i.push(a);
        i.push(b);
        assert_eq!(i.len(), 2);
        assert_eq!(
            (i.poll_key(), i.poll_key(), i.poll_key()),
            (Some(a), Some(b), None)
        );
    }

    #[test]
    fn the_display_offers_mode_13h_only() {
        let mut d = MockDisplay::new();
        assert_eq!(d.modes(), &[MODE_13H]);
        assert_eq!(d.set_mode(0x12), Err(HalError::Unsupported));
        assert_eq!(d.mode(), None);
        assert_eq!(d.set_mode(0x13), Ok(MODE_13H));
        assert_eq!(d.mode(), Some(MODE_13H));
        d.restore_text_mode();
        assert_eq!((d.mode(), d.text_restores()), (None, 1));
    }

    #[test]
    fn the_palette_is_recorded_and_checked() {
        let mut d = MockDisplay::new();
        let red = Rgb6 { r: 63, g: 0, b: 0 };
        d.set_palette(255, &[red]).unwrap();
        assert_eq!(d.palette()[255], red);
        assert_eq!(
            d.set_palette(255, &[red, red]),
            Err(HalError::InvalidArgument)
        );
        assert_eq!(
            d.set_palette(0, &[Rgb6 { r: 64, g: 0, b: 0 }]),
            Err(HalError::InvalidArgument)
        );
        assert!(d.wait_vretrace().is_ok());
    }

    #[test]
    fn present_records_the_frame_without_padding() {
        let mut d = MockDisplay::new();
        assert_eq!(d.present(&[0; 64_000], 320), Err(HalError::Device));
        d.set_mode(0x13).unwrap();
        let empty = d.checksum();
        assert_eq!(empty, 0xcbf2_9ce4_8422_2325);
        // A pitch of 324 with padding bytes set to 0xEE that must not appear in the recorded frame.
        let mut buf = vec![0xEE; 324 * 200];
        for y in 0..200 {
            for x in 0..320 {
                buf[y * 324 + x] = (x ^ y) as u8;
            }
        }
        d.present(&buf, 324).unwrap();
        assert_eq!(d.presents(), 1);
        assert_eq!(d.frame().len(), 64_000);
        assert_eq!(d.frame()[321], 1 ^ 1);
        assert_eq!(d.frame()[2 * 320 + 5], 5 ^ 2);
        assert_eq!(d.frame()[319], 319_u32 as u8);
        assert_eq!(d.frame()[320], 1);
        let first = d.checksum();
        assert_ne!(first, empty);
        d.present(&buf, 324).unwrap();
        assert_eq!(d.checksum(), first);
        assert_eq!(d.present(&buf[..1000], 324), Err(HalError::InvalidArgument));
        assert_eq!(d.present(&buf, 300), Err(HalError::InvalidArgument));
    }
}
