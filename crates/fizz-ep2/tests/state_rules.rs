// Tests of the state table and the rule phase: enter and touch rows, active sets, the lock as a plain rule, the death reset and the hash.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Each test builds a small room from level records (`StateRec`, `BindingRec`, `RuleRec`) through `World::set_level`, the path a
//! baked level takes, and checks one behaviour of `docs/RULES.md`.

use fizz_ep2::level::{self, SourceError};
use fizz_ep2::rules::{KEY_BLUE, KEY_RED, RuleError};
use fizz_ep2::shot::Shot;
use fizz_ep2::tiles::{self, DOOR_R, FILL};
use fizz_ep2::world::RESPAWN_AFTER;
use fizz_ep2::{Event, World, WorldFlags};
use fizz_pack::level::{
    COND_NOT_HOLDS, FLAG_REPEAT, OP_NEXT, OP_TOGGLE, TRIGGER_ENTER, TRIGGER_START, TRIGGER_TOUCH,
};
use fizz_pack::{ActionRec, BindingRec, CondRec, RuleRec, StateRec};
use fizz_phys::TileMap;
use fizz_sim::InputFrame;

/// The rectangle the enter and touch rows watch: columns 8 and 9, rows 1 to 3.
const ZONE: (u16, u16, u16, u16) = (8, 1, 9, 3);
/// The door's rectangle: column 20, rows 1 and 2.
const DOOR: (u16, u16, u16, u16) = (20, 1, 20, 2);

/// A 24 by 12 room with a solid floor and Ben standing at the left. With `door`, a red door stands at [`DOOR`], bound to state
/// `door.0` with the active set `door.1`.
fn room(
    states: &[StateRec],
    door: Option<(u8, u8)>,
    rules: &[RuleRec],
) -> Result<World, SourceError> {
    let mut m = TileMap::new(24, 12, tiles::props());
    for x in 0..24 {
        m.data[x] = FILL;
    }
    let mut bindings = Vec::new();
    if let Some((state, mask)) = door {
        m.set(20, 1, DOOR_R);
        m.set(20, 2, DOOR_R);
        bindings.push(BindingRec::door(state, mask, DOOR));
    }
    let mut w = World::new(m, (2.0, 1.0));
    w.set_level(states, &bindings, rules)?;
    Ok(w)
}

fn rect_rule(trigger: u8, rect: (u16, u16, u16, u16), act: [ActionRec; 2]) -> RuleRec {
    let (u0, u1) = RuleRec::union_rect(rect.0, rect.1, rect.2, rect.3);
    RuleRec {
        trigger,
        u0,
        u1,
        act,
        ..RuleRec::EMPTY
    }
}

fn enter(act: ActionRec) -> RuleRec {
    rect_rule(TRIGGER_ENTER, ZONE, [act, ActionRec::EMPTY])
}

fn op(op: u8, state: u8) -> ActionRec {
    ActionRec {
        op,
        a: state,
        ..ActionRec::EMPTY
    }
}

fn repeat(r: RuleRec) -> RuleRec {
    RuleRec {
        flags: FLAG_REPEAT,
        ..r
    }
}

fn with_cond(r: RuleRec, c0: CondRec, c1: CondRec) -> RuleRec {
    RuleRec {
        cond: [c0, c1],
        ..r
    }
}

/// The `lock` sugar for the red key on state 0 over [`DOOR`].
fn red_lock() -> RuleRec {
    with_cond(
        rect_rule(
            TRIGGER_TOUCH,
            DOOR,
            [ActionRec::set(0, 1), ActionRec::spend_key(KEY_RED)],
        ),
        CondRec::state_in(0, 0b01),
        CondRec::holds(KEY_RED),
    )
}

/// Puts Ben on the floor with his lower-left corner at `x` and runs one tick.
fn stand_at(w: &mut World, x: f64) {
    w.ben.b.x = x;
    w.ben.b.y = 1.0;
    w.ben.b.vx = 0.0;
    w.ben.b.vy = 0.0;
    w.step(InputFrame::default());
}

fn kill_and_respawn(w: &mut World) {
    w.ben.dead = RESPAWN_AFTER + 1.0;
    w.step(InputFrame::default());
    assert!(w.ben.dead == 0.0, "the respawn ran");
}

fn door_events(w: &World) -> Vec<(i32, i32)> {
    w.events()
        .iter()
        .filter_map(|e| match *e {
            Event::DoorOpen { x, y } => Some((x, y)),
            _ => None,
        })
        .collect()
}

#[test]
fn entering_a_rect_sets_a_state_once_and_the_bound_door_follows() {
    let mut w = room(
        &[StateRec::new(2, 0)],
        Some((0, 0b10)),
        &[enter(ActionRec::set(0, 1))],
    )
    .unwrap();
    assert!(w.map.solid(20, 1, false, 0.0), "the door starts closed");
    stand_at(&mut w, 5.0);
    assert_eq!(w.states().values(), &[0]);
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[1]);
    assert!(!w.map.solid(20, 1, false, 0.0), "the bound door opens");
    // Ben's probe is nowhere near the door, so the door event is at the centre cell of its rectangle: (20, 1) for rows 1 and 2.
    assert_eq!(door_events(&w), [(20 * 16 + 8, 16 + 8)]);
    assert_eq!(w.fired(), 1);
}

#[test]
fn a_row_without_repeat_fires_once_per_life_and_with_repeat_on_every_entry() {
    for rep in [false, true] {
        let row = enter(op(OP_NEXT, 0));
        let row = if rep { repeat(row) } else { row };
        let mut w = room(&[StateRec::new(4, 0)], None, &[row]).unwrap();
        for _ in 0..3 {
            stand_at(&mut w, 8.5);
            stand_at(&mut w, 8.6); // still inside: not an entry
            stand_at(&mut w, 3.0);
        }
        assert_eq!(w.states().values(), &[if rep { 3 } else { 1 }]);
    }
}

#[test]
fn next_wraps_and_toggle_flips_a_two_valued_state() {
    let mut w = room(
        &[StateRec::new(3, 2), StateRec::new(2, 0)],
        None,
        &[repeat(rect_rule(
            TRIGGER_ENTER,
            ZONE,
            [op(OP_NEXT, 0), op(OP_TOGGLE, 1)],
        ))],
    )
    .unwrap();
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[0, 1], "2 wraps to 0");
    stand_at(&mut w, 3.0);
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[1, 0]);
}

#[test]
fn a_door_is_open_for_every_value_in_its_active_set() {
    // Open at 0 and 2, closed at 1: one binding, and the door event only on the step into the set.
    let mut w = room(
        &[StateRec::new(3, 0)],
        Some((0, 0b101)),
        &[repeat(enter(op(OP_NEXT, 0)))],
    )
    .unwrap();
    assert!(!w.map.solid(20, 1, false, 0.0));
    let mut seen = Vec::new();
    for _ in 0..4 {
        stand_at(&mut w, 8.5);
        seen.push((
            w.states().values()[0],
            w.map.solid(20, 1, false, 0.0),
            door_events(&w).len(),
        ));
        stand_at(&mut w, 3.0);
    }
    assert_eq!(
        seen,
        [(1, true, 0), (2, false, 1), (0, false, 0), (1, true, 0)]
    );
}

#[test]
fn a_later_row_sees_an_earlier_rows_change_in_the_same_tick() {
    let set = enter(ActionRec::set(0, 1));
    let guarded = with_cond(
        enter(ActionRec::set(1, 1)),
        CondRec::state_in(0, 0b10),
        CondRec::EMPTY,
    );
    let states = [StateRec::new(2, 0), StateRec::new(2, 0)];
    let mut w = room(&states, None, &[set, guarded]).unwrap();
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[1, 1]);
    // In the other order the guard is read before the state is set, so the row does not fire, and as an edge it waits for the
    // next entry.
    let mut w = room(&states, None, &[guarded, set]).unwrap();
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[1, 0]);
    stand_at(&mut w, 8.6);
    assert_eq!(w.states().values(), &[1, 0]);
    stand_at(&mut w, 3.0);
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[1, 1]);
}

#[test]
fn two_conditions_are_an_and_and_a_key_can_be_held_or_not() {
    let states = [StateRec::new(2, 0)];
    let not_blue = CondRec {
        kind: COND_NOT_HOLDS,
        a: KEY_BLUE,
        b: 0,
    };
    let row = with_cond(
        enter(ActionRec::set(0, 1)),
        CondRec::holds(KEY_RED),
        not_blue,
    );
    for (flags, fires) in [
        (0, false),
        (WorldFlags::KEY_RED, true),
        (WorldFlags::KEY_RED | WorldFlags::KEY_BLUE, false),
        (WorldFlags::KEY_BLUE, false),
    ] {
        let mut w = room(&states, None, &[row]).unwrap();
        w.flags.set(flags);
        stand_at(&mut w, 8.5);
        assert_eq!(w.states().values(), &[u8::from(fires)], "{flags:#b}");
        assert_eq!(
            w.fired(),
            u32::from(fires),
            "a row that fails its conditions is not fired"
        );
    }
}

#[test]
fn the_lock_is_a_plain_rule_that_runs_after_the_shots() {
    let mut w = room(&[StateRec::new(2, 0)], Some((0, 0b10)), &[red_lock()]).unwrap();
    // Ben stands against the door facing it, with the red key lying where he stands: he takes it in his contacts, and the lock
    // row, run after the shots, sees it on the same tick (Fizz 1's lock phase ran before the keys and saw it a tick later).
    let x = 20.0 - w.ben.b.w - 0.05;
    w.set_key(x + w.ben.b.w / 2.0, 1.4);
    // A shot that ends this tick: its end is raised in the shots, before the rule phase.
    w.shots.push(Shot {
        x: 4.0,
        y: 5.0,
        vx: 0.0,
        vy: 0.0,
        life: 0.001,
    });
    w.ben.face = 1.0;
    stand_at(&mut w, x);
    let kinds: Vec<&str> = w
        .events()
        .iter()
        .map(|e| match e {
            Event::Key { .. } => "key",
            Event::ShotEnd { .. } => "shotend",
            Event::DoorOpen { .. } => "dooropen",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["key", "shotend", "dooropen"]);
    assert_eq!(w.states().values(), &[1]);
    assert!(w.flags.contains(WorldFlags::KEY_RED_USED));
    assert!(!w.flags.contains(WorldFlags::KEY_RED));
    // The event is at the door's centre cell, (20, 1) for rows 1 and 2, not where Fizz 1 raised it above Ben's probe.
    assert_eq!(door_events(&w), [(20 * 16 + 8, 16 + 8)]);
    // Fired once: the key is gone and the state is open, so nothing happens on the next ticks of the contact.
    stand_at(&mut w, x);
    assert!(door_events(&w).is_empty());
}

#[test]
fn a_touch_is_a_level_that_a_repeating_row_reads_on_every_tick_of_the_contact() {
    let touch = rect_rule(TRIGGER_TOUCH, ZONE, [op(OP_NEXT, 0), ActionRec::EMPTY]);
    for (row, want) in [(touch, 1), (repeat(touch), 4)] {
        let mut w = room(&[StateRec::new(8, 0)], None, &[row]).unwrap();
        for _ in 0..4 {
            stand_at(&mut w, 8.5); // the body overlaps the rectangle
        }
        stand_at(&mut w, 3.0);
        assert_eq!(w.states().values(), &[want]);
    }
}

#[test]
fn a_death_restores_every_state_the_doors_and_the_rule_memory_with_no_door_event() {
    let mut w = room(
        &[StateRec::new(2, 1), StateRec::new(8, 3)],
        Some((0, 0b10)),
        &[enter(ActionRec::set(0, 0)), enter(op(OP_NEXT, 1))],
    )
    .unwrap();
    assert!(
        !w.map.solid(20, 1, false, 0.0),
        "state 0 starts open, so the door does"
    );
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[0, 4]);
    assert!(w.map.solid(20, 1, false, 0.0));
    assert_eq!(w.fired(), 0b11);
    kill_and_respawn(&mut w);
    assert_eq!(w.states().values(), &[1, 3]);
    assert!(!w.map.solid(20, 1, false, 0.0), "the door is as it began");
    assert!(door_events(&w).is_empty(), "a reset raises no door event");
    assert_eq!((w.deaths, w.fired()), (1, 0));
    // The once-per-life memory and the inside memory are clear, so the rows fire again.
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[0, 4]);
}

#[test]
fn a_dead_ben_raises_no_enter() {
    let mut w = room(&[StateRec::new(2, 0)], None, &[enter(ActionRec::set(0, 1))]).unwrap();
    w.ben.dead = 0.5;
    stand_at(&mut w, 8.5);
    assert_eq!(w.states().values(), &[0]);
}

#[test]
fn state_and_rule_memory_are_hashed_for_any_world_that_declares_a_state() {
    // A state no door binds, so the door word cannot carry it.
    let row = enter(ActionRec::set(0, 1));
    let a = room(&[StateRec::new(2, 0)], None, &[row]).unwrap();
    let b = room(&[StateRec::new(2, 1)], None, &[row]).unwrap();
    assert_ne!(a.hash(), b.hash(), "the start value is hashed");
    // Two worlds alike but for one row's fired bit (the other row's condition fails, so it does not fire): the memory is hashed.
    let failing = with_cond(row, CondRec::state_in(0, 0b01), CondRec::EMPTY);
    let mut fired = room(&[StateRec::new(2, 1)], None, &[row]).unwrap();
    let mut unfired = room(&[StateRec::new(2, 1)], None, &[failing]).unwrap();
    stand_at(&mut fired, 8.5);
    stand_at(&mut unfired, 8.5);
    assert_eq!(fired.states().values(), unfired.states().values());
    assert_eq!((fired.fired(), unfired.fired()), (1, 0));
    assert_ne!(fired.hash(), unfired.hash());
    // A world without states hashes no block: its hash is the bare world's.
    let none = room(&[], None, &[]).unwrap();
    let bare = {
        let mut m = TileMap::new(24, 12, tiles::props());
        for x in 0..24 {
            m.data[x] = FILL;
        }
        World::new(m, (2.0, 1.0))
    };
    assert_eq!(none.hash(), bare.hash());
    // A level of locks alone hashes the block like any other: the rules themselves are level data and not hashed, so a door
    // with its lock and the same door with none hash alike until the lock fires.
    let mut lock = room(&[StateRec::new(2, 0)], Some((0, 0b10)), &[red_lock()]).unwrap();
    let mut no_lock = room(&[StateRec::new(2, 0)], Some((0, 0b10)), &[]).unwrap();
    assert_eq!(lock.hash(), no_lock.hash());
    for w in [&mut lock, &mut no_lock] {
        w.flags.set(WorldFlags::KEY_RED);
        w.ben.face = 1.0;
        let x = 20.0 - w.ben.b.w - 0.05;
        stand_at(w, x);
    }
    assert_eq!(lock.states().values(), &[1]);
    assert_ne!(lock.hash(), no_lock.hash());
    // The slice declares its two doors' states, so its hash carries the block too.
    assert_eq!(level::world().states().len(), 2);
}

#[test]
fn the_same_inputs_give_the_same_hash_twice() {
    let run = || {
        let mut w = room(
            &[StateRec::new(5, 1)],
            Some((0, 0b10010)),
            &[repeat(enter(op(OP_NEXT, 0)))],
        )
        .unwrap();
        let mut hashes = Vec::new();
        for i in 0..60u32 {
            stand_at(&mut w, if i % 7 < 3 { 8.5 } else { 4.0 });
            hashes.push(w.hash());
        }
        hashes
    };
    assert_eq!(run(), run());
}

#[test]
fn bad_records_are_refused_and_there_is_no_channel_to_name() {
    let rule = |r: RuleRec| room(&[StateRec::new(2, 0)], None, &[r]).map(|_| ());
    assert_eq!(
        rule(enter(ActionRec::set(0, 2))),
        Err(SourceError::Rule(RuleError::State))
    );
    assert_eq!(
        rule(enter(op(OP_NEXT, 1))),
        Err(SourceError::Rule(RuleError::State))
    );
    // The retired lock trigger (1), which named a switch channel, is not a trigger at all; a door binding names a state and a set.
    assert_eq!(
        rule(RuleRec {
            trigger: 1,
            ..red_lock()
        }),
        Err(SourceError::Rule(RuleError::Trigger))
    );
    assert_eq!(
        rule(RuleRec {
            trigger: TRIGGER_START,
            ..RuleRec::EMPTY
        }),
        Err(SourceError::Rule(RuleError::Unbuilt(TRIGGER_START)))
    );
    assert_eq!(
        rule(RuleRec {
            delay: 5,
            ..enter(ActionRec::set(0, 1))
        }),
        Err(SourceError::Rule(RuleError::Delay))
    );
    // The door's bit is derived from its binding's index; two bindings get bits 0 and 1 whatever their states.
    let mut m = TileMap::new(24, 12, tiles::props());
    m.set(20, 1, DOOR_R);
    m.set(22, 1, DOOR_R);
    let mut w = World::new(m, (2.0, 1.0));
    w.set_level(
        &[StateRec::new(2, 0), StateRec::new(2, 1)],
        &[
            BindingRec::door(1, 0b10, (22, 1, 22, 1)),
            BindingRec::door(0, 0b10, (20, 1, 20, 1)),
        ],
        &[],
    )
    .unwrap();
    assert_eq!(w.map.switch_bit(22, 1), Some(0));
    assert_eq!(w.map.switch_bit(20, 1), Some(1));
    assert_eq!(
        w.map.switches, 0b10,
        "binding 0's state starts open, binding 1's closed"
    );
}
