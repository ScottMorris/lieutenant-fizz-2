// Replays the shared Fizz 1 scenarios through Fizz 2's Ben and compares every tick with the golden traces, bit for bit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each `tests/fizz1/<name>.scn` is parsed (the format is described in `tools/fizz1-trace/README.md`), run through
//! [`fizz_ep2::World`] and written out in the trace row format; the rows must equal the committed `<name>.trace` exactly.
//! The tolerance is zero: positions, velocities, squash and the Gloop's stun are compared as `f64` bit patterns.
//!
//! The scenarios run through the whole world, so the chicken, the shots, the red key and door and the hazards are proved
//! by the same traces as Ben's movement, and every field a trace carries is compared, including the look timers, the aim and the first Gloop's. One
//! departure is masked: Fizz 1 rewrites the red door's tile when it opens and never touches the switch word, while Fizz 2
//! keeps the door as a switched tile on its own channel, so that one bit is left out of the `switches` comparison (see
//! `tiles::DOOR_RED_BIT`).

use fizz_ep2::tiles::{DOOR_RED_BIT, props};
use fizz_ep2::{World, WorldFlags};
use fizz_phys::TileMap;
use fizz_sim::{Buttons, InputFrame};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Fizz 1's tile ids for the letters a scenario may use, with the properties Fizz 1's `props()` gives them.
fn tile(c: char) -> Option<u8> {
    Some(match c {
        '.' => 0,
        '#' => 1,
        'B' => 2,
        '=' => 3,
        '^' => 4,
        '~' => 5,
        '/' => 6,
        '\\' => 7,
        'a' => 8,
        'b' => 9,
        'c' => 10,
        'd' => 11,
        'D' => 12,
        _ => return None,
    })
}

fn buttons(word: &str) -> Buttons {
    if word == "-" {
        return Buttons::NONE;
    }
    word.chars().fold(Buttons::NONE, |acc, c| {
        acc.union(match c {
            'L' => Buttons::LEFT,
            'R' => Buttons::RIGHT,
            'U' => Buttons::UP,
            'D' => Buttons::DOWN,
            'J' => Buttons::JUMP,
            'P' => Buttons::POGO,
            'F' => Buttons::FIRE,
            _ => panic!("unknown button '{c}' in '{word}'"),
        })
    })
}

struct Scenario {
    name: String,
    map: TileMap,
    start: (f64, f64),
    invulnerable: bool,
    gloops: Vec<(f64, f64, f64)>,
    key: Option<(f64, f64)>,
    script: Vec<(u32, Buttons)>,
}

fn parse(text: &str) -> Scenario {
    let mut name = String::new();
    let mut rows: Vec<Vec<u8>> = Vec::new();
    let mut start = (0.0, 0.0);
    let mut invulnerable = false;
    let mut gloops = Vec::new();
    let mut key = None;
    let mut script = Vec::new();
    #[derive(PartialEq)]
    enum Block {
        Top,
        Map,
        Input,
    }
    let mut block = Block::Top;
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with('#') && block != Block::Map || line.trim().is_empty() {
            continue;
        }
        if line == "end" {
            block = Block::Top;
            continue;
        }
        match block {
            Block::Map => rows.push(
                line.chars()
                    .map(|c| tile(c).unwrap_or_else(|| panic!("unknown tile letter '{c}'")))
                    .collect(),
            ),
            Block::Input => {
                let mut it = line.split_whitespace();
                let ticks = it.next().unwrap().parse().unwrap();
                script.push((ticks, buttons(it.next().unwrap())));
            }
            Block::Top => {
                let mut it = line.split_whitespace();
                match it.next() {
                    Some("scenario") => name = it.next().unwrap().to_string(),
                    Some("map") => block = Block::Map,
                    Some("input") => block = Block::Input,
                    Some("start") => {
                        start = (
                            it.next().unwrap().parse().unwrap(),
                            it.next().unwrap().parse().unwrap(),
                        )
                    }
                    Some("invulnerable") => invulnerable = true,
                    Some("gloop") => {
                        let mut n = || it.next().unwrap().parse::<f64>().unwrap();
                        gloops.push((n(), n(), n()));
                    }
                    Some("key") => {
                        assert_eq!(it.next(), Some("red"));
                        let mut n = || it.next().unwrap().parse::<f64>().unwrap();
                        key = Some((n(), n()));
                    }
                    other => panic!("unknown directive {other:?}"),
                }
            }
        }
    }
    let (w, h) = (rows[0].len() as i32, rows.len() as i32);
    let mut map = TileMap::new(w, h, props());
    for (i, row) in rows.iter().enumerate() {
        for (x, &t) in row.iter().enumerate() {
            map.set(x as i32, h - 1 - i as i32, t);
        }
    }
    Scenario {
        name,
        map,
        start,
        invulnerable,
        gloops,
        key,
        script,
    }
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// One trace row, in the field order of the trace format, with the first Gloop's part when there is one.
fn row(tick: u32, w: &World) -> String {
    let mut out = String::new();
    let (ben, b) = (&w.ben, &w.ben.b);
    let _ = write!(
        out,
        "{tick} {} {} {} {} {} {} {} {} {} {} {:08x} {} {} {}",
        hex(b.x),
        hex(b.y),
        hex(b.vx),
        hex(b.vy),
        ben.face as i32,
        hex(ben.squash),
        u8::from(ben.pogo),
        u8::from(b.on_ground),
        u8::from(ben.dead > 0.0),
        u8::from(w.flags.contains(WorldFlags::KEY_RED)),
        w.map.switches & !DOOR_RED_BIT,
        hex(ben.look_up),
        hex(ben.look_down),
        ben.aim,
    );
    if let Some(c) = w.chickens.first() {
        let _ = write!(
            out,
            " | {} {} {} {} {} {} {}",
            hex(c.b.x),
            hex(c.b.y),
            hex(c.b.vx),
            hex(c.b.vy),
            c.dir as i32,
            hex(c.stun),
            u8::from(c.dead),
        );
    }
    out
}

fn simulate(s: &Scenario) -> Vec<String> {
    let mut w = World::new(s.map.clone(), s.start);
    if s.invulnerable {
        w.ben.inv = 1e9;
    }
    for &(x, y, dir) in &s.gloops {
        w.add_chicken(x, y, dir);
    }
    if let Some((x, y)) = s.key {
        w.set_key(x, y);
    }
    let mut prev = Buttons::NONE;
    let mut rows = vec![row(0, &w)];
    let mut tick = 0;
    for &(ticks, held) in &s.script {
        for _ in 0..ticks {
            w.step(InputFrame::from_transition(prev, held));
            prev = held;
            tick += 1;
            rows.push(row(tick, &w));
        }
    }
    rows
}

const FIELDS: [&str; 23] = [
    "tick",
    "x",
    "y",
    "vx",
    "vy",
    "face",
    "squash",
    "pogo",
    "ground",
    "dead",
    "keys_red",
    "switches",
    "look_up",
    "look_down",
    "aim",
    "|",
    "gx",
    "gy",
    "gvx",
    "gvy",
    "gdir",
    "gstun",
    "gdead",
];

fn check(name: &str) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fizz1");
    let scn = std::fs::read_to_string(dir.join(format!("{name}.scn"))).unwrap();
    let golden = std::fs::read_to_string(dir.join(format!("{name}.trace"))).unwrap();
    let s = parse(&scn);
    assert_eq!(s.name, name);
    let want: Vec<&str> = golden.lines().skip(3).collect();
    let got = simulate(&s);
    assert_eq!(got.len(), want.len(), "{name}: tick count differs");
    for (n, (g, w)) in got.iter().zip(&want).enumerate() {
        if g != w {
            let field = g
                .split(' ')
                .zip(w.split(' '))
                .position(|(a, b)| a != b)
                .map_or("?", |i| FIELDS[i]);
            panic!("{name}: first difference at tick {n}, field {field}\n  got  {g}\n  want {w}");
        }
    }
}

macro_rules! traces {
    ($($name:ident),* $(,)?) => {
        $( #[test] fn $name() { check(stringify!($name)); } )*
    };
}

traces!(
    fall,
    run,
    jump_full,
    jump_cut,
    pogo_idle,
    pogo_held,
    pogo_ceiling,
    oneway,
    wall_step,
    slope,
    gloop_patrol,
    stomp,
    stomp_pogo,
    fizz_stun,
    key_door,
    route,
    look,
    aim,
);
