# DOS start-up check

The DOS start-up check is a read-only text screen that shows what the machine has and how the game will use its sound. This document describes the model behind it, `crates/fizz-machine`: what the screen shows, the three sound states, how choices resolve, the settings file and flags, the probe plan, and the golden file that locks the screen. [`PLATFORM.md`](PLATFORM.md) describes the DOS runtime the probes will live in, and [`ARCHITECTURE.md`](ARCHITECTURE.md) places the crate.

## Contents

- [Status](#status)
- [What the screen shows](#what-the-screen-shows)
- [The three sound states](#the-three-sound-states)
- [Attributes and monochrome adapters](#attributes-and-monochrome-adapters)
- [How a plan is resolved](#how-a-plan-is-resolved)
- [The OPL3 rule](#the-opl3-rule)
- [Settings file and flags](#settings-file-and-flags)
- [The probe plan](#the-probe-plan)
- [Memory floor](#memory-floor)
- [The golden file](#the-golden-file)
- [Not verified](#not-verified)

## Status

Only the pure model exists: the probe data, settings, the resolver, the OPL write router and the screen builder, all tested on the host. No program uses the crate yet. The DOS probes, the `/CHECK` mode and the settings file reader arrive in later changes, so nothing here is visible to a player and the manual does not mention it.

## What the screen shows

The check is an 80 by 25 text screen (code page 437 cells at `0xB8000`, or `0xB0000` in mode 7 on a monochrome adapter) in the manner of Fizz 1's start-up screen, which it ports. It is read-only: Enter continues, Esc leaves, and no other key does anything. It has no settings.

- **Title:** `LIEUTENANT FIZZ 2 - SYSTEM CHECK`, centred in the top box.
- **Video:** the adapter and its best mode. A game needs a VGA, so any other adapter marks the mode row as needed.
- **Input:** keyboard, mouse and joystick.
- **Sound:** the PC speaker, `AdLib (OPL2)` and the Sound Blaster, each in one of three states. The Sound Blaster row carries its DSP version and base as plain text, such as `Sound Blaster 4.05 at 220, OPL3`, and a Pro 1 reads `SB 3.00 at 220, 2 x OPL2 (mono)`.
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
- **Sound Blaster** is present when a DSP answered or an OPL3 is present. It is in use when music resolved to OPL3 voices or effects to channel 14.

So an SB16 whose player chose AdLib for both music and effects shows AdLib in use and the Sound Blaster available. On a Sound Blaster Pro 1 or 2.0 the Sound Blaster row is available and the AdLib row is in use, because the game uses their single OPL2. When music runs on the AdLib role and effects on channel 14 of an OPL3, both rows are in use, because one chip serves both.

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
| Effects Auto | channel 14 on an OPL3, else channel 5 on any FM chip, else the speaker |
| Effects AdLib | channel 5, else the speaker |
| Effects Sound Blaster | channel 14 on an OPL3, else channel 5, else the speaker |
| Effects Speaker | the speaker |
| Effects Off | off |

Music never uses the speaker. Music and effects are independent choices. When a request cannot be met, the plan records one of six reason codes: `music-sb-to-adlib`, `music-sb-to-off`, `music-adlib-to-off`, `fx-sb-to-adlib`, `fx-sb-to-speaker` and `fx-adlib-to-speaker`. The screen shows a fallback only through the states and the footer phrase; the reason codes are for the `SOUND` line a later change prints.

Channels: OPL2 music uses 0 to 4 and 6 to 8, OPL3 music uses 0 to 4, 9 to 13 and 15 to 17 (6 to 8 are drums), and channels 5 and 14 stay reserved for effects whatever is chosen.

## The OPL3 rule

The chip is global. `Resolved::opl` is `Opl3` exactly when an OPL3 is present and music or effects resolved to the Sound Blaster, `Opl2` when only OPL2 roles are used, and `None` when nothing uses the chip. In OPL3 mode the audio service writes register `0x105` = 1 and `0x104` = 0, and `route(mode, bank, reg, val)` sets bits 4 and 5 (`| 0x30`) on every bank-0 write to registers `0xC0` to `0xC8`, so an OPL2-format voice sounds in both speakers (centred). Bank 1 writes and every write in OPL2 or no-chip mode pass through unchanged. Shutdown and a panic write `0x105` = 0 and key off both banks; that belongs to the audio service, not this crate.

## Settings file and flags

`FIZZ2.CFG` lives in the current directory and is 8 bytes:

| Byte | Value |
| --- | --- |
| 0 to 3 | `FZ2C` |
| 4 | version, 1 |
| 5 | music: 0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster |
| 6 | effects: 0 Auto, 1 Off, 2 AdLib, 3 Sound Blaster, 4 Speaker |
| 7 | reserved, written as 0 and ignored on reading |

`Config::parse` of a missing or empty file is Auto with no issue. A wrong length, magic or version makes both fields Auto and returns a `ConfigIssue`; an unknown value makes only its own field Auto and returns an issue. `Config::encode` writes the file's bytes. The file is saved whenever the player changes a setting (the Options screen that does this does not exist yet), and the check screen reads it back to set the states.

Flags are one-run overrides that win over the file, field by field, and never write it. Matching is case-insensitive and a later flag wins over an earlier one:

| Flag | Effect |
| --- | --- |
| `/MUSIC:ADLIB`, `/MUSIC:SB`, `/MUSIC:OFF` | the music choice |
| `/FX:ADLIB`, `/FX:SB`, `/FX:SPEAKER`, `/FX:OFF` | the effects choice |
| `/ADLIB` | music AdLib, effects AdLib |
| `/SB` | music Sound Blaster, effects Sound Blaster |
| `/SPEAKER` | music off, effects speaker |
| `/NOSOUND` | music off, effects off |

`apply_flags` leaves anything else alone and counts it in its `FlagReport` (with the index of the first), so the caller can warn. The count includes the program's own mode flags, such as `/LIVE`, which the caller reads itself.

## The probe plan

The DOS runtime will fill a `Probe` with the methods below. The DOSBox Staging 0.83 results are from the planner's runs of each method; none has been run in this repository yet.

| Item | Method | DOSBox result |
| --- | --- | --- |
| Video | INT 10h AX=1A00h (BL 8 colour VGA, 7 mono VGA); else AH=12h BL=10h (BL changes on an EGA); else INT 11h bits 4 and 5 (01 or 10 CGA, 11 MDA or Hercules). MCGA is not taken as VGA | vga and svga 1A/08; ega BL=03; cga and hercules BL=10, INT 11h D426 and D436 |
| Mouse | only if the real-mode INT 33h vector is not null (a null vector hangs DOS); AX=0 returns FFFF | FFFF; driver off 0000 |
| Joystick | port `0x201` is not FF | FF; 2axis FC |
| OPL2 | Fizz 1's timer test at `0x388`: status AND E0 is 00, then C0 | none gives FF; opl2 passes |
| OPL3 | after the timer test, status AND 06 is 0 (an OPL2 reads 06) | sb16 and sbpro2 00; sb1, sb2 and opl2 06 |
| Two OPL2 (Pro 1) | the timer test answers at base+2 | sbpro1 and dualopl2 answer at `0x222`; an OPL3 reads FF there |
| Sound Blaster | DSP reset at base+6 (write 1, wait 3 microseconds, write 0); poll base+0xE bit 7; read 0xAA at base+0xA; bases from `BLASTER` `A`, then `0x220` and `0x240` only; bounded polling | answers at 220 and 240 |
| DSP version | command `0xE1`: a label only, since the chip type always comes from the status probe | sb1 1.05, sb2 2.01, sbpro1 3.00, sbpro2 3.02, sb16 4.05 |
| `BLASTER` | the environment variable, such as `A220 I7 D1 H5 T6`, parsed by `parse_blaster` | |
| x87 | `fninit; fnstcw`, then (control word AND `0x103F`) equals `0x003F` | 037F |
| CPU | EFLAGS AC toggles for a 486, ID toggles for a 586 or later; CPUID is never executed unless ID toggles | 386 0/0, 486 1/0, pentium 1/1 |
| Memory | DPMI function 0500h, free physical pages times 4 KB (not the largest block) | memsize 16, 8, 4, 2 and 1 give 3696, 1648, 624, 112 and 102 pages |
| PC speaker | assumed present | |

`parse_blaster` reads the `A`, `I`, `D`, `H`, `P` and `T` fields, tolerates any case, extra, missing, repeated and malformed fields (the last valid value of a field wins; a bad one is ignored), and never fails.

## Memory floor

`MEMORY_FLOOR_KB` is 4096. Free memory below it shows a warning row (`N MB free, 4 MB wanted`) and does not stop the game; the check only blocks start when a VGA or an x87 is missing. Fizz 1's 10 MB does not apply: it came from that game's large simulation buffer, which Fizz 2 does not have. The value is a placeholder: the DOSBox memory ladder planned with the probes (4, 2 and 1 MB) will measure what the slice needs and tune it.

## The golden file

`crates/fizz-machine/tests/check-cells.txt` holds the screen for thirteen machines in Fizz 1's format: a `== name` line, 25 lines of text with code page 437 characters shown as their Unicode look-alikes, then 25 lines of attributes at two hexadecimal digits a cell (background nibble, then foreground). The cases are an i486 with an SB16 on Auto, an AdLib card only, a Sound Blaster Pro 1, a Sound Blaster 2.0, speaker only, EGA, CGA, Hercules, a 386 without an x87, low memory, a Pentium with everything, an SB16 with music and effects set to AdLib, and an SB16 with effects on the speaker and music on the Sound Blaster.

A test compares the screen with the file. When the change is intended, regenerate it and review the diff:

```bash
BLESS=1 cargo test -p fizz-machine --test golden
```

## Not verified

- Every probe method on real hardware, and the DOSBox results above in this repository: they are the planner's runs, and the pure crate takes their outcomes as input.
- Real-hardware reads at base+2 on an OPL3, and the OPL3 status test on clones: OPL4, ESS and Yamaha parts, and the FM of an AWE or ViBRA.
- A monochrome VGA and a missing x87 beyond forced values.
- How the screen looks on a real monochrome adapter. The attribute choices follow the documented behaviour of MDA and Hercules text, but no such adapter was available.
- The screen in DOSBox: this change draws cells only on the host. The DOS wiring reads them back from video memory in a later change.
