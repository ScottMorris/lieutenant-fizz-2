// `memset` on a string instruction: `rep stosd` stores a dword a clock on a 486, where the generic loop costs several.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// `compiler-builtins-mem` supplies `memset` as a plain loop with a long set-up path, and every `slice::fill` in the shared
// crates ends in it. The drawing fills tile rows of 16 bytes and frame rows of 320, so the set-up and the per-byte cost both
// matter. The function here takes over the symbol at link time (the linker prefers a strong definition to the weak one in
// `compiler_builtins`), so the portable crates stay `no_std` and `forbid(unsafe_code)` and are not changed for DOS.
//
// The body is `asm!` rather than a loop so the compiler cannot turn it back into a call to itself. It starts with a byte fill up
// to the next dword boundary of the destination, then dwords, then the last bytes; a misaligned dword write costs extra clocks
// on a 486, and runs under 8 bytes skip straight to bytes.
use core::arch::asm;

/// Runs shorter than this are filled a byte at a time: the dword path would spend its set-up on the alignment and the tail.
const BYTE_RUN: usize = 8;

/// # Safety
/// `dest` must be valid for `n` bytes of writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let byte = c as u8;
    let mut p = dest;
    let mut left = n;
    if left >= BYTE_RUN {
        let head = (p as usize).wrapping_neg() & 3;
        // SAFETY: `head` is at most 3 and `left >= 8`, so the bytes written are inside the `n` the caller vouched for.
        unsafe { asm!("rep stosb", inout("ecx") head => _, inout("edi") p, in("al") byte, options(nostack, preserves_flags)) };
        left -= head;
        let words = left / 4;
        let fill = u32::from(byte) * 0x0101_0101;
        // SAFETY: `words * 4 <= left`, the bytes still to write, and `p` is `dest + head`.
        unsafe { asm!("rep stosd", inout("ecx") words => _, inout("edi") p, in("eax") fill, options(nostack, preserves_flags)) };
        left &= 3;
    }
    // SAFETY: `left` bytes remain at `p`, inside the `n` the caller vouched for.
    unsafe { asm!("rep stosb", inout("ecx") left => _, inout("edi") p => _, in("al") byte, options(nostack, preserves_flags)) };
    dest
}
