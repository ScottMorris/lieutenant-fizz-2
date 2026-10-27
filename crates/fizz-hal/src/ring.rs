// A single-producer, single-consumer ring: how an interrupt handler hands key bytes or events to the game loop.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: adapted from Desk's `hal/hal/src/ring.rs` (liminal-hq/desk), keeping its design and dropping the pieces this
// project does not use.
//
// Lock-free with nothing but 32-bit loads, stores and `swap`: the producer alone writes `head`, the consumer alone writes `tail`,
// and the indices run freely and wrap (the capacity is a power of two, so `index & (N - 1)` is the slot and `head - tail` the
// length even across the wrap). No compare-and-swap, which DOSBox's `cputype = 486` rejects, and no lock, which an interrupt could
// deadlock on. On x86 an acquire load and a release store are plain `mov`s; the orderings stop the compiler moving the slot
// access across the index update.
//
// `split` hands out the two ends once, so the single-producer, single-consumer rule is checked by the types. This is the only
// module in the crate that uses `unsafe`; each block carries its justification.
#![allow(unsafe_code)]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// A bounded queue of `N` items of `T` (`N` a power of two), typically in a `static`.
pub struct SpscRing<T: Copy, const N: usize> {
    slots: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicUsize,
    tail: AtomicUsize,
    dropped: AtomicU32,
    split: AtomicBool,
}

// SAFETY: the ends are unique (`split` hands each out once), each slot is written only by the producer while outside `tail..head`
// and read only by the consumer while inside, and the index updates are release stores read with acquire loads; `T: Send` holds.
unsafe impl<T: Copy + Send, const N: usize> Sync for SpscRing<T, N> {}

impl<T: Copy, const N: usize> SpscRing<T, N> {
    /// An empty ring. `N` must be a power of two of at least 2; a wrong `N` fails the build wherever the ring is made.
    pub const fn new() -> Self {
        const {
            assert!(
                N >= 2 && N.is_power_of_two(),
                "SpscRing capacity must be a power of two"
            )
        };
        SpscRing {
            slots: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU32::new(0),
            split: AtomicBool::new(false),
        }
    }

    /// The two ends, the first time it is called; `None` after that. `swap` is `xchg`, which every x86 has.
    pub fn split(&self) -> Option<(Producer<'_, T, N>, Consumer<'_, T, N>)> {
        if self.split.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some((Producer { ring: self }, Consumer { ring: self }))
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    /// Items pushed while the ring was full and therefore lost. Only the producer writes it.
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl<T: Copy, const N: usize> Default for SpscRing<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// The writing end. Lives in an interrupt handler's state.
pub struct Producer<'a, T: Copy, const N: usize> {
    ring: &'a SpscRing<T, N>,
}

/// The reading end. Lives with the game loop.
pub struct Consumer<'a, T: Copy, const N: usize> {
    ring: &'a SpscRing<T, N>,
}

impl<T: Copy, const N: usize> Producer<'_, T, N> {
    /// Adds an item. If the ring is full the new item is dropped and counted, never blocked on: an interrupt handler must not wait.
    pub fn push(&mut self, item: T) -> Result<(), T> {
        let r = self.ring;
        let head = r.head.load(Ordering::Relaxed);
        let tail = r.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) == N {
            // Single writer, so a load and a store are enough (no read-modify-write instruction).
            r.dropped.store(
                r.dropped.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
            return Err(item);
        }
        // SAFETY: the slot at `head` is outside `tail..head`, so the consumer is not reading it, and only this unique producer writes.
        unsafe { (*r.slots[head & (N - 1)].get()).write(item) };
        r.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<T: Copy, const N: usize> Consumer<'_, T, N> {
    /// Takes the oldest item, if any.
    pub fn pop(&mut self) -> Option<T> {
        let r = self.ring;
        let tail = r.tail.load(Ordering::Relaxed);
        let head = r.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // SAFETY: the slot at `tail` is inside `tail..head`, so the producer wrote it (the acquire load of `head` makes the write
        // visible) and will not touch it until `tail` moves past it below.
        let item = unsafe { (*r.slots[tail & (N - 1)].get()).assume_init() };
        r.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.ring.head.load(Ordering::Acquire) == self.ring.tail.load(Ordering::Relaxed)
    }

    /// Items lost to a full ring so far.
    pub fn dropped(&self) -> u32 {
        self.ring.dropped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_come_out_in_order() {
        let ring: SpscRing<u8, 4> = SpscRing::new();
        let (mut tx, mut rx) = ring.split().unwrap();
        assert!(rx.is_empty());
        assert_eq!(rx.pop(), None);
        for b in 1..=3 {
            tx.push(b).unwrap();
        }
        assert!(!rx.is_empty());
        assert_eq!(
            [rx.pop(), rx.pop(), rx.pop(), rx.pop()],
            [Some(1), Some(2), Some(3), None]
        );
    }

    #[test]
    fn a_full_ring_drops_the_new_item_and_counts() {
        let ring: SpscRing<u8, 4> = SpscRing::new();
        let (mut tx, mut rx) = ring.split().unwrap();
        for b in 1..=4 {
            tx.push(b).unwrap();
        }
        assert_eq!(tx.push(5), Err(5));
        assert_eq!(tx.push(6), Err(6));
        assert_eq!(rx.dropped(), 2);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(rx.pop(), Some(1));
        tx.push(7).unwrap();
        assert_eq!(
            [rx.pop(), rx.pop(), rx.pop(), rx.pop(), rx.pop()],
            [Some(2), Some(3), Some(4), Some(7), None]
        );
    }

    #[test]
    fn split_hands_out_the_ends_once() {
        let ring: SpscRing<u8, 2> = SpscRing::new();
        assert!(ring.split().is_some());
        assert!(ring.split().is_none());
        assert_eq!(ring.capacity(), 2);
    }

    #[test]
    fn the_indices_wrap() {
        let ring: SpscRing<u32, 2> = SpscRing::new();
        ring.head.store(usize::MAX, Ordering::Relaxed);
        ring.tail.store(usize::MAX, Ordering::Relaxed);
        let (mut tx, mut rx) = ring.split().unwrap();
        tx.push(7).unwrap();
        tx.push(8).unwrap();
        assert!(tx.push(9).is_err());
        assert_eq!((rx.pop(), rx.pop(), rx.pop()), (Some(7), Some(8), None));
    }

    #[test]
    fn a_producer_thread_and_a_consumer_thread_agree() {
        // Threads stand in for the interrupt and the game loop: on the host they really do run at the same time.
        static RING: SpscRing<u32, 8> = SpscRing::new();
        let (mut tx, mut rx) = RING.split().unwrap();
        const COUNT: u32 = 100_000;
        let producer = std::thread::spawn(move || {
            for i in 0..COUNT {
                while tx.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });
        let mut expected = 0;
        while expected < COUNT {
            if let Some(v) = rx.pop() {
                assert_eq!(v, expected);
                expected += 1;
            }
        }
        producer.join().unwrap();
        assert!(rx.pop().is_none());
    }
}
