# Proposal

## Why

The hunks of a commit (`trial_check::diff_scan`, kept per commit in the `commit_diff` store), its patch and the pairs
of a review plan came from `FileDiffs::blocks_of`, which compared under the wall-time budgets of the views (100 ms for
the inner ranges, 1 s for the lines). On a loaded machine a pair ran out: by lines alone, or one block of both whole
texts. Those blocks then went into the `commit_diff` store, and every later scan read them, while the design promised
that a diff the time cut short reaches no store. A scan could also take such a diff from the memory of the views. The
`file_diff` store kept a diff whose lines ran out when a side was over the size limits of the inner ranges, since the
rule told a cut by `line_level_only` within those limits alone.

Measured on 191,924 modified pairs of 20,000 commits of the IntelliJ monorepo (`examples/scan_limits.rs`, load ~20):
the budgets of the views cut 17 pairs short, 14 of them with other blocks; without a deadline the comparisons took
198 s on all threads against 168 s (wall 11.1 s against 9.4 s), the longest pair 2.2 s (a 4.6 MB SARIF file, over the
inner limits), 69 pairs over 100 ms (the ~1 MB `composer_packages.ndjson` dictionaries, 1.3 s against 0.55 s). On the
217 commits of a regroup range: 0.94 s against 1.00 s, none over 13 ms. End to end on the Studio, both builds alternated
(`pnpm perf:monorepo` on 92 commits of idea-1, best / median of 3, load ~12): `brd commits` cold 1.03 / 1.03 s against
1.03 / 1.04 s, `brd check` cold 2.54 / 2.55 s against 2.47 / 2.49 s, a full sync 0.58 / 0.60 s against
0.55 / 0.56 s; a cold `brd review-plan` of those 92 commits 6.5–6.6 s both, of 2,211 commits 26.5–26.6 s both.

## What Changes

- `blocks_of` compares with `DiffBudget::NONE`: only the limits of the work bound a scan (`MAX_INNER_BYTES`,
  `MAX_INNER_LINES`, the changes the comparison may count). It takes no diff by lines alone from memory.
- `diff_texts_in` says whether a deadline ran out (`BudgetedDiff::out_of_time`); the views and a merge of one side
  keep a diff out of time in memory only, also when its lines ran out on a file over the inner limits.
- The versions of the `commit_diff` (7), `commit_lines` (6), `file_diff` (`MODEL_VERSION` 15) and `file_merge`
  (`MERGE_MODEL_VERSION` 10) stores, so no value a time cut may have made is read again.
- The views keep their budgets.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: a scan compares without a deadline.

## Impact

- `crates/text-diff/src/model.rs` (`diff_texts_in`, `BudgetedDiff`), `crates/file-diffs/src/blocks.rs`,
  `crates/file-diffs/src/service.rs`, `crates/file-diffs/src/merge.rs`, `crates/object-cache/src/cache.rs`.
- `crates/file-diffs/examples/scan_limits.rs`: the measurement.
- `docs/design/git-operations.md` (File diffs), `docs/design/text-diff.md` (Budget).
