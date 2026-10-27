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
- [Memory floor](#memory-floor)
- [The golden file](#the-golden-file)
- [Not verified](#not-verified)

## Status

The pure model and the probes exist. The model (the probe data, settings, the resolver, the OPL write router and the screen builder) is tested on the host. The probes are in the DOS runtime and `FIZZ2.EXE /PROBE` runs them, prints what they found and the hash of the screen those values make, and `tools/check-dos.sh` runs it under 21 DOSBox configurations. The `/CHECK` screen itself, the settings file reader and the sound flags in the program arrive in later changes, so nothing here is visible to a player beyond `/PROBE`, which the manual describes.

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
- The screen in DOSBox: `/PROBE` hashes the cells and does not write them to video memory. Reading them back from video memory arrives with `/CHECK`.
