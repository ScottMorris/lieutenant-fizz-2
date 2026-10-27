// Replays the shared Fizz 1 scenarios through Fizz 2's Ben and compares every tick with the golden traces, bit for bit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each `tests/fizz1/<name>.scn` is parsed (the format is described in `tools/fizz1-trace/README.md`), run through
//! [`fizz_ep2::Ben`] and written out in the trace row format; the rows must equal the committed `<name>.trace` exactly. The
//! tolerance is zero: positions, velocities and squash are compared as `f64` bit patterns.
//!
//! The scenarios with a Gloop, a key or the Fizz Blaster need entities this package does not have yet; their tests are
//! ignored with the package that brings them.

use fizz_ep2::Ben;
use fizz_phys::tilemap::{
    ONEWAY, SLOPE_L22A, SLOPE_L22B, SLOPE_L45, SLOPE_R22A, SLOPE_R22B, SLOPE_R45, SOLID,
};
use fizz_phys::{TileMap, TileProps};
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

fn props() -> TileProps {
    let mut p = TileProps::default();
    for t in [1, 2, 12] {
        p.set_flags(t, SOLID);
    }
    p.set_flags(3, ONEWAY);
    for (t, s) in [
        (6, SLOPE_R45),
        (7, SLOPE_L45),
        (8, SLOPE_R22A),
        (9, SLOPE_R22B),
        (10, SLOPE_L22A),
        (11, SLOPE_L22B),
    ] {
        p.set_slope(t, s);
    }
    p
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
    entities: bool,
    script: Vec<(u32, Buttons)>,
}

fn parse(text: &str) -> Scenario {
    let mut name = String::new();
    let mut rows: Vec<Vec<u8>> = Vec::new();
    let mut start = (0.0, 0.0);
    let (mut invulnerable, mut entities) = (false, false);
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
                    Some("gloop") | Some("key") => entities = true,
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
        entities,
        script,
    }
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// One trace row, in the field order of the trace format, for Ben alone.
fn row(tick: u32, ben: &Ben, map: &TileMap) -> String {
    let mut out = String::new();
    let b = &ben.b;
    let _ = write!(
        out,
        "{tick} {} {} {} {} {} {} {} {} {} 0 {:08x}",
        hex(b.x),
        hex(b.y),
        hex(b.vx),
        hex(b.vy),
        ben.face as i32,
        hex(ben.squash),
        u8::from(ben.pogo),
        u8::from(b.on_ground),
        u8::from(ben.dead > 0.0),
        map.switches,
    );
    out
}

fn simulate(s: &Scenario) -> Vec<String> {
    let mut ben = Ben::new(s.start.0, s.start.1);
    if s.invulnerable {
        ben.inv = 1e9;
    }
    let mut prev = Buttons::NONE;
    let mut rows = vec![row(0, &ben, &s.map)];
    let mut tick = 0;
    for &(ticks, held) in &s.script {
        for _ in 0..ticks {
            ben.tick(&s.map, &[], InputFrame::from_transition(prev, held));
            prev = held;
            tick += 1;
            rows.push(row(tick, &ben, &s.map));
        }
    }
    rows
}

const FIELDS: [&str; 12] = [
    "tick", "x", "y", "vx", "vy", "face", "squash", "pogo", "ground", "dead", "keys_red",
    "switches",
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
        // Ignore the Gloop part of a golden row; scenarios here have no entity.
        let w = w.split(" | ").next().unwrap();
        if g != w {
            let field = g
                .split(' ')
                .zip(w.split(' '))
                .position(|(a, b)| a != b)
                .map_or("?", |i| FIELDS[i]);
            panic!("{name}: first difference at tick {n}, field {field}\n  got  {g}\n  want {w}");
        }
    }
    assert!(!s.entities, "{name}: needs entities this package lacks");
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
);

#[test]
#[ignore = "P4: needs the Gloop (chicken) and the Fizz Blaster shots"]
fn gloop_patrol() {
    check("gloop_patrol");
}

#[test]
#[ignore = "P4: needs the Gloop (chicken) and stomping"]
fn stomp() {
    check("stomp");
}

#[test]
#[ignore = "P4: needs the Gloop (chicken) and stomping"]
fn stomp_pogo() {
    check("stomp_pogo");
}

#[test]
#[ignore = "P4: needs the Gloop (chicken) and the Fizz Blaster shots"]
fn fizz_stun() {
    check("fizz_stun");
}

#[test]
#[ignore = "P4: needs the red key, the door and world flags"]
fn key_door() {
    check("key_door");
}

#[test]
#[ignore = "P4: needs spike hazards and the slice level's contacts"]
fn route() {
    check("route");
}
