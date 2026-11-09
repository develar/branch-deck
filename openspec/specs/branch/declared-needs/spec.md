---
name: Declared Needs
description: Requirements for the order of landing the user declares (`brd need`): the branches that must be in master before a branch lands, stored in `refs/branch-deck/needs`, met once integration detection proves them landed, refusing the Safe Push, the Dry Run and the land of the branch and of a branch built on it on every route until then, `--allow-unlanded` and Start Anyway lifting the refusal.
style: plain-1
targets:
  - crates/history-rewrite/src/needs.rs
  - crates/history-rewrite/src/store_lines.rs
  - crates/history-rewrite/src/snapshot.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/sync-core/src/sync.rs
  - crates/sync-core/src/sync_inputs.rs
  - crates/sync-core/src/companions.rs
  - crates/sync-core/src/repository_config.rs
  - crates/sync-core/src/config_snapshot.rs
  - crates/bd-rewrite/src/companion_command.rs
  - crates/app-api/src/app/repository_config.rs
  - src/components/settings/LandingPage.tsx
  - src/components/settings/CompanionsSetting.tsx
  - src/components/repository/RepositorySelector.tsx
  - src/hooks/useConfigWrite.ts
  - src/hooks/useDeclareNeed.ts
  - src/hooks/useBranchContextActions.ts
  - src/components/branch/InlineNeedInput.tsx
  - src/presenters/actions.ts
  - src/model/store.ts
  - src/stores/branch-sync.ts
  - src/presenters/repository-links.ts
  - crates/domain-types/src/session.rs
  - crates/domain-types/src/lib.rs
  - crates/repository-model/src/table.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-types/src/actions.rs
  - crates/repository-model/src/action_view.rs
  - crates/domain-types/src/landing.rs
  - crates/domain-types/src/patronus.rs
  - crates/domain-types/src/rewrite.rs
  - crates/domain-types/src/error.rs
  - crates/domain-policy/src/declared_needs.rs
  - crates/domain-policy/src/landing_needs.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/land.rs
  - crates/trial-check/src/landing_gate.rs
  - crates/repository-model/src/branch_table.rs
  - crates/repository-model/src/derive.rs
  - crates/repository-model/src/patronus_view.rs
  - crates/repository-model/src/warm_start.rs
  - crates/operations/src/land.rs
  - crates/operations/src/rewrite.rs
  - crates/bd-rewrite/src/need_command.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-remote/src/safe_push_command.rs
  - crates/bd-remote/src/land_command.rs
  - crates/bd-cli/src/agent_help.rs
  - src/presenters/landing.ts
  - src/presenters/next-step.ts
  - src/presenters/rewrite.ts
  - src/hooks/useStackActions.ts
  - src/hooks/useSafePush.ts
---

# Declared Needs

Status: Implemented
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
- Say it where the found needs are said (row 1′ of [Next Step](../next-step/spec.md)), as soon as the sync read the store.
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
  [@test] crates/sync-core/src/needs_test.rs (`a_need_is_one_line_of_several_names_and_a_removal_keeps_the_others`, `a_need_names_a_branch_of_this_repository_or_one_of_a_companion`)
- A write must change only the line it names: the lines of another worktree, and the lines it can not read, stay as
  they are.
  [@test] crates/sync-core/src/needs_test.rs (`a_write_keeps_the_lines_of_another_worktree_and_the_lines_it_can_not_read`)
- The ref must be in the inputs of a sync, so a need declared in `brd` syncs the window on its next focus.
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Declare and take back

- `brd need <branch> <needed>…` must add the names to the line of a branch that groups commits ahead of the baseline,
  its store commit written before the repository lock and moved under it with compare-and-swap on the commit it read,
  the one ref it writes, and rewrite no commit; the same name again writes nothing (`changed: false`).
  `brd need <branch> --remove <needed>…` takes names back, the line with its last name. An unknown branch, a need of
  the branch on itself, a name that is no branch name (`a:b:c`) and the removal of a name never declared are refused.
  [@test] crates/bd-rewrite/src/need_test.rs (`a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line`, `a_need_of_an_unknown_branch_a_self_need_and_a_need_that_was_never_declared_are_refused`)
  [@test] crates/sync-core/src/needs_test.rs (`a_need_of_an_unknown_branch_of_itself_or_of_a_name_that_is_no_branch_is_refused`)
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_need_and_its_removal_lock_only_the_write`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_need_and_its_removal`)
- A need must wait for the repository lock as a park does: `brd need` up to 10 s, then `locked`.
  [@test] crates/sync-core/src/needs_test.rs (`a_need_waits_for_the_repository_lock`)

### Rename and landing

- A rename must carry a need both ways, in its regroup transaction: as the branch that needs (the line moves) and as
  the name it needs (the value is rewritten); the undo token gets `needs:N1..N2` after `parked:`, and its undo moves the
  line back. A rename that would make a branch need itself drops the line; a rename of a branch without needs writes
  no line.
  [@test] crates/sync-core/src/needs_test.rs (`a_rename_carries_the_need_as_the_branch_and_as_the_name_it_needs_and_its_undo_moves_it_back`, `a_rename_of_a_branch_without_needs_leaves_the_store_alone`)
- A sync must drop a needed name once its newest archived branch is integrated (`integrated_children`, in the one drop
  of the stores, best effort, only while the lock is free): only that name, the line with its last name; a name of a
  companion once the last sync of the companion archived it as integrated ([below](#companion-repositories)). An undo
  of a rename passes over those drops.
  [@test] crates/sync-core/src/needs_test.rs (`a_sync_keeps_a_need_it_can_not_prove_landed_and_a_drop_takes_only_the_name_that_landed`, `an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### The rule

- A declared need that is not in master must refuse the start of its branch (`LandingRefusal::UnlandedNeed`, the third
  refusal of the landing gate) with the state of the needed branch: `active` (it groups commits here), `unapplied`
  (an unapply keeps it), `missing` (no branch of that name), `companionUnreadable` (its companion can not be read).
  No commit proves it and no build fails: the refusal has no `file:line` and no commit. One rule decides it for the app
  and `brd` (`domain_policy::declared_needs`), over the lines of the table, the states of the needed branches and the
  chains of the stack.
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_that_needs_an_active_branch_is_refused_its_own_need_without_a_fix`, `every_state_of_a_needed_branch_refuses_the_start_and_names_itself`, `a_need_of_a_branch_that_landed_refuses_nothing`, `nothing_declared_refuses_nothing`)
- A branch built on a branch that declared a need must inherit it: they land in one robot. The refusal names the
  nearest branch of the chain that declared it (`inherited_from`) and the fix `Unstack`, which lands the branch alone.
  A branch needed by the branch it is built on inherits no need of itself; needs are never followed through needs, so
  two branches that need each other are each refused once.
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_built_on_one_that_needs_inherits_the_need_and_unstacking_it_is_the_fix`, `a_need_of_a_branch_further_down_the_chain_is_inherited_once_and_the_nearest_owner_names_the_fix`, `a_branch_that_is_needed_by_the_branch_it_is_built_on_inherits_no_need_of_itself`, `two_branches_that_need_each_other_are_each_refused_once`, `a_branch_of_another_worktree_keeps_its_declared_need_although_no_chain_names_it`)
- While a robot lands the needed branch, the refusal must name it (`landing_in`), as a missing declaration does.
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_robot_that_lands_the_needed_branch_already_is_named_by_the_refusal`)
- The whole stack must be refused the needs of every branch it lands.
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_of_a_member_refuses_the_start_of_the_whole_stack`)

### The start

- `brd safe-push`, `--patronus-dry-run` and `brd land` must refuse a start the rule refuses with `unlanded_need`
  (exit 4), nothing pushed, no robot started: the message names what lands first, `brd need <branch> --remove
  <needed>` of the branch that declared it, and `--allow-unlanded`, which the agent passes only when the user says the
  order no longer holds; an inherited need also names `brd unstack <branch>` as its next step, after the user
  approves. `--dry-run` lists the same refusal in `refusals`. `--allow-unlanded` lifts this refusal only.
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`, `a_removed_need_refuses_nothing`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- The app's start must refuse it the same way (`domain_policy::land::decide` over the same refusals): its popover says
  why, and Start Anyway passes `allowUnlanded`.
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
  [@test] src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`)

### Every route

- The refusals must be known as soon as the sync read the store, on every route into master (Patronus, a push, a
  merge of the pull request) and before Patronus or GitHub answer: the table of the sync carries the lines
  (`BranchTable::needs`, the frame of the warm start too), and the session derives the refusals beside the found needs
  (`repository_model::derive::declared_needs_of`).
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- The next step must be row 1′ (`needs_branch`) while a declared need refuses the start, before the wait for an answer
  of Patronus or of the gate of the route (row 0); a reason of the subject (row 1) still comes first. A found need
  waits for the answers as before.
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_names_a_declared_need_before_any_answer_and_after_a_reason_of_the_subject`)
- `brd status` must list the refusals in `landing_needs` of the branch with the code `unlanded_need`, `needs_branch`,
  `state`, `inherited_from` and `fix`: on every route, the declared ones first; off Space only a branch a declared need
  refuses has `landing_needs`, and `--brief` none.
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### The app

- Row 1′ must name the branch the user declared lands first ("Needs docs"), its hover the reason; a click opens the
  Safe Push popover, which says it and starts anyway; for an inherited need the click runs the unstack ("Unstack ui
  from auth"), which the toast of a refused start offers too.
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the user declared lands first, opens the Safe Push, and unstacks a branch that inherits the need`)
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and take it back`)
- The Safe Push popover must say the reason before the start, and Start Anyway must start with `allowUnlanded`.
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should say in the Safe Push popover what lands first, and start anyway with the override of the declared order`)
- The branch menu (right-click and "⋯") must offer "Add Need…" beside "Park Branch…", disabled without commits: a form
  under the row, "What lands before auth", its field focused ("A branch of this repository, or companion:branch"),
  the other branches of the table as chips, Enter or Add Need writes it (the rewrite `need`, queued as every rewrite,
  then the sync and the toast "Declared that docs lands before auth" with Undo, the store back as it was); a refusal
  of the core keeps the form with the typing. Of a branch that declared needs it must offer "Remove Need: docs", for
  two or more a submenu "Remove Need ›" of them, each the rewrite `unneed`. Rust decides both actions
  (`ActionId::AddNeed`, `RemoveNeed`, `domain_policy::actions`) over the declared needs of the branch, which the view
  carries in its structure (`BranchesGrouped::needs` → `BranchStructure.needs`).
  [@test] crates/domain-policy/src/actions_test.rs (`a_need_is_added_to_a_branch_with_commits_and_removed_while_one_is_declared`)
  [@test] crates/repository-model/src/derive_test.rs (`the_structure_of_a_branch_names_the_needs_it_declared`)
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and take it back`)
- The reason must name the state of the needed branch in the words below, and the robot that lands it.
  [@test] src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`, `says why a branch built on the one that declared the need inherits it, and unstacks it to land it alone`)
- A need written by the app's rewrites (`Rewrite(Need)`, `Rewrite(Unneed)`) must be worded as the other rewrites:
  "Declaring what lands before auth", "Cannot declare that docs lands before auth", "Removed the need of auth for docs".
  [@test] src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)

### Companion repositories

- A primary must list its companions in its local config (`branchdeck.companion.<name>.path`, the name as it is
  written), each companion its primary (`branchdeck.primary`), both the paths of the main worktrees, so its leases and
  linked worktrees name the same repository. `brd companion add <path> [--name]`, `brd companion remove <name>`,
  the section Companions of Settings → Landing (the tab of the repository) and its Add Companion… (the folder dialog,
  named by the directory) and Remove write both sides through one writer (`repository_config::write_companion`);
  `brd companion list` and the page read them with the problem of each path. A companion has one primary, a primary is no companion, a repository no companion of itself,
  a name is letters, digits, `-`, `_` and `.`; a removal unsets the primary of the companion only while it names this
  repository.
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)
- The main-window project selector shows the selected repository's links from its session: a primary labels the group
  "Companions of app", a companion "Primary and companions" with the primary first. A linked repository appears only
  once, apart from the remaining "Recent" projects; with no links the existing list has no labels. The trigger keeps
  the selected path and its width. Opening the pop-up starts no Git process; a sync carries the linked paths and why
  one is unreadable. A linked project outside Recent can be selected; it shows the full path under its shortened path
  until its first sync, then the usual sync data. A path that is gone or no Git repository is disabled and says
  "Not readable: …".
  [@test] src/presenters/repository-links.test.ts (`puts linked repositories after Recent and removes their duplicate recent entries`, `names a companion's primary group and keeps the primary first`, `returns one unnamed section when there are no links`)
  [@test] tests/e2e/repository/header.spec.ts (`should show the unlabelled recent-project list when there are no companions`)
  [@test] tests/e2e/repository/companion-links.spec.ts (`should show and switch to a companion that has never been opened @webkit`)
- A need of a companion must name it as `<name>:<branch>`: in a primary one of its companions, in a companion its
  primary (by the name of its directory) or another companion of that primary.
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)
- How a branch of a companion stands must be read of what the last sync of the companion left, never by a sync of it:
  its config, one listing of its branches under its prefix (of its own config, else this one's) and the integration
  notes of their tips, three processes per companion, none while no need names one. Grouped is `active`, unapplied
  `unapplied`, a newest archived branch integrated is landed, anything else `missing`; a companion that is gone, no git
  repository or no companion of this repository is `companionUnreadable` with the reason, and neither `brd status`
  nor the sync fails on it. The sync reads them in its detection, drops the landed ones from the store and passes the
  rest to the table; `brd status` reads them without a sync.
  [@test] crates/sync-core/src/companions_test.rs (`a_branch_of_a_companion_is_active_unapplied_integrated_or_missing_as_its_last_sync_left_it`, `a_companion_that_is_gone_or_unknown_makes_its_needs_unreadable_and_nothing_fails`)
  [@test] crates/bd-remote/src/status_test.rs (`a_need_of_a_branch_of_a_companion_refuses_until_the_companion_landed_it`)
- `brd status --companions` must add the status of every repository a need here can name, each by its own context
  and its prefix as the companion reader takes it (`companions`: `name`, `path`, `status`, or the `error` why it can
  not be read); without the flag nothing of the companions is read.
  [@test] crates/bd-remote/src/status_test.rs (`status_with_companions_tells_the_status_of_each_companion_and_why_one_can_not_be_read`)
- A write of the companions must sync the main window again (`repository_config_changed`, `companions`): the states
  of the branches a need names come with the sync.
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)

## User Experience

| State of the needed branch | The reason (`subject`: "auth" or "the whole auth stack") |
| --- | --- |
| `active` | "You declared that docs lands before auth: it is not in master yet, so a start of auth lands them out of order." |
| `unapplied` | "You declared that docs lands before auth, and it is unapplied: it cannot land at all. Reapply it, or remove the need." |
| `missing` | "You declared that docs lands before auth, and no branch of that name is here: a start of auth lands what you ordered last. Remove the need, or sync the branch that carries it." |
| `companionUnreadable` | "You declared that docs lands before auth, and the companion ci-config is not readable: ‹reason›. Fix it in Settings → Landing, or remove the need." |
| inherited | adds "ui is built on auth, which needs it: they land in one robot. Unstack ui from auth to land it alone." |
| a robot lands it | adds "docs is landing in its Safe Push, which tests it without this start: a start beside it fails too." |

- `brd need auth docs`, `brd need auth --remove docs`; `brd status` → `landing_needs: [{code: "unlanded_need", …}]`;
  `brd safe-push auth` → `unlanded_need`, `--allow-unlanded` only on the user's word.
- Settings → Landing, the tab of the repository, section Companions: the list (name, path, "Not readable: …"), Add Companion… (the folder dialog), Remove; empty:
  "No companion repositories. A need can name a branch of another repository once you add one here."; of a companion:
  "A companion of app: a need here names its branches as app:branch. Add companions there."
- `brd companion add ../ci-config`, `brd need deploy ci-config:prepare`, `brd status --companions`.
- The menu of a row: "Add Need…" → the form "What lands before JIRA-456", chips of the other branches → Enter → toast
  "Declared that feature-auth lands before JIRA-456" · Undo, the row "Needs feature-auth"; "Remove Need: feature-auth"
  (two or more: "Remove Need ›").

## Open Questions / Risks

- A branch of a companion lands when the companion syncs after it landed: until then its need still refuses here. The
  row says "Needs ci-config:prepare", and `brd sync` in the companion meets it.
- A rename of a branch in a companion does not carry the needs of other repositories that name it (their stores are
  theirs): such a need becomes `missing`, and the user declares it again.
- A need of a branch that is nowhere stays until the user removes it (`missing`), as a stack parent of a gone branch.
