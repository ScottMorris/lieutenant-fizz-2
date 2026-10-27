// Seeded random number generator and stateless 2D hash for deterministic jitter.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from Fizz 1 `crates/sim/src/rng.rs` at commit `4bce036`; transformed as in fizz-dos `m2/patches/sim.patch`.
// Edits: none beyond this comment.

/// Stateless 2D integer hash in [0, 1), used for deterministic decoration and spawn jitter.
pub fn hashf(x: i32, y: i32) -> f64 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ ((h as u32) >> 13) as i32).wrapping_mul(1_274_126_177);
    f64::from((h ^ ((h as u32) >> 16) as i32) as u32) / 4_294_967_296.0
}

/// Small xorshift generator: deterministic, allocation free, good enough for particles.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(if seed == 0 { 0x9e37_79b9 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        s
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_u32()) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_in_range() {
        assert_eq!(hashf(3, 4), hashf(3, 4));
        for i in 0..200 {
            let v = hashf(i, 7);
            assert!((0.0..1.0).contains(&v));
        }
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(5);
        let mut b = Rng::new(5);
        for _ in 0..10 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
        assert!((0.0..1.0).contains(&a.next_f64()));
    }
}
