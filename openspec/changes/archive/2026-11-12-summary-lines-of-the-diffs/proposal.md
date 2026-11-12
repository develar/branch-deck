# Proposal

## Why

The scopes of a diff and its blocks of the imports alone come from the syntax summary of each side, which counts the
lines of git: a line ends at `\n` alone. The blocks count the lines of the diffs, where a lone `\r` ends a line too.
`file_diffs::service::import_blocks` split the text at `\n` and set the lines of the statements beside the lines of
the blocks as they were, so below a lone `\r` every statement and every scope stood one line early for each lone
`\r` above it. A line of code right above the first statement (a `@file:` annotation of Kotlin, a directive of
TypeScript) was read as an import: a review with its plan played its block down and N and P passed a change of code.
A hunk header named the declaration before the one the hunk is in. The units of a review plan had translated the
lines of a summary already (`TextLines::of_git`, private to `review-plan`).

## What Changes

- `TextLines` moves to `text_diff::text_lines`, where both crates reach it, with `of_summary` for a span of a
  summary (from 1, both ends included).
- `file_diffs::service::side_facts` gives the scopes and the import statements by the lines of the diffs, for the
  diffs of the views and the scopes of a merge; `scopes_of_summary` takes the lines of its text, so the patch of
  `brd` reads the same lines; `import_blocks` reads the text by the same lines.
- `MODEL_VERSION` 19 and `MERGE_MODEL_VERSION` 12: a stored diff or merge may hold scopes and import blocks by the
  lines of git.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-files`: the scopes of a diff are at the lines of the diff.
- `branch/review-plan`: the statements of the imports are read at the lines of the diff.

## Impact

- `crates/text-diff/src/text_lines.rs` (new), `crates/file-diffs/src/service.rs`, `scopes.rs`, `patch.rs`,
  `merge.rs`, `crates/review-plan/src/whitespace.rs`, `units.rs`, `files.rs`.
- `docs/design/git-operations.md` (File diffs, Scopes), `docs/design/text-diff.md`, `docs/design/change-review.md`.
