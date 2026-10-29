// The Lieutenant Fizz 2 asset pack (`.FAP`): the reader, a raw-deflate decoder and, behind the `write` feature, the writer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reader is `no_std`, allocation-free, `unsafe`-free and dependency-free, so the DOS executable, the Web build and the baker
// share one definition. The format spec is `docs/FAP.md` (written by package `m2/pack-format`); this crate was written in parallel
// from the plan in `m2-fap-bake.md`, so where the plan was silent the choices below are the ones to reconcile with the spec.
//
// Layout choices beyond the plan's sketch (all little-endian):
// - Header (32 bytes): `FAP\x1A`; u16 version (1); u16 header size (32); u8 target; u8 flags (0); u16 section count; u32
//   compatibility number; u32 total length; u32 checksum; 8 reserved bytes that must be zero.
// - The checksum is FNV-1a 32 over the whole file with the four checksum bytes (offset 20) read as zero. The total length must equal
//   the file length exactly.
// - A directory entry is 20 bytes: u32 tag, u32 offset, u32 stored length, u32 decoded length, u8 codec, u8 section version,
//   u16 count (the section's item count, informational). Entries are sorted by tag; among equal tags the directory order is the
//   section's index. Offsets are 16-aligned, ascending in directory order (so overlap checking needs no allocation), at or past
//   the end of the directory, and each section lies inside the file. Padding bytes are zero (the checksum covers them).
// - The compatibility number is `compatibility(content)`: FNV-1a over `SCHEMA_HASH` (see its docs for exactly what feeds it) and then
//   over `content`, a byte table the game crate supplies for the tile ids and object enums, which this crate cannot see.
// - Codecs: 0 stored (stored length equals decoded length), 1 raw deflate (RFC 1951) decoded into exactly the recorded length.
//   Anything else is refused.
// - `LEVL` layout: see `level.rs`; object kind numbers are 1 key-red, 2 key-blue, 3 snack, 4 chicken, 5 gadget, 6 switch, 7 enemy.
//
// Decoder size (measured 2026-10-29, i586, panic=abort, the `inflate` function plus its three helpers, bounds checks included): about
// 2.9 KB at opt-level `s` and 2.3 KB at `z`; the plan's 1 KB figure is for a decoder without Rust's bounds and overflow checks. The
// decoder is written for small code (canonical codes decoded bit by bit, length and distance tables computed rather than stored) at
// the price of speed, and uses about 1.3 KB of stack. Re-measure with `cargo rustc --release -p fizz-pack --target
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
pub use pack::{Entry, Error, PackView, compatibility, fnv};
