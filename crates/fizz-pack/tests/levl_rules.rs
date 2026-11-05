// LEVL rule and string records: round trips, the reader's refusals, boundaries and a deflate round trip through the pack.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use fizz_pack::level::*;
use fizz_pack::pack::{CODEC_DEFLATE, TAG_LEVL, TARGET_DOS};
use fizz_pack::write::{LevelSpec, PackWriter, encode_level};
use fizz_pack::{Expect, LevelView, ObjectRec, PackView, RuleRec, StringRec, compatibility};

const W: u16 = 40;
const H: u16 = 12;

fn obj(kind: u8, sub: u8) -> ObjectRec {
    ObjectRec {
        kind,
        sub,
        flags: 0,
        channel: 0,
        x_bits: 3.5f64.to_bits(),
        y_bits: 4.5f64.to_bits(),
        p_bits: 0,
    }
}

/// Objects: 0 snack, 1 chicken, 2 red key.
fn objects() -> [ObjectRec; 3] {
    [
        obj(KIND_SNACK, 1),
        obj(KIND_CHICKEN, 0),
        obj(KIND_KEY_RED, 0),
    ]
}

fn encode(rules: &[RuleRec], strings: &[StringRec]) -> Vec<u8> {
    let tiles = vec![0u8; usize::from(W) * usize::from(H)];
    encode_level(&LevelSpec {
        width: W,
        height: H,
        switches: 0b10,
        floor_solid: false,
        spawn_x_bits: 1.5f64.to_bits(),
        spawn_y_bits: 1.5f64.to_bits(),
        name: "Rules",
        objects: &objects(),
        rules,
        strings,
        tiles: &tiles,
    })
}

fn rec(trigger: u8) -> RuleRec {
    RuleRec {
        trigger,
        ..RuleRec::EMPTY
    }
}

fn enter() -> RuleRec {
    let (u0, u1) = RuleRec::union_rect(1, 2, 14, 2);
    RuleRec {
        u0,
        u1,
        op: OP_ON,
        channel: 1,
        cell_a: (15, 7),
        effect: 19,
        caption: CUE_LEVEL_BASE,
        cell_b: (15, 7),
        delay: 30,
        ..rec(TRIGGER_ENTER)
    }
}

fn lock() -> RuleRec {
    RuleRec {
        subject: 0,
        op: OP_OFF,
        channel: 1,
        cell_a: (5, 5),
        ..rec(TRIGGER_LOCK)
    }
}

fn timer() -> RuleRec {
    RuleRec {
        u0: 60,
        u1: 90,
        flags: FLAG_REPEAT | FLAG_GUARD | FLAG_GUARD_ON,
        guard_ch: 31,
        op: OP_TOGGLE,
        channel: 2,
        cell_a: (0, 0),
        ..rec(TRIGGER_TIME)
    }
}

fn strings() -> [StringRec; 2] {
    [
        StringRec::new(2, "slam!"),
        StringRec::new(6, "Mind the gap \u{2026}"),
    ]
}

fn every_trigger() -> Vec<RuleRec> {
    let (u0, u1) = RuleRec::union_rect(0, 0, 39, 11);
    vec![
        lock(),
        rec(TRIGGER_START),
        enter(),
        RuleRec {
            subject: 2,
            ..rec(TRIGGER_TAKE)
        },
        RuleRec {
            u0,
            u1,
            ..rec(TRIGGER_SHOOT)
        },
        RuleRec {
            subject: SUBJECT_ANY,
            ..rec(TRIGGER_STOMP)
        },
        RuleRec {
            subject: 1,
            ..rec(TRIGGER_STUN)
        },
        timer(),
    ]
}

fn refuse(rules: &[RuleRec]) -> LevelError {
    LevelView::new(&encode(rules, &strings())).unwrap_err()
}

fn accepts(rules: &[RuleRec]) -> bool {
    LevelView::new(&encode(rules, &strings())).is_ok()
}

fn pad(b: &[u8]) -> [u8; STRING_TEXT_MAX] {
    let mut t = [0; STRING_TEXT_MAX];
    t[..b.len()].copy_from_slice(b);
    t
}

#[test]
fn rules_and_strings_round_trip() {
    let rules = every_trigger();
    let strs = strings();
    let b = encode(&rules, &strs);
    // header, name padded to 8, three objects, rules, strings, tiles
    assert_eq!(b.len(), 32 + 8 + 32 * (3 + 8 + 2) + 480);
    assert_eq!((b[28], b[29], b[30], b[31]), (8, 2, 0, 0));
    let v = LevelView::new(&b).unwrap();
    assert_eq!((v.rule_count(), v.string_count()), (8, 2));
    assert_eq!(v.rules().collect::<Vec<_>>(), rules);
    assert_eq!(v.strings().collect::<Vec<_>>(), strs.to_vec());
    assert_eq!(v.object_count(), 3);
    assert_eq!(v.tiles().len(), 480);
    assert_eq!(v.string(1).as_str(), "Mind the gap \u{2026}");
    assert_eq!(v.rule(2).rect(), (1, 2, 14, 2));
    assert_eq!(v.rule(7).time(), (60, 90));
    assert!(v.rule(7).repeat() && v.rule(7).guard() && v.rule(7).guard_on());
    assert_eq!(RuleRec::from_bytes(&rules[2].to_bytes()), rules[2]);
    assert_eq!(StringRec::from_bytes(&strs[0].to_bytes()), strs[0]);
}

#[test]
fn record_byte_layout() {
    let b = encode(&[enter()], &[StringRec::new(2, "slam!")]);
    let r = 32 + 8 + 3 * 32;
    assert_eq!(b[r..r + 4], [TRIGGER_ENTER, 0, 0, 0]);
    assert_eq!(b[r + 4..r + 12], [1, 0, 2, 0, 14, 0, 2, 0]);
    assert_eq!(b[r + 12..r + 14], 30u16.to_le_bytes());
    assert_eq!(b[r + 14..r + 16], [OP_ON, 1]);
    assert_eq!(b[r + 16..r + 20], [15, 0, 7, 0]);
    assert_eq!(b[r + 20..r + 22], [19, CUE_LEVEL_BASE]);
    assert_eq!(b[r + 22..r + 26], [15, 0, 7, 0]);
    assert_eq!(b[r + 26..r + 32], [0; 6]);
    let s = r + 32;
    assert_eq!(b[s..s + 8], [5, 2, b's', b'l', b'a', b'm', b'!', 0]);
}

#[test]
fn boundary_counts() {
    // 32 rules and 32 strings pass.
    let rules: Vec<RuleRec> = (0..32).map(|_| rec(TRIGGER_START)).collect();
    let strs: Vec<StringRec> = (0..32).map(|i| StringRec::new(i % 7, "x")).collect();
    let b = encode(&rules, &strs);
    let v = LevelView::new(&b).unwrap();
    assert_eq!((v.rule_count(), v.string_count()), (32, 32));
    // A header that claims 33 of either is refused before the length is looked at.
    let mut bad = b.clone();
    bad[28] = 33;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::BadRule);
    let mut bad = b.clone();
    bad[29] = 33;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::BadString);
    // Counts that disagree with the length are a size error.
    let mut bad = b.clone();
    bad[28] = 31;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::TrailingBytes);
    let mut bad = b;
    bad[29] = 0;
    assert_eq!(LevelView::new(&bad).unwrap_err(), LevelError::TrailingBytes);
    // The reserved word at 30 must be zero.
    let mut b = encode(&[], &[]);
    b[31] = 1;
    assert_eq!(LevelView::new(&b).unwrap_err(), LevelError::BadField);
}

#[test]
fn max_width_is_not_checked_here() {
    let extras = "\u{2026}\u{2014}\u{2013}\u{00B7}\u{2018}\u{2019}\u{201C}\u{201D}";
    assert!(extras.len() <= STRING_TEXT_MAX);
    let wide = "W".repeat(30);
    let b = encode(&[], &[StringRec::new(0, &wide), StringRec::new(0, extras)]);
    assert!(LevelView::new(&b).is_ok());
}

#[test]
fn string_refusals() {
    let ok = |s: StringRec| LevelView::new(&encode(&[], &[s])).is_ok();
    let bad =
        |s: StringRec| LevelView::new(&encode(&[], &[s])).err() == Some(LevelError::BadString);
    // 30 bytes, 1 byte and each charset edge pass.
    assert!(ok(StringRec::new(0, &"a".repeat(30))));
    assert!(ok(StringRec::new(6, " ")));
    assert!(ok(StringRec::new(0, "~")));
    for c in STRING_EXTRAS {
        let s = char::from_u32(c).unwrap().to_string();
        assert!(ok(StringRec::new(0, &s)), "{c:#x}");
    }
    // Empty, over long, style, charset, UTF-8, padding.
    let raw = StringRec::new(0, "ab");
    assert!(bad(StringRec { len: 0, ..raw }));
    assert!(bad(StringRec { len: 31, ..raw }));
    assert!(bad(StringRec {
        style: STYLE_COUNT,
        ..raw
    }));
    assert!(ok(StringRec {
        style: STYLE_COUNT - 1,
        ..raw
    }));
    assert!(bad(StringRec {
        len: 1,
        text: pad(&[0x1F]),
        ..raw
    }));
    assert!(bad(StringRec {
        len: 1,
        text: pad(&[0x7F]),
        ..raw
    }));
    assert!(bad(StringRec::new(0, "\u{00E9}"))); // e acute is not in the font
    assert!(bad(StringRec::new(0, "\u{2022}"))); // a bullet is not in the extras
    assert!(bad(StringRec {
        len: 2,
        text: pad(&[0xC3, 0x28]),
        ..raw
    })); // invalid UTF-8
    assert!(bad(StringRec {
        len: 1,
        text: pad(&[0xC3]),
        ..raw
    })); // a cut sequence
    assert!(bad(StringRec {
        len: 1,
        text: pad(b"ab"),
        ..raw
    })); // padding that is not zero
    assert!(bad(StringRec {
        len: 1,
        text: pad(&[0]),
        ..raw
    })); // NUL is outside the charset
}

#[test]
fn rule_refusals() {
    let bad = LevelError::BadRule;
    // Triggers.
    for t in [0, 9, 10, 11, 255] {
        assert_eq!(refuse(&[rec(t)]), bad, "trigger {t}");
    }
    // Flags.
    assert_eq!(
        refuse(&[RuleRec {
            flags: 8,
            ..rec(TRIGGER_START)
        }]),
        bad
    );
    assert_eq!(
        refuse(&[RuleRec {
            flags: FLAG_GUARD_ON,
            ..rec(TRIGGER_START)
        }]),
        bad
    );
    // Guard channel: below 32 with a guard, zero without.
    assert_eq!(
        refuse(&[RuleRec {
            flags: FLAG_GUARD,
            guard_ch: 32,
            ..rec(TRIGGER_START)
        }]),
        bad
    );
    assert_eq!(
        refuse(&[RuleRec {
            guard_ch: 3,
            ..rec(TRIGGER_START)
        }]),
        bad
    );
    // Subjects: take needs a snack or key, stomp and stun a chicken or any, the rest zero.
    let subj = |t, s| RuleRec {
        subject: s,
        ..rec(t)
    };
    assert!(accepts(&[
        subj(TRIGGER_TAKE, 0),
        subj(TRIGGER_TAKE, 2),
        subj(TRIGGER_STOMP, 1),
        subj(TRIGGER_STUN, SUBJECT_ANY)
    ]));
    assert_eq!(refuse(&[subj(TRIGGER_TAKE, 1)]), bad); // a chicken
    assert_eq!(refuse(&[subj(TRIGGER_TAKE, 3)]), bad); // past the objects
    assert_eq!(refuse(&[subj(TRIGGER_TAKE, SUBJECT_ANY)]), bad);
    assert_eq!(refuse(&[subj(TRIGGER_STOMP, 0)]), bad); // a snack
    assert_eq!(refuse(&[subj(TRIGGER_STUN, 9)]), bad);
    assert_eq!(refuse(&[subj(TRIGGER_START, 1)]), bad);
    assert_eq!(refuse(&[subj(TRIGGER_TIME, 1)]), bad);
    assert_eq!(
        refuse(&[RuleRec {
            subject: SUBJECT_ANY,
            ..lock()
        }]),
        bad
    );
    // Rect: inside the map, not inverted.
    let rect = |x0, y0, x1, y1| {
        let (u0, u1) = RuleRec::union_rect(x0, y0, x1, y1);
        RuleRec {
            u0,
            u1,
            ..rec(TRIGGER_ENTER)
        }
    };
    assert!(accepts(&[rect(0, 0, W - 1, H - 1), rect(7, 7, 7, 7)]));
    assert_eq!(refuse(&[rect(0, 0, W, 0)]), bad);
    assert_eq!(refuse(&[rect(0, 0, 0, H)]), bad);
    assert_eq!(refuse(&[rect(5, 0, 4, 0)]), bad);
    assert_eq!(refuse(&[rect(0, 5, 0, 4)]), bad);
    // A union on a trigger that has none.
    let (r0, r1) = RuleRec::union_rect(0, 0, 3, 3);
    for t in [
        TRIGGER_START,
        TRIGGER_TAKE,
        TRIGGER_STOMP,
        TRIGGER_STUN,
        TRIGGER_LOCK,
    ] {
        let r = RuleRec {
            u0: r0,
            u1: r1,
            ..if t == TRIGGER_LOCK { lock() } else { rec(t) }
        };
        assert_eq!(refuse(&[r]), bad, "trigger {t}");
    }
    // Time: at >= 1, every 0 or >= MIN_PERIOD, REPEAT iff every != 0.
    let time = |at, every, flags| RuleRec {
        u0: at,
        u1: every,
        flags,
        ..rec(TRIGGER_TIME)
    };
    assert!(accepts(&[
        time(1, 0, 0),
        time(1, MIN_PERIOD, FLAG_REPEAT),
        time(u32::MAX, u32::MAX, FLAG_REPEAT)
    ]));
    assert_eq!(refuse(&[time(0, 0, 0)]), bad);
    assert_eq!(refuse(&[time(1, MIN_PERIOD - 1, FLAG_REPEAT)]), bad);
    assert_eq!(refuse(&[time(1, 1, FLAG_REPEAT)]), bad);
    assert_eq!(refuse(&[time(1, 0, FLAG_REPEAT)]), bad);
    assert_eq!(refuse(&[time(1, 10, 0)]), bad);
    // Slot A: op, channel, cell, and an empty slot is all zero.
    let a = |op, channel, cell_a| RuleRec {
        op,
        channel,
        cell_a,
        ..rec(TRIGGER_START)
    };
    assert!(accepts(&[a(OP_TOGGLE, 31, (W - 1, H - 1))]));
    assert_eq!(refuse(&[a(4, 0, (0, 0))]), bad);
    assert_eq!(refuse(&[a(OP_ON, 32, (0, 0))]), bad);
    assert_eq!(refuse(&[a(OP_ON, 0, (W, 0))]), bad);
    assert_eq!(refuse(&[a(OP_ON, 0, (0, H))]), bad);
    assert_eq!(refuse(&[a(OP_NONE, 1, (0, 0))]), bad);
    assert_eq!(refuse(&[a(OP_NONE, 0, (1, 0))]), bad);
    // Slot B: caption codes, a cell inside the map when used, zero when empty.
    let b = |effect, caption, cell_b| RuleRec {
        effect,
        caption,
        cell_b,
        ..rec(TRIGGER_START)
    };
    assert!(accepts(&[
        b(CUE_NONE, 0x3F, (1, 1)),
        b(0, CUE_NONE, (0, 0)),
        b(CUE_NONE, CUE_LEVEL_BASE + 1, (1, 1)),
        b(255 - 1, CUE_NONE, (W - 1, H - 1)),
    ]));
    assert_eq!(refuse(&[b(CUE_NONE, CUE_LEVEL_BASE + 2, (1, 1))]), bad); // only two strings
    assert_eq!(refuse(&[b(CUE_NONE, 0x7F, (1, 1))]), bad);
    assert_eq!(refuse(&[b(CUE_NONE, 0xFE, (1, 1))]), bad);
    assert_eq!(refuse(&[b(3, CUE_NONE, (W, 0))]), bad);
    assert_eq!(refuse(&[b(CUE_NONE, CUE_NONE, (1, 0))]), bad);
    // A level string reference when the level has no strings.
    assert_eq!(
        LevelView::new(&encode(&[b(CUE_NONE, CUE_LEVEL_BASE, (1, 1))], &[])).unwrap_err(),
        bad
    );
    // Reserved bytes.
    let mut raw = encode(&[rec(TRIGGER_START)], &[]);
    raw[32 + 8 + 3 * 32 + 31] = 1;
    assert_eq!(LevelView::new(&raw).unwrap_err(), bad);
}

#[test]
fn lock_shape() {
    let bad = LevelError::BadRule;
    assert!(accepts(&[lock()]));
    assert_eq!(
        refuse(&[RuleRec {
            op: OP_ON,
            ..lock()
        }]),
        bad
    );
    assert_eq!(
        refuse(&[RuleRec {
            op: OP_NONE,
            channel: 0,
            cell_a: (0, 0),
            ..lock()
        }]),
        bad
    );
    assert_eq!(refuse(&[RuleRec { delay: 1, ..lock() }]), bad);
    assert_eq!(
        refuse(&[RuleRec {
            flags: FLAG_GUARD,
            guard_ch: 2,
            ..lock()
        }]),
        bad
    );
    assert_eq!(
        refuse(&[RuleRec {
            flags: FLAG_REPEAT,
            ..lock()
        }]),
        bad
    );
}

#[test]
fn rules_in_a_pack_survive_deflate() {
    let rules = every_trigger();
    let strs = strings();
    let b = encode(&rules, &strs);
    let compat = compatibility(b"t");
    let mut w = PackWriter::new(TARGET_DOS, compat, "LIGHT");
    w.add_compressed(TAG_LEVL, 1, 3, &b);
    let f = w.finish();
    let want = Expect {
        target: TARGET_DOS,
        stem: "LIGHT",
        compat,
    };
    let p = PackView::parse(&f, &want).unwrap();
    let e = p.find(TAG_LEVL, 0).unwrap();
    assert_eq!(e.codec, CODEC_DEFLATE, "a level of zero tiles compresses");
    let mut out = vec![0u8; e.decoded_len as usize];
    let n = p.decode(&e, &mut out).unwrap();
    assert_eq!(&out[..n], &b[..]);
    let v = LevelView::with_count(&out[..n], 3).unwrap();
    assert_eq!(v.rules().collect::<Vec<_>>(), rules);
    assert_eq!(v.strings().collect::<Vec<_>>(), strs.to_vec());
}
