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

| Key | What it does |
| --- | --- |
| Arrow keys or `W` `A` `S` `D` | Move the square |
| Space | Dash in the direction you last moved |
| Esc | Quit (DOS only) |

On the Web there is no quit key; close the tab.

### DOS modes

Add one of these after the program name, such as `FIZZ2.EXE /REPLAY`. With no mode, you get `/LIVE`.

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

- **Same:** the colours of the square and the text, the controls, the 60-steps-a-second pace and the HUD wording.
- **Different:** the DOS version has an Esc key, three modes and a 30-second limit in `/LIVE`; the Web version runs for as long as the page is open. The Web version runs in a browser window of any size; the DOS version fills a 320 by 200 VGA screen. The screen behind the square is a darker blue on the Web than on DOS. Neither version makes sound yet.

## Coming

Episode 2 is planned to bring an overworld, an inner world and an underworld; levels with keycards and doors; new gadgets beyond the pogo stick; new enemies; a lighthouse level; music; saves; and menus. None of this is in the builds yet.
