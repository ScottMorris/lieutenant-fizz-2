// Fizz 1's four small jokes as presenter view state: counters read after every step, a one-off line per level visit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`JokeWatch`] is Fizz 1's `Fun` counters (`world.rs`: `self.fun`, `joke(..)` and the `JOKE_*` bits) kept as a presenter's view state
//! in the way [`crate::FallWatch`] is: each presenter keeps one, calls [`JokeWatch::observe`] after every `fizz_ep2::step` and shows
//! what it returns. The rules do not know about it, so the state hash and the replay transcript cannot move, and the Web and DOS
//! presenters agree because they run the same code on the same worlds and inputs. Nothing displays a joke yet (there is no toast
//! line); the value is ready for one, and each presenter keeps what it last saw so a test or a tool can read it.
//!
//! Fizz 1's rules, which this reproduces (`episodes/episode-1/game/src/world.rs` at `4bce036`, texts in `text.rs`):
//!
//! - **Thirty pogo bounces in a row** ([`JokeKind::Pogo`]). Every bounce off the ground on the pogo stick counts (`Event::Bounce`: the
//!   stomp bounce off a chicken is not one). The count goes back to zero only when the pogo stick is switched off (the Pogo press that
//!   turns it off), not on landing, dying or any other pause: stopping the stick is what breaks the run.
//! - **Eight stomps** ([`JokeKind::Stomp`]). Fizz 1 counts the stomps on marshmallows, which are bounces that always happen. Fizz 2 has
//!   no marshmallows and counts the stomps on chickens (`Event::Stomp`), which are the stomps that stun one. Fizz 1 never resets the
//!   count within a level visit. A bounce off a chicken that is already stunned raises no event in Fizz 2 and does not count; Fizz 1's
//!   other enemies are not counted either.
//! - **Twelve wall-kicks** ([`JokeKind::Kick`]), in total for the visit, never reset (`Event::Kick`).
//! - **Twenty seconds of standing still** ([`JokeKind::Idle`]). A step adds one tick (`STEP`) to the idle time when no button is held and
//!   Ben was standing on the ground when the step began, and puts it back to zero otherwise; the joke tells when the sum is above 20.0
//!   (the same floating-point sum Fizz 1 makes, so it is told on the same tick). A step that began with Ben dead or past the exit leaves
//!   the sum as it was, as Fizz 1's early returns do. The pose is Fizz 1's `on_ground`, so a pogo that is on the ground between bounces
//!   counts only if no button is held, which a bouncing pogo seldom has.
//! - **Once per level visit.** Each joke is told once. Fizz 1 reloads the level after a death and starts the counters again; Fizz 2's
//!   respawn is the same visit again, so the watch clears itself when it sees Ben go from dead to alive, and a presenter clears it when
//!   it restarts the level.
//!
//! The state a step began with (standing, dead, past the exit, on the pogo stick) is what the last call saw at its end, which is the same
//! state, so the watch needs the step's [`InputFrame`] and nothing else from the presenter.

use fizz_ep2::{Event, STEP, World, WorldFlags};
use fizz_sim::{Buttons, InputFrame};

/// Pogo bounces in a row that earn the joke (Fizz 1's `fun.pogos >= 30`).
pub const POGO_BOUNCES: u32 = 30;
/// Stomps that earn the joke (Fizz 1's `fun.marsh >= 8`, on marshmallows).
pub const STOMPS: u32 = 8;
/// Wall-kicks that earn the joke (Fizz 1's `fun.kicks >= 12`).
pub const KICKS: u32 = 12;
/// Seconds standing still that earn the joke: the idle time must be above this (Fizz 1's `fun.idle > 20.0`).
pub const IDLE_SECONDS: f64 = 20.0;

/// Which joke. The order is the order Fizz 1's code tells them in within one step: standing still, the pogo, a kick, a stomp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JokeKind {
    /// Standing still for [`IDLE_SECONDS`] (Fizz 1's `JokeIdle`).
    Idle = 0,
    /// [`POGO_BOUNCES`] bounces on the pogo stick in a row (`JokePogo`).
    Pogo = 1,
    /// [`KICKS`] wall-kicks (`JokeKick`).
    Kick = 2,
    /// [`STOMPS`] stomps on chickens (`JokeMarsh`, on marshmallows in Fizz 1).
    Stomp = 3,
}

impl JokeKind {
    /// The four kinds, in the order they are told within a step.
    pub const ALL: [JokeKind; 4] = [
        JokeKind::Idle,
        JokeKind::Pogo,
        JokeKind::Kick,
        JokeKind::Stomp,
    ];

    /// A short name for a log line or a test.
    pub const fn name(self) -> &'static str {
        match self {
            JokeKind::Idle => "idle",
            JokeKind::Pogo => "pogo",
            JokeKind::Kick => "kick",
            JokeKind::Stomp => "stomp",
        }
    }

    /// The toast text: Fizz 1's, word for word.
    pub const fn text(self) -> &'static str {
        JOKE_TEXTS[self as usize]
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Fizz 1's four joke texts (`Toast::JokeIdle`, `JokePogo`, `JokeKick` and `JokeMarsh` in its `text.rs`), indexed by [`JokeKind`]. The
/// marshmallow line is kept as it is: its words are about marshmallows, and Fizz 2 will write its own once a toast line shows it.
pub const JOKE_TEXTS: [&str; 4] = [
    "Ben whistles a little tune. The level waits politely",
    "Thirty bounces. Ben is not a kangaroo, but he is doing a very good impression",
    "Ben's knees file a formal complaint about all this wall-kicking",
    "Ben has been told that marshmallows are not trampolines. He disagrees",
];

/// A joke that has just been earned: its kind and the line to show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Joke {
    pub kind: JokeKind,
    pub text: &'static str,
}

impl Joke {
    pub const fn new(kind: JokeKind) -> Self {
        Joke {
            kind,
            text: kind.text(),
        }
    }
}

/// The jokes one step earned, usually none. More than one in a step is possible (Fizz 1 would raise two toasts); they come out in
/// [`JokeKind::ALL`] order.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Jokes(u8);

impl Jokes {
    pub const NONE: Jokes = Jokes(0);

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, kind: JokeKind) -> bool {
        self.0 & kind.bit() != 0
    }

    /// The earned jokes, in [`JokeKind::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = Joke> {
        JokeKind::ALL
            .into_iter()
            .filter(move |&k| self.contains(k))
            .map(Joke::new)
    }
}

/// The jokes' counters and what the last step ended with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JokeWatch {
    pogos: u32,
    stomps: u32,
    kicks: u32,
    idle: f64,
    /// The kinds told this visit (a [`JokeKind`] bit each).
    told: u8,
    /// Ben stood on the ground when the last step ended.
    ground: bool,
    /// Ben was dead when the last step ended.
    dead: bool,
    /// Ben had touched the exit when the last step ended.
    won: bool,
    /// The pogo stick was on when the last step ended.
    pogo: bool,
}

impl Default for JokeWatch {
    fn default() -> Self {
        Self::new()
    }
}

impl JokeWatch {
    /// A watch at the start of a level visit: nothing counted, nothing told, Ben alive in the air as a new world has him.
    pub const fn new() -> Self {
        JokeWatch {
            pogos: 0,
            stomps: 0,
            kicks: 0,
            idle: 0.0,
            told: 0,
            ground: false,
            dead: false,
            won: false,
            pogo: false,
        }
    }

    /// Starts a new level visit (a presenter restarting the level).
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// Reads the step that just ran, given the `input` it ran with, and returns the jokes it earned. Pure apart from the watch's own
    /// fields; call it once per `fizz_ep2::step`, whatever the Captions setting or any other option says.
    pub fn observe(&mut self, world: &World, input: InputFrame) -> Jokes {
        let ben = &world.ben;
        let dead = ben.dead > 0.0;
        if self.dead && !dead {
            // The respawn: a new visit, with Ben as `Ben::new` leaves him.
            self.clear();
            return Jokes::NONE;
        }
        let mut told = Jokes::NONE;
        // Fizz 1's `tick_ben` returns before any of this while Ben is dead or the level is won.
        if !self.dead && !self.won {
            if input.pressed.contains(Buttons::POGO) && self.pogo {
                self.pogos = 0;
            }
            if input.held.is_empty() && self.ground {
                self.idle += STEP;
                if self.idle > IDLE_SECONDS {
                    self.tell(JokeKind::Idle, &mut told);
                }
            } else {
                self.idle = 0.0;
            }
            for event in world.events() {
                match event {
                    Event::Bounce { .. } => {
                        self.pogos += 1;
                        if self.pogos >= POGO_BOUNCES {
                            self.tell(JokeKind::Pogo, &mut told);
                        }
                    }
                    Event::Kick { .. } => {
                        self.kicks += 1;
                        if self.kicks >= KICKS {
                            self.tell(JokeKind::Kick, &mut told);
                        }
                    }
                    Event::Stomp { .. } => {
                        self.stomps += 1;
                        if self.stomps >= STOMPS {
                            self.tell(JokeKind::Stomp, &mut told);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.ground = ben.b.on_ground;
        self.dead = dead;
        self.won = world.flags.contains(WorldFlags::EXIT_REACHED);
        self.pogo = ben.pogo;
        told
    }

    fn tell(&mut self, kind: JokeKind, told: &mut Jokes) {
        if self.told & kind.bit() == 0 {
            self.told |= kind.bit();
            told.0 |= kind.bit();
        }
    }
}

#[cfg(test)]
mod tests;
