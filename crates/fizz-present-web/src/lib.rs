// Web presenter: paces the slice's rules from browser time, draws them and exposes them to JavaScript.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The page pushes `performance.now()` into `App::frame`. A millisecond `WebClock` and the shared integer `Pacer` turn that into
// whole 60 Hz ticks, so the Web build paces exactly as the DOS build does from its PIT. Rules never see the browser clock.
mod menus;

use core::cell::Cell;
use fizz_hal::{Clock, Pacer, Ticks};
use fizz_menu::MenuKey;
use fizz_sfx::place::Placement;
use fizz_sim::{Buttons, InputFrame, InputLatch, TICK_HZ};
use fizz_slice_view::{
    CAPTION_TICKS, CaptionView, DrawOptions, EventCaptions, HudAnim, Puffs, ben_view_box,
    caption_view, draw_slice_puffs, event_sound,
};
use menus::{Menus, Screen};
use wasm_bindgen::prelude::*;

/// The height of the Web view in pixels: 13 tiles of 16, the number of tiles Fizz 1 shows from top to bottom in a level. The DOS
/// picture stays 200 tall; the Web view is not a projection of it.
pub const WEB_VIEW_H: usize = 208;

/// The default Web view width, used until the page reports its window with [`App::set_view`].
pub const WEB_VIEW_W: usize = 320;

/// The narrowest and widest view the Web presenter draws, in pixels: 6 tiles, about what Fizz 1 shows across a portrait phone, and
/// 80 tiles. The ceiling is only a sanity bound on the buffers: 1280 by 208 is 266 KB of indices plus 1.06 MB of RGBA, about
/// 1.3 MB, and it covers an aspect of 6.1 (a 32:9 monitor is 3.6). A level narrower than the view is centred, with sky at the sides.
///
/// The view size is for drawing only. `set_view` reaches the view and camera code and nothing else: the rules never read it
/// (no waking, culling or spawning by view size), so the state hash and the events are the same at every size.
pub const MIN_VIEW_W: usize = 96;
pub const MAX_VIEW_W: usize = 1280;

/// The shortest and tallest view [`App::set_view`] accepts, in pixels. The page keeps [`WEB_VIEW_H`]; the limits only keep a
/// stray call from making a huge buffer.
pub const MIN_VIEW_H: usize = 64;
pub const MAX_VIEW_H: usize = 416;

/// Most ticks run for one displayed frame. Time beyond this is dropped, so a hidden tab does not cause a burst on return.
const MAX_CATCH_UP: u32 = 5;

/// Most effects kept between two calls to [`App::take_effect`]. A page that never drains them (a stalled script, say) must
/// not grow the queue without bound; effects past this are dropped, the newest first. One chicken clucks about once a second, so
/// a page that drains each frame never gets near it.
const MAX_PENDING_EFFECTS: usize = 32;

/// A clock of whole milliseconds fed by the time JavaScript pushes in.
#[derive(Debug, Default)]
pub struct WebClock {
    ms: Cell<u32>,
}

impl WebClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the reading from a millisecond timestamp. The fraction is rounded up so that a 60 Hz display, whose frames
    /// arrive 16.67 ms apart, never loses a tick to truncation. The counter wraps like the HAL's other clocks.
    pub fn set_ms(&self, now_ms: f64) {
        let whole = if now_ms.is_finite() && now_ms > 0.0 {
            now_ms.ceil() as u64
        } else {
            0
        };
        self.ms.set(whole as u32);
    }
}

impl Clock for WebClock {
    fn now(&self) -> Ticks {
        Ticks(self.ms.get())
    }

    /// One tick per millisecond is 1000 Hz, which is 1,000,000 millihertz.
    fn millihertz(&self) -> u32 {
        1_000_000
    }
}

/// The sixteen EGA colours as opaque RGBA, in standard order: the table `docs/SPRITES.md` lists, which the DOS presenter loads
/// as DAC values, so the canvas shows the colours the VGA screen shows.
const EGA_RGBA: [[u8; 4]; 16] = {
    const fn c(r: u8, g: u8, b: u8) -> [u8; 4] {
        [r, g, b, 0xFF]
    }
    [
        c(0x00, 0x00, 0x00),
        c(0x00, 0x00, 0xAA),
        c(0x00, 0xAA, 0x00),
        c(0x00, 0xAA, 0xAA),
        c(0xAA, 0x00, 0x00),
        c(0xAA, 0x00, 0xAA),
        c(0xAA, 0x55, 0x00),
        c(0xAA, 0xAA, 0xAA),
        c(0x55, 0x55, 0x55),
        c(0x55, 0x55, 0xFF),
        c(0x55, 0xFF, 0x55),
        c(0x55, 0xFF, 0xFF),
        c(0xFF, 0x55, 0x55),
        c(0xFF, 0x55, 0xFF),
        c(0xFF, 0xFF, 0x55),
        c(0xFF, 0xFF, 0xFF),
    ]
};

/// The slice as the page sees it: paced by the shared integer `Pacer` and a key latch, over `fizz_ep2`'s world.
///
/// `frame` runs whole ticks. `render` draws the world into a palette-indexed buffer, [`WEB_VIEW_H`] tall and as wide as the
/// window's shape asks (see [`App::set_view`]), with the routine the DOS presenter also uses, then expands it to RGBA for the canvas. The page reads that buffer straight out of the module's memory
/// through [`App::rgba_ptr`].
#[wasm_bindgen]
pub struct App {
    world: fizz_ep2::World,
    latch: InputLatch,
    clock: WebClock,
    pacer: Option<Pacer>,
    held: Buttons,
    view_w: usize,
    view_h: usize,
    indexed: Vec<u8>,
    rgba: Vec<u8>,
    /// The effect ids emitted since the page last drained them (today only the cluck: ids 0 to 3, its variant), oldest first, each with where its chicken was in the view at the
    /// step that emitted it. `effect_head` is the next one to hand out.
    effects: Vec<(u8, Placement)>,
    effect_head: usize,
    /// The placement of the effect `take_effect` handed out last, which `effect_pan` and `effect_gain` read.
    taken: Placement,
    /// Whether the cluck captions are shown. The picture never carries them on the Web: the page draws them as text.
    captions: bool,
    /// Whether the view shakes when Ben dies. The page turns it off for Reduced Motion.
    shake: bool,
    /// The live captions of the last [`App::render`], empty when captions are off.
    live_captions: Vec<CaptionView>,
    /// The shot-end puffs on show: view state fed from the step events, drawn into the picture like the DOS presenter's.
    puffs: Puffs,
    /// The event captions (fzzt, clunk and the rest): view state fed from the step events whether or not captions are shown.
    event_captions: EventCaptions,
    /// The HUD's score count-up, `+N` and key rings, fed from the step like the puffs. Motion Reduced is the shake being off.
    hud: HudAnim,
    /// The Paused and Options screens. While one is up `frame` does not step.
    menus: Menus,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new() -> App {
        App {
            world: fizz_ep2::level::world(),
            latch: InputLatch::new(),
            clock: WebClock::new(),
            pacer: None,
            held: Buttons::NONE,
            view_w: WEB_VIEW_W,
            view_h: WEB_VIEW_H,
            indexed: vec![0; WEB_VIEW_W * WEB_VIEW_H],
            rgba: vec![0; WEB_VIEW_W * WEB_VIEW_H * 4],
            effects: Vec::with_capacity(MAX_PENDING_EFFECTS),
            effect_head: 0,
            taken: Placement::CENTRE,
            captions: true,
            shake: true,
            live_captions: Vec::new(),
            puffs: Puffs::new(),
            event_captions: EventCaptions::new(),
            hud: HudAnim::new(),
            menus: Menus::new(),
        }
    }

    /// Records a key edge. `bits` are `fizz_sim::Buttons` bits; repeated downs are not new presses.
    pub fn key(&mut self, bits: u8, down: bool) {
        self.latch.set(Buttons(bits), down);
    }

    /// Advances the rules to browser time `now_ms` and returns how many ticks ran. The first call only starts the pacer.
    ///
    /// The first tick of a call takes the latch's edges, so a press that began and ended between two calls still counts once;
    /// the others see only the held buttons.
    pub fn frame(&mut self, now_ms: f64) -> u32 {
        // A screen being up holds the game. The pacer is dropped, so the first frame after it starts it again from that moment and
        // the time spent in the menu is never caught up.
        if self.menus.screen != Screen::Closed {
            self.hold();
            return 0;
        }
        self.clock.set_ms(now_ms);
        let Some(pacer) = self.pacer.as_mut() else {
            self.pacer = Some(Pacer::new(&self.clock, TICK_HZ, MAX_CATCH_UP));
            return 0;
        };
        let due = pacer.due(&self.clock);
        for i in 0..due {
            let input = if i == 0 {
                let f = self.latch.frame();
                self.held = f.held;
                f
            } else {
                InputFrame {
                    held: self.held,
                    pressed: Buttons::NONE,
                }
            };
            fizz_ep2::step(&mut self.world, input);
            self.collect_events();
        }
        due
    }

    /// The next effect id (0 to `fizz_sfx::EFFECT_COUNT - 1`: the cluck's variants, then fzzt, stun and the rest; see `captions.rs`) the rules emitted since the last call, oldest first, or -1 when there
    /// are none left. Call it until it returns -1 after each [`App::frame`]. Each effect is handed out once and none is lost when
    /// a frame runs several ticks: the events are read after every step, because the rules clear them on the next one. It
    /// allocates nothing. The page decides what an effect sounds like (`assets/sfx/effects.ts`); the rules and the replay transcript never see this.
    ///
    /// An effect that is returned has a place: read [`App::effect_pan`] and [`App::effect_gain`] straight after, before the next call.
    pub fn take_effect(&mut self) -> i32 {
        let Some(&(variant, placement)) = self.effects.get(self.effect_head) else {
            return -1;
        };
        self.taken = placement;
        self.effect_head += 1;
        if self.effect_head == self.effects.len() {
            self.effects.clear();
            self.effect_head = 0;
        }
        i32::from(variant)
    }

    /// The stereo position, from -1 (left) to 1 (right), of the effect [`App::take_effect`] returned last: where the chicken was in the
    /// view when it clucked, with Fizz 1's pan width (0.6, so within +-0.6). The page hands it to an equal-power stereo panner. Classic
    /// sound ignores it and the page plays at 0.
    pub fn effect_pan(&self) -> f32 {
        f32::from(self.taken.pan) / 4096.0
    }

    /// The distance loudness, 1 down to 0.4, of the effect [`App::take_effect`] returned last: 1 inside the view, falling by half a unit
    /// per view-half beyond it. The pan and this are the pure `fizz_sfx::place` function's, the same one DOS uses, so a sound is placed
    /// the same way on both. They depend on the view size the page reported, and the rules never see them.
    pub fn effect_gain(&self) -> f32 {
        f32::from(self.taken.distance) / 4096.0
    }

    /// Starts the level again from its first tick. The pacer keeps running, so no time is lost or repeated. Every held key is
    /// forgotten, so the page releases its own keys when it calls this.
    pub fn reset(&mut self) {
        self.world = fizz_ep2::level::world();
        self.latch = InputLatch::new();
        self.held = Buttons::NONE;
        self.effects.clear();
        self.effect_head = 0;
        self.taken = Placement::CENTRE;
        self.puffs.clear();
        self.event_captions.clear();
        self.hud = HudAnim::new();
    }

    /// Queues the sound of every event the step that just ran emitted, and takes its shot ends for the puffs and its captions for the
    /// caption list. Each event with a sound gets its effect id and where it sounds (`fizz_slice_view::event_sound`, the table in
    /// `captions.rs`), whatever the Captions setting says. The page plays them in order, each its own sound, as Fizz 1 does.
    fn collect_events(&mut self) {
        let options = DrawOptions {
            captions: false,
            shake: self.shake,
        };
        for event in self.world.events() {
            // The camera is the one the picture is drawn with, in the view the page last set.
            if let Some((id, place)) =
                event_sound(&self.world, event, self.view_w, self.view_h, &options)
            {
                queue_effect(&mut self.effects, id, place);
            }
        }
        self.puffs.observe(&self.world);
        self.hud.observe(&self.world, !self.shake);
        self.event_captions
            .observe(&self.world, self.view_w, self.view_h, &options);
    }

    /// Turns the cluck captions on or off (on by default). The beak still moves; only the text goes.
    pub fn set_captions(&mut self, on: bool) {
        self.captions = on;
    }

    /// Turns the death shake on or off (on by default). Off draws the camera still, which is the Reduced Motion setting.
    pub fn set_shake(&mut self, on: bool) {
        self.shake = on;
    }

    /// Whether the view shakes when Ben dies.
    pub fn shake(&self) -> bool {
        self.shake
    }

    /// Whether the cluck captions are shown.
    pub fn captions(&self) -> bool {
        self.captions
    }

    /// How many captions are live after the last [`App::render`]. Zero when captions are off.
    ///
    /// The page draws these as text over the canvas, not as pixels in the picture, so the words keep a readable size at
    /// any canvas size. Read them with [`App::caption_word`] and the other `caption_` accessors for an index below this.
    pub fn caption_count(&self) -> usize {
        self.live_captions.len()
    }

    /// The word of caption `i` (lower-case, as in Fizz 1), or an empty string past the end.
    pub fn caption_word(&self, i: usize) -> String {
        self.live_captions
            .get(i)
            .map_or_else(String::new, |c| c.text.into())
    }

    /// The x of caption `i`, the chicken's centre, in view pixels (0 to `width`; it may lie up to a tile outside, and is not clamped).
    pub fn caption_x(&self, i: usize) -> f64 {
        self.live_captions.get(i).map_or(0.0, |c| f64::from(c.x))
    }

    /// How many view pixels above caption `i`'s y the word rests: 32 (two Fizz 1 tiles) above a chicken's feet, 0 for an event
    /// caption, whose y is the point the event happened at.
    pub fn caption_anchor_above(&self, i: usize) -> f64 {
        self.live_captions
            .get(i)
            .map_or(0.0, |c| if c.event { 0.0 } else { 32.0 })
    }

    /// The y of caption `i`, the chicken's feet, in canvas pixels (0 to `height`; it may lie up to a tile outside, and is not clamped).
    pub fn caption_y(&self, i: usize) -> f64 {
        self.live_captions.get(i).map_or(0.0, |c| f64::from(c.y))
    }

    /// How far into its life caption `i` is, from 0 (new) to just under 1 (gone). It is the same life the pixel caption
    /// has, `CAPTION_TICKS` ticks.
    pub fn caption_progress(&self, i: usize) -> f64 {
        self.live_captions
            .get(i)
            .map_or(1.0, |c| f64::from(c.age) / f64::from(CAPTION_TICKS))
    }

    /// The colour of caption `i` as `#rrggbb`: the variant's bright colour, which Fizz 1's Web text also uses.
    pub fn caption_colour(&self, i: usize) -> String {
        let [r, g, b, _] = self
            .live_captions
            .get(i)
            .map_or([255; 4], |c| EGA_RGBA[usize::from(c.bright & 15)]);
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// Sets the size of the view in pixels and makes the frame buffers match. The width is clamped to [`MIN_VIEW_W`]..=
    /// [`MAX_VIEW_W`] and the height to [`MIN_VIEW_H`]..=[`MAX_VIEW_H`]; read [`App::width`] and [`App::height`] for what was
    /// kept. The page calls this when the window's shape changes: it keeps the height at [`WEB_VIEW_H`] (13 tiles) and sets the
    /// width from the aspect, as Fizz 1 does. Nothing in the rules changes; the buffers are zeroed until the next
    /// [`App::render`], so the page renders before it copies. A call that changes nothing does nothing.
    pub fn set_view(&mut self, w: u32, h: u32) {
        let w = (w as usize).clamp(MIN_VIEW_W, MAX_VIEW_W);
        let h = (h as usize).clamp(MIN_VIEW_H, MAX_VIEW_H);
        if (w, h) == (self.view_w, self.view_h) {
            return;
        }
        self.view_w = w;
        self.view_h = h;
        self.indexed = vec![0; w * h];
        self.rgba = vec![0; w * h * 4];
        self.live_captions.clear();
    }

    /// The tallest view the page should ask for in normal play, 13 tiles, in pixels.
    pub fn default_view_h() -> u32 {
        WEB_VIEW_H as u32
    }

    /// The narrowest view width, in pixels.
    pub fn min_view_w() -> u32 {
        MIN_VIEW_W as u32
    }

    /// The widest view width, in pixels.
    pub fn max_view_w() -> u32 {
        MAX_VIEW_W as u32
    }

    /// Draws the current world into the frame buffers.
    pub fn render(&mut self) {
        // The picture has no captions on the Web; they are text over the canvas (see `caption_count`).
        draw_slice_puffs(
            &mut self.indexed,
            self.view_w,
            self.view_h,
            &self.world,
            self.world.tick.0 as u32,
            &DrawOptions {
                captions: false,
                shake: self.shake,
            },
            &self.puffs,
        );
        self.live_captions.clear();
        if self.captions {
            self.live_captions.extend(
                self.world
                    .chickens
                    .iter()
                    .filter_map(|c| caption_view(&self.world, self.view_w, self.view_h, c)),
            );
            self.live_captions.extend(self.event_captions.views(
                &self.world,
                self.view_w,
                self.view_h,
                &DrawOptions {
                    captions: false,
                    shake: self.shake,
                },
            ));
        }
        let (pixels, _) = self.rgba.as_chunks_mut::<4>();
        for (px, &i) in pixels.iter_mut().zip(&self.indexed) {
            *px = EGA_RGBA[usize::from(i & 15)];
        }
    }

    /// The width of the frame in pixels.
    pub fn width(&self) -> u32 {
        self.view_w as u32
    }

    /// The height of the frame in pixels.
    pub fn height(&self) -> u32 {
        self.view_h as u32
    }

    /// The address of the RGBA frame in the module's memory: `rgba_len` bytes, valid until the next call that changes the app.
    /// Call [`App::render`] first.
    pub fn rgba_ptr(&self) -> *const u8 {
        self.rgba.as_ptr()
    }

    /// The length of the RGBA frame in bytes.
    pub fn rgba_len(&self) -> usize {
        self.rgba.len()
    }

    pub fn tick(&self) -> f64 {
        self.world.tick.0 as f64
    }

    pub fn hash_hex(&self) -> String {
        format!("{:016x}", fizz_ep2::hash(&self.world).finish())
    }

    /// Fizz Blaster shots left.
    pub fn ammo(&self) -> u32 {
        self.world.ammo
    }

    /// Whether Ben is carrying the red key.
    pub fn has_key(&self) -> bool {
        self.world.flags.contains(fizz_ep2::WorldFlags::KEY_RED)
    }

    pub fn deaths(&self) -> u32 {
        self.world.deaths
    }

    /// What the HUD shows now, as the integers the page draws it from (`fizz_slice_view::HudView`): the Fizz count, the score shown
    /// (counting up after a snack), the points of the snack just taken (0 for none), the deaths, and then for each of the four key
    /// kinds (red, blue, green, gold) three numbers: whether the level has the kind, how many Ben holds and whether its ring shows.
    pub fn hud_view(&self) -> Vec<i32> {
        let v = self.hud.view(&self.world);
        let mut out = vec![v.fizz as i32, v.score as i32, v.pop as i32, v.deaths as i32];
        for k in &v.keys {
            out.extend([i32::from(k.in_level), i32::from(k.held), i32::from(k.ring)]);
        }
        out
    }

    /// Where Ben's sprite lies in the view now: left, top, width and height in view pixels (the view is [`App::width`] by
    /// [`App::height`]). The page uses it to keep its HUD off Ben; nothing in the rules reads it.
    pub fn ben_box(&self) -> Vec<i32> {
        let options = DrawOptions {
            captions: false,
            shake: self.shake,
        };
        let (x, y, w, h) = ben_view_box(&self.world, self.view_w, self.view_h, &options);
        vec![x, y, w, h]
    }

    /// Whether the red door has been opened. It stays open through a death.
    pub fn door_open(&self) -> bool {
        self.world
            .flags
            .contains(fizz_ep2::WorldFlags::DOOR_RED_OPEN)
    }

    /// Whether Ben has reached the exit.
    pub fn exit_reached(&self) -> bool {
        self.world
            .flags
            .contains(fizz_ep2::WorldFlags::EXIT_REACHED)
    }
}

/// Keeps effect `id` for the page unless the queue is full.
fn queue_effect(effects: &mut Vec<(u8, Placement)>, id: u8, place: Placement) {
    if effects.len() < MAX_PENDING_EFFECTS {
        effects.push((id, place));
    }
}

/// The Paused and Options screens, for the page's overlay (`web/options.ts`). Screen codes: 0 none, 1 Paused, 2 Options. Row ids are
/// `fizz_menu::RowId` numbers (Music 0, Placement 2, Motion 4, Captions 5, Keys 6, Back 7, Sound 8, Fullscreen 9; Resume 10,
/// Options 11, Restart 13). None of this reaches the rules or the state hash.
#[wasm_bindgen]
impl App {
    /// Drops the pacer, so the next [`App::frame`] starts it afresh and no time spent away is caught up. [`App::frame`] does this
    /// itself while a screen is up; the page also calls it when it holds the game for another reason.
    pub fn hold(&mut self) {
        self.pacer = None;
    }

    /// Which screen is up: 0 none, 1 Paused, 2 Options.
    pub fn menu_screen(&self) -> u8 {
        self.menus.screen.code()
    }

    /// Opens Paused (Resume, Options, Restart) with the cursor on Resume.
    pub fn menu_open_paused(&mut self) {
        self.menus.open_paused();
        self.hold();
    }

    /// Opens Options directly; Back then resumes play.
    pub fn menu_open_options(&mut self) {
        self.menus.open_options(false);
        self.hold();
    }

    /// Closes whichever screen is up and resumes play.
    pub fn menu_close(&mut self) {
        self.menus.close();
    }

    /// Handles a key: 0 Up, 1 Down, 2 Left, 3 Right, 4 Confirm, 5 Back. Returns 0 nothing, 1 the cursor moved, 2 a value changed
    /// (see [`App::menu_last_row`]), 3 an action row was chosen, 4 the screen changed or closed. Choosing Restart starts the level
    /// again, as [`App::reset`] does.
    pub fn menu_key(&mut self, key: u8) -> u8 {
        let key = match key {
            0 => MenuKey::Up,
            1 => MenuKey::Down,
            2 => MenuKey::Left,
            3 => MenuKey::Right,
            4 => MenuKey::Confirm,
            5 => MenuKey::Back,
            _ => return menus::KEY_NOTHING,
        };
        let out = self.menus.key(key);
        if self.menus.take_restart() {
            self.reset();
        }
        out
    }

    /// The row id the last key changed or chose.
    pub fn menu_last_row(&self) -> u8 {
        self.menus.last_row()
    }

    /// Puts the cursor on row `i` (a mouse pointer coming over it).
    pub fn menu_select(&mut self, i: usize) {
        self.menus.select(i);
    }

    /// A click on row `i`: selects it and presses Confirm, which steps a choice row and chooses an action row. Same return as
    /// [`App::menu_key`].
    pub fn menu_click(&mut self, i: usize) -> u8 {
        self.menus.select(i);
        self.menu_key(4)
    }

    /// A click on a stepper arrow of row `i`: selects it and steps its value by `d` (negative is back).
    pub fn menu_step(&mut self, i: usize, d: i32) -> u8 {
        self.menus.select(i);
        self.menu_key(if d < 0 { 2 } else { 3 })
    }

    /// The number of rows on the screen that is up.
    pub fn menu_row_count(&self) -> usize {
        self.menus.rows().len()
    }

    /// The id of row `i`.
    pub fn menu_row_id(&self, i: usize) -> u8 {
        self.menus.rows().as_slice().get(i).map_or(0, |r| r.id.0)
    }

    pub fn menu_row_label(&self, i: usize) -> String {
        self.menus
            .rows()
            .as_slice()
            .get(i)
            .map_or_else(String::new, |r| r.label.into())
    }

    /// The value word of row `i`, with ` *` already on it when a switch of the address set it. Empty for an action row.
    pub fn menu_row_value(&self, i: usize) -> String {
        self.menus
            .rows()
            .as_slice()
            .get(i)
            .map_or_else(String::new, |r| r.value.as_str().into())
    }

    pub fn menu_row_selected(&self, i: usize) -> bool {
        self.menus
            .rows()
            .as_slice()
            .get(i)
            .is_some_and(|r| r.selected)
    }

    pub fn menu_row_overridden(&self, i: usize) -> bool {
        self.menus
            .rows()
            .as_slice()
            .get(i)
            .is_some_and(|r| r.overridden)
    }

    /// The note to show under the rows, empty when none applies.
    pub fn menu_note(&self) -> String {
        self.menus.note().into()
    }

    /// The index of row `row`'s current choice, in the order the row cycles: Sound, Music and Captions 0 On, 1 Off; Placement 0
    /// Enhanced, 1 Classic; Motion 0 System, 1 Reduced, 2 Full; Keys 0 Keen, 1 Modern; Fullscreen 0 Off, 1 On.
    pub fn menu_value(&self, row: u8) -> u32 {
        self.menus.value(row)
    }

    /// Loads a remembered choice into row `row`, in the order of [`App::menu_value`].
    pub fn set_value(&mut self, row: u8, index: u32) {
        self.menus.set_value(row, index);
    }

    /// Marks row `row` as set by the address for this visit; it shows `*` until the player changes it.
    pub fn set_override(&mut self, row: u8) {
        self.menus.set_override(row);
    }

    pub fn menu_overridden(&self, row: u8) -> bool {
        self.menus.overridden(row)
    }

    /// Whether the browser can go fullscreen, which decides whether the Fullscreen row exists.
    pub fn set_fullscreen_available(&mut self, yes: bool) {
        self.menus.set_fullscreen_available(yes);
    }

    /// Tells the Fullscreen row whether the page is fullscreen now.
    pub fn set_fullscreen(&mut self, on: bool) {
        self.menus.set_fullscreen(on);
    }
}

/// The slice replay transcript, byte for byte what `fizz_ep2::replay` writes and `crates/fizz-ep2/slice-replay.txt` holds.
#[wasm_bindgen]
pub fn slice_replay_transcript() -> String {
    let mut out = String::new();
    fizz_ep2::replay(&mut out).expect("writing to a String cannot fail");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const FRAME_MS: f64 = 1000.0 / 60.0;

    #[test]
    fn captions_are_text_for_the_page_and_never_pixels() {
        let mut app = App::new();
        assert!(app.captions());
        // A chicken in view, just after a double cluck.
        app.world.ben.b.x = 12.0;
        app.world.ben.b.y = 11.0;
        app.world.chickens[0].cluck_variant = 2;
        app.world.chickens[0].cluck_age = 1;
        app.render();
        let on = app.rgba.clone();
        assert_eq!(app.caption_count(), 1);
        assert_eq!(app.caption_word(0), "bwuk-BWUK");
        assert_eq!(app.caption_colour(0), "#ff5555");
        assert!(app.caption_progress(0) > 0.0 && app.caption_progress(0) < 0.1);
        assert!(
            (0.0..f64::from(app.width())).contains(&app.caption_x(0))
                && (0.0..f64::from(app.height())).contains(&app.caption_y(0))
        );
        // The picture is the same with the captions off: the words are not in it. The beak is, and stays.
        app.set_captions(false);
        assert!(!app.captions());
        app.render();
        assert_eq!(app.caption_count(), 0);
        assert_eq!(on, app.rgba);
        app.set_captions(true);
        app.render();
        assert_eq!(app.caption_count(), 1);
    }

    #[test]
    fn reduced_motion_keeps_the_view_still_when_ben_dies() {
        let mut app = App::new();
        assert!(app.shake());
        app.world.ben.b.x = 14.0;
        app.world.ben.b.y = 3.0;
        app.world.ben.kill();
        app.world.ben.dead = 0.001 + 1.0 / 60.0;
        app.render();
        let shaken = app.rgba.clone();
        app.set_shake(false);
        assert!(!app.shake());
        app.render();
        assert_ne!(shaken, app.rgba);
        // Alive, the setting changes nothing.
        app.world.ben.dead = 0.0;
        app.render();
        let still = app.rgba.clone();
        app.set_shake(true);
        app.render();
        assert_eq!(still, app.rgba);
    }

    #[test]
    fn a_screen_holds_the_game_and_resuming_does_not_catch_up() {
        let mut app = App::new();
        app.frame(0.0);
        assert_eq!(app.frame(100.0), MAX_CATCH_UP);
        let tick = app.tick();
        app.menu_open_paused();
        assert_eq!(app.menu_screen(), 1);
        assert_eq!(app.frame(60_000.0), 0);
        assert_eq!(app.tick(), tick);
        app.menu_key(5);
        assert_eq!(app.menu_screen(), 0);
        // The first frame after resuming only restarts the pacer; the minute away is not run.
        assert_eq!(app.frame(61_000.0), 0);
        assert_eq!(app.frame(61_017.0), 1);
        assert_eq!(app.tick(), tick + 1.0);
    }

    #[test]
    fn restart_from_paused_starts_the_level_again() {
        let mut app = App::new();
        app.frame(0.0);
        app.frame(50.0);
        assert!(app.tick() > 0.0);
        app.menu_open_paused();
        assert_eq!(app.menu_click(2), 4);
        assert_eq!(app.menu_screen(), 0);
        assert_eq!(app.tick(), 0.0);
    }

    #[test]
    fn opening_the_menus_leaves_the_replay_hash_alone() {
        let mut app = App::new();
        app.menu_open_options();
        app.menu_click(1);
        app.menu_close();
        assert_eq!(app.hash_hex(), App::new().hash_hex());
    }

    #[test]
    fn a_long_stall_is_capped() {
        let mut app = App::new();
        app.frame(0.0);
        assert_eq!(app.frame(60_000.0), MAX_CATCH_UP);
        assert_eq!(app.frame(60_000.0), 0);
    }

    #[test]
    fn a_millisecond_clock_gives_sixty_ticks_per_second() {
        let clock = WebClock::new();
        assert_eq!(clock.millihertz(), 1_000_000);
        let mut pacer = Pacer::new(&clock, TICK_HZ, MAX_CATCH_UP);
        let mut total = 0;
        for ms in 1..=1000 {
            clock.set_ms(f64::from(ms));
            total += pacer.due(&clock);
        }
        assert_eq!(total, 60);
    }

    #[test]
    fn the_clock_ignores_nonsense_timestamps() {
        let clock = WebClock::new();
        clock.set_ms(f64::NAN);
        assert_eq!(clock.now(), Ticks(0));
        clock.set_ms(-5.0);
        assert_eq!(clock.now(), Ticks(0));
        clock.set_ms(10.2);
        assert_eq!(clock.now(), Ticks(11));
    }

    /// Sets the page's key edges for the slice's seven buttons.
    fn set_keys(app: &mut App, prev: Buttons, now: Buttons) {
        for bit in [1u8, 2, 4, 8, 16, 32, 64] {
            let (was, is) = (prev.contains(Buttons(bit)), now.contains(Buttons(bit)));
            if was != is {
                app.key(bit, is);
            }
        }
    }

    #[test]
    fn the_slice_app_matches_the_rules_after_every_tick() {
        let mut app = App::new();
        let mut reference = fizz_ep2::level::world();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                assert_eq!(app.frame(now_ms), 1);
                fizz_ep2::step(&mut reference, InputFrame::from_transition(prev, held));
                assert_eq!(
                    app.hash_hex(),
                    format!("{:016x}", fizz_ep2::hash(&reference).finish())
                );
                prev = held;
            }
        }
        assert_eq!(app.tick(), fizz_ep2::SLICE_TICKS as f64);
        assert!(app.exit_reached());
        assert!(app.door_open());
        assert!(!app.has_key(), "the door spent the key");
        assert_eq!(app.deaths(), 1);
    }

    #[test]
    fn event_captions_reach_the_page_as_text_and_the_setting_gates_only_the_text() {
        // Play to just after the first shot (the replay fires at tick 31): its "fzzt" is a caption on Ben's blaster.
        let mut app = App::new();
        let (mut prev, mut now_ms, mut tick) = (Buttons::NONE, 0.0, 0);
        app.frame(now_ms);
        'play: for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                app.frame(now_ms);
                prev = held;
                tick += 1;
                if tick == 33 {
                    break 'play;
                }
            }
        }
        app.render();
        let on = app.rgba.clone();
        let words: Vec<String> = (0..app.caption_count())
            .map(|i| app.caption_word(i))
            .collect();
        let i = words
            .iter()
            .position(|w| w == "fzzt")
            .expect("a fzzt caption");
        assert_eq!(app.caption_colour(i), "#55ffff");
        assert_eq!(app.caption_anchor_above(i), 0.0);
        assert!(app.caption_progress(i) > 0.0 && app.caption_progress(i) < 0.1);
        // Off hides the text; the list still fills, so turning it back on shows the caption still alive.
        app.set_captions(false);
        app.render();
        assert_eq!(app.caption_count(), 0);
        assert_eq!(on, app.rgba, "the words are never in the picture");
        app.set_captions(true);
        app.render();
        assert!(app.caption_count() >= 1);
    }

    #[test]
    fn the_hud_view_hands_the_page_the_shared_model() {
        let mut app = App::new();
        // Fizz 5, score 0, no pop, deaths 0; the red and blue sockets are in the level, empty, and green and gold are not.
        assert_eq!(
            app.hud_view(),
            [5, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        // A snack and the red key: the pop shows at once, the score counts up a point every two ticks, the red socket rings.
        app.collect_events();
        app.world.snacks[0].taken = true;
        app.world.flags.set(fizz_ep2::WorldFlags::KEY_RED);
        let points = app.world.snacks[0].kind.points() as i32;
        app.world.tick = app.world.tick.next();
        app.collect_events();
        let v = app.hud_view();
        assert_eq!((v[1], v[2]), (0, points));
        assert_eq!(&v[4..7], &[1, 1, 1]);
        app.world.tick = app.world.tick.next();
        app.collect_events();
        assert_eq!(app.hud_view()[1], 1.min(points));
        // Reduced Motion (the shake off) jumps the score and shows no ring.
        let mut still = App::new();
        still.set_shake(false);
        still.collect_events();
        still.world.snacks[0].taken = true;
        still.world.flags.set(fizz_ep2::WorldFlags::KEY_RED);
        still.world.tick = still.world.tick.next();
        still.collect_events();
        let v = still.hud_view();
        assert_eq!((v[1], v[2], v[6]), (points, points, 0));
    }

    #[test]
    fn the_app_draws_the_shot_end_puffs_a_reference_presenter_draws() {
        // A reference that feeds its own Puffs from the rules' events, as the DOS presenter does, must draw the same pixels
        // as the app does while a puff is alive, and the puff must show at the ticks the DOS bench samples.
        let mut app = App::new();
        let mut reference = fizz_ep2::level::world();
        let mut puffs = Puffs::new();
        let options = DrawOptions {
            captions: false,
            shake: true,
        };
        let (w, h) = (WEB_VIEW_W, WEB_VIEW_H);
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        let mut puffed = 0;
        app.frame(now_ms);
        for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                assert_eq!(app.frame(now_ms), 1);
                fizz_ep2::step(&mut reference, InputFrame::from_transition(prev, held));
                puffs.observe(&reference);
                prev = held;
                let t = reference.tick.0 as u32;
                if puffs.live(t).next().is_none() {
                    continue;
                }
                app.render();
                let mut want = vec![0; w * h];
                draw_slice_puffs(&mut want, w, h, &reference, t, &options, &puffs);
                assert_eq!(app.indexed, want, "tick {t}");
                let mut bare = vec![0; w * h];
                draw_slice_puffs(&mut bare, w, h, &reference, t, &options, &Puffs::new());
                if [110, 118, 122, 290].contains(&t) {
                    assert_ne!(want, bare, "tick {t} shows a puff");
                    puffed += 1;
                }
            }
        }
        assert_eq!(puffed, 4);
    }

    #[test]
    fn the_slice_app_ends_on_the_golden_final_hash() {
        let mut app = App::new();
        let (mut prev, mut now_ms) = (Buttons::NONE, 0.0);
        app.frame(now_ms);
        for &(n, held) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..n {
                set_keys(&mut app, prev, held);
                now_ms += FRAME_MS;
                app.frame(now_ms);
                prev = held;
            }
        }
        let golden = include_str!("../../fizz-ep2/slice-replay.txt");
        let last = golden.lines().last().unwrap();
        assert_eq!(
            last,
            format!("FINAL TICK {} HASH {}", app.tick(), app.hash_hex())
        );
    }

    #[test]
    fn the_slice_transcript_equals_the_golden_file() {
        assert_eq!(
            slice_replay_transcript(),
            include_str!("../../fizz-ep2/slice-replay.txt")
        );
    }

    #[test]
    fn a_slow_slice_frame_runs_several_ticks_with_held_input_only() {
        let mut app = App::new();
        app.frame(0.0);
        app.key(8, true);
        assert_eq!(app.frame(50.0), 3);
        let mut reference = fizz_ep2::level::world();
        fizz_ep2::step(
            &mut reference,
            InputFrame::from_transition(Buttons::NONE, Buttons::RIGHT),
        );
        let held = InputFrame {
            held: Buttons::RIGHT,
            pressed: Buttons::NONE,
        };
        fizz_ep2::step(&mut reference, held);
        fizz_ep2::step(&mut reference, held);
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&reference).finish())
        );
    }

    #[test]
    fn a_press_released_between_frames_still_counts_once() {
        let mut app = App::new();
        app.frame(0.0);
        // Pressed and released before the next frame: the latch holds the press for one tick.
        app.key(16, true);
        app.key(16, false);
        assert_eq!(app.frame(17.0), 1);
        let mut reference = fizz_ep2::level::world();
        fizz_ep2::step(
            &mut reference,
            InputFrame {
                held: Buttons::NONE,
                pressed: Buttons::JUMP,
            },
        );
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&reference).finish())
        );
    }

    /// A fresh level stepped with no input for `n` ticks.
    fn level_world_after_idle_ticks(n: u32) -> fizz_ep2::World {
        let mut w = fizz_ep2::level::world();
        for _ in 0..n {
            fizz_ep2::step(
                &mut w,
                InputFrame::from_transition(Buttons::NONE, Buttons::NONE),
            );
        }
        w
    }

    #[test]
    fn reset_returns_to_the_first_tick_and_forgets_held_keys() {
        let mut app = App::new();
        let start = app.hash_hex();
        app.frame(0.0);
        app.key(8, true);
        app.frame(100.0);
        assert_ne!(app.hash_hex(), start);
        app.reset();
        assert_eq!(app.hash_hex(), start);
        assert_eq!(app.tick(), 0.0);
        // The held Right was forgotten: Ben stands still while time passes.
        let idle = level_world_after_idle_ticks(app.frame(200.0));
        assert_eq!(
            app.hash_hex(),
            format!("{:016x}", fizz_ep2::hash(&idle).finish())
        );
        assert_eq!(app.ammo(), fizz_ep2::world::START_AMMO);
        assert!(!app.has_key() && !app.door_open() && !app.exit_reached());
        assert_eq!(app.deaths(), 0);
    }

    /// Every cluck an idle world emits in its first `ticks` ticks, as `(tick, variant)`, stepped one tick at a time.
    fn idle_clucks(ticks: u32) -> Vec<(u64, u8)> {
        let mut w = fizz_ep2::level::world();
        let mut out = Vec::new();
        for _ in 0..ticks {
            fizz_ep2::step(
                &mut w,
                InputFrame::from_transition(Buttons::NONE, Buttons::NONE),
            );
            for e in w.events() {
                if let fizz_ep2::Event::Cluck { variant, .. } = *e {
                    out.push((w.tick.0, variant));
                }
            }
        }
        out
    }

    fn drain(app: &mut App) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let v = app.take_effect();
            if v < 0 {
                return out;
            }
            out.push(u8::try_from(v).unwrap());
        }
    }

    #[test]
    fn a_cluck_in_an_early_tick_of_a_multi_tick_frame_is_not_lost() {
        let first = idle_clucks(400)[0];
        let mut app = App::new();
        app.frame(0.0);
        // Run to the tick before the first cluck, one tick per frame, then one frame of three ticks that covers it with
        // two further ticks after it. Only the earliest of the three steps emits the event.
        let mut now_ms = 0.0;
        while app.tick() < (first.0 - 1) as f64 {
            now_ms += FRAME_MS;
            app.frame(now_ms);
        }
        assert!(drain(&mut app).is_empty());
        now_ms += 3.0 * FRAME_MS;
        assert_eq!(app.frame(now_ms), 3);
        assert_eq!(app.tick(), (first.0 + 2) as f64);
        assert!(app.world.events().is_empty(), "the last step emitted none");
        assert_eq!(drain(&mut app), vec![first.1]);
    }

    #[test]
    fn every_cluck_is_handed_out_once_in_order_whatever_the_frame_size() {
        let expected: Vec<u8> = idle_clucks(1500).iter().map(|&(_, v)| v).collect();
        assert!(expected.len() >= 3, "the test needs a few clucks");
        for per_frame in [1u32, 2, 3, 5] {
            let mut app = App::new();
            app.frame(0.0);
            let (mut now_ms, mut got) = (0.0, Vec::new());
            while app.tick() < 1500.0 {
                now_ms += f64::from(per_frame) * FRAME_MS;
                app.frame(now_ms);
                got.extend(drain(&mut app));
                assert_eq!(app.take_effect(), -1, "already drained");
            }
            // The last frame can run past 1500 ticks, so compare the clucks both runs share.
            assert_eq!(
                got[..expected.len()],
                expected[..],
                "{per_frame} ticks a frame"
            );
        }
    }

    #[test]
    fn clucks_wait_for_the_page_across_frames_and_are_dropped_on_reset() {
        let first = idle_clucks(400)[0];
        let mut app = App::new();
        app.frame(0.0);
        // Two frames with no drain between them keep the cluck until it is taken.
        let mut now_ms = 0.0;
        while app.tick() < (first.0 + 1) as f64 {
            now_ms += FRAME_MS;
            app.frame(now_ms);
        }
        now_ms += FRAME_MS;
        app.frame(now_ms);
        assert_eq!(drain(&mut app), vec![first.1]);
        // A cluck waiting when the level restarts belongs to the old world.
        app.reset();
        while app.tick() < first.0 as f64 {
            now_ms += FRAME_MS;
            app.frame(now_ms);
        }
        assert_eq!(app.take_effect(), i32::from(first.1));
        app.reset();
        assert_eq!(app.take_effect(), -1);
    }

    #[test]
    fn a_cluck_has_a_place_in_the_view_and_the_view_never_reaches_the_rules() {
        // The same idle run at three window widths: the same clucks and the same state hash, but a cluck is placed in the view it is
        // heard in, so the pan and gain follow the width.
        let run = |w: u32| {
            let mut app = App::new();
            app.set_view(w, WEB_VIEW_H as u32);
            app.frame(0.0);
            let (mut now_ms, mut got) = (0.0, Vec::new());
            while app.tick() < 1500.0 {
                now_ms += FRAME_MS;
                app.frame(now_ms);
                loop {
                    let v = app.take_effect();
                    if v < 0 {
                        break;
                    }
                    got.push((v, app.effect_pan(), app.effect_gain()));
                }
            }
            (got, app.hash_hex())
        };
        let (narrow, narrow_hash) = run(96);
        let (wide, wide_hash) = run(1280);
        assert_eq!(narrow_hash, wide_hash, "the view is not in the rules");
        assert!(narrow.len() >= 3);
        for (v, pan, gain) in narrow.iter().chain(&wide) {
            assert!(
                (0..4).contains(v) && pan.abs() <= 0.6 + 1e-3 && (0.4 - 1e-3..=1.0).contains(gain)
            );
        }
        let variants = |l: &[(i32, f32, f32)]| l.iter().map(|c| c.0).collect::<Vec<_>>();
        assert_eq!(variants(&narrow), variants(&wide));
        assert!(
            narrow != wide,
            "the same clucks are placed differently in a different view"
        );
    }

    #[test]
    fn the_queue_is_bounded_and_every_variant_is_in_range() {
        let mut app = App::new();
        for i in 0..(MAX_PENDING_EFFECTS as u8 * 2) {
            queue_effect(&mut app.effects, i % 4, Placement::CENTRE);
        }
        let kept = drain(&mut app);
        assert_eq!(kept.len(), MAX_PENDING_EFFECTS);
        assert_eq!(kept[..4], [0, 1, 2, 3], "the oldest are kept");
        for (_, v) in idle_clucks(3000) {
            assert!(v < fizz_ep2::CLUCK_VARIANTS);
        }
    }

    #[test]
    fn every_sounding_event_of_the_replay_reaches_the_page_as_its_effect_in_order() {
        // Drive the app's world through the slice script and queue each step's events as `frame` does; the page must be handed one
        // effect per event but a shot ending, with the id `presentation` names, a pan within Fizz 1's width and a gain in 0.4 to 1.
        let mut app = App::new();
        app.world = fizz_ep2::level::world();
        let mut want = Vec::new();
        let mut got = Vec::new();
        let mut prev = Buttons::NONE;
        for &(frames, now) in fizz_ep2::SLICE_SCRIPT {
            for _ in 0..frames {
                fizz_ep2::step(&mut app.world, InputFrame::from_transition(prev, now));
                prev = now;
                app.collect_events();
                for e in app.world.events() {
                    if let Some(id) = fizz_slice_view::presentation(e).effect {
                        want.push(id);
                    }
                }
                loop {
                    let v = app.take_effect();
                    if v < 0 {
                        break;
                    }
                    assert!(app.effect_pan().abs() <= 0.6 + 1e-3);
                    assert!((0.4 - 1e-3..=1.0).contains(&app.effect_gain()));
                    got.push(u8::try_from(v).unwrap());
                }
            }
        }
        assert_eq!(got, want);
        assert_eq!(
            got.len(),
            5 + 13 + 1 + 1 + 1 + 1 + 3 + 6 + 1 + 9 + 5 + 2 + jumps(&got)
        );
        // Every effect an event plays (4 to 14, and 17 the soda); the others (15 boing (FM), 16 jump (FM), 18 fzzt (FM)) are alternates no event plays.
        for id in (4..=14).chain([17]) {
            assert!(got.contains(&id), "effect {id} is never played");
        }
        assert!(
            !got.iter().any(|id| matches!(*id, 15 | 16 | 18)),
            "the alternates are not wired to events"
        );
    }

    /// How many jumps (effect 13) a list of effects has.
    fn jumps(list: &[u8]) -> usize {
        list.iter().filter(|&&id| id == 13).count()
    }

    #[test]
    fn the_rgba_frame_is_opaque_and_the_size_of_the_view() {
        let mut app = App::new();
        app.render();
        assert_eq!((app.width(), app.height()), (320, 208));
        assert_eq!(app.rgba_len(), 320 * 208 * 4);
        let (pixels, rest) = app.rgba.as_chunks::<4>();
        assert!(rest.is_empty());
        // Every pixel is one of the sixteen EGA colours, which are all opaque.
        assert!(pixels.iter().all(|p| EGA_RGBA.contains(p)));
        // The slice draws a whole picture, not a blank frame.
        assert!(pixels.iter().any(|p| p != &EGA_RGBA[0]));
    }

    #[test]
    fn set_view_resizes_the_frame_and_clamps() {
        let mut app = App::new();
        let hash = app.hash_hex();
        for (ask, got) in [
            ((99, 208), (99, 208)),
            ((1280, 208), (1280, 208)),
            ((10, 208), (96, 208)),
            ((5000, 5000), (1280, 416)),
            ((0, 0), (96, 64)),
        ] {
            app.set_view(ask.0, ask.1);
            assert_eq!((app.width(), app.height()), got);
            assert_eq!(app.rgba_len(), got.0 as usize * got.1 as usize * 4);
            app.render();
            let (pixels, rest) = app.rgba.as_chunks::<4>();
            assert!(rest.is_empty() && pixels.iter().all(|p| EGA_RGBA.contains(p)));
        }
        // The view never touches the rules.
        assert_eq!(app.hash_hex(), hash);
    }

    #[test]
    fn the_view_size_never_reaches_the_rules() {
        // The same moves at a phone, the default and an ultra-wide view, drawn every frame, hash identically tick for tick.
        let run = |w: u32| {
            let mut app = App::new();
            app.set_view(w, 208);
            let (mut prev, mut now_ms, mut hashes) = (Buttons::NONE, 0.0, Vec::new());
            app.frame(now_ms);
            for &(n, held) in fizz_ep2::SLICE_SCRIPT {
                for _ in 0..n {
                    set_keys(&mut app, prev, held);
                    now_ms += FRAME_MS;
                    app.frame(now_ms);
                    app.render();
                    hashes.push(app.hash_hex());
                    prev = held;
                }
            }
            hashes
        };
        let base = run(320);
        assert_eq!(run(96), base);
        assert_eq!(run(1280), base);
    }

    #[test]
    fn captions_stay_in_view_pixels_at_any_size() {
        for w in [96, 99, 640, 1280] {
            let mut app = App::new();
            app.set_view(w, 208);
            app.world.ben.b.x = 12.0;
            app.world.ben.b.y = 11.0;
            app.world.chickens[0].cluck_variant = 2;
            app.world.chickens[0].cluck_age = 1;
            app.render();
            if app.caption_count() == 1 {
                let x = app.caption_x(0);
                assert!((-16.0..=f64::from(w) + 16.0).contains(&x), "{w}: {x}");
            }
        }
    }

    #[test]
    fn the_palette_matches_the_sprite_document() {
        let hex = [
            "000000", "0000aa", "00aa00", "00aaaa", "aa0000", "aa00aa", "aa5500", "aaaaaa",
            "555555", "5555ff", "55ff55", "55ffff", "ff5555", "ff55ff", "ffff55", "ffffff",
        ];
        for (c, h) in EGA_RGBA.iter().zip(hex) {
            assert_eq!(format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2]), h);
        }
    }
}
