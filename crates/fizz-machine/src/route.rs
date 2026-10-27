// The OPL write router: the one rule that lets an OPL2-format stream drive an OPL3 chip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// An OPL3 in its own mode powers up with channel outputs 0xC0-0xC8 routed to neither speaker unless bits 4 and 5 are set, and
// OPL2-format music never sets them. Setting both bits on every bank-0 write to those registers centres each voice, which is
// what a mono OPL2 stream means. Bank 1 carries only OPL3-aware writes (the effect channel 14) and passes through untouched.

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

/// The value to write for `val` at `reg` in `bank` (0 or 1): in OPL3 mode a bank-0 write to registers 0xC0 to 0xC8 gets bits 4
/// and 5 set so the voice sounds in both speakers; every other write, and every write in OPL2 or no-chip mode, is unchanged.
pub fn route(mode: OplMode, bank: u8, reg: u8, val: u8) -> u8 {
    if mode == OplMode::Opl3 && bank == 0 && (0xC0..=0xC8).contains(&reg) {
        val | 0x30
    } else {
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opl3_bank0_feedback_registers_get_both_outputs() {
        for reg in 0xC0..=0xC8 {
            assert_eq!(route(OplMode::Opl3, 0, reg, 0x00), 0x30);
            assert_eq!(route(OplMode::Opl3, 0, reg, 0x0F), 0x3F);
            assert_eq!(route(OplMode::Opl3, 0, reg, 0x30), 0x30);
            assert_eq!(route(OplMode::Opl3, 0, reg, 0xC5), 0xF5);
        }
    }

    #[test]
    fn neighbouring_registers_are_untouched() {
        for reg in [0xBF, 0xC9, 0xBD, 0xE0, 0x20, 0x01, 0x08] {
            assert_eq!(route(OplMode::Opl3, 0, reg, 0x01), 0x01, "reg {reg:#x}");
        }
    }

    #[test]
    fn bank1_is_untouched() {
        for reg in 0..=255u8 {
            assert_eq!(route(OplMode::Opl3, 1, reg, 0x5A), 0x5A);
        }
    }

    #[test]
    fn opl2_and_none_never_change_a_value() {
        for mode in [OplMode::Opl2, OplMode::None] {
            for bank in 0..2 {
                for reg in 0..=255u8 {
                    assert_eq!(route(mode, bank, reg, 0x07), 0x07);
                }
            }
        }
    }

    #[test]
    fn only_the_two_routing_bits_ever_change() {
        for val in 0..=255u8 {
            let out = route(OplMode::Opl3, 0, 0xC3, val);
            assert_eq!(out & !0x30, val & !0x30);
            assert_eq!(out & 0x30, 0x30);
        }
    }
}
