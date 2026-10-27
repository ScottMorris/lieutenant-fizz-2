// Provides the fixed-tick clock and state hash that every Fizz presentation must agree on.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![forbid(unsafe_code)]

mod input;

pub use input::{Buttons, InputFrame, InputLatch};

/// Simulation steps per second. Presenters call the rules this often and never faster.
pub const TICK_HZ: u32 = 60;

/// A count of completed simulation steps since a level or game began.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tick(pub u64);

impl Tick {
    pub const ZERO: Self = Self(0);

    /// Returns the tick after this one.
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// A 64-bit FNV-1a digest of the state that feeds back into the rules.
///
/// Native, Web and DOS builds hash the same fields in the same order after each tick, so a
/// disagreement between their digests means the simulations diverged. Floating-point values are
/// hashed by their IEEE bit pattern, which makes any difference in rounding visible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateHash(u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl StateHash {
    pub const fn new() -> Self {
        Self(FNV_OFFSET)
    }

    pub fn write_u8(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    pub fn write_u32(&mut self, value: u32) {
        for byte in value.to_le_bytes() {
            self.write_u8(byte);
        }
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_u32(value as u32);
    }

    pub fn write_u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.write_u8(byte);
        }
    }

    /// Hashes the exact bit pattern of a double, so a one-ulp difference changes the digest.
    pub fn write_f64(&mut self, value: f64) {
        self.write_u64(value.to_bits());
    }

    pub const fn finish(self) -> u64 {
        self.0
    }
}

impl Default for StateHash {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_count_up_from_zero() {
        assert_eq!(Tick::ZERO.next(), Tick(1));
        assert!(Tick(2) > Tick(1));
    }

    #[test]
    fn the_empty_hash_is_the_fnv_offset_basis() {
        assert_eq!(StateHash::new().finish(), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn hashing_matches_the_published_fnv1a_vector() {
        // FNV-1a 64-bit of the single byte "a".
        let mut hash = StateHash::new();
        hash.write_u8(b'a');
        assert_eq!(hash.finish(), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn field_order_changes_the_digest() {
        let mut a = StateHash::new();
        a.write_u32(1);
        a.write_u32(2);
        let mut b = StateHash::new();
        b.write_u32(2);
        b.write_u32(1);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn a_one_ulp_difference_in_a_double_is_visible() {
        let mut a = StateHash::new();
        a.write_f64(1.0);
        let mut b = StateHash::new();
        b.write_f64(f64::from_bits(1.0_f64.to_bits() + 1));
        assert_ne!(a.finish(), b.finish());
    }
}
