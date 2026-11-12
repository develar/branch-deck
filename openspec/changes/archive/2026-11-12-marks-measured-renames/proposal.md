# Proposal

## Why

The marks of the diffs were measured on real commits (6,316 of a monorepo of Kotlin, Java and C#, 1,500 of Branch
Deck in Rust and TypeScript) before the runs of a renamed name were made quiet, as the plan of the marks asks. A
sample of the runs their proof allowed was read by hand, and it held changes of behaviour: lines of a variable that
another one had taken the place of. `val subscription = subscriptions[id]` had become `val channel =
subscriptions[id]?.selectedChannel`, and `if (channel != null)` read as "renamed only", the same tokens with another
question. And half of the local renames the places alone made (189 of 374 on 2,111 commits) had no line at all
that changed in the name alone (`val isRoot = …` gone, `val nameComponent = …` in its place).

## What Changes

- A name bound inside a body is a rename only where a line changed in it alone. Two bindings with as many names at
  the same places, every line of which changed otherwise too, are one variable in the place of another: nothing is
  marked.
- A rename of a local is proven only where nothing but quiet lines changed from its first place to its last: runs
  of formatting, runs of proven renames, blank lines. So the code that declares the name and reads it is the same
  code under another name. A declaration that changed otherwise, on its line or on a later one, a use on a line with
  another change and a place that makes more of the name leave the runs of the rename labels.
- The kind `renamed` is switched on: with the rule above, 145 proven runs of real commits were read and none
  changed behaviour. The local of the `commit_marks` template is three quiet runs, and `Ledger.kt` has five changes
  left to read.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Renames.

## Impact

- `crates/file-diffs/src/marks_renames.rs` (`Renames::keep_the_named`, `Renames::prove`), `marks.rs` (`is_quiet`,
  the blocks read again without a name that is no rename), `service.rs` (`MODEL_VERSION` 15).
- `tests/e2e/commit-files/renamed-names.spec.ts`, `quiet-changes.spec.ts`, `moved-lines.spec.ts`,
  `subject-marks.spec.ts`, `tests/e2e/helpers/marks-helpers.ts`: the tests read the engine; the rewrite of its
  answers that stood for the switch is gone.
- `docs/design/diff-view.md` ("Measured on real history").
