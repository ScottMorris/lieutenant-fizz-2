// Prints the register writes the DOS audio service makes for one cluck, as `tick bank reg val` lines (the listening clips and the
// cost figures in docs/SFX.md read them, so they come from the real player, not a copy of it).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   cargo run -q -p fizz-sfx --example stream -- <variant> pair <left-attenuation> <right-attenuation>
//   cargo run -q -p fizz-sfx --example stream -- <variant> mono <attenuation>
//   cargo run -q -p fizz-sfx --example stream -- <variant> at <x> <y> <view-x> <view-y> <view-w> <view-h>   (pair, placed)
use fizz_sfx::place::{View, place};
use fizz_sfx::{OplPair, OplPlayer};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let n = |i: usize| a[i].parse::<i32>().expect("a number");
    let variant = n(0) as usize;
    let mut tick = 0u32;
    match a[1].as_str() {
        "mono" => {
            let mut p = OplPlayer::mono(variant, n(2) as u8);
            while p.tick(&mut |b, r, v| println!("{tick} {b} {r} {v}")) {
                tick += 1;
            }
        }
        mode => {
            let (l, r) = if mode == "pair" {
                (n(2) as u8, n(3) as u8)
            } else {
                let view = View {
                    x: n(4),
                    y: n(5),
                    w: n(6),
                    h: n(7),
                };
                let p = place(n(2), n(3), view);
                eprintln!("{p:?} attenuation {} {}", p.left_attenuation(), p.right_attenuation());
                (p.left_attenuation(), p.right_attenuation())
            };
            let mut p = OplPair::same(variant, l, r);
            while p.tick(&mut |b, r, v| println!("{tick} {b} {r} {v}")) {
                tick += 1;
            }
        }
    }
}
