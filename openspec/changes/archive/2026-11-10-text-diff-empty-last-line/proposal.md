# Proposal

## Why

The diff engine compared the empty line after the final line end of a text as a line like any other, as IntelliJ
compares the lines of a document. That line could pair with a blank line of the other text, and the empty last line
of the other text, which no count takes, went into a block instead: an added file counted one line fewer for its first
blank line (+7 where `git diff --numstat` counts +8), a config that lost a server after a blank line −3 for −4, a class
that grows by a method, then a blank line and two declarations after its closing brace, +8 −0 for +9 −0, and the diff
drew the blank line as unchanged. The counts of the tree and of the header of a card, and every block the views,
`brd commits --patch` and the dependency edges read, were one blank line off in such a file.

## What Changes

- The comparison of the views leaves the empty last line of each text out (`text_diff::provider::compare_for_view`):
  the two empty last lines pair with each other, and when only one text has one, it is the change of the final line end,
  a block of its own or the end of the block that reaches the ends of both texts. The ported functions of the engine
  keep the lines of IntelliJ.
- The stores of what the engine computed are recomputed: `file_diff` (`MODEL_VERSION` 6), `file_merge`
  (`MERGE_MODEL_VERSION` 3, the two-way diff of a one-sided conflict), `commit_diff` 6, `commit_lines` 5.
- The example of a file the engine aligns otherwise than git, in the Map of a review, is a call moved into the test above
  it (+3 −3 by the engine, +1 −1 by git), as the old one no longer differs.
- The E2E snapshots of the amend dialog, the conflict viewer and the concerns of a review count those lines.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: Reading the diffs: the empty line after a final line end pairs only with its like.
- `branch/review-tour`: The Map: the example of a file the engine aligns otherwise.

## Impact

- `crates/text-diff/src/provider.rs`, `crates/text-diff/src/manager.rs` (`create_inner_fragments` for the crate),
  `crates/file-diffs/src/service.rs`, `crates/file-diffs/src/merge.rs`, `crates/object-cache/src/cache.rs`;
  `docs/design/text-diff.md` (Key Decision 8), `docs/design/git-operations.md` (the store versions).
