// Video: DAC colours, mode descriptions, the error type and the `Display` trait a presenter draws through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Deliberate deviation from Desk: Desk's `Display` hands out a `FrameBuffer` over an `MmioRegion`, which needs `unsafe` in the
// shared crate. Here the presenter renders into its own indexed back buffer and calls `present(pixels, pitch)`; the platform
// implementation copies the rows to the screen and holds the only `unsafe`. The HAL itself contains none.

/// A colour as the VGA DAC holds it: six bits per channel, 0 to 63.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb6 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// A video mode a display offers. `id` is the platform's mode number (BIOS mode 0x13 is 320x200, 256 colours).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub id: u16,
    pub width: u16,
    pub height: u16,
}

/// Why a display call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalError {
    /// The platform cannot do this.
    Unsupported,
    /// An argument was out of range or the wrong size.
    InvalidArgument,
    /// The device reported a fault.
    Device,
}

/// An indexed-colour display.
pub trait Display {
    /// The modes this display can enter.
    fn modes(&self) -> &[ModeInfo];
    /// Enters the mode with the given id and returns its description.
    fn set_mode(&mut self, id: u16) -> Result<ModeInfo, HalError>;
    /// Returns to the text mode that was active before `set_mode`. Safe to call when no mode was set.
    fn restore_text_mode(&mut self);
    /// Loads `colours` into the palette starting at index `first`.
    fn set_palette(&mut self, first: u8, colours: &[Rgb6]) -> Result<(), HalError>;
    /// Waits for the start of the vertical retrace, where the platform supports it.
    fn wait_vretrace(&self) -> Result<(), HalError>;
    /// Copies an indexed frame to the screen. `pitch` is the number of bytes from one row to the next in `pixels`.
    fn present(&mut self, pixels: &[u8], pitch: usize) -> Result<(), HalError>;
}
