# AGENTS.md

## Project Status

Lieutenant Fizz 2 is the start of Lieutenant Fizz — Episode 2, built so Episode 3 can reuse the same simulation. It is one game with two first-class presentations, Web and DOS, developed in parallel. Fizz 1 (`liminal-hq/lieutenant-fizz`) is the original: a working multiplatform prototype that Fizz 2 learns from and does not change. Desk (`liminal-hq/desk`) is the Rust-on-DOS foundation; its ideas are mirrored here for now and swapped for the real crates later.

The repository was seeded from the findings of an earlier proof of concept, `fizz-2-dos-poc1` (tag `poc-1`). Those findings are recorded in `docs/POC-1-FINDINGS.md`. No proof-of-concept code is carried over except by deliberate, documented decision.

## Source of Truth

- `docs/ARCHITECTURE.md` defines the layers, the crate plan and the Web and DOS parity policy.
- `README.md` defines the repository entry point and what currently works.
- Code is the source of truth once it exists. Update the corresponding document in the same branch when a behaviour or architecture decision changes.
- Fizz 1 and Desk are references for proven techniques, not code to copy without a documented decision. When Fizz 2 reproduces a Fizz 1 behaviour, it names the Fizz 1 source and proves the match with a test.

## Localization and Markdown

All UI strings, identifiers, comments, commit messages, pull request descriptions and documentation use Canadian English except exact external spellings. Use `colour`, `centre`, `behaviour`, `cancelled`, and `licence` (noun). Do not hard-wrap markdown prose; each paragraph and bullet is one source line. Use a real em dash in prose, never `--` as a substitute.

## Authoring Voice

Write code comments, documentation and pull request prose as the author of the artefact, for a later reader. State facts and reasoning directly; do not mention reviews, reviewers, conversation history or commit hashes. Commit messages and PR test plans may record debugging context and measurements.

## Git Workflow

Do not push, force-push, merge or alter shared settings without explicit user instruction. Use merge commits only; never squash or rebase-merge. Name branches `feat/<short-description>`, `docs/<short-description>`, `chore/<short-description>`, or `fix/issue-<number>-<short-description>`. Stage exact paths with `git add <path>`; never use `git add -A` or `git commit -a`. Do not commit build products, `target/`, `out/`, `dist/`, or `vendor/`.

Use Conventional Commits. The repository's first commit is the sole exception, titled `Initial Commit`. Commit bodies explain what and why, hard-wrap at about 72–100 columns, backtick every code-level reference, and end with the agent co-author trailer when an agent authored the commit. Write markdown-heavy messages to a file and use `git commit -F`; inspect the result with `git log -1 --pretty=fuller`.

PR titles start with a capital letter, use the imperative mood, remain near 70 characters, and have no Conventional Commit prefix. PRs use `## Summary` followed by `## Test plan`, have a category label (`enhancement`, `bug`, `documentation`, `testing`, `ci`, `build`, or `chore`) and relevant scope labels (`sim`, `web`, `dos`, `graphics`, `input`, `sound`, `levels`, `rust`, `performance`, `experimental`, `ui`, `design-docs`, `pages`, `deploy`).

## Local Tooling and Releases

Rust is pinned in `rust-toolchain.toml`; DOS builds use `RUSTC_BOOTSTRAP=1` only for the documented `-Zbuild-std` and target-specification steps. The scripts are the source of truth for exact commands. Do not install system packages to get a build working without asking. Use `$TMPDIR`-style scratch paths when jobs share a machine. The CI image for DOS work is the `ci-dos` tier of `ghcr.io/liminal-hq/ci-dos`, pinned by digest; tool-version changes belong in the shared image repository.

Releases add their version section to `CHANGELOG.md` in the release PR. Tags are pushed only when explicitly requested, after the release PR merges. Release notes group merged PRs by their labels, so labels stay accurate; `skip-changelog` and `internal` deliberately omit work from those notes.

## Web and DOS Parity

- Every milestone ships on both presentations. A feature is not done until it has a Web path and a DOS path. The DOS path may be reduced, such as fewer effects, flatter lighting or simpler audio, but it must run.
- Web leads on feel and fidelity; DOS leads on budgets. The 486DX is the floor for DOS, and DOS is a bonus platform: it may cut content or features, and it never limits the game's design.
- The state hash from `fizz-sim` is the arbiter. A change that makes the native, Web and DOS hashes disagree is a bug until proven otherwise.
- Tools that only the Web side needs (editors, previews) may be `std`. Nothing in the rules layer may depend on a presenter.

## Code and Runtime Rules

- The rules layer (`fizz-sim` and the game logic built on it) is `no_std` plus `alloc`, forbids `unsafe`, and has no clocks, randomness or platform types. A frame is a function of the steps that led to it. Hosts, presenters, cookers and tools may use `std`.
- Fizz 1's numeric model is the reference: floating point under the determinism discipline. Use basic IEEE operations only, never reorder floating-point operations for speed, use a software `libm` for transcendental functions, and set x87 to 53-bit precision on DOS before the first tick. Document any departure.
- The simulation emits intent: a scene of tiles, sprites, lights and a camera, a composition source, and named events. Platform limits (palette depth, OPL patches, voice counts, capacity constants) live in the presenter or backend that needs them, not in the shared contract.
- Fidelity is lost at cook time, not at run time. Offline host tools produce per-target assets. DOS parses nothing at start-up that a tool could have prepared.
- The portable layers never use DOS, DPMI, BIOS, port I/O, interrupt vectors or raw video-memory pointers. The DOS platform layer mirrors Desk's hardware abstraction layer; platform boundaries isolate audited `unsafe` with a one-line `SAFETY:` justification.
- Interrupt handlers do bounded work only, allocate nothing, and hand input or completed work to the game loop through a bounded single-producer/single-consumer queue. Every vector is restored on exit and panic.
- File names remain 8.3 on the DOS side until a documented compatibility decision changes that rule.
- No speculative scaffolding: a crate, trait or driver arrives with the consumer and test that need it. A piece is not done until a second consumer exercises it.

## Testing

Run `tools/check.sh` before a PR. Add a test that fails before a bug fix. Keep the simulation deterministic and prove it with state hashes and golden input traces, and prove any Fizz 1 behaviour match with a trace test against the Fizz 1 source. Test drivers against host-side models before DOSBox probes. State exactly what ran and what did not; no result implies real-hardware validation, and audio cannot be verified headlessly.

## Documentation

Update `docs/ARCHITECTURE.md` when layers, crates or the parity policy change and `README.md` when build or demonstration use changes. Prefer stable reference documents over undocumented decisions in comments or pull requests.

## Licence and Copyright

The project is dual-licensed under Apache-2.0 OR MIT. New or substantially rewritten `.rs`, `.c`, `.h`, `.ts`, `.js`, `.py`, `.sh` and workflow YAML files begin with a summary, copyright and SPDX header:

```text
// Brief one-line summary of what this file does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
```

Use `#` for shell, Python and YAML, and `/* */` for C. Do not add headers to markdown, JSON, lockfiles or generated/config-only files.

## Agent Behaviour

Read the relevant document before implementing. Make the requested change without unrelated refactors. State uncertainty and unverified items plainly. Stay within assigned files when several agents share the checkout.
