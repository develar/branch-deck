---
name: Unapply Branch
description: Requirements for unapplying a virtual branch, which takes its commits and their files out of the current branch and keeps the branch in the unapplied namespace, atomically, refused by local changes on the paths it writes.
style: plain-1
targets:
  - ../../crates/sync-core/src/unapply_branch.rs
  - ../../crates/git-ops/src/unapply.rs
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/git-ops/src/worktree_update.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-ops/src/rewrite_plan.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/branch-integration/src/archive.rs
  - ../../src-tauri/src/commands/unapply_branch.rs
  - ../../src/hooks/useUnapplyBranch.ts
  - ../../src/components/branch/InlineUnapplyConfirm.tsx
---

# Unapply Branch

Status: Implemented
Date: 2026-10-21

## Summary

Some work must leave the local branch for a while: the world is not ready for it, a team prepares a proper fix, a
change waits for a decision. Parking keeps it local but in the build; unapplying takes it out. The commits of the
virtual branch leave the current branch, the files they changed go back to how they are without them, and the virtual
branch is kept in the unapplied namespace (`{prefix}/unapplied/<name>`), which no sync archives or deletes.

## Goals

- Take a branch out of the current branch, its files too, without losing its work.
- Keep the user's work on other paths as it is, staged or not.
- All or nothing: HEAD, the files and the branch move together, or nothing does.

## Non-goals

- Dropping commits from a range that contains merge commits.
- Taking the children of a stacked branch out with it: they stay, built on the next branch below that is applied.

## Requirements

### Confirmation

- "Unapply…" in the branch menu (right-click and "⋯") must expand the branch and open a confirmation below its row that
  says how many commits leave HEAD with their changes, and that uncommitted changes to those files stop it. Its
  "Unapply Branch" button takes the focus, so Enter confirms; Escape cancels.
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`, `should keep the branch and say why when the unapply fails, and close the confirmation with Escape`)

### Result

- The commits of the branch, as they group now, must leave the current branch, and the commits after them must be
  replayed in order. The app's request names the commits it shows: when the branch groups others now, the unapply is
  refused, "sync and try again".
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_takes_the_commits_of_the_branch_out_of_head_with_their_files`, `unapply_refuses_an_unknown_or_changed_branch_one_not_built_and_one_checked_out`)
- The files the commits changed must follow: a file they added is gone, a file they changed is as the remaining commits
  leave it, and `git status` shows nothing of them.
  [@test] ../../crates/git-ops/src/unapply_test.rs (`the_files_follow_the_unapply_and_the_virtual_branch_moves_in_the_same_transaction`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`)
- The virtual branch must move to the unapplied namespace, under a name that no ref takes, lies below or lies above
  (git refuses `a` next to `a/b`); a conflicting level gets the `-N` suffix (`feature-1/x`). A later sync leaves it as
  it is.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`test_unapply_avoids_a_ref_below_the_name`, `test_unapply_avoids_a_ref_above_the_name`, `unapply_takes_the_commits_of_the_branch_out_of_head_with_their_files`)
- The move of HEAD, the move of the virtual branch and the files must happen together: one ref transaction, and when the
  files can't follow (a file changed since the check), HEAD and the branch move back and nothing is changed. A crash
  between the move and the files leaves a mark that the next move of the files finishes.
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_file_changed_after_the_check_moves_head_and_the_branch_back`, `an_interrupted_unapply_finishes_its_files`)
- The toast "Unapplied ‹branch›" must name the commits that left HEAD and the unapplied branch that keeps them.
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept`)

### Working tree and index

- Staged, unstaged and untracked work on paths the unapply does not write must stay as it is.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_keeps_unrelated_work_staged_and_unstaged`)
- A local change on a path the unapply writes, staged or not, must refuse it with the paths, and change nothing.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_refuses_a_local_change_on_a_path_it_writes_and_changes_nothing`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_local_change_on_a_path_the_unapply_writes_refuses_it_staged_or_not`)

### Refusals

- The unapply must be refused, and change nothing, in these cases:
  - no commit groups into the branch, or it groups others than the request names;
  - the virtual branch does not exist, or is not the copies of its commits now (a conflict, a sync that did not run):
    a commit would leave HEAD without a copy;
  - a worktree has the virtual branch checked out;
  - a rebase, merge, cherry-pick or revert is in progress, or HEAD is detached;
  - the range from the oldest dropped commit to HEAD contains a merge commit;
  - the branch moved meanwhile.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_refuses_an_unknown_or_changed_branch_one_not_built_and_one_checked_out`)
  [@test] ../../crates/git-ops/src/rewrite_plan_test.rs (`rewrite_refused_during_merge`)
- The unapply must run under the repository lock: while another Branch Deck operation holds it, the unapply waits up
  to 10 s, then it is refused with nothing changed and the error toast says that another operation holds the lock; a
  HEAD the request was not made for refuses it too.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)
- A later commit that depends on a dropped commit must make the unapply fail with a conflict, and no ref, index
  entry or file may change.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_conflict_changes_nothing`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_conflict_of_the_drop_changes_nothing`)
- A failed unapply must keep the branch in the table and say why in the error toast "Cannot unapply ‹branch›".
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep the branch and say why when the unapply fails, and close the confirmation with Escape`)

### Process budget

- An unapply must cost the processes `a_drop` pins: the grouping read once, one `replay` per kept run, one dry run of
  the files and one move.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_drop`)

## User Experience

- Unapply… → the confirmation below the row ("This removes 2 commits and their changes from HEAD"), Enter → the row
  leaves the table, the files with it → toast "Unapplied docs" · "Removed 1 commit from HEAD · kept in
  jane/unapplied/docs".

## Open Questions / Risks

- `git replay` reports a conflict only by its exit code, so the error cannot yet name the conflicting commit or files.

## References

- [Git Operations](../../docs/design/git-operations.md) ("Drop and unapply")
- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Parked Branches](parked-branches.spec.md): the branches kept local in the build
