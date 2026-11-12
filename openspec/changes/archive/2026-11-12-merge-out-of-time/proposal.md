# Proposal

## Why

The `file_merge` store kept a merge of three texts unless `line_level_only && fits_inner`: a merge whose lines ran out
of time on a text over the limits of the inner ranges (one block of the three whole texts) was stored, and every later
view of that conflict read it, under any load. The two-way diffs and the merges of one side already decide by the
deadline itself (`BudgetedDiff::out_of_time`, 2026-11-12-scan-diffs-no-deadline).

## What Changes

- `merge_texts_in` says whether a deadline ran out (`BudgetedMerge::out_of_time`): the lines, or the words of a block
  (a block that `block_of` did not complete); a `TooManyChanges` of the lines is a limit of the work and is not this.
- The views store no merge out of time (`MergeWork::merge`); `fits_inner` of `merge.rs` is gone.
- `MERGE_MODEL_VERSION` 11, so no stored merge that a time cut may have made is read again.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: a merge the time cut short reaches no store.

## Impact

- `crates/text-diff/src/merge_model.rs`, `crates/file-diffs/src/merge.rs`, `crates/file-diffs/src/stored_layout_test.rs`.
- `docs/design/git-operations.md` (the caches of the merges).
