# Proposal

## Why

The same measurement read the moves of real commits. Of 1,712 moves within a file on 2,111 commits, 641 paired
one of several equal lines with another: the same statement of each test of a file (`val ref =
getMQReferenceAtCaret()` three times), the closing lines of each target of a build file, the same records of a
report in JSON. Such a pair says nothing: any of the equal lines moved as well as any other. No number of
characters tells them from a move (they hold up to 290). And the measurement itself had no home: the counts and the
samples it reads are what decides a quiet kind and a threshold, now and after the next change of a rule.

## What Changes

- A move counts by its exact lines that stand once among the lines a move may take, on each side: only they hold
  the 24 characters. Lines that stand at two places name no move.
- The run the files of a subject add to a file takes no line of a quiet run of its pair: the lines of a renamed
  local, shown once with the rows of the file, stay quiet beside the call of a name another file declares.
- The runs the index confirmed are quiet, as their kind is now.
- `brd commits --mark-stats` (hidden) measures real commits as a view reads them: the counts of each kind, the
  stops, the times with and without the marks, a sample of each kind with its lines (`FileDiffs::measure`,
  `FileDiffs::without_marks`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the groups Moves, Limits and Subject; the new group Measurement.

## Impact

- `crates/text-diff/src/moves.rs` (the lines that stand once), `crates/file-diffs/src/marks_subject.rs`
  (`merged_runs`), `marks_measure.rs` (new), `service.rs` (`FileDiffs::collect`, `without_marks`).
- `crates/bd-rewrite/src/mark_stats.rs` (new), `commits.rs`, `crates/bd-cli/src/lib.rs`.
- `docs/design/diff-view.md` ("Measured on real history"), `docs/design/text-diff.md` ("Moves").
