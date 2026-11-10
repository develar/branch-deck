# Proposal

## Why

A copy that a resolve by words wrote was reused while its source held, whatever resolve version its note named: the
copy changed only with its inputs, not with an upgrade of the engine. The merge of version 1 resolved conflicts that git
sees, and that the merge of today leaves to git (two insertions at the end of a file, 2ec269d4c), so such a copy kept
a merge that no current sync would make. The user decided that only the affected copies are rebuilt.

## What Changes

- A copy whose mapping note names an older resolve version is checked once by its tree, as a note without a source is:
  the same tree keeps the copy (no new commit, no move of the branch, no push) and its note takes this resolve, so the
  next sync takes it as it is; another tree copies the commit again, and a conflict that this resolve leaves is the
  conflict of git. A note of a newer version is taken as it is.
- The check costs what a commit that conflicts and resolves costs, once; its note goes with the others into the notes
  `fast-import` of the sync.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli/agent-cli`: `brd sync`: a copy of an older resolve by words is checked once.

## Impact

- `crates/git-ops/src/copy_commit.rs`; the fixtures of a word conflict move to `test_utils::sync_fixtures`;
  `docs/design/git-operations.md` ("Virtual commit reuse").
