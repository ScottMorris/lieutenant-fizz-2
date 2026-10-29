// Container tests: round trips with the writer, level bytes, malformed packs and determinism.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_pack::level::{KIND_CHICKEN, KIND_SNACK, LevelError};
use fizz_pack::pack::{
    CODEC_DEFLATE, CODEC_STORED, ENTRY_LEN, HEADER_LEN, SCHEMA_HASH, TAG_LEVL, TAG_MUSC, TAG_OPL2,
    TAG_OPL3, TARGET_DOS, checksum,
};
use fizz_pack::write::{LevelSpec, PackWriter, deflate_fixed, deflate_stored, encode_level, seal};
use fizz_pack::{Error, InflateError, LevelView, ObjectRec, PackView, compatibility, fnv};

const COMPAT: u32 = compatibility(b"tiles v1");

fn level_bytes() -> Vec<u8> {
    let objects = [
        ObjectRec {
            kind: KIND_SNACK,
            sub: 2,
            flags: 0,
            channel: 0,
            x_bits: 3.5f64.to_bits(),
            y_bits: 4.0f64.to_bits(),
            p_bits: 0,
        },
        ObjectRec {
            kind: KIND_CHICKEN,
            sub: 0,
            flags: 1,
            channel: 9,
            x_bits: 8.0f64.to_bits(),
            y_bits: 11.0f64.to_bits(),
            p_bits: (-1.0f64).to_bits(),
        },
    ];
    let tiles: Vec<u8> = (0..12 * 5).map(|i| (i % 7) as u8).collect();
    encode_level(&LevelSpec {
        width: 12,
        height: 5,
        switches: 0b101,
        floor_solid: true,
        spawn_x_bits: 11.7f64.to_bits(),
        spawn_y_bits: 2.0f64.to_bits(),
        name: "Slice",
        objects: &objects,
        tiles: &tiles,
    })
}

/// Four sections: two LEVL (index 0 compressed if smaller, index 1 stored), one deflated MUSC, one stored OPL3.
fn sample() -> Vec<u8> {
    let mut w = PackWriter::new(TARGET_DOS, COMPAT);
    w.add_stored(TAG_OPL3, 1, 3, b"opl3 bytes");
    w.add_compressed(TAG_LEVL, 1, 2, &level_bytes());
    w.add_stored(TAG_LEVL, 1, 2, &level_bytes());
    w.add_compressed(
        TAG_MUSC,
        1,
        0,
        &b"song song song song song song ".repeat(20),
    );
    w.finish()
}

fn put_u32(f: &mut [u8], at: usize, v: u32) {
    f[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn get_u32(f: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(f[at..at + 4].try_into().unwrap())
}

/// Edits a sample pack and reseals it, so only the edit (not the checksum) can be the reason for refusal.
fn edited(edit: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut f = sample();
    edit(&mut f);
    seal(&mut f);
    f
}

/// Offset of directory entry `i`.
fn ent(i: usize) -> usize {
    HEADER_LEN + i * ENTRY_LEN
}

#[test]
fn round_trip_levels_and_sections() {
    let f = sample();
    let p = PackView::parse(&f, COMPAT).unwrap();
    assert_eq!((p.target(), p.flags(), p.len()), (TARGET_DOS, 0, 4));
    assert_eq!(p.count_of(TAG_LEVL), 2);
    assert!(p.find(TAG_LEVL, 2).is_none() && p.find(TAG_OPL2, 0).is_none());
    let tags: Vec<u32> = p.entries().map(|e| e.tag).collect();
    assert!(tags.windows(2).all(|w| w[0] <= w[1]));
    assert!(p.entries().all(|e| e.offset % 16 == 0));

    let want = level_bytes();
    for index in 0..2 {
        let e = p.find(TAG_LEVL, index).unwrap();
        assert_eq!((e.decoded_len as usize, e.count), (want.len(), 2));
        let mut buf = vec![0u8; want.len() + 10];
        let n = p.decode(&e, &mut buf).unwrap();
        assert_eq!(&buf[..n], &want[..]);
    }
    let m = p.find(TAG_MUSC, 0).unwrap();
    assert_eq!(m.codec, CODEC_DEFLATE);
    assert!(m.stored_len < m.decoded_len);
    assert!(p.stored_in_place(&m).is_none());
    let o = p.find(TAG_OPL3, 0).unwrap();
    assert_eq!(p.stored_in_place(&o), Some(&b"opl3 bytes"[..]));
    assert_eq!(o.codec, CODEC_STORED);
}

#[test]
fn level_view_reads_every_field() {
    let b = level_bytes();
    let v = LevelView::new(&b).unwrap();
    assert_eq!(
        (v.width(), v.height(), v.switches(), v.floor_solid()),
        (12, 5, 0b101, true)
    );
    assert_eq!(v.spawn(), (11.7, 2.0));
    assert_eq!(v.spawn_bits().0, 11.7f64.to_bits());
    assert_eq!(v.name(), b"Slice");
    assert_eq!(v.object_count(), 2);
    let c = v.object(1);
    assert_eq!(
        (c.kind, c.sub, c.flags, c.channel, c.x(), c.y(), c.p()),
        (KIND_CHICKEN, 0, 1, 9, 8.0, 11.0, -1.0)
    );
    assert_eq!(v.objects().count(), 2);
    assert_eq!(v.tiles().len(), 60);
    assert_eq!(v.tiles()[8], 1);
    // Header 32, name padded to 8, two 32-byte objects, then the tiles.
    assert_eq!(b.len(), 32 + 8 + 64 + 60);
}

#[test]
fn level_view_refuses_wrong_sizes_and_fields() {
    let b = level_bytes();
    assert_eq!(LevelView::new(&b[..31]).unwrap_err(), LevelError::Truncated);
    assert_eq!(
        LevelView::new(&b[..b.len() - 1]).unwrap_err(),
        LevelError::Truncated
    );
    let mut long = b.clone();
    long.push(0);
    assert_eq!(
        LevelView::new(&long).unwrap_err(),
        LevelError::TrailingBytes
    );
    let mut bad = b.clone();
    bad[8] = 2;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::BadField);
    let mut bad = b.clone();
    bad[13] = 1;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::BadField);
    // A width and height that would multiply far past the data.
    let mut huge = b.clone();
    huge[0..4].copy_from_slice(&[0xFF; 4]);
    assert_eq!(LevelView::new(&huge).unwrap_err(), LevelError::Truncated);
    // An object count that runs past the data.
    let mut many = b;
    many[10..12].copy_from_slice(&[0xFF; 2]);
    assert_eq!(LevelView::new(&many).unwrap_err(), LevelError::Truncated);
}

#[test]
fn level_with_no_objects_and_a_long_name() {
    let name = "n".repeat(255);
    let b = encode_level(&LevelSpec {
        width: 1,
        height: 2,
        switches: 0,
        floor_solid: false,
        spawn_x_bits: 0,
        spawn_y_bits: 0,
        name: &name,
        objects: &[],
        tiles: &[7, 8],
    });
    let v = LevelView::new(&b).unwrap();
    assert_eq!(
        (v.name().len(), v.object_count(), v.tiles()),
        (255, 0, &[7, 8][..])
    );
    assert!(!v.floor_solid());
}

#[test]
fn header_refusals() {
    let ok = sample();
    assert!(PackView::parse(&ok, COMPAT).is_ok());
    assert_eq!(
        PackView::parse(&ok[..20], COMPAT).unwrap_err(),
        Error::Truncated
    );
    assert_eq!(PackView::parse(&[], COMPAT).unwrap_err(), Error::Truncated);
    assert_eq!(
        PackView::parse(&edited(|f| f[0] = b'X'), COMPAT).unwrap_err(),
        Error::BadMagic
    );
    assert_eq!(
        PackView::parse(&edited(|f| f[3] = b'Z'), COMPAT).unwrap_err(),
        Error::BadMagic
    );
    assert_eq!(
        PackView::parse(&edited(|f| f[4] = 2), COMPAT).unwrap_err(),
        Error::BadVersion
    );
    assert_eq!(
        PackView::parse(&edited(|f| f[6] = 36), COMPAT).unwrap_err(),
        Error::BadHeaderLen
    );
    assert_eq!(
        PackView::parse(&edited(|f| f[28] = 1), COMPAT).unwrap_err(),
        Error::Reserved
    );
    assert_eq!(
        PackView::parse(&ok, COMPAT ^ 1).unwrap_err(),
        Error::Incompatible
    );
    assert_eq!(
        PackView::parse(&ok, compatibility(&[])).unwrap_err(),
        Error::Incompatible
    );
    // A pack baked with another content table is incompatible even though everything else is intact.
    let mut w = PackWriter::new(TARGET_DOS, compatibility(b"tiles v2"));
    w.add_stored(TAG_LEVL, 1, 0, b"x");
    assert_eq!(
        PackView::parse(&w.finish(), COMPAT).unwrap_err(),
        Error::Incompatible
    );
}

#[test]
fn length_and_checksum_refusals() {
    let ok = sample();
    assert_eq!(
        PackView::parse(&ok[..ok.len() - 1], COMPAT).unwrap_err(),
        Error::Truncated
    );
    let mut long = ok.clone();
    long.push(0);
    assert_eq!(
        PackView::parse(&long, COMPAT).unwrap_err(),
        Error::LengthMismatch
    );
    // Any flipped bit is caught by the checksum (not resealed).
    for at in [8usize, 12, 40, ok.len() - 1] {
        let mut bad = ok.clone();
        bad[at] ^= 0x40;
        assert_eq!(
            PackView::parse(&bad, COMPAT).unwrap_err(),
            Error::BadChecksum,
            "byte {at}"
        );
    }
    let mut bad = ok;
    bad[20] ^= 1;
    assert_eq!(
        PackView::parse(&bad, COMPAT).unwrap_err(),
        Error::BadChecksum
    );
}

#[test]
fn checksum_reads_its_own_field_as_zero() {
    let f = sample();
    let mut zeroed = f.clone();
    zeroed[20..24].fill(0);
    assert_eq!(checksum(&f), checksum(&zeroed));
    assert_eq!(checksum(&f), get_u32(&f, 20));
    assert_eq!(fnv(b""), 0x811C_9DC5);
    assert_eq!(fnv(b"a"), 0xE40C_292C); // the published FNV-1a 32 vector for "a"
}

#[test]
fn directory_refusals() {
    // Entries in tag order: 0 LEVL, 1 LEVL, 2 MUSC, 3 OPL3.
    let f = edited(|f| put_u32(f, ent(3) + 8, 1_000_000));
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::BadCodec); // stored lengths now differ
    let f = edited(|f| {
        put_u32(f, ent(3) + 8, 1_000_000);
        put_u32(f, ent(3) + 12, 1_000_000);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::OutOfRange);
    // Offset plus length overflows 32 bits.
    let f = edited(|f| {
        put_u32(f, ent(3) + 4, 0xFFFF_FFF0);
        put_u32(f, ent(3) + 8, 0x20);
        put_u32(f, ent(3) + 12, 0x20);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::OutOfRange);
    // A section inside the header and directory.
    let f = edited(|f| put_u32(f, ent(0) + 4, 16));
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::OutOfRange);
    // Misaligned.
    let f = edited(|f| {
        let o = get_u32(f, ent(1) + 4);
        put_u32(f, ent(1) + 4, o + 1);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Misaligned);
    // Overlap: entry 1 starts where entry 0 starts.
    let f = edited(|f| {
        let o = get_u32(f, ent(0) + 4);
        put_u32(f, ent(1) + 4, o);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Overlap);
    // Overlap by length: entry 2 (deflated) grows into entry 3.
    let f = edited(|f| {
        let l = get_u32(f, ent(2) + 8);
        put_u32(f, ent(2) + 8, l + 16);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Overlap);
    // Sections out of ascending offset order (swap two offsets).
    let f = edited(|f| {
        let (a, b) = (get_u32(f, ent(2) + 4), get_u32(f, ent(3) + 4));
        put_u32(f, ent(2) + 4, b);
        put_u32(f, ent(3) + 4, a);
    });
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Overlap);
    // Tags out of order.
    let f = edited(|f| put_u32(f, ent(0), u32::from_le_bytes(*b"ZZZZ")));
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Unsorted);
    // Unknown codec.
    let f = edited(|f| f[ent(3) + 16] = 2);
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::BadCodec);
    // A section count that reaches past the file.
    let f = edited(|f| f[10..12].copy_from_slice(&1000u16.to_le_bytes()));
    assert_eq!(PackView::parse(&f, COMPAT).unwrap_err(), Error::Truncated);
}

#[test]
fn decode_refusals() {
    // A deflate section whose recorded decoded length is a lie, in both directions.
    let data = b"abcabcabcabcabcabc".repeat(10);
    let stream = deflate_fixed(&data);
    for (claim, want) in [
        (data.len() as u32 + 5, Error::Inflate(InflateError::Short)),
        (data.len() as u32 - 5, Error::Inflate(InflateError::Overrun)),
    ] {
        let mut w = PackWriter::new(TARGET_DOS, COMPAT);
        w.add_raw(TAG_MUSC, CODEC_DEFLATE, 1, 0, stream.clone(), claim);
        let f = w.finish();
        let p = PackView::parse(&f, COMPAT).unwrap();
        let e = p.find(TAG_MUSC, 0).unwrap();
        let mut out = vec![0u8; 1000];
        assert_eq!(p.decode(&e, &mut out).unwrap_err(), want);
    }
    // Output buffer too small.
    let mut w = PackWriter::new(TARGET_DOS, COMPAT);
    w.add_compressed(TAG_MUSC, 1, 0, &data);
    let f = w.finish();
    let p = PackView::parse(&f, COMPAT).unwrap();
    let e = p.find(TAG_MUSC, 0).unwrap();
    assert_eq!(
        p.decode(&e, &mut [0u8; 10]).unwrap_err(),
        Error::OutputTooSmall
    );
    // A stored deflate stream with a bad complement.
    let mut bad = deflate_stored(b"hello");
    bad[3] ^= 1;
    let mut w = PackWriter::new(TARGET_DOS, COMPAT);
    w.add_raw(TAG_MUSC, CODEC_DEFLATE, 1, 0, bad, 5);
    let f = w.finish();
    let p = PackView::parse(&f, COMPAT).unwrap();
    let e = p.find(TAG_MUSC, 0).unwrap();
    assert_eq!(
        p.decode(&e, &mut [0u8; 5]).unwrap_err(),
        Error::Inflate(InflateError::BadBlock)
    );
}

#[test]
fn every_truncation_of_a_pack_is_refused() {
    let f = sample();
    for cut in 0..f.len() {
        assert!(PackView::parse(&f[..cut], COMPAT).is_err(), "cut {cut}");
    }
}

#[test]
fn single_byte_corruption_never_panics() {
    let f = sample();
    for at in 0..f.len() {
        let mut bad = f.clone();
        bad[at] ^= 0xA5;
        seal(&mut bad); // pass the checksum so the deeper checks run
        if let Ok(p) = PackView::parse(&bad, COMPAT) {
            let mut buf = vec![0u8; 4096];
            for e in p.entries() {
                if let Ok(n) = p.decode(&e, &mut buf) {
                    let _ = LevelView::new(&buf[..n]).map(|l| l.objects().count());
                }
            }
        }
    }
}

#[test]
fn the_pack_is_deterministic_and_insertion_order_sets_the_index() {
    assert_eq!(sample(), sample());
    let (a, b) = (level_bytes(), vec![0u8; 8]);
    let mut w = PackWriter::new(TARGET_DOS, COMPAT);
    w.add_stored(TAG_LEVL, 1, 0, &a)
        .add_stored(TAG_LEVL, 1, 0, &b);
    let f = w.finish();
    let p = PackView::parse(&f, COMPAT).unwrap();
    assert_eq!(p.stored(&p.find(TAG_LEVL, 0).unwrap()), &a[..]);
    assert_eq!(p.stored(&p.find(TAG_LEVL, 1).unwrap()), &b[..]);
    // The file ends exactly at the last section, and the gap before each section is zero.
    let last = p.entries().last().unwrap();
    assert_eq!(f.len(), (last.offset + last.stored_len) as usize);
    let first = p.entry(0);
    assert!(
        f[HEADER_LEN + 2 * ENTRY_LEN..first.offset as usize]
            .iter()
            .all(|&x| x == 0)
    );
}

#[test]
fn compatibility_is_a_function_of_the_content_table() {
    assert_ne!(compatibility(b"a"), compatibility(b"b"));
    assert_eq!(compatibility(&[]), SCHEMA_HASH);
    // Pinned so an accidental schema edit is a deliberate test change (and a deliberate pack-version decision).
    assert_eq!(SCHEMA_HASH, 0xBB01_B10B); // also recomputed independently in Python when written
}
