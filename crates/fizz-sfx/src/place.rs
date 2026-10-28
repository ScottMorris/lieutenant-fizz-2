// Positional sound: where a sound source is on screen, as a stereo level per side and a distance factor.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The formula is Fizz 1's (`packages/engine/src/sound-field.ts`, `placeSound` and `FIELD`, read only; pinned 4bce036), moved to whole
// pixels so it is the same on every platform and needs no floating point:
//
//   dx = (x - cam.x) / half.w,  dy = (y - cam.y) / half.h      (cam is the view's centre, half is half the view)
//   d  = max(|dx|, |dy|)
//   pan  = 0.6 * clamp(dx, -1, 1)                               (so the far side is never silent)
//   dist = 1 if d <= 1 else max(0.4, 1 - 0.5 * (d - 1))
//   left, right = cos(a), sin(a) with a = (pan + 1) * PI / 4    (equal power: left^2 + right^2 = 1)
//
// Fizz 1 applies `dist` on top of both sides, so each side's level is `dist * cos(a)` or `dist * sin(a)`.
//
// Why fixed point. The DOS runtime denies float arithmetic and the x87 is not set up inside the audio path, and the result has to be
// the same number on the host test, the Web page and the DOS machine. The position and view are whole pixels, the ratios are Q12
// (4096 is 1.0), the cosine is a 129-entry Q15 table (a step of 1/64 of the full pan range, about 0.07 dB at the loudest slope, far
// finer than the OPL's 0.75 dB step) checked against `f64` in the tests, and the dB step is found by comparing the level with 63
// precomputed thresholds, never by a logarithm. The function allocates nothing and has no state.
//
// View only. This module takes plain integers. It cannot read the rules, and the rules cannot read it: `fizz-ep2` does not depend on
// this crate (a test checks the manifest), so what is heard cannot change the state hash or an `EVENT` line.

/// 1.0 in the Q12 ratios (`dx`, `dy`, pan and the distance factor).
pub const ONE_Q12: i32 = 4096;
/// 1.0 in the Q15 side levels.
pub const ONE_Q15: u32 = 32768;
/// Fizz 1's pan width, 0.6, as the ratio 3 / 5.
const PAN_NUM: i32 = 3;
const PAN_DEN: i32 = 5;
/// Fizz 1's distance floor, 0.4, in Q12 (1638.4 rounded down).
const DIST_FLOOR_Q12: i32 = 1638;

/// The view a source is placed in, in pixels of the map: the top left corner (the camera as the presenter draws with it) and the
/// size. On the Web the size follows the window; on DOS it is 320 by 200 (10 by 6.25 tiles of 16 pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// What a source's place on screen means for the sound: a distance factor and a level for each side, as powers of the amplitude (Q12
/// and Q15). The levels already include the distance, so a player applies `left` and `right` and needs nothing else; `distance` is
/// kept apart for the paths with one output (a mono OPL2, or the Web's gain node).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The pan from -4096 (hard left) to 4096 (hard right) in Q12; Fizz 1's width keeps it within about +-2457.
    pub pan: i16,
    /// The distance factor, 4096 (full) down to 1638 (0.4).
    pub distance: u16,
    /// The left and right levels in Q15, with the distance in them.
    pub left: u16,
    pub right: u16,
}

/// The cosine of `i * PI / 256` for `i` in 0 to 128 in Q15: the left level at pan `(i - 64) / 64`, and the right level at the mirror
/// index `128 - i`.
const COS_Q15: [u16; 129] = [
    32767, 32766, 32758, 32746, 32729, 32706, 32679, 32647, 32610, 32568, 32522, 32470, 32413,
    32352, 32286, 32214, 32138, 32058, 31972, 31881, 31786, 31686, 31581, 31471, 31357, 31238,
    31114, 30986, 30853, 30715, 30572, 30425, 30274, 30118, 29957, 29792, 29622, 29448, 29269,
    29086, 28899, 28707, 28511, 28311, 28106, 27897, 27684, 27467, 27246, 27020, 26791, 26557,
    26320, 26078, 25833, 25583, 25330, 25073, 24812, 24548, 24279, 24008, 23732, 23453, 23170,
    22884, 22595, 22302, 22006, 21706, 21403, 21097, 20788, 20475, 20160, 19841, 19520, 19195,
    18868, 18538, 18205, 17869, 17531, 17190, 16846, 16500, 16151, 15800, 15447, 15091, 14733,
    14373, 14010, 13646, 13279, 12910, 12540, 12167, 11793, 11417, 11039, 10660, 10279, 9896, 9512,
    9127, 8740, 8351, 7962, 7571, 7180, 6787, 6393, 5998, 5602, 5205, 4808, 4410, 4011, 3612, 3212,
    2811, 2411, 2009, 1608, 1206, 804, 402, 0,
];

/// `STEP_THRESHOLD_Q15[n - 1]` is the Q15 level halfway (in dB) between attenuation steps `n - 1` and `n` of 0.75 dB, that is
/// `32768 * 10^(-0.75 * (n - 0.5) / 20)`: a level below it is at least `n` steps down.
const STEP_THRESHOLD_Q15: [u16; 63] = [
    31383, 28787, 26406, 24221, 22218, 20380, 18694, 17147, 15729, 14428, 13234, 12139, 11135,
    10214, 9369, 8594, 7883, 7231, 6633, 6084, 5581, 5119, 4696, 4307, 3951, 3624, 3324, 3049,
    2797, 2566, 2353, 2159, 1980, 1816, 1666, 1528, 1402, 1286, 1180, 1082, 992, 910, 835, 766,
    703, 644, 591, 542, 497, 456, 419, 384, 352, 323, 296, 272, 249, 229, 210, 192, 176, 162, 148,
];

const fn scale(c: u16, distance: u16) -> u16 {
    ((c as u32 * distance as u32) >> 12) as u16
}

/// The OPL total-level attenuation, 0 to 63 steps of 0.75 dB, nearest to a Q15 amplitude level (32768 is full). A level of zero or
/// the quietest the chip can reach gives 63, which is about -47 dB: quiet, not silent.
pub fn attenuation_for_level(level_q15: u32) -> u8 {
    let mut steps = 0;
    for &t in &STEP_THRESHOLD_Q15 {
        if level_q15 < u32::from(t) {
            steps += 1;
        } else {
            break;
        }
    }
    steps
}

impl Placement {
    /// A source at the middle of the view at full distance, which is also what Classic mode plays: the pan 0 and the distance 1.
    pub const CENTRE: Placement = Placement::from_pan_and_distance(0, ONE_Q12 as u16);

    const fn from_pan_and_distance(pan: i32, distance: u16) -> Placement {
        // Index 0..=128 of the cosine table, rounded to the nearest 1/64 of the pan range.
        let i = ((pan + ONE_Q12 + 32) >> 6) as usize;
        let i = if i > 128 { 128 } else { i };
        Placement {
            pan: pan as i16,
            distance,
            left: scale(COS_Q15[i], distance),
            right: scale(COS_Q15[128 - i], distance),
        }
    }

    /// The attenuation for the left channel of the OPL3 pair (the level of the left side, 0 to 63 steps of 0.75 dB).
    pub fn left_attenuation(&self) -> u8 {
        attenuation_for_level(u32::from(self.left))
    }

    /// The attenuation for the right channel of the OPL3 pair.
    pub fn right_attenuation(&self) -> u8 {
        attenuation_for_level(u32::from(self.right))
    }

    /// The attenuation for a single output with no pan (an OPL2, or an OPL3 on the AdLib role): the distance only, so a source in
    /// view is at the full level the effect was authored at.
    pub fn mono_attenuation(&self) -> u8 {
        attenuation_for_level(u32::from(self.distance) << 3)
    }
}

/// Places a sound source at (`x`, `y`) in map pixels (y down, as the camera) inside `view`. A view with no area gives
/// [`Placement::CENTRE`]. The arithmetic is 64-bit, so no coordinate overflows.
pub fn place(x: i32, y: i32, view: View) -> Placement {
    if view.w <= 0 || view.h <= 0 {
        return Placement::CENTRE;
    }
    // The ratio of the source's offset from the view's centre to half the view, in Q12, truncated toward zero.
    let ratio = |p: i32, origin: i32, size: i32| -> i32 {
        let offset = 2 * (i64::from(p) - i64::from(origin)) - i64::from(size);
        (offset * i64::from(ONE_Q12) / i64::from(size)).clamp(-(1 << 24), 1 << 24) as i32
    };
    let dx = ratio(x, view.x, view.w);
    let dy = ratio(y, view.y, view.h);
    let d = dx.abs().max(dy.abs());
    let distance = if d <= ONE_Q12 {
        ONE_Q12
    } else {
        (ONE_Q12 - (d - ONE_Q12) / 2).max(DIST_FLOOR_Q12)
    };
    let pan = dx.clamp(-ONE_Q12, ONE_Q12) * PAN_NUM / PAN_DEN;
    Placement::from_pan_and_distance(pan, distance as u16)
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    const V: View = View {
        x: 100,
        y: 50,
        w: 320,
        h: 200,
    };

    /// Fizz 1's `placeSound` in `f64`, the oracle.
    fn fizz1(x: f64, y: f64, v: View) -> (f64, f64, f64, f64) {
        let dx = (x - (f64::from(v.x) + f64::from(v.w) / 2.0)) / (f64::from(v.w) / 2.0);
        let dy = (y - (f64::from(v.y) + f64::from(v.h) / 2.0)) / (f64::from(v.h) / 2.0);
        let d = dx.abs().max(dy.abs());
        let pan = 0.6 * dx.clamp(-1.0, 1.0);
        let gain = if d <= 1.0 {
            1.0
        } else {
            (1.0 - 0.5 * (d - 1.0)).max(0.4)
        };
        let a = (pan.clamp(-1.0, 1.0) + 1.0) * core::f64::consts::PI / 4.0;
        (pan, gain, a.cos() * gain, a.sin() * gain)
    }

    #[test]
    fn the_cosine_table_is_the_rounded_cosine() {
        for (i, &c) in COS_Q15.iter().enumerate() {
            let want = ((i as f64 * core::f64::consts::PI / 256.0).cos() * 32768.0)
                .round()
                .min(32767.0);
            assert_eq!(f64::from(c), want, "entry {i}");
        }
    }

    #[test]
    fn the_step_thresholds_are_half_way_in_decibels() {
        for (i, &t) in STEP_THRESHOLD_Q15.iter().enumerate() {
            let n = (i + 1) as f64;
            let want = (32768.0 * 10f64.powf(-0.75 * (n - 0.5) / 20.0)).round();
            assert_eq!(f64::from(t), want, "threshold {n}");
        }
        assert!(STEP_THRESHOLD_Q15.windows(2).all(|w| w[0] > w[1]));
    }

    /// The dB table the docs print: step, level and the dB it stands for.
    #[test]
    fn attenuation_steps_are_075_db_and_monotone() {
        assert_eq!(attenuation_for_level(32768), 0);
        assert_eq!(attenuation_for_level(0), 63);
        let mut last = 0;
        for level in (0..=32768u32).rev().step_by(7) {
            let a = attenuation_for_level(level);
            assert!(a >= last);
            last = a;
        }
        // Every step lands within half a step (0.375 dB) of the true decibels, over the range the placement reaches.
        for level in (3000u32..=32768).step_by(13) {
            let db = -20.0 * (f64::from(level) / 32768.0).log10();
            let err = (f64::from(attenuation_for_level(level)) * 0.75 - db).abs();
            assert!(err <= 0.38, "level {level}: {err} dB off");
        }
    }

    #[test]
    fn centre_is_both_sides_equal_about_minus_three_decibels() {
        let p = place(100 + 160, 50 + 100, V);
        assert_eq!((p.pan, p.distance), (0, 4096));
        assert_eq!(p.left, p.right);
        assert_eq!(p.left, 23170);
        assert_eq!((p.left_attenuation(), p.right_attenuation()), (4, 4));
        assert_eq!(p.mono_attenuation(), 0);
        assert_eq!(Placement::CENTRE, p);
    }

    #[test]
    fn the_far_side_is_at_most_about_minus_ten_decibels_and_never_silent() {
        // The edges of the view: pan is +-0.6 (Fizz 1's width), so the quiet side is sin(0.1 PI) = 0.309 and the loud side 0.951.
        let left_edge = place(100, 150, V);
        let right_edge = place(100 + 320, 150, V);
        assert_eq!(left_edge.pan, -2457);
        assert_eq!(right_edge.pan, 2457);
        assert_eq!(
            (left_edge.left, left_edge.right),
            (right_edge.right, right_edge.left)
        );
        assert!(
            (31000..=31300).contains(&left_edge.left),
            "{}",
            left_edge.left
        );
        assert!(
            (10000..=10300).contains(&left_edge.right),
            "{}",
            left_edge.right
        );
        // 0.4 dB down on the near side, 10.2 dB down on the far side: 1 step and 13 steps (9.75 dB).
        assert_eq!(
            (left_edge.left_attenuation(), left_edge.right_attenuation()),
            (1, 13)
        );
        assert_eq!(
            (
                right_edge.left_attenuation(),
                right_edge.right_attenuation()
            ),
            (13, 1)
        );
        // Even far outside the view the pan does not go past the width and the distance stops at 0.4.
        let far = place(100 - 100_000, 150, V);
        assert_eq!(far.pan, -2457);
        assert_eq!(far.distance, 1638);
        assert!(far.right > 0 && far.right_attenuation() < 63);
    }

    #[test]
    fn distance_starts_outside_the_view_and_stops_at_a_floor() {
        // Inside the view (also the corners) it is full. One more half-width out on the x axis is d = 2 and 1 - 0.5 = 0.5.
        assert_eq!(place(100, 50, V).distance, 4096);
        assert_eq!(place(100 + 320 + 160, 150, V).distance, 2048);
        assert_eq!(place(260, 50 + 200 + 100, V).distance, 2048);
        assert_eq!(place(100 + 320 + 10_000, 150, V).distance, 1638);
        // 0.5 is -6 dB, which is 8 steps; the floor 0.4 is -8 dB, 11 steps.
        assert_eq!(place(100 + 320 + 160, 150, V).mono_attenuation(), 8);
        assert_eq!(place(100 + 320 + 10_000, 150, V).mono_attenuation(), 11);
    }

    #[test]
    fn it_follows_fizz_1s_formula_within_the_table_and_step_error() {
        for v in [
            V,
            View {
                x: 0,
                y: 0,
                w: 320,
                h: 200,
            },
            View {
                x: 33,
                y: 7,
                w: 1280,
                h: 208,
            },
            View {
                x: 500,
                y: 0,
                w: 96,
                h: 208,
            },
        ] {
            for x in (v.x - 3 * v.w..v.x + 4 * v.w).step_by(37) {
                for y in (v.y - 2 * v.h..v.y + 3 * v.h).step_by(41) {
                    let p = place(x, y, v);
                    let (pan, gain, l, r) = fizz1(f64::from(x), f64::from(y), v);
                    assert!((f64::from(p.pan) / 4096.0 - pan).abs() < 0.001);
                    assert!((f64::from(p.distance) / 4096.0 - gain).abs() < 0.001);
                    assert!(
                        (f64::from(p.left) / 32768.0 - l).abs() < 0.006,
                        "({x},{y}) left"
                    );
                    assert!(
                        (f64::from(p.right) / 32768.0 - r).abs() < 0.006,
                        "({x},{y}) right"
                    );
                    // Equal power before the distance.
                    let power = (f64::from(p.left).powi(2) + f64::from(p.right).powi(2)).sqrt()
                        / f64::from(p.distance)
                        / 8.0;
                    assert!((power - 1.0).abs() < 0.003, "power {power}");
                }
            }
        }
    }

    #[test]
    fn the_view_follows_the_window_so_the_same_source_pans_less_in_a_wider_view() {
        // A source 200 px right of the view's centre: 1.25 half-widths in a 320 view, 0.3125 in a 1280 view.
        let narrow = place(
            160 + 200,
            100,
            View {
                x: 0,
                y: 0,
                w: 320,
                h: 200,
            },
        );
        let wide = place(
            640 + 200,
            100,
            View {
                x: 0,
                y: 0,
                w: 1280,
                h: 200,
            },
        );
        assert_eq!(narrow.pan, 2457);
        assert!(wide.pan > 0 && wide.pan < narrow.pan);
        assert!(narrow.distance < 4096 && wide.distance == 4096);
    }

    #[test]
    fn degenerate_views_and_huge_coordinates_do_not_panic() {
        for v in [
            View {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            },
            View {
                x: 0,
                y: 0,
                w: -5,
                h: 10,
            },
        ] {
            assert_eq!(place(10, 10, v), Placement::CENTRE);
        }
        for x in [i32::MIN, i32::MAX, 0] {
            for y in [i32::MIN, i32::MAX, 0] {
                let p = place(
                    x,
                    y,
                    View {
                        x: i32::MAX,
                        y: i32::MIN,
                        w: 1,
                        h: 1,
                    },
                );
                assert!(p.distance >= 1638 && p.pan.unsigned_abs() <= 2457);
            }
        }
    }

    /// The placement is a function of integers only, so it cannot reach the rules, and the rules must not reach it. A run of the
    /// slice replay with a placement worked out for every cluck, in a different view each time, gives the same transcript and
    /// hash as the golden file, and the rules crate does not depend on any view or sound crate.
    #[test]
    fn placing_sounds_never_changes_the_rules_the_hash_or_the_events() {
        use fizz_ep2::replay::{SLICE_SCRIPT, write_final, write_start, write_step};
        let golden = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fizz-ep2/slice-replay.txt"
        ))
        .unwrap();

        let mut world = fizz_ep2::level::world();
        let mut out = std::string::String::new();
        write_start(&mut out, &world).unwrap();
        let mut tick = 0u32;
        let mut placed = 0;
        let mut prev = fizz_sim::Buttons::NONE;
        for &(ticks, buttons) in SLICE_SCRIPT {
            for _ in 0..ticks {
                fizz_ep2::step(
                    &mut world,
                    fizz_sim::InputFrame::from_transition(prev, buttons),
                );
                prev = buttons;
                for e in world.events() {
                    if let fizz_ep2::Event::Cluck { chicken, .. } = *e {
                        // The presenter's read: the chicken's position after the step and a view of its own choosing.
                        let c = &world.chickens[usize::from(chicken)];
                        let view = View {
                            x: (tick as i32 * 7) % 500,
                            y: 0,
                            w: 96 + (tick as i32 % 13) * 90,
                            h: 208,
                        };
                        let p = place((c.b.x * 16.0) as i32, 200 - (c.b.y * 16.0) as i32, view);
                        assert!(p.distance >= 1638);
                        placed += 1;
                    }
                }
                write_step(&mut out, &world).unwrap();
                tick += 1;
            }
        }
        write_final(&mut out, &world).unwrap();
        assert!(placed > 0, "the script has clucks");
        assert_eq!(
            out, golden,
            "the transcript, hash and EVENT lines are the golden ones whatever the view"
        );

        let manifest = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fizz-ep2/Cargo.toml"
        ))
        .unwrap();
        for forbidden in ["fizz-sfx", "fizz-view", "fizz-slice-view", "fizz-present"] {
            assert!(
                !manifest.contains(forbidden),
                "the rules crate must not depend on {forbidden}"
            );
        }
    }
}
