---
name: Amend Changes
description: Requirements for amending selected uncommitted changes into a commit of the current branch without touching the working tree or the user's staged work.
style: plain-1
targets:
  - ../../crates/git-ops/src/amend_operations.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/git-ops/src/rewrite_guard.rs
  - ../../crates/sync-core/src/amend_to_branch.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/operations/src/rewrite.rs
  - ../../src/hooks/useAmendChanges.ts
  - ../../src/components/branch/InlineAmendChangesInput.tsx
  - ../../src/components/branch/UncommittedChangesCard.tsx
  - ../../src/components/conflict/SubjectFileDiffList.tsx
  - ../../src/presenters/rewrite.ts
  - ../../crates/sync-core/src/uncommitted_changes.rs
  - ../../crates/git-ops/src/index_lock.rs
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
  uncommitted changes reads the diffs again, the shown ones stay meanwhile. A new answer during a read reads them once
  more after it (never dropped, however many came). The diffs are of the working tree of their repository: those of
  another repository are another entry, never shown for this one.
  [@test] ../../src/stores/file-diffs.test.ts (`reads the uncommitted changes again on a refresh, and keeps the loaded ones until the new read ends`)
  [@test] ../../src/stores/file-diffs.test.ts (`reads the uncommitted changes once more after a refresh asked during a read, never dropping it`, `keeps the uncommitted changes of two repositories apart, each read from its own working tree`)

### Working tree and index

- The amend must not write to the working tree.
- Staged changes on paths that the amend does not touch must stay staged.
- The amended files must show as unmodified afterwards: their working-tree versions are staged, like
  `git commit -- <paths>` does, replacing a partly staged version.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_keeps_unrelated_staged_changes`)
- An amend whose index cannot follow the branch it moved (a file unreadable since the amend was computed, the index
  lock of another process past its wait) must answer that failure, with the advice to run `git reset -q`: the branch
  was rewritten once, and the amend never runs again on the HEAD it wrote itself.
  [@test] ../../crates/operations/src/rewrite_test.rs (`an_amend_whose_index_cannot_follow_the_branch_it_moved_answers_that_and_runs_once`)

### Refusals

- The amend must be refused, and change nothing, in these cases:
  - a rebase, merge, cherry-pick or revert is in progress;
  - HEAD is detached;
  - the range from the target to HEAD contains a merge commit;
  - the branch moved meanwhile.
  [@test] ../../crates/git-ops/src/amend_operations_test.rs (`amend_refuses_range_with_merges`)
  [@test] ../../crates/git-ops/src/rewrite_plan_test.rs (`rewrite_refused_during_merge`)
- The amend of the app is the rewrite `amend` of the engine, named on the table the window shows, and must write
  under the repository lock: the history, the merge, the amended commit and the replay of the commits above it come
  before the lock, and under it only the witness of HEAD, the move of the branch and the staging of the amended files
  (one `update-index`, no refresh). A HEAD another process moved meanwhile (`bd update`, a `bd` rewrite of a commit
  below, a commit on top) is refused (`head_moved`) before the amended commit is looked for on it, under the lock by
  one `rev-parse` before the history is read again; the engine then resolves the amend again on HEAD as it is now and
  amends the copy of the commit there, found by its author, author time and subject. While another
  process holds the lock (an agent's `bd pick --follow`, `bd`) or the app's own push, the
  amend waits as long as the holder lives, its inline form saying whom for, the step of its operation ("Waiting for
  `bd pick` (pid 4242, for 12 s)"), with Stop Waiting (`cancel_operation`), which ends the wait with nothing changed
  and the form kept; only a holder no record names refuses it after 10 s, with nothing changed and the error toast
  saying that another operation holds the lock; a HEAD the request was not made for refuses it too. An error keeps the
  form; a conflict opens the conflict viewer below the row in its place.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`, `a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`, `every_rewrite_refuses_another_head_that_has_none_of_its_commits`)
  [@test] ../../crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] ../../crates/operations/src/rewrite_test.rs (`an_amend_waiting_for_the_lock_of_another_process_that_rewrites_below_its_commit_amends_the_copy`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`an_amend_and_an_issue_reference_lock_only_the_witness_and_the_write`)
- A conflict between the amended change and a later commit must refuse the amend and change no ref, index entry or
  file.

## Testing / Local Run

- `pnpm test:backend`: `crates/git-ops/src/amend_operations_test.rs`.
- `pnpm e2e`: `tests/e2e/amend-changes` covers the form.

## Open Questions / Risks

- The index update after the branch moved waits for the index lock of another process (at most 30 s) and removes one
  that no process owns, as the moves do ([Index lock](../../docs/design/git-operations.md#index-lock),
  `the_index_of_a_rewrite_follows_past_a_lock_that_no_process_owns`). If it still fails, the error suggests
  `git reset -q` and is not retryable: the branch was rewritten already.

## References

- [Git Operations](../../docs/design/git-operations.md)
- [Commit Row](commit-row.spec.md)
