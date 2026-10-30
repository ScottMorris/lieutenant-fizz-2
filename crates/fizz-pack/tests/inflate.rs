// Deflate decoder tests: zlib-made vectors, rejected streams, and round trips through the crate's own compressor.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod vectors;

use fizz_pack::write::{deflate, deflate_fixed, deflate_stored};
use fizz_pack::{InflateError, inflate};
use vectors::VECTORS;

const TEXT: &str = "Lieutenant Fizz walks past the red door, grabs the blue key, and the chicken clucks twice. \
Snacks are worth ten points; the chicken is worth none. Fizz jumps, Fizz lands, Fizz waits. ";

/// The plain input of each vector, mirroring `plain()` in gen_vectors.py.
fn plain(name: &str) -> Vec<u8> {
    match name {
        "text" => TEXT.repeat(6).into_bytes(),
        "run" => {
            let mut v = vec![b'a'; 1000];
            v.extend(b"abc".repeat(100));
            v.extend((0..=255u8).chain(0..=255u8));
            v
        }
        "short" => b"hello, hello, hello".to_vec(),
        "empty" => Vec::new(),
        "noise" => {
            let (mut x, mut out) = (12345u64, Vec::new());
            for _ in 0..900 {
                x = (x * 1103515245 + 12345) & 0x7FFF_FFFF;
                out.push(b"eeeeettttaaoonnissrh dlu"[((x >> 16) % 24) as usize]);
            }
            let head = out[..200].to_vec();
            out.extend(head);
            out
        }
        _ => panic!("unknown recipe {name}"),
    }
}

#[test]
fn zlib_vectors_decode_to_their_input() {
    assert!(
        VECTORS.iter().any(|v| v.3 == 0)
            && VECTORS.iter().any(|v| v.3 == 1)
            && VECTORS.iter().any(|v| v.3 == 2)
    );
    for (name, recipe, stream, _) in VECTORS {
        let want = plain(recipe);
        let mut out = vec![0u8; want.len()];
        inflate(stream, &mut out).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(out, want, "{name}");
    }
}

#[test]
fn every_truncation_of_every_vector_is_rejected() {
    for (name, recipe, stream, _) in VECTORS {
        let want = plain(recipe);
        for cut in 0..stream.len() {
            let mut out = vec![0u8; want.len()];
            // A prefix must never decode to the full input (it may decode to an error of any kind, never panic).
            assert!(
                inflate(&stream[..cut], &mut out).is_err(),
                "{name} cut at {cut}"
            );
        }
    }
}

#[test]
fn a_decoded_length_lie_is_rejected_both_ways() {
    for (name, recipe, stream, _) in VECTORS {
        let want = plain(recipe).len();
        let mut big = vec![0u8; want + 1];
        assert_eq!(
            inflate(stream, &mut big),
            Err(InflateError::Short),
            "{name} long"
        );
        if want > 0 {
            let mut small = vec![0u8; want - 1];
            let e = inflate(stream, &mut small).unwrap_err();
            assert!(
                matches!(e, InflateError::Overrun | InflateError::Truncated),
                "{name} short: {e:?}"
            );
        }
    }
}

#[test]
fn bad_block_type_and_stored_complement_are_rejected() {
    let mut out = [0u8; 8];
    assert_eq!(
        inflate(&[0b0000_0111], &mut out),
        Err(InflateError::BadBlock)
    ); // final, type 3
    assert_eq!(
        inflate(&[1, 5, 0, 0, 0, 1, 2, 3, 4, 5], &mut out),
        Err(InflateError::BadBlock)
    );
    assert_eq!(
        inflate(&[1, 5, 0, 0xFA, 0xFF, 1, 2], &mut out),
        Err(InflateError::Truncated)
    );
    assert_eq!(
        inflate(&[1, 5, 0, 0xFA, 0xFF, 1, 2, 3, 4, 5], &mut out[..4]),
        Err(InflateError::Overrun)
    );
}

#[test]
fn an_invalid_distance_is_rejected() {
    // Fixed block: literal 'a', then length 3 at distance 2 (only one byte exists).
    let mut w = Bits::default();
    w.put(1, 1);
    w.put(1, 2);
    w.lit(b'a' as u32);
    w.lit(257); // length 3
    w.code(1, 5); // distance symbol 1 = distance 2
    w.lit(256);
    let mut out = [0u8; 4];
    assert_eq!(
        inflate(&w.finish(), &mut out),
        Err(InflateError::BadDistance)
    );
}

#[test]
fn a_deflate_bomb_stops_at_the_output_bound() {
    // 'a' then maximal length-258 matches at distance 1, many times: would expand to 1 MB, the buffer is 300 bytes.
    let mut w = Bits::default();
    w.put(1, 1);
    w.put(1, 2);
    w.lit(b'a' as u32);
    for _ in 0..4000 {
        w.lit(285);
        w.code(0, 5);
    }
    w.lit(256);
    let stream = w.finish();
    assert!(stream.len() < 8000);
    let mut out = [0u8; 300];
    assert_eq!(inflate(&stream, &mut out), Err(InflateError::Overrun));
    let mut exact = vec![0u8; 1 + 258 * 4000];
    inflate(&stream, &mut exact).unwrap();
    assert!(exact.iter().all(|&b| b == b'a'));
}

#[test]
fn bad_symbols_and_tables_are_rejected() {
    // Fixed block using length symbol 286 (a code the fixed table has but deflate forbids).
    let mut w = Bits::default();
    w.put(1, 1);
    w.put(1, 2);
    w.lit(286);
    let mut out = [0u8; 4];
    assert_eq!(inflate(&w.finish(), &mut out), Err(InflateError::BadSymbol));
    // Fixed block using distance symbol 30.
    let mut w = Bits::default();
    w.put(1, 1);
    w.put(1, 2);
    w.lit(b'a' as u32);
    w.lit(257);
    w.code(30, 5);
    assert_eq!(inflate(&w.finish(), &mut out), Err(InflateError::BadSymbol));
    // Dynamic block with 286+ literal codes declared as 287 (HLIT 30).
    let mut w = Bits::default();
    w.put(1, 1);
    w.put(2, 2);
    w.put(30, 5);
    w.put(0, 5);
    w.put(0, 4);
    assert_eq!(inflate(&w.finish(), &mut out), Err(InflateError::BadTable));
}

#[test]
fn the_writers_streams_round_trip() {
    let mut inputs: Vec<Vec<u8>> = ["text", "run", "short", "empty", "noise"]
        .iter()
        .map(|n| plain(n))
        .collect();
    inputs.push(vec![0u8; 70_000]); // two stored blocks, long runs
    inputs.push(
        (0..200_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect(),
    );
    for (i, data) in inputs.iter().enumerate() {
        for (name, stream) in [
            ("stored", deflate_stored(data)),
            ("fixed", deflate_fixed(data)),
            ("best", deflate(data)),
        ] {
            let mut out = vec![0u8; data.len()];
            inflate(&stream, &mut out).unwrap_or_else(|e| panic!("input {i} {name}: {e:?}"));
            assert_eq!(&out, data, "input {i} {name}");
        }
    }
    assert!(deflate_fixed(&plain("text")).len() < plain("text").len() / 3);
}

#[test]
fn the_compressor_is_deterministic() {
    let data = plain("text");
    assert_eq!(deflate_fixed(&data), deflate_fixed(&data));
    assert_eq!(deflate(&data), deflate(&data));
}

/// A tiny least-significant-bit-first writer for hand-made streams (fixed Huffman codes only).
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, v: u32, bits: u32) {
        self.acc |= u64::from(v) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    fn code(&mut self, c: u32, bits: u32) {
        self.put(c.reverse_bits() >> (32 - bits), bits);
    }
    fn lit(&mut self, s: u32) {
        match s {
            0..=143 => self.code(0x30 + s, 8),
            144..=255 => self.code(0x190 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0xC0 + s - 280, 8),
        }
    }
    fn finish(mut self) -> Vec<u8> {
        self.put(0, 7); // flush with zero padding
        self.out
    }
}
