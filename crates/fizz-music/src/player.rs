// The stepper that plays a music register stream: allocation-free, one call per 560 Hz tick, in the style of `fizz-sfx`'s `OplPlayer`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The player never touches hardware. It hands the register writes that are due to the caller, which sends them to the chip (or, on the
// host, prints or hashes them). It remembers the last value written to each channel's key register so that `silence` can key the
// music off without disturbing anything else on the chip.

use crate::{Arrangement, RECORD_BYTES};

/// One register write: the bank (0 for registers 0x000 to 0x0ff, 1 for 0x100 to 0x1ff), the register's low byte and the value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Write {
    pub bank: u8,
    pub reg: u8,
    pub val: u8,
}

/// Which chip a stream is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Card {
    /// An OPL2 (or an OPL3 in its OPL2 mode): bank 0 only, five melodic channels and the drums.
    Opl2,
    /// An OPL3: both banks, thirteen melodic channels with stereo output bits, and the drums.
    Opl3,
}

/// The most writes `silence` can make: a key-off for each of the 16 music channels in both banks (channels 5 and 14 are the effects'),
/// and the rhythm register.
pub const MAX_SILENCE_WRITES: usize = 17;

/// The channels (0 to 17, the second bank's from 9) that belong to the effects and are never keyed by the music.
const EFFECT_CHANNELS: [usize; 2] = [5, 14];
const KEY_ON: u8 = 0x20;
/// The rhythm register's five drum key bits (bass drum, snare, tom, cymbal, hi-hat).
const RHYTHM_KEYS: u8 = 0x1f;

/// A 32-bit FNV-1a accumulator. Integer-only, so the DOS build can run it in an interrupt handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv(u32);

impl Fnv {
    pub const fn new() -> Self {
        Self(0x811c_9dc5)
    }

    pub fn byte(&mut self, b: u8) {
        self.0 = (self.0 ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }

    /// One write the way the generator hashes it: its due tick (u32 little-endian, counted from the first record), then its bank,
    /// register and value.
    pub fn write(&mut self, tick: u32, w: Write) {
        for b in tick.to_le_bytes() {
            self.byte(b);
        }
        self.byte(w.bank);
        self.byte(w.reg);
        self.byte(w.val);
    }

    pub const fn value(&self) -> u32 {
        self.0
    }
}

impl Default for Fnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Steps one arrangement. Call [`tick`](Self::tick) once per 560 Hz tick; it plays the prelude once, then loops at `loop_start`.
#[derive(Clone, Debug)]
pub struct MusicPlayer {
    arrangement: Arrangement,
    next: usize,
    delay: u32,
    loops: u32,
    paused: bool,
    /// The last value written to each channel's 0xB0 register (bank 0's nine channels, then bank 1's), for channels the music owns.
    keys: [u8; 18],
    /// The last value written to 0xBD.
    rhythm: u8,
}

impl MusicPlayer {
    /// A player at the start of `arrangement`.
    pub fn new(arrangement: Arrangement) -> Self {
        Self {
            arrangement,
            next: 0,
            delay: 0,
            loops: 0,
            paused: false,
            keys: [0; 18],
            rhythm: 0,
        }
    }

    /// Steps the writes due on this tick into `out` and returns how many. A well-formed stream never puts more than
    /// [`MAX_WRITES_PER_TICK`](crate::MAX_WRITES_PER_TICK) on one tick, so a buffer of that size is always enough; if `out` is
    /// smaller the rest wait for the next call (late, in order, never dropped). Returns 0 while paused.
    pub fn tick(&mut self, out: &mut [Write]) -> usize {
        if self.paused {
            return 0;
        }
        let mut n = 0;
        loop {
            if self.delay > 0 {
                self.delay -= 1;
                return n;
            }
            if n == out.len() {
                return n;
            }
            if self.next >= self.arrangement.record_count {
                self.next = self.arrangement.loop_start;
                self.loops += 1;
            }
            let at = self.next * RECORD_BYTES;
            let b = &self.arrangement.records[at..at + RECORD_BYTES];
            let packed = u16::from_le_bytes([b[2], b[3]]);
            let w = Write {
                bank: (packed >> 15) as u8,
                reg: b[0],
                val: b[1],
            };
            self.delay = u32::from(packed & 0x7fff);
            self.next += 1;
            self.remember(w);
            out[n] = w;
            n += 1;
        }
    }

    fn remember(&mut self, w: Write) {
        if w.bank == 0 && w.reg == 0xbd {
            self.rhythm = w.val;
        } else if w.bank <= 1 && (0xb0..=0xb8).contains(&w.reg) {
            let ch = usize::from(w.bank) * 9 + usize::from(w.reg - 0xb0);
            if !EFFECT_CHANNELS.contains(&ch) {
                self.keys[ch] = w.val;
            }
        }
    }

    /// Keys off the music and nothing else: for each music channel whose last 0xB0 write had the key bit set, writes that value with
    /// the bit cleared, and clears the rhythm register's drum key bits. It never writes channel 5 or 14's registers or 0x104 and
    /// 0x105, and it leaves instruments, levels and pitches alone, so the music can carry on afterwards. Returns the writes made
    /// (none if nothing is sounding); [`MAX_SILENCE_WRITES`] always suffice, and with a smaller `out` call again for the rest.
    pub fn silence(&mut self, out: &mut [Write]) -> usize {
        let mut n = 0;
        for ch in 0..18 {
            if n == out.len() {
                return n;
            }
            if EFFECT_CHANNELS.contains(&ch) || self.keys[ch] & KEY_ON == 0 {
                continue;
            }
            let val = self.keys[ch] & !KEY_ON;
            out[n] = Write {
                bank: (ch / 9) as u8,
                reg: 0xb0 + (ch % 9) as u8,
                val,
            };
            self.keys[ch] = val;
            n += 1;
        }
        if n < out.len() && self.rhythm & RHYTHM_KEYS != 0 {
            let val = self.rhythm & !RHYTHM_KEYS;
            out[n] = Write {
                bank: 0,
                reg: 0xbd,
                val,
            };
            self.rhythm = val;
            n += 1;
        }
        n
    }

    /// Whether every music channel is keyed off, as far as this player has written.
    pub fn is_silent(&self) -> bool {
        self.rhythm & RHYTHM_KEYS == 0
            && self
                .keys
                .iter()
                .enumerate()
                .all(|(ch, k)| EFFECT_CHANNELS.contains(&ch) || k & KEY_ON == 0)
    }

    /// Stops stepping and keys the music off (as [`silence`](Self::silence); returns the writes made).
    pub fn pause(&mut self, out: &mut [Write]) -> usize {
        self.paused = true;
        self.silence(out)
    }

    /// Carries on from where [`pause`](Self::pause) stopped, without skipping or repeating a record. Instruments survive the key-off;
    /// the notes that were sounding stay off until their next key-on.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Goes back to the start of the prelude, unpaused. It writes nothing and keeps what it knows of the chip's key registers, so
    /// `silence` still works afterwards; call `silence` first if the music is sounding and should not hang.
    pub fn reset(&mut self) {
        self.next = 0;
        self.delay = 0;
        self.loops = 0;
        self.paused = false;
    }

    /// How many times the stream has jumped back to `loop_start` (0 during the prelude and the first pass).
    pub fn loops(&self) -> u32 {
        self.loops
    }
}

/// Steps a fresh player for `ticks` ticks and returns the writes made and their FNV-1a hash, each hashed at its tick counted from
/// the start (loops are not subtracted). For `ticks == prelude_ticks + loop_ticks` this is one pass and the hash is the
/// arrangement's `hash`.
pub fn writes_for(arrangement: Arrangement, ticks: u32) -> (u32, u32) {
    let mut p = MusicPlayer::new(arrangement);
    let (mut buf, mut count, mut fnv) = ([Write::default(); 32], 0u32, Fnv::new());
    for tick in 0..ticks {
        let n = p.tick(&mut buf);
        for w in &buf[..n] {
            fnv.write(tick, *w);
        }
        count += n as u32;
    }
    (count, fnv.value())
}

/// How many times a fresh player has jumped back to `loop_start` after `ticks` ticks.
pub fn count_loops(arrangement: Arrangement, ticks: u32) -> u32 {
    let mut p = MusicPlayer::new(arrangement);
    let mut buf = [Write::default(); 32];
    for _ in 0..ticks {
        p.tick(&mut buf);
    }
    p.loops()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MAX_WRITES_PER_TICK, level_song, reserved_registers};
    use std::vec::Vec;

    const CARDS: [Card; 2] = [Card::Opl2, Card::Opl3];
    const FNV_OFFSET: u32 = 0x811c_9dc5;

    fn arr(card: Card) -> Arrangement {
        level_song().arrangement(card)
    }

    /// Steps `ticks` ticks, returning each tick's writes.
    fn run(p: &mut MusicPlayer, ticks: u32) -> Vec<Vec<Write>> {
        let mut buf = [Write::default(); 32];
        (0..ticks)
            .map(|_| {
                let n = p.tick(&mut buf);
                assert!(n <= MAX_WRITES_PER_TICK as usize, "{n} writes in one tick");
                buf[..n].to_vec()
            })
            .collect()
    }

    fn forbidden(w: &Write) -> bool {
        reserved_registers().contains(&w.reg) || (w.bank == 1 && (w.reg == 0x04 || w.reg == 0x05))
    }

    /// The stream read straight from the file the generator wrote, expanded without the player: (due tick, write) per record.
    fn reference(file: &str) -> Vec<(u32, Write)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(file);
        let bytes = std::fs::read(path).expect("the generated stream");
        let mut tick = 0;
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| {
                let packed = u16::from_le_bytes([c[2], c[3]]);
                let due = tick;
                tick += u32::from(packed & 0x7fff);
                (
                    due,
                    Write {
                        bank: (packed >> 15) as u8,
                        reg: c[0],
                        val: c[1],
                    },
                )
            })
            .collect()
    }

    #[test]
    fn one_pass_reproduces_the_committed_hash_and_record_count() {
        for card in CARDS {
            let a = arr(card);
            let (count, hash) = writes_for(a, a.pass_ticks());
            assert_eq!(count as usize, a.record_count, "{card:?}");
            assert_eq!(hash, a.hash, "{card:?}");
            // One tick short of the pass loses nothing but the pass's last records only if they were due on the last tick.
            let (short, _) = writes_for(a, a.pass_ticks() - 1);
            assert!(short <= count);
        }
    }

    #[test]
    fn the_fnv_accumulator_matches_known_values() {
        // FNV-1a 32 of the empty input and of "a" (0xe40c292c).
        assert_eq!(Fnv::new().value(), FNV_OFFSET);
        let mut f = Fnv::default();
        f.byte(b'a');
        assert_eq!(f.value(), 0xe40c_292c);
    }

    #[test]
    fn two_loops_repeat_the_pass_and_never_touch_a_reserved_register() {
        for card in CARDS {
            let a = arr(card);
            let mut p = MusicPlayer::new(a);
            // Step as fast as possible (no clock): the prelude, two full loops, and the tick the third pass starts on.
            let total = a.prelude_ticks + 2 * a.loop_ticks + 1;
            let ticks = run(&mut p, total);
            assert_eq!(p.loops(), 2, "{card:?}");
            assert_eq!(count_loops(a, a.pass_ticks()), 0);
            assert_eq!(count_loops(a, a.pass_ticks() + 1), 1);
            assert_eq!(count_loops(a, total), 2);

            let (mut first, mut loop1, mut loop2) = (Fnv::new(), Fnv::new(), Fnv::new());
            let (mut n1, mut n2, mut most) = (0, 0, 0);
            for (t, writes) in ticks.iter().enumerate() {
                let t = t as u32;
                most = most.max(writes.len());
                for w in writes {
                    assert!(!forbidden(w), "{card:?} tick {t}: {w:?}");
                    if t < a.pass_ticks() {
                        first.write(t, *w);
                    }
                    if (a.pass_ticks()..a.prelude_ticks + 2 * a.loop_ticks).contains(&t) {
                        loop1.write(t - a.prelude_ticks - a.loop_ticks, *w);
                        n1 += 1;
                    } else if t >= a.prelude_ticks + 2 * a.loop_ticks {
                        // The third pass has begun: it repeats the loop's first writes.
                    }
                    if (a.prelude_ticks..a.pass_ticks()).contains(&t) {
                        loop2.write(t - a.prelude_ticks, *w);
                        n2 += 1;
                    }
                }
            }
            assert_eq!(first.value(), a.hash, "{card:?}");
            assert_eq!(n1, n2, "{card:?}");
            assert_eq!(loop1.value(), loop2.value(), "{card:?}: loop passes differ");
            assert!(most <= MAX_WRITES_PER_TICK as usize, "{card:?}");
            assert_eq!(most, a.max_writes_per_tick as usize, "{card:?}");
        }
    }

    #[test]
    fn the_stepper_equals_the_generated_stream() {
        for (card, file) in [
            (Card::Opl2, "crater-opl2.bin"),
            (Card::Opl3, "crater-opl3.bin"),
        ] {
            let a = arr(card);
            let stream = reference(file);
            assert_eq!(stream.len(), a.record_count);
            let mut p = MusicPlayer::new(a);
            let ticks = run(&mut p, a.pass_ticks() + a.loop_ticks);
            // Pass one: every record on its tick, in order.
            let got: Vec<(u32, Write)> = ticks
                .iter()
                .enumerate()
                .flat_map(|(t, ws)| ws.iter().map(move |w| (t as u32, *w)))
                .collect();
            let first = &got[..stream.len()];
            assert_eq!(first, &stream[..], "{card:?}");
            // Pass two: the loop's records, shifted by one loop.
            let looped: Vec<(u32, Write)> = stream[a.loop_start..]
                .iter()
                .map(|(t, w)| (t + a.loop_ticks, *w))
                .collect();
            assert_eq!(&got[stream.len()..], &looped[..], "{card:?}");
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn silence_after_every_tick_leaves_the_music_off_and_touches_no_reserved_register() {
        for card in CARDS {
            let a = arr(card);
            let mut p = MusicPlayer::new(a);
            let mut buf = [Write::default(); 32];
            let mut off = [Write::default(); MAX_SILENCE_WRITES];
            let mut chip = [[0u8; 256]; 2];
            let (mut keyed_at_some_point, mut silenced) = (false, 0);
            for t in 0..a.pass_ticks() {
                let n = p.tick(&mut buf);
                for w in &buf[..n] {
                    chip[w.bank as usize][w.reg as usize] = w.val;
                }
                // Clone: silence must not disturb the player that carries on.
                let mut probe = p.clone();
                let m = probe.silence(&mut off);
                assert!(m <= MAX_SILENCE_WRITES);
                let mut after = chip;
                for w in &off[..m] {
                    assert!(!forbidden(w), "{card:?} tick {t}: {w:?}");
                    assert!((0xb0..=0xb8).contains(&w.reg) || (w.bank == 0 && w.reg == 0xbd));
                    after[w.bank as usize][w.reg as usize] = w.val;
                }
                for bank in 0..2 {
                    for reg in 0xb0..=0xb8usize {
                        let ch = bank * 9 + reg - 0xb0;
                        if ch != 5 && ch != 14 {
                            assert_eq!(after[bank][reg] & 0x20, 0, "{card:?} tick {t} ch {ch}");
                        }
                    }
                }
                assert_eq!(after[0][0xbd] & 0x1f, 0, "{card:?} tick {t}");
                assert!(probe.is_silent());
                // A second call has nothing left to do.
                assert_eq!(probe.silence(&mut off), 0);
                if m > 0 {
                    silenced += 1;
                    keyed_at_some_point = true;
                }
            }
            assert!(
                keyed_at_some_point && silenced > 100,
                "{card:?}: {silenced}"
            );
        }
    }

    #[test]
    fn silence_keeps_the_rest_of_each_register_and_fits_a_small_buffer() {
        let mut p = MusicPlayer::new(arr(Card::Opl3));
        let mut buf = [Write::default(); 32];
        let mut chip = [[0u8; 256]; 2];
        for _ in 0..6000 {
            let n = p.tick(&mut buf);
            for w in &buf[..n] {
                chip[w.bank as usize][w.reg as usize] = w.val;
            }
        }
        let mut one = [Write::default(); 1];
        let mut total = 0;
        loop {
            let n = p.silence(&mut one);
            if n == 0 {
                break;
            }
            let w = one[0];
            let old = chip[w.bank as usize][w.reg as usize];
            let cleared = if w.reg == 0xbd { 0x1f } else { 0x20 };
            assert_eq!(w.val, old & !cleared);
            chip[w.bank as usize][w.reg as usize] = w.val;
            total += 1;
        }
        assert!(total > 0 && p.is_silent());
    }

    #[test]
    fn pause_and_resume_continue_the_stream_without_skipping_or_repeating() {
        for card in CARDS {
            let a = arr(card);
            let whole = {
                let mut p = MusicPlayer::new(a);
                run(&mut p, 12_000)
            };
            let mut p = MusicPlayer::new(a);
            let mut got: Vec<Vec<Write>> = Vec::new();
            let mut off = [Write::default(); MAX_SILENCE_WRITES];
            let mut buf = [Write::default(); 32];
            while got.len() < 12_000 {
                let burst = 4_321.min(12_000 - got.len());
                got.extend(run(&mut p, burst as u32));
                if got.len() == 12_000 {
                    break;
                }
                let m = p.pause(&mut off);
                assert!(m > 0, "something was sounding");
                assert!(p.is_paused() && p.is_silent());
                // While paused it writes nothing and does not advance.
                for _ in 0..1000 {
                    assert_eq!(p.tick(&mut buf), 0);
                }
                p.resume();
            }
            assert_eq!(got, whole, "{card:?}");
        }
    }

    #[test]
    fn instruments_survive_a_key_off() {
        // Silence writes only the key registers, so no operator, feedback or frequency-low register is ever written by it.
        let mut p = MusicPlayer::new(arr(Card::Opl3));
        let mut buf = [Write::default(); 32];
        for _ in 0..3000 {
            p.tick(&mut buf);
        }
        let mut off = [Write::default(); MAX_SILENCE_WRITES];
        let m = p.silence(&mut off);
        assert!(
            off[..m]
                .iter()
                .all(|w| (0xb0..=0xb8).contains(&w.reg) || w.reg == 0xbd)
        );
    }

    #[test]
    fn reset_starts_over() {
        let a = arr(Card::Opl2);
        let mut p = MusicPlayer::new(a);
        let first = run(&mut p, 2000);
        run(&mut p, a.pass_ticks());
        p.reset();
        assert_eq!(p.loops(), 0);
        assert_eq!(run(&mut p, 2000), first);
    }

    #[test]
    fn a_small_buffer_delays_writes_but_drops_none() {
        let a = arr(Card::Opl2);
        let mut p = MusicPlayer::new(a);
        let mut one = [Write::default(); 1];
        let mut count = 0usize;
        let mut ticks = 0;
        while count < a.record_count {
            count += p.tick(&mut one);
            ticks += 1;
            assert!(ticks < 2 * a.pass_ticks(), "the stream stalled at {count}");
        }
        assert_eq!(count, a.record_count);
    }
}
