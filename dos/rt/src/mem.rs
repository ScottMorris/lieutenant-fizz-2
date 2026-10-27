// `memset` and `memcpy` on string instructions: the rep forms move a dword a clock on a 486, where the generic loops cost several.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository.
//
// `compiler-builtins-mem` supplies these as plain loops with a long set-up path, and every `slice::fill` and `copy_from_slice`
// in the shared crates ends in one of them, as does the copy of the frame to video memory. The drawing fills tile rows of 16
// bytes and frame rows of 320, so the set-up and the per-byte cost both matter. The functions here take over the symbols at link
// time (the linker prefers a strong definition to the weak one in `compiler_builtins`), so the portable crates stay `no_std` and
// `forbid(unsafe_code)` and are not changed for DOS. `memmove` and `memcmp` stay as they were.
//
// The bodies are `asm!` rather than loops so the compiler cannot turn them back into calls to themselves. Each starts with a byte
// move up to the next dword boundary of the destination, then dwords, then the last bytes; a misaligned dword write costs extra
// clocks on a 486, and runs under 8 bytes skip straight to bytes. LLVM keeps `esi` for itself on this target, so `memcpy` swaps
// the source pointer in and out of it around the string instruction.
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

/// # Safety
/// `dest` must be valid for `n` bytes of writes, `src` for `n` bytes of reads, and the two must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut d = dest;
    let mut s = src;
    let mut left = n;
    if left >= BYTE_RUN {
        let head = (d as usize).wrapping_neg() & 3;
        // SAFETY: `head` is at most 3 and `left >= 8`, so both ranges are inside the `n` the caller vouched for.
        unsafe {
            asm!("xchg {s}, esi", "rep movsb", "xchg {s}, esi", inout("ecx") head => _, inout("edi") d, s = inout(reg) s, options(nostack, preserves_flags))
        };
        left -= head;
        let words = left / 4;
        // SAFETY: `words * 4 <= left`, the bytes still to copy from `s` to `d`.
        unsafe {
            asm!("xchg {s}, esi", "rep movsd", "xchg {s}, esi", inout("ecx") words => _, inout("edi") d, s = inout(reg) s, options(nostack, preserves_flags))
        };
        left &= 3;
    }
    // SAFETY: `left` bytes remain at `s` and `d`, inside the `n` the caller vouched for.
    unsafe {
        asm!("xchg {s}, esi", "rep movsb", "xchg {s}, esi", inout("ecx") left => _, inout("edi") d => _, s = inout(reg) s => _, options(nostack, preserves_flags))
    };
    dest
}
