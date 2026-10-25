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
  - ../../crates/domain-types/src/lib.rs
  - ../../crates/bd-rewrite/src/park_command.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/bd-remote/src/push_command.rs
  - ../../crates/bd-remote/src/safe_push_command.rs
  - ../../crates/repository-model/src/push_guard.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../src-tauri/src/commands/parked_branches.rs
  - ../../src/lib/parked.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/branch-next-step.ts
  - ../../src/lib/landing-node.ts
  - ../../src/lib/branch-filter.ts
  - ../../src/hooks/useLandingOverview.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useParkBranch.ts
  - ../../src/components/branch/ParkedBranchesCard.tsx
  - ../../src/components/branch/BranchDropZones.tsx
  - ../../src/components/branch/DragTargetBar.tsx
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/hooks/useBranchDropTarget.ts
  - ../../src/lib/branch-drop.ts
  - ../../src/components/branch/InlineParkBranchInput.tsx
  - ../../src/components/branch/BranchRowStatus.tsx
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
- A park or an unpark of several branches (the app's drag of a selection, an undo) must write all their lines in one
  store commit, or none: an unknown branch refuses the park of all, one that is not parked the unpark of all. The
  processes don't grow with the number of branches.
  [@test] ../../crates/sync-core/src/parked_test.rs (`several_branches_park_and_unpark_in_one_store_commit_or_not_at_all`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_park_and_an_unpark`)
- A park must wait for the repository lock as a rewrite does: `bd park` up to 10 s, then `locked`; the app's park and
  unpark as long as the holder lives, the row's "⋯" saying whom for with Stop Waiting, which ends the wait with
  nothing changed.
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
  [@test] ../../crates/bd-rewrite/src/park_test.rs (`park_and_unpark_print_the_note_and_status_names_the_parked_branches`)
- `bd push` must refuse a named parked branch, or one built on a parked branch, with `parked` (exit 4), nothing
  pushed; with `--all` or `--with-children` it leaves it out (`skipped`, `code: parked`) and pushes the others, and
  refuses only when nothing else is left. The stack branch of a stack with a parked member stays local.
  [@test] ../../crates/bd-remote/src/push_test.rs (`a_parked_branch_and_what_is_built_on_it_are_never_pushed`)
- `bd safe-push` must refuse a parked branch, a branch built on one and a whole stack with a parked member with
  `parked`, `--check` and `--patronus-dry-run` included, before Patronus is asked.
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_parked_branch_and_a_branch_built_on_it_start_nothing`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_as_bd_answers_today`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the landing cases answer as the app answers today`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_as_bd_answers_today`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the next step row cases answer as the app answers today`)
- `bd help agent` and the skill must tell an agent never to push or unpark a parked branch on its own.
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`agent_help_names_every_command_and_option_that_exists`)

### The app

- A parked branch, and every branch parked with it, must leave the branch table and its stack islands for the Parked
  card after them: its rows as in the table (expand, review, the conflict view, the keys and the preview), a stack graph
  of their own, a parked child of a branch to land a root there. The card is open by default; its collapse is kept per
  repository and hides its rows from the keys.
  [@test] ../../src/stores/branch-table-rows.test.ts (`moves a parked branch and the branches built on it into the Parked card, a stack graph of their own`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a stack root with the branches built on it, edit its note and unpark it from the menus`, `should collapse the Parked card and keep its rows out of the table`)
- The node of a parked row must be `parked`, hollow and neutral, never a move, also with a conflict; a running robot,
  a Safe Push that lands it or a landing comes first. The header, the landing overview, the chips and F2 must leave it
  out: a parked branch is in no unit, no chip but All, and no move.
  [@test] ../../src/lib/landing-node.test.ts (`is parked, never a move, also with a conflict, until something lands it already`)
  [@test] ../../src/lib/branch-filter.test.ts (`puts a parked branch into All only: it has a card of its own, no chip, no move for F2`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`)
- Nothing of it may start: its next step is row 1 of [Next Step](next-step.spec.md), Safe Push and Dry Run disabled
  with "Parked: Unpark it to push" (a branch parked with its parent: "Built on parked auth: Unpark auth to push"; the
  whole stack: "Holds parked auth: land the stack in pieces"), its push disabled with the same reason on the row, in the
  menu, the preview and the review, and so is Push Up to Here of its commits, their push arrow and their menu
  ([Stacked Branches](stacked-branches.spec.md#push-up-to-here)). Push with Children of a branch to land leaves its parked children out, and the stack
  branch of a stack that holds one stays local. Patronus is not asked for its readiness.
  [@test] ../../src/lib/parked.test.ts (`starts no Safe Push and no Dry Run of a parked branch: its step is none, on every surface`, `says why nothing starts, of a branch, of one built on it and of the whole stack`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)
  [@test] ../../src/lib/push-up-to.test.ts (`keeps a parked branch and a branch built on it local, in the words of their push button`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_as_bd_answers_today`)
  [@test] ../../src/lib/scenario-cases.test.ts (`the next step row cases answer as the app answers today`)
- The app's push must refuse it in Rust too, as `bd push` does (`repository_model::push_guard`, under the repository
  lock): a push of a parked branch, of a branch built on one, or of the stack branch of a stack that holds one pushes
  nothing ("Nothing was pushed: login is parked: IJPL-1; it stays local until the user unparks it", "… holds parked
  login; land the stack in pieces"), so a push the page did not check (a race, IPC) never sends it; an unapplied branch
  under the name of a parked one is no branch of the table and pushes.
  [@test] ../../crates/repository-model/src/push_guard_test.rs (`a_parked_branch_and_a_branch_built_on_parked_are_refused`, `an_unapplied_branch_is_checked_by_nothing_but_the_lock`)
- The row must say it: the Status cell "Parked" or "Parked with auth" (the note in its hover) instead of the push
  state, its error or conflict still shown; the name cell of the parked branch shows its note instead of the summary,
  issue keys as links.
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)
- The branch menu (right-click and "⋯") must offer "Park Branch…" (a form with the note, one line, optional; Enter
  parks) beside "Unapply…", the other way to set a branch aside, and of a parked branch "Edit Park Note…" and "Unpark Branch"; of a branch parked with its parent "Unpark
  ‹parent›". "Park Branch…" is disabled while a robot lands the branch, and says that an open pull request stays open.
  A park or an unpark is a write of the repository (it waits for a rewrite), followed by a sync, then a toast "Parked
  auth" or "Unparked auth" whose Undo puts the line back as it was. The park, the unpark and their Undo belong to the
  repository of the click: an Undo clicked after a switch of the repository puts the line back there (synced when it
  opens), never in the repository shown.
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo`, `should park a stack root with the branches built on it, edit its note and unpark it from the menus`)
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should take the park back in its repository with the Undo of its toast clicked after a switch of the repository`)
- Branches dragged from the table onto the park zone of the drag bar, or onto the Parked card, also collapsed (its
  header, its rows), must be parked at once, without a note, in one write of the store and one toast "Parked core and
  docs", whose Undo takes back every line; parked branches dragged onto the unpark zone of the drag bar must be
  unparked the same way. A branch parked with its parent is unparked only with it: dragged alone, the zone refuses it
  and says why. A drag of both kinds parks the ones that land and unparks the parked ones, each on its own zone. A
  target refuses with the reason of the menu ([Move Commits](move-commits.spec.md#drag-and-drop)).
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park a branch dragged onto the park zone of the drag bar, unpark it dragged onto the unpark zone, take both back with Undo, and drag the commits of a parked row onto a branch`, `should park the selected branches dropped on a row or the header of the Parked card at once with one Undo, and unpark a stack by its root, never by a branch parked with it`)
  [@test] ../../src/lib/branch-drop.test.ts (`parks the dragged branches that land and leaves the parked ones, refuses one a robot lands, and waits for the sync`, `unparks the dragged branches with a line of their own, the ones parked with them along, never one alone, and leaves those that land`)
- The branch selection bar must offer Park and Unpark of the selected branches, by the same rules as the zones: Park
  while some of them land, Unpark while some are parked, disabled with the reason while they can't now.
  [@test] ../../tests/e2e/parked-branches/parked-branches.spec.ts (`should park an unselected branch dropped on the collapsed Parked card and keep the selection, and park and unpark the selection with its buttons, of both kinds by the part each takes`)

## User Experience

- Park Branch… → the inline note form in the row ("Why it stays local, e.g. waits for the fix of IJPL-123
  (optional)"), Enter parks → the row moves into the Parked card → toast "Parked docs" · Undo.
- The Parked card: "Parked · 2 branches", a help popover, the rows; the node hollow and grey, Status "Parked", the
  note beside the name.
- Drag a branch row → "Park docs" in the drag bar at the bottom of the window, the Parked card ringed → drop on either
  → toast "Parked docs" · Undo, the row scrolled into view in the card; drag a parked row → "Unpark docs" in the bar.
- Select branches → Park or Unpark in the selection bar.
- `bd park docs --note '…'`, `bd unpark docs`; an agent never parks or unparks on its own.

## Open Questions / Risks

- A park does not convert an open pull request into a draft: a merge on GitHub still lands the branch. The menu says
  so; GitHub has no draft conversion in `crates/github` yet.
- The key of a line is the branch name as the grouping makes it: turning stacked branches on or off can change it, and
  a renamed issue group is carried only by Rename.
