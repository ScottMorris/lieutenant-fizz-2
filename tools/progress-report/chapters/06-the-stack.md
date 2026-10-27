---
id: stack
n: 6
title: Eight pull requests, one merge
date: 2026-10-27
summary: Milestone 0 was built as a stack of eight pull requests, each based on the branch below it, and landed on main as one merge commit; the banner fix followed as its own.
prs: [1, 2, 3, 4, 5, 6, 7, 8, 10]
---
Milestone 0 was written as eight branches, each based on the one below: `feat/m0-hello-rules` on `main`, `feat/m0-hal` on it, then the DOS tooling, the DOS presenter, the DOS program, the Web presenter, the parity gate and the platform documents on top. Each branch had its own pull request, with CI on its own head. The branches were stacked with the `gh stack` extension.

The stack {{stack.numbers}} landed on {{stack.when}} as one merge commit on `main`, `{{stack.merge}}`, titled after the top pull request. Its second parent is the head of #8, and every lower head is an ancestor of it, so the merge brought all eight branches in at once: {{stack.commits}} commits and +{{stack.added}} lines. GitHub records the eight pull requests as merged within the same few seconds and names the same merge commit for each. CI on `main` for that commit: {{stack.ci}}.

<!-- figure pr-stack -->

The sizes below count each pull request's own commits: those between the head of the pull request below it and its own head. #3, the DOS target, runtime and packaging tools, is the largest at +{{pr3.added}} lines.

<!-- figure pr-sizes -->

Before the merge, CI ran on each head. The `check` job ran on all eight; the `parity` job exists from #7 up, so #7 and #8 ran both. #10, the banner fix that matches the README's Ben to his Fizz 1 pogo sprites, was based on `main` and merged later as its own merge commit, `{{pr10.merge}}`.

<!-- figure ci -->

## The shelves

### Catalogued
- All eight pull requests of the stack are merged, with every check green on each head and CI {{stack.ci}} on `{{stack.merge}}` (`gh pr list`, `gh run list --branch main`).
- The merge commit `gh` names for each of #1 to #8 is the commit where `git` first finds its head on `main`; the generator warns when they differ.
- #10 is merged with CI {{pr10.ci}} on `{{pr10.merge}}`.

### Uncatalogued
- The tool and commands that merged the stack: the repository records the result, one merge commit and eight merged pull requests, and not how it was produced.

### Wish list
- The same per-pull-request evidence for later stacks, which the generator draws as they land.

> **From the stacks — Moving a stack.** Since Git 2.38, `git rebase --update-refs` moves every branch that points into the commits being rebased, so a whole stack of branches follows when the bottom one is rebased. The `gh stack` extension manages the matching chain of pull requests on GitHub.
> Source: [git-rebase documentation](https://git-scm.com/docs/git-rebase)

```figures
id: pr-stack
kind: diagram
track: shared
title: The Milestone 0 stack and its merge
alt: Diagram of eight stacked pull requests, number 8 on top and number 1 at the bottom, each with its branch, commit count and lines added, landing on main through one merge commit.
caption: Eight branches, one on another, and the merge commit that brought them in.
at: figure pr-stack
thumb: svg

id: pr-sizes
kind: chart
track: shared
title: Lines added per pull request
alt: Bar chart of the lines each merged pull request added with its own commits; the DOS tooling pull request is the largest.
caption: The size of each pull request's own work.
at: figure pr-sizes
thumb: svg

id: ci
kind: chart
track: shared
title: CI on each pull request and on main
alt: Table chart of every merged pull request with the result of its checks on the pull request head and of CI on the merge commit on main, all green.
caption: Checks on each head, and CI on the merge commit.
at: figure ci
thumb: svg
```
