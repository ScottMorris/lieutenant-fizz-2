// The f64 functions `core` does not have, from the `libm` crate so every target (native, web, DOS) computes the same values.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from fizz-dos `m2/sim/src/fmath.rs` (see fizz-dos `m2/patches/sim.patch`), trimmed to the functions
// `tilemap`, `body`, `platform` and `rng` use. `exp` is removed because `libm` runs x87 assembly for it on x86 targets without SSE.

/// Method-call syntax for the `libm` functions, so `x.floor()` reads the same with and without `std`.
pub trait FloatMath: Sized {
    fn floor(self) -> Self;
    fn cos(self) -> Self;
}

impl FloatMath for f64 {
    fn floor(self) -> f64 {
        libm::floor(self)
    }
    fn cos(self) -> f64 {
        libm::cos(self)
    }
}
