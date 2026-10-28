# DOS start-up check

The DOS start-up check is a read-only text screen that shows what the machine has and how the game will use its sound. This document describes the model behind it, `crates/fizz-machine`: what the screen shows, the three sound states, how choices resolve, the settings file and flags, the probes and the `/PROBE` mode, and the golden file that locks the screen. [`PLATFORM.md`](PLATFORM.md) describes the DOS runtime the probes live in, and [`ARCHITECTURE.md`](ARCHITECTURE.md) places the crate.

## Contents

- [Status](#status)
- [What the screen shows](#what-the-screen-shows)
- [The three sound states](#the-three-sound-states)
- [Attributes and monochrome adapters](#attributes-and-monochrome-adapters)
- [How a plan is resolved](#how-a-plan-is-resolved)
- [The OPL3 rule](#the-opl3-rule)
- [Settings file and flags](#settings-file-and-flags)
- [The probes](#the-probes)
- [The `/PROBE` mode](#the-probe-mode)
- [The `/CHECK` and `/CHECKCELLS` modes](#the-check-and-checkcells-modes)
- [Rendering the cells to pictures](#rendering-the-cells-to-pictures)
- [Memory floor](#memory-floor)
- [The golden file](#the-golden-file)
- [Not verified](#not-verified)

## Status

The pure model and the probes exist. The model (the probe data, settings, the resolver, the OPL write router and the screen builder) is tested on the host. The probes are in the DOS runtime and `FIZZ2.EXE /PROBE` runs them, prints what they found and the hash of the screen those values make, and `tools/check-dos.sh` runs it under 21 DOSBox configurations. `FIZZ2.EXE /CHECK` shows the screen and `/CHECKCELLS` reads it back from video memory; both are described below. `FIZZ2.EXE` also reads `FIZZ2.CFG` and the sound flags, resolves them against the probe and prints the `SOUND` line in `/LIVE`, `/REPLAY` and `/PROBE` (see [Settings file and flags](#settings-file-and-flags)), and the audio service plays the cluck on the device the plan names (see [`SFX.md`](SFX.md#dos-playback)). `/CHECK` and `/CHECKCELLS` read the same file and flags, so the sound rows show what the game would use.

## What the screen shows

The check is an 80 by 25 text screen (code page 437 cells at `0xB8000`, or `0xB0000` in mode 7 on a monochrome adapter) in the manner of Fizz 1's start-up screen, which it ports. It is read-only: Enter continues, Esc leaves, and no other key does anything. It has no settings.

- **Title:** `LIEUTENANT FIZZ 2 - SYSTEM CHECK`, centred in the top box.
- **Video:** the adapter and its best mode. A game needs a VGA, so any other adapter marks the mode row as needed.
- **Input:** keyboard, mouse and joystick.
- **Sound:** the PC speaker, `AdLib (OPL2)` and the Sound Blaster, each in one of three states. The Sound Blaster row shows only the device and its chip: `Sound Blaster (OPL3)`, `Sound Blaster (OPL2)` for an SB 1.x or 2.0, and `Sound Blaster (2 x OPL2)` for a Pro 1; with no Sound Blaster it reads `Sound Blaster`. The detailed text (DSP version, base and `mono` wording, such as `Sound Blaster 4.05 at 220, OPL3`) is still built by `sound_blaster_detail` and is switched on by `SHOW_SB_DETAIL` in `screen.rs`, which is `false`, for a later detailed view.
- **Machine:** the processor class, the x87 (needed) and the free memory.
- **Footer:** the resolved plan as a read-only phrase (`Music: Sound Blaster (OPL3)   Effects: PC speaker`), then `Press Enter to play, Esc to quit`, or, if a VGA or an x87 is missing, `! Needs VGA and x87: Esc to quit`.

Fizz 1's speed test and its "14 lighting shades" row are dropped. The Video panel therefore has two rows and the third is blank.

## The three sound states

Each sound row is in exactly one state, derived from the probe and the resolved plan, with no extra lines and no bracket text:

| State | Meaning | Marker |
| --- | --- | --- |
| Missing | The device is not on this machine | `x` |
| Available | The device is on this machine and the plan does not use it | a tick |
| In use | The plan uses it for music, effects or both | a pointer |

The rows are defined this way:

- **PC speaker** is assumed present. It is in use when effects resolved to the speaker.
- **AdLib (OPL2)** is present when any FM chip answered. It is in use when music resolved to OPL2 voices or effects to channel 5.
- **Sound Blaster** is present when a DSP answered or an OPL3 is present. It is in use when music resolved to OPL3 voices or effects to the OPL3 pair (channels 5 and 14).

So an SB16 whose player chose AdLib for both music and effects shows AdLib in use and the Sound Blaster available. On a Sound Blaster Pro 1 or 2.0 the Sound Blaster row is available and the AdLib row is in use, because the game uses their single OPL2. When music runs on the AdLib role and effects on the OPL3 pair, both rows are in use, because one chip serves both.

## Attributes and monochrome adapters

A cell holds the character in the low byte and the attribute in the high byte: foreground in bits 0 to 3, background in bits 4 to 6, and bit 7 always clear so nothing blinks.

| State | Colour adapters | Monochrome adapter (MDA, Hercules) |
| --- | --- | --- |
| Missing | `0x08` dark grey on black | `0x07` normal |
| Available | `0x07` light grey on black | `0x0F` bright |
| In use | `0x1F` bright white on blue, across the whole row | `0x70` inverse, across the whole row |

A monochrome adapter in mode 7 renders only normal (`0x07`), bright (`0x0F`) and inverse (`0x70`) and shows a dark grey foreground on black as black, so there the screen uses those three values and nothing else, and the order dim, normal, highlighted is kept. The markers repeat each state in the character, so no state depends on an attribute alone. Low memory and needed items use a yellow-on-brown and a white-on-red bar on colour adapters. A VGA driving a monochrome display renders colour attributes as greys, so it keeps the colour set.

## How a plan is resolved

`resolve(&Settings, &Probe)` is deterministic and never fails. A Sound Blaster Pro 1 counts as one OPL2 in mono. An AdLib card beside a separate Sound Blaster is not supported as two chips, because both answer at `0x388`.

| Choice | Result |
| --- | --- |
| Music Auto | OPL3 voices on an OPL3, else OPL2 voices on any FM chip, else off |
| Music AdLib | OPL2 voices, else off |
| Music Sound Blaster | OPL3 voices on an OPL3, else OPL2 voices, else off |
| Music Off | off |
| Effects Auto | the OPL3 pair (channel 5 left, channel 14 right) on an OPL3, else channel 5 on any FM chip, else the speaker |
| Effects AdLib | channel 5, else the speaker |
| Effects Sound Blaster | the OPL3 pair on an OPL3, else channel 5, else the speaker |
| Effects Speaker | the speaker |
| Effects Off | off |

Music never uses the speaker. Music and effects are independent choices. When a request cannot be met, the plan records one of six reason codes: `music-sb-to-adlib`, `music-sb-to-off`, `music-adlib-to-off`, `fx-sb-to-adlib`, `fx-sb-to-speaker` and `fx-adlib-to-speaker`. The screen shows a fallback only through the states and the footer phrase; the reason codes are for the `SOUND` line (see below).

Channels: OPL2 music uses 0 to 4 and 6 to 8, OPL3 music uses 0 to 4, 9 to 13 and 15 to 17 (6 to 8 are drums), and channels 5 and 14 stay reserved for effects whatever is chosen.

## The OPL3 rule

The chip is global. `Resolved::opl` is `Opl3` exactly when an OPL3 is present and music or effects resolved to the Sound Blaster, `Opl2` when only OPL2 roles are used, and `None` when nothing uses the chip. In OPL3 mode the audio service writes register `0x105` = 1 and `0x104` = 0, and `route(mode, bank, reg, val)` sets bits 4 and 5 (`| 0x30`) on every bank-0 write to registers `0xC0` to `0xC8`, so an OPL2-format voice sounds in both speakers (centred). Bank 1 writes and every write in OPL2 or no-chip mode pass through unchanged. Shutdown and a panic write `0x105` = 0 and key off both banks; that belongs to the audio service, not this crate.

## Settings file and flags

`FIZZ2.CFG` lives in the current directory. Version 3, the version written, is 13 bytes:

| Byte | Value |
| --- | --- |
| 0 to 3 | `FZ2C` |
| 4 | version, 3 |
| 5 | music: 0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster |
| 6 | effects: 0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster, 4 Speaker |
| 7 | frame rate: 0 Auto, 1 30 Hz, 2 60 Hz (read by `/LIVE` and `/REPLAY`, see below) |
| 8 | detail level, reserved: written as 0 and ignored on reading |
| 9 | motion: 0 Full, 1 Reduced (the death shake and the death spin) |
| 10 | sound placement: 0 Enhanced, 1 Classic (Classic centres every effect at full volume) |
| 11 | captions: 0 On, 1 Off |
| 12 | keys: 0 Keen, 1 Modern |

In every field zero is the default, so a file of zeros after the header is a player who has changed nothing. Version 2, which an earlier build wrote, is the first 9 bytes with version 2; version 1 is 8 bytes: `FZ2C`, version 1, music, effects, one reserved byte. Both are still read, with the fields they lack at their defaults, and saving writes version 3. The length must match the version (8 for 1, 9 for 2, 13 for 3); the reader keeps one length per version and the layout has no spare bytes.

`Config::parse` (sound only) and `Config::parse_prefs` (every field) of a missing or empty file are all defaults with no issue. A length that does not fit the version, a wrong magic or an unknown version makes every field its default and returns a `ConfigIssue`; an unknown value makes only its own field its default and returns an issue (the first one found when there are several). `Config::encode_prefs` writes the file's bytes; `Config::encode` writes sound settings with every other field at its default and is for callers that hold no other field. The file is saved whenever the player changes a setting (the Options screen that does this does not exist yet), and the check screen reads it back to set the states.

Flags are one-run overrides that win over the file, field by field, and never write it. Matching is case-insensitive and a later flag wins over an earlier one:

| Flag | Effect |
| --- | --- |
| `/MUSIC:ADLIB`, `/MUSIC:SB`, `/MUSIC:OFF` | the music choice |
| `/FX:ADLIB`, `/FX:SB`, `/FX:SPEAKER`, `/FX:OFF` | the effects choice |
| `/ADLIB` | music AdLib, effects AdLib |
| `/SB` | music Sound Blaster, effects Sound Blaster |
| `/SPEAKER` | music off, effects speaker |
| `/NOSOUND` | music off, effects off |
| `/FPS30`, `/FPS60`, `/FPSAUTO` | the frame rate (byte 7): 30 Hz, 60 Hz, or Auto; they win over the file, the last one wins, and they combine with the sound flags and any mode |
| `/MODERN`, `/KEEN` | the key layout (byte 12): `/KEEN` overrides a saved Modern |
| `/CAPTIONS`, `/NOCAPTIONS` | the captions (byte 11) |

Motion and sound placement have no switch yet. `Invocation::over(&Prefs)` in `switches.rs` replaces each field a switch chose in the file's `Prefs` and leaves the rest; the result is what the run uses, and the file's own `Prefs` are what a later save writes, so a switch is never saved.

`apply_flags` leaves anything else alone and counts it in its `FlagReport` (with the index of the first), so the caller can warn. The count includes the program's own mode flags, such as `/LIVE`, which the caller reads itself. The program's switches, these among them, are listed once in `fizz_machine::switches`, which also writes the `/?` help and sends an unrecognised switch or a bad value such as `/MUSIC:FOO` to an error line and exit status 2 (see [PLATFORM.md](PLATFORM.md)).

### On DOS

`dos/fizz2/src/sound.rs` does the reading. `load_file` opens `FIZZ2.CFG` through the runtime's `dos::File` (a missing or unreadable file is Auto with no message; it reads at most 32 bytes, so a longer file fails the length check), `Plan::new` applies the flags to the file's settings (file over Auto, flag over file, per field) and resolves them against the probe, and the plan's line is printed once the screen is back in text mode. The probe runs first because its waits need the timer at the BIOS rate, so `/LIVE` and `/REPLAY` probe before they hook the timer. `sound::is_flag` lets the mode search skip the sound flags, so `FIZZ2.EXE /NOSOUND` is still `/LIVE`. The plan is kept in `Plan::resolved` for the audio service to use.

`/LIVE` and `/REPLAY` also print the `AUDIO WRITES <n> SPEAKER STEPS <m>` line (effects and the start and stop key-offs only) and, next to it, `MUSIC SONG crater ARR <opl2|opl3|off> TICKS <t> WRITES <w> LOOPS <k> HASH <fnv>`: the arrangement the plan named and what the music wrote, counted by the service (`TICKS` is the music player's own 560 Hz count, not wall time; `HASH` is the 32-bit FNV-1a of each write's tick, bank, register and value as the stream authored it, 8 lower-case hexadecimal digits). `MUSIC DETAIL ORDER <ok|bad|none> MAX_PER_TICK <n>` says whether the OPL3 enable preceded the first music write and the most writes the music put on one tick. `tools/check-dos.sh` requires `ARR` to match the `SOUND` line, zero everywhere for `off`, and for a chip the host's `fizz_music::writes_for` result for the same `TICKS` (the `mline` example prints the line). `/MUSICTEST` (a hidden developer switch) prints the `SOUND`, `MUSIC` and `MUSIC DETAIL` lines after stepping the prelude, two full loops and one tick through the real port writes with no timer, and the check compares it with the host for the OPL2 and OPL3 arrangements.

`/REPLANTEST` (a hidden developer switch) runs the audio service and changes its devices live through `audio::replan`: eight plans (Auto, AdLib, Sound Blaster music with the speaker, music off on the speaker, Sound Blaster for both, everything off, AdLib music with the speaker, and Auto again), each resolved against the machine, with a cluck started 100 ticks before each switch so the next switch comes while it still sounds. It prints `REPLAN <from> -> <to> GATE0 <g> OPL3 <on|off> GATE <g> CHANGED <yes|no> MUSIC0 <t> WRITES <n> STEPS <n> TICKS <n> MUSIC <t>` for each plan and then `REPLAN DONE 8`, `FPU CW` and `VECTORS RESTORED`. `GATE0` is port 0x61 just before the switch and `GATE` as the switch returned (both read from the port); `OPL3` is the service's record of its last write to register 0x105; `MUSIC0` is the music player's tick count when the switch returned (0 when the music started again); `WRITES`, `STEPS` and `TICKS` count the OPL writes, speaker steps and handler ticks of the half second that follows. `tools/check-dos.sh` runs it under an SB16, a Sound Blaster Pro 2, an AdLib alone and no card, and requires on every line that OPL3 is on exactly when the new plan has OPL3 music or the pair (so off after a switch to AdLib), that the gate is closed after any change (and that at least one change left a sounding speaker cluck), that `TICKS` rises, that music writes and ticks follow a plan with music and speaker steps a plan with the speaker (and neither otherwise), that music starts again from `MUSIC0 0` when its device changed, and that the run ends with the vectors restored. DOSBox is the only evidence: nobody has heard a live switch.

The Options screen in play (`dos/fizz2/src/menu.rs`, described in [PLATFORM.md](PLATFORM.md#the-dos-runtime)) is exercised by `tools/check-dos.sh` through DOSBox's autotype. Every `/LIVE` run that used to end on `esc` now types `esc up enter`: Esc opens Paused, Up from Resume wraps to Quit to DOS, and Enter chooses it. The Options cases type, from Paused, `down enter` to open Options (the rows are Music, Effects, Sound placement, Frame rate, Motion, Captions, Keys, Back) and `esc down enter` to leave it and Quit to DOS. They check: Classic placement (two `down`, `right`) saves `465A3243030000000000010000` and prints `OPTIONS SAVED yes`; a visit that changes nothing prints no `OPTIONS SAVED` line and creates no file; the frame rate set to 30 (three `down`, two `right`) gives `FRAMES ... RATE 30 MODE 30` and the 30 Hz byte; `/OPTIONS /FPS30` over a file with 60 Hz saves the placement and keeps the file's frame-rate byte, with `FRAMES ... MODE 30` for the run; `/OPTIONS` stops no ticks (`LIVE TICKS 0`), `/OPTIONS /MODERN` takes `s s d esc` and does not save the layout; a read-only `FIZZ2.CFG` prints `OPTIONS SAVED no`, exits 0 with the vectors restored and leaves the file and the directory as they were; and a run left in Paused ends at the wall-clock cap with few world ticks. `OPTIONS SAVED yes|no` is printed after `AUDIO` and `MUSIC` and before `MOVED`, only when a save was tried. The light-red message itself is not seen by any check (the check has no screen to read); `fizz-present-dos` tests prove the pixels. `/OPTIONS` is a hidden developer switch that opens the Options screen at once and leaves on Esc.

`/MENUSHOT` (a hidden developer switch) draws the Options and Paused screens over the slice's first frame, presents each, and prints `MENUSHOT options <digest>` and `MENUSHOT paused <digest>` (the FNV-1a digest of the whole frame, as `BENCH FRAME` uses). It starts no timer, keyboard or audio hook. `tools/check-dos.sh` requires both lines to equal what `cargo run -p fizz-present-dos --example menushot` prints on the host.

The `SOUND` line is `SOUND MUSIC <x> FX <y>` and then, each after a space, the reasons the result is not what was asked for, in this order: a file problem (`cfg-length`, `cfg-magic`, `cfg-version`, `cfg-music`, `cfg-effects`, `cfg-framerate`, `cfg-motion`, `cfg-placement`, `cfg-captions`, `cfg-keys`), the music reason, the effects reason. `<x>` is `off`, `opl2` or `opl3`; `<y>` is `off`, `ch5`, `ch14` (the OPL3 pair: the word keeps the name of its second channel, which is the one that makes channel 5 stereo) or `speaker`. `fizz_machine::write_sound` writes it.

`sound::frame_rate(flag)` reads the frame-rate byte with the same `load_file` and returns the flag if one was given, else the file's byte, else Auto; `frames::Drawing` builds the `FrameGovernor` from it (see [PLATFORM.md](PLATFORM.md#the-dos-runtime)). A flag never reaches `save`, so `FIZZ2.CFG` is never written by `/FPS30` and the others. `tools/check-dos.sh` plants version 2 files with the byte at 0, 1, 2 and 5 (a bad value is Auto, with `cfg-framerate` in the `SOUND` line), runs `/LIVE` with and without a flag over each and checks the `FRAMES` line's rate and mode, the half-as-many frames at 30 Hz and that the file is byte for byte as planted.

`main` reads the file with `sound::load_file`, applies the switches with `Invocation::over` and decides from the result: `/LIVE`, `/REPLAY` and `/BENCH` draw captions only when Captions is On, run the death shake and spin only when Motion is Full (`DrawOptions::shake`), and place clucks by where the chicken is (Enhanced) or at the centre (Classic); `/LIVE` uses the Keen or the Modern layout.

`sound::save` writes the file the safe way (`dos/fizz2/src/cfgfile.rs`, described in [PLATFORM.md](PLATFORM.md#safe-writes-and-int-24h)): it writes `FIZZ2.NEW`, reads it back and compares, deletes `FIZZ2.CFG` and renames the new file over it, and returns the error if any step fails. `load_file` falls back to `FIZZ2.NEW` when `FIZZ2.CFG` is missing (a save that was interrupted after the delete); a damaged `FIZZ2.NEW` is Auto with the usual `cfg-` reason. The Options screen calls it when it is left with a change (see below), and so does `/CFGTEST`, a test mode and not a player's switch: it saves the settings the flags give, with the frame rate 60 Hz, reads the file back and prints `CFG SAVED`, `CFG BYTES <18 hexadecimal digits>` (what was read back, re-encoded), `CFG ROUNDTRIP yes|no`, `INT24 RESTORED yes|no` (the real-mode critical-error vector is where it was) and the platform lines. `tools/check-dos.sh` also plants a read-only `FIZZ2.CFG` (expects `CFG SAVED no`, exit 0, both restored lines `yes`, the file unchanged and no `FIZZ2.NEW`), a leftover valid `FIZZ2.NEW` with no `FIZZ2.CFG` (the `SOUND` line shows its settings and reading creates nothing), a damaged one (Auto), and checks that a normal save leaves no `FIZZ2.NEW`. It overwrites `FIZZ2.CFG` in the current directory, so `tools/check-dos.sh` runs it in a scratch copy of `dist/dos`. That script also runs `/PROBE` under sb16, sb1 and no card with each flag, with a version 2 file, a version 1 file and each kind of damaged file, checks the exact `SOUND` line, checks that a run with flags leaves the file byte for byte as it was (or absent), and runs `/REPLAY` under eight combinations of file and flags, each of which must print the golden `TICK`, `FINAL` and `EVENT` lines.

Nothing in the program calls `sound::save` yet except `/CFGTEST`, a test mode and not a player's switch: it saves the settings the flags give, with the frame rate 60 Hz, Reduced motion and Classic placement (so no byte is left at zero by accident) and the keys and captions that `/MODERN` and `/NOCAPTIONS` give, reads the file back and prints `CFG SAVED`, `CFG BYTES <26 hexadecimal digits>` (what was read back, re-encoded), `CFG ROUNDTRIP yes|no` and the platform lines. It overwrites `FIZZ2.CFG` in the current directory, so `tools/check-dos.sh` runs it in a scratch copy of `dist/dos`. That script also runs `/PROBE` under sb16, sb1 and no card with each flag, with a version 3 file, a version 2 file, a version 1 file and each kind of damaged file (a bad motion, placement, captions or keys byte resets only that field and names it), checks the exact `SOUND` line, checks that a run with flags leaves the file byte for byte as it was (or absent), and runs `/REPLAY` under eight combinations of file and flags, each of which must print the golden `TICK`, `FINAL` and `EVENT` lines.

## The probes

The probes are in `dos/rt/src/probe.rs`; `dos/fizz2/src/machine.rs` calls them in dependency order (the `BLASTER` variable, the Sound Blaster DSP, then the FM chip, which uses the DSP's base) and fills a `Probe`. All of them run before the 60 Hz timer is hooked. The waits use `pit::delay_us`, which reads the latched PIT channel 0 count (two counts per input clock in the BIOS's mode 3) and refuses to run while the timer is hooked, because the counter then has another period; a probe that cannot wait reports the device as absent. Every wait on a device is bounded in the number of reads, so a missing or misbehaving card costs milliseconds and never a hang. Real-mode interrupts go through `dpmi::simulate_int`. The DOSBox Staging 0.83 results below are from the runs `tools/check-dos.sh` makes.

| Item | Method | Failure modes | DOSBox result |
| --- | --- | --- | --- |
| Video | INT 10h AX=1A00h (BL 8 colour VGA, 7 mono VGA); else AH=12h BL=10h (BL changes on an EGA); else INT 11h bits 4 and 5 (01 or 10 CGA, 11 MDA or Hercules). MCGA is not taken as VGA | A BIOS without the display combination call falls through to the EGA call; an equipment word of 00 is taken as CGA | `svga_s3` and `vgaonly` vga; `ega` ega; `cga` cga; `hercules` mdahercules |
| Mouse | only if the real-mode INT 33h vector is not null (a null vector hangs DOS); AX=0 returns FFFF | A vector that points at an `IRET` returns AX unchanged, which reads as no mouse | built-in driver yes; `builtin_dos_mouse_driver = off` no |
| Joystick | port `0x201` is not FF | A floating bus that reads something other than FF reports a joystick | none no; `joysticktype = 2axis` yes |
| OPL2 | Fizz 1's timer test at `0x388`: status AND E0 is 00, then C0 | A card that mirrors the ports could pass; a chip that is slow to start its timer fails | `sbtype = none` none; sb1, sb2, sb16 with `oplmode = opl2`, and none with `oplmode = opl2`: passes |
| OPL3 | after the timer test, status AND 06 is 0 (an OPL2 reads 06) | OPL3 clones that read 06 are taken as OPL2, which is the safe side | sbpro2 and sb16 opl3; sb1, sb2 and `oplmode = opl2` opl2 |
| Two OPL2 (Pro 1) | on a machine with a DSP, and not an OPL3, the timer test answers at DSP base+2 | Not tried without a DSP, so an AdLib card and a second chip elsewhere is not found | sbpro1 dualopl2; sbpro2 and sb16 do not reach it |
| Sound Blaster | DSP reset at base+6 (write 1, wait 5 microseconds, write 0); poll base+0xE bit 7 up to 200 times; read 0xAA at base+0xA; bases: `BLASTER` `A` when it is 210h to 280h in steps of 10h, then 220h and 240h | An implausible `A` is ignored. A card at another base is not found. An empty bus reads FF, which is not 0xAA | answers at 220 and, with `sbbase = 240`, at 240 |
| DSP version | command `0xE1` once the write port is ready: a label only, since the chip type always comes from the status probe | If the version cannot be read it is 0.00 | sb1 1.05, sb2 2.01, sbpro1 3.00, sbpro2 3.02, sb16 4.05 |
| `BLASTER` | the environment variable, such as `A220 I7 D1 H5 T6`, parsed by `parse_blaster` | Missing, repeated and malformed fields are ignored | sb1 `A220 I7 D1 T1`; sb2 `T3`; sbpro1 `T2`; sbpro2 `T4`; sb16 `H5 T6`; none unset |
| x87 | `fninit; fnstcw`, then (control word AND `0x103F`) equals `0x003F`; the control word is put back to `0x027F` | Forced values only: no emulated machine here lacks an x87 | yes in every configuration (037F after `fninit`) |
| CPU | EFLAGS AC (bit 18) toggles for a 486, ID (bit 21) toggles for a 586 or later. CPUID is never executed | A 486 without the ID flag is a 486 even if it has CPUID | `cputype = 386` 386; the default 486; `pentium` 586 |
| Memory | DPMI function 0500h, free physical pages (field 5) times 4 KB, not the largest block; a count above the machine's total means none | A host that returns FFFFFFFFh gives 0 | memsize 16, 8, 4, 2 and 1: 14,544, 6,352, 2,256, 208 and 168 KB, which are 3,636, 1,588, 564, 52 and 42 pages |
| PC speaker | assumed present | | |

`parse_blaster` reads the `A`, `I`, `D`, `H`, `P` and `T` fields, tolerates any case, extra, missing, repeated and malformed fields (the last valid value of a field wins; a bad one is ignored), and never fails.

The planner's runs gave 3,696, 1,648, 624, 112 and 102 free pages for the same memory sizes with a smaller program; `FIZZ2.EXE` itself holds about 60 pages more (its image and a 256 KB heap block), which accounts for the difference. Every other result matched the planner's.

## The `/PROBE` mode

`FIZZ2.EXE /PROBE` runs the probes and prints, with no key press and without touching video memory or the timer:

- `PROBE <key> <value>`, one line for each `Probe` field in this order: `video` (`vga`, `monovga`, `ega`, `cga`, `mdahercules`), `mouse`, `joystick` (`yes` or `no`), `fm` (`none`, `opl2`, `dualopl2`, `opl3`), `dsp` (`none`, or the version and the base in hexadecimal, such as `4.05 220`), `blaster` (`none`, or the fields present, such as `A220 I7 D1 H5 T6`), `x87`, `cpu` (`386`, `486`, `586`) and `free_kb`.
- `CELLS <hash>`: the FNV-1a 64 hash, in 16 hexadecimal digits, of the 4,000 bytes of the 2,000 cells that `cells` builds for those values with the default settings (character, then attribute, in video-memory order).
- `HEAP <bytes>`, what the program's heap has taken from the host, and `PAGES FREE <n>`, the host's free page count.
- `EXIT yes`, `FPU CW 027F` and `VECTORS RESTORED yes`, the platform lines the other modes print (the vectors are compared before and after the probes).

`write_probe` writes the lines and `parse_probe` reads them back, both in `fizz-machine`, so the program and the host agree on the spelling. `cargo run -q -p fizz-machine --example cells-hash` reads a transcript on standard input and prints the `CELLS` line the host expects for its `PROBE` lines. `tools/check-dos.sh` runs `/PROBE` under these configurations, asserts the expected `PROBE` lines of each and that the program's `CELLS` equals the host's: no sound card, sb1, sb2, sbpro1, sbpro2, sb16 (also at base 240, with `oplmode = opl2` and with `oplmode = none`), no card with `oplmode = opl2`, the mouse driver off, a 2-axis joystick, the machines `ega`, `cga` and `hercules`, `cputype` 386 and pentium, and `memsize` 16, 4, 2 and 1. The runs take about 25 seconds in all. The check never runs the interactive `/CHECK`, which does not exist yet.

## The `/CHECK` and `/CHECKCELLS` modes

`FIZZ2.EXE /CHECK` runs the probes, resolves the sound plan, builds the 2,000 cells with `fizz_machine::cells` and writes them to the text page: `0xB8000` in mode 3, or `0xB0000` in mode 7 when the adapter is MDA or Hercules (`VideoClass::is_text_mono`). It runs before any timer or keyboard hook (the probes' waits need the PIT at the BIOS rate), reads keys through the BIOS (INT 16h), and hides the cursor. Esc puts the text mode back and exits with status 0; Enter puts it back and goes on to `/LIVE`. Enter is ignored while `ready` says a VGA or an x87 is missing, as the footer states. Any other key does nothing. The mode is registered with the clean-up list, so a panic leaves the screen in text mode too. The code is `dos/fizz2/src/check.rs` and `dos/rt/src/text.rs`.

`/CHECK` waits for a person, so no script, test or CI job may run it. `crates/fizz-machine/tests/no-check-in-scripts.rs` reads every script and workflow under `tools/` and `.github/` and fails on any non-comment line that names `/CHECK` other than as the start of `/CHECKCELLS`.

`FIZZ2.EXE /CHECKCELLS` is the form scripts use. It draws the same screen, reads the 4,000 bytes back from video memory, puts the text mode back and prints, with no key press:

- the `PROBE` lines, as `/PROBE` does;
- `CELLROW <row> <hex>`, 25 lines, each with 160 hexadecimal digits: the character byte and then the attribute byte of each of the row's 80 cells, in video-memory order (`write_cells_hex` and `parse_cells_hex` in `fizz-machine`);
- `CELLS <hash>`, the hash of the bytes read back, and `CELLS WRITTEN <hash>`, the hash of the cells the program wrote (the two differ only if the adapter's memory did not hold what was written);
- `EXIT yes`.

`cargo run -q -p fizz-machine --example cells-hex` reads those `PROBE` lines on standard input and prints the `CELLROW` lines the pure builder makes for the same values with the default settings. `tools/check-dos.sh` runs `/CHECKCELLS` for a machine with no sound card, an SB16, an SB Pro 1 and the EGA, CGA and Hercules machines, and fails unless the lines are identical. A unit test in `fizz-machine` checks that the `CELLROW` text round-trips through `parse_cells_hex` for a colour, an EGA, a CGA and a monochrome screen.

## Rendering the cells to pictures

`python3 -I tools/render-cells.py <input> <output-dir>` draws cells as PNG files with a VGA 8x16 font, for the manual and the archive. The input is the golden file (one picture for each case, named after it) or the output of `/CHECKCELLS` (one picture, `check.png`). The sixteen text colours are VGA's. The font is not part of the repository: the tool reads a PSF2 console font with a Unicode table from the machine that runs it (the Linux `default8x16` by default, or `--font <file>`), and uses only the Python standard library. Running it over the golden file produces thirteen 1280 by 800 pictures, which are not committed.

## Memory floor

`MEMORY_FLOOR_KB` is 1,024. Free memory below it shows a warning row (`N KB free, 1 MB wanted`) and does not stop the game; the check only blocks start when a VGA or an x87 is missing. Fizz 1's 10 MB does not apply: it came from that game's large simulation buffer, which Fizz 2 does not have.

The value comes from a ladder. `/REPLAY` was run in DOSBox at `memsize` 4, 3, 2 and 1 (DOSBox's smallest), and each run printed the same transcript, the final hash (`2145503c0d3b625f` when the ladder was run; the golden has changed since) and `EXIT yes` as the 16 MB run, with the sound interrupt not yet present. So the slice has no memory floor above DOSBox's minimum. At `memsize = 1` CWSDPMI serves extended memory from what DOS leaves (about 410 KB), and after the program has loaded and taken its 256 KB heap block 168 KB (42 pages) is still free, so the slice needs nothing beyond what the program already holds. One megabyte free is a margin for what the next milestones add (the audio buffers and a level's pack), and it still warns a 2 MB machine, which has about 208 KB free. A DOSBox ladder cannot see the speed cost of paging on a real machine, and CWSDPMI's swap file was never created in any run. `tools/check-dos.sh` repeats the `/REPLAY` at `memsize = 1` and compares its transcript with the golden file. Raise the value when the audio service or the packs land and the ladder is run again.

When the constant changed from its first value of 4,096, the golden cells for the low-memory machine changed with it (the case is now 512 KB, so it still shows the warning), and `BLESS=1 cargo test -p fizz-machine --test golden` regenerated the file.

## The golden file

`crates/fizz-machine/tests/check-cells.txt` holds the screen for thirteen machines in Fizz 1's format: a `== name` line, 25 lines of text with code page 437 characters shown as their Unicode look-alikes, then 25 lines of attributes at two hexadecimal digits a cell (background nibble, then foreground). The cases are an i486 with an SB16 on Auto, an AdLib card only, a Sound Blaster Pro 1, a Sound Blaster 2.0, speaker only, EGA, CGA, Hercules, a 386 without an x87, low memory (512 KB), a Pentium with everything, an SB16 with music and effects set to AdLib, and an SB16 with effects on the speaker and music on the Sound Blaster.

A test compares the screen with the file. When the change is intended, regenerate it and review the diff:

```bash
BLESS=1 cargo test -p fizz-machine --test golden
```

## Not verified

- Every probe method on real hardware. The DOSBox results above are from DOSBox Staging 0.83 only, and the pure crate takes their outcomes as input.
- Real-hardware reads at base+2 on an OPL3 (the probe does not touch that port once the OPL3 test has passed, so DOSBox's answer there was not seen), and the OPL3 status test on clones: OPL4, ESS and Yamaha parts, and the FM of an AWE or ViBRA.
- A monochrome VGA, and a missing x87: DOSBox always emulates one, so the no-x87 path is exercised with forced values only.
- The DSP and FM timing on real cards: the delays (3.3 and 23 microseconds after an FM write are rounded up to 4 and 24, the reset wait to 5) follow the datasheets, and DOSBox does not enforce them.
- `pit::delay_us` on a machine whose timer another program left in a different mode: it assumes the BIOS's mode 3 and cannot read the mode back from the PIT.
- How the screen looks on a real monochrome adapter. The attribute choices follow the documented behaviour of MDA and Hercules text, but no such adapter was available.
- The screen on a person's eyes: `/CHECKCELLS` proves that video memory holds the cells the builder made (in DOSBox, for colour, EGA, CGA and Hercules), and the renders show what they should look like with a VGA font, but the interactive `/CHECK` (keys, cursor hiding, Enter to `/LIVE`) is run by hand only and was not driven in this change. On a real machine nothing was seen.
