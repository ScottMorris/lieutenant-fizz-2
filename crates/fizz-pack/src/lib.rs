// The Lieutenant Fizz 2 asset pack (`.FAP`): the reader, a raw-deflate decoder and, behind the `write` feature, the writer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reader is `no_std`, allocation-free, `unsafe`-free and dependency-free, so the DOS executable, the Web build and the baker
// share one definition. The format is specified in `docs/FAP.md`, which is the source of truth: where this crate and that document
// differ, the crate is wrong. The summary below is only a pointer; the field tables live in the spec.
//
// - Header 32 bytes (the last 8 are the pack stem), directory entries 24 bytes (the last 4 are the FNV-1a 32 of the section's stored
//   bytes), sections on 16-byte boundaries in ascending offset order, file length a multiple of 16.
// - Compatibility numbers: `compatibility(content)` for world and shared packs, `audio_compatibility(device)` for `MUSIC.*`.
// - Codecs: 0 stored, 1 raw deflate (RFC 1951); anything else is refused. Codec 2 is reserved for LZMA and is not implemented.
// - Section checksums are checked whenever stored bytes are handed out or decoded; the whole-file checksum by `PackView::parse`.
//
// Decoder size (measured 2026-10-29, i586, panic=abort, the `inflate` function plus its three helpers, bounds checks included): about
// 2.9 KB at opt-level `s` and 2.3 KB at `z`; the plan's earlier 1 KB figure is for a decoder without Rust's bounds and overflow
// checks. The decoder is written for small code (canonical codes decoded bit by bit, length and distance tables computed rather than
// stored) at the price of speed, and uses about 1.3 KB of stack. Re-measure with `cargo rustc --release -p fizz-pack --target
// i586-unknown-linux-gnu -- --emit=obj -C panic=abort -C opt-level=z` and `nm -S --size-sort` on the object.
#![no_std]
#![forbid(unsafe_code)]

#[cfg(feature = "write")]
extern crate alloc;

pub mod inflate;
pub mod level;
pub mod pack;
#[cfg(feature = "write")]
pub mod write;

pub use inflate::{InflateError, inflate};
pub use level::{LevelError, LevelView, ObjectRec};
pub use pack::{
    Device, Entry, Error, Expect, PackView, audio_compatibility, compatibility, fnv, stem_bytes,
};
