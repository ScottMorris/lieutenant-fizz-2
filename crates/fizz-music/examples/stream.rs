// Prints the register writes the music player makes, as `tick bank reg val` lines (the renderer and the cost figures in docs/MUSIC.md
// read them, so they come from the real player, not a copy of it).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   cargo run -q -p fizz-music --example stream -- <opl2|opl3> [ticks]
//
// `ticks` defaults to one pass (the prelude and the loop); a larger number plays on round the loop.
use fizz_music::{Card, MusicPlayer, Write, level_song};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let card = match a.first().map(String::as_str) {
        Some("opl2") => Card::Opl2,
        Some("opl3") => Card::Opl3,
        _ => panic!("usage: stream <opl2|opl3> [ticks]"),
    };
    let arrangement = level_song().arrangement(card);
    let ticks = a.get(1).map_or(arrangement.pass_ticks(), |t| {
        t.parse().expect("a tick count")
    });
    let mut player = MusicPlayer::new(arrangement);
    let mut buf = [Write::default(); 32];
    for tick in 0..ticks {
        let n = player.tick(&mut buf);
        for w in &buf[..n] {
            println!("{tick} {} {} {}", w.bank, w.reg, w.val);
        }
    }
}
