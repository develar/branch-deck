---
name: Amend Changes
description: Requirements for amending selected uncommitted changes into a commit of the current branch without touching the working tree or the user's staged work.
style: plain-1
targets:
  - ../../crates/git-ops/src/amend_operations.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/git-ops/src/rewrite_guard.rs
  - ../../crates/sync-core/src/amend_to_branch.rs
  - ../../crates/sync-core/src/repository_lock.rs
  - ../../src-tauri/src/commands/amend_to_branch.rs
  - ../../src/hooks/useAmendChanges.ts
  - ../../src/components/branch/InlineAmendChangesInput.tsx
  - ../../src/components/branch/UncommittedChangesCard.tsx
  - ../../src/components/conflict/SubjectFileDiffList.tsx
---

# Amend Changes

Status: Draft
Date: 2026-10-14

## Summary

The user selects files from the uncommitted changes and amends them into a commit of the current branch. The target
can be HEAD, an older commit or the root commit. Branch Deck rewrites the branch on objects and refs only. The working
tree is never written, and the user's other staged work stays staged.

## Goals

- Put exactly the selected changes into the target commit, whatever commits follow it.
- Keep the working tree byte for byte, and keep the staged changes that the amend does not touch.
- Fail without any change when the amend cannot be done cleanly.

## Non-goals

- Amending into a range that contains merge commits.
- Resolving conflicts between the amended change and later commits.

## Requirements

### Content

- The amend must take only the selected files from the working tree. Other modified files must stay uncommitted.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_only_selected_files`)
- Amending a commit other than HEAD must apply only the working-tree delta relative to HEAD. Changes that later
  commits made to the same file must not move into the target commit.
- After the amend, the tip tree must equal HEAD's tree plus the selected files.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_non_head_applies_only_delta`)
- The amended commit must keep its author and its message. Its committer and the committer of the commits after it
  become the current user.

### The diffs of the uncommitted changes

- The Uncommitted Changes card must show the diff of each changed file as the diff engine computes it, the working
  tree against HEAD under the whitespace policy, with the engine's `+N −M` in its header; a new answer of the
  uncommitted changes reads the diffs again, the shown ones stay meanwhile.
  [@test] ../../src/stores/file-diffs.test.ts (`reads the uncommitted changes again on a refresh, and keeps the loaded ones meanwhile`)

### Working tree and index

- The amend must not write to the working tree.
- Staged changes on paths that the amend does not touch must stay staged.
- The amended files must show as unmodified afterwards: their working-tree versions are staged, like
  `git commit -- <paths>` does, replacing a partly staged version.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_keeps_unrelated_staged_changes`)

### Refusals

- The amend must be refused, and change nothing, in these cases:
  - a rebase, merge, cherry-pick or revert is in progress;
  - HEAD is detached;
  - the range from the target to HEAD contains a merge commit;
  - the branch moved meanwhile.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_refuses_range_with_merges`)
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`rewrite_refused_during_merge`)
- The amend must run under the repository lock: while another Branch Deck operation holds it, the amend waits up to
  10 s, then it is refused with nothing changed and the error toast says that another operation holds the lock; a
  HEAD the request was not made for refuses it too.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)
- A conflict between the amended change and a later commit must refuse the amend and change no ref, index entry or
  file.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/amend_operations_test.rs`.
- `pnpm e2e`: `tests/e2e/amend-changes` covers the form.

## Open Questions / Risks

- If the index update fails after the branch moved (e.g. a concurrent `git add` holds the index lock), the error
  suggests `git reset -q`.

## References

- [Git Operations](../../docs/design/git-operations.md)
- [Commit Row](commit-row.spec.md)
