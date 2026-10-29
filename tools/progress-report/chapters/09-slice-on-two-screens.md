---
id: slice
n: 9
title: The slice on two screens
date: 2026-10-27
summary: One small vertical level with Ben, a chicken, a key and a door, played on the Web in a window that sets its own width and on DOS in 320 by 200, with the chicken's cluck, captions and place in the stereo field, the view-only effects, DOS sound and music, the HUD, the Fizz colour, the frame-rate policy, the Paused and Options screens, and the start-up check, probe and help screens.
prs: [24, 28, 31, 32, 34, 38, 39, 42, 44, 58, 59, 60, 62, 63, 65, 67, 69, 72, 73, 74, 75, 76, 79, 80, 81, 83, 84, 86, 87, 88, 89, 90, 92, 93, 94, 95]
---
The first slice is a cut-down lighthouse: a tall room split into two shafts, Ben, one chicken, a red key, a red door and an exit. It plays on the Web and on DOS from the same rules and the same drawing routine, `fizz_slice_view::draw_slice`. This chapter follows the slice from the level to the two screens and the DOS start-up screens. Chapter 8 covers the rules underneath it.

## The level (#29)

`crates/fizz-ep2/src/level.rs` writes the level as text, 32 tiles wide and 48 tall, and bakes it into the byte array a `TileMap` holds at compile time, so nothing is parsed at run time. Ben starts on the floor of the left shaft, a few steps from a run of spikes. One-way ledges three tiles apart climb the shaft, alternating left and right so each full jump crosses it. The third ledge is long: the chicken walks it and the red key lies at its far left end. Above it the ledges reach the red door, three tiles of the wall between the shafts. Through the door a second shaft climbs to a gallery in the open air, where the exit stands.

<!-- figure maze -->

The replay of chapter 5's golden file plays this route once with a death on the spikes, carrying the key, in the first life and a stomp on the chicken in the second. It ends at the exit with the door open and the key spent, which `tools/check-dos.sh` asserts on DOS as `EXIT yes`.

## Ben's moves (#24, #27, #47)

The buttons are the arrows, jump, pogo and fire. `fizz-sim`'s `Buttons` grew the jump, pogo and fire bits and `fizz-present-dos` and `web/main.js` gained Fizz 1's two key layouts: Keen (arrows, Ctrl, Alt, Space) and Modern (arrows or `WASD`, `Z`, `X`, `C`). Ben walks and runs with Fizz 1's acceleration and friction, jumps with a cut when Jump is released early, rides the pogo stick and bounces higher while Jump is held, and fires the Fizz Blaster: five shots that fly about a second and stop on walls.

Looking and aiming are Fizz 1's (#47). Up or Down held alone while Ben stands still starts a timer; after 0.2 seconds he looks, and the view slides up to 3.6 tiles that way over about a third of a second, then back when the key is released. Up held with Fire shoots straight up; Down held in the air shoots straight down; Down on the ground shoots ahead. The timers, the aim and a new `look_offset` are in the state hash, and the `look` and `aim` traces prove the rules against Fizz 1. At the foot of the first shaft the map's edge stops the view from sliding, so only Ben's pose changes there; the shots below were taken on the first ledge.

## The chicken, its cluck and its captions

The chicken is Fizz 1's Gloop with placeholder art: it patrols its ledge, turns at walls and edges, kills Ben on contact from the side or below, and is stunned for three seconds by a stomp or six by a shot. Each chicken draws a cluck timer from 90 to 360 ticks and a variant from four, from Fizz 1's `hashf` over the tick, the chicken's index and its draw count. The rules emit `Event::Cluck { variant, chicken }`, the transcript prints it as an `EVENT` line, and a stunned chicken is silent. The golden transcript has three clucks, at ticks 197 (variant 2), 853 (variant 1) and 1094 (variant 0), and three shot ends, at ticks 96, 106 and 281.

<!-- figure cluck-path -->

The cluck is visible as well as audible. For 12 ticks, and twice for the double clucks, a hand-drawn beak overlay opens the chicken's beak (#48), and a caption floats above it for 57 ticks (Fizz 1's 950 milliseconds), rising 12 pixels and fading: `buh-GAWK!`, `bwuk`, `bwuk-BWUK` or `bwuk, bwuk`. `fizz_slice_view::caption_view` describes a live caption once for both presenters: its word, colours, age and the chicken's true screen position. DOS draws the word as written, in lower and upper case, in Fizz 1's proportional pixel font (#80; the 8 by 8 capitals font stays for `/BENCH` text). The Web page puts it in a layer of text over the canvas in Fizz 1's bold italic type, sized from the tile's size on screen. Neither presenter moves a caption to keep it in view: the frame's edge cuts it off as its chicken walks out, and it is dropped once the chicken is more than a tile outside. `/NOCAPTIONS` on DOS and the Captions button (or `V`) on the Web turn the words off; the beak still opens.

<!-- figure shots web-cluck.png dos-cluck.png -->

## Stars, puff, shake and spin

Four effects come from Fizz 1 and change only what is drawn. The rules, the hash and the events are untouched, and each is a pinned frame in `fizz-slice-view`'s tests.

- **Stun stars (#59).** Fizz 1's `stars0` and `stars1` sprites, 16 by 8, are exported into `fizz1-slice.json`. They sit 0.3 tiles above a stunned chicken's head and swap every 10 ticks (Fizz 1's 6 Hz), with the alert frame and the flash in the last second of the stun.
- **Shot puff (#65).** The rules emit `Event::ShotEnd { x, y }` for each shot that hits a wall, runs out of life or stuns the chicken, and the transcript prints `EVENT <tick> shotend <x> <y>`. A fixed list of puffs, `fizz_slice_view::Puffs`, fed from those events by both presenters, plays Fizz 1's four 16 by 16 puff frames for 18 ticks, in light magenta, the Fizz colour (#87), where Fizz 1 uses cyan.
- **Death shake (#63).** When Ben dies the camera's x moves by a baked table of at most one pixel for 18 ticks, derived from his `dead` timer. The Web page's Motion button (Full or Reduced, defaulting to the system's reduced-motion setting) turns it off, and on DOS the Motion row of the Options screen does (#94, #95).
- **Death spin (#72).** Ben turns about his centre as he dies, at Fizz 1's `rot += 8 * dt`. The view rotates the `ben_jump` sprite by nearest-neighbour inverse mapping in integers, from a baked cosine and sine table indexed by the ticks since the death, read from `Ben::dead`. It follows the Motion setting with the shake: Reduced keeps the flat flickering pose, on both screens. Ten death frames are pinned in both settings, and tick 400's pinned digest changed on purpose.

<!-- figure shots web-stars.png dos-stars.png dos-puff.png web-puff.png -->

A still cannot show the shake, which is a shift of at most one pixel, and no spin picture is kept either: the spin is pinned as frame digests, not looked at.

## The Web screen (#32, #54, #58, #60)

The Web page fills the window. The canvas is 13 tiles (208 pixels) tall and as wide as the window's shape gives, from 96 to 1,280 pixels, rebuilt through `App::set_view` after a debounced resize or turn of the device, and scaled by the window's height over 208 with no letterbox. A phone held upright shows about six tiles across, and an ultra-wide window shows more of the level beside the shaft. The rules never read the view's size, so the hash is the same at every window size. A small HUD in Fizz 1's look sits in the top-left corner of the picture (#86). The controls, the notes and the seven lines of text (`TICK`, `HASH`, `AMMO`, `KEY`, `DOOR`, `DEATHS`, `EXIT`) sit in a panel opened with `H` or the corner `?` button.

<!-- figure shots web-start.png web-portrait.png web-wide.png -->

The Web screenshots are taken with a fake clock that the capture script advances by exactly one tick per frame, and the script presses the golden replay's keys one tick at a time through the DevTools protocol. The page's `HASH` at tick 300 in the stars shot, `177f6e86fd438b25`, is the golden transcript's `TICK 300` line.

<!-- figure shots web-look-up.png web-aim-up.png -->

## The DOS screen (#31, #34, #39, #42)

`FIZZ2.EXE` draws the slice in mode 13h, 320 by 200, through `fizz-present-dos`, which adds the HUD pill in the top-left corner (#83; the 8 by 8 strip of `AMMO`, `KEY`, `DEATHS` and `TICK` is gone) and the `LEVEL CLEAR` banner. In `/LIVE` it waits for the vertical retrace once per drawn frame, with a bounded poll so a status port that never toggles cannot hang it. The step, the drawing and the copy to video memory are timed by `/BENCH` against the 16,667 microsecond tick: at 26,800 cycles a millisecond in DOSBox the tick costs {{ms.benchTickUs}} microseconds, of which the step is {{ms.benchStepUs}}, the drawing {{ms.benchDrawUs}} and the copy {{ms.benchPresentUs}} (`snapshots/measured.json`). `FIZZ2.EXE` is {{ms.exeBytes}} bytes unstripped.

<!-- figure shots dos-puff.png dos-exit.png -->

The DOS keys are typed into DOSBox through the X server, as a person would press them, for the live shots.

<!-- figure shots dos-look-up.png dos-aim-up.png dos-aim-down.png -->

## DOS sound (#62, #74)

The cluck plays on four DOS paths. A new audio service in the runtime moves the timer to 560 Hz (PIT divisor 2131), keeps the BIOS clock at 18.2 Hz with the same accumulator, and paces the simulation to 60 steps a second with the integer `Pacer`. From the interrupt it steps the cluck as OPL2 channel 5 on an AdLib or a Sound Blaster, as the OPL3 pair on a Sound Blaster Pro 2 or later (below), or as PC speaker steps every fourth tick. With no device or with sound off it does nothing. `FIZZ2.EXE` hands each `Event::Cluck` to the service, and on exit and on panic it silences the chip and the speaker and puts register `0x105` back to 0. `/REPLAY` and `/LIVE` print `SOUND MUSIC <x> FX <y>` and, at the end, `AUDIO WRITES <n> SPEAKER STEPS <m>`, which is how a run with no ears tells which path played. The sound settings (`FIZZ2.CFG` version 2 and the `/MUSIC:`, `/FX:`, `/ADLIB`, `/SB`, `/SPEAKER` and `/NOSOUND` switches, #55) choose the path from the probe and the player's choices, with a fallback word when a device is missing. `tools/check-dos.sh` runs `/REPLAY` under a ladder of sound configurations (an OPL3 Sound Blaster, a Sound Blaster Pro 2, an OPL2 Sound Blaster, an AdLib-only machine, the speaker, no card and `/NOSOUND`) and requires the golden `TICK`, `FINAL` and `EVENT` lines, the BIOS tick bound and `VECTORS RESTORED yes` in each, so the sound cannot change the game.

The cluck has a place (#74). `fizz_sfx::place`, a pure function of whole numbers, applies Fizz 1's formula once (`placeSound` in `sound-field.ts`): the chicken's offset from the middle of the view, as a fraction of half the view, gives a pan up to 0.6 either way and a distance factor that falls to 0.4 outside the view, and an equal-power law turns them into a level for each ear. The program works it out from the chicken and the camera as drawn, and the rules never see it. On an OPL3 with effects on the Sound Blaster (`FX ch14`), the effect is the pair: channel 5 (bank 0, output bit `0x10`) is the left ear and channel 14 (bank 1, output bit `0x20`) the right, each at its own carrier level in 0.75 dB steps. On an OPL2 or an AdLib the distance becomes the level of channel 5 and the pan is dropped, and the PC speaker ignores the placement. The Web gives each Undertone voice an equal-power `StereoPannerNode` and a gain, and the H panel's Sound placement button chooses Enhanced, the default, or Classic, which plays every cluck centred at full level as Fizz 1's Classic mode does; DOS is always Enhanced for now. `docs/SOUND.md` ("Placing sounds") has the formula, the tables and the per-platform path; it is the place to read how each path loses something.

The cluck itself changed in #69: the first variant's `buh` is 85 milliseconds, up from 50, and its `GAWK` starts 45 milliseconds later, so the variant runs about 345 milliseconds instead of 300. The other three variants, the events and the hashes are unchanged.

## The level music (#73, #75)

`assets/music/` holds the source of the slice's level music, "Crater Fields", which is Fizz 1's track used as a placeholder test song and not an official Fizz 2 track. The builder is Fizz 1's, ported and held to a saved fixture of its output, and `bun tools/music/notes.ts --check` guards the committed note list: 39 bars, 63.24 seconds. The Web page plays it (#75): `web/music.ts` loops the song with Undertone from the first key press or click, on a music bus at 0.65 that shares one `AudioContext` and a limiter with the cluck's bus, and the Music button in the H panel and `M` stop it. The rules never start, stop or read it, so no event, hash or transcript line changed. DOS plays it too (#76, #79, #81): the lowering writes OPL2 and OPL3 register streams, `fizz_music::MusicPlayer` steps them from the same 560 Hz handler before the effect, and `/LIVE` and `/REPLAY` print a `MUSIC` line that the check compares with the host's. `docs/MUSIC.md` has the layout, both players and the plan; the owner heard the Web music roll on 28 October 2026, and nobody has reported hearing the DOS music.

## The HUD, the frame rate, the colour and the menus (#83 to #95)

The HUD (#83, #86) follows Fizz 1's: `Fizz` is the shots left, `Deaths` the count and a chip shows the red key. On DOS it is a 72 by 27 pixel pill at (4, 4) in Fizz 1's pixel font, drawn translucent over the picture: the palette gains the sixteen EGA colours dimmed to 4/13 in entries 16 to 31 (Fizz 1's `ui::dim`), and the cached strip is rendered again only when a value changes. `TICK` left the live screen and stays in `/BENCH`, the transcripts and the Web panel. At 26,800 cycles a millisecond the `/BENCH` drawing average fell from 2,802 microseconds for the old 8 by 8 strip to 2,229 for the cached box and rose to 2,340 for the translucent pill, and the eleven `BENCH FRAME` digests are pinned in both the host tests and `tools/check-dos.sh`. The Web HUD is DOM in `#stage`, a dark panel scaled by the stage's height that moves to the other corner when Ben comes near; `web/hud.ts` holds its rules and `tools/check-web-hud.py` plays the golden replay against the built page in headless Chromium.

The frame-rate policy (#84) leaves the rules at 60 steps a second and changes how often `/LIVE` and `/REPLAY` draw. `/FPS30` draws every second tick, `/FPS60` every tick, and `/FPSAUTO` (the default) averages 32 drawn frames on the 560 Hz clock in `fizz_machine::FrameGovernor`: it chooses 30 Hz when a frame takes 101 to 170% of a sixtieth of a second and 60 Hz above 185% or below 67%, which are the thresholds of Fizz 1's DOS port. Both modes print `FRAMES <n> TICKS <t> RATE <hz> MODE <m>`. `tools/check-dos.sh` runs the replay at `/FPS30` (about half as many drawn frames as ticks), Auto at 60 at 26,800 cycles and Auto at 30 at 4,500 cycles, and each must print the golden transcript.

The Fizz colour (#87) is one accent, light magenta (EGA 13, `#ff55ff`, grape), defined in `fizz_view::theme` and as `--fizz` in `web/style.css`, with a test that keeps the two equal. The HUD number, Ben's shots and puffs, the soda can of the menus and the menu plate follow it; Fizz 1 draws them cyan. It is a view change only: the state hash is unchanged and the frame digests were pinned again, with the HUD contrast floor for the number set at 3.0.

The settings changed shape underneath the screens. `FIZZ2.CFG` version 3 (#88) is 13 bytes and adds motion, sound placement, captions and key layout to the sound and frame-rate bytes; zero is always the default, versions 1 and 2 still read, a bad value resets only its own field, and every switch (`/KEEN` joins `/MODERN`, `/CAPTIONS` and `/NOCAPTIONS`) overrides one field for one run and is never saved. The save (#89) writes `FIZZ2.NEW`, reads it back, deletes the old file and renames the new one, with a three-byte INT 24h handler answering Fail during it so no "Abort, Retry, Fail?" lands on mode 13h. `audio::replan` (#90) switches the music and effects devices while the game runs and is checked by register counts under an SB16, a Pro 2, an AdLib alone and no card.

The screens come in four steps. `fizz-menu` (#92) is the shared model: a list with a cursor, the Options rows and their value words for DOS and the Web, and the Paused menu, as `no_std` code with host tests. The DOS drawing (#93) dims the frozen picture once into the palette's dimmed half, draws a 2x white heading over a Fizz-colour rule and a clipped-corner plate with black ink for the chosen row, and Fizz 1's soda can bobbing one pixel every 20 ticks; `/MENUSHOT` proves the DOS digests equal the host's. The Web screens (#94) open with Esc or Pause over the picture, hold the game and its pacer with no catch-up on resuming, and save one `localStorage` blob. The DOS flow (#95) opens Paused with Esc in `/LIVE`, holds the world and the frame governor while a screen is up with the music playing, applies each change live, and saves `FIZZ2.CFG` when Options is left with a change; `tools/check-dos.sh` types `esc up enter` where it used to type `esc` and adds cases for the saved bytes, the live frame rate, a switch that is not saved, a read-only file, the hidden `/OPTIONS` mode and a menu left open until the wall-clock cap. `FIZZ2.EXE` is now 177,664 bytes.

<!-- figure shots dos-paused.png dos-options.png -->

<!-- figure shots web-paused.png web-options.png -->

The owner's playtest on 28 October 2026 reported that the Web HUD looks good, the cluck (buh-gawk) plays and the music rolls, the spin death, the stars and the puffs look good, and the translucent DOS pill matches Fizz 1's look. The same screenshots are retaken from the current builds, and the DOS Paused and Options pictures are grabs from DOSBox running `/LIVE` with the keys typed through the X server.

## /CHECK, /PROBE and /?

Three more screens belong to the DOS machine work (Milestone 1.5). `/PROBE` (#50) prints what the runtime finds: the display adapter, mouse, joystick, FM chip, Sound Blaster and its `BLASTER` line, x87, processor family and free memory. `/CHECK` (#52) shows the same facts on a text-mode start-up screen with the sound the game would use lit; Enter plays and Esc quits. `/CHECKCELLS` draws that screen, reads its 80 by 25 cells back from video memory and prints them, which is how the screen is tested without a person pressing a key, and `tools/render-cells.py` draws those cells with a VGA font. `/?` (#67) prints the modes, keys, captions and sound switches from the one table in `fizz-machine` that the argument parser also reads, and exits before any hook or mode change.

<!-- figure shots check.png -->

<!-- figure dos-help -->

<!-- figure dos-probe -->

<!-- figure dos-transcript -->

## The shelves

### Catalogued
- #29, #31, #32, #34, #38, #39, #42 and #44 are merged into `main`; the last of them, #44, landed on `{{pr44.merge}}` with CI {{pr44.ci}}.
- The golden replay (`FINAL TICK {{golden.finalTick}} HASH {{golden.final}}`, three `cluck`, three `shotend` and 31 other `EVENT` lines) is printed by native, Web and DOS: `tools/check-parity.sh` printed `PARITY ok {{ms.parityHash}}`, and the DOSBox run in the transcript above matches the file line for line, `EVENT` lines included (`snapshots/measured.json`; the build checks the transcript).
- Both screens call `fizz_slice_view::draw_slice`; frame digests at nine ticks of the replay, and for the puff, the stars and the death frame, are pinned in its tests, and `/BENCH` compares eleven `BENCH FRAME` digests from DOS with the host's.
- The chicken's `EVENT` lines are the evidence that a cluck happens at the same tick and with the same variant on all three platforms.
- DOS audio paths write what they should: the channel 5 effect, the OPL3 on the AdLib role and the pair print `AUDIO WRITES` of 569, 590 and 1135 for the golden replay's four clucks, and `tools/check-dos.sh` requires exactly those relations; `tools/check-dos.sh` requires register writes on a chip, steps on the speaker and zeros when silent, and the same transcript under every configuration. The last measured run printed `BIOS TICKS {{ms.biosTicks}}` for the {{golden.finalTick}}-tick replay under the 560 Hz timer.
- `/PROBE` runs under 21 DOSBox configurations, `/CHECKCELLS` under six machines with the cells equal to the host builder's, `/?` prints the text of `crates/fizz-machine/tests/help.txt`, and a host test refuses any script that runs the interactive `/CHECK`.
- The Web cluck path is checked with a stand-in `AudioContext`: `web/sfx.test.ts` under `bun test` and `tools/check-web-sfx.py` in headless Chromium show what is scheduled, that nothing plays before a key press or while muted, that a pan other than the middle adds a panner to each voice, and that the cluck at tick 197 schedules variant 2 with its panners.
- The Web music is checked with the same kind of stand-in: `bun test web/music.test.ts` matches the sources scheduled over one loop with the note list of `assets/music/notes/crater.txt` for start time, pitch and pan, and `tools/check-web-sfx.py` shows nothing sounding before a key press and the Music button and `M` working in headless Chromium. `bun tools/music/notes.ts --check` guards the note list itself.
- The HUD, the frame-rate policy, `FIZZ2.CFG` version 3, the crash-safe save, device switching and the screens (#83 to #95) are merged with the checks named in the section above: the `BENCH FRAME` digests and the `FRAMES` lines in `tools/check-dos.sh`, the byte-for-byte saves and the typed-key Options cases there, `/REPLANTEST` under four machines, `/MENUSHOT`'s digests, `tools/check-web-hud.py`, `tools/check-web-options.py`, and `cargo test -p fizz-menu`; the golden hash is unchanged by all of them (`PARITY ok {{ms.parityHash}}`).
- The screenshots on this page are records in `media/captures.json`: headless Chromium for the Web, and DOSBox grabbed under a virtual display for DOS, each with its command, commit and date.
- The owner's playtests of 28 October 2026: the death spin, the stun stars and the shot puff look good, the cluck effects sound good, the panning is hard to tell on DOS because the slice's level is small, the Web music is working and rolls, the Web HUD looks good, the cluck plays as buh-gawk, and the translucent DOS HUD pill matches Fizz 1's look. These are one person's observations, not measurements, and the notes do not say which screen showed which picture in every case.

### Uncatalogued
- Hearing the cluck on real hardware. The owner has heard it in the preview build (see the playtest line above), but nobody has listened on a real OPL2, OPL3, Sound Blaster or PC speaker: the DOS evidence is register writes and speaker steps counted by the program, and the Web evidence is the voices a stand-in recorded. The register write timings are Fizz 1's, not measured here, and the cost of the 560 Hz interrupt on a 486 is not in `/BENCH`, which still runs at 60 Hz.
- The stun stars, the puff, the death shake and the death spin on a real display. The owner reports that the spin, the stars and the puff look good, without saying on which screen; the stars and the puff were also grabbed from DOSBox's window under a virtual display (the DOS shots above); the shake, a shift of at most a pixel, was not mentioned and has not been pictured.
- The Web page played by a person: the screenshots come from headless Chromium with scripted keys, and the owner's playtests do not record fullscreen, Keyboard Lock or the panel being driven by hand. Firefox and Safari have not loaded the page.
- A real 486, a real VGA display and a real keyboard controller: the DOS screenshots are DOSBox's scaled window, and the `/BENCH` figures are DOSBox at a fixed cycle count, which hides what the `f64` rules cost on a 486DX.
- The DOS Paused and Options screens on a real display and keyboard, how long `FIZZ2.CFG` takes to save to a floppy (a write-protected disk has not been tried either), the key repeat in a menu, the frame rate on a real 486 (how 30 Hz looks has not been judged) and a live switch of Music or Effects heard by anyone. The Web screens in a real browser, with a pointer, touch, fullscreen Esc or a screen reader.
- The `/CHECK` screen shown on a real screen, and `/?` read on a real console; whether FreeDOS's `COMMAND.COM` passes `/?` through to the program.
- The levels of the Web music's bus and the cluck's bus, which are a first guess: the owner says the music is working, not that the mix is right.
- Sound on the Web: the first-key start under each browser's autoplay policy, and the silence on a hidden tab, in a real browser.

### Wish list
- The other Fizz 1 sound effects for the slice with their captions (Milestone 4), and effects authored in stereo, which the pair already allows with one record stream for each side.
- The HUD extras of Fizz 1's survey that remain (a toast line and a level-name banner), and the start-up check screen that runs before play.
- Fizz 1's art and enemy roster in place of the placeholder chicken (Milestone 2), and the camera's pixel-locking (Milestone 3).
- A first listen: the owner on a real AdLib, Sound Blaster and speaker, then a move of the cluck to the catalogued shelf.

> **From the workbench — OPL2 and OPL3.** The AdLib card of 1987 carried Yamaha's YM3812, known as the OPL2, with nine two-operator FM channels. The Sound Blaster Pro 2 and later cards carry the OPL3 (YMF262), which adds a second register bank, 18 two-operator channels and stereo outputs once register `0x105` switches it on, which is why `fizz-sfx` addresses the right ear of its panned effect as channel 14 of the second bank.
> Source: [Yamaha YMF262, Wikipedia](https://en.wikipedia.org/wiki/Yamaha_YMF262)

```figures
id: maze
kind: diagram
track: shared
title: Map of the slice level
alt: Colour map of the 32 by 48 tile level with five numbered places: the start on the floor of the left shaft beside red spikes, the red key and the chicken on the third green ledge, the brown red door in the wall between the shafts, and the yellow exit on the gallery at the top of the right shaft, joined by a dashed route that climbs the left ledges, passes through the door and climbs the right ledges.
caption: The level as written in level.rs, with the key and door route.
at: figure maze
thumb: svg

id: cluck-path
kind: diagram
track: shared
title: The cluck from event to speaker
alt: Flow diagram with the shared rules at the top (World::step emitting Event::Cluck, and the EVENT line that all three platforms print), a DOS row (audio::effect with the placement, the 560 Hz audio service, OplPair, OplPlayer or SpeakerPlayer, the OPL2 channel 5, the OPL3 pair of channels 5 and 14 or the speaker ports, a real chip nobody has listened to) and a Web row (take_effect, the main.js loop, Sfx.playEffect with a pan and a gain, the AudioContext voices, a real browser nobody has listened to), with the evidence under each box.
caption: Each box names its file and the check behind it; the last box of each row is uncatalogued.
at: figure cluck-path
thumb: svg

id: cluck-shots
kind: screenshot
track: shared
title: The cluck on Web and DOS
alt: Two screenshots of the slice just after the chicken clucks, the first from the Web build and the second from DOSBox: the chicken has its beak open and a caption floats above it, as text on the Web and as pixels in Fizz 1's font on DOS.
caption: The same cluck, drawn as text on the Web and as pixels on DOS.
at: figure shots web-cluck.png dos-cluck.png
thumb: img:media/dos-cluck.png

id: effect-shots
kind: screenshot
track: shared
title: Stun stars and the shot puff
alt: Four screenshots, two from the Web build and two from DOSBox: a stunned chicken with two small yellow and white stars above its head, and a small light magenta puff where a shot ended against the wall of the shaft.
caption: The stars at tick 300 on the Web and a little later on DOS, and the puff at tick 108 on the Web and a few seconds into the DOS run.
at: figure shots web-stars.png dos-stars.png dos-puff.png web-puff.png
thumb: img:media/dos-stars.png

id: web-sizes
kind: screenshot
track: web
title: The Web view at three window shapes
alt: Three screenshots of the Web slice at 960 by 540, 390 by 780 and 1600 by 520 pixels: each shows the same 13 tiles of height, with about six tiles of width on the phone-shaped window and the right wall and sky beyond it on the wide one.
caption: The Web view follows the window's shape.
at: figure shots web-start.png web-portrait.png web-wide.png
thumb: img:media/web-start.png

id: web-look
kind: screenshot
track: web
title: Looking up and aiming up on the Web
alt: Two screenshots of the Web slice: Ben on the first ledge with the view risen to show the chicken and a fading caption above it, and Ben with his blaster raised and a shot rising above him.
caption: Up held alone looks up; Up with Fire shoots up.
at: figure shots web-look-up.png web-aim-up.png
thumb: img:media/web-look-up.png

id: dos-slice-shots
kind: screenshot
track: dos
title: The slice in DOSBox, finished and in flight
alt: Two DOSBox screenshots of FIZZ2.EXE: Ben at the foot of the left shaft with a light magenta puff at the wall and the HUD pill in the corner, and Ben on the gallery beside the yellow exit under a LEVEL CLEAR banner on a light blue sky, with Fizz 5 and Deaths 1 in the HUD pill.
caption: The replay's first puff, a few seconds in, and its end.
at: figure shots dos-puff.png dos-exit.png
thumb: img:media/dos-exit.png

id: dos-live-shots
kind: screenshot
track: dos
title: Look and aim in DOSBox
alt: Three DOSBox screenshots of FIZZ2.EXE in live play: Ben looking up with the chicken and the words BWUK, BWUK in view, Ben firing straight up, and Ben in the air firing straight down.
caption: Keys typed through the X server into DOSBox.
at: figure shots dos-look-up.png dos-aim-up.png dos-aim-down.png
thumb: img:media/dos-look-up.png

id: dos-menu-shots
kind: screenshot
track: dos
title: The Paused and Options screens in DOSBox
alt: Two DOSBox screenshots of FIZZ2.EXE in live play: the frozen picture dimmed behind the heading Paused with the rows Resume, Options and Quit to DOS and a light magenta plate on Resume, and the Options screen with Music, Effects, Sound placement, Frame rate, Motion, Captions, Keys and Back, the plate on Effects and the values in light magenta on the right.
caption: Esc opens Paused; Options is its second row. Both are grabs of the running game.
at: figure shots dos-paused.png dos-options.png
thumb: img:media/dos-options.png

id: web-menu-shots
kind: screenshot
track: web
title: The Paused and Options screens on the Web
alt: Two screenshots of the Web slice in headless Chromium, 960 by 540: the picture dimmed behind a dark panel titled Paused with Resume chosen on a light magenta plate, and the Options panel with Sound, Music, Placement, Motion, Captions, Keys, Fullscreen and Back and their values.
caption: The same two screens on the Web, 60 ticks after load.
at: figure shots web-paused.png web-options.png
thumb: img:media/web-options.png

id: check-shot
kind: screenshot
track: dos
title: The /CHECK screen
alt: The system check screen drawn in text mode: an orange title bar reading Lieutenant Fizz 2 - System Check, four framed boxes for video, input, sound and machine with tick and cross marks, the PC speaker row lit, and the line Music off, Effects PC speaker with Press Enter to play, Esc to quit.
caption: The check screen for DOSBox's default machine, drawn from FIZZ2.EXE /CHECKCELLS.
at: figure shots check.png
thumb: img:media/check.png

id: help-text
kind: transcript
track: dos
title: FIZZ2.EXE /? help screen
alt: Terminal output of FIZZ2.EXE /? : the title, the usage line, the modes, the keys, the captions and the sound switches in groups, and the exit status line.
caption: The help screen's text, equal to the golden file the host tests use.
at: figure dos-help
thumb: term

id: probe-text
kind: transcript
track: dos
title: FIZZ2.EXE /PROBE output
alt: Terminal output of FIZZ2.EXE /PROBE: the PROBE lines for video, mouse, joystick, FM chip, Sound Blaster, x87, processor and free memory, then the CELLS hash, the SOUND line and the platform lines.
caption: What FIZZ2.EXE finds in DOSBox's default machine.
at: figure dos-probe
thumb: term

id: slice-transcript
kind: transcript
track: dos
title: FIZZ2.EXE /REPLAY output
alt: Terminal output of FIZZ2.EXE /REPLAY: the SLICE REPLAY 1 line, TICK lines every 60 ticks, four cluck, eight shotend and 58 other EVENT lines, the FINAL line and the EXIT, FPU, BIOS, SOUND, AUDIO and VECTORS lines.
caption: The transcript of the grabbed DOSBox run, line for line.
at: figure dos-transcript
thumb: term
```
