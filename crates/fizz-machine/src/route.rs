// The OPL write router: the one rule that lets an OPL2-format stream drive an OPL3 chip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// An OPL3 in its own mode powers up with channel outputs 0xC0-0xC8 routed to neither speaker unless bits 4 and 5 are set, and
// OPL2-format streams never set them. Setting both bits on every bank-0 write to those registers centres each voice, which is
// what a mono OPL2 stream means. An OPL3-aware stream (the OPL3 music arrangement, the effect pair) authors its own output bits to
// place each voice left, right or centre, so it passes through untouched: the rule is applied to a stream by its kind, never to the
// chip. Bank 1 carries only OPL3-aware writes and passes through untouched either way.

/// How the chip is being used for the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OplMode {
    /// Nothing uses the FM chip.
    None,
    /// The chip runs as an OPL2 (the power-up state of an OPL3).
    Opl2,
    /// The chip runs in OPL3 mode (register 0x105 bit 0 set).
    Opl3,
}

/// What format a register stream is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    /// An OPL2-format stream: bank 0 only, with no output bits in the C0-C8 writes. On an OPL3 in its own mode it needs the OR rule.
    Opl2,
    /// An OPL3-aware stream: it sets the C0-C8 output bits itself (stereo placement), so nothing is added.
    Opl3,
}

/// The value to write for `val` at `reg` in `bank` (0 or 1) from a `stream`: an OPL2-format stream's bank-0 write to registers 0xC0 to
/// 0xC8, in OPL3 mode, gets bits 4 and 5 set so the voice sounds in both speakers; every other write (an OPL3-aware stream's included,
/// whatever the mode), and every write in OPL2 or no-chip mode, is unchanged.
pub fn route(mode: OplMode, stream: Stream, bank: u8, reg: u8, val: u8) -> u8 {
    if mode == OplMode::Opl3 && stream == Stream::Opl2 && bank == 0 && (0xC0..=0xC8).contains(&reg)
    {
        val | 0x30
    } else {
        val
    }
}

/// The writes that put the chip in the state `mode` names, in the order they must be made before any other write: for OPL3 mode,
/// register 0x105 bit 0 (the OPL3 enable), then 0x104 = 0 (every channel two-operator). Nothing for the other modes. Bank-1
/// writes of an OPL3-aware stream are meaningless until the first has been made.
pub const fn enable_writes(mode: OplMode) -> &'static [(u8, u8, u8)] {
    match mode {
        OplMode::Opl3 => &[(1, 0x05, 0x01), (1, 0x04, 0x00)],
        OplMode::Opl2 | OplMode::None => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opl3_bank0_feedback_registers_get_both_outputs() {
        for reg in 0xC0..=0xC8 {
            assert_eq!(route(OplMode::Opl3, Stream::Opl2, 0, reg, 0x00), 0x30);
            assert_eq!(route(OplMode::Opl3, Stream::Opl2, 0, reg, 0x0F), 0x3F);
            assert_eq!(route(OplMode::Opl3, Stream::Opl2, 0, reg, 0x30), 0x30);
            assert_eq!(route(OplMode::Opl3, Stream::Opl2, 0, reg, 0xC5), 0xF5);
        }
    }

    #[test]
    fn neighbouring_registers_are_untouched() {
        for reg in [0xBF, 0xC9, 0xBD, 0xE0, 0x20, 0x01, 0x08] {
            assert_eq!(
                route(OplMode::Opl3, Stream::Opl2, 0, reg, 0x01),
                0x01,
                "reg {reg:#x}"
            );
        }
    }

    #[test]
    fn bank1_is_untouched() {
        for reg in 0..=255u8 {
            assert_eq!(route(OplMode::Opl3, Stream::Opl2, 1, reg, 0x5A), 0x5A);
        }
    }

    #[test]
    fn opl2_and_none_never_change_a_value() {
        for (mode, stream) in [OplMode::Opl2, OplMode::None]
            .into_iter()
            .flat_map(|m| [(m, Stream::Opl2), (m, Stream::Opl3)])
        {
            for bank in 0..2 {
                for reg in 0..=255u8 {
                    assert_eq!(route(mode, stream, bank, reg, 0x07), 0x07);
                }
            }
        }
    }

    #[test]
    fn only_the_two_routing_bits_ever_change() {
        for val in 0..=255u8 {
            let out = route(OplMode::Opl3, Stream::Opl2, 0, 0xC3, val);
            assert_eq!(out & !0x30, val & !0x30);
            assert_eq!(out & 0x30, 0x30);
        }
    }

    #[test]
    fn an_opl3_aware_stream_keeps_the_output_bits_it_authored() {
        for bank in 0..2 {
            for reg in 0..=255u8 {
                for val in [0x00, 0x10, 0x20, 0x30, 0x0F, 0xC5] {
                    assert_eq!(route(OplMode::Opl3, Stream::Opl3, bank, reg, val), val);
                }
            }
        }
    }

    #[test]
    fn opl2_music_on_an_opl3_chip_is_centred_and_opl3_music_is_not() {
        // /MUSIC:ADLIB /FX:SB on a Sound Blaster 16: the chip runs in OPL3 mode for the effect pair, the OPL2 music goes through the
        // OR rule, and the OPL3 arrangement's authored left, right and centre bits come out as written.
        for (authored, centred) in [
            (0x00, 0x30),
            (0x01, 0x31),
            (0x0e, 0x3e),
            (0x10, 0x30),
            (0x20, 0x30),
        ] {
            assert_eq!(
                route(OplMode::Opl3, Stream::Opl2, 0, 0xC2, authored),
                centred
            );
        }
        for authored in [0x10, 0x20, 0x30, 0x11, 0x2e] {
            assert_eq!(
                route(OplMode::Opl3, Stream::Opl3, 0, 0xC2, authored),
                authored
            );
        }
    }

    #[test]
    fn opl3_mode_is_enabled_first_and_only_then() {
        assert_eq!(enable_writes(OplMode::Opl3)[0], (1, 0x05, 0x01));
        assert!(enable_writes(OplMode::Opl2).is_empty() && enable_writes(OplMode::None).is_empty());
    }
}
