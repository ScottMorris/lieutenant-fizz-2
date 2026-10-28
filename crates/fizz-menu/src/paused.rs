// The Paused menu: Resume, Options and Quit to DOS (or Restart on the Web).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::menu::{Entry, Kind, Menu, MenuKey, Outcome, RowId};
use crate::options::{Platform, RowList, RowView};
use crate::text::Text;

/// The Paused menu. Back (Esc) closes it, which resumes play.
#[derive(Clone, Copy, Debug)]
pub struct PausedScreen {
    platform: Platform,
    menu: Menu,
}

impl PausedScreen {
    pub const RESUME: RowId = RowId(10);
    pub const OPTIONS: RowId = RowId(11);
    pub const QUIT: RowId = RowId(12);
    pub const RESTART: RowId = RowId(13);

    /// A menu with the cursor on Resume.
    pub const fn new(platform: Platform) -> PausedScreen {
        PausedScreen {
            platform,
            menu: Menu::new(),
        }
    }

    fn entries(&self) -> [Entry; 3] {
        let last = match self.platform {
            Platform::Dos => Self::QUIT,
            Platform::Web => Self::RESTART,
        };
        [Self::RESUME, Self::OPTIONS, last].map(|id| Entry {
            id,
            kind: Kind::Action,
        })
    }

    pub fn cursor(&self) -> usize {
        self.menu.cursor()
    }

    /// Up from Resume wraps to the last row; Esc closes. Confirm on a row reports `Chosen`.
    pub fn key(&mut self, key: MenuKey) -> Outcome {
        let rows = self.entries();
        self.menu.key(key, &rows, |_, _| false)
    }

    /// The label of a paused row.
    pub fn label(id: RowId) -> &'static str {
        match id {
            Self::RESUME => "Resume",
            Self::OPTIONS => "Options",
            Self::QUIT => "Quit to DOS",
            Self::RESTART => "Restart",
            _ => "",
        }
    }

    /// The rows to draw. Their values are empty.
    pub fn rows(&self) -> RowList {
        let mut list = RowList::new();
        for (i, e) in self.entries().iter().enumerate() {
            list.push(RowView {
                id: e.id,
                label: Self::label(e.id),
                value: Text::EMPTY,
                selected: i == self.menu.cursor(),
                overridden: false,
            });
        }
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_ends_in_quit_and_up_from_resume_wraps_to_it() {
        let mut p = PausedScreen::new(Platform::Dos);
        assert_eq!(p.rows().as_slice()[0].label, "Resume");
        assert_eq!(p.key(MenuKey::Up), Outcome::Moved);
        assert_eq!(p.key(MenuKey::Confirm), Outcome::Chosen(PausedScreen::QUIT));
        assert_eq!(p.rows().as_slice()[2].label, "Quit to DOS");
    }

    #[test]
    fn web_ends_in_restart() {
        let mut p = PausedScreen::new(Platform::Web);
        p.key(MenuKey::Up);
        assert_eq!(
            p.key(MenuKey::Confirm),
            Outcome::Chosen(PausedScreen::RESTART)
        );
    }

    #[test]
    fn options_is_the_second_row_and_back_closes() {
        let mut p = PausedScreen::new(Platform::Dos);
        p.key(MenuKey::Down);
        assert_eq!(
            p.key(MenuKey::Confirm),
            Outcome::Chosen(PausedScreen::OPTIONS)
        );
        assert_eq!(p.key(MenuKey::Back), Outcome::Closed);
        assert!(p.rows().as_slice()[1].selected);
    }
}
