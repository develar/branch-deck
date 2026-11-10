# Proposal

## Why

The resolve by words took IntelliJ's simple resolve as it is: under the default policy first, then ignoring
whitespace. That second step takes line ends as whitespace too, so it joins lines into one that no side has, and a
sync commits the result unread. Of 35,610 resolves of 1M random merges, 24,402 took the second step and 3,346 wrote
such a line; the default policy wrote none. On real history (4,000 commits of a large monorepo, each copied without
the latest earlier commit of its files: 2,365 files that conflict in git), the default policy resolves 383 files and
the second step 16 more, 4 of them by code lines joined into one (`releaseOnFailure = true;try {`,
`composite.showPreparationProgress = falsecomposite.initDeferred.complete(Unit)`).

The merge also took a last line without an end for the same line with one, as IntelliJ takes line ends as equal: a
final line end removed on one side beside a change of that line, or of the lines after it, on the other merged without
the conflict that git sees. Of 30,000 random merges, 62 merged so where git conflicts.

## What Changes

- The resolve of `merge_texts`, which the conflict view proposes and a sync commits, takes the default policy alone
  (`merge_resolve_util::try_resolve_by_default`); the ported `try_resolve` and its tests stay.
- The merge pairs the last line of each side only with the last line of the base of its kind: an empty last line with
  an empty last line, a line without an end with an equal line without an end (`last_line::with_last_lines`). The
  views of a two-way diff keep their presentation of a change of the final line end.
- `RESOLVE_VERSION` 4 and `MERGE_MODEL_VERSION` 5; a copy of an older resolve (versions 1 to 3, all with the second
  step) is checked once by its tree.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-row`: Conflict view: the merge sees the conflicts of git at the end of a file, also of a last line
  without an end; a conflict that only ignoring whitespace resolves stays with the user.
- `cli/agent-cli`: `brd sync`: the words resolve under the default policy alone.

## Impact

- `crates/text-diff/src/merge_resolve_util.rs`, `merge_model.rs`, `by_line.rs`, `last_line.rs`;
  `crates/git-ops/src/word_resolve.rs` (`RESOLVE_VERSION`), `crates/file-diffs/src/merge.rs` (`MERGE_MODEL_VERSION`);
  `docs/design/text-diff.md` (Key Decisions 8 and 9), `docs/design/git-operations.md` (Resolve by words).
