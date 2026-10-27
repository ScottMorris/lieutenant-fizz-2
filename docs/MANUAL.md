# Lieutenant Fizz 2 — Player's Manual

This manual is updated with every change a player can see or do, in the same pull request as the change. It describes the builds as they are today, not as they are planned. The rule is in the "Game Manual" section of [`AGENTS.md`](../AGENTS.md).

## What this is

Lieutenant Fizz 2 is a technical preview. On DOS you can now play the first slice of the game: one small vertical maze with a red key, a red door, a chicken and an exit, using the pogo stick, the jump and the Fizz Blaster from Lieutenant Fizz 1. There is no story, no menu and no sound yet. The Web version has not caught up: it still shows the earlier test program, a yellow square on a dark blue screen that you can move around and make dash. Both exist to prove that the two versions behave identically.

## Running it

### Web

Build the page with `tools/build-web.sh`, then serve the `dist/web` folder with any static file server and open it in a browser. The build needs the Rust toolchain from the repository and a matching `wasm-bindgen` command-line tool; [`PLATFORM.md`](PLATFORM.md) has the exact version.

### DOS

Build the program with `tools/build-dos.sh`, which writes `FIZZ2.EXE` and `CWSDPMI.EXE` into `dist/dos`. Keep the two files together and run `FIZZ2.EXE` in DOSBox or js-dos. It needs a VGA display, and it targets a 486-class PC. The build needs the DOS toolchain described in [`PLATFORM.md`](PLATFORM.md); you do not need a C compiler.

## Controls

There are two sets of keys, and you use one at a time. Keen is the default on the Web and on DOS.

| Action | Keen keys (default) | Modern keys |
| --- | --- | --- |
| Move | Arrow keys | Arrow keys or `W` `A` `S` `D` |
| Jump | Ctrl | `Z` |
| Pogo stick | Alt | `X` |
| Fizz (fire) | Space | `C` or Space |
| Quit | Esc (DOS only) | Esc (DOS only) |
| Fullscreen | `F` (Web only) | `F` (Web only) |

On DOS every key does something in the slice, as [The slice on DOS](#the-slice-on-dos) describes. On the Web, only the arrows and Jump do anything in this preview: the arrows move the square and Jump makes it dash in the direction you last moved. Pogo and Fizz have no effect there yet. Keys from the other set do nothing, so `W` does not move you under the Keen keys.

On the Web there is no quit key; close the tab.

### Choosing your keys

- **Web:** the Keys button under the screen switches between Keen and Modern, and the line above it lists the keys in use. The page remembers your choice on this browser, if the browser allows it to.
- **DOS:** start with `FIZZ2.EXE /MODERN` for the Modern keys. Without it you get the Keen keys.

### Fullscreen and browser shortcuts

On the Web, press `F` or use the Fullscreen button to fill the screen, and press it again to leave. A web page cannot stop a browser tab from acting on a few shortcuts: Ctrl+W closes the tab whatever the game does, and some desktops keep their own window and workspace shortcuts. In fullscreen, Chromium-based browsers (Chrome, Edge and similar) let the game keep Ctrl, Alt and the other game keys to itself; other browsers do not. If a shortcut keeps getting in your way, switch to the Modern keys, which do not use Ctrl or Alt.

### DOS modes

Add one of these after the program name, such as `FIZZ2.EXE /SLICE`. With no mode, you get `/LIVE`. Add `/MODERN` as well, or by itself, to use the Modern keys in `/LIVE`, as in `FIZZ2.EXE /MODERN`.

- `/LIVE` — you play the slice. It ends when you press Esc, or by itself after 30 seconds, and then prints the final tick, the hash, and whether Ben moved from where he started.
- `/SLICE` — plays a built-in run through the whole slice by itself, with no keyboard input: about 21 seconds in which Ben shoots and stomps the chicken, dies once on the spikes, takes the key, opens the door and walks into the exit. It then prints the list of hashes it reached and a few lines about the machine, including `EXIT yes` when Ben got out. Use it to check that your copy agrees with the Web version, and to watch the level without playing it.
- `/REPLAY` — plays the earlier 10-second test program script by itself, with no keyboard input, then prints its list of hashes. It is kept for the test program and will go when the test program does.
- `/PANIC` — makes the program stop on purpose, to show that it puts the keyboard, timer and screen mode back as it found them. It is a test, and it exits with status 3.

## The slice on DOS

![The DOS build in DOSBox, a few seconds into the built-in run: Ben on a ledge of the shaft facing the chicken, with the HUD along the top.](img/dos-slice.png)

*Screenshot of the DOS build with the slice, `FIZZ2.EXE /SLICE` in DOSBox Staging, at tick 249 of the built-in run. The emulator scales the 320 by 200 picture up.*

You are Ben, in a cut-down lighthouse of 32 tiles by 48. The screen follows you up and down and left and right. There is no sound.

The strip along the top is the HUD. `AMMO` is how many Fizz shots you have left, `KEY` shows `RED` while you carry the red key, `DEATHS` counts how many times you have died, and `TICK` counts game steps, 60 a second.

### The level

You start on the floor of the left shaft, with spikes a few steps to your right. The shaft climbs by ledges of one-way platforms, bright green on the screen, which you can jump up through and land on from above. A chicken walks the long third ledge, and the red key lies at its far left end. Above that the ledges lead up to the red door, a brown door in the wall that divides the two shafts. Through the door a second shaft climbs again to a gallery in the open air, where a yellow exit stands. Touching the exit ends the level: the game shows `LEVEL CLEAR` and Ben stops.

Tiles are told apart by colour: dark blue is the wall behind the rooms, dark grey and light grey are solid blocks you cannot pass, bright green is a one-way platform, bright red is spikes, brown is the red door and yellow is the exit.

### Moves

- **Walk and run:** the left and right arrows. Ben speeds up and slows down rather than stopping at once.
- **Jump:** press Jump. A full jump rises a little over three tiles, enough to climb from one ledge to the next. Let go of Jump early to cut the jump short.
- **Pogo stick:** press Pogo to get on the stick and press it again to get off. On the stick Ben bounces by himself and you steer him in the air with the arrows; he cannot walk. Hold Jump as he lands for a much higher bounce. He squashes for a moment at the bottom of each bounce.
- **Fizz:** press Fizz to fire a bubble of fizz from the Fizz Blaster in the way Ben faces. Hold Up while you press it to fire straight up, or Down while Ben is in the air to fire straight down. You have five shots, and a shot that hits a wall or flies about a second just pops. You get all five back after you die.

### The red key and the door

Walk onto the red key to pick it up; the `KEY` in the HUD turns red. Carry it to the red door and walk into it: the door opens and the key is used up. The door stays open for the rest of the run, even if you die afterwards. If you die while still carrying the key, it goes back to where it lay.

### The chicken

The chicken walks to and fro along its ledge and turns round at walls and edges. Touching it from the side or below kills you. You can deal with it two ways. Land on it from above, with or without the pogo stick, and it is stunned for three seconds and you bounce off it, higher on the stick. Or hit it with a Fizz shot and it is stunned for six seconds. A stunned chicken lies flattened and harmless and flashes for its last second before it wakes. It looks like a brown chicken with a crest; the artwork is a stand-in for the real enemy.

### Death and respawn

Spikes, the chicken, and falling out of the bottom of the map all kill you. Ben tumbles through the air for a moment, then you start again at the beginning of the level, a second and a half later, with five shots, the chicken back at its start and the key back where it lay if you were carrying it. The `DEATHS` count goes up. An open door stays open.

## The Web screen

Three lines of text show how the run is going in the Web test program.

- `TICK` counts game steps. There are 60 a second.
- `HASH` is a 16-digit fingerprint of everything that has happened so far. Two copies that were given the same moves show the same fingerprint, on any platform.
- `DASH` counts how many times you have dashed.

The square stays inside the screen. Letting go of the keys makes it slow to a stop rather than halting at once.

## Web and DOS compared

The two versions run the same rules engine, so the same moves give the same `TICK` and `HASH` on both. Today the Web page still shows the earlier test program and the DOS build plays the slice, so the two screens do not look alike yet.

- **Same:** the 60-steps-a-second pace and the two sets of keys.
- **Different:** the DOS version plays the slice, has an Esc key, four modes, `/MODERN` and a 30-second limit in `/LIVE`; the Web version has a Keys button, a fullscreen key and runs for as long as the page is open, and shows the test square. The Web version runs in a browser window of any size; the DOS version fills a 320 by 200 VGA screen. Neither version makes sound yet.

## Coming

Episode 2 is planned to bring the slice to the Web; sound, starting with the chicken's cluck; an overworld, an inner world and an underworld; more levels with keycards and doors; new gadgets beyond the pogo stick; new enemies; a lighthouse level; music; saves; and menus. None of this is in the builds yet.
