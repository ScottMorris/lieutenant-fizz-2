# Changelog

## Unreleased

- Start the repository: house rules, the `fizz-sim` fixed-tick and state-hash foundation, documentation and the animated README banner.
- Add the Milestone 0 platform skeleton: a marker moved by input at a fixed 60 Hz on Web and DOS, with the same state-hash transcript on native x86_64, Web (WASM) and DOS (headless DOSBox), compared by `tools/check-parity.sh` and a `parity` CI job in the pinned `ci-dos` image. It adds `fizz-hello` (throwaway rules), `fizz-hal`, `fizz-present-dos`, `fizz-present-web`, the `dos/` workspace with its runtime and `FIZZ2.EXE`, the Web page, and `docs/PLATFORM.md`. `InputFrame` and its types now live in `fizz-sim`.
