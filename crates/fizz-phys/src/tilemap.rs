// Tile map with per-tile solidity, one-way, switchable and slope properties.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from Fizz 1 `crates/sim/src/tilemap.rs` at commit `4bce036`; transformed as in fizz-dos `m2/patches/sim.patch`.
// Edits: `use alloc::{vec, vec::Vec};` added; imports ordered by rustfmt for the 2024 edition.

use alloc::{vec, vec::Vec};

/// Tile solid from every side.
pub const SOLID: u8 = 1;
/// Tile solid only when landing on it from above (one-way platform).
pub const ONEWAY: u8 = 2;
/// Tile solid only while its switch channel is on (switchable bridge, gate).
pub const SWITCHED: u8 = 4;
/// Solid tile that carries a body standing on it to the left.
pub const CONVEY_L: u8 = 8;
/// Solid tile that carries a body standing on it to the right.
pub const CONVEY_R: u8 = 16;
/// Tile a body can climb. Combine with [`ONEWAY`] for a ladder top that can be stood on.
pub const LADDER: u8 = 32;

/// A surface a body can free-climb in any direction (a crystal vine on a cave wall). Unlike a
/// ladder it does not snap the body to a column.
pub const CLIMB: u8 = 64;

/// Speed (tiles per second) at which a conveyor tile carries a body standing on it.
pub const CONVEYOR_SPEED: f64 = 3.0;

/// Slope shapes, mapped from tile ids through [`TileProps`].
pub const SLOPE_R45: u8 = 1;
pub const SLOPE_L45: u8 = 2;
pub const SLOPE_R22A: u8 = 3;
pub const SLOPE_R22B: u8 = 4;
pub const SLOPE_L22A: u8 = 5;
pub const SLOPE_L22B: u8 = 6;

/// Surface height (0..1 within the tile) of a slope shape at local x `lx` (0..1).
/// 22.5 degree slopes span two tiles: the A half rises 0..0.5, the B half 0.5..1.
pub fn slope_height(shape: u8, lx: f64) -> f64 {
    match shape {
        SLOPE_R45 => lx,
        SLOPE_L45 => 1.0 - lx,
        SLOPE_R22A => lx / 2.0,
        SLOPE_R22B => 0.5 + lx / 2.0,
        SLOPE_L22A => 1.0 - lx / 2.0,
        SLOPE_L22B => 0.5 - lx / 2.0,
        _ => 0.0,
    }
}

/// Per-tile-id behaviour table: collision flags, slope shape and switch channel.
#[derive(Clone, Debug)]
pub struct TileProps {
    flags: [u8; 256],
    slope: [u8; 256],
    channel: [u8; 256],
}

impl Default for TileProps {
    fn default() -> Self {
        TileProps {
            flags: [0; 256],
            slope: [0; 256],
            channel: [0; 256],
        }
    }
}

impl TileProps {
    pub fn set_flags(&mut self, id: u8, flags: u8) {
        self.flags[id as usize] = flags;
    }

    pub fn set_slope(&mut self, id: u8, shape: u8) {
        self.slope[id as usize] = shape;
    }

    /// Assigns a [`SWITCHED`] tile id to one of the map's 32 switch channels.
    pub fn set_channel(&mut self, id: u8, channel: u8) {
        debug_assert!(channel < 32, "switch channel out of range");
        self.channel[id as usize] = channel;
    }

    pub fn channel(&self, id: u8) -> u8 {
        self.channel[id as usize]
    }

    pub fn flags(&self, id: u8) -> u8 {
        self.flags[id as usize]
    }

    pub fn slope(&self, id: u8) -> u8 {
        self.slope[id as usize]
    }
}

/// A rectangular tile grid. Row 0 is the bottom of the world (y grows upwards).
#[derive(Clone, Debug)]
pub struct TileMap {
    pub w: i32,
    pub h: i32,
    pub data: Vec<u8>,
    pub props: TileProps,
    /// One bit per switch channel; a [`SWITCHED`] tile is solid while its channel's bit is set.
    pub switches: u32,
    /// What lies below row 0: solid for the overworld (a hard edge), open for levels (a pit).
    pub floor_solid: bool,
}

impl TileMap {
    pub fn new(w: i32, h: i32, props: TileProps) -> Self {
        TileMap {
            w,
            h,
            data: vec![0; (w * h) as usize],
            props,
            switches: 0,
            floor_solid: false,
        }
    }

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.w && y >= 0 && y < self.h
    }

    /// Tile id at (x, y); 0 outside the map.
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if self.in_bounds(x, y) {
            self.data[(y * self.w + x) as usize]
        } else {
            0
        }
    }

    pub fn set(&mut self, x: i32, y: i32, t: u8) {
        if self.in_bounds(x, y) {
            self.data[(y * self.w + x) as usize] = t;
        }
    }

    /// Whether a switch channel is on.
    pub fn switch(&self, channel: u8) -> bool {
        self.switches & (1 << channel) != 0
    }

    pub fn set_switch(&mut self, channel: u8, on: bool) {
        if on {
            self.switches |= 1 << channel;
        } else {
            self.switches &= !(1 << channel);
        }
    }

    /// Flips a switch channel and returns its new state.
    pub fn toggle_switch(&mut self, channel: u8) -> bool {
        self.switches ^= 1 << channel;
        self.switch(channel)
    }

    /// Horizontal speed (tiles per second, negative is left) that the tile at (x, y) imparts to a
    /// body standing on it; 0 for anything that is not a conveyor.
    pub fn drift_at(&self, cx: i32, cy: i32) -> f64 {
        let f = self.props.flags(self.get(cx, cy));
        if f & CONVEY_R != 0 {
            CONVEYOR_SPEED
        } else if f & CONVEY_L != 0 {
            -CONVEYOR_SPEED
        } else {
            0.0
        }
    }

    /// Whether the tile at (x, y) can be climbed.
    pub fn has_ladder(&self, cx: i32, cy: i32) -> bool {
        self.props.flags(self.get(cx, cy)) & LADDER != 0
    }

    /// Whether the tile at (x, y) is a free-climb surface such as a vine.
    pub fn has_climb(&self, cx: i32, cy: i32) -> bool {
        self.props.flags(self.get(cx, cy)) & CLIMB != 0
    }

    pub fn is_slope(&self, t: u8) -> bool {
        self.props.slope(t) != 0
    }

    /// True for tiles that are solid from every side regardless of state.
    pub fn is_solid_tile(&self, t: u8) -> bool {
        self.props.flags(t) & SOLID != 0
    }

    /// Whether the cell blocks movement. Outside the left/right edge is a wall, above the top
    /// is open and below the bottom follows `floor_solid`. `down` and `prev_bottom` enable
    /// one-way landing: only solid when the body was entirely above the tile last tick.
    pub fn solid(&self, cx: i32, cy: i32, down: bool, prev_bottom: f64) -> bool {
        if cx < 0 || cx >= self.w {
            return true;
        }
        if cy < 0 {
            return false;
        }
        if cy >= self.h {
            return self.floor_solid;
        }
        let id = self.data[(cy * self.w + cx) as usize];
        let f = self.props.flags(id);
        if f & SOLID != 0 {
            return true;
        }
        if f & SWITCHED != 0 {
            return self.switch(self.props.channel(id));
        }
        if f & ONEWAY != 0 {
            return down && prev_bottom >= f64::from(cy) + 1.0 - 1e-6;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props() -> TileProps {
        let mut p = TileProps::default();
        p.set_flags(1, SOLID);
        p.set_flags(2, ONEWAY);
        p.set_flags(3, SWITCHED);
        p.set_slope(4, SLOPE_R45);
        p.set_flags(5, SWITCHED);
        p.set_channel(5, 1);
        p.set_flags(6, SOLID | CONVEY_R);
        p.set_flags(7, SOLID | CONVEY_L);
        p.set_flags(8, LADDER);
        p.set_flags(9, LADDER | ONEWAY);
        p.set_flags(10, CLIMB);
        p
    }

    #[test]
    fn slope_heights_are_continuous() {
        assert_eq!(slope_height(SLOPE_R22A, 1.0), slope_height(SLOPE_R22B, 0.0));
        assert_eq!(slope_height(SLOPE_L22A, 1.0), slope_height(SLOPE_L22B, 0.0));
        assert_eq!(slope_height(SLOPE_R45, 0.25), 0.25);
        assert_eq!(slope_height(SLOPE_L45, 0.25), 0.75);
    }

    #[test]
    fn solidity_rules() {
        let mut m = TileMap::new(4, 4, props());
        m.set(1, 1, 1);
        m.set(2, 1, 2);
        m.set(3, 1, 3);
        assert!(m.solid(1, 1, false, 0.0));
        assert!(m.solid(-1, 0, false, 0.0), "side edges are walls");
        assert!(!m.solid(0, -1, false, 0.0), "pit below");
        assert!(!m.solid(0, 9, false, 0.0), "open above");
        assert!(!m.solid(2, 1, false, 0.0));
        assert!(m.solid(2, 1, true, 2.0), "one-way solid from above");
        assert!(!m.solid(2, 1, true, 1.5), "one-way open from inside");
        assert!(!m.solid(3, 1, false, 0.0));
        m.set_switch(0, true);
        assert!(m.solid(3, 1, false, 0.0));
        m.floor_solid = true;
        assert!(m.solid(0, 9, false, 0.0));
    }

    #[test]
    fn switch_channels_are_independent() {
        let mut m = TileMap::new(4, 4, props());
        m.set(0, 1, 3);
        m.set(1, 1, 5);
        assert!(!m.solid(0, 1, false, 0.0) && !m.solid(1, 1, false, 0.0));
        m.set_switch(1, true);
        assert!(!m.solid(0, 1, false, 0.0), "channel 0 tile stays open");
        assert!(m.solid(1, 1, false, 0.0), "channel 1 tile closes");
        assert!(m.switch(1) && !m.switch(0));
        assert!(m.toggle_switch(0));
        assert!(m.solid(0, 1, false, 0.0));
        assert!(!m.toggle_switch(0));
        assert!(!m.solid(0, 1, false, 0.0));
        m.set_switch(1, false);
        assert!(!m.solid(1, 1, false, 0.0));
    }

    #[test]
    fn conveyors_drift_in_their_direction_and_nothing_else_does() {
        let mut m = TileMap::new(4, 4, props());
        m.set(0, 0, 6);
        m.set(1, 0, 7);
        m.set(2, 0, 1);
        assert_eq!(m.drift_at(0, 0), CONVEYOR_SPEED);
        assert_eq!(m.drift_at(1, 0), -CONVEYOR_SPEED);
        assert_eq!(m.drift_at(2, 0), 0.0);
        assert_eq!(m.drift_at(-5, 9), 0.0, "outside the map");
        assert!(m.solid(0, 0, false, 0.0), "a conveyor is also solid");
    }

    #[test]
    fn vines_are_climbable_but_not_solid_and_not_ladders() {
        let mut m = TileMap::new(4, 4, props());
        m.set(1, 1, 10);
        assert!(m.has_climb(1, 1));
        assert!(!m.has_ladder(1, 1));
        assert!(!m.has_climb(0, 0));
        assert!(!m.solid(1, 1, true, 5.0), "a vine never blocks");
    }

    #[test]
    fn ladders_are_found_and_a_ladder_top_is_one_way() {
        let mut m = TileMap::new(4, 4, props());
        m.set(0, 1, 8);
        m.set(1, 2, 9);
        assert!(m.has_ladder(0, 1) && m.has_ladder(1, 2));
        assert!(!m.has_ladder(2, 2));
        assert!(!m.solid(0, 1, true, 5.0), "a plain ladder never blocks");
        assert!(m.solid(1, 2, true, 3.0), "a ladder top can be landed on");
        assert!(!m.solid(1, 2, false, 3.0), "and is open from inside");
    }
}
