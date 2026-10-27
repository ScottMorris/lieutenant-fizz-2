// A single-producer, single-consumer byte ring, for handing bytes from an interrupt handler to the program.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The handler is the only producer and the program the only consumer. Each side writes one index and only reads the other, so plain
// atomic loads and stores are enough: no compare-and-swap (DOSBox's `cputype = 486` refuses CMPXCHG), and 32-bit indices, the widest
// atomic the target has. The storage is atomic bytes too, so the type needs no `unsafe`. A full ring drops the new byte rather than
// blocking the handler, and counts the drop.
use core::sync::atomic::{AtomicU8, AtomicU32, Ordering};

/// A ring of `N` bytes (`N` a power of two) that holds at most `N - 1` at a time.
pub struct Ring<const N: usize> {
    buf: [AtomicU8; N],
    /// Next index to read; written by the consumer only.
    head: AtomicU32,
    /// Next index to write; written by the producer only.
    tail: AtomicU32,
    dropped: AtomicU32,
}

impl<const N: usize> Ring<N> {
    const MASK: u32 = {
        assert!(N.is_power_of_two() && N >= 2 && N <= 1 << 16);
        (N - 1) as u32
    };

    pub const fn new() -> Ring<N> {
        Ring {
            buf: [const { AtomicU8::new(0) }; N],
            head: AtomicU32::new(0),
            tail: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
        }
    }

    /// Producer side. Returns false (and counts a drop) if the ring is full.
    pub fn push(&self, byte: u8) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let next = tail.wrapping_add(1) & Self::MASK;
        if next == self.head.load(Ordering::Acquire) {
            self.dropped
                .store(self.dropped.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
            return false;
        }
        self.buf[tail as usize].store(byte, Ordering::Relaxed);
        self.tail.store(next, Ordering::Release);
        true
    }

    /// Consumer side. Returns the oldest byte, if any.
    pub fn pop(&self) -> Option<u8> {
        let head = self.head.load(Ordering::Relaxed);
        if head == self.tail.load(Ordering::Acquire) {
            return None;
        }
        let byte = self.buf[head as usize].load(Ordering::Relaxed);
        self.head.store(head.wrapping_add(1) & Self::MASK, Ordering::Release);
        Some(byte)
    }

    /// Bytes dropped because the ring was full.
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Whether the ring holds nothing.
    pub fn is_empty(&self) -> bool {
        self.head.load(Ordering::Relaxed) == self.tail.load(Ordering::Acquire)
    }
}

impl<const N: usize> Default for Ring<N> {
    fn default() -> Ring<N> {
        Ring::new()
    }
}
