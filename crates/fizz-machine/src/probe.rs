// The probe model: plain data describing what a machine has, and the parser for the `BLASTER` environment variable.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The DOS runtime fills in a `Probe` from hardware; nothing here talks to hardware. The methods that find each value are
// listed in `docs/DOS-CHECK.md`.

/// The best display adapter found. MCGA is not taken as VGA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoClass {
    Vga,
    /// A VGA driving a monochrome display.
    MonoVga,
    Ega,
    Cga,
    /// A monochrome text adapter, MDA or Hercules.
    MdaHercules,
}

impl VideoClass {
    /// Whether the game's 320x200 256-colour mode is available.
    pub fn has_vga(self) -> bool {
        matches!(self, VideoClass::Vga | VideoClass::MonoVga)
    }

    /// Whether the text screen lives at 0xB0000 in mode 7, with attributes that degrade to normal, bright and inverse.
    pub fn is_text_mono(self) -> bool {
        self == VideoClass::MdaHercules
    }
}

/// What answered at the FM chip's ports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FmKind {
    None,
    /// One OPL2 (YM3812) at 0x388: an AdLib, a Sound Blaster 1 or 2.
    Opl2,
    /// Two OPL2 chips, a Sound Blaster Pro 1. The game uses it as one OPL2, mono.
    DualOpl2,
    /// An OPL3 (YMF262): a Sound Blaster Pro 2 or 16.
    Opl3,
}

/// A Sound Blaster's DSP. The version is a label only: the chip type comes from `Probe::fm`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundBlaster {
    pub dsp_major: u8,
    pub dsp_minor: u8,
    /// The base port that answered the DSP reset, such as 0x220.
    pub base: u16,
}

/// The fields of the `BLASTER` environment variable that were present and valid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blaster {
    /// `A`: the base port, in hexadecimal.
    pub base: Option<u16>,
    /// `I`: the interrupt, in decimal.
    pub irq: Option<u8>,
    /// `D`: the 8-bit DMA channel.
    pub dma: Option<u8>,
    /// `H`: the 16-bit DMA channel.
    pub hdma: Option<u8>,
    /// `P`: the MPU-401 port, in hexadecimal.
    pub mpu: Option<u16>,
    /// `T`: the card type code.
    pub card: Option<u8>,
}

/// The processor family, from the EFLAGS bits that toggle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuClass {
    I386,
    I486,
    /// A Pentium or later (the ID flag toggles).
    I586Plus,
}

/// Everything the check screen and the resolver need to know about the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub video: VideoClass,
    pub mouse: bool,
    pub joystick: bool,
    pub fm: FmKind,
    /// The Sound Blaster DSP, if one answered.
    pub sb: Option<SoundBlaster>,
    pub blaster: Blaster,
    pub x87: bool,
    pub cpu: CpuClass,
    /// Free physical memory in kilobytes.
    pub free_kb: u32,
}

impl Probe {
    /// A bare machine: a colour VGA, a 486 with an x87 and 8 MB free, no mouse, joystick or sound but the speaker.
    pub const BARE: Probe = Probe {
        video: VideoClass::Vga,
        mouse: false,
        joystick: false,
        fm: FmKind::None,
        sb: None,
        blaster: Blaster {
            base: None,
            irq: None,
            dma: None,
            hdma: None,
            mpu: None,
            card: None,
        },
        x87: true,
        cpu: CpuClass::I486,
        free_kb: 8 * 1024,
    };
}

/// Reads a run of digits; a bad digit, an empty run or an overflow gives `None`.
fn number(s: &str, radix: u32) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut n: u32 = 0;
    for c in s.chars() {
        n = n.checked_mul(radix)?.checked_add(c.to_digit(radix)?)?;
    }
    Some(n)
}

/// Parses a `BLASTER` value such as `A220 I7 D1 H5 T6 P330`. Letters are case-insensitive; fields may come in any order,
/// be missing or repeat (the last valid one wins); a field with a bad value, and any unknown field, is ignored.
pub fn parse_blaster(text: &str) -> Blaster {
    let mut out = Blaster::default();
    for token in text.split_ascii_whitespace() {
        let mut chars = token.chars();
        let Some(letter) = chars.next() else { continue };
        let rest = chars.as_str();
        match letter.to_ascii_uppercase() {
            'A' => {
                out.base = number(rest, 16)
                    .and_then(|n| u16::try_from(n).ok())
                    .or(out.base)
            }
            'P' => {
                out.mpu = number(rest, 16)
                    .and_then(|n| u16::try_from(n).ok())
                    .or(out.mpu)
            }
            'I' => {
                out.irq = number(rest, 10)
                    .and_then(|n| u8::try_from(n).ok())
                    .or(out.irq)
            }
            'D' => {
                out.dma = number(rest, 10)
                    .and_then(|n| u8::try_from(n).ok())
                    .or(out.dma)
            }
            'H' => {
                out.hdma = number(rest, 10)
                    .and_then(|n| u8::try_from(n).ok())
                    .or(out.hdma)
            }
            'T' => {
                out.card = number(rest, 10)
                    .and_then(|n| u8::try_from(n).ok())
                    .or(out.card)
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_string() {
        let b = parse_blaster("A220 I7 D1 H5 T6");
        assert_eq!(b.base, Some(0x220));
        assert_eq!(b.irq, Some(7));
        assert_eq!(b.dma, Some(1));
        assert_eq!(b.hdma, Some(5));
        assert_eq!(b.card, Some(6));
        assert_eq!(b.mpu, None);
    }

    #[test]
    fn lower_case_and_mpu() {
        let b = parse_blaster("a240 i5 d3 p330 t4");
        assert_eq!(
            (b.base, b.irq, b.dma, b.mpu, b.card),
            (Some(0x240), Some(5), Some(3), Some(0x330), Some(4))
        );
        assert_eq!(b.hdma, None);
    }

    #[test]
    fn missing_and_extra_fields() {
        assert_eq!(parse_blaster(""), Blaster::default());
        assert_eq!(parse_blaster("   "), Blaster::default());
        let b = parse_blaster("  T3   A220  E620 X9 ");
        assert_eq!((b.base, b.card), (Some(0x220), Some(3)));
        assert_eq!((b.irq, b.dma, b.hdma, b.mpu), (None, None, None, None));
    }

    #[test]
    fn bad_values_are_ignored() {
        let b = parse_blaster("A I7 Dx Hz T999 AG20");
        assert_eq!(b.base, None);
        assert_eq!(b.irq, Some(7));
        assert_eq!((b.dma, b.hdma, b.card), (None, None, None));
    }

    #[test]
    fn last_valid_field_wins() {
        assert_eq!(parse_blaster("A220 A240").base, Some(0x240));
        assert_eq!(parse_blaster("A220 Azz").base, Some(0x220));
    }

    #[test]
    fn overflow_is_ignored() {
        assert_eq!(parse_blaster("A123456789").base, None);
        assert_eq!(parse_blaster("A12345").base, None);
        assert_eq!(parse_blaster("I256").irq, None);
    }

    #[test]
    fn video_class_helpers() {
        assert!(VideoClass::Vga.has_vga() && VideoClass::MonoVga.has_vga());
        assert!(
            !VideoClass::Ega.has_vga()
                && !VideoClass::Cga.has_vga()
                && !VideoClass::MdaHercules.has_vga()
        );
        assert!(VideoClass::MdaHercules.is_text_mono() && !VideoClass::MonoVga.is_text_mono());
    }
}
