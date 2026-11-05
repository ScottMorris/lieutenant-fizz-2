// The door bindings of a level: which states its switched tiles read, and the private switch bits the sim derives from them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A level's **door bindings** (`docs/FAP.md`, binding kind 1) in the form the world runs them. A door binding says: the switched
//! tiles in this rectangle are passable while this state's value is in this set. The sim does not read the state in the solidity
//! test; it keeps a **derived bit** for each door binding in `TileMap::switches`, set while the door is closed (the state's value is
//! not in the active set), and binds each switched cell of the rectangle to that bit (`TileMap::bind_switch`). Door binding `i`
//! owns bit [`FIRST_DOOR_BIT`] + `i`. Neither the source nor the bytes name a bit.
//!
//! Until the rule phase exists (`docs/RULES.md`), the world holds no state values: a door's start bit follows from its state's start
//! value, and the lock phase (see `rules.rs`) clears the bit when a lock fires. The loader checks what the pack cannot, because it
//! knows the tile table: every cell of a door rectangle is a switched tile, and every switched tile in the map is covered by a
//! door binding.

use alloc::vec::Vec;
use fizz_pack::level::{BIND_DOOR, MAX_DOOR_BINDINGS, MAX_STATE_VALUES};
use fizz_pack::{BindingRec, StateRec};
use fizz_phys::tilemap::SWITCHED;
use fizz_phys::{TileMap, TileProps};

/// The bit of the first door binding in `TileMap::switches`.
pub const FIRST_DOOR_BIT: u8 = 1;

/// The most door bindings this build can derive a bit for (one bit each, from [`FIRST_DOOR_BIT`] up).
pub const MAX_DOORS: usize = MAX_DOOR_BINDINGS - FIRST_DOOR_BIT as usize;

/// Why a level's states and bindings were refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorError {
    /// A state with fewer than 2 or more than 8 values, or a start value out of range.
    State(usize),
    /// A binding that is not a door binding (the other kinds have no consumer yet), or that breaks the binding rules: a state that
    /// does not exist, a mask that is empty or has a bit above the state's values, or a rect that is inverted or outside the map.
    Binding(usize),
    /// More door bindings than this build derives bits for.
    TooMany,
    /// Binding `i`'s rect holds a cell that is not a switched tile.
    NotSwitched(usize),
    /// A switched tile at `(x, y)` that no door binding covers.
    Unbound(u16, u16),
    /// Two door bindings cover one cell.
    Overlap(usize),
}

/// One door binding with its derived bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Door {
    pub state: u8,
    /// The active set: the state's values at which the door is passable.
    pub mask: u8,
    /// `(x0, y0, x1, y1)`, both corners included.
    pub rect: (u16, u16, u16, u16),
    /// The derived bit of `TileMap::switches`, set while the door is closed.
    pub bit: u8,
}

/// A level's states and door bindings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Doors {
    states: Vec<StateRec>,
    doors: Vec<Door>,
}

impl Doors {
    /// No states and no doors.
    pub fn none() -> Doors {
        Doors::default()
    }

    /// Reads the states and bindings of a `w` by `h` map whose tiles are `tiles` (row-major, bottom row first).
    pub fn from_recs(
        states: &[StateRec],
        bindings: &[BindingRec],
        tiles: &[u8],
        w: u16,
        h: u16,
        props: &TileProps,
    ) -> Result<Doors, DoorError> {
        for (i, s) in states.iter().enumerate() {
            if !(2..=MAX_STATE_VALUES as u8).contains(&s.values) || s.start >= s.values {
                return Err(DoorError::State(i));
            }
        }
        let mut doors: Vec<Door> = Vec::new();
        let mut covered = alloc::vec![false; usize::from(w) * usize::from(h)];
        for (i, b) in bindings.iter().enumerate() {
            if b.kind != BIND_DOOR {
                return Err(DoorError::Binding(i));
            }
            let Some(state) = states.get(usize::from(b.state)) else {
                return Err(DoorError::Binding(i));
            };
            let (x0, y0, x1, y1) = b.rect;
            if b.mask == 0
                || b.mask & !state.all_values() != 0
                || x0 > x1
                || y0 > y1
                || x1 >= w
                || y1 >= h
            {
                return Err(DoorError::Binding(i));
            }
            if doors.len() >= MAX_DOORS {
                return Err(DoorError::TooMany);
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let at = usize::from(y) * usize::from(w) + usize::from(x);
                    if !switched(props, tiles[at]) {
                        return Err(DoorError::NotSwitched(i));
                    }
                    if core::mem::replace(&mut covered[at], true) {
                        return Err(DoorError::Overlap(i));
                    }
                }
            }
            doors.push(Door {
                state: b.state,
                mask: b.mask,
                rect: b.rect,
                bit: FIRST_DOOR_BIT + doors.len() as u8,
            });
        }
        for (at, &t) in tiles.iter().enumerate() {
            if switched(props, t) && !covered[at] {
                return Err(DoorError::Unbound(
                    (at % usize::from(w)) as u16,
                    (at / usize::from(w)) as u16,
                ));
            }
        }
        Ok(Doors {
            states: states.to_vec(),
            doors,
        })
    }

    /// The door bindings in binding order.
    pub fn doors(&self) -> &[Door] {
        &self.doors
    }

    /// The state table.
    pub fn states(&self) -> &[StateRec] {
        &self.states
    }

    /// The switch word a level starts with, and a death restores: a bit is set for each door whose state starts at a value outside
    /// its active set (the door is closed).
    pub fn start_switches(&self) -> u32 {
        let mut bits = 0;
        for d in &self.doors {
            let start = self.states[usize::from(d.state)].start;
            if d.mask & (1 << start) == 0 {
                bits |= 1 << d.bit;
            }
        }
        bits
    }

    /// Binds every cell of every door rectangle to its door's bit in `map`.
    pub fn bind(&self, map: &mut TileMap) {
        for d in &self.doors {
            let (x0, y0, x1, y1) = d.rect;
            for y in y0..=y1 {
                for x in x0..=x1 {
                    map.bind_switch(i32::from(x), i32::from(y), d.bit);
                }
            }
        }
    }

    /// The only door bound to `state`, or `None` when it has none or more than one.
    pub fn door_of(&self, state: u8) -> Option<&Door> {
        let mut it = self.doors.iter().filter(|d| d.state == state);
        match (it.next(), it.next()) {
            (Some(d), None) => Some(d),
            _ => None,
        }
    }

    /// The values of `state` at which no door bound to it is open: the complement of the door's active set within the state's values.
    pub fn closed_set(&self, d: &Door) -> u8 {
        self.states[usize::from(d.state)].all_values() & !d.mask
    }
}

/// Whether a tile id is a switched tile in `props`.
fn switched(props: &TileProps, id: u8) -> bool {
    props.flags(id) & SWITCHED != 0
}
