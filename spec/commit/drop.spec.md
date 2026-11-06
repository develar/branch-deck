---
name: Drop Commits
description: Requirements for dropping commits ahead of the baseline from the current branch for good, their files following, as the agent's way to take back its own commit instead of a revert that lands both into master; refused by a later commit that depends on a dropped one or would become empty and by local changes on the paths it writes, with an undo, also for a commit a lease already picked onto its home branch.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/drop_commits.rs
  - ../../crates/git-ops/src/unapply.rs
  - ../../crates/git-ops/src/rewrite_plan.rs
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/domain-types/src/rewrite.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/operations/src/rewrite.rs
  - ../../crates/app-api/src/api_error.rs
  - ../../crates/bd-update/src/drop_command.rs
  - ../../crates/bd-update/src/undo_command.rs
  - ../../crates/bd-lease/src/lease_command.rs
  - ../../crates/bd-cli/src/lib.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../src/presenters/rewrite.ts
  - ../../src/presenters/operations.ts
  - ../../src/presenters/drop.ts
  - ../../src/hooks/useDropCommits.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/components/commit/InlineDropConfirm.tsx
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/lib/review-actions.ts
---

# Drop Commits

Status: Implemented
Date: 2026-11-03

## Summary

The work of a branch is done when it is in master, so a commit that must not land must leave the branch before it
lands, not be answered by a revert: a commit and its revert land into master together, two commits that change
nothing. A drop takes commits ahead of the baseline out of the current branch for good; the commits after them are
replayed, and the files follow, as an unapply's do ([Unapply and Reapply](../branch/unapply.spec.md)), through the same
check and the same guarded move. A commit and its revert dropped together leave every file as it is. An agent drops its
own commit with `brd drop`; a commit its lease already picked onto the shared branch is dropped there.

## Goals

- Take back a commit that must not land, its files with it, without a revert in master.
- Clean up a commit and a revert of it that are both ahead of the baseline.
- Undo the drop, files included.

## Non-goals

- Dropping a commit on the baseline: it landed, a revert of it is the user's to decide.
- Recognizing a revert (`Revert "X"`, "This reverts commit …") by its message: a fold of it would rewrite X under its
  own message when the revert is partial, and the revert groups into no branch (its subject has no prefix). Both are
  named to the drop instead.
- Refusing a drop on a branch that is pushed or under a robot: no rewrite of the branch refuses that; the agent asks the
  user ([Agent CLI](../cli/agent-cli.spec.md)), and the output names the branches the next sync builds anew.

## Requirements

### Result

- The commits named must leave the current branch, the commits after them replayed in order, and the files they changed
  must follow: a file they added is gone, a file they changed is as the remaining commits leave it, and `git status`
  shows nothing of them; work on other paths stays as it is. The branch moves with the reflog message `branch-deck: drop`.
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`)
  [@test] ../../crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`)
- A commit and its revert dropped together must leave HEAD's tree and every file as they are (`files_changed: false`),
  and a later commit whose trees stay must keep its mapping note, so the next sync keeps its virtual copy.
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_drop_of_a_commit_and_its_revert_moves_no_file_and_a_later_commit_keeps_its_note`)
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_commit_and_its_revert_drop_without_a_file_moving_and_empty_their_branch`)
  [@test] ../../crates/bd-update/src/drop_test.rs (`drop_of_a_commit_and_its_revert_moves_no_file`)
- The result must name each dropped commit with its subject and its branch, whether the files changed, the branches the
  next sync builds anew (those of the dropped commits and of the later commits whose files change: a pushed one needs a
  push again) and the branches left without a commit, which the next sync archives.
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`, `a_commit_and_its_revert_drop_without_a_file_moving_and_empty_their_branch`)
- The move of HEAD and the files must happen together: when the files can't follow, HEAD moves back and nothing is
  changed; a crash between the move and the files leaves the mark that the next move of the files finishes.
  [@test] ../../crates/git-ops/src/unapply_test.rs (`an_interrupted_drop_finishes_its_files`)

### Refusals

- The drop must be refused, and change nothing, no temporary ref left, in these cases:
  - an id that names no commit ahead of the baseline (`not_ahead_of_baseline`, with `commits`), an ambiguous one or none
    (`invalid_arguments`);
  - a later commit that depends on a dropped one (a conflict), one that holds only changes of the dropped commits and
    would become empty ("drop it too"), or the root commit (`drop_conflict`);
  - a local change, staged or not, on a path the drop writes (`uncommitted_overlap`, with `paths`);
  - a rebase, merge, cherry-pick or revert in progress, a detached HEAD, a merge in the range;
  - another HEAD than the request was made for (`head_moved`).
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_drop_is_refused_by_a_later_commit_that_depends_on_it_would_become_empty_or_by_the_root`, `a_drop_under_a_staged_change_names_the_path`, `a_drop_is_refused_during_a_merge`)
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_drop_refuses_commits_that_are_not_ahead_and_another_head`)
  [@test] ../../crates/bd-update/src/drop_test.rs (`drop_refuses_what_is_not_ahead_a_dependent_later_commit_local_changes_and_another_head`)
- The app and `brd` must answer each refusal with the same code.
  [@test] ../../crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`)

### Lock and engine

- The drop and its undo must compute before the repository lock (the commits ahead and their grouping, the replay, the
  count of the new history, the dry run of the files, the carried mapping notes; the reflog and the dry run of the
  undo) and hold it only for the witness of HEAD and of the mark of an interrupted move, and the guarded move.
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_drop_and_its_undo_lock_only_the_witness_and_the_move`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_drop_of_commits`)
- The drop must be the rewrite `drop` of the engine and its undo the rewrite `undo` of its `drop:<previous>..<new>`
  token; a drop queued behind a rewrite of its commit names it through the new id that rewrite answered.
  [@test] ../../crates/operations/src/rewrite_test.rs (`a_drop_and_its_undo_run_through_the_engine_and_a_queued_drop_follows_a_reword`)
  [@test] ../../src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)

### Undo

- The undo must bring the commits back with their ids, their files and their mapping notes, refused as the undo of an
  unapply is (a new commit on top: `undo_stale`; local changes on its paths); undoing again is no error
  (`already_undone`). The token of a drop is `drop:` with two full ids, and only the undo of a drop takes it.
  [@test] ../../crates/history-rewrite/src/drop_commits_test.rs (`a_drop_takes_the_commits_and_their_files_out_and_names_what_the_next_sync_builds_anew`, `a_drop_token_names_two_full_ids`)
  [@test] ../../crates/git-ops/src/unapply_test.rs (`a_drop_takes_the_commits_and_their_files_out_of_head_and_its_undo_brings_them_back`)

### App

- The context menu of commits (a branch, the unassigned commits, the preview, the review window) must offer "Drop
  Commit…", "Drop N Commits…" for a selection, its key Backspace or Delete (⌫ on a Mac, Del elsewhere), which opens the
  same with rows selected. It must open a confirmation below the lead row of the commits, as "Move to…" does: the
  commits, "This removes N commits and their changes from HEAD" with what follows (the files, a commit and its revert,
  local changes, the Undo, a pushed branch), "Drop Commit" ("Drop Commits") focused, so Enter drops, Escape and Cancel
  close it.
  [@test] ../../tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`, `should drop the selected commit with the key, and say which branch it leaves without a commit`)
- Success must show "Dropped N commits" with Undo, which brings them back with their files, its description saying
  that the files did not change (a commit and its revert) and which branches have no commits left.
  [@test] ../../tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`, `should drop the selected commit with the key, and say which branch it leaves without a commit`)
  [@test] ../../src/presenters/drop.test.ts (`says that the files stayed and names the branches left without a commit`)
- A refusal must keep the commits and show "Cannot drop the commits" with why.
  [@test] ../../tests/e2e/drop-commits/drop-commits.spec.ts (`should keep the commits and say why when the drop is refused`)

### `brd drop`

- `brd drop <commit>... [--expect-head <id>] [--dry-run]` must drop the commits as above and print `dropped`,
  `files_changed`, `rebuilt_branches`, `emptied_branches`, `previous_head`, `new_head` and its `undo_token`;
  `--dry-run` builds the drop and drops it, with no ref, note or file changed and no record.
  [@test] ../../crates/bd-update/src/drop_test.rs (`drop_takes_the_commit_and_its_file_out_of_head_and_the_undo_brings_them_back`, `drop_outputs_match_the_schema`)
- In a lease of the pool, a commit its home branch has (a pick landed it) must be refused as `drop_in_home` (exit 4)
  with the two next steps: the drop in the shared checkout, then `brd lease pick`, which lands nothing and moves the
  lease past the dropped commit, so the lease can be returned.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_session_drops_its_landed_commit_in_the_shared_checkout_and_its_lease_follows`)
- The agent's rules must name `brd drop` for a commit of its own that must not land, never `git revert`, both dropped
  when a revert exists, and a commit of another session or of the user only after the user asked.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`every_refusal_is_in_the_help_of_its_command`)

## References

- [Unapply and Reapply](../branch/unapply.spec.md): the shared check and move
- [Squash Fixups](fixups.spec.md): a fixup that cancels its target drops it too
- [Git Operations](../../docs/design/git-operations.md), "Drop and unapply"
