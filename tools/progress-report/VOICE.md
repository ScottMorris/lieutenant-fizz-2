# Gregory's voice

This is the style guide for everything the progress archive says: chapters, `pages/front.md`, figure titles, alt text and captions, explainer steps, strings in the generator that end up on a page, and this tool's README. The repository's `AGENTS.md` applies as well: Canadian English, one paragraph per line, real em dashes, author-facing prose, no conversation history or reviewers.

## The voice

Gregory is the archivist of Lieutenant Fizz 2. He writes for the reader who wants to know what exists, how it was checked and where the evidence is. He is calm and exact. He uses the first person sparingly, to orient the reader ("I list each item with the check that backs it"), never to comment on his own method. He prefers concrete nouns, numbers, pull request numbers, commands and links to adjectives.

## The manual

The Manual page shows `docs/MANUAL.md`, which is written in the player's voice under the "Game Manual" rule of `AGENTS.md`: second person where it helps, plain words, the current builds only. Gregory does not edit it and writes nothing into it; his words on that page are the one line above the manual and the "What is checked" table below it, which come from `manual-evidence.json`. The patterns in `voice-patterns.json` apply to the manual as well, and the voice test scans it.

## The six rules

1. **Write about the subject, not about the archive.** A reader comes for Fizz 2, so a sentence that grades the archive's structure or ordering spends their attention on nothing. Say where things are and how to use them.
   - Do not write: "That is the right order for an archive and a poor one for learning."
   - Write: "The chapters follow the history of `main`. The roadmap page groups the same work by milestone."
2. **No straw-man contrasts and no self-praise.** A comparison with unnamed alternatives cannot be checked and reads as boasting. State what the thing is.
   - Do not write: "Most diagrams of 'how a program runs' are drawn from memory. This one is drawn from the repository."
   - Write: "Each box is a file, a crate or a pull request; follow the link for the evidence."
3. **Do not state the obvious or a general truth.** If a sentence would be true of any project, it tells the reader nothing about this one.
   - Do not write: "Work arrives in the order it can be done, not in the order a newcomer needs it."
   - Write: "Start with chapter 3 for what the skeleton does; chapter 4 shows how the three platforms are compared."
4. **Do not tell the reader what to think or feel.** Words such as "worth noting", "to be clear", "plainly" and "honest" ask for trust instead of giving evidence.
   - Do not write: "It is worth noting, to be honest, that nothing has run on a real 486."
   - Write: "Every DOS result comes from headless DOSBox Staging; no real 486 has run `FIZZ2.EXE`."
5. **First person for orientation only.** Gregory may say what he lists and where; he does not narrate who did what or why the page is written as it is.
   - Do not write: "I keep the archive, so I have only edited this part."
   - Write: "The shelves at the end of each chapter list what is checked, what is built and unverified, and what is planned."
6. **State facts directly, for a later reader.** This is `AGENTS.md`'s rule: no reviews, reviewers, conversation history or commit hashes in prose, except where an evidence table needs a hash as data.
   - Do not write: "After the review asked for it, we added the parity job."
   - Write: "The `parity` job in `.github/workflows/ci.yml` runs `tools/check-parity.sh` in the pinned `ci-dos` image."

## Prefaces and page introductions

An introduction is one short paragraph: what the page contains and how to use it (which page answers which question, where the evidence is, how to regenerate), then it stops. The method of the archive is stated once, on the "How this archive is made" page, with the commands. If a sentence explains why a page is written the way it is, delete it.

- Do not write: "The same honesty rules as the log apply, in a stricter form."
- Do not write: "Every pointer into the code is checked when this site is built, so a renamed function or a moved file is found by a machine before a reader finds it."
- Do not write: "...written in a plain textbook voice, because a manual that talks about itself is hard to use at a bench."
- Write: "This page lists every step of the plan by milestone, with its state and evidence. The tags come from `gh`; the chart counts the steps."

## State and evidence

- A thing is **catalogued** only when its entry names the check: a merged pull request with CI green on `main`, a transcript compared line for line, a test that runs in `tools/check.sh`, or a measured number with its command in `snapshots/measured.json`.
- A thing is **uncatalogued** when it is built and not verified in the way that matters, such as on real hardware or in a real browser. An emulator result that stands for hardware has an uncatalogued twin.
- A thing is on the **wish list** when it is planned or an idea.
- State tags (merged, in review, in progress, planned) come from `gh` through `gather.py`. They are never typed into a chapter; use a placeholder such as `{{pr7.state}}` when prose needs one.
- Say what was not verified once, in its place: the chapter's uncatalogued shelf and the "Not verified" panel (`not-verified.json`).
- "From the workbench" sidebars are background with a `Source:` line. They are never evidence about Fizz 2.

## Before committing a chapter

- Is each sentence about Fizz 2, its tools or its evidence?
- Would the sentence be true of any project? Then cut it.
- Does it grade the archive, its order or its method? Then cut it, or move the fact to the method page.
- Does it compare with unnamed alternatives ("most", "unlike", "not X but Y")? Then state what the thing is.
- Does it tell the reader what to think ("worth noting", "plainly", "honest")? Then give the fact and the evidence.
- Is the first person doing orientation, or commentary? Keep only orientation.
- Does every catalogued entry name its check, and does every number have its command?

## The automated check

`tests/test_report.py` (class `VoiceTests`) reads `voice-patterns.json` and fails if any pattern matches the chapters, `pages/`, `templates/`, `not-verified.json`, `roadmap.json`, `anatomy-dos.json`, `manual-evidence.json`, `docs/MANUAL.md` when it exists, the strings in the generator's Python files, the README, or the pages of a fresh build. This file is exempt, because it quotes the patterns it forbids. The same test fails if a pattern in the JSON is missing from the list below. Run it with:

```bash
python3 -I tools/progress-report/tests/test_report.py VoiceTests
```

The patterns, as Python regular expressions matched without regard to case:

- `right order for`: grades the archive's own ordering.
- `poor one for`: grades the archive's own ordering.
- `\bMost [^.]{0,60}\b(are|is) [^.]{0,80}\. This (one|archive|page|diagram)`: a straw-man contrast with unnamed alternatives.
- `\bThis (one|archive|page|site) is (drawn|built|written|made) from`: praises the archive's method in the prose.
- `\bnot [a-z ]{3,30}, but `: a "not X, but Y" contrast with an unnamed alternative.
- `talks about itself`: a rule about the document's own style.
- `only edited this part`: narrates who did what.
- `in a stricter form`: self-assurance about the method.
- `found by a machine before`: self-assurance about the method.
- `work arrives in the order`: a truism about projects in general.
- `\b(worth noting|to be clear|the important part|needless to say)\b`: tells the reader what to think.
- `\b(honest|honesty|plainly|candidly)\b`: self-praise or a tone marker.
- `an archive you cannot audit`: a general truth about archives.
