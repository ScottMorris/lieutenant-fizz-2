// The screen mix for the OPL music: an approximation of Fizz 1's "menu up, so the music goes quieter and duller" on a chip with no filter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Fizz 1's Web build closes a 900 Hz low-pass and drops the music to 0.7 gain (-3.1 dB) while Paused or Options is up. An OPL has no
// filter and no master volume for one group of channels, so this keeps a shadow of the level the music AUTHORED for every operator
// and sends the authored level plus an offset:
//
// - quieter: the carriers (the operators that reach the output) go up by 4 steps of 0.75 dB, 3.0 dB, the nearest whole number of
//   steps to 3.1 dB;
// - darker: the modulator of an FM (connection bit 0 clear) channel goes up by 6 steps, 4.5 dB. A modulator's level sets how much
//   it bends its carrier (the modulation index), and the index sets how much energy sits in the upper partials, so a weaker
//   modulator is a duller, rounder tone: the nearest an OPL gets to a low-pass. Halving the index (6 dB) would take a second
//   partial down about 12 dB and a third about 18 dB, which is steeper than a 900 Hz low-pass on a mid note, so 4.5 dB is the
//   deliberately gentler choice. It is a tuning constant to adjust by ear;
// - an additive channel (connection bit 0 set) has no modulator, both operators sound, and both take the carrier offset. In rhythm
//   mode the operators of channels 7 and 8 (hi-hat, snare, tom, cymbal) are each a voice, so they are treated the same way.
//
// Levels are 6 bits (0 to 63, 0.75 dB a step, larger is quieter), clamped at 63; bits 6 and 7 (the key scale level) are kept.
// The offset is applied in the music write path: `observe` takes each authored write, updates the shadow and returns the value to
// send, so a note started or a patch changed during the menu is not un-muffled. A change of mix moves a ramp variable, one unit
// at a time, and `step` sends the levels that differ from it (at most the number of writes the caller allows, round-robin), so a level
// moves one step at a time (never more than two when the writes lag) and a held note does not click. Channels 5 and 14 (the effects'),
// and everything but registers 0x40 to 0x55, are never touched. With the mix at `Play` and the ramp settled the offset is zero and
// `observe` returns each value unchanged.

use crate::Write;

/// Which mix is in force. The screen picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mix {
    /// Playing: the music as authored.
    Play,
    /// The Paused screen: quieter and duller.
    Pause,
    /// The Options screen: the same as `Pause`.
    Options,
    /// A card (Fizz 1's milder "cleared" mix, a half-depth version of `Pause`). Reserved: nothing selects it yet.
    Card,
}

impl Mix {
    /// How far into the ramp this mix sits, 0 to [`RAMP_MAX`].
    pub const fn depth(self) -> u8 {
        match self {
            Mix::Play => 0,
            Mix::Pause | Mix::Options => RAMP_MAX,
            Mix::Card => RAMP_MAX / 2,
        }
    }

    pub const fn to_u8(self) -> u8 {
        match self {
            Mix::Play => 0,
            Mix::Pause => 1,
            Mix::Options => 2,
            Mix::Card => 3,
        }
    }

    pub const fn from_u8(v: u8) -> Mix {
        match v {
            1 => Mix::Pause,
            2 => Mix::Options,
            3 => Mix::Card,
            _ => Mix::Play,
        }
    }
}

/// Ramp units from open to fully muffled.
pub const RAMP_MAX: u8 = 12;
/// Level steps (0.75 dB each) a carrier goes up at full depth: 3.0 dB.
pub const CARRIER_STEPS: u8 = 4;
/// Level steps a modulator of an FM channel goes up at full depth: 4.5 dB.
pub const MODULATOR_STEPS: u8 = 6;
/// Ticks between ramp units while closing and while opening: 12 units take 100 ticks (0.18 s at 560 Hz) and 199 ticks (0.36 s),
/// Fizz 1's 0.18 s and 0.35 s.
pub const ATTACK_PERIOD: u8 = 9;
pub const RELEASE_PERIOD: u8 = 18;
/// The most level writes `step` is asked for in a tick by the audio service.
pub const MAX_MIX_WRITES: usize = 2;

/// Operator slots in the shadow: 18 for each bank.
const SLOTS: usize = 36;
/// The channels (0 to 17, the second bank's from 9) the effects own.
const EFFECT_CHANNELS: [usize; 2] = [5, 14];

/// The carrier offset at ramp position `r`.
pub const fn carrier_offset(r: u8) -> u8 {
    r / (RAMP_MAX / CARRIER_STEPS)
}

/// The FM modulator offset at ramp position `r`.
pub const fn modulator_offset(r: u8) -> u8 {
    r / (RAMP_MAX / MODULATOR_STEPS)
}

/// The slot (0 to 17 in its bank) of the operator whose level register is `reg`, if it is one.
fn slot_of(reg: u8) -> Option<usize> {
    if !(0x40..=0x55).contains(&reg) || reg % 8 > 5 {
        return None;
    }
    let off = (reg - 0x40) as usize;
    Some((off / 8) * 6 + off % 8)
}

/// The level register of `slot` in its bank.
const fn reg_of(slot: usize) -> u8 {
    0x40 + ((slot / 6) * 8 + slot % 6) as u8
}

/// The channel (0 to 17) an operator slot belongs to and whether it is the carrier (the second operator).
const fn channel_of(bank_slot: usize) -> (usize, bool) {
    let (bank, s) = (bank_slot / 18, bank_slot % 18);
    (bank * 9 + (s / 6) * 3 + s % 3, s % 6 >= 3)
}

/// The shadow of the music's authored operator levels, the mix ramp and the levels last sent.
#[derive(Clone, Debug)]
pub struct MixState {
    target: u8,
    ramp: u8,
    wait: u8,
    /// Operators whose authored level has been seen (bit `bank * 18 + slot`).
    known: u64,
    authored: [u8; SLOTS],
    /// The offset in the level last sent for each operator.
    applied: [u8; SLOTS],
    /// The last level value sent for each operator, for the check.
    sent: [u8; SLOTS],
    /// Bit `ch` is the connection bit of channel `ch`'s last authored 0xC0 write.
    conn: u32,
    rhythm: bool,
    cursor: usize,
    /// Something may differ from the ramp: a scan is worth making.
    dirty: bool,
}

impl MixState {
    /// A state with the ramp already at `mix` (a new device after a switch takes the offset at once).
    pub const fn new(mix: Mix) -> MixState {
        MixState {
            target: mix.depth(),
            ramp: mix.depth(),
            wait: 0,
            known: 0,
            authored: [0; SLOTS],
            applied: [0; SLOTS],
            sent: [0; SLOTS],
            conn: 0,
            rhythm: false,
            cursor: 0,
            dirty: false,
        }
    }

    /// Chooses the mix to ramp to.
    pub fn set(&mut self, mix: Mix) {
        self.target = mix.depth();
    }

    /// Where the ramp is, 0 to [`RAMP_MAX`].
    pub fn ramp(&self) -> u8 {
        self.ramp
    }

    fn wanted(&self, bank_slot: usize) -> u8 {
        let (ch, carrier) = channel_of(bank_slot);
        let both = self.conn >> ch & 1 != 0 || (self.rhythm && (ch == 7 || ch == 8));
        if carrier || both {
            carrier_offset(self.ramp)
        } else {
            modulator_offset(self.ramp)
        }
    }

    fn level(authored: u8, offset: u8) -> u8 {
        (authored & 0xc0) | ((authored & 0x3f) + offset).min(63)
    }

    /// Takes one authored write of the music and returns the value to send for it: the authored value, with the current offset on an
    /// operator level. Always call it for every write of the music, in order.
    pub fn observe(&mut self, w: Write) -> u8 {
        if w.bank == 0 && w.reg == 0xbd {
            if self.rhythm != (w.val & 0x20 != 0) {
                self.rhythm = !self.rhythm;
                self.dirty = true;
            }
        } else if (0xc0..=0xc8).contains(&w.reg) && w.bank <= 1 {
            let ch = usize::from(w.bank) * 9 + usize::from(w.reg - 0xc0);
            if !EFFECT_CHANNELS.contains(&ch) {
                let bit = u32::from(w.val & 1) << ch;
                if self.conn & (1 << ch) != bit {
                    self.conn = self.conn & !(1 << ch) | bit;
                    self.dirty = true;
                }
            }
        } else if let Some(s) = slot_of(w.reg)
            && w.bank <= 1
        {
            let i = usize::from(w.bank) * 18 + s;
            if !EFFECT_CHANNELS.contains(&channel_of(i).0) {
                let offset = self.wanted(i);
                self.known |= 1 << i;
                self.authored[i] = w.val;
                self.applied[i] = offset;
                self.sent[i] = Self::level(w.val, offset);
                return self.sent[i];
            }
        }
        w.val
    }

    /// Moves the ramp one unit if it is due, then writes into `out` the levels that differ from it, round-robin from where the last
    /// call stopped, and returns how many. `out` may be empty: the ramp still moves and the writes wait.
    pub fn step(&mut self, out: &mut [Write]) -> usize {
        if self.ramp == self.target {
            self.wait = 0;
        } else {
            if self.wait > 0 {
                self.wait -= 1;
            } else {
                if self.ramp < self.target {
                    self.ramp += 1;
                    self.wait = ATTACK_PERIOD - 1;
                } else {
                    self.ramp -= 1;
                    self.wait = RELEASE_PERIOD - 1;
                }
                self.dirty = true;
            }
        }
        if !self.dirty {
            return 0;
        }
        let mut n = 0;
        let start = self.cursor;
        for k in 0..SLOTS {
            let i = (start + k) % SLOTS;
            if self.known >> i & 1 == 0 {
                continue;
            }
            let offset = self.wanted(i);
            if offset == self.applied[i] {
                continue;
            }
            if n == out.len() {
                return n;
            }
            self.applied[i] = offset;
            self.sent[i] = Self::level(self.authored[i], offset);
            out[n] = Write {
                bank: (i / 18) as u8,
                reg: reg_of(i % 18),
                val: self.sent[i],
            };
            n += 1;
            self.cursor = (i + 1) % SLOTS;
        }
        // A whole pass found nothing more to send.
        self.dirty = false;
        n
    }

    /// For the check: the operators seen, those sent with an offset, those whose last value sent differs from what the ramp now asks
    /// for, and the largest offset sent to a carrier and to an FM modulator.
    pub fn audit(&self) -> Audit {
        let mut a = Audit::default();
        for i in 0..SLOTS {
            if self.known >> i & 1 == 0 {
                continue;
            }
            a.ops += 1;
            if self.applied[i] != 0 {
                a.muted += 1;
            }
            if self.sent[i] != Self::level(self.authored[i], self.wanted(i)) {
                a.mismatch += 1;
            }
            let (ch, carrier) = channel_of(i);
            let both = self.conn >> ch & 1 != 0 || (self.rhythm && (ch == 7 || ch == 8));
            if carrier || both {
                a.max_carrier = a.max_carrier.max(self.applied[i]);
            } else {
                a.max_modulator = a.max_modulator.max(self.applied[i]);
            }
        }
        a
    }
}

/// What [`MixState::audit`] counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Audit {
    pub ops: u32,
    pub muted: u32,
    pub mismatch: u32,
    pub max_carrier: u8,
    pub max_modulator: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(bank: u8, reg: u8, val: u8) -> Write {
        Write { bank, reg, val }
    }

    fn run(m: &mut MixState, ticks: u32) -> (u32, usize) {
        let (mut total, mut most) = (0, 0);
        for _ in 0..ticks {
            let mut out = [Write::default(); MAX_MIX_WRITES];
            let n = m.step(&mut out);
            total += n as u32;
            most = most.max(n);
        }
        (total, most)
    }

    #[test]
    fn registers_map_to_slots_and_back() {
        let mut seen = 0u32;
        for reg in 0x40..=0x55u8 {
            if let Some(s) = slot_of(reg) {
                assert_eq!(reg_of(s), reg);
                seen |= 1 << s;
            } else {
                assert!(reg % 8 > 5);
            }
        }
        assert_eq!(seen, (1 << 18) - 1);
        // Channel 0 is slots 0 (modulator) and 3 (carrier); channel 3 starts at slot 6; channel 8 is slots 14 and 17.
        assert_eq!(channel_of(0), (0, false));
        assert_eq!(channel_of(3), (0, true));
        assert_eq!(channel_of(6), (3, false));
        assert_eq!(channel_of(17), (8, true));
        assert_eq!(channel_of(18 + 1), (9 + 1, false));
    }

    #[test]
    fn play_is_a_pass_through() {
        let mut m = MixState::new(Mix::Play);
        for reg in 0..=0xff {
            for val in [0u8, 1, 0x3f, 0x40, 0x7f, 0xc5, 0xff] {
                assert_eq!(m.observe(w(0, reg, val)), val);
                assert_eq!(m.observe(w(1, reg, val)), val);
            }
        }
        assert_eq!(m.step(&mut [Write::default(); 2]), 0);
    }

    #[test]
    fn the_offsets_at_full_depth() {
        assert_eq!((carrier_offset(0), modulator_offset(0)), (0, 0));
        assert_eq!(
            (carrier_offset(RAMP_MAX), modulator_offset(RAMP_MAX)),
            (CARRIER_STEPS, MODULATOR_STEPS)
        );
        // One ramp unit never moves a level more than one step.
        for r in 1..=RAMP_MAX {
            assert!(carrier_offset(r) - carrier_offset(r - 1) <= 1);
            assert!(modulator_offset(r) - modulator_offset(r - 1) <= 1);
        }
    }

    #[test]
    fn a_new_state_at_pause_applies_at_once_and_keeps_the_key_scale_bits() {
        let mut m = MixState::new(Mix::Pause);
        m.observe(w(0, 0xc0, 0x04)); // FM
        // Channel 0: modulator 0x20 (slot 0), carrier 0x23 (slot 3).
        assert_eq!(m.observe(w(0, 0x40, 0x4a)), 0x40 | (0x0a + 6));
        assert_eq!(m.observe(w(0, 0x43, 0xc5)), 0xc0 | (0x05 + 4));
    }

    #[test]
    fn levels_clamp_at_63() {
        let mut m = MixState::new(Mix::Pause);
        m.observe(w(0, 0xc0, 0));
        assert_eq!(m.observe(w(0, 0x40, 0x3f)), 0x3f);
        assert_eq!(m.observe(w(0, 0x43, 0xbe)), 0x80 | 0x3f);
        assert_eq!(m.observe(w(0, 0x43, 0x3d)), 0x3f);
    }

    #[test]
    fn additive_channels_and_rhythm_voices_duck_both_operators() {
        let mut m = MixState::new(Mix::Pause);
        m.observe(w(0, 0xc1, 0x01)); // channel 1 additive
        assert_eq!(m.observe(w(0, 0x41, 0x10)), 0x10 + 4);
        assert_eq!(m.observe(w(0, 0x44, 0x10)), 0x10 + 4);
        m.observe(w(0, 0xc2, 0x00)); // channel 2 FM
        assert_eq!(m.observe(w(0, 0x42, 0x10)), 0x10 + 6);
        assert_eq!(m.observe(w(0, 0x45, 0x10)), 0x10 + 4);
    }

    #[test]
    fn rhythm_mode_ducks_both_operators_of_channels_7_and_8() {
        let mut m = MixState::new(Mix::Pause);
        m.observe(w(0, 0xbd, 0x20));
        m.observe(w(0, 0xc7, 0x00));
        // Channel 7: modulator slot 12 + 1, carrier slot 12 + 4.
        let (md, ca) = (reg_of(12 + 1), reg_of(12 + 4));
        assert_eq!(channel_of(12 + 1).0, 7);
        assert_eq!(m.observe(w(0, md, 0x08)), 0x08 + 4);
        assert_eq!(m.observe(w(0, ca, 0x08)), 0x08 + 4);
    }

    #[test]
    fn the_effect_channels_and_other_registers_are_untouched() {
        let mut m = MixState::new(Mix::Pause);
        // Channel 5: slots 8 and 11 of bank 0 are registers 0x4a and 0x4d... through the table.
        let (md, ca) = (reg_of(6 + 2), reg_of(9 + 2));
        assert_eq!(channel_of(6 + 2).0, 5);
        assert_eq!(m.observe(w(0, md, 0x10)), 0x10);
        assert_eq!(m.observe(w(0, ca, 0x10)), 0x10);
        // Channel 14 is the second bank's sixth channel.
        assert_eq!(channel_of(18 + 6 + 2).0, 14);
        assert_eq!(m.observe(w(1, md, 0x10)), 0x10);
        assert_eq!(m.observe(w(1, ca, 0x10)), 0x10);
        for (bank, reg) in [
            (0, 0x20),
            (0, 0x60),
            (0, 0x80),
            (0, 0xe0),
            (1, 0x04),
            (1, 0x05),
            (0, 0x56),
            (0, 0x46),
            (0, 0xb3),
            (0, 0xc5),
        ] {
            assert_eq!(m.observe(w(bank, reg, 0x11)), 0x11, "{bank} {reg:#x}");
        }
        assert_eq!(m.step(&mut [Write::default(); 8]), 0);
    }

    #[test]
    fn the_ramp_closes_in_100_ticks_and_opens_in_199_and_the_writes_follow() {
        let mut m = MixState::new(Mix::Play);
        // Eight FM channels (an OPL2 chip's music) at level 0x10.
        for ch in 0u8..8 {
            if ch == 5 {
                continue;
            }
            m.observe(w(0, 0xc0 + ch, 0));
            let (a, b) = (
                0x40 + [0, 1, 2, 8, 9, 10, 16, 17, 18][usize::from(ch)],
                0x43 + [0, 1, 2, 8, 9, 10, 16, 17, 18][usize::from(ch)],
            );
            m.observe(w(0, a, 0x10));
            m.observe(w(0, b, 0x10));
        }
        m.set(Mix::Pause);
        let mut at = None;
        let mut most = 0;
        for t in 0..200 {
            let mut out = [Write::default(); MAX_MIX_WRITES];
            most = most.max(m.step(&mut out));
            if at.is_none() && m.ramp() == RAMP_MAX {
                at = Some(t + 1);
            }
        }
        assert_eq!(at, Some(1 + 11 * u32::from(ATTACK_PERIOD)));
        assert!(most <= MAX_MIX_WRITES);
        let a = m.audit();
        assert_eq!(
            (a.ops, a.muted, a.mismatch, a.max_carrier, a.max_modulator),
            (14, 14, 0, 4, 6)
        );
        m.set(Mix::Play);
        let mut opened = None;
        for t in 0..300 {
            m.step(&mut [Write::default(); MAX_MIX_WRITES]);
            if opened.is_none() && m.ramp() == 0 {
                opened = Some(t + 1);
            }
        }
        assert_eq!(opened, Some(1 + 11 * u32::from(RELEASE_PERIOD)));
        let a = m.audit();
        assert_eq!((a.muted, a.mismatch), (0, 0));
    }

    #[test]
    fn no_level_moves_more_than_two_steps_in_one_write() {
        let mut m = MixState::new(Mix::Play);
        for ch in 0u8..3 {
            m.observe(w(0, 0xc0 + ch, 0));
            m.observe(w(0, 0x40 + ch, 0x10));
            m.observe(w(0, 0x43 + ch, 0x10));
        }
        m.set(Mix::Pause);
        let mut last = [0x10u8; 36];
        for _ in 0..300 {
            let mut out = [Write::default(); MAX_MIX_WRITES];
            let n = m.step(&mut out);
            for o in &out[..n] {
                let i = usize::from(o.bank) * 18 + slot_of(o.reg).unwrap();
                assert!(o.val.abs_diff(last[i]) <= 2, "{} -> {}", last[i], o.val);
                last[i] = o.val;
            }
        }
    }

    #[test]
    fn a_note_started_in_the_menu_is_not_unmuffled_and_a_write_budget_of_zero_defers() {
        let mut m = MixState::new(Mix::Play);
        m.observe(w(0, 0xc0, 0));
        m.observe(w(0, 0x40, 0x10));
        m.observe(w(0, 0x43, 0x10));
        m.set(Mix::Pause);
        // No writes allowed: the ramp still moves, nothing is sent, and the audit shows the lag.
        for _ in 0..120 {
            assert_eq!(m.step(&mut []), 0);
        }
        assert_eq!(m.ramp(), RAMP_MAX);
        assert_eq!(m.audit().mismatch, 2);
        // A new authored level goes out already offset.
        assert_eq!(m.observe(w(0, 0x43, 0x20)), 0x24);
        // The rest catches up.
        run(&mut m, 4);
        assert_eq!(m.audit().mismatch, 0);
    }

    #[test]
    fn a_connection_change_re_levels_the_modulator() {
        let mut m = MixState::new(Mix::Pause);
        m.observe(w(0, 0xc0, 0));
        m.observe(w(0, 0x40, 0x10));
        assert_eq!(m.audit().max_modulator, 6);
        m.observe(w(0, 0xc0, 1));
        let (n, _) = run(&mut m, 4);
        assert_eq!(n, 1);
        assert_eq!(m.audit().max_modulator, 0);
        assert_eq!(m.audit().mismatch, 0);
    }

    #[test]
    fn the_card_mix_is_half_depth() {
        assert_eq!(Mix::from_u8(Mix::Card.to_u8()), Mix::Card);
        let m = MixState::new(Mix::Card);
        assert_eq!(
            (carrier_offset(m.ramp()), modulator_offset(m.ramp())),
            (2, 3)
        );
    }
}
