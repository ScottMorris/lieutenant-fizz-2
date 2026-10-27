// The clean-up list: what must be undone before the program ends, whether it exits or panics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: copied from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/cleanup.rs, which was adapted from
// Desk spikes/d0-rust-dos/rt/src/cleanup.rs. The only change is rustfmt with dos/rustfmt.toml (130 columns).
//
// A hooked interrupt vector left pointing into a program that has gone is a crash waiting for the next tick, and a screen left
// in a graphics mode is a user waiting at a prompt they cannot read. Whatever installs such a thing registers a function here;
// `exit` and the panic handler run them in reverse order, once each.
//
// Plain loads and stores, no compare-and-swap: the program is single-threaded, and DOSBox's `cputype = 486` refuses CMPXCHG (a
// real 486 from the B step on has it), so nothing in the runtime may use it.
use core::sync::atomic::{AtomicUsize, Ordering};

const SLOTS: usize = 8;

static LIST: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];

/// Registers `f` to run at exit. Returns false if the list is full.
pub fn register(f: fn()) -> bool {
    for slot in &LIST {
        if slot.load(Ordering::SeqCst) == 0 {
            slot.store(f as usize, Ordering::SeqCst);
            return true;
        }
    }
    false
}

/// Removes `f` from the list without running it (the owner has already undone its work).
pub fn forget(f: fn()) {
    for slot in &LIST {
        if slot.load(Ordering::SeqCst) == f as usize {
            slot.store(0, Ordering::SeqCst);
        }
    }
}

/// Runs every registered function, last registered first, and empties the list.
pub fn run_all() {
    for slot in LIST.iter().rev() {
        let f = slot.swap(0, Ordering::SeqCst);
        if f != 0 {
            // SAFETY: the value was stored from a `fn()` by `register`.
            let f: fn() = unsafe { core::mem::transmute::<usize, fn()>(f) };
            f();
        }
    }
}
