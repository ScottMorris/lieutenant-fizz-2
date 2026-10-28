// The Options screen: its rows for each platform, every label and value word, and the rules for stepping a value.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `Values` holds every setting; `Caps` says what this platform and machine can do; `OptionsScreen` holds the cursor and applies
// keys. A value this machine cannot use is skipped when cycling. A saved choice whose device is missing is still shown, with
// "(not found)", and is kept until the player changes it. A row set by a command-line switch shows `*` until the player changes
// it, which drops the switch's value for the rest of the run.
//
// The Web page has no devices, so its Sound and Music rows are On or Off; they keep the same fields as DOS' Effects and Music
// (Auto is On), and `Values::to_prefs` writes them that way. The Web's `System` motion has no `Prefs` form and saves as Full.

use fizz_machine::{
    Captions, FrameRate, FxChoice, Keys, Motion, MusicChoice, PlacementMode, Prefs, Resolved,
    SoundState,
};
use fizz_machine::{FxPlan, MusicPlan};

use crate::menu::{Entry, Kind, Menu, MenuKey, Outcome, RowId};
use crate::text::Text;

/// Which presentation is asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Dos,
    Web,
}

/// A sound device, for the text of an Auto row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    None,
    Sb,
    AdLib,
    Speaker,
}

impl Device {
    fn word(self) -> &'static str {
        match self {
            Device::None => "off",
            Device::Sb => "Sound Blaster",
            Device::AdLib => "AdLib",
            Device::Speaker => "PC speaker",
        }
    }
}

/// What the platform and this machine offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    pub platform: Platform,
    /// A Sound Blaster is present (DOS only).
    pub sb: bool,
    /// An FM chip is present (DOS only).
    pub adlib: bool,
    /// The PC speaker is present (DOS only).
    pub speaker: bool,
    /// What music Auto would use on this machine.
    pub auto_music: Device,
    /// What effects Auto would use on this machine.
    pub auto_fx: Device,
    /// The frame rate the governor is drawing at now: 60 or 30. Shown in "Auto (60 Hz)".
    pub rate: u32,
    /// The Web shows the Fullscreen row only when the page can go fullscreen.
    pub fullscreen: bool,
}

impl Caps {
    /// The DOS capabilities. `auto` is the plan the resolver gives for the default settings (`resolve(&Settings::default(),
    /// &probe)`), not the plan for the player's current choice, so "Auto (...)" names what Auto would do. `rate` is the
    /// governor's current rate.
    pub fn dos(auto: &Resolved, rate: u32) -> Caps {
        Caps {
            platform: Platform::Dos,
            sb: auto.states.sb != SoundState::Missing,
            adlib: auto.states.adlib != SoundState::Missing,
            speaker: auto.states.speaker != SoundState::Missing,
            auto_music: match auto.music {
                MusicPlan::Off => Device::None,
                MusicPlan::Opl2 => Device::AdLib,
                MusicPlan::Opl3 => Device::Sb,
            },
            auto_fx: match auto.fx {
                FxPlan::Off => Device::None,
                FxPlan::Ch5 => Device::AdLib,
                FxPlan::Ch14 => Device::Sb,
                FxPlan::Speaker => Device::Speaker,
            },
            rate,
            fullscreen: false,
        }
    }

    /// The Web capabilities; `fullscreen` is whether the page can go fullscreen.
    pub const fn web(fullscreen: bool) -> Caps {
        Caps {
            platform: Platform::Web,
            sb: false,
            adlib: false,
            speaker: false,
            auto_music: Device::None,
            auto_fx: Device::None,
            rate: 60,
            fullscreen,
        }
    }
}

/// The Motion setting as the screen has it. The Web adds `System`, which follows the device's reduced-motion preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionPick {
    System,
    #[default]
    Full,
    Reduced,
}

/// Every setting the screen edits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Values {
    pub music: MusicChoice,
    pub fx: FxChoice,
    pub frame_rate: FrameRate,
    pub motion: MotionPick,
    pub placement: PlacementMode,
    pub captions: Captions,
    pub keys: Keys,
    /// Web only; not in `Prefs`.
    pub fullscreen: bool,
}

impl Values {
    /// The values for saved `prefs`.
    pub fn from_prefs(prefs: &Prefs) -> Values {
        Values {
            music: prefs.sound.music,
            fx: prefs.sound.fx,
            frame_rate: prefs.frame_rate,
            motion: match prefs.motion {
                Motion::Full => MotionPick::Full,
                Motion::Reduced => MotionPick::Reduced,
            },
            placement: prefs.placement,
            captions: prefs.captions,
            keys: prefs.keys,
            fullscreen: false,
        }
    }

    /// The `Prefs` to save through `FIZZ2.CFG`. `System` motion saves as Full.
    pub fn to_prefs(&self) -> Prefs {
        let mut prefs = Prefs::default();
        prefs.sound.music = self.music;
        prefs.sound.fx = self.fx;
        prefs.frame_rate = self.frame_rate;
        prefs.motion = match self.motion {
            MotionPick::Reduced => Motion::Reduced,
            _ => Motion::Full,
        };
        prefs.placement = self.placement;
        prefs.captions = self.captions;
        prefs.keys = self.keys;
        prefs
    }
}

impl RowId {
    pub const MUSIC: RowId = RowId(0);
    pub const EFFECTS: RowId = RowId(1);
    pub const PLACEMENT: RowId = RowId(2);
    pub const FRAME_RATE: RowId = RowId(3);
    pub const MOTION: RowId = RowId(4);
    pub const CAPTIONS: RowId = RowId(5);
    pub const KEYS: RowId = RowId(6);
    pub const BACK: RowId = RowId(7);
    /// The Web's Sound row (On or Off for the effects).
    pub const SOUND: RowId = RowId(8);
    pub const FULLSCREEN: RowId = RowId(9);
}

/// The note to draw when any row shows `*`.
pub const OVERRIDE_NOTE: &str = "* set by a switch; changing it applies to this run";

/// The rows a command-line switch has set for this run, as a bit for each row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overrides(u16);

impl Overrides {
    pub const NONE: Overrides = Overrides(0);

    pub const fn with(self, row: RowId) -> Overrides {
        Overrides(self.0 | (1 << row.0))
    }

    pub const fn has(self, row: RowId) -> bool {
        self.0 & (1 << row.0) != 0
    }

    pub const fn any(self) -> bool {
        self.0 != 0
    }

    fn clear(&mut self, row: RowId) {
        self.0 &= !(1 << row.0);
    }
}

/// One row to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowView {
    pub id: RowId,
    pub label: &'static str,
    /// The value word, already with " (not found)" or " *" when they apply. Empty for an action row.
    pub value: Text,
    pub selected: bool,
    /// A switch set this row for the run; `value` ends in " *".
    pub overridden: bool,
}

const BLANK: RowView = RowView {
    id: RowId(0),
    label: "",
    value: Text::EMPTY,
    selected: false,
    overridden: false,
};

/// The most rows any screen has.
pub const MAX_ROWS: usize = 10;

/// The rows of a screen, in a fixed array.
#[derive(Clone, Copy, Debug)]
pub struct RowList {
    rows: [RowView; MAX_ROWS],
    len: usize,
}

impl RowList {
    pub const fn new() -> RowList {
        RowList {
            rows: [BLANK; MAX_ROWS],
            len: 0,
        }
    }

    pub(crate) fn push(&mut self, row: RowView) {
        if self.len < MAX_ROWS {
            self.rows[self.len] = row;
            self.len += 1;
        }
    }

    pub fn as_slice(&self) -> &[RowView] {
        &self.rows[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for RowList {
    fn default() -> Self {
        Self::new()
    }
}

const DOS_ROWS: [RowId; 8] = [
    RowId::MUSIC,
    RowId::EFFECTS,
    RowId::PLACEMENT,
    RowId::FRAME_RATE,
    RowId::MOTION,
    RowId::CAPTIONS,
    RowId::KEYS,
    RowId::BACK,
];

const WEB_ROWS: [RowId; 8] = [
    RowId::SOUND,
    RowId::MUSIC,
    RowId::PLACEMENT,
    RowId::MOTION,
    RowId::CAPTIONS,
    RowId::KEYS,
    RowId::FULLSCREEN,
    RowId::BACK,
];

/// The ids of the rows on `caps`' platform, in order.
fn ids(caps: &Caps) -> ([RowId; 8], usize) {
    match caps.platform {
        Platform::Dos => (DOS_ROWS, DOS_ROWS.len()),
        Platform::Web => {
            let mut out = [RowId::BACK; 8];
            let mut n = 0;
            for id in WEB_ROWS {
                if id == RowId::FULLSCREEN && !caps.fullscreen {
                    continue;
                }
                out[n] = id;
                n += 1;
            }
            (out, n)
        }
    }
}

fn label(platform: Platform, id: RowId) -> &'static str {
    match (platform, id) {
        (_, RowId::MUSIC) => "Music",
        (_, RowId::EFFECTS) => "Effects",
        (Platform::Dos, RowId::PLACEMENT) => "Sound placement",
        (Platform::Web, RowId::PLACEMENT) => "Placement",
        (_, RowId::FRAME_RATE) => "Frame rate",
        (_, RowId::MOTION) => "Motion",
        (_, RowId::CAPTIONS) => "Captions",
        (_, RowId::KEYS) => "Keys",
        (_, RowId::BACK) => "Back",
        (_, RowId::SOUND) => "Sound",
        (_, RowId::FULLSCREEN) => "Fullscreen",
        _ => "",
    }
}

/// How many choices a row cycles through.
fn count(platform: Platform, id: RowId) -> usize {
    match (platform, id) {
        (Platform::Dos, RowId::MUSIC) => 4,
        (Platform::Dos, RowId::EFFECTS) => 5,
        (Platform::Web, RowId::MOTION) => 3,
        (_, RowId::FRAME_RATE) => 3,
        (_, RowId::BACK) => 0,
        _ => 2,
    }
}

const MUSIC: [MusicChoice; 4] = [
    MusicChoice::Auto,
    MusicChoice::Sb,
    MusicChoice::AdLib,
    MusicChoice::Off,
];
const FX: [FxChoice; 5] = [
    FxChoice::Auto,
    FxChoice::Sb,
    FxChoice::AdLib,
    FxChoice::Speaker,
    FxChoice::Off,
];
const RATES: [FrameRate; 3] = [FrameRate::Auto, FrameRate::Hz60, FrameRate::Hz30];
const MOTION_DOS: [MotionPick; 2] = [MotionPick::Full, MotionPick::Reduced];
const MOTION_WEB: [MotionPick; 3] = [MotionPick::System, MotionPick::Reduced, MotionPick::Full];

/// The position of the current value of a row in its list of choices.
fn index(platform: Platform, id: RowId, v: &Values) -> usize {
    let on = |off: bool| usize::from(off);
    match (platform, id) {
        (Platform::Dos, RowId::MUSIC) => MUSIC.iter().position(|m| *m == v.music).unwrap_or(0),
        (Platform::Web, RowId::MUSIC) => on(v.music == MusicChoice::Off),
        (Platform::Dos, RowId::EFFECTS) => FX.iter().position(|f| *f == v.fx).unwrap_or(0),
        (_, RowId::SOUND) => on(v.fx == FxChoice::Off),
        (_, RowId::PLACEMENT) => on(v.placement == PlacementMode::Classic),
        (_, RowId::FRAME_RATE) => RATES.iter().position(|r| *r == v.frame_rate).unwrap_or(0),
        (Platform::Dos, RowId::MOTION) => {
            MOTION_DOS.iter().position(|m| *m == v.motion).unwrap_or(0)
        }
        (Platform::Web, RowId::MOTION) => {
            MOTION_WEB.iter().position(|m| *m == v.motion).unwrap_or(0)
        }
        (_, RowId::CAPTIONS) => on(v.captions == Captions::Off),
        (_, RowId::KEYS) => on(v.keys == Keys::Modern),
        (_, RowId::FULLSCREEN) => on(v.fullscreen),
        _ => 0,
    }
}

/// Sets a row to its `i`th choice.
fn set(platform: Platform, id: RowId, v: &mut Values, i: usize) {
    match (platform, id) {
        (Platform::Dos, RowId::MUSIC) => v.music = MUSIC[i],
        (Platform::Web, RowId::MUSIC) => {
            v.music = if i == 0 {
                MusicChoice::Auto
            } else {
                MusicChoice::Off
            }
        }
        (Platform::Dos, RowId::EFFECTS) => v.fx = FX[i],
        (_, RowId::SOUND) => {
            v.fx = if i == 0 {
                FxChoice::Auto
            } else {
                FxChoice::Off
            }
        }
        (_, RowId::PLACEMENT) => {
            v.placement = if i == 0 {
                PlacementMode::Enhanced
            } else {
                PlacementMode::Classic
            }
        }
        (_, RowId::FRAME_RATE) => v.frame_rate = RATES[i],
        (Platform::Dos, RowId::MOTION) => v.motion = MOTION_DOS[i],
        (Platform::Web, RowId::MOTION) => v.motion = MOTION_WEB[i],
        (_, RowId::CAPTIONS) => v.captions = if i == 0 { Captions::On } else { Captions::Off },
        (_, RowId::KEYS) => v.keys = if i == 0 { Keys::Keen } else { Keys::Modern },
        (_, RowId::FULLSCREEN) => v.fullscreen = i == 1,
        _ => {}
    }
}

/// Whether this machine can use the `i`th choice of a row.
fn available(caps: &Caps, id: RowId, i: usize) -> bool {
    if caps.platform == Platform::Web {
        return true;
    }
    match id {
        RowId::MUSIC => match MUSIC[i] {
            MusicChoice::Sb => caps.sb,
            MusicChoice::AdLib => caps.adlib,
            _ => true,
        },
        RowId::EFFECTS => match FX[i] {
            FxChoice::Sb => caps.sb,
            FxChoice::AdLib => caps.adlib,
            FxChoice::Speaker => caps.speaker,
            _ => true,
        },
        _ => true,
    }
}

/// The word for the `i`th choice of a row, with Auto's resolved device.
fn word(caps: &Caps, id: RowId, i: usize) -> Text {
    let on_off = |off: bool| if off { "Off" } else { "On" };
    match (caps.platform, id) {
        (Platform::Dos, RowId::MUSIC) => match MUSIC[i] {
            MusicChoice::Auto => Text::join(&["Auto (", caps.auto_music.word(), ")"]),
            MusicChoice::Sb => Text::join(&["Sound Blaster"]),
            MusicChoice::AdLib => Text::join(&["AdLib"]),
            MusicChoice::Off => Text::join(&["Off"]),
        },
        (Platform::Dos, RowId::EFFECTS) => match FX[i] {
            FxChoice::Auto => Text::join(&["Auto (", caps.auto_fx.word(), ")"]),
            FxChoice::Sb => Text::join(&["Sound Blaster"]),
            FxChoice::AdLib => Text::join(&["AdLib"]),
            FxChoice::Speaker => Text::join(&["PC speaker"]),
            FxChoice::Off => Text::join(&["Off"]),
        },
        (Platform::Web, RowId::MUSIC) | (_, RowId::SOUND) => Text::join(&[on_off(i == 1)]),
        (_, RowId::PLACEMENT) => Text::join(&[["Enhanced", "Classic"][i]]),
        (_, RowId::FRAME_RATE) => match RATES[i] {
            FrameRate::Auto => {
                let hz = if caps.rate == 30 { "30 Hz" } else { "60 Hz" };
                Text::join(&["Auto (", hz, ")"])
            }
            FrameRate::Hz60 => Text::join(&["60 Hz"]),
            FrameRate::Hz30 => Text::join(&["30 Hz"]),
        },
        (Platform::Dos, RowId::MOTION) => Text::join(&[motion_word(MOTION_DOS[i])]),
        (Platform::Web, RowId::MOTION) => Text::join(&[motion_word(MOTION_WEB[i])]),
        (_, RowId::CAPTIONS) => Text::join(&[on_off(i == 1)]),
        (_, RowId::KEYS) => Text::join(&[["Keen", "Modern"][i]]),
        (_, RowId::FULLSCREEN) => Text::join(&[on_off(i == 0)]),
        _ => Text::EMPTY,
    }
}

fn motion_word(m: MotionPick) -> &'static str {
    match m {
        MotionPick::System => "System",
        MotionPick::Full => "Full",
        MotionPick::Reduced => "Reduced",
    }
}

/// The Options screen: a cursor, the values and which rows a switch overrode.
#[derive(Clone, Copy, Debug)]
pub struct OptionsScreen {
    /// The index of the selected row in the platform's row list.
    pub cursor: usize,
    pub values: Values,
    /// Rows a switch set for this run; each is cleared when the player changes that row.
    pub overridden: Overrides,
    /// Set once any value really differs after a key; a presenter saves `values.to_prefs()` when it leaves the screen with this
    /// set. Moving the cursor, choosing Back and a step that changes nothing never set it.
    pub changed: bool,
}

impl OptionsScreen {
    pub const fn new(values: Values, overridden: Overrides) -> OptionsScreen {
        OptionsScreen {
            cursor: 0,
            values,
            overridden,
            changed: false,
        }
    }

    /// Handles a key. `Closed` is returned for Back and for choosing the Back row; `Changed(row)` for a value that moved.
    pub fn key(&mut self, key: MenuKey, caps: &Caps) -> Outcome {
        let (ids, n) = ids(caps);
        let mut entries = [Entry {
            id: RowId::BACK,
            kind: Kind::Action,
        }; 8];
        for (e, id) in entries.iter_mut().zip(ids).take(n) {
            e.id = id;
            e.kind = if id == RowId::BACK {
                Kind::Action
            } else {
                Kind::Choice
            };
        }
        let mut menu = Menu::new();
        menu.set_cursor(self.cursor, &entries[..n]);
        let platform = caps.platform;
        let values = &mut self.values;
        let out = menu.key(key, &entries[..n], |id, dir| {
            step(caps, platform, id, values, dir)
        });
        self.cursor = menu.cursor();
        match out {
            Outcome::Changed(id) => {
                self.changed = true;
                self.overridden.clear(id);
                out
            }
            Outcome::Chosen(id) if id == RowId::BACK => Outcome::Closed,
            other => other,
        }
    }

    /// The rows to draw.
    pub fn rows(&self, caps: &Caps) -> RowList {
        let (ids, n) = ids(caps);
        let mut list = RowList::new();
        for (pos, id) in ids.iter().copied().take(n).enumerate() {
            let overridden = self.overridden.has(id);
            let value = if count(caps.platform, id) == 0 {
                Text::EMPTY
            } else {
                let i = index(caps.platform, id, &self.values);
                let w = word(caps, id, i);
                let missing = if available(caps, id, i) {
                    ""
                } else {
                    " (not found)"
                };
                Text::join(&[w.as_str(), missing, if overridden { " *" } else { "" }])
            };
            list.push(RowView {
                id,
                label: label(caps.platform, id),
                value,
                selected: pos == self.cursor,
                overridden,
            });
        }
        list
    }

    /// True when any row shows `*`, so the screen should draw `OVERRIDE_NOTE`.
    pub fn show_note(&self) -> bool {
        self.overridden.any()
    }
}

/// Moves a row's value one choice in `dir`, wrapping and skipping choices this machine cannot use. Returns whether it moved.
fn step(caps: &Caps, platform: Platform, id: RowId, v: &mut Values, dir: i8) -> bool {
    let n = count(platform, id);
    if n < 2 {
        return false;
    }
    let cur = index(platform, id, v);
    for k in 1..n {
        let j = if dir < 0 {
            (cur + n - k) % n
        } else {
            (cur + k) % n
        };
        if available(caps, id, j) {
            let before = *v;
            set(platform, id, v, j);
            return *v != before;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TEXT_CAP;
    use fizz_machine::{OplMode, SoundStates};

    fn resolved(sb: SoundState, adlib: SoundState, music: MusicPlan, fx: FxPlan) -> Resolved {
        Resolved {
            opl: OplMode::Opl2,
            music,
            fx,
            music_reason: None,
            fx_reason: None,
            states: SoundStates {
                speaker: SoundState::Available,
                adlib,
                sb,
            },
        }
    }

    fn sb16() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::InUse,
                SoundState::InUse,
                MusicPlan::Opl3,
                FxPlan::Ch14,
            ),
            60,
        )
    }

    fn adlib_only() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::Missing,
                SoundState::InUse,
                MusicPlan::Opl2,
                FxPlan::Ch5,
            ),
            60,
        )
    }

    fn no_card() -> Caps {
        Caps::dos(
            &resolved(
                SoundState::Missing,
                SoundState::Missing,
                MusicPlan::Off,
                FxPlan::Speaker,
            ),
            30,
        )
    }

    fn screen() -> OptionsScreen {
        OptionsScreen::new(Values::default(), Overrides::NONE)
    }

    fn goto(s: &mut OptionsScreen, caps: &Caps, row: RowId) {
        while s.rows(caps).as_slice()[s.cursor].id != row {
            s.key(MenuKey::Down, caps);
        }
    }

    fn value(s: &OptionsScreen, caps: &Caps, row: RowId) -> std::string::String {
        let list = s.rows(caps);
        let r = list.as_slice().iter().find(|r| r.id == row).unwrap();
        std::string::String::from(r.value.as_str())
    }

    fn ids_of(s: &OptionsScreen, caps: &Caps) -> std::vec::Vec<(&'static str, u8)> {
        s.rows(caps)
            .as_slice()
            .iter()
            .map(|r| (r.label, r.id.0))
            .collect()
    }

    #[test]
    fn dos_rows_match_the_plan() {
        let labels: std::vec::Vec<_> = ids_of(&screen(), &sb16()).iter().map(|r| r.0).collect();
        assert_eq!(
            labels,
            [
                "Music",
                "Effects",
                "Sound placement",
                "Frame rate",
                "Motion",
                "Captions",
                "Keys",
                "Back"
            ]
        );
    }

    #[test]
    fn web_rows_match_the_plan_and_fullscreen_is_conditional() {
        let with: std::vec::Vec<_> = ids_of(&screen(), &Caps::web(true))
            .iter()
            .map(|r| r.0)
            .collect();
        assert_eq!(
            with,
            [
                "Sound",
                "Music",
                "Placement",
                "Motion",
                "Captions",
                "Keys",
                "Fullscreen",
                "Back"
            ]
        );
        let without = ids_of(&screen(), &Caps::web(false));
        assert_eq!(without.len(), 7);
        assert!(without.iter().all(|r| r.0 != "Fullscreen"));
    }

    #[test]
    fn up_wraps_to_back_and_back_row_closes() {
        let caps = sb16();
        let mut s = screen();
        assert_eq!(s.key(MenuKey::Up, &caps), Outcome::Moved);
        assert_eq!(s.cursor, 7);
        assert_eq!(s.key(MenuKey::Confirm, &caps), Outcome::Closed);
        assert_eq!(s.key(MenuKey::Back, &caps), Outcome::Closed);
        assert!(!s.changed);
    }

    #[test]
    fn right_and_confirm_step_forward_and_left_back_with_wrap() {
        let caps = sb16();
        let mut s = screen();
        // Music: Auto, Sound Blaster, AdLib, Off.
        assert_eq!(s.key(MenuKey::Right, &caps), Outcome::Changed(RowId::MUSIC));
        assert_eq!(s.values.music, MusicChoice::Sb);
        s.key(MenuKey::Confirm, &caps);
        assert_eq!(s.values.music, MusicChoice::AdLib);
        s.key(MenuKey::Right, &caps);
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.music, MusicChoice::Auto);
        s.key(MenuKey::Left, &caps);
        assert_eq!(s.values.music, MusicChoice::Off);
        s.key(MenuKey::Left, &caps);
        assert_eq!(s.values.music, MusicChoice::AdLib);
    }

    #[test]
    fn unavailable_values_are_skipped_when_cycling() {
        let caps = adlib_only();
        let mut s = screen();
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.music, MusicChoice::AdLib);
        let caps = no_card();
        let mut s = screen();
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.music, MusicChoice::Off);
        goto(&mut s, &caps, RowId::EFFECTS);
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.fx, FxChoice::Speaker);
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.fx, FxChoice::Off);
        s.key(MenuKey::Left, &caps);
        s.key(MenuKey::Left, &caps);
        assert_eq!(s.values.fx, FxChoice::Auto);
    }

    #[test]
    fn a_missing_saved_device_is_shown_and_kept() {
        let caps = adlib_only();
        let values = Values {
            music: MusicChoice::Sb,
            ..Values::default()
        };
        let mut s = OptionsScreen::new(values, Overrides::NONE);
        assert_eq!(value(&s, &caps, RowId::MUSIC), "Sound Blaster (not found)");
        // Moving the cursor and leaving the screen keep the saved choice.
        s.key(MenuKey::Down, &caps);
        s.key(MenuKey::Back, &caps);
        assert_eq!(s.values.to_prefs().sound.music, MusicChoice::Sb);
        assert!(!s.changed);
        // Stepping forward from it goes to the next usable choice.
        s.cursor = 0;
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.music, MusicChoice::AdLib);
        assert_eq!(value(&s, &caps, RowId::MUSIC), "AdLib");
    }

    #[test]
    fn auto_names_what_it_resolves_to() {
        assert_eq!(
            value(&screen(), &sb16(), RowId::MUSIC),
            "Auto (Sound Blaster)"
        );
        assert_eq!(
            value(&screen(), &adlib_only(), RowId::EFFECTS),
            "Auto (AdLib)"
        );
        assert_eq!(value(&screen(), &no_card(), RowId::MUSIC), "Auto (off)");
        assert_eq!(
            value(&screen(), &no_card(), RowId::EFFECTS),
            "Auto (PC speaker)"
        );
        assert_eq!(
            value(&screen(), &no_card(), RowId::FRAME_RATE),
            "Auto (30 Hz)"
        );
        assert_eq!(value(&screen(), &sb16(), RowId::FRAME_RATE), "Auto (60 Hz)");
    }

    #[test]
    fn an_override_shows_a_star_and_changing_the_row_clears_it() {
        let caps = sb16();
        let mut s = OptionsScreen::new(
            Values {
                motion: MotionPick::Reduced,
                ..Values::default()
            },
            Overrides::NONE.with(RowId::MOTION).with(RowId::KEYS),
        );
        assert!(s.show_note());
        assert_eq!(value(&s, &caps, RowId::MOTION), "Reduced *");
        assert!(
            s.rows(&caps)
                .as_slice()
                .iter()
                .find(|r| r.id == RowId::MOTION)
                .unwrap()
                .overridden
        );
        goto(&mut s, &caps, RowId::MOTION);
        s.key(MenuKey::Right, &caps);
        assert_eq!(value(&s, &caps, RowId::MOTION), "Full");
        assert!(!s.overridden.has(RowId::MOTION));
        // Another row's star stays until that row changes.
        assert!(s.overridden.has(RowId::KEYS));
        assert!(s.show_note());
    }

    #[test]
    fn changed_is_set_only_by_a_real_change() {
        let caps = sb16();
        let mut s = screen();
        s.key(MenuKey::Left, &caps);
        assert!(s.changed);
        let mut s = screen();
        for k in [MenuKey::Down, MenuKey::Up, MenuKey::Down] {
            s.key(k, &caps);
        }
        assert!(!s.changed);
        s.cursor = 7;
        s.key(MenuKey::Confirm, &caps);
        s.key(MenuKey::Left, &caps);
        assert!(!s.changed);
    }

    #[test]
    fn a_step_that_changes_nothing_is_none() {
        let mut v = Values::default();
        let caps = sb16();
        assert!(!step(&caps, Platform::Dos, RowId::BACK, &mut v, 1));
        assert_eq!(v, Values::default());
    }

    #[test]
    fn dos_motion_skips_system_and_web_has_three() {
        let caps = sb16();
        let mut s = screen();
        goto(&mut s, &caps, RowId::MOTION);
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.motion, MotionPick::Reduced);
        s.key(MenuKey::Right, &caps);
        assert_eq!(s.values.motion, MotionPick::Full);

        let web = Caps::web(false);
        let mut s = OptionsScreen::new(
            Values {
                motion: MotionPick::System,
                ..Values::default()
            },
            Overrides::NONE,
        );
        goto(&mut s, &web, RowId::MOTION);
        assert_eq!(value(&s, &web, RowId::MOTION), "System");
        s.key(MenuKey::Right, &web);
        assert_eq!(value(&s, &web, RowId::MOTION), "Reduced");
        s.key(MenuKey::Right, &web);
        assert_eq!(value(&s, &web, RowId::MOTION), "Full");
        s.key(MenuKey::Right, &web);
        assert_eq!(value(&s, &web, RowId::MOTION), "System");
    }

    #[test]
    fn web_sound_and_music_are_on_or_off() {
        let web = Caps::web(true);
        let mut s = screen();
        assert_eq!(value(&s, &web, RowId::SOUND), "On");
        s.key(MenuKey::Right, &web);
        assert_eq!(value(&s, &web, RowId::SOUND), "Off");
        assert_eq!(s.values.fx, FxChoice::Off);
        s.key(MenuKey::Down, &web);
        s.key(MenuKey::Right, &web);
        assert_eq!(value(&s, &web, RowId::MUSIC), "Off");
        assert_eq!(s.values.music, MusicChoice::Off);
        goto(&mut s, &web, RowId::FULLSCREEN);
        assert_eq!(value(&s, &web, RowId::FULLSCREEN), "Off");
        s.key(MenuKey::Confirm, &web);
        assert_eq!(value(&s, &web, RowId::FULLSCREEN), "On");
    }

    #[test]
    fn the_small_rows_have_their_words() {
        let caps = sb16();
        let mut s = screen();
        for (row, first, second) in [
            (RowId::PLACEMENT, "Enhanced", "Classic"),
            (RowId::CAPTIONS, "On", "Off"),
            (RowId::KEYS, "Keen", "Modern"),
        ] {
            goto(&mut s, &caps, row);
            assert_eq!(value(&s, &caps, row), first);
            s.key(MenuKey::Right, &caps);
            assert_eq!(value(&s, &caps, row), second);
        }
        goto(&mut s, &caps, RowId::FRAME_RATE);
        s.key(MenuKey::Right, &caps);
        assert_eq!(value(&s, &caps, RowId::FRAME_RATE), "60 Hz");
        s.key(MenuKey::Right, &caps);
        assert_eq!(value(&s, &caps, RowId::FRAME_RATE), "30 Hz");
    }

    #[test]
    fn values_round_trip_through_prefs() {
        let mut prefs = Prefs::default();
        prefs.sound.music = MusicChoice::AdLib;
        prefs.sound.fx = FxChoice::Speaker;
        prefs.frame_rate = FrameRate::Hz30;
        prefs.motion = Motion::Reduced;
        prefs.placement = PlacementMode::Classic;
        prefs.captions = Captions::Off;
        prefs.keys = Keys::Modern;
        assert_eq!(Values::from_prefs(&prefs).to_prefs(), prefs);
        assert_eq!(Values::from_prefs(&Prefs::default()), Values::default());
        let system = Values {
            motion: MotionPick::System,
            ..Values::default()
        };
        assert_eq!(system.to_prefs().motion, Motion::Full);
    }

    #[test]
    fn every_label_and_value_fits_the_capacity() {
        let all_caps = [sb16(), adlib_only(), no_card(), Caps::web(true)];
        for caps in all_caps {
            let (ids, n) = ids(&caps);
            for id in ids.iter().take(n) {
                assert!(label(caps.platform, *id).len() <= TEXT_CAP);
                for i in 0..count(caps.platform, *id) {
                    // The longest form: a device word that can be missing, marked as overridden. Auto and Off are never missing.
                    let w = word(&caps, *id, i);
                    let missing = if w.as_str().starts_with("Auto") {
                        ""
                    } else {
                        " (not found)"
                    };
                    assert!(
                        Text::fits(&[w.as_str(), missing, " *"]),
                        "{} {}",
                        label(caps.platform, *id),
                        w.as_str()
                    );
                    assert!(w.len() < TEXT_CAP);
                }
            }
        }
        // The rows of the worst case: everything missing and overridden.
        let caps = no_card();
        let s = OptionsScreen::new(
            Values {
                music: MusicChoice::Sb,
                fx: FxChoice::Sb,
                ..Values::default()
            },
            Overrides(0xFFFF),
        );
        for r in s.rows(&caps).as_slice() {
            assert!(r.value.len() <= TEXT_CAP);
            assert!(!r.value.as_str().contains('?'));
        }
        assert_eq!(
            value(&s, &caps, RowId::EFFECTS),
            "Sound Blaster (not found) *"
        );
    }
}
