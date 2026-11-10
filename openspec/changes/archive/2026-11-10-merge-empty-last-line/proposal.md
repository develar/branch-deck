# Proposal

## Why

The merge of the engine compared the empty line after the final line end of each text as a line like any other, as
IntelliJ does, so the empty last line of the base could pair with a blank line of a side. A blank line that one side
added at the end of a file and a line that the other added there then stood on two sides of that line: two insertions,
no conflict, where `git merge-file` stops (`x\n\n` and `x\ny\n` of the base `x\n`, with CRLF too, and `\n` and `x\n`
of an empty base). The conflict view said nothing conflicts, and the resolve by words committed the merge of a
conflict of git that it never saw as one.

## What Changes

- The merge (`merge_texts`) compares each side with the base without the empty last line of either and pairs that line
  only with its like (`text_diff::last_line`, shared with the comparison of the views), then joins the two
  comparisons as the source does (`by_line::merge_whole_texts`): the conflicts at the end of a file are git's, and a
  clean merge there gives git's result.
- `RESOLVE_VERSION` 2: no remembered resolution or conflict view of the old merge is taken; a copy that the old resolve
  wrote stays while its inputs hold, as before. `MERGE_MODEL_VERSION` 4: the stored merges of the conflict view are
  merged again.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-row`: Conflict view: the merge sees the conflicts of git at the end of a file.

## Impact

- `crates/text-diff/src/last_line.rs` (new), `crates/text-diff/src/by_line.rs`, `crates/text-diff/src/manager.rs`,
  `crates/text-diff/src/merge_model.rs`, `crates/text-diff/src/provider.rs` (the same pairing through the shared
  helper), `crates/git-ops/src/word_resolve.rs` (`RESOLVE_VERSION`), `crates/file-diffs/src/merge.rs`
  (`MERGE_MODEL_VERSION`); `docs/design/text-diff.md` (Key Decision 8), `docs/design/git-operations.md` (Resolve by
  words).
