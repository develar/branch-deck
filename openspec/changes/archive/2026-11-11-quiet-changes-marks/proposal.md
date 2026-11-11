# Proposal

## Why

A reviewer reads every changed block of a diff, also the one that only wrapped a call over three lines or indented
forty lines inside a new `if`. Such a block can be passed only when it provably is no change of the program, and a
view can say so only when the diff says which lines are which. The diff had no place for it: a block was a change or,
under a whitespace policy, a pale change equal without its whitespace, which a space inside a string is too.

## What Changes

- A text diff has marks (`TextDiff.marks`): the runs of lines that changed in one way alone, the moved lines, the
  renamed names (both empty until their stages) and `starts`, the line the reading of each block starts at. The marks
  change no block. `starts` is the one source of where a reader stops: a block without an entry has nothing to read.
- The file-diffs service marks two kinds of runs from the tokens of the two sides
  ([Equal by Tokens](../../specs/commit/equal-by-tokens/spec.md)): lines with the same tokens are a run of formatting,
  quiet; lines with the same code and other comments are a run of comments, a label. A block of several changes is
  cut at the line boundaries both sides share.
- Never marked: a block with a side of no lines, blank lines alone, a block the policy cut off, a pair whose sides
  are not read by one grammar, a text without tokens, a side over the size the inner ranges are compared for. A file
  without a grammar keeps its pale block of whitespace as a stop.
- The marks come with the content of a file. A block is asked a question of its characters before the tokens are
  read, so a file whose blocks cannot hold a run is not parsed; the tokens of a blob are kept per grammar.
- The limits are by work (the size of a side, tokens, edits), never by time: a diff the time cut short has the
  marks of its blocks.
- The E2E template `commit_marks` holds a case of every kind of mark, for the views of the later stages.

## Capabilities

### New Capabilities

- `commit/quiet-changes`: the marks of a diff the Rust side decides: the runs of formatting and of comments, what
  is never marked, the starts, the limits.

### Modified Capabilities

None (the key of a diff by its grammars is the change `diff-key-by-grammar` of the same commit).

## Impact

- `crates/text-diff/src/model.rs` (`DiffMarks`, `LineRun`, `RunKind`, `LineMove`, `Rename`, `starts_of`),
  `crates/text-diff/src/lcs/diff.rs` (`build_changes_ints_within`).
- `crates/file-diffs/src/marks.rs`, `marks_runs.rs` (new), `service.rs`, `benches/marks_bench.rs`.
- `crates/test-utils/src/repo_template.rs` (`commit_marks`), `crates/test-server/`.
- `src/lib/bindings.ts` and the `TextDiff` literals of the page.
- `docs/design/diff-view.md` ("Marks"), `docs/design/text-diff.md`, `docs/design/git-operations.md`.
