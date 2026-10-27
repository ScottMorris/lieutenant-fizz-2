// A Rust runtime for 32-bit DOS programs that run under a DPMI host (CWSDPMI) behind DJGPP's stub: no C library, no crt0.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from fizz-2-dos-poc1 crates/fizz2-dos-rt/src/lib.rs (itself from Desk spikes/d0-rust-dos/rt/src/lib.rs).
// Changes here: the module list is reduced to what Fizz 2 uses (no `hal`, `music`, `mem` or `bios_kbd`; no Desk or engine
// dependency), and `ring`, `port`, `kbd` and a new `irq` and `timer` are added.
//
// One concern per module:
//
// - `start`    the entry point `_start`, the registers the stub hands over, the stack and the stub's information block
// - `dpmi`     a client for the DPMI host: INT 31h, memory blocks, descriptors, locking, interrupt vectors, real-mode calls
// - `dos`      INT 21h wrappers: console and file I/O, the exit call
// - `heap`     the global allocator, over DPMI memory blocks
// - `env`      the command tail and the environment, read from the PSP
// - `fpu`      the x87 control word: read it, set 53-bit precision
// - `io`       `print!`, `println!` and a `core::fmt` writer over standard output
// - `panic`    the panic handler: run the clean-up list, report, exit with status 3 (the `panic-handler` feature, on by default)
// - `cleanup`  what must be undone before the program ends, by exit or by panic
// - `port`     port I/O: `inb` and `outb`
// - `ring`     a single-producer, single-consumer byte ring for handing bytes from an interrupt handler to the program
// - `irq`      the interrupt registry: a stub per IRQ line that calls a Rust handler, and the end-of-interrupt
// - `timer`    IRQ 0 at 60 Hz, with the BIOS clock still advancing at 18.2 Hz
// - `kbd`      IRQ 1: raw scancodes into a ring, and the BIOS keyboard flags put back at the end
// - `vga`      mode 13h, the palette, the vertical retrace and the frame buffer
//
// Single-threaded, with one exception: interrupt handlers, which touch only `static`s and never allocate, call DOS or use the x87.
#![no_std]
// The imported DPMI client and allocator are audited as units, with their `unsafe` blocks, rather than per operation.
#![allow(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod cleanup;
pub mod dos;
pub mod dpmi;
pub mod env;
pub mod fpu;
pub mod heap;
pub mod io;
pub mod irq;
pub mod kbd;
#[cfg(feature = "panic-handler")]
mod panic;
pub mod port;
pub mod ring;
pub mod start;
pub mod timer;
pub mod vga;

pub use dos::exit;

/// Declares the program's entry function, which runs after the runtime is up. It returns the exit status.
///
/// ```ignore
/// fizz_dos_rt::entry!(main);
/// fn main() -> i32 { fizz_dos_rt::println!("hello"); 0 }
/// ```
#[macro_export]
macro_rules! entry {
    ($f:path) => {
        #[unsafe(no_mangle)]
        pub extern "Rust" fn __rt_main() -> i32 {
            let f: fn() -> i32 = $f;
            f()
        }
    };
}
