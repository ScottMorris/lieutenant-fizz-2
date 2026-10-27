// Time: a wrapping tick counter, the `Clock` trait and an integer pacer that turns clock ticks into simulation steps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The pacer keeps its remainder in clock-tick-times-millihertz units, so no rate ever rounds: with clock rate `f` millihertz and
// simulation rate `s` hertz, `d` elapsed clock ticks are worth `d * s * 1000 / f` simulation steps. Each call adds
// `d * s * 1000` to the accumulator, takes `acc / f` whole steps and keeps `acc % f`, so the remainder is always below `f` and
// no time is lost or invented. A 60 Hz clock against a 60 Hz simulation is exactly 1:1, and a 1000 Hz clock gives 60 steps per
// 1000 ticks with no drift. The work is 64-bit integer only; the 32-bit tick counter is differenced with wrapping subtraction.

/// A reading of a monotonic 32-bit tick counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticks(pub u32);

impl Ticks {
    /// Ticks from `earlier` to `self`, correct across one wrap of the counter.
    pub const fn since(self, earlier: Ticks) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }
}

/// A monotonic clock that advances by whole ticks at a known rate.
pub trait Clock {
    fn now(&self) -> Ticks;
    /// The tick rate in millihertz (60 Hz is 60,000). Zero if the clock is not running.
    fn millihertz(&self) -> u32;
}

/// Converts elapsed clock time into whole simulation steps using an integer accumulator.
#[derive(Clone, Copy, Debug)]
pub struct Pacer {
    last: Ticks,
    sim_hz: u32,
    max_catch_up: u32,
    /// Remainder in units of 1/(clock millihertz) simulation steps; always below the clock rate.
    acc: u64,
}

impl Pacer {
    /// Starts pacing from the clock's current reading. At most `max_catch_up` steps are returned per `due` call.
    pub fn new(clock: &impl Clock, sim_hz: u32, max_catch_up: u32) -> Self {
        Pacer {
            last: clock.now(),
            sim_hz,
            max_catch_up,
            acc: 0,
        }
    }

    /// The number of simulation steps now due. When more than `max_catch_up` are due the excess time is dropped, not carried, so
    /// a long stall does not cause a burst of steps afterwards.
    pub fn due(&mut self, clock: &impl Clock) -> u32 {
        let now = clock.now();
        let elapsed = now.since(self.last);
        self.last = now;
        let mhz = u64::from(clock.millihertz());
        if mhz == 0 || elapsed == 0 {
            return 0;
        }
        if self.acc >= mhz {
            self.acc = 0;
        }
        self.acc += u64::from(elapsed) * u64::from(self.sim_hz) * 1000;
        let steps = self.acc / mhz;
        self.acc %= mhz;
        if steps > u64::from(self.max_catch_up) {
            self.acc = 0;
            return self.max_catch_up;
        }
        steps as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    struct Fake {
        now: Cell<u32>,
        mhz: u32,
    }

    impl Clock for Fake {
        fn now(&self) -> Ticks {
            Ticks(self.now.get())
        }
        fn millihertz(&self) -> u32 {
            self.mhz
        }
    }

    fn fake(start: u32, mhz: u32) -> Fake {
        Fake {
            now: Cell::new(start),
            mhz,
        }
    }

    #[test]
    fn since_wraps() {
        assert_eq!(Ticks(5).since(Ticks(2)), 3);
        assert_eq!(Ticks(1).since(Ticks(u32::MAX)), 2);
    }

    #[test]
    fn sixty_hertz_clock_is_one_to_one() {
        let c = fake(0, 60_000);
        let mut p = Pacer::new(&c, 60, 10);
        for _ in 0..600 {
            c.now.set(c.now.get() + 1);
            assert_eq!(p.due(&c), 1);
        }
    }

    #[test]
    fn millisecond_clock_gives_sixty_steps_per_second_without_drift() {
        let c = fake(0, 1_000_000);
        let mut p = Pacer::new(&c, 60, 10);
        let mut total = 0;
        for ms in 1..=10_000u32 {
            c.now.set(ms);
            total += p.due(&c);
            if ms % 1000 == 0 {
                assert_eq!(total, 60 * (ms / 1000));
            }
        }
        assert_eq!(total, 600);
    }

    #[test]
    fn no_elapsed_time_is_zero_steps() {
        let c = fake(7, 1_000_000);
        let mut p = Pacer::new(&c, 60, 5);
        assert_eq!(p.due(&c), 0);
        assert_eq!(p.due(&c), 0);
    }

    #[test]
    fn catch_up_is_capped_and_the_excess_dropped() {
        let c = fake(0, 1_000_000);
        let mut p = Pacer::new(&c, 60, 5);
        c.now.set(10_000);
        assert_eq!(p.due(&c), 5);
        assert_eq!(p.due(&c), 0);
        c.now.set(10_017);
        assert_eq!(p.due(&c), 1);
    }

    #[test]
    fn the_clock_wrapping_is_handled() {
        let c = fake(u32::MAX - 2, 1_000_000);
        let mut p = Pacer::new(&c, 60, 100);
        let mut total = 0;
        for _ in 0..1000 {
            c.now.set(c.now.get().wrapping_add(1));
            total += p.due(&c);
        }
        assert_eq!(total, 60);
        assert!(c.now.get() < 1000);
    }

    #[test]
    fn a_stopped_clock_yields_nothing() {
        let c = fake(0, 0);
        let mut p = Pacer::new(&c, 60, 5);
        c.now.set(100);
        assert_eq!(p.due(&c), 0);
    }
}
