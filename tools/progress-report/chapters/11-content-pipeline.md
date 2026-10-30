---
id: pipeline
n: 11
title: The start of the content pipeline
date: 2026-10-30
summary: Milestone 2 begins with the formats: the Fizz Asset Pack (`.FAP`) specification, the `fizz-pack` reader with its deflate decoder, the OPX register-stream format that audio sections hold, and the CI change that lets a pull request run only the DOS checks its paths can affect. The baker and the loaders are not built.
prs: [109, 111, 112, 113, 119, 123]
---
Milestone 2 is the content pipeline: the host tool `fizz-bake` turns authored assets into packs, and the Web and DOS builds read them, so that DOS parses nothing at start-up that the baker could prepare. What has landed so far is the format, its reader and a stream format for audio, with the CI work that made the checks cheaper to run. No game code loads a pack yet and `fizz-bake` does not exist.

## The Fizz Asset Pack (#109)

`docs/FAP.md` specifies the pack, and it is the source of truth for the format: the reader follows it, and where the two differ the crate is wrong. A pack starts with a 32-byte header (a magic, a format version, a target, a compatibility number, the file's length, a checksum and the file's stem), then a directory of 24-byte entries, then sections that start on 16-byte boundaries. The stem in the header lets a reader tell that one file was renamed over another.

- **Sections.** `LEVL` is one level, `FXPK` the effects of one device, `OPL3` and `OPL2` one stream for one song, and `MUSC`, `SPRT` and `BDRP` are outlined and not yet specified.
- **Deflate from day one.** Each section has a codec byte: stored, or raw deflate baked at level 9. Compression is part of version 1, so no pack format has to change when the music grows. A small section is stored when deflate does not make it smaller. Nine early-experiment songs for both chips, 1,274,402 bytes raw, came to 147,136 bytes (11.5%) in the experiment that chose it, measured with an earlier layout and to be measured again on OPX.
- **Per-device audio.** A pack holds one audio section for each chip, so a DOS machine decodes only the one that fits its card and the speaker plays effects only: the spec has no speaker music.
- **The hybrid layout.** `SHARED.FAP` holds Ben, the HUD, the effects and the song table; each world has a `<WORLD>.FAP` of levels, backdrops and sprites; `MUSIC.OP3` and `MUSIC.OP2` hold one section per song for each chip. The names follow the DOS 8.3 rule, and moving a section between files changes the baker's manifest and not the format.
- **The compatibility number.** The header's number comes from the pack's schema and not from the build: an FNV-1a hash of the layouts, tags, codecs and object kinds a reader must understand, continued for a world or shared pack over the game's tile properties and ids. A commit that changes no layout leaves every pack valid. Music files take a number of their own, so a level change does not invalidate them.
- **Limits.** A decoded section is at most 96 KB by default, and the stated machine floor is a 4 MB computer with about 2 MB free, against the 1 MB Fizz 1 needed.

The spec also lays out the baker, a proof that a baked level equals the `const` level, and the packaging on a Fizz 1 floppy; those parts describe a plan with measured inputs and not code.

## The fizz-pack reader (#111, #112)

`crates/fizz-pack` reads a `.FAP` from caller buffers. It is `no_std`, has no allocator and forbids `unsafe`. It checks the header, the directory and the checksums, decodes raw deflate into a buffer of the entry's decoded length, refuses each malformed case with its own error, and gives a `LevelView` over a decoded level section. The decoder is about 2.3 KB of machine code at opt-level `z` and 2.9 KB at `s`, measured on an i586 build, which is more than the 1 KB first guessed because Rust adds bounds and overflow checks. Its tests pin the schema hashes, so a change to the schema is a deliberate test edit. The second pull request (#112) reconciled the spec with the reader and applied the owner's decisions: the file names, the DOS search order, the stem in the header, tile property flags in the compatibility number, a separate number for music, and no speaker music.

## OPX (#113)

`docs/OPX.md` specifies OPX as an open, standalone format for a stream of timed OPL2 or OPL3 register writes, or of PC speaker divisors. It takes IMF's write-then-wait simplicity and VGM's structure, and puts both on a declared tick grid.

- **Grid and header.** The header's `grid_hz` says how many whole ticks make a second, and every wait is a whole number of ticks. The game's OPL music and effects use 560 Hz and speaker effects 140 Hz. A 32-byte header carries the chip, the grid, the total ticks and a loop point.
- **Opcodes.** A write is three bytes and a short wait one; a long wait is wider. A player advances by commands and a tick counter, in place, without allocating, so a 560 Hz interrupt handler can run it.
- **Text twin.** A line-based form of every file, for diffs, tests and hand-written examples, round-trips to the same bytes for a file in canonical form.
- **Profiles.** A profile is a set of extra rules for one use. The Fizz profile fixes the grids, reserves channels 5 and 14 for effects, and describes the idea of one OPL3 song whose bank 0 is a complete arrangement and whose extras sit in bank 1.
- **Importers.** IMF, DRO and VGM map onto OPX exactly at their own rates; moving a stream onto another grid is a separate, reported step.

The status is a draft, version 0.1, experimental. The engine's `OplStep` record exists and plays today, but the format, the text twin and the planned `oplstream` crate (a header, a `no_std` stepper and validator, a writer and the importers) are proposals, and nothing reads or writes an OPX file yet. Adopting it for the effects and the music is Milestone 4.5 on the [roadmap](chapter:roadmap).

## The CI change (#123, #119)

The full DOS gate, `tools/check-dos.sh`, took about 18 minutes locally, 13 of them in 27 replays of 28 seconds each. #123 split its cases into named groups (`base` always, then `input`, `view`, `audio`, `config-sound`, `menu`, `machine` and a reserved `pack`), added `tools/ci-scope.sh`, which reads the paths a pull request changes and names the groups they can affect, and lets independent replays run in parallel (`CHECK_DOS_JOBS`). A pull request runs its groups and a slim parity check; a push to `main`, the nightly schedule at 05:17 UTC and a manual run run everything. Measured locally, the full gate went from about 18 minutes to about 8, and the `base` group runs in about 45 seconds. #119 gave the DOS runtime's `memset` and `memcpy` the C signatures the compiler expects, which cleared two suspicious-symbol warnings from the build log.

On GitHub's runners, `gh run view` gives these durations for 30 October 2026. The full gate on `main` ran 587 seconds for the OPX merge, against about 1,160 to 1,200 seconds for the runs before the package. Pull request runs took 126 seconds for a Web-only change, 572 to 590 seconds for most others and 783 seconds for the soda can. The first full run on `main` after the soda can merged (#127, `e46fbe6`) failed at 426 seconds: the parallel replay of a corrupt `FIZZ2.CFG` printed `did not restore the vectors`. A full local run of the same tree passed with two replays at a time, and no repeat run on `main` or nightly run had finished when this edition was built.

## What comes next

The planned order is the level source refactor (separating the slice's level text from the Rust that bakes it), the level script dialect, `fizz-bake` itself, the pack loaders on both platforms, then the audio and sprite sections and the tools for floppy images. The roadmap keeps `bake` as planned.

## The shelves

### Catalogued
- `docs/FAP.md` (#109), the `fizz-pack` reader (#111, #112) and `docs/OPX.md` (#113) are merged into `main`: #109 landed on `{{pr109.merge}}` with CI {{pr109.ci}}, #111 on `{{pr111.merge}}` with CI {{pr111.ci}}, #112 on `{{pr112.merge}}` with CI {{pr112.ci}}, #113 on `{{pr113.merge}}` with CI {{pr113.ci}}.
- The reader's behaviour is `cargo test -p fizz-pack`, which `tools/check.sh` runs through `cargo test --workspace`: the header, directory and checksum checks, each error, the decoder and the pinned schema hashes.
- The check groups are `tools/check-dos.sh --list`, and `tools/ci-scope.sh` picks them from paths; the full gate printed `DOS CHECK ok` on this edition's tree, with `PARITY ok {{ms.parityHash}}`.

### Uncatalogued
- The decoder's size of 2.3 to 2.9 KB: measured once on an i586 build and recorded in `docs/FAP.md`; no check holds it.
- The compression figures (11.5% of 1,274,402 bytes) come from an earlier layout and the planning notes, and are to be measured again on OPX once an encoder exists.
- The CI durations above are single runs on GitHub's runners: the full gate's repeatability, and the failure on `main` for #127, are not understood beyond the log line.
- The pack on a real DOS machine, the 4 MB floor and the 96 KB limit on real memory: the free-memory figures come from a DOSBox `memsize` ladder.
- OPX has no reader or writer, so its examples, opcodes and text twin have not been run.

### Wish list
- `fizz-bake`, the level script dialect, the pack loaders, the audio and sprite sections and the floppy image tools, in the order above.
- The `oplstream` crate and OPX in the effects and the music (Milestone 4.5).

> **From the workbench — DEFLATE.** Deflate combines LZ77, which replaces a repeat with a pointer to earlier bytes, with Huffman codes, which give frequent symbols short bit strings. Phil Katz designed it for PKZIP, and RFC 1951 defines the format that zip, gzip and PNG share. A decoder needs only the output buffer as its window, which is how `fizz-pack` reads it without a separate 32 KB history.
> Source: [RFC 1951, DEFLATE Compressed Data Format Specification](https://www.rfc-editor.org/rfc/rfc1951)
