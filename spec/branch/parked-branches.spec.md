---
name: Parked Branches
description: Requirements for parked branches, the branches a user keeps on the local branch without pushing or landing them, stored in `refs/branch-deck/parked`, refused by every push and kept out of Your Move.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/parked.rs
  - ../../crates/history-rewrite/src/store_lines.rs
  - ../../crates/history-rewrite/src/snapshot.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/sync-core/src/parked.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/sync_inputs.rs
  - ../../crates/sync-types/src/lib.rs
  - ../../crates/bd-cli/src/park_command.rs
  - ../../crates/bd-cli/src/status.rs
  - ../../crates/bd-cli/src/push_command.rs
  - ../../crates/bd-cli/src/safe_push_command.rs
  - ../../crates/bd-cli/src/error.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../src-tauri/src/commands/parked_branches.rs
---

# Parked Branches

Status: Implemented
Date: 2026-10-20

## Summary

Some commits stay on the local branch only because they must not land yet: they wait for a proper fix elsewhere, a
workaround the user needs locally, a change kept for a later review. Before, such a branch looked like any other: a
filled node, a move in Your Move, a Safe Push offered on its row. A user had to remember not to press it, and an agent
running `bd push --all` pushed it.

A parked branch is a virtual branch the user keeps local. Its commits stay where they are, and the sync builds it as
before, so its conflicts still show. No push sends it, no robot lands it, and it never counts as the user's move. A
branch built on it carries its commits, so it is parked with it.

## Goals

- Keep a branch local by one action, with an optional note that says why, and let it go by another.
- Make no push, Safe Push or Dry Run send it, in the app and in `bd`, and so no agent.
- Keep it out of every count of the user's moves.
- Store it as the other per-branch facts of Branch Deck: shared by every worktree, versioned, never pushed.

## Non-goals

- Park one commit. The unit is the virtual branch; a commit is moved into a branch of its own first (Move to Branch).
- Take the commits out of HEAD. That is unapply ([Unapply](unapply.spec.md)); a parked branch stays in the build.
- Stop a merge on GitHub of a pull request that is open already, or a robot that runs: parking is no remote state.
- Push or fetch the store. A new clone or another machine does not see the parked branches.

## Requirements

### Store

- The parked branches must live in `refs/branch-deck/parked`: a commit whose tree holds the file `parked`, one
  `branch<TAB>note` line per branch, sorted, the note one line or empty. Each write must be a new commit on the
  previous one. It is read in the `cat-file --batch` of the issue groups and the stack parents, so the grouping inputs
  still cost two processes, and the sync sends the map in `BranchesGrouped`.
  [@test] ../../crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_park_and_an_unpark`, `an_unchanged_sync`)
- A line whose key is not a branch name, or a branch listed twice, must be a problem: never used, and kept as it is by
  a write of another key.
  [@test] ../../crates/sync-core/src/parked_test.rs (`unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are`)
- The store must decide no group: a sync builds a parked branch as any other.
  [@test] ../../crates/sync-core/src/parked_test.rs (`a_sync_keeps_every_line_it_can_not_prove_dead`)
- The ref must be in the fingerprint of the sync inputs, so a park in `bd` syncs the window on its next focus.
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Park and unpark

- A park must write the line of a branch that groups commits ahead of the baseline, under the repository lock, with
  compare-and-swap on the commit it read, and rewrite no commit. A park of a parked branch changes its note; the same
  line again writes nothing. The note is trimmed; two lines or more than 500 characters are refused.
  [@test] ../../crates/sync-core/src/parked_test.rs (`park_writes_a_line_without_a_rewrite_and_unpark_drops_it`, `park_refuses_an_unknown_branch_and_a_note_of_two_lines_unpark_one_not_parked`)
- A park must wait for the repository lock as a rewrite does, and fail with `locked` after the wait.
  [@test] ../../crates/sync-core/src/parked_test.rs (`park_waits_for_the_repository_lock`)
- An unpark must drop the line, also of a branch that groups no commit here; a branch that is not parked is
  `not_parked`.
  [@test] ../../crates/sync-core/src/parked_test.rs (`unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are`)
- The app's commands must report the note before and after.
  [@test] ../../crates/sync-core/src/parked_test.rs (`the_app_wrapper_reports_the_notes`)

### Parked with a parent

- A branch built on a parked branch (its effective base, or the declared parent of a blocked branch, up the chain)
  must be parked too, with the note of the nearest parked ancestor, which it names. Only the parked branch itself has
  a line.
  [@test] ../../crates/sync-core/src/parked_test.rs (`a_branch_built_on_a_parked_one_is_parked_with_it`)

### Rename and cleanup

- A rename must carry the line to the new name in its regroup transaction, a merge into a parked branch keeping the
  note of the target; the undo token gets `parked:P1..P2` after `stacks:`, and its undo moves the line back. A rename
  of a branch that is not parked writes no line.
  [@test] ../../crates/sync-core/src/parked_test.rs (`a_rename_carries_the_line_and_its_undo_moves_it_back`)
- A sync must drop the line of a branch without commits here once its newest archived branch is integrated, as the
  stack parents (`integrated_children`), best effort and with its own drop message; every other line stays. An undo of
  a rename passes over those drops; a park after the rename makes it `parked_moved`.
  [@test] ../../crates/sync-core/src/parked_test.rs (`a_sync_keeps_every_line_it_can_not_prove_dead`, `an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)

### `bd`

- `bd park <branch> [--note]` and `bd unpark <branch>` must print one object (`park` in `bd schema`); `bd status`
  names `parked: {note, with}` per branch and `idle_parked`, `--brief` too.
  [@test] ../../crates/bd-cli/src/park_test.rs (`park_and_unpark_print_the_note_and_status_names_the_parked_branches`)
- `bd push` must refuse a named parked branch, or one built on a parked branch, with `parked` (exit 4), nothing
  pushed; with `--all` or `--with-children` it leaves it out (`skipped`, `code: parked`) and pushes the others, and
  refuses only when nothing else is left. The stack branch of a stack with a parked member stays local.
  [@test] ../../crates/bd-cli/src/push_test.rs (`a_parked_branch_and_what_is_built_on_it_are_never_pushed`)
- `bd safe-push` must refuse a parked branch, a branch built on one and a whole stack with a parked member with
  `parked`, `--check` and `--patronus-dry-run` included, before Patronus is asked.
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`a_parked_branch_and_a_branch_built_on_it_start_nothing`)
- `bd help agent` and the skill must tell an agent never to push or unpark a parked branch on its own.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)
