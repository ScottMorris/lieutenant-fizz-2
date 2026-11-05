// Tile map with per-tile solidity, one-way, switchable and slope properties.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: Derived from Fizz 1 `crates/sim/src/tilemap.rs` at commit `4bce036`; transformed as in fizz-dos `m2/patches/sim.patch`.
// Edits: `use alloc::{vec, vec::Vec};` added; imports ordered by rustfmt for the 2024 edition. A switched tile's channel is no
// longer a property of its tile id: the map binds each switched cell to a bit of `switches` (`bind_switch`), so a level says which
// state a door reads and the tile id carries only the art.

use alloc::{vec, vec::Vec};

/// Tile solid from every side.
pub const SOLID: u8 = 1;
/// Tile solid only when landing on it from above (one-way platform).
pub const ONEWAY: u8 = 2;
/// Tile solid only while the switch bit bound to its cell is on (switchable bridge, gate). A cell with no bit bound is open.
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

/// A cell bound to no switch bit.
const NO_SWITCH: u8 = 0xFF;

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

/// Per-tile-id behaviour table: collision flags and slope shape.
#[derive(Clone, Debug)]
pub struct TileProps {
    flags: [u8; 256],
    slope: [u8; 256],
}

impl Default for TileProps {
    fn default() -> Self {
        TileProps {
            flags: [0; 256],
            slope: [0; 256],
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
    /// One bit per switch; a [`SWITCHED`] tile is solid while the bit bound to its cell is set.
    pub switches: u32,
    /// For each cell, row-major, the switch bit a [`SWITCHED`] tile in it reads, or [`NO_SWITCH`].
    switch_cells: Vec<u8>,
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
            switch_cells: vec![NO_SWITCH; (w * h) as usize],
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

    /// Whether switch bit `bit` is on.
    pub fn switch(&self, bit: u8) -> bool {
        self.switches & (1 << bit) != 0
    }

    pub fn set_switch(&mut self, bit: u8, on: bool) {
        if on {
            self.switches |= 1 << bit;
        } else {
            self.switches &= !(1 << bit);
        }
    }

    /// Flips switch bit `bit` and returns its new state.
    pub fn toggle_switch(&mut self, bit: u8) -> bool {
        self.switches ^= 1 << bit;
        self.switch(bit)
    }

    /// Binds the cell `(x, y)` to switch bit `bit` (below 32): a [`SWITCHED`] tile there is solid while the bit is set. Outside the
    /// map this does nothing.
    pub fn bind_switch(&mut self, x: i32, y: i32, bit: u8) {
        debug_assert!(bit < 32, "switch bit out of range");
        if self.in_bounds(x, y) {
            self.switch_cells[(y * self.w + x) as usize] = bit;
        }
    }

    /// Binds every cell holding tile id `id` to switch bit `bit`.
    pub fn bind_switch_tiles(&mut self, id: u8, bit: u8) {
        debug_assert!(bit < 32, "switch bit out of range");
        for (cell, &t) in self.switch_cells.iter_mut().zip(&self.data) {
            if t == id {
                *cell = bit;
            }
        }
    }

    /// The switch bit bound to the cell `(x, y)`, if there is one.
    pub fn switch_bit(&self, x: i32, y: i32) -> Option<u8> {
        if !self.in_bounds(x, y) {
            return None;
        }
        match self.switch_cells[(y * self.w + x) as usize] {
            NO_SWITCH => None,
            b => Some(b),
        }
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
            return self.switch_bit(cx, cy).is_some_and(|b| self.switch(b));
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
        m.bind_switch(3, 1, 0);
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
    fn switch_bits_are_independent() {
        let mut m = TileMap::new(4, 4, props());
        m.set(0, 1, 3);
        m.set(1, 1, 5);
        m.bind_switch(0, 1, 0);
        m.bind_switch(1, 1, 1);
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
    fn a_switched_tile_reads_the_bit_of_its_cell_and_not_of_its_id() {
        let mut m = TileMap::new(4, 4, props());
        // Two cells of one tile id, bound to different bits; a third, unbound, is open whatever the switches are.
        m.set(0, 1, 3);
        m.set(1, 1, 3);
        m.set(2, 1, 3);
        m.bind_switch(0, 1, 4);
        m.bind_switch(1, 1, 5);
        m.switches = u32::MAX;
        assert!(m.solid(0, 1, false, 0.0) && m.solid(1, 1, false, 0.0));
        assert!(!m.solid(2, 1, false, 0.0), "an unbound cell is open");
        m.set_switch(4, false);
        assert!(!m.solid(0, 1, false, 0.0) && m.solid(1, 1, false, 0.0));
        assert_eq!((m.switch_bit(0, 1), m.switch_bit(2, 1)), (Some(4), None));
        assert_eq!(m.switch_bit(-1, 0), None);
        m.bind_switch(9, 9, 1); // outside the map: nothing happens
        let mut t = TileMap::new(4, 4, props());
        t.set(0, 0, 3);
        t.set(3, 3, 3);
        t.bind_switch_tiles(3, 7);
        assert_eq!((t.switch_bit(0, 0), t.switch_bit(3, 3)), (Some(7), Some(7)));
        assert_eq!(t.switch_bit(1, 1), None);
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
