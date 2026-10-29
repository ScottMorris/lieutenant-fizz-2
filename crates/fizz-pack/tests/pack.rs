// Container tests: round trips with the writer, level bytes, malformed packs and determinism.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_pack::level::{KIND_CHICKEN, KIND_SNACK, LevelError};
use fizz_pack::pack::{
    CODEC_DEFLATE, CODEC_STORED, CONTAINER_HASH, ENTRY_LEN, HEADER_LEN, LEVEL_HASH, SCHEMA_HASH,
    SECTION_LIMIT, TAG_FXPK, TAG_LEVL, TAG_MUSC, TAG_OPL2, TAG_OPL3, TAG_SPKR, TARGET_DOS,
    TARGET_WEB, audio_hash, checksum,
};
use fizz_pack::write::{LevelSpec, PackWriter, deflate_fixed, deflate_stored, encode_level, seal};
use fizz_pack::{
    Device, Error, Expect, InflateError, LevelView, ObjectRec, PackView, audio_compatibility,
    compatibility, fnv,
};

const COMPAT: u32 = compatibility(b"tiles v1");
const STEM: &str = "SHARED";

// Pinned schema numbers, computed independently in Python (FNV-1a 32 over the same feeds).
const PIN_CONTAINER: u32 = 0x6F16_BA34;
const PIN_LEVEL: u32 = 0xEB2F_AD5D;
const PIN_AUDIO_OPL3: u32 = 0xB899_1606;
const PIN_AUDIO_OPL2: u32 = 0x8D24_DCC8;
const PIN_AUDIO_SPKR: u32 = 0x855E_3CF6;
const PIN_SCHEMA: u32 = 0x7C49_EA7E;
const PIN_COMPAT_OPL3: u32 = 0x2E1A_73F9;

fn expect() -> Expect<'static> {
    Expect {
        target: TARGET_DOS,
        stem: STEM,
        compat: COMPAT,
    }
}

fn parse(f: &[u8]) -> Result<PackView<'_>, Error> {
    PackView::parse(f, &expect())
}

fn pack(target: u8, compat: u32, stem: &str) -> PackWriter {
    PackWriter::new(target, compat, stem)
}

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
            flags: 0,
            channel: 0,
            x_bits: 8.0f64.to_bits(),
            y_bits: 3.0f64.to_bits(),
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
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
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
    let p = parse(&f).unwrap();
    assert_eq!((p.target(), p.len()), (TARGET_DOS, 4));
    assert_eq!(p.count_of(TAG_LEVL), 2);
    assert!(p.find(TAG_LEVL, 2).is_none() && p.find(TAG_OPL2, 0).is_none());
    let tags: Vec<u32> = p.entries().map(|e| e.tag).collect();
    assert!(
        tags.windows(2)
            .all(|w| w[0].swap_bytes() <= w[1].swap_bytes())
    );
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
    assert!(p.stored_in_place(&m).unwrap().is_none());
    let o = p.find(TAG_OPL3, 0).unwrap();
    assert_eq!(p.stored_in_place(&o), Ok(Some(&b"opl3 bytes"[..])));
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
        (KIND_CHICKEN, 0, 0, 0, 8.0, 3.0, -1.0)
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
    bad[29] = 1;
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
    let name = "n".repeat(48);
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
        (48, 0, &[7, 8][..])
    );
    assert!(!v.floor_solid());
}

#[test]
fn header_refusals() {
    let ok = sample();
    assert!(parse(&ok).is_ok());
    assert_eq!(parse(&ok[..20]).unwrap_err(), Error::Truncated);
    assert_eq!(parse(&[]).unwrap_err(), Error::Truncated);
    assert_eq!(
        parse(&edited(|f| f[0] = b'X')).unwrap_err(),
        Error::BadMagic
    );
    assert_eq!(
        parse(&edited(|f| f[3] = b'Z')).unwrap_err(),
        Error::BadMagic
    );
    assert_eq!(parse(&edited(|f| f[4] = 2)).unwrap_err(), Error::BadVersion);
    assert_eq!(
        parse(&edited(|f| f[6] = 36)).unwrap_err(),
        Error::BadHeaderLen
    );
    assert_eq!(parse(&edited(|f| f[9] = 1)).unwrap_err(), Error::BadFlags);
    let with = |compat: u32| Expect { compat, ..expect() };
    assert_eq!(
        PackView::parse(&ok, &with(COMPAT ^ 1)).unwrap_err(),
        Error::Incompatible
    );
    assert_eq!(
        PackView::parse(&ok, &with(compatibility(&[]))).unwrap_err(),
        Error::Incompatible
    );
    // A pack baked with another content table is incompatible even though everything else is intact.
    let mut w = pack(TARGET_DOS, compatibility(b"tiles v2"), STEM);
    w.add_stored(TAG_LEVL, 1, 0, b"x");
    assert_eq!(parse(&w.finish()).unwrap_err(), Error::Incompatible);
}

#[test]
fn length_and_checksum_refusals() {
    let ok = sample();
    assert_eq!(parse(&ok[..ok.len() - 1]).unwrap_err(), Error::Truncated);
    let mut long = ok.clone();
    long.push(0);
    assert_eq!(parse(&long).unwrap_err(), Error::LengthMismatch);
    // Any flipped bit is caught by the checksum (not resealed).
    for at in [40usize, 56, 100, ok.len() - 1] {
        let mut bad = ok.clone();
        bad[at] ^= 0x40;
        assert_eq!(parse(&bad).unwrap_err(), Error::BadChecksum, "byte {at}");
    }
    let mut bad = ok;
    bad[20] ^= 1;
    assert_eq!(parse(&bad).unwrap_err(), Error::BadChecksum);
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
    assert_eq!(parse(&f).unwrap_err(), Error::BadCodec); // stored lengths now differ
    let f = edited(|f| {
        put_u32(f, ent(3) + 8, 1_000_000);
        put_u32(f, ent(3) + 12, 1_000_000);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::OutOfRange);
    // Offset plus length overflows 32 bits.
    let f = edited(|f| {
        put_u32(f, ent(3) + 4, 0xFFFF_FFF0);
        put_u32(f, ent(3) + 8, 0x20);
        put_u32(f, ent(3) + 12, 0x20);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::OutOfRange);
    // A section inside the header and directory.
    let f = edited(|f| put_u32(f, ent(0) + 4, 16));
    assert_eq!(parse(&f).unwrap_err(), Error::OutOfRange);
    // Misaligned.
    let f = edited(|f| {
        let o = get_u32(f, ent(1) + 4);
        put_u32(f, ent(1) + 4, o + 1);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::Misaligned);
    // Overlap: entry 1 starts where entry 0 starts.
    let f = edited(|f| {
        let o = get_u32(f, ent(0) + 4);
        put_u32(f, ent(1) + 4, o);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::Overlap);
    // Overlap by length: entry 2 (deflated) grows into entry 3.
    let f = edited(|f| {
        let l = get_u32(f, ent(2) + 8);
        put_u32(f, ent(2) + 8, l + 16);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::Overlap);
    // Sections out of ascending offset order (swap two offsets).
    let f = edited(|f| {
        let (a, b) = (get_u32(f, ent(2) + 4), get_u32(f, ent(3) + 4));
        put_u32(f, ent(2) + 4, b);
        put_u32(f, ent(3) + 4, a);
    });
    assert_eq!(parse(&f).unwrap_err(), Error::Overlap);
    // Tags out of order.
    let f = edited(|f| put_u32(f, ent(0), u32::from_le_bytes(*b"ZZZZ")));
    assert_eq!(parse(&f).unwrap_err(), Error::Unsorted);
    // Unknown codec.
    let f = edited(|f| f[ent(3) + 16] = 2);
    assert_eq!(parse(&f).unwrap_err(), Error::BadCodec);
    // A section count that reaches past the file.
    let f = edited(|f| f[10..12].copy_from_slice(&1000u16.to_le_bytes()));
    assert_eq!(parse(&f).unwrap_err(), Error::Truncated);
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
        let mut w = pack(TARGET_DOS, COMPAT, STEM);
        w.add_raw(TAG_MUSC, CODEC_DEFLATE, 1, 0, stream.clone(), claim);
        let f = w.finish();
        let p = parse(&f).unwrap();
        let e = p.find(TAG_MUSC, 0).unwrap();
        let mut out = vec![0u8; 1000];
        assert_eq!(p.decode(&e, &mut out).unwrap_err(), want);
    }
    // Output buffer too small.
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
    w.add_compressed(TAG_MUSC, 1, 0, &data);
    let f = w.finish();
    let p = parse(&f).unwrap();
    let e = p.find(TAG_MUSC, 0).unwrap();
    assert_eq!(
        p.decode(&e, &mut [0u8; 10]).unwrap_err(),
        Error::OutputTooSmall
    );
    // A stored deflate stream with a bad complement.
    let mut bad = deflate_stored(b"hello");
    bad[3] ^= 1;
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
    w.add_raw(TAG_MUSC, CODEC_DEFLATE, 1, 0, bad, 5);
    let f = w.finish();
    let p = parse(&f).unwrap();
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
        assert!(parse(&f[..cut]).is_err(), "cut {cut}");
    }
}

#[test]
fn single_byte_corruption_never_panics() {
    let f = sample();
    for at in 0..f.len() {
        let mut bad = f.clone();
        bad[at] ^= 0xA5;
        seal(&mut bad); // pass the checksum so the deeper checks run
        if let Ok(p) = parse(&bad) {
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
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
    w.add_stored(TAG_LEVL, 1, 0, &a)
        .add_stored(TAG_LEVL, 1, 0, &b);
    let f = w.finish();
    let p = parse(&f).unwrap();
    assert_eq!(p.stored(&p.find(TAG_LEVL, 0).unwrap()), Ok(&a[..]));
    assert_eq!(p.stored(&p.find(TAG_LEVL, 1).unwrap()), Ok(&b[..]));
    // The file ends within 16 bytes of the last section, zero padded, and its length is a multiple of 16.
    let last = p.entries().last().unwrap();
    let end = (last.offset + last.stored_len) as usize;
    assert!(f.len().is_multiple_of(16) && f.len() - end < 16 && f[end..].iter().all(|&x| x == 0));
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
    // Pinned so an accidental schema edit is a deliberate test change (and a deliberate pack-version decision). The values were also
    // recomputed independently in Python when written.
    assert_eq!(CONTAINER_HASH, PIN_CONTAINER);
    assert_eq!(LEVEL_HASH, PIN_LEVEL);
    assert_eq!(audio_hash(Device::Opl3), PIN_AUDIO_OPL3);
    assert_eq!(audio_hash(Device::Opl2), PIN_AUDIO_OPL2);
    assert_eq!(audio_hash(Device::Speaker), PIN_AUDIO_SPKR);
    assert_eq!(SCHEMA_HASH, PIN_SCHEMA);
    assert_eq!(audio_compatibility(Device::Opl3), PIN_COMPAT_OPL3);
}

#[test]
fn audio_numbers_are_per_device_and_ignore_the_content_table() {
    let (a, b, c) = (
        audio_compatibility(Device::Opl3),
        audio_compatibility(Device::Opl2),
        audio_compatibility(Device::Speaker),
    );
    assert!(a != b && b != c && a != c);
    for x in [a, b, c] {
        assert_ne!(x, SCHEMA_HASH);
        assert_ne!(x, COMPAT);
    }
    // An audio file refuses a world pack's number and another device's.
    let mut w = pack(TARGET_DOS, a, "MUSIC");
    w.add_stored(TAG_OPL3, 1, 1, b"song");
    let f = w.finish();
    let want = Expect {
        target: TARGET_DOS,
        stem: "MUSIC",
        compat: a,
    };
    assert!(PackView::parse(&f, &want).is_ok());
    for compat in [b, COMPAT] {
        let other = Expect { compat, ..want };
        assert_eq!(
            PackView::parse(&f, &other).unwrap_err(),
            Error::Incompatible
        );
    }
}

#[test]
fn target_and_stem_are_checked() {
    let ok = sample();
    let web = Expect {
        target: TARGET_WEB,
        ..expect()
    };
    assert_eq!(PackView::parse(&ok, &web).unwrap_err(), Error::WrongTarget);
    // A renamed or misplaced file: LIGHT.FAP copied over SHARED.FAP, or the other way.
    for stem in ["LIGHT", "SHARE", "SHAREDXX", "shared", ""] {
        let other = Expect { stem, ..expect() };
        assert_eq!(
            PackView::parse(&ok, &other).unwrap_err(),
            Error::Stem,
            "{stem:?}"
        );
    }
    // Zero padding is as good as space padding.
    let zero_padded = edited(|f| f[30..32].fill(0));
    assert!(parse(&zero_padded).is_ok());
    // Anything after the padding, or a space inside the stem, is refused.
    assert_eq!(parse(&edited(|f| f[31] = b'X')).unwrap_err(), Error::Stem);
    assert_eq!(parse(&edited(|f| f[26] = b' ')).unwrap_err(), Error::Stem);
}

#[test]
fn section_checksums_are_checked_on_every_access() {
    // Damage a section's stored byte and reseal the file: the whole-file checksum passes, the section's does not.
    let ok = sample();
    let e = parse(&ok).unwrap().find(TAG_OPL3, 0).unwrap();
    let at = e.offset as usize;
    let bad = edited(|f| f[at] ^= 1);
    let p = parse(&bad).unwrap();
    assert_eq!(p.stored(&e), Err(Error::SectionChecksum));
    assert_eq!(p.stored_in_place(&e), Err(Error::SectionChecksum));
    assert_eq!(p.decode(&e, &mut [0u8; 64]), Err(Error::SectionChecksum));
    // The same through a damaged directory checksum.
    let bad = edited(|f| f[ent(3) + 20] ^= 1);
    let p = parse(&bad).unwrap();
    assert_eq!(
        p.stored(&p.find(TAG_OPL3, 0).unwrap()),
        Err(Error::SectionChecksum)
    );
    // Damage to a compressed section is caught before the deflate stream is touched.
    let m = parse(&ok).unwrap().find(TAG_MUSC, 0).unwrap();
    let bad = edited(|f| f[m.offset as usize + 2] ^= 0x10);
    assert_eq!(
        parse(&bad).unwrap().decode(&m, &mut [0u8; 1000]),
        Err(Error::SectionChecksum)
    );
}

#[test]
fn a_loader_can_read_one_section_of_a_big_file() {
    let f = sample();
    let head = &f[..HEADER_LEN + 4 * ENTRY_LEN];
    let p = PackView::parse_head(head, f.len() as u64, &expect()).unwrap();
    let e = p.find(TAG_MUSC, 0).unwrap();
    // The head view has no section bytes.
    assert_eq!(p.stored(&e), Err(Error::Truncated));
    // The loader reads the section itself and decodes it; the whole-file checksum is never looked at.
    let stored = &f[e.offset as usize..(e.offset + e.stored_len) as usize];
    let mut out = vec![0u8; e.decoded_len as usize];
    assert_eq!(e.decode_stored(stored, &mut out), Ok(out.len()));
    assert_eq!(out, b"song song song song song song ".repeat(20));
    let mut damaged = stored.to_vec();
    damaged[1] ^= 1;
    assert_eq!(
        e.decode_stored(&damaged, &mut out),
        Err(Error::SectionChecksum)
    );
    assert_eq!(
        e.decode_stored(&stored[1..], &mut out),
        Err(Error::Truncated)
    );
    // A damaged file whose head is intact passes parse_head (the check belongs to the section) but not parse.
    let mut bad = f.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(PackView::parse_head(&bad[..head.len()], bad.len() as u64, &expect()).is_ok());
    assert_eq!(parse(&bad).unwrap_err(), Error::BadChecksum);
    // The head still has to agree with the real length of the file.
    assert_eq!(
        PackView::parse_head(head, f.len() as u64 + 16, &expect()).unwrap_err(),
        Error::LengthMismatch
    );
    assert_eq!(
        PackView::parse_head(&head[..40], f.len() as u64, &expect()).unwrap_err(),
        Error::Truncated
    );
}

#[test]
fn tags_sort_bytewise_not_as_numbers() {
    // As little-endian numbers "ABZZ" is greater than "BAAA"; bytewise it is smaller, and the writer and reader use bytewise.
    let (lo, hi) = (u32::from_le_bytes(*b"ABZZ"), u32::from_le_bytes(*b"BAAA"));
    assert!(lo > hi);
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
    w.add_stored(hi, 1, 0, b"hi").add_stored(lo, 1, 0, b"lo");
    let mut f = w.finish();
    let p = parse(&f).unwrap();
    assert_eq!((p.entry(0).tag, p.entry(1).tag), (lo, hi));
    // The reverse order is refused.
    let (a, b) = (get_u32(&f, ent(0)), get_u32(&f, ent(1)));
    put_u32(&mut f, ent(0), b);
    put_u32(&mut f, ent(1), a);
    seal(&mut f);
    assert_eq!(parse(&f).unwrap_err(), Error::Unsorted);
    // The effect-set tag sorts before LEVL, the speaker tag after OPL3.
    assert!(TAG_FXPK.swap_bytes() < TAG_LEVL.swap_bytes());
    assert!(TAG_SPKR.swap_bytes() > TAG_OPL3.swap_bytes());
}

#[test]
fn placeholders_and_limits() {
    let mut w = pack(TARGET_DOS, COMPAT, STEM);
    w.add_placeholder(TAG_OPL3, 1)
        .add_stored(TAG_LEVL, 1, 2, &level_bytes());
    let f = w.finish();
    let p = parse(&f).unwrap();
    let ph = p.find(TAG_OPL3, 0).unwrap();
    assert!(ph.is_placeholder() && ph.stored_len == 0);
    assert_eq!(p.stored(&ph), Ok(&[][..]));
    assert!(!p.find(TAG_LEVL, 0).unwrap().is_placeholder());
    // The per-section limit.
    assert_eq!(SECTION_LIMIT, 96 * 1024);
    assert_eq!(p.check_limit(SECTION_LIMIT), Ok(()));
    assert_eq!(p.check_limit(100), Err(Error::TooLarge));
}

#[test]
fn the_file_length_is_a_multiple_of_16() {
    // A claimed length that is not a multiple of 16 is refused even when it matches the bytes.
    let mut f = sample();
    assert!(f.len().is_multiple_of(16));
    f.push(0);
    let n = f.len() as u32;
    f[16..20].copy_from_slice(&n.to_le_bytes());
    seal(&mut f);
    assert_eq!(parse(&f).unwrap_err(), Error::LengthMismatch);
}

#[test]
fn level_count_must_match_the_directory() {
    let b = level_bytes();
    assert!(LevelView::with_count(&b, 2).is_ok());
    assert_eq!(
        LevelView::with_count(&b, 3).unwrap_err(),
        LevelError::CountMismatch
    );
}

#[test]
fn level_layout_is_the_spec_layout() {
    let b = level_bytes();
    // Spawn at 12 and 20, the reserved word at 28.
    assert_eq!(b[12..20], 11.7f64.to_le_bytes());
    assert_eq!(b[20..28], 2.0f64.to_le_bytes());
    assert_eq!(b[28..32], [0; 4]);
}

#[test]
fn level_content_refusals() {
    let b = level_bytes();
    let bad = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut v = b.clone();
        edit(&mut v);
        LevelView::new(&v).unwrap_err()
    };
    let put = |v: &mut Vec<u8>, at: usize, x: f64| v[at..at + 8].copy_from_slice(&x.to_le_bytes());
    assert_eq!(bad(&|v| v[0..2].fill(0)), LevelError::BadSize);
    assert_eq!(bad(&|v| v[2..4].fill(0)), LevelError::BadSize);
    assert_eq!(bad(&|v| v[9] = 49), LevelError::BadName);
    assert_eq!(bad(&|v| v[32] = 0x07), LevelError::BadName);
    assert_eq!(bad(&|v| v[32] = 0x80), LevelError::BadName);
    assert_eq!(bad(&|v| v[39] = b'x'), LevelError::BadName); // padding not zero
    assert_eq!(bad(&|v| put(v, 12, f64::NAN)), LevelError::BadPosition);
    assert_eq!(bad(&|v| put(v, 12, 12.0)), LevelError::BadPosition);
    assert_eq!(bad(&|v| put(v, 20, -0.5)), LevelError::BadPosition);
    // Object 0 (a snack) starts at 40 and object 1 (the chicken) at 72: kind, sub, flags, channel, x, y, p.
    assert_eq!(bad(&|v| v[40] = 0), LevelError::BadKind);
    assert_eq!(bad(&|v| v[40] = 8), LevelError::BadKind);
    assert_eq!(bad(&|v| v[73] = 1), LevelError::BadKind); // sub on a chicken
    assert_eq!(bad(&|v| v[42] = 1), LevelError::BadField);
    assert_eq!(bad(&|v| put(v, 48, 5.0 + 7.0)), LevelError::BadPosition);
    assert_eq!(bad(&|v| put(v, 64, f64::INFINITY)), LevelError::BadPosition);
}
