#!/bin/bash
# Chooses which groups of DOS checks a pull request needs, from the paths it changes.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: tools/ci-scope.sh [--base <ref>]   the paths changed since the merge base with <ref> (default origin/main)
#        tools/ci-scope.sh --files          the paths come from standard input, one per line
#        tools/ci-scope.sh --self-test      check the table below (one test per rule) and exit
#
# Standard output is one line for tools/check-dos.sh --groups: a comma list such as `base,audio,menu`, or `all` for the full gate.
# Standard error says why, one line per changed path with the rule that matched it. `base` is always in the answer.
#
# How it decides: each path takes the FIRST rule of RULES whose pattern matches it (a shell glob where `*` also matches `/`), and the
# answer is the union of the groups of those rules. A path that no rule matches selects `all`: a file the table has never heard of may
# change anything, so the safe answer is the full gate. A rule may name `all` itself (build scripts, the workflow, the lock file, the
# golden transcript). No changed paths at all selects `base`.
#
# The groups are the ones in tools/check-dos.sh (--list). Adding a crate or a DOS source file means adding a rule here; until then its
# changes run the full gate.
set -euo pipefail
cd "$(dirname "$0")/.."

ALL_GROUPS="base input view audio config-sound menu machine pack"

# pattern ; groups ; example path (the self-test checks each rule against its example, so an example must reach its own rule first)
RULES=(
  # Documentation: it cannot change what the programs do.
  '*.md;base;docs/PLATFORM.md'
  'docs/*;base;docs/diagram.svg'
  'tools/progress-report/*;base;tools/progress-report/gather.py'
  'tools/docs/*;base;tools/docs/positional-sound.py'
  'LICENSE-*;base;LICENSE-MIT'
  'NOTICE;base;NOTICE'
  'assets/hero.svg;base;assets/hero.svg'
  '.gitignore;base;.gitignore'
  # Scripts and files that decide what runs or how it is built: the full gate.
  '.github/*;all;.github/workflows/ci.yml'
  'Cargo.lock;all;Cargo.lock'
  'Cargo.toml;all;Cargo.toml'
  'rust-toolchain.toml;all;rust-toolchain.toml'
  'tools/check-dos.sh;all;tools/check-dos.sh'
  'tools/dosrun.sh;all;tools/dosrun.sh'
  'tools/build-dos.sh;all;tools/build-dos.sh'
  'tools/setup-dos.sh;all;tools/setup-dos.sh'
  'tools/check-parity.sh;all;tools/check-parity.sh'
  'tools/ci-scope.sh;all;tools/ci-scope.sh'
  'tools/elf2go32.py;all;tools/elf2go32.py'
  'crates/fizz-ep2/slice-replay.txt;all;crates/fizz-ep2/slice-replay.txt'
  # Host-only checks and the Web presenter: the host checks and Web parity (both in every run) are what they can break.
  'tools/check.sh;base;tools/check.sh'
  'tools/check-headers.sh;base;tools/check-headers.sh'
  'tools/check-docs.py;base;tools/check-docs.py'
  'tools/check-web-*;base;tools/check-web-hud.py'
  'tools/build-web.sh;base;tools/build-web.sh'
  'tools/web-replay.mjs;base;tools/web-replay.mjs'
  'crates/fizz-present-web/*;base;crates/fizz-present-web/src/lib.rs'
  'web/*;base;web/sound.ts'
  'package.json;base;package.json'
  'bun.lock;base;bun.lock'
  # The DOS programs. The runtime's audio and keyboard files have a group of their own; the rest of the runtime serves every group.
  'dos/rtsmoke/*;base;dos/rtsmoke/src/main.rs'
  'dos/rt/src/audio.rs;audio;dos/rt/src/audio.rs'
  'dos/rt/src/kbd.rs;input;dos/rt/src/kbd.rs'
  'dos/rt/src/vga.rs;view,machine;dos/rt/src/vga.rs'
  'dos/rt/*;all;dos/rt/src/pit.rs'
  'dos/fizz2/src/cfgfile.rs;config-sound,machine;dos/fizz2/src/cfgfile.rs'
  'dos/fizz2/src/sound.rs;audio,config-sound;dos/fizz2/src/sound.rs'
  'dos/fizz2/src/replan.rs;audio,menu;dos/fizz2/src/replan.rs'
  'dos/fizz2/src/menu.rs;menu;dos/fizz2/src/menu.rs'
  'dos/fizz2/src/frames.rs;view;dos/fizz2/src/frames.rs'
  'dos/fizz2/src/bench.rs;view,audio;dos/fizz2/src/bench.rs'
  'dos/fizz2/src/hal.rs;input,menu;dos/fizz2/src/hal.rs'
  'dos/fizz2/src/check.rs;machine;dos/fizz2/src/check.rs'
  'dos/fizz2/src/machine.rs;machine,config-sound;dos/fizz2/src/machine.rs'
  'dos/*;all;dos/fizz2/src/main.rs'
  # Shared crates and assets, by what they feed.
  'crates/fizz-sfx/*;audio;crates/fizz-sfx/src/lib.rs'
  'crates/fizz-music/*;audio;crates/fizz-music/src/lib.rs'
  'crates/fizz-pack/*;audio,pack;crates/fizz-pack/src/lib.rs'
  'assets/sfx/*;audio;assets/sfx/clucks.json'
  'assets/music/*;audio;assets/music/crater.ts'
  'tools/sfx/*;audio;tools/sfx/downmix.ts'
  'tools/music/*;audio;tools/music/generate.ts'
  'crates/fizz-machine/*;config-sound,machine;crates/fizz-machine/src/switches.rs'
  'crates/fizz-menu/*;menu;crates/fizz-menu/src/lib.rs'
  'crates/fizz-present-dos/src/menu.rs;menu;crates/fizz-present-dos/src/menu.rs'
  'crates/fizz-present-dos/*;view;crates/fizz-present-dos/src/hud.rs'
  'crates/fizz-view/*;view;crates/fizz-view/src/lib.rs'
  'crates/fizz-slice-view/*;view;crates/fizz-slice-view/src/lib.rs'
  'crates/fizz-hal/*;input,menu;crates/fizz-hal/src/lib.rs'
  'crates/fizz-ep2/src/world.rs;view,audio;crates/fizz-ep2/src/world.rs'
  'crates/fizz-ep2/src/replay.rs;view,audio;crates/fizz-ep2/src/replay.rs'
  'crates/fizz-ep2/src/rules.rs;view,audio;crates/fizz-ep2/src/rules.rs'
  'crates/fizz-ep2/src/cues.rs;view,audio;crates/fizz-ep2/src/cues.rs'
  'crates/fizz-ep2/*;view;crates/fizz-ep2/src/ben.rs'
  'crates/fizz-phys/*;view;crates/fizz-phys/src/lib.rs'
  'crates/fizz-sim/*;view;crates/fizz-sim/src/lib.rs'
  'tools/sprites/*;view;tools/sprites/emit-rust.ts'
  'assets/sprites/*;view;assets/sprites/ben.txt'
  'tools/font/*;view;tools/font/export-fizz1.ts'
)

# classify: paths on standard input; the answer on standard output, the reasons on standard error.
classify() {
  local f rule pat groups matched any=0 all=0 sel=" base " g
  while IFS= read -r f; do
    [ -n "$f" ] || continue
    any=1
    matched=
    for rule in "${RULES[@]}"; do
      pat="${rule%%;*}"
      groups="${rule#*;}"; groups="${groups%%;*}"
      # shellcheck disable=SC2254
      case "$f" in
        $pat) matched=1; break ;;
      esac
    done
    if [ -z "$matched" ]; then
      echo "ci-scope: $f -> all (no rule matches this path)" >&2
      all=1
      continue
    fi
    echo "ci-scope: $f -> $groups (rule '$pat')" >&2
    if [ "$groups" = all ]; then
      all=1
    else
      for g in ${groups//,/ }; do sel="$sel$g "; done
    fi
  done
  [ "$any" = 1 ] || echo "ci-scope: no changed paths -> base" >&2
  if [ "$all" = 1 ]; then
    echo all
    return
  fi
  local out=
  for g in $ALL_GROUPS; do
    case "$sel" in *" $g "*) out="$out${out:+,}$g" ;; esac
  done
  echo "$out"
}

fail() { echo "ci-scope: FAIL: $*" >&2; exit 1; }

self_test() {
  local rule pat groups example want got n=0 bad=0 files
  # One test per rule: its example path, alone, gives exactly the rule's groups (with base).
  for rule in "${RULES[@]}"; do
    pat="${rule%%;*}"
    groups="${rule#*;}"; example="${groups#*;}"; groups="${groups%%;*}"
    if [ "$groups" = all ]; then want=all; else want="$(ALL_GROUPS="$ALL_GROUPS" order "base,$groups")"; fi
    got="$(printf '%s\n' "$example" | classify 2> /dev/null)"
    if [ "$got" != "$want" ]; then
      echo "ci-scope: rule '$pat': example $example gave '$got', expected '$want'" >&2
      bad=1
    fi
    n=$((n + 1))
  done
  # Every group a rule names exists.
  for rule in "${RULES[@]}"; do
    groups="${rule#*;}"; groups="${groups%%;*}"
    for g in ${groups//,/ }; do
      case " all $ALL_GROUPS " in *" $g "*) ;; *) echo "ci-scope: rule '${rule%%;*}' names the unknown group '$g'" >&2; bad=1 ;; esac
    done
  done
  # The groups are the ones check-dos.sh runs.
  if [ -x tools/check-dos.sh ]; then
    got="$(tools/check-dos.sh --list | awk '{printf "%s%s", (NR > 1 ? " " : ""), $1}')"
    [ "$got" = "$ALL_GROUPS" ] || { echo "ci-scope: tools/check-dos.sh --list is '$got', not '$ALL_GROUPS'" >&2; bad=1; }
  fi
  # The combinations the rules exist for.
  expect() {
    local want="$1" got
    shift
    got="$(printf '%s\n' "$@" | classify 2> /dev/null)"
    [ "$got" = "$want" ] || { echo "ci-scope: [$*] gave '$got', expected '$want'" >&2; bad=1; }
    n=$((n + 1))
  }
  expect base                                  # nothing changed
  expect base docs/PLATFORM.md README.md tools/progress-report/chapters/05-parity.md
  expect all some/new/file.rs                  # unknown path: the full gate
  expect all docs/PLATFORM.md some/new/file.rs # one unknown path among docs is enough
  expect base,audio crates/fizz-sfx/src/lib.rs assets/sfx/clucks.json README.md
  expect base,view,audio crates/fizz-ep2/src/world.rs
  expect base,view crates/fizz-ep2/src/ben.rs crates/fizz-sim/src/lib.rs
  expect base,input,view,menu crates/fizz-hal/src/lib.rs crates/fizz-view/src/lib.rs
  expect base,view crates/fizz-present-dos/src/hud.rs
  expect base,menu crates/fizz-present-dos/src/menu.rs
  expect all dos/rt/src/pit.rs docs/PLATFORM.md
  expect all .github/workflows/ci.yml
  expect all tools/check-dos.sh
  # --files reads standard input, so a path with a space stays one path.
  expect all 'docs/a b.md' 'x y'
  [ "$bad" = 0 ] || fail "the self-test found problems (above)"
  echo "ci-scope: self-test ok ($n tests)"
}

# order <comma list>: the list in the canonical group order, without repeats.
order() {
  local out= g
  for g in $ALL_GROUPS; do
    case ",$1," in *",$g,"*) out="$out${out:+,}$g" ;; esac
  done
  echo "$out"
}

case "${1:-}" in
  --self-test) self_test ;;
  --files) classify ;;
  --base) [ $# -ge 2 ] || fail "--base needs a ref"; git diff --name-only --no-renames "$2...HEAD" | classify ;;
  "") git diff --name-only --no-renames "origin/main...HEAD" | classify ;;
  *) fail "unknown argument $1 (see the usage at the top of the script)" ;;
esac
