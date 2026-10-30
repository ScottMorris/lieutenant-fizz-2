// The Paused and Options screens in `/LIVE`: which one is up, the keys, the live effect of each change and the save to `FIZZ2.CFG`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Provenance: written for this repository; the flow (Esc opens Paused, Options inside it, the choice applied as it is made) follows
// Fizz 1's DOS shell. The rows, keys and look are `fizz-menu` and `fizz_present_dos::menu`; this file is the part that touches the machine.
//
// While a screen is up the loop does not step the world, so the game, its effects and the frame governor are held; the audio service
// goes on, so the music keeps playing. Each change applies as the key is pressed: captions and motion go to the draw options, the
// key layout to the input map, the frame rate to the governor (`Drawing::set_mode`), the placement to `play_events` and a device
// through `audio::replan` with a plan resolved from the machine the start-up probes found, the way start-up resolves it. The file
// is written when the player leaves Options with something changed (not after each key). A row that a switch set for this run is
// starred and keeps the file's own value in the saved file, so a switch is never saved. A failed save shows "Could not write
// FIZZ2.CFG" in light red and the game carries on.
use crate::{CLASSIC_PLACEMENT, frames::Drawing, sound};
use alloc::boxed::Box;
use core::sync::atomic::Ordering;
use fizz_dos_rt::{audio, timer};
use fizz_hal::KeyCode;
use fizz_machine::{Captions, Keys, Motion, PlacementMode, Prefs, Probe, Resolved, Settings, resolve};
use fizz_menu::{Caps, MenuKey, MotionPick, OptionsScreen, Outcome, Overrides, PausedScreen, Platform, RowId, Values};
use fizz_present_dos::menu::{MenuView, menu_key};
use fizz_present_dos::{DrawOptions, Frame, Layout};

/// What a failed save says.
pub const SAVE_FAILED: &str = "Could not write FIZZ2.CFG";

/// The settings the loop reads each pass, which the Options screen changes live.
#[derive(Clone, Copy)]
pub struct Applied {
    pub layout: Layout,
    pub draw: DrawOptions,
}

impl Applied {
    /// What `prefs` selects.
    pub fn from_prefs(prefs: &Prefs) -> Applied {
        Applied {
            layout: match prefs.keys {
                Keys::Keen => Layout::Keen,
                Keys::Modern => Layout::Modern,
            },
            draw: DrawOptions {
                captions: prefs.captions == Captions::On,
                // Motion gates the death shake and spin; Reduced leaves them off.
                shake: prefs.motion == Motion::Full,
            },
        }
    }
}

/// What a key asks the loop to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// Carry on with the screen.
    Stay,
    /// Close the screens and go back to the game.
    Resume,
    /// End the run (Quit to DOS, or Esc on the `/OPTIONS` screen).
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Which {
    Closed,
    Paused,
    Options,
}

pub struct Menus {
    view: Box<MenuView>,
    which: Which,
    paused: PausedScreen,
    options: OptionsScreen,
    /// The values in force now, and the rows a switch still holds.
    values: Values,
    overridden: Overrides,
    /// What `FIZZ2.CFG` holds (the last save's prefs once one succeeds).
    file: Prefs,
    /// A change has not reached the file yet (a failed save leaves it set, so the next leaving tries again).
    dirty: bool,
    probe: Probe,
    auto: Resolved,
    opened_at: u32,
    saved: Option<bool>,
    /// `/OPTIONS`: leaving Options ends the run.
    standalone: bool,
}

/// The rows `effective` differs from `file` in are the rows a switch set.
fn overrides(file: &Prefs, effective: &Prefs) -> Overrides {
    let mut o = Overrides::NONE;
    for (differs, row) in [
        (file.sound.music != effective.sound.music, RowId::MUSIC),
        (file.sound.fx != effective.sound.fx, RowId::EFFECTS),
        (file.frame_rate != effective.frame_rate, RowId::FRAME_RATE),
        (file.captions != effective.captions, RowId::CAPTIONS),
        (file.keys != effective.keys, RowId::KEYS),
    ] {
        if differs {
            o = o.with(row);
        }
    }
    o
}

impl Menus {
    /// `file` is `FIZZ2.CFG`'s own prefs and `effective` is what this run uses (the file with the switches over it).
    ///
    /// Never inlined: building the view puts a 64 KB frame on the stack for a moment, and inlined into `live` the stack slot would be
    /// reserved for the whole run on top of the game's own frame. The 256 KB stack holds `live` (76 KB) and this (76 KB) with room to
    /// spare only because the mode functions in `main.rs` are not inlined into `main` either.
    #[inline(never)]
    pub fn new(file: Prefs, effective: Prefs, probe: Probe, standalone: bool) -> Menus {
        let values = Values::from_prefs(&effective);
        let overridden = overrides(&file, &effective);
        Menus {
            view: Box::write(Box::<MenuView>::new_uninit(), MenuView::new()),
            which: Which::Closed,
            paused: PausedScreen::new(Platform::Dos),
            options: OptionsScreen::new(values, overridden),
            values,
            overridden,
            file,
            dirty: false,
            // "Auto (...)" names what Auto would do on this machine, not what the player's choice does.
            auto: resolve(&Settings::default(), &probe),
            probe,
            opened_at: 0,
            saved: None,
            standalone,
        }
    }

    pub fn is_open(&self) -> bool {
        self.which != Which::Closed
    }

    /// `Some(true)` or `Some(false)` once a save has been tried: whether the latest one reached the disk.
    pub fn saved(&self) -> Option<bool> {
        self.saved
    }

    /// Opens Paused over `picture` (the last frame drawn).
    pub fn open_paused(&mut self, picture: &Frame) {
        self.view.open(picture);
        self.view.set_message("");
        self.paused = PausedScreen::new(Platform::Dos);
        self.which = Which::Paused;
        self.opened_at = timer::ticks();
        audio::set_mix(audio::Mix::Pause);
    }

    /// Opens Options over `picture` without Paused behind it (`/OPTIONS`).
    pub fn open_options(&mut self, picture: &Frame) {
        self.view.open(picture);
        self.opened_at = timer::ticks();
        self.enter_options();
    }

    fn enter_options(&mut self) {
        self.view.set_message("");
        self.view.invalidate();
        self.options = OptionsScreen::new(self.values, self.overridden);
        self.which = Which::Options;
        audio::set_mix(audio::Mix::Options);
    }

    fn caps(&self, rate: u32) -> Caps {
        Caps::dos(&self.auto, rate)
    }

    /// Handles a pressed key.
    pub fn key(&mut self, code: KeyCode, applied: &mut Applied, drawing: &mut Drawing) -> Step {
        let Some(key) = menu_key(code, applied.layout) else {
            return Step::Stay;
        };
        match self.which {
            Which::Closed => Step::Stay,
            Which::Paused => match self.paused.key(key) {
                Outcome::Closed => self.close(),
                Outcome::Chosen(PausedScreen::RESUME) => self.close(),
                Outcome::Chosen(PausedScreen::OPTIONS) => {
                    self.enter_options();
                    Step::Stay
                }
                Outcome::Chosen(PausedScreen::QUIT) => {
                    self.which = Which::Closed;
                    audio::set_mix(audio::Mix::Play);
                    Step::Quit
                }
                _ => Step::Stay,
            },
            Which::Options => {
                let caps = self.caps(drawing.rate());
                let outcome = self.options.key(key, &caps);
                self.values = self.options.values;
                self.overridden = self.options.overridden;
                match outcome {
                    Outcome::Changed(row) => {
                        self.dirty = true;
                        self.apply(row, applied, drawing);
                        Step::Stay
                    }
                    Outcome::Closed => self.leave_options(),
                    _ => Step::Stay,
                }
            }
        }
    }

    fn close(&mut self) -> Step {
        self.view.set_message("");
        self.which = Which::Closed;
        audio::set_mix(audio::Mix::Play);
        Step::Resume
    }

    /// Leaves Options: saves if anything changed, then goes back to Paused with the cursor on Options (or ends the run for `/OPTIONS`).
    fn leave_options(&mut self) -> Step {
        let ok = !self.dirty || self.save();
        if self.standalone {
            self.which = Which::Closed;
            audio::set_mix(audio::Mix::Play);
            return Step::Quit;
        }
        self.paused = PausedScreen::new(Platform::Dos);
        let _ = self.paused.key(MenuKey::Down);
        self.view.invalidate();
        self.which = Which::Paused;
        audio::set_mix(audio::Mix::Pause);
        self.view.set_message(if ok { "" } else { SAVE_FAILED });
        Step::Stay
    }

    /// Writes `FIZZ2.CFG`. A row a switch still holds keeps the file's value, because switches are never saved.
    fn save(&mut self) -> bool {
        let mut prefs = self.values.to_prefs();
        if self.overridden.has(RowId::MUSIC) {
            prefs.sound.music = self.file.sound.music;
        }
        if self.overridden.has(RowId::EFFECTS) {
            prefs.sound.fx = self.file.sound.fx;
        }
        if self.overridden.has(RowId::FRAME_RATE) {
            prefs.frame_rate = self.file.frame_rate;
        }
        if self.overridden.has(RowId::CAPTIONS) {
            prefs.captions = self.file.captions;
        }
        if self.overridden.has(RowId::KEYS) {
            prefs.keys = self.file.keys;
        }
        let ok = sound::save(&prefs).is_ok();
        self.saved = Some(ok);
        if ok {
            self.file = prefs;
            self.dirty = false;
        }
        ok
    }

    /// Makes the change to `row` take effect now.
    fn apply(&mut self, row: RowId, applied: &mut Applied, drawing: &mut Drawing) {
        let v = &self.values;
        match row {
            RowId::MUSIC | RowId::EFFECTS => {
                let plan = resolve(
                    &Settings {
                        music: v.music,
                        fx: v.fx,
                    },
                    &self.probe,
                );
                let _ = audio::replan(&plan);
            }
            RowId::PLACEMENT => CLASSIC_PLACEMENT.store(v.placement == PlacementMode::Classic, Ordering::Relaxed),
            RowId::FRAME_RATE => drawing.set_mode(v.frame_rate),
            RowId::MOTION => applied.draw.shake = v.motion != MotionPick::Reduced,
            RowId::CAPTIONS => applied.draw.captions = v.captions == Captions::On,
            RowId::KEYS => {
                applied.layout = match v.keys {
                    Keys::Keen => Layout::Keen,
                    Keys::Modern => Layout::Modern,
                }
            }
            _ => {}
        }
    }

    /// Draws the open screen into `frame`; true when `frame` changed and should be presented.
    pub fn draw(&mut self, frame: &mut Frame, rate: u32) -> bool {
        let tick = (u64::from(timer::ticks().wrapping_sub(self.opened_at)) * 60 / 560) as u32;
        match self.which {
            Which::Closed => false,
            Which::Paused => self.view.draw_paused(&self.paused, tick, frame),
            Which::Options => {
                let caps = self.caps(rate);
                self.view.draw_options(&self.options, &caps, tick, frame)
            }
        }
    }
}
