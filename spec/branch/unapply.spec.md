---
name: Unapply Branch
description: Requirements for unapplying a virtual branch, which drops its commits from the current branch and moves the branch to the unapplied namespace, atomically and without touching the working tree.
style: plain-1
targets:
  - ../../crates/sync-core/src/unapply_branch.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-ops/src/rewrite_plan.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/git-ops/src/rewrite_guard.rs
  - ../../crates/branch-integration/src/archive.rs
  - ../../src-tauri/src/commands/unapply_branch.rs
  - ../../src/hooks/useUnapplyBranch.ts
  - ../../src/components/branch/InlineUnapplyConfirm.tsx
---

# Unapply Branch

Status: Draft
Date: 2026-10-14

## Summary

Unapplying a virtual branch drops its commits from the current branch and moves the virtual branch to the unapplied
namespace. Branch Deck replays the remaining commits on objects only. The branch move and the rename happen in one ref
transaction. The working tree is never written, so the dropped content stays in it as uncommitted changes.

## Goals

- Remove a feature from the current branch without losing its work.
- Keep the user's staged changes that the drop does not touch.
- All or nothing: either the drop and the rename both happen, or neither does.

## Non-goals

- Dropping commits from a range that contains merge commits.
- Reverting the dropped content in the working tree.

## Requirements

### Confirmation

- "Unapply…" in the branch menu must expand the branch and open a confirmation below its row that says how many
  commits leave HEAD. Its "Unapply Branch" button takes the focus, so Enter confirms; Escape cancels.

### Result

- The commits of the branch must leave the current branch, and the commits after them must be replayed in order.
- The virtual branch must move to the unapplied namespace, under a name that no ref takes, lies below or lies above
  (git refuses `a` next to `a/b`); a conflicting level gets the `-N` suffix (`feature-1/x`).
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`test_unapply_avoids_a_ref_below_the_name`, `test_unapply_avoids_a_ref_above_the_name`)
- The drop and the rename must happen in one ref transaction. A failed drop must not leave a renamed branch.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_is_atomic`)

### Working tree and index

- The unapply must not write to the working tree.
- The dropped content must stay as unstaged working-tree changes.
- Files that the dropped commits added must become untracked.
- Staged changes on paths that the drop does not touch must stay staged.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_keeps_unrelated_staged_changes`)

### Refusals

- The unapply must be refused, and change nothing, in these cases:
  - a rebase, merge, cherry-pick or revert is in progress;
  - HEAD is detached;
  - the range from the oldest dropped commit to HEAD contains a merge commit;
  - staged changes overlap the paths that the drop changes;
  - the branch moved meanwhile.
  [@test] ../../crates/git-ops/src/rewrite_plan_test.rs (`rewrite_refused_during_merge`)
- A refusal for staged changes must name the paths whose staged version the drop would lose, as git stores them (a
  quote in the name included), and no path whose staged change the drop keeps.
  [@test] ../../crates/git-ops/src/rewrite_plan_test.rs (`a_drop_under_a_staged_change_names_the_path`)
- The unapply must run under the repository lock: while another Branch Deck operation holds it, the unapply waits up
  to 10 s, then it is refused with nothing changed and the error toast says that another operation holds the lock; a
  HEAD the request was not made for refuses it too.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)
- A later commit that depends on a dropped commit must make the unapply fail with a conflict, and no ref, index
  entry or file may change.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_conflict_changes_nothing`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/unapply_branch_test.rs`.

## Open Questions / Risks

- `git replay` reports a conflict only by its exit code, so the error cannot yet name the conflicting commit or files.

## References

- [Git Operations](../../docs/design/git-operations.md)
- [Virtual Branches](../../docs/design/virtual-branches.md)
