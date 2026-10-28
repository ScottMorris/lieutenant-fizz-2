// A generic list with a cursor: Up and Down wrap, Left and Right step a choice row, Confirm chooses, Back closes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The menu knows the shape of its rows and nothing of their values. A choice row asks the caller to step its value through a
// callback and reports `Changed` only when the callback says the value really moved. The title and pause menus use it as well.

/// A key as the menu understands it. Presenters map their own keys: Left is -1, Right and Confirm are +1, Back is Esc or Pogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

/// Names a row. The constants belong to the screens that use them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowId(pub u8);

/// What a row does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Left, Right and Confirm step its value.
    Choice,
    /// Confirm and Right choose it; Left does nothing.
    Action,
}

/// One row of a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: RowId,
    pub kind: Kind,
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing happened.
    None,
    /// The cursor moved.
    Moved,
    /// A choice row's value changed.
    Changed(RowId),
    /// An action row was chosen.
    Chosen(RowId),
    /// Back was pressed.
    Closed,
}

/// The cursor of a list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Menu {
    cursor: usize,
}

impl Menu {
    pub const fn new() -> Menu {
        Menu { cursor: 0 }
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Puts the cursor on row `index`, or on the last row when `index` is past the end.
    pub fn set_cursor(&mut self, index: usize, rows: &[Entry]) {
        self.cursor = index.min(rows.len().saturating_sub(1));
    }

    /// Handles `key` over `rows`. `step(id, dir)` moves a choice row's value one step (`dir` is -1 or +1, wrapping, skipping
    /// values that cannot be used) and returns whether the value now differs from before.
    pub fn key(
        &mut self,
        key: MenuKey,
        rows: &[Entry],
        mut step: impl FnMut(RowId, i8) -> bool,
    ) -> Outcome {
        if key == MenuKey::Back {
            return Outcome::Closed;
        }
        if rows.is_empty() {
            return Outcome::None;
        }
        self.cursor = self.cursor.min(rows.len() - 1);
        let before = self.cursor;
        match key {
            MenuKey::Up => {
                self.cursor = if before == 0 {
                    rows.len() - 1
                } else {
                    before - 1
                };
                moved(before, self.cursor)
            }
            MenuKey::Down => {
                self.cursor = (before + 1) % rows.len();
                moved(before, self.cursor)
            }
            MenuKey::Left | MenuKey::Right | MenuKey::Confirm => {
                let row = rows[before];
                let dir = if key == MenuKey::Left { -1 } else { 1 };
                match row.kind {
                    Kind::Choice if step(row.id, dir) => Outcome::Changed(row.id),
                    Kind::Choice => Outcome::None,
                    Kind::Action if key == MenuKey::Left => Outcome::None,
                    Kind::Action => Outcome::Chosen(row.id),
                }
            }
            MenuKey::Back => Outcome::Closed,
        }
    }
}

fn moved(before: usize, after: usize) -> Outcome {
    if before == after {
        Outcome::None
    } else {
        Outcome::Moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: [Entry; 3] = [
        Entry {
            id: RowId(1),
            kind: Kind::Choice,
        },
        Entry {
            id: RowId(2),
            kind: Kind::Choice,
        },
        Entry {
            id: RowId(3),
            kind: Kind::Action,
        },
    ];

    #[test]
    fn up_and_down_wrap() {
        let mut m = Menu::new();
        assert_eq!(m.key(MenuKey::Up, &ROWS, |_, _| false), Outcome::Moved);
        assert_eq!(m.cursor(), 2);
        assert_eq!(m.key(MenuKey::Down, &ROWS, |_, _| false), Outcome::Moved);
        assert_eq!(m.cursor(), 0);
    }

    #[test]
    fn a_single_row_does_not_move() {
        let mut m = Menu::new();
        assert_eq!(
            m.key(MenuKey::Down, &ROWS[..1], |_, _| false),
            Outcome::None
        );
    }

    #[test]
    fn left_steps_back_and_right_and_confirm_step_forward() {
        let mut m = Menu::new();
        let mut seen = std::vec::Vec::new();
        for k in [MenuKey::Left, MenuKey::Right, MenuKey::Confirm] {
            let out = m.key(k, &ROWS, |id, d| {
                seen.push((id.0, d));
                true
            });
            assert_eq!(out, Outcome::Changed(RowId(1)));
        }
        assert_eq!(seen, [(1, -1), (1, 1), (1, 1)]);
    }

    #[test]
    fn an_unchanged_step_is_none() {
        let mut m = Menu::new();
        assert_eq!(m.key(MenuKey::Right, &ROWS, |_, _| false), Outcome::None);
    }

    #[test]
    fn action_rows_are_chosen_and_left_ignores_them() {
        let mut m = Menu::new();
        m.set_cursor(2, &ROWS);
        assert_eq!(m.key(MenuKey::Left, &ROWS, |_, _| true), Outcome::None);
        assert_eq!(
            m.key(MenuKey::Confirm, &ROWS, |_, _| true),
            Outcome::Chosen(RowId(3))
        );
        assert_eq!(
            m.key(MenuKey::Right, &ROWS, |_, _| true),
            Outcome::Chosen(RowId(3))
        );
    }

    #[test]
    fn back_closes_and_an_empty_list_is_inert() {
        let mut m = Menu::new();
        assert_eq!(m.key(MenuKey::Back, &ROWS, |_, _| true), Outcome::Closed);
        assert_eq!(m.key(MenuKey::Down, &[], |_, _| true), Outcome::None);
        assert_eq!(m.key(MenuKey::Back, &[], |_, _| true), Outcome::Closed);
    }
}
