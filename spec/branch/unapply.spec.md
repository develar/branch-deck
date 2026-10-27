---
name: Unapply and Reapply
description: Requirements for unapplying a virtual branch, which takes its commits and their files out of the current branch and keeps the branch in the unapplied namespace, atomically, and for reapplying it, which puts its commits back on top with their grouping prefix; both refused by local changes on the paths they write, both with an undo.
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
  - ../../crates/operations/src/rewrite.rs
  - ../../src/lib/rewrite-run.ts
  - ../../src/lib/git/delete-kept-branch.ts
  - ../../src/hooks/useUnapplyBranch.ts
  - ../../src/components/branch/InlineUnapplyConfirm.tsx
  - ../../crates/bd-update/src/unapply_command.rs
  - ../../crates/bd-update/src/undo_command.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/domain-types/src/lib.rs
  - ../../crates/sync-core/src/push.rs
  - ../../crates/sync-core/src/delete_archived_branch.rs
  - ../../crates/bd-remote/src/push_command.rs
  - ../../crates/bd-remote/src/safe_push_command.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../src/stores/unapplied-branches.ts
  - ../../src/components/branch/UnappliedBranchTableCard.tsx
  - ../../src/hooks/useUnappliedBranchMenu.ts
  - ../../src/lib/push-run.ts
  - ../../crates/repository-model/src/table.rs
  - ../../crates/sync-core/src/reapply_branch.rs
  - ../../crates/git-ops/src/reapply.rs
  - ../../crates/git-ops/src/pick.rs
  - ../../src/lib/git/reapply.ts
---

# Unapply and Reapply

Status: Implemented
Date: 2026-10-21

## Summary

Some work must leave the local branch for a while: the world is not ready for it, a team prepares a proper fix, a
change waits for a decision. Parking keeps it local but in the build; unapplying takes it out. The commits of the
virtual branch leave the current branch, the files they changed go back to how they are without them, and the virtual
branch is kept in the unapplied namespace (`{prefix}/unapplied/<name>`), which no sync archives or deletes. Reapply
puts its commits back on top of the current branch, their files and their grouping prefix with them.

## Goals

- Take a branch out of the current branch, its files too, without losing its work.
- Keep the user's work on other paths as it is, staged or not.
- All or nothing: HEAD, the files and the branch move together, or nothing does.
- Take it back at once with Undo, and let an agent do the same with `bd unapply` and `bd undo`.
- See what waits outside HEAD, keep a copy of it on the remote, and delete it once it is not needed.
- Bring it back when the world is ready, as the branch it was.

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
- The unapply, the reapply, the delete of an unapplied branch and their undo must be rewrites of the engine (the intent
  `rewrite`, `execute`): the session runs them one after the other with the other rewrites of the app, in the order
  they came, each on what the one before it left, and a second start of the same one joins it; the delete holds only
  the ref it drops, so it runs beside a rewrite and a sync. The unapply, the reapply and their undo run under the
  repository lock: while another process holds it (an agent's `bd pick --follow`, `bd`) or the app's own push, the
  app's wait as long as the holder lives, their surface (the confirmation, the "⋯" of the row) saying whom for with
  Stop Waiting (`cancel_operation` of the operation), which ends the wait with nothing changed; only a holder no record
  names refuses them after 10 s, with nothing changed and the error toast saying that another operation holds the
  lock; `bd unapply` and `bd reapply` wait 10 s, then `locked`. A HEAD the request was not made for refuses it too.
  [@test] ../../crates/operations/src/rewrite_test.rs (`an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine`, `a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] ../../src/lib/rewrite-run.test.ts (`shows whom a rewrite of the engine waits for and stops the wait by its operation`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`, `a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_refuses_another_head_than_the_expected_one`)
- The index lock of another process must make the unapply, the reapply and their undo wait (at most 30 s per write)
  and an ownerless one be removed, as for `bd pick`; one that stays must move HEAD back and answer `locked`.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)
- A later commit that depends on a dropped commit must make the unapply fail with a conflict, and no ref, index
  entry or file may change.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`unapply_conflict_changes_nothing`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_conflict_of_the_drop_changes_nothing`)
- A failed unapply must keep the branch in the table and say why in the error toast "Cannot unapply ‹branch›".
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep the branch and say why when the unapply fails, and close the confirmation with Escape`)

### Reapply

- Reapply (the menu of an unapplied row, `bd reapply <name>`) must put the branch's own commits (after its stack note,
  else its merge base with the baseline) back on top of HEAD, the files following, and delete the unapplied branch in
  the same transaction; author kept, committer the current user. A commit whose subject groups elsewhere must get
  `(<name>) ` back, since a virtual commit keeps no grouping prefix; an issue branch comes back as it was. The next sync
  builds the branch again.
  [@test] ../../crates/sync-core/src/reapply_branch_test.rs (`reapply_puts_the_commits_back_on_top_with_their_prefix_and_files_and_the_sync_builds_the_branch_again`, `an_issue_branch_comes_back_without_a_prefix`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should reapply an unapplied branch from both menus, the branch back in the table, and take it back with Undo`)
- A commit HEAD has by its patch must be left out, one that becomes empty drops; a dry run must change nothing.
  [@test] ../../crates/sync-core/src/reapply_branch_test.rs (`a_dry_run_changes_nothing_and_says_what_the_reapply_would_do`)
- The reapply must be refused, and change nothing, for an unknown branch, a commit that conflicts with HEAD (its
  files named), local changes on a path it writes, a branch checked out in a worktree, and a HEAD with every commit of
  it already; under the repository lock, as every rewrite.
  [@test] ../../crates/sync-core/src/reapply_branch_test.rs (`reapply_refuses_an_unknown_branch_a_conflict_and_local_changes_in_the_way_and_changes_nothing`, `reapply_waits_for_the_repository_lock_and_fails_with_locked`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should keep an unapplied branch whose reapply conflicts and say why`)
- The toast "Reapplied ‹branch›" must offer Undo, which moves HEAD and the files back and creates the unapplied branch
  again; the token `reapply:<previous head>..<new head>:<tip>:<unapplied branch>`; stale once HEAD moved on.
  [@test] ../../crates/sync-core/src/reapply_branch_test.rs (`the_undo_of_a_reapply_takes_the_commits_out_again_and_keeps_the_unapplied_branch`, `a_reapply_token_names_full_ids_and_the_unapplied_branch`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should reapply an unapplied branch from both menus, the branch back in the table, and take it back with Undo`)
- `bd reapply <name> [--dry-run] [--expect-head <id>]` must print one object (`reapply` in `bd schema`) and refuse with
  `reapply_conflict` (`commits`, `paths`), `uncommitted_overlap`, `nothing_to_reapply` and `unknown_branch`.
  [@test] ../../crates/bd-update/src/unapply_test.rs (`reapply_puts_the_branch_back_with_its_prefix_and_the_undo_takes_it_out_again`)

### The Unapplied card

- Every sync must report the unapplied branches once, also none, with the tip, its subject and the remote status of
  each one whose remote branch exists; one without a remote branch costs no process.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`a_sync_reports_the_unapplied_branches_with_the_remote_status_of_a_pushed_one`)
- The app must list them in the Unapplied card, after the branch table and the Parked card, before the archived
  branches, open by default, its rows out of the branch table: the name, the subject, the Remote cell ("Not pushed",
  "Pushed", "2 commits to push", "Remote differs"), and its commits below an expanded row; a help popover says what an
  unapplied branch is.
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should list an unapplied branch in its own card with its commits, out of the branch table`)
- The menu of a row (right-click and "⋯") must offer Reapply, Push (Force Push when the remote has another history,
  disabled when it has it as it is), Safe Push disabled with "Not in master: Reapply it to land it", Copy, and Delete
  Unapplied Branch….
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should push an unapplied branch under its own name from both menus, its status shown with no sync, and refuse its Safe Push with why`)
- Push must push it under its own name (`{prefix}/unapplied/<name>`) with a lease on the remote head of its status, the
  intent `Push` with the scope `unapplied` (`operations::push`): no robot read, no hold, no retarget (C3), "Force Push"
  of its menu the user's choice to drop what the remote has; the
  toast "Pushed ‹branch›" says the remote keeps it, and its Remote cell shows the status the push left from the patch of
  the session, with no sync after it. A pull request of its virtual branch stays as it was.
  [@test] ../../crates/sync-core/src/push_test.rs (`an_unapplied_branch_is_pushed_under_its_own_name`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_of_an_unapplied_branch_is_planned_and_run_by_the_engine`)
  [@test] ../../crates/repository-model/src/table_test.rs (`the_outcome_of_a_push_stays_over_the_remote_statuses_of_a_sync_that_started_before_it`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should push an unapplied branch under its own name from both menus, its status shown with no sync, and refuse its Safe Push with why`)
- Delete Unapplied Branch… must ask for its name typed, then delete it with compare-and-swap on its tip, refused while
  a worktree has it; only its own rewrite deletes it (`deleteUnapplied`, by its full name). Its row leaves the card as
  a patch of the session, with no sync after it; a sync that listed it before the delete, or one that fails, does not
  bring it back. The form shows the delete while it runs and stays with the typed name after a failure.
  [@test] ../../crates/sync-core/src/delete_archived_branch_test.rs (`deletes_an_unapplied_branch_only_by_its_own_command`)
  [@test] ../../crates/repository-model/src/table_test.rs (`a_deleted_archived_or_unapplied_branch_stays_deleted_by_the_sync_that_listed_it_before_and_by_one_that_fails`)
  [@test] ../../crates/repository-model/src/session_test.rs (`deleting_an_archived_or_unapplied_branch_answers_a_patch_and_runs_no_sync`)
  [@test] ../../crates/operations/src/rewrite_test.rs (`an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should delete an unapplied branch after its name is typed, with no sync after it`)

### Undo

- The toast "Unapplied ‹branch›" must offer Undo (and Activity the same: the rewrite `undo` of its token, in the
  repository of the unapply), which brings the commits, their files and the virtual branch back as they were: HEAD at its previous commit, the virtual branch at its tip, the unapplied branch
  gone, in one transaction, the files following; the next sync reuses the copies.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`the_undo_of_an_unapply_brings_the_commits_their_files_and_the_virtual_branch_back`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`the_undo_brings_the_commits_their_files_and_the_virtual_branch_back`)
  [@test] ../../tests/e2e/unapplied-branches/unapplied-branches.spec.ts (`should take the unapply back with the Undo of its toast: the commits, their files and the branch return`)
- The undo token must be `unapply:<previous head>..<new head>:<tip>:<virtual branch>:<unapplied branch>`, full ids and
  full branch names; any other text is no token of an unapply.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`an_unapply_token_names_full_ids_and_both_branches`)
- The undo must be refused, and change nothing, when the unapply is not the last move of the branch (a commit since)
  or a local change sits on a path it writes; undoing again is no error, nothing changes.
  [@test] ../../crates/sync-core/src/unapply_branch_test.rs (`the_undo_of_an_unapply_is_refused_after_a_new_commit_and_by_a_local_change_on_its_paths`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`the_undo_of_an_unapply_that_is_not_the_last_move_is_refused`)

### `bd`

- `bd unapply <branch> [--expect-head <id>]` must unapply as the app does and print one object (`unapply` in
  `bd schema`) with the unapplied branch, the commits, both heads and the `undo_token`; `bd undo` of it must undo it
  (`restored_branch`, `already_undone`).
  [@test] ../../crates/bd-update/src/unapply_test.rs (`unapply_takes_the_branch_and_its_files_out_of_head_and_the_undo_brings_them_back`, `unapply_outputs_match_the_schema`)
- Its refusals must be codes: `unknown_branch` (exit 2), `uncommitted_overlap` with `paths`, `branch_not_built`,
  `branch_checked_out` and `drop_conflict` (exit 4).
  [@test] ../../crates/bd-update/src/unapply_test.rs (`unapply_refuses_an_unknown_branch_one_not_built_and_local_changes_in_the_way`)
- `bd status` must list the unapplied branches (`unapplied`: name, branch, tip, subject, the remote head and whether it
  is up to date), from the listing it read; `bd push` and `bd safe-push` must refuse the name of one as `unapplied`
  (exit 4), and `bd push --unapplied <name>` must push it under its own name, as the app's push of it (the intent
  `Push` with the scope `unapplied`, after a sync): leased on the remote head the sync saw (or `--expect-remote`), a
  push that drops commits of its remote branch it lacks refused as `push_refused` until `--force`.
  [@test] ../../crates/bd-update/src/unapply_test.rs (`status_lists_an_unapplied_branch_push_refuses_it_by_its_name_and_pushes_it_with_unapplied`)
  [@test] ../../crates/bd-remote/src/push_test.rs (`an_unapplied_branch_whose_remote_has_commits_it_lacks_is_pushed_only_with_force`)
- `bd help agent` and the skill must tell an agent to unapply only when the user asks.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### Process budget

- An unapply must cost the processes `a_drop` pins: the grouping read once, one `replay` per kept run, one dry run of
  the files and one move; its undo those of `an_undo_of_an_unapply`.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_drop`, `an_undo_of_an_unapply`)
- A reapply must cost the processes `a_reapply` pins (one `fast-import` for the restored messages, one `replay`, one
  dry run of the files, one move); its undo those of `an_undo_of_a_reapply`.
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_reapply`, `an_undo_of_a_reapply`)

## User Experience

- Unapply… → the confirmation below the row ("This removes 2 commits and their changes from HEAD"), Enter → the row
  leaves the table, the files with it → toast "Unapplied docs" · "Removed 1 commit from HEAD · kept in
  jane/unapplied/docs" · Undo.
- The Unapplied card: "Unapplied · 1 branch", the row "docs · Document the API · Not pushed"; Push → toast "Pushed docs"
  · "The remote keeps it as jane/unapplied/docs"; Reapply → the row back in the branch table → toast "Reapplied docs"
  · "1 commit back on top of HEAD" · Undo.
- `bd unapply docs` → `{"unapplied_branch":"jane/unapplied/docs",…,"undo_token":"unapply:…"}`; `bd undo unapply:…`;
  `bd status` → `"unapplied":[{"name":"jane/unapplied/docs","branch":"docs",…}]`; `bd push --unapplied docs`;
  `bd reapply docs` → `{"reapplied":[…],"prefix_restored":1,"undo_token":"reapply:…"}`.

## Open Questions / Risks

- `git replay` reports a conflict of the unapply's drop only by its exit code, so its error cannot yet name the
  conflicting commit or files; the reapply's can (the binary search of the pick).
- A name that got a `-N` suffix at the unapply (`docs-1`) comes back under that name: the prefix restored is the one
  of the unapplied branch. A `(parent/child)` prefix comes back as `(child)`; the stack parents
  (`refs/branch-deck/stack-parents`) stack it again where they name its parent.
- A sync between a reapply and its undo builds the virtual branch; after the undo the next sync archives it.

## References

- [Git Operations](../../docs/design/git-operations.md) ("Drop and unapply")
- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Parked Branches](parked-branches.spec.md): the branches kept local in the build
