# Lieutenant Fizz 2 — Player's Manual

This manual is updated with every change a player can see or do, in the same pull request as the change. It describes the builds as they are today, not as they are planned. The rule is in the "Game Manual" section of [`AGENTS.md`](../AGENTS.md).

## What this is

Lieutenant Fizz 2 is a technical preview. There is no game yet: no levels, no enemies, no items and no story to play. What you can run today is a small test program, available on the Web and on DOS, that shows a yellow square on a dark blue screen that you can move around and make dash. It exists to prove that both versions behave identically.

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
| Fire | Space | `C` or Space |
| Quit | Esc (DOS only) | Esc (DOS only) |
| Fullscreen | `F` (Web only) | `F` (Web only) |

Only the arrows and Jump do anything in this preview: the arrows move the square and Jump makes it dash in the direction you last moved. Pogo and Fire are ready for the game but have no effect yet. Keys from the other set do nothing, so `W` does not move you under the Keen keys.

On the Web there is no quit key; close the tab.

### Choosing your keys

- **Web:** the Keys button under the screen switches between Keen and Modern, and the line above it lists the keys in use. The page remembers your choice on this browser, if the browser allows it to.
- **DOS:** start with `FIZZ2.EXE /MODERN` for the Modern keys. Without it you get the Keen keys.

### Fullscreen and browser shortcuts

On the Web, press `F` or use the Fullscreen button to fill the screen, and press it again to leave. A web page cannot stop a browser tab from acting on a few shortcuts: Ctrl+W closes the tab whatever the game does, and some desktops keep their own window and workspace shortcuts. In fullscreen, Chromium-based browsers (Chrome, Edge and similar) let the game keep Ctrl, Alt and the other game keys to itself; other browsers do not. If a shortcut keeps getting in your way, switch to the Modern keys, which do not use Ctrl or Alt.

### DOS modes

Add one of these after the program name, such as `FIZZ2.EXE /REPLAY`. With no mode, you get `/LIVE`. Add `/MODERN` as well, or by itself, to use the Modern keys in `/LIVE`, as in `FIZZ2.EXE /MODERN`.

- `/LIVE` — you control the square. It ends when you press Esc, or by itself after 30 seconds, and then prints the final tick, the hash, and whether you moved.
- `/REPLAY` — plays a built-in 10-second script by itself, with no keyboard input, then prints the list of hashes it reached. Use it to check that your copy agrees with the Web version.
- `/PANIC` — makes the program stop on purpose, to show that it puts the keyboard, timer and screen mode back as it found them. It is a test, and it exits with status 3.

## The screen

Three lines of text show how the run is going.

- `TICK` counts game steps. There are 60 a second.
- `HASH` is a 16-digit fingerprint of everything that has happened so far. Two copies that were given the same moves show the same fingerprint, on any platform.
- `DASH` counts how many times you have dashed.

The square stays inside the screen. Letting go of the keys makes it slow to a stop rather than halting at once.

## Web and DOS compared

The two versions run the same rules, so the same moves give the same `TICK` and `HASH` on both.

- **Same:** the colours, the two sets of keys, the 60-steps-a-second pace and the HUD wording.
- **Different:** the DOS version has an Esc key, three modes, `/MODERN` and a 30-second limit in `/LIVE`; the Web version has a Keys button, a fullscreen key and runs for as long as the page is open. The Web version runs in a browser window of any size; the DOS version fills a 320 by 200 VGA screen. Neither version makes sound yet.

## Coming

Episode 2 is planned to bring an overworld, an inner world and an underworld; levels with keycards and doors; new gadgets beyond the pogo stick; new enemies; a lighthouse level; music; saves; and menus. None of this is in the builds yet.
