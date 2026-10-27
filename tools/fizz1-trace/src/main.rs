// Produces golden traces from Lieutenant Fizz 1's own simulation for the scenarios in `crates/fizz-ep2/tests/fizz1`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Reads each `.scn` scenario, builds a headless Fizz 1 world from it in the style of the `arena()` helper in Fizz 1's
//! `tests.rs`, steps it with the scripted buttons and writes the bit-exact `.trace` beside it. See `README.md` for the formats.
//!
//! The tool only reads Fizz 1. It refuses to run unless the checkout is at the pinned commit with a clean tree.

use lf_episode_1::ents::{Ent, Item, ItemKind, Kind, St, info};
use lf_episode_1::tiles::*;
use lf_episode_1::world::{Mode, Player, World, input};
use lf_sim::{Body, TileMap, hashf};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The Fizz 1 commit the traces are generated from.
const PINNED: &str = "4bce036";
const DEFAULT_FIZZ1: &str = "/home/scott/source/liminal-hq/lieutenant-fizz";
/// The first line of every trace file.
const TRACE_MAGIC: &str = "FIZZ1 TRACE 1";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fizz1-trace: {e}");
            ExitCode::FAILURE
        }
    }
}

fn git(fizz1: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(fizz1)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        return Err(format!("git {} failed in {fizz1}", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Refuses to continue unless Fizz 1 is at the pinned commit and has no uncommitted change.
fn check_fizz1() -> Result<(), String> {
    let fizz1 = std::env::var("FIZZ1").unwrap_or_else(|_| DEFAULT_FIZZ1.to_string());
    let head = git(&fizz1, &["rev-parse", "--short", "HEAD"])?;
    if head != PINNED {
        return Err(format!(
            "Fizz 1 at {fizz1} is at {head}, not the pinned {PINNED}"
        ));
    }
    let status = git(&fizz1, &["status", "--porcelain"])?;
    if !status.is_empty() {
        return Err(format!(
            "Fizz 1 at {fizz1} has uncommitted changes:\n{status}"
        ));
    }
    Ok(())
}

struct Scenario {
    name: String,
    /// Tile ids, row 0 at the top as written.
    rows: Vec<Vec<u8>>,
    start: (f64, f64),
    invulnerable: bool,
    gloops: Vec<(f64, f64, f64)>,
    keys: Vec<(f64, f64)>,
    /// Run-length input: ticks and the held button bits.
    script: Vec<(u32, u32)>,
}

fn tile_for(c: char) -> Option<u8> {
    Some(match c {
        '.' => EMPTY,
        '#' => FILL,
        'B' => BLOCK,
        '=' => PLAT,
        '^' => SPIKE,
        '~' => CHOC,
        '/' => R45,
        '\\' => L45,
        'a' => R22A,
        'b' => R22B,
        'c' => L22A,
        'd' => L22B,
        'D' => DOOR_R,
        _ => return None,
    })
}

fn buttons(word: &str) -> Result<u32, String> {
    if word == "-" {
        return Ok(0);
    }
    let mut bits = 0;
    for c in word.chars() {
        bits |= match c {
            'L' => input::LEFT,
            'R' => input::RIGHT,
            'U' => input::UP,
            'D' => input::DOWN,
            'J' => input::JUMP,
            'P' => input::POGO,
            'F' => input::FIRE,
            _ => return Err(format!("unknown button '{c}' in '{word}'")),
        };
    }
    Ok(bits)
}

fn number(word: Option<&str>, what: &str) -> Result<f64, String> {
    let word = word.ok_or_else(|| format!("missing {what}"))?;
    word.parse::<f64>()
        .map_err(|_| format!("bad {what} '{word}'"))
}

fn parse(text: &str) -> Result<Scenario, String> {
    let mut s = Scenario {
        name: String::new(),
        rows: Vec::new(),
        start: (0.0, 0.0),
        invulnerable: false,
        gloops: Vec::new(),
        keys: Vec::new(),
        script: Vec::new(),
    };
    #[derive(PartialEq)]
    enum Block {
        Top,
        Map,
        Input,
    }
    let mut block = Block::Top;
    let mut have_start = false;
    for (n, raw) in text.lines().enumerate() {
        let at = |e: String| format!("line {}: {e}", n + 1);
        let line = raw.trim_end();
        if line.trim_start().starts_with('#') && block != Block::Map || line.trim().is_empty() {
            continue;
        }
        if line == "end" {
            block = Block::Top;
            continue;
        }
        match block {
            Block::Map => {
                let row = line
                    .chars()
                    .map(|c| tile_for(c).ok_or_else(|| at(format!("unknown tile letter '{c}'"))))
                    .collect::<Result<Vec<u8>, String>>()?;
                if s.rows.first().is_some_and(|r| r.len() != row.len()) {
                    return Err(at("map rows differ in width".into()));
                }
                s.rows.push(row);
            }
            Block::Input => {
                let mut it = line.split_whitespace();
                let ticks = it.next().and_then(|t| t.parse::<u32>().ok());
                let word = it.next();
                let (Some(ticks), Some(word), None) = (ticks, word, it.next()) else {
                    return Err(at(format!("bad input line '{line}'")));
                };
                s.script.push((ticks, buttons(word).map_err(at)?));
            }
            Block::Top => {
                let mut it = line.split_whitespace();
                match it.next() {
                    Some("scenario") => s.name = it.next().unwrap_or_default().to_string(),
                    Some("map") => block = Block::Map,
                    Some("input") => block = Block::Input,
                    Some("start") => {
                        s.start = (
                            number(it.next(), "start x").map_err(at)?,
                            number(it.next(), "start y").map_err(at)?,
                        );
                        have_start = true;
                    }
                    Some("invulnerable") => s.invulnerable = true,
                    Some("gloop") => s.gloops.push((
                        number(it.next(), "gloop x").map_err(at)?,
                        number(it.next(), "gloop y").map_err(at)?,
                        number(it.next(), "gloop direction").map_err(at)?,
                    )),
                    Some("key") => match it.next() {
                        Some("red") => s.keys.push((
                            number(it.next(), "key x").map_err(at)?,
                            number(it.next(), "key y").map_err(at)?,
                        )),
                        other => return Err(at(format!("unknown key colour {other:?}"))),
                    },
                    other => return Err(at(format!("unknown directive {other:?}"))),
                }
            }
        }
    }
    if s.name.is_empty() || s.rows.is_empty() || !have_start || s.script.is_empty() {
        return Err("scenario needs a name, a map, a start and an input script".into());
    }
    Ok(s)
}

/// Builds the headless world: the `arena()` pattern from Fizz 1's `tests.rs`, with the map and entities of the scenario.
fn build(s: &Scenario) -> World {
    let mut w = World::new();
    w.game_new();
    w.enter_level(lf_episode_1::levels::CRATER);
    w.events.clear();
    w.ents.clear();
    w.items.clear();
    w.plats.clear();
    w.rooms.clear();
    w.room_alpha.clear();
    w.hazards.clear();
    w.static_lights.clear();
    w.arena = None;
    let (width, height) = (s.rows[0].len() as i32, s.rows.len() as i32);
    w.map = TileMap::new(width, height, props());
    for (i, row) in s.rows.iter().enumerate() {
        for (x, &t) in row.iter().enumerate() {
            w.map.set(x as i32, height - 1 - i as i32, t);
        }
    }
    w.p = Player::level(s.start.0, s.start.1);
    if s.invulnerable {
        w.p.inv = 1e9;
    }
    w.cam_x = s.start.0;
    w.cam_y = s.start.1;
    w.pcx = w.cam_x;
    w.pcy = w.cam_y;
    for &(x, y, dir) in &s.gloops {
        w.ents.push(gloop(x, y, dir));
    }
    for &(x, y) in &s.keys {
        w.items.push(Item {
            kind: ItemKind::KeyRed,
            x,
            y,
            taken: false,
        });
    }
    w
}

/// A Gloop as Fizz 1's `init_ent` builds one (that function is crate-private).
fn gloop(x: f64, y: f64, dir: f64) -> Ent {
    let d = info(Kind::Gloop);
    let b = Body::new(x - d.w / 2.0 + 0.5, y, d.w, d.h);
    Ent {
        kind: Kind::Gloop,
        dir,
        stun: 0.0,
        t: hashf(x as i32, 7) * 10.0,
        cd: 0.0,
        fire_t: 1.5,
        state: St::Idle,
        st: 0.0,
        ax: b.x,
        ay: b.y,
        b,
        rot: 0.0,
        alpha: 1.0,
        hp: 0,
        puff: false,
        touch: false,
        dead: false,
        stray: 0.0,
        ride: false,
    }
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// One trace row for the world as it stands.
fn row(out: &mut String, tick: u32, w: &World) {
    let (p, b) = (&w.p, &w.p.b);
    let _ = write!(
        out,
        "{tick} {} {} {} {} {} {} {} {} {} {} {:08x}",
        hex(b.x),
        hex(b.y),
        hex(b.vx),
        hex(b.vy),
        p.face as i32,
        hex(p.squash),
        u8::from(p.pogo),
        u8::from(b.on_ground),
        u8::from(p.dead > 0.0),
        u8::from(w.keys_red),
        w.map.switches,
    );
    if let Some(g) = w.ents.iter().find(|e| e.kind == Kind::Gloop) {
        let _ = write!(
            out,
            " | {} {} {} {} {} {} {}",
            hex(g.b.x),
            hex(g.b.y),
            hex(g.b.vx),
            hex(g.b.vy),
            g.dir as i32,
            hex(g.stun),
            u8::from(g.dead),
        );
    }
    out.push('\n');
}

/// Runs the scenario and returns the trace text. Panics (through `Err`) if automatic mantle starts.
fn trace(s: &Scenario) -> Result<String, String> {
    let mut w = build(s);
    if w.mode != Mode::Level {
        return Err("world is not in level mode".into());
    }
    let mut out = String::new();
    let _ = writeln!(out, "{TRACE_MAGIC}");
    let _ = writeln!(out, "scenario {}", s.name);
    let _ = writeln!(out, "commit {PINNED}");
    let mut tick = 0;
    row(&mut out, tick, &w);
    for &(ticks, held) in &s.script {
        for _ in 0..ticks {
            w.step(held);
            tick += 1;
            if w.p.mantle.is_some() {
                return Err(format!(
                    "scenario {}: automatic mantle started at tick {tick}; the script must stay clear of ledges",
                    s.name
                ));
            }
            // The queues only ever grow in a headless run; nothing reads them.
            w.events.clear();
            row(&mut out, tick, &w);
        }
    }
    Ok(out)
}

fn scenario_dir(args: &[String]) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    match args.iter().position(|a| a == "--dir") {
        Some(i) => PathBuf::from(args.get(i + 1).map_or("", String::as_str)),
        None => manifest.join("../../crates/fizz-ep2/tests/fizz1"),
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = scenario_dir(&args);
    let check = args.iter().any(|a| a == "--check");
    let mut names: Vec<String> = Vec::new();
    let mut skip = false;
    for a in &args {
        if skip {
            skip = false;
        } else if a == "--dir" {
            skip = true;
        } else if !a.starts_with("--") {
            names.push(a.clone());
        }
    }
    if args.iter().any(|a| a == "--all") {
        let mut found = Vec::new();
        for e in
            std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?
        {
            let path = e.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|x| x == "scn") {
                found.push(
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        found.sort();
        names = found;
    }
    if names.is_empty() {
        return Err("usage: fizz1-trace [--dir <dir>] [--check] (--all | <name>...)".into());
    }
    check_fizz1()?;
    let mut stale = Vec::new();
    for name in &names {
        let scn = dir.join(format!("{name}.scn"));
        let text = std::fs::read_to_string(&scn)
            .map_err(|e| format!("cannot read {}: {e}", scn.display()))?;
        let s = parse(&text).map_err(|e| format!("{}: {e}", scn.display()))?;
        if s.name != *name {
            return Err(format!(
                "{}: scenario name '{}' does not match the file name",
                scn.display(),
                s.name
            ));
        }
        let text = trace(&s)?;
        let target = dir.join(format!("{name}.trace"));
        if check {
            if std::fs::read_to_string(&target).ok().as_deref() != Some(text.as_str()) {
                stale.push(name.clone());
            }
        } else {
            std::fs::write(&target, &text)
                .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
            println!(
                "{name}: {} ticks, {} bytes",
                text.lines().count() - 3 - 1,
                text.len()
            );
        }
    }
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!("traces differ from Fizz 1: {}", stale.join(", ")))
    }
}
