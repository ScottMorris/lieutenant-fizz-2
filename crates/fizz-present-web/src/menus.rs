// The Web page's Paused and Options screens: which one is up, the cursor, and the values, over `fizz-menu`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Plain Rust with no `wasm-bindgen`, so host tests cover it; `App` wraps it. The rules and the replay never read any of this: a
// screen being up only stops `App::frame` from stepping, and the values it holds are presentation settings. The page draws the
// rows as DOM (`web/options.ts`) and applies each change as it happens.
use fizz_machine::settings::{Captions, FxChoice, Keys, MusicChoice, PlacementMode};
use fizz_menu::{
    Caps, MenuKey, MotionPick, OVERRIDE_NOTE, OptionsScreen, Outcome, Overrides, PausedScreen,
    Platform, RowId, RowList, Values,
};

/// Which screen is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Closed,
    Paused,
    /// `from_paused` is whether Back returns to Paused (true) or resumes play (false).
    Options {
        from_paused: bool,
    },
}

impl Screen {
    /// The page's number for a screen, as `menu_screen` returns it.
    pub const fn code(self) -> u8 {
        match self {
            Screen::Closed => 0,
            Screen::Paused => 1,
            Screen::Options { .. } => 2,
        }
    }
}

/// What a key did, as the page sees it.
pub const KEY_NOTHING: u8 = 0;
pub const KEY_MOVED: u8 = 1;
pub const KEY_CHANGED: u8 = 2;
pub const KEY_CHOSEN: u8 = 3;
/// The screen changed: it closed, or Paused and Options switched.
pub const KEY_SCREEN: u8 = 4;

pub struct Menus {
    pub screen: Screen,
    paused: PausedScreen,
    options: OptionsScreen,
    fullscreen_available: bool,
    /// The row the last key changed or chose.
    last: u8,
    /// Set when the Restart row was chosen; `App` takes it and starts the level again.
    restart: bool,
}

impl Menus {
    pub fn new() -> Menus {
        let values = Values {
            motion: MotionPick::System,
            ..Values::default()
        };
        Menus {
            screen: Screen::Closed,
            paused: PausedScreen::new(Platform::Web),
            options: OptionsScreen::new(values, Overrides::NONE),
            fullscreen_available: false,
            last: 0,
            restart: false,
        }
    }

    fn caps(&self) -> Caps {
        Caps::web(self.fullscreen_available)
    }

    pub fn open_paused(&mut self) {
        self.paused = PausedScreen::new(Platform::Web);
        self.screen = Screen::Paused;
    }

    pub fn open_options(&mut self, from_paused: bool) {
        self.options.cursor = 0;
        self.screen = Screen::Options { from_paused };
    }

    pub fn close(&mut self) {
        self.screen = Screen::Closed;
    }

    pub fn set_fullscreen_available(&mut self, yes: bool) {
        self.fullscreen_available = yes;
    }

    /// Takes the pending Restart request.
    pub fn take_restart(&mut self) -> bool {
        core::mem::take(&mut self.restart)
    }

    /// The row id the last key changed or chose.
    pub fn last_row(&self) -> u8 {
        self.last
    }

    /// Handles `key` on whichever screen is up and says what happened.
    pub fn key(&mut self, key: MenuKey) -> u8 {
        let caps = self.caps();
        match self.screen {
            Screen::Closed => KEY_NOTHING,
            Screen::Paused => match self.paused.key(key) {
                Outcome::Closed => self.shut(),
                Outcome::Chosen(id) if id == PausedScreen::RESUME => self.shut(),
                Outcome::Chosen(id) if id == PausedScreen::OPTIONS => {
                    self.open_options(true);
                    KEY_SCREEN
                }
                Outcome::Chosen(id) if id == PausedScreen::RESTART => {
                    self.restart = true;
                    self.shut()
                }
                Outcome::Moved => KEY_MOVED,
                _ => KEY_NOTHING,
            },
            Screen::Options { from_paused } => match self.options.key(key, &caps) {
                Outcome::Closed => {
                    self.screen = if from_paused {
                        Screen::Paused
                    } else {
                        Screen::Closed
                    };
                    KEY_SCREEN
                }
                Outcome::Changed(id) => {
                    self.last = id.0;
                    KEY_CHANGED
                }
                Outcome::Chosen(id) => {
                    self.last = id.0;
                    KEY_CHOSEN
                }
                Outcome::Moved => KEY_MOVED,
                Outcome::None => KEY_NOTHING,
            },
        }
    }

    fn shut(&mut self) -> u8 {
        self.screen = Screen::Closed;
        KEY_SCREEN
    }

    /// Puts the cursor on row `i` of the screen that is up.
    pub fn select(&mut self, i: usize) {
        let n = self.rows().len();
        if n == 0 {
            return;
        }
        let i = i.min(n - 1);
        match self.screen {
            Screen::Closed => {}
            Screen::Options { .. } => self.options.cursor = i,
            Screen::Paused => {
                // The Paused cursor has no setter; Down wraps, so it reaches any of the rows in fewer than `n` presses.
                for _ in 0..n {
                    if self.paused.cursor() == i {
                        break;
                    }
                    self.paused.key(MenuKey::Down);
                }
            }
        }
    }

    pub fn rows(&self) -> RowList {
        match self.screen {
            Screen::Closed => RowList::new(),
            Screen::Paused => self.paused.rows(),
            Screen::Options { .. } => self.options.rows(&self.caps()),
        }
    }

    /// The note under the rows, empty when none applies.
    pub fn note(&self) -> &'static str {
        match self.screen {
            Screen::Options { .. } if self.options.show_note() => OVERRIDE_NOTE,
            _ => "",
        }
    }

    /// Marks a row as set by the address for this visit (`?audio=`). Changing the row on the screen clears the mark.
    pub fn set_override(&mut self, row: u8) {
        self.options.overridden = self.options.overridden.with(RowId(row));
    }

    pub fn overridden(&self, row: u8) -> bool {
        self.options.overridden.has(RowId(row))
    }

    pub fn set_fullscreen(&mut self, on: bool) {
        self.options.values.fullscreen = on;
    }

    /// The index of a row's current choice, in the order the row cycles through them: Sound, Music and Captions On then Off;
    /// Placement Enhanced then Classic; Motion System, Reduced, Full; Keys Keen then Modern; Fullscreen Off then On.
    pub fn value(&self, row: u8) -> u32 {
        let v = &self.options.values;
        let flag = |b: bool| u32::from(b);
        match RowId(row) {
            RowId::SOUND => flag(v.fx == FxChoice::Off),
            RowId::MUSIC => flag(v.music == MusicChoice::Off),
            RowId::PLACEMENT => flag(v.placement == PlacementMode::Classic),
            RowId::MOTION => match v.motion {
                MotionPick::System => 0,
                MotionPick::Reduced => 1,
                MotionPick::Full => 2,
            },
            RowId::CAPTIONS => flag(v.captions == Captions::Off),
            RowId::KEYS => flag(v.keys == Keys::Modern),
            RowId::FULLSCREEN => flag(v.fullscreen),
            _ => 0,
        }
    }

    /// Sets a row to its `index`th choice, in the order of [`Menus::value`]. This is how the page loads what it remembered; it
    /// moves no cursor and clears no override.
    pub fn set_value(&mut self, row: u8, index: u32) {
        let v = &mut self.options.values;
        let first = index == 0;
        match RowId(row) {
            RowId::SOUND => v.fx = if first { FxChoice::Auto } else { FxChoice::Off },
            RowId::MUSIC => {
                v.music = if first {
                    MusicChoice::Auto
                } else {
                    MusicChoice::Off
                }
            }
            RowId::PLACEMENT => {
                v.placement = if first {
                    PlacementMode::Enhanced
                } else {
                    PlacementMode::Classic
                }
            }
            RowId::MOTION => {
                v.motion = match index {
                    0 => MotionPick::System,
                    1 => MotionPick::Reduced,
                    _ => MotionPick::Full,
                }
            }
            RowId::CAPTIONS => v.captions = if first { Captions::On } else { Captions::Off },
            RowId::KEYS => v.keys = if first { Keys::Keen } else { Keys::Modern },
            RowId::FULLSCREEN => v.fullscreen = !first,
            _ => {}
        }
    }
}

impl Default for Menus {
    fn default() -> Self {
        Menus::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(m: &Menus) -> Vec<String> {
        m.rows()
            .as_slice()
            .iter()
            .map(|r| r.label.to_string())
            .collect()
    }

    #[test]
    fn paused_has_resume_options_restart_and_back_resumes() {
        let mut m = Menus::new();
        m.open_paused();
        assert_eq!(labels(&m), ["Resume", "Options", "Restart"]);
        assert_eq!(m.key(MenuKey::Back), KEY_SCREEN);
        assert_eq!(m.screen, Screen::Closed);
        m.open_paused();
        assert_eq!(m.key(MenuKey::Confirm), KEY_SCREEN);
        assert_eq!(m.screen, Screen::Closed);
    }

    #[test]
    fn restart_asks_for_a_restart_once() {
        let mut m = Menus::new();
        m.open_paused();
        m.key(MenuKey::Up);
        assert_eq!(m.key(MenuKey::Confirm), KEY_SCREEN);
        assert!(m.take_restart());
        assert!(!m.take_restart());
    }

    #[test]
    fn options_from_paused_returns_to_paused_and_from_the_panel_resumes() {
        let mut m = Menus::new();
        m.open_paused();
        m.key(MenuKey::Down);
        m.key(MenuKey::Confirm);
        assert_eq!(m.screen.code(), 2);
        assert_eq!(m.key(MenuKey::Back), KEY_SCREEN);
        assert_eq!(m.screen, Screen::Paused);
        m.open_options(false);
        m.key(MenuKey::Back);
        assert_eq!(m.screen, Screen::Closed);
    }

    #[test]
    fn the_web_rows_follow_the_menu_model_and_fullscreen_needs_the_browser() {
        let mut m = Menus::new();
        m.open_options(false);
        assert_eq!(
            labels(&m),
            [
                "Sound",
                "Music",
                "Placement",
                "Motion",
                "Captions",
                "Keys",
                "Back"
            ]
        );
        m.set_fullscreen_available(true);
        assert_eq!(m.rows().len(), 8);
        assert_eq!(m.rows().as_slice()[3].value.as_str(), "System");
    }

    #[test]
    fn a_step_changes_the_value_and_reports_the_row() {
        let mut m = Menus::new();
        m.open_options(false);
        assert_eq!(m.value(RowId::SOUND.0), 0);
        assert_eq!(m.key(MenuKey::Confirm), KEY_CHANGED);
        assert_eq!(m.last_row(), RowId::SOUND.0);
        assert_eq!(m.value(RowId::SOUND.0), 1);
        assert_eq!(m.rows().as_slice()[0].value.as_str(), "Off");
        // Motion cycles System, Reduced, Full and back.
        m.select(3);
        for want in [1, 2, 0] {
            m.key(MenuKey::Right);
            assert_eq!(m.value(RowId::MOTION.0), want);
        }
        m.key(MenuKey::Left);
        assert_eq!(m.value(RowId::MOTION.0), 2);
    }

    #[test]
    fn loaded_values_round_trip_through_their_indexes() {
        let mut m = Menus::new();
        for (row, max) in [(8u8, 2), (0, 2), (2, 2), (4, 3), (5, 2), (6, 2), (9, 2)] {
            for i in 0..max {
                m.set_value(row, i);
                assert_eq!(m.value(row), i, "row {row}");
            }
        }
    }

    #[test]
    fn an_override_shows_a_star_and_the_note_until_the_row_is_changed() {
        let mut m = Menus::new();
        m.set_value(RowId::PLACEMENT.0, 1);
        m.set_override(RowId::PLACEMENT.0);
        m.open_options(false);
        assert_eq!(m.rows().as_slice()[2].value.as_str(), "Classic *");
        assert_eq!(m.note(), OVERRIDE_NOTE);
        m.select(2);
        m.key(MenuKey::Right);
        assert!(!m.overridden(RowId::PLACEMENT.0));
        assert_eq!(m.note(), "");
        assert_eq!(m.rows().as_slice()[2].value.as_str(), "Enhanced");
    }

    #[test]
    fn selecting_a_row_moves_the_cursor_on_both_screens() {
        let mut m = Menus::new();
        m.open_paused();
        m.select(2);
        assert!(m.rows().as_slice()[2].selected);
        m.select(0);
        assert!(m.rows().as_slice()[0].selected);
        m.open_options(false);
        m.select(4);
        assert!(m.rows().as_slice()[4].selected);
        m.key(MenuKey::Down);
        assert!(m.rows().as_slice()[5].selected);
    }
}
