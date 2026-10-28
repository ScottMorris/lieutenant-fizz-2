// Generated OPL2 and OPL3 register streams for the level music. This crate only holds the tables; the player that steps them comes later.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The streams are lowered by `tools/music/generate.ts` from the song source under `assets/music/` (see `docs/MUSIC.md`) and checked in
// CI; `data.rs` and `data/*.bin` are generated. The song is a placeholder: Fizz 1's "Crater Fields". Nothing here allocates, touches
// hardware or is `unsafe`.
#![no_std]
#![forbid(unsafe_code)]

mod data;

/// Bytes in one record: the register's low byte, the value, then a little-endian u16 holding the bank in its top bit and the number
/// of 560 Hz ticks to wait after the write in the low 15 bits (the layout of `fizz_sfx::OplStep`).
pub const RECORD_BYTES: usize = 4;

/// A note the OPL2 arrangement could not carry on its five melodic channels, as the note list names it.
#[derive(Clone, Copy, Debug)]
pub struct Dropped {
    /// The start on the note list's 96-tick bar grid.
    pub tick: u32,
    pub part: &'static str,
    pub midi: u8,
}

/// One lowered arrangement of a song.
#[derive(Clone, Copy, Debug)]
pub struct Arrangement {
    /// The records, [`RECORD_BYTES`] each: the prelude (the one-time setup), then the loop.
    pub records: &'static [u8],
    pub record_count: usize,
    /// The index of the first record of the loop; the player jumps back to it and never repeats the prelude.
    pub loop_start: usize,
    /// Ticks from the first record to the record at `loop_start`.
    pub prelude_ticks: u32,
    /// Ticks from the record at `loop_start` round to it again.
    pub loop_ticks: u32,
    /// Melodic notes played, not counting the dropped ones.
    pub notes: u32,
    /// Rhythm-mode drum hits in the loop.
    pub drum_hits: u32,
    pub max_writes_per_tick: u32,
    /// The 32-bit FNV-1a of one pass over each record's due tick (u32 little-endian), bank, register and value.
    pub hash: u32,
    /// The melodic channels (0 to 17) the stream writes; 6 to 8 are the drums, 5 and 14 are never written.
    pub channels: &'static [u8],
    pub dropped: &'static [Dropped],
}

/// A song and its two arrangements.
#[derive(Clone, Copy, Debug)]
pub struct Song {
    pub id: &'static str,
    pub title: &'static str,
    /// Whether the song is a placeholder rather than an official Fizz 2 track.
    pub placeholder: bool,
    pub bpm: u32,
    pub bars: u32,
    /// Five melodic channels (0 to 4) and the rhythm-mode drums on 6 to 8, for an OPL2 or an OPL3 card.
    pub opl2: Arrangement,
    /// Thirteen melodic channels with partner layers and stereo output bits, for an OPL3.
    pub opl3: Arrangement,
}

/// Every song of the source.
pub fn songs() -> &'static [Song] {
    &data::SONGS
}

/// The song the level plays.
pub fn level_song() -> &'static Song {
    &data::SONGS[data::LEVEL]
}

/// The registers (low bytes, either bank) of the effects' channels 5 and 14, which no music stream writes.
pub fn reserved_registers() -> &'static [u8] {
    &data::RESERVED_REGISTERS
}

/// The most register writes any tick of a music stream may carry.
pub const MAX_WRITES_PER_TICK: u32 = data::MAX_WRITES_PER_TICK;

#[cfg(test)]
mod tests {
    use super::*;

    /// One record: (bank, register, value, wait).
    fn record(a: &Arrangement, i: usize) -> (u8, u8, u8, u32) {
        let b = &a.records[i * RECORD_BYTES..(i + 1) * RECORD_BYTES];
        let packed = u16::from_le_bytes([b[2], b[3]]);
        ((packed >> 15) as u8, b[0], b[1], u32::from(packed & 0x7fff))
    }

    fn arrangements(s: &Song) -> [(&'static str, &Arrangement); 2] {
        [("opl2", &s.opl2), ("opl3", &s.opl3)]
    }

    #[test]
    fn the_tables_are_present_and_their_sizes_match_the_generated_counts() {
        assert!(!songs().is_empty());
        for s in songs() {
            for (name, a) in arrangements(s) {
                assert!(a.record_count > 0, "{} {name}", s.id);
                assert_eq!(
                    a.records.len(),
                    a.record_count * RECORD_BYTES,
                    "{} {name}",
                    s.id
                );
                assert!(a.loop_start < a.record_count, "{} {name}", s.id);
            }
        }
        assert!(level_song().placeholder);
    }

    #[test]
    fn the_ticks_the_hash_and_the_write_cap_match_the_stream() {
        for s in songs() {
            for (name, a) in arrangements(s) {
                let (mut tick, mut prelude, mut hash) = (0u32, 0u32, 0x811c_9dc5u32);
                let (mut run, mut most) = (0u32, 0u32);
                for i in 0..a.record_count {
                    let (bank, reg, val, wait) = record(a, i);
                    if i == a.loop_start {
                        prelude = tick;
                    }
                    for byte in tick.to_le_bytes().into_iter().chain([bank, reg, val]) {
                        hash = (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193);
                    }
                    run += 1;
                    most = most.max(run);
                    if wait > 0 {
                        run = 0;
                    }
                    tick += wait;
                }
                assert_eq!(prelude, a.prelude_ticks, "{} {name}", s.id);
                assert_eq!(tick - prelude, a.loop_ticks, "{} {name}", s.id);
                assert_eq!(hash, a.hash, "{} {name}", s.id);
                assert_eq!(most, a.max_writes_per_tick, "{} {name}", s.id);
                assert!(most <= MAX_WRITES_PER_TICK, "{} {name}", s.id);
            }
        }
    }

    #[test]
    fn no_stream_writes_an_effects_register_or_the_opl3_control_registers() {
        for s in songs() {
            for (name, a) in arrangements(s) {
                assert!(
                    !a.channels.contains(&5) && !a.channels.contains(&14),
                    "{} {name}",
                    s.id
                );
                for i in 0..a.record_count {
                    let (bank, reg, _, _) = record(a, i);
                    assert!(
                        !reserved_registers().contains(&reg),
                        "{} {name}: record {i}",
                        s.id
                    );
                    assert!(
                        !(bank == 1 && (reg == 0x04 || reg == 0x05)),
                        "{} {name}: record {i}",
                        s.id
                    );
                    if name == "opl2" {
                        assert_eq!(bank, 0, "{} {name}: record {i}", s.id);
                    }
                }
            }
        }
    }

    #[test]
    fn the_opl2_arrangement_drops_what_the_generator_says_it_drops() {
        for s in songs() {
            assert!(s.opl3.dropped.is_empty(), "{}", s.id);
            assert!(s.opl3.notes >= s.opl2.notes, "{}", s.id);
        }
    }
}
