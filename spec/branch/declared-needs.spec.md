---
name: Declared Needs
description: Requirements for the order of landing the user declares (`brd need`): the branches that must be in master before a branch lands, stored in `refs/branch-deck/needs`, met once integration detection proves them landed, refusing the Safe Push, the Dry Run and the land of the branch and of a branch built on it on every route until then, `--allow-unlanded` and Start Anyway lifting the refusal.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/needs.rs
  - ../../crates/history-rewrite/src/store_lines.rs
  - ../../crates/history-rewrite/src/snapshot.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/sync_inputs.rs
  - ../../crates/domain-types/src/landing.rs
  - ../../crates/domain-types/src/patronus.rs
  - ../../crates/domain-types/src/rewrite.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/domain-policy/src/declared_needs.rs
  - ../../crates/domain-policy/src/landing_needs.rs
  - ../../crates/domain-policy/src/next_step.rs
  - ../../crates/domain-policy/src/land.rs
  - ../../crates/trial-check/src/landing_gate.rs
  - ../../crates/repository-model/src/branch_table.rs
  - ../../crates/repository-model/src/derive.rs
  - ../../crates/repository-model/src/patronus_view.rs
  - ../../crates/repository-model/src/warm_start.rs
  - ../../crates/operations/src/land.rs
  - ../../crates/operations/src/rewrite.rs
  - ../../crates/bd-rewrite/src/need_command.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/bd-remote/src/safe_push_command.rs
  - ../../crates/bd-remote/src/land_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../src/presenters/landing.ts
  - ../../src/presenters/next-step.ts
  - ../../src/presenters/rewrite.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/hooks/useSafePush.ts
---

# Declared Needs

Status: Implemented (the branches of this repository; companion repositories and the menu of the row are next)
Date: 2026-11-07

## Summary

Some changes land in an order no build can see: a test that downloads its data before the CI step that prepared it
goes away, a config file read by another repository. The order lived in the commit messages ("deploy this only
after…"), and `brd safe-push` started the wrong branch first; a branch stacked on the first one landed both in one
robot and skipped the step between them (issue #197).

The user declares the order: `brd need remove-prepare-task test-data-download` says that `test-data-download` lands
before `remove-prepare-task`. Until it is in master, no Safe Push, Dry Run or land of `remove-prepare-task`, nor of a
branch built on it, starts; the row names it ("Needs test-data-download") on every route, before any answer of Patronus
or GitHub. Once integration detection proves the needed branch landed, the sync drops the need.

## Goals

- Declare and take back the order of two branches in one command, stored as the other per-branch facts of Branch Deck:
  shared by every worktree, versioned, never pushed.
- Refuse every start that lands a branch before what it needs, in the app and in `brd`, and so for every agent; let
  the user's word lift the refusal.
- Say it where the found needs are said (row 1′ of [Next Step](next-step.spec.md)), as soon as the sync read the store.
- Meet the need by itself once the needed branch landed.

## Non-goals

- Start the next Safe Push once the needed one merged.
- Wait for CI to pick up a changed configuration: "landed" is "integrated into the baseline".
- Follow needs through needs: a need of a needed branch is that branch's own.
- A companion of two primaries.

## Requirements

### Store

- The declared needs must live in `refs/branch-deck/needs`: a commit whose tree holds the file `needs`, one
  `branch<TAB>needed needed…` line per branch, sorted, the needed names sorted and space-separated, each a branch of this
  repository or `<companion>:<branch>`. It is read in the `cat-file --batch` of the other stores, so the grouping
  inputs still cost two processes, and the report of the sync carries the lines to the table of the session
  (`BranchTable::needs`); the page gets the refusals derived of them, never the lines.
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_need_is_one_line_of_several_names_and_a_removal_keeps_the_others`, `a_need_names_a_branch_of_this_repository_or_one_of_a_companion`)
- A write must change only the line it names: the lines of another worktree, and the lines it can not read, stay as
  they are.
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_write_keeps_the_lines_of_another_worktree_and_the_lines_it_can_not_read`)
- The ref must be in the inputs of a sync, so a need declared in `brd` syncs the window on its next focus.
  [@test] ../../crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Declare and take back

- `brd need <branch> <needed>…` must add the names to the line of a branch that groups commits ahead of the baseline,
  its store commit written before the repository lock and moved under it with compare-and-swap on the commit it read,
  the one ref it writes, and rewrite no commit; the same name again writes nothing (`changed: false`).
  `brd need <branch> --remove <needed>…` takes names back, the line with its last name. An unknown branch, a need of
  the branch on itself, a name that is no branch name (`a:b:c`) and the removal of a name never declared are refused.
  [@test] ../../crates/bd-rewrite/src/need_test.rs (`a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line`, `a_need_of_an_unknown_branch_a_self_need_and_a_need_that_was_never_declared_are_refused`)
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_need_of_an_unknown_branch_of_itself_or_of_a_name_that_is_no_branch_is_refused`)
  [@test] ../../crates/history-rewrite/src/regroup_test.rs (`a_need_and_its_removal_lock_only_the_write`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`a_need_and_its_removal`)
- A need must wait for the repository lock as a park does: `brd need` up to 10 s, then `locked`.
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_need_waits_for_the_repository_lock`)

### Rename and landing

- A rename must carry a need both ways, in its regroup transaction: as the branch that needs (the line moves) and as
  the name it needs (the value is rewritten); the undo token gets `needs:N1..N2` after `parked:`, and its undo moves the
  line back. A rename that would make a branch need itself drops the line; a rename of a branch without needs writes
  no line.
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_rename_carries_the_need_as_the_branch_and_as_the_name_it_needs_and_its_undo_moves_it_back`, `a_rename_of_a_branch_without_needs_leaves_the_store_alone`)
- A sync must drop a needed name once its newest archived branch is integrated (`integrated_children`, in the one drop
  of the stores, best effort, only while the lock is free): only that name, the line with its last name. A name of a
  companion stays: this repository can not prove a branch of another one landed. An undo of a rename passes over
  those drops.
  [@test] ../../crates/sync-core/src/needs_test.rs (`a_sync_keeps_a_need_it_can_not_prove_landed_and_a_drop_takes_only_the_name_that_landed`, `an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)
  [@test] ../../crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### The rule

- A declared need that is not in master must refuse the start of its branch (`LandingRefusal::UnlandedNeed`, the third
  refusal of the landing gate) with the state of the needed branch: `active` (it groups commits here), `unapplied`
  (an unapply keeps it), `missing` (no branch of that name), `companionUnreadable` (its companion can not be read).
  No commit proves it and no build fails: the refusal has no `file:line` and no commit. One rule decides it for the app
  and `brd` (`domain_policy::declared_needs`), over the lines of the table, the states of the needed branches and the
  chains of the stack.
  [@test] ../../crates/domain-policy/src/declared_needs_test.rs (`a_branch_that_needs_an_active_branch_is_refused_its_own_need_without_a_fix`, `every_state_of_a_needed_branch_refuses_the_start_and_names_itself`, `a_need_of_a_branch_that_landed_refuses_nothing`, `nothing_declared_refuses_nothing`)
- A branch built on a branch that declared a need must inherit it: they land in one robot. The refusal names the
  nearest branch of the chain that declared it (`inherited_from`) and the fix `Unstack`, which lands the branch alone.
  A branch needed by the branch it is built on inherits no need of itself; needs are never followed through needs, so
  two branches that need each other are each refused once.
  [@test] ../../crates/domain-policy/src/declared_needs_test.rs (`a_branch_built_on_one_that_needs_inherits_the_need_and_unstacking_it_is_the_fix`, `a_need_of_a_branch_further_down_the_chain_is_inherited_once_and_the_nearest_owner_names_the_fix`, `a_branch_that_is_needed_by_the_branch_it_is_built_on_inherits_no_need_of_itself`, `two_branches_that_need_each_other_are_each_refused_once`, `a_branch_of_another_worktree_keeps_its_declared_need_although_no_chain_names_it`)
- While a robot lands the needed branch, the refusal must name it (`landing_in`), as a missing declaration does.
  [@test] ../../crates/domain-policy/src/declared_needs_test.rs (`a_robot_that_lands_the_needed_branch_already_is_named_by_the_refusal`)
- The whole stack must be refused the needs of every branch it lands.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`a_declared_need_of_a_member_refuses_the_start_of_the_whole_stack`)

### The start

- `brd safe-push`, `--patronus-dry-run` and `brd land` must refuse a start the rule refuses with `unlanded_need`
  (exit 4), nothing pushed, no robot started: the message names what lands first, `brd need <branch> --remove
  <needed>` of the branch that declared it, and `--allow-unlanded`, which the agent passes only when the user says the
  order no longer holds; an inherited need also names `brd unstack <branch>` as its next step, after the user
  approves. `--dry-run` lists the same refusal in `refusals`. `--allow-unlanded` lifts this refusal only.
  [@test] ../../crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`, `a_removed_need_refuses_nothing`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- The app's start must refuse it the same way (`domain_policy::land::decide` over the same refusals): its popover says
  why, and Start Anyway passes `allowUnlanded`.
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
  [@test] ../../src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`)

### Every route

- The refusals must be known as soon as the sync read the store, on every route into master (Patronus, a push, a
  merge of the pull request) and before Patronus or GitHub answer: the table of the sync carries the lines
  (`BranchTable::needs`, the frame of the warm start too), and the session derives the refusals beside the found needs
  (`repository_model::derive::declared_needs_of`).
  [@test] ../../crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- The next step must be row 1′ (`needs_branch`) while a declared need refuses the start, before the wait for an answer
  of Patronus or of the gate of the route (row 0); a reason of the subject (row 1) still comes first. A found need
  waits for the answers as before.
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`row_1_prime_names_a_declared_need_before_any_answer_and_after_a_reason_of_the_subject`)
- `brd status` must list the refusals in `landing_needs` of the branch with the code `unlanded_need`, `needs_branch`,
  `state`, `inherited_from` and `fix`: on every route, the declared ones first; off Space only a branch a declared need
  refuses has `landing_needs`, and `--brief` none.
  [@test] ../../crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)
  [@test] ../../crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### The app

- Row 1′ must name the branch the user declared lands first ("Needs docs"), its hover the reason; a click opens the
  Safe Push popover, which says it and starts anyway; for an inherited need the click runs the unstack ("Unstack ui
  from auth"), which the toast of a refused start offers too.
  [@test] ../../src/presenters/next-step.test.ts (`row 1′: names the branch the user declared lands first, opens the Safe Push, and unstacks a branch that inherits the need`)
- The reason must name the state of the needed branch in the words below, and the robot that lands it.
  [@test] ../../src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`, `says why a branch built on the one that declared the need inherits it, and unstacks it to land it alone`)
- A need written by the app's rewrites (`Rewrite(Need)`, `Rewrite(Unneed)`) must be worded as the other rewrites:
  "Declaring what lands before auth", "Cannot declare that docs lands before auth", "Removed the need of auth for docs".
  [@test] ../../src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)

## User Experience

| State of the needed branch | The reason (`subject`: "auth" or "the whole auth stack") |
| --- | --- |
| `active` | "You declared that docs lands before auth: it is not in master yet, so a start of auth lands them out of order." |
| `unapplied` | "You declared that docs lands before auth, and it is unapplied: it cannot land at all. Reapply it, or remove the need." |
| `missing` | "You declared that docs lands before auth, and no branch of that name is here: a start of auth lands what you ordered last. Remove the need, or sync the branch that carries it." |
| `companionUnreadable` | "You declared that docs lands before auth, and the companion ci-config is not readable: ‹reason›. Make it readable again, or remove the need." |
| inherited | adds "ui is built on auth, which needs it: they land in one robot. Unstack ui from auth to land it alone." |
| a robot lands it | adds "docs is landing in its Safe Push, which tests it without this start: a start beside it fails too." |

- `brd need auth docs`, `brd need auth --remove docs`; `brd status` → `landing_needs: [{code: "unlanded_need", …}]`;
  `brd safe-push auth` → `unlanded_need`, `--allow-unlanded` only on the user's word.

## Open Questions / Risks

- A need of a branch of a companion refuses nothing yet: only the companion can say how its branch stands, and the
  companions are not read yet.
- A need of a branch that is nowhere stays until the user removes it (`missing`), as a stack parent of a gone branch.
