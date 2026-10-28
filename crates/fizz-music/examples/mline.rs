// Prints the `MUSIC` line the DOS build prints for a run of the music, computed on the host by `writes_for` and `count_loops`, so
// `tools/check-dos.sh` can compare the console's line with it byte for byte.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
//   cargo run -q -p fizz-music --example mline -- <opl2|opl3|off> [ticks]
//
// `ticks` defaults to what `/MUSICTEST` steps: the prelude, two full loops and the tick the third pass starts on. `off` prints the line
// of a run with no music: no ticks, no writes, the empty hash.
use fizz_music::{Card, Fnv, count_loops, level_song, writes_for};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let song = level_song();
    let name = a.first().map_or("", String::as_str);
    let card = match name {
        "opl2" => Some(Card::Opl2),
        "opl3" => Some(Card::Opl3),
        "off" => None,
        _ => panic!("usage: mline <opl2|opl3|off> [ticks]"),
    };
    let (ticks, writes, loops, hash) = match card {
        None => (0, 0, 0, Fnv::new().value()),
        Some(card) => {
            let arrangement = song.arrangement(card);
            let ticks = a.get(1).map_or(
                arrangement.prelude_ticks + 2 * arrangement.loop_ticks + 1,
                |t| t.parse().expect("ticks is a number"),
            );
            let (writes, hash) = writes_for(arrangement, ticks);
            (ticks, writes, count_loops(arrangement, ticks), hash)
        }
    };
    println!(
        "MUSIC SONG {} ARR {name} TICKS {ticks} WRITES {writes} LOOPS {loops} HASH {hash:08x}",
        song.id
    );
}
