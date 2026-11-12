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

## Purpose

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

### Requirement: Store: The needs live in one sorted line per branch

The declared needs MUST live in `refs/branch-deck/needs`: a commit whose tree holds the file `needs`, one
`branch<TAB>needed needed…` line per branch, sorted, the needed names sorted and space-separated, each a branch of this
repository or `<companion>:<branch>`.

#### Scenario: The names of one branch share its line, sorted
- **WHEN** a branch declares several needs and a removal takes one of them back
- **THEN** the store holds one line of the branch, its needed names sorted and space-separated
- **AND** the removal keeps the other names on the line
  [@test] crates/sync-core/src/needs_test.rs (`a_need_is_one_line_of_several_names_and_a_removal_keeps_the_others`)

#### Scenario: A needed name is a branch here or of a companion
- **WHEN** a needed name is read
- **THEN** it is a branch of this repository or `<companion>:<branch>`, and no other text is a name
  [@test] crates/sync-core/src/needs_test.rs (`a_need_names_a_branch_of_this_repository_or_one_of_a_companion`)

### Requirement: Store: The store is read in the batch of the other stores

The store MUST be read in the `cat-file --batch` of the other stores, so the grouping inputs still cost two processes.

#### Scenario: The grouping inputs of a sync
- **WHEN** a sync reads its grouping inputs
- **THEN** the store of the needs is read in the `cat-file --batch` of the other stores
- **AND** the grouping inputs still cost two processes

### Requirement: Store: The table carries the lines, the page only the refusals

The report of the sync MUST carry the lines to the table of the session (`BranchTable::needs`); the page MUST get the
refusals derived of them, never the lines.

#### Scenario: A sync reports the declared needs
- **WHEN** a sync read the store
- **THEN** its report carries the lines to the table of the session (`BranchTable::needs`)
- **AND** the page gets the refusals derived of them, never the lines

### Requirement: Store: A write changes only the line it names

A write MUST change only the line it names: the lines of another worktree, and the lines it can not read, stay as they
are.

#### Scenario: Lines of another worktree and lines it can not read
- **GIVEN** the store holds a line of another worktree and a line it can not read
- **WHEN** a need is written
- **THEN** only the line it names changes
- **AND** the line of another worktree and the line it can not read stay as they are
  [@test] crates/sync-core/src/needs_test.rs (`a_write_keeps_the_lines_of_another_worktree_and_the_lines_it_can_not_read`)

### Requirement: Store: The ref is an input of a sync

The ref MUST be in the inputs of a sync, so a need declared in `brd` syncs the window on its next focus.

#### Scenario: A need declared in brd
- **WHEN** a need is declared in `brd`
- **THEN** the inputs of a sync change with the ref
- **AND** the window syncs on its next focus
  [@test] crates/sync-core/src/sync_inputs_test.rs (`changes_with_what_a_sync_reads_and_not_with_what_it_writes`)

### Requirement: Declare and take back: `brd need` adds the names to the line of a branch

`brd need <branch> <needed>…` MUST add the names to the line of a branch that groups commits ahead of the baseline; the
same name again writes nothing (`changed: false`).

#### Scenario: A branch declares what lands before it and a second need joins its line
- **GIVEN** a branch that groups commits ahead of the baseline
- **WHEN** `brd need <branch> <needed>…` runs
- **THEN** the names join the line of the branch
- **AND** the same name again writes nothing (`changed: false`)
  [@test] crates/bd-rewrite/src/need_test.rs (`a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line`)

### Requirement: Declare and take back: `--remove` takes names back

`brd need <branch> --remove <needed>…` MUST take names back, the line with its last name.

#### Scenario: A removal of a name and of the last name
- **WHEN** `brd need <branch> --remove <needed>…` runs
- **THEN** the names are taken back from the line of the branch
- **AND** the removal of its last name takes the line
  [@test] crates/bd-rewrite/src/need_test.rs (`a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line`)

### Requirement: Declare and take back: A need writes only its store under the lock

The store commit of a need MUST be written before the repository lock and moved under it with compare-and-swap on the
commit it read, the one ref it writes; a need MUST rewrite no commit.

#### Scenario: The lock holds only the write
- **WHEN** a need or its removal runs
- **THEN** its store commit is written before the repository lock
- **AND** it is moved under the lock with compare-and-swap on the commit it read, the one ref it writes
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_need_and_its_removal_lock_only_the_write`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_need_and_its_removal`)

#### Scenario: No commit is rewritten
- **WHEN** a need is declared or removed
- **THEN** HEAD stays as it was: only the store moved
  [@test] crates/bd-rewrite/src/need_test.rs (`a_branch_declares_what_lands_before_it_and_a_second_need_joins_its_line`)

### Requirement: Declare and take back: A need that names no valid case is refused

An unknown branch that would need, a need of the branch on itself, a name that is no branch name (`a:b:c`) and the
removal of a name never declared MUST be refused; a needed branch that is nowhere yet is no refusal.

#### Scenario: An unknown branch, a self need and a removal never declared
- **WHEN** `brd need` names an unknown branch as the one that needs, a need of the branch on itself, or the removal of a name never declared
- **THEN** it is refused
- **AND** a need of a branch that is nowhere yet is declared: the order can be declared before the branch is synced
  [@test] crates/bd-rewrite/src/need_test.rs (`a_need_of_an_unknown_branch_a_self_need_and_a_need_that_was_never_declared_are_refused`)

#### Scenario: The core refuses a name that is no branch name too
- **WHEN** a need names an unknown branch as the one that needs, the branch itself, a name that is no branch name (`a:b:c`), or removes a name never declared
- **THEN** it is refused and no store commit is written
  [@test] crates/sync-core/src/needs_test.rs (`a_need_of_an_unknown_branch_of_itself_or_of_a_name_that_is_no_branch_is_refused`)

### Requirement: Declare and take back: A need waits for the repository lock as a park does

A need MUST wait for the repository lock as a park does: `brd need` up to 10 s, then `locked`.

#### Scenario: Another process holds the lock
- **GIVEN** another process holds the repository lock
- **WHEN** `brd need` runs
- **THEN** it waits up to 10 s, then fails with `locked`
  [@test] crates/sync-core/src/needs_test.rs (`a_need_waits_for_the_repository_lock`)

### Requirement: Rename and landing: A rename carries a need both ways

A rename MUST carry a need both ways, in its regroup transaction: as the branch that needs (the line moves) and as the
name it needs (the value is rewritten). A rename that would make a branch need itself MUST drop the line; a rename of a
branch without needs MUST write no line.

#### Scenario: A rename of the branch that needs and of the branch it needs
- **WHEN** the branch that needs is renamed
- **THEN** its line moves to the new name, in the regroup transaction
- **AND** a rename of the branch it needs rewrites the value
- **AND** a rename that would make a branch need itself drops the line
  [@test] crates/sync-core/src/needs_test.rs (`a_rename_carries_the_need_as_the_branch_and_as_the_name_it_needs_and_its_undo_moves_it_back`)

#### Scenario: A rename of a branch without needs
- **WHEN** a branch that neither needs nor is needed is renamed
- **THEN** the rename writes no line
  [@test] crates/sync-core/src/needs_test.rs (`a_rename_of_a_branch_without_needs_leaves_the_store_alone`)

### Requirement: Rename and landing: The undo of a rename moves the line back

The undo token of a rename that carries a need MUST get `needs:N1..N2` after `parked:`, and its undo MUST move the line
back.

#### Scenario: The undo of a rename of the branch that needs
- **WHEN** a rename carried a need and its undo runs
- **THEN** the undo token holds `needs:N1..N2` after `parked:`
- **AND** the undo moves the line back
  [@test] crates/sync-core/src/needs_test.rs (`a_rename_carries_the_need_as_the_branch_and_as_the_name_it_needs_and_its_undo_moves_it_back`)

### Requirement: Rename and landing: A sync drops a needed name once it is integrated

A sync MUST drop a needed name once its newest archived branch is integrated (`integrated_children`, in the one drop of
the stores, best effort, only while the lock is free): only that name, the line with its last name.

#### Scenario: A sync keeps what it can not prove landed and drops only the name that landed
- **WHEN** a sync runs and the newest archived branch of one needed name is integrated
- **THEN** only that name is dropped, the line with its last name
- **AND** a need it can not prove landed stays
  [@test] crates/sync-core/src/needs_test.rs (`a_sync_keeps_a_need_it_can_not_prove_landed_and_a_drop_takes_only_the_name_that_landed`)

#### Scenario: The needed branch lands
- **GIVEN** a branch that declared a need refused its start
- **WHEN** the needed branch is integrated into the baseline and a sync runs
- **THEN** the sync drops the line and nothing refuses the start any longer
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### Requirement: Rename and landing: A name of a companion is dropped once the companion archived it

A sync MUST drop a name of a companion once the last sync of the companion archived it as integrated
([below](#companion-repositories)).

#### Scenario: The companion's last sync archived the branch as integrated
- **GIVEN** a need names `<companion>:<branch>`
- **WHEN** the last sync of the companion archived that branch as integrated and a sync runs here
- **THEN** the sync drops that name
  [@test] crates/bd-remote/src/status_test.rs (`a_need_of_a_branch_of_a_companion_refuses_until_the_companion_landed_it`)

### Requirement: Rename and landing: An undo of a rename passes over the drops of a sync

An undo of a rename MUST pass over the drops of a sync.

#### Scenario: A sync dropped a name after the rename
- **GIVEN** a rename, then a sync that dropped a needed name
- **WHEN** the undo of the rename runs
- **THEN** the rename is undone and the drop stays
  [@test] crates/sync-core/src/needs_test.rs (`an_undo_of_a_rename_passes_over_the_drops_of_a_sync`)

### Requirement: The rule: A declared need not in master refuses the start of its branch

A declared need that is not in master MUST refuse the start of its branch (`LandingRefusal::UnlandedNeed`, the third
refusal of the landing gate) with the state of the needed branch: `active` (it groups commits here), `unapplied` (an
unapply keeps it), `missing` (no branch of that name), `companionUnreadable` (its companion can not be read).

#### Scenario: The needed branch is active
- **GIVEN** a branch declared a need of a branch that groups commits here
- **WHEN** the rule decides its start
- **THEN** the start is refused its own need with the state `active`, without a fix
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_that_needs_an_active_branch_is_refused_its_own_need_without_a_fix`)

#### Scenario: Every state of a needed branch
- **WHEN** the needed branch is `unapplied`, `missing` or `companionUnreadable`
- **THEN** the start is refused, the refusal naming the needed branch and its state
  [@test] crates/domain-policy/src/declared_needs_test.rs (`every_state_of_a_needed_branch_refuses_the_start_and_names_itself`)

#### Scenario: The needed branch landed
- **GIVEN** the needed branch is in master (a sync dropped the line)
- **WHEN** the rule decides the start
- **THEN** nothing is refused
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_need_of_a_branch_that_landed_refuses_nothing`)

#### Scenario: Nothing declared
- **GIVEN** no need is declared
- **WHEN** the rule decides the start
- **THEN** nothing is refused
  [@test] crates/domain-policy/src/declared_needs_test.rs (`nothing_declared_refuses_nothing`)

### Requirement: The rule: The refusal has no file:line and no commit

No commit proves a declared need and no build fails: the refusal MUST have no `file:line` and no commit.

#### Scenario: A refusal of a declared need
- **WHEN** a declared need refuses a start
- **THEN** the refusal has no `file:line` and no commit

### Requirement: The rule: One rule decides it for the app and brd

One rule MUST decide it for the app and `brd` (`domain_policy::declared_needs`), over the lines of the table, the
states of the needed branches and the chains of the stack.

#### Scenario: A line of a branch no chain names
- **GIVEN** a line of the table of a branch of another worktree, which no chain of the stack names
- **WHEN** `domain_policy::declared_needs` decides over the lines, the states and the chains
- **THEN** the branch keeps its declared need
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_of_another_worktree_keeps_its_declared_need_although_no_chain_names_it`)

### Requirement: The rule: A branch built on one that declared a need inherits it

A branch built on a branch that declared a need MUST inherit it: they land in one robot. The refusal MUST name the
nearest branch of the chain that declared it (`inherited_from`) and the fix `Unstack`, which lands the branch alone.

#### Scenario: A branch built on the one that needs
- **GIVEN** `ui` is built on `auth`, which needs `docs`
- **WHEN** the rule decides the start of `ui`
- **THEN** `ui` inherits the need from `auth` (`inherited_from`), with the fix `Unstack`
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_built_on_one_that_needs_inherits_the_need_and_unstacking_it_is_the_fix`)

#### Scenario: Two branches of the chain declared the same need
- **GIVEN** a branch built on a chain of which two branches declared the same need
- **WHEN** the rule decides its start
- **THEN** the need is inherited once, and the nearest branch of the chain that declared it names the fix
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_need_of_a_branch_further_down_the_chain_is_inherited_once_and_the_nearest_owner_names_the_fix`)

### Requirement: The rule: Needs are never followed through needs

A branch needed by the branch it is built on MUST inherit no need of itself; needs MUST never be followed through
needs, so two branches that need each other are each refused once.

#### Scenario: A branch needed by the branch it is built on
- **GIVEN** `auth` needs `ui`, and `ui` is built on `auth`
- **WHEN** the rule decides the start of `ui`
- **THEN** `ui` inherits no need of itself
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_branch_that_is_needed_by_the_branch_it_is_built_on_inherits_no_need_of_itself`)

#### Scenario: Two branches that need each other
- **GIVEN** two branches that need each other
- **WHEN** the rule decides their starts
- **THEN** each is refused once
  [@test] crates/domain-policy/src/declared_needs_test.rs (`two_branches_that_need_each_other_are_each_refused_once`)

### Requirement: The rule: A robot that lands the needed branch is named

While a robot lands the needed branch, the refusal MUST name it (`landing_in`), as a missing declaration does.

#### Scenario: The needed branch is landing in its Safe Push
- **GIVEN** a robot lands the needed branch
- **WHEN** the rule refuses the start
- **THEN** the refusal names that robot (`landing_in`)
  [@test] crates/domain-policy/src/declared_needs_test.rs (`a_robot_that_lands_the_needed_branch_already_is_named_by_the_refusal`)

### Requirement: The rule: The whole stack is refused the needs of every branch it lands

The whole stack MUST be refused the needs of every branch it lands.

#### Scenario: A member of the stack declared a need
- **GIVEN** a member of the stack declared a need that is not in master
- **WHEN** the whole stack starts
- **THEN** the start is refused
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_of_a_member_refuses_the_start_of_the_whole_stack`)

### Requirement: The start: `brd` refuses a start the rule refuses with `unlanded_need`

`brd safe-push`, `--patronus-dry-run` and `brd land` MUST refuse a start the rule refuses with `unlanded_need` (exit 4),
nothing pushed, no robot started.

#### Scenario: A Safe Push of a branch with an unlanded need
- **GIVEN** a branch declared a need that is not in master
- **WHEN** `brd safe-push`, `--patronus-dry-run` or `brd land` starts it
- **THEN** it fails with `unlanded_need` (exit 4)
- **AND** nothing is pushed and no robot started
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

#### Scenario: A removed need
- **GIVEN** the need was declared and then removed
- **WHEN** the branch starts
- **THEN** nothing refuses it as `unlanded_need`
  [@test] crates/bd-remote/src/landed_test.rs (`a_removed_need_refuses_nothing`)

#### Scenario: The landing cases
- **WHEN** the landing cases run through `brd`
- **THEN** they answer as in the app
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: The start: The refusal names what lands first and how to lift it

The message of the refusal MUST name what lands first, `brd need <branch> --remove <needed>` of the branch that
declared it, and `--allow-unlanded`, which the agent passes only when the user says the order no longer holds.

#### Scenario: The message of the refusal
- **WHEN** `brd safe-push` refuses a start with `unlanded_need`
- **THEN** the message names what lands first and `brd need <branch> --remove <needed>` of the branch that declared it
- **AND** it names `--allow-unlanded`, only when the user says the order no longer holds
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### Requirement: The start: An inherited need names the unstack as its next step

An inherited need MUST also name `brd unstack <branch>` as its next step, after the user approves.

#### Scenario: A start of the branch built on the one that declared the need
- **WHEN** the start of a branch that inherits the need is refused
- **THEN** the refusal names `brd unstack <branch>` as its next step, after the user approves
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### Requirement: The start: `--dry-run` lists the same refusal

`--dry-run` MUST list the same refusal in `refusals`.

#### Scenario: A dry run of a refused start
- **WHEN** `brd safe-push <branch> --dry-run` runs for a branch the rule refuses
- **THEN** it lists the `unlanded_need` refusal in `refusals`
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

### Requirement: The start: `--allow-unlanded` lifts this refusal only

`--allow-unlanded` MUST lift this refusal only.

#### Scenario: The user says the order no longer holds
- **WHEN** the start runs with `--allow-unlanded`
- **THEN** no `unlanded_need` refusal stops it
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)

#### Scenario: Another refusal of the same start
- **WHEN** the start runs with `--allow-unlanded` and another refusal holds
- **THEN** that refusal still stops it

### Requirement: The start: The app's start refuses it the same way

The app's start MUST refuse it the same way (`domain_policy::land::decide` over the same refusals).

#### Scenario: The landing cases in the app and in brd
- **WHEN** the landing cases run in the app and in `brd`
- **THEN** `domain_policy::land::decide` refuses the same starts over the same refusals
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: The start: The popover says why and Start Anyway lifts it

When the app's start is refused by a declared need, its popover MUST say why, and Start Anyway MUST pass
`allowUnlanded`.

#### Scenario: A refused start in the app
- **WHEN** the app's start is refused by a declared need
- **THEN** its popover says why
- **AND** Start Anyway passes `allowUnlanded`
  [@test] src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`)

### Requirement: Every route: The refusals are known as soon as the sync read the store

The refusals MUST be known as soon as the sync read the store, on every route into master (Patronus, a push, a merge
of the pull request) and before Patronus or GitHub answer.

#### Scenario: A declared need off Space
- **GIVEN** a route without Patronus, and no answer of Patronus or GitHub
- **WHEN** the sync read the store
- **THEN** the refusals of the declared needs are known, with the state of each branch they name
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)

#### Scenario: Patronus and a push
- **WHEN** the landing cases of a declared need run on Patronus and on a repository that lands by a push
- **THEN** the declared need refuses the start alike in the app and `brd`
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Every route: The session derives the refusals beside the found needs

The table of the sync MUST carry the lines (`BranchTable::needs`, the frame of the warm start too), and the session
MUST derive the refusals beside the found needs (`repository_model::derive::declared_needs_of`).

#### Scenario: A sync and a warm start
- **WHEN** a sync reports its table, or the warm start reads its frame
- **THEN** the lines are in `BranchTable::needs`
- **AND** the session derives the refusals beside the found needs (`repository_model::derive::declared_needs_of`)

### Requirement: Every route: The next step is row 1′ while a declared need refuses the start

The next step MUST be row 1′ (`needs_branch`) while a declared need refuses the start, before the wait for an answer
of Patronus or of the gate of the route (row 0); a reason of the subject (row 1) still comes first. A found need
waits for the answers as before.

#### Scenario: Before any answer and after a reason of the subject
- **GIVEN** a declared need refuses the start
- **WHEN** Patronus has not answered, or the gate of the route waits (row 0)
- **THEN** the next step is row 1′ (`needs_branch`)
- **AND** a reason of the subject (row 1) still comes first
- **AND** a found need waits for the answers as before
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_names_a_declared_need_before_any_answer_and_after_a_reason_of_the_subject`)

### Requirement: Every route: `brd status` lists the refusals in `landing_needs`

`brd status` MUST list the refusals in `landing_needs` of the branch with the code `unlanded_need`, `needs_branch`,
`state`, `inherited_from` and `fix`: on every route, the declared ones first.

#### Scenario: The refusals of the branch and of the branch built on it
- **WHEN** `brd status` runs while a declared need refuses a branch and the branch built on it
- **THEN** `landing_needs` of the branch holds the code `unlanded_need`, `needs_branch` and `state`
- **AND** of the branch built on it also `inherited_from` and `fix`
  [@test] crates/bd-remote/src/landed_test.rs (`a_declared_need_refuses_the_start_of_the_branch_and_of_the_one_built_on_it_until_it_lands`)
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)

#### Scenario: Declared and found needs of one branch
- **GIVEN** a branch has declared needs and found needs, on any route
- **WHEN** `brd status` runs
- **THEN** `landing_needs` lists the declared ones first

### Requirement: Every route: Off Space only a refused branch has `landing_needs`

Off Space only a branch a declared need refuses MUST have `landing_needs`, and `--brief` none.

#### Scenario: Off Space and brief
- **WHEN** `brd status` runs off Space
- **THEN** only a branch a declared need refuses has `landing_needs`
- **AND** `brd status --brief` lists none
  [@test] crates/bd-remote/src/status_test.rs (`a_declared_need_is_in_the_status_off_space_with_the_state_of_the_branch_it_names`)

### Requirement: The app: Row 1′ names the branch the user declared lands first

Row 1′ MUST name the branch the user declared lands first ("Needs docs"), its hover the reason; a click opens the Safe
Push popover, which says it and starts anyway.

#### Scenario: The row of a branch that declared a need
- **GIVEN** a branch declared a need that is not in master
- **WHEN** its row shows row 1′
- **THEN** it names the branch it needs ("Needs docs"), its hover the reason
- **AND** a click opens the Safe Push popover, which says it and starts anyway
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the user declared lands first, opens the Safe Push, and unstacks a branch that inherits the need`)
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and undo the declaration`)

### Requirement: The app: The click of an inherited need runs the unstack

For an inherited need the click of row 1′ MUST run the unstack ("Unstack ui from auth"), which the toast of a refused
start offers too.

#### Scenario: The row of a branch that inherits the need
- **GIVEN** `ui` is built on `auth`, which needs `docs`
- **WHEN** the user clicks row 1′ of `ui`
- **THEN** the unstack runs ("Unstack ui from auth")
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the user declared lands first, opens the Safe Push, and unstacks a branch that inherits the need`)

#### Scenario: The toast of a refused start
- **WHEN** a start of a branch that inherits the need is refused
- **THEN** its toast offers the unstack too

### Requirement: The app: The Safe Push popover says the reason and starts anyway

The Safe Push popover MUST say the reason before the start, and Start Anyway MUST start with `allowUnlanded`.

#### Scenario: Start Anyway with the override of the declared order
- **WHEN** the user opens the Safe Push popover of a branch with a declared need
- **THEN** it says what lands first, before the start
- **AND** Start Anyway starts with `allowUnlanded`
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should say in the Safe Push popover what lands first, and start anyway with the override of the declared order`)

### Requirement: The app: The branch menu offers Add Need beside Park Branch

The branch menu (right-click and "⋯") MUST offer "Add Need…" beside "Park Branch…", disabled without commits.

#### Scenario: A branch with commits and one without
- **WHEN** the user opens the menu of a branch, by right-click or "⋯"
- **THEN** it offers "Add Need…" beside "Park Branch…"
- **AND** of a branch without commits it is disabled
  [@test] crates/domain-policy/src/actions_test.rs (`a_need_is_added_to_a_branch_with_commits_and_removed_while_one_is_declared`)
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and undo the declaration`, `should take back a declared need from the menu, one of two from its submenu`)

### Requirement: The app: Add Need opens a form under the row

"Add Need…" MUST open a form under the row, "What lands before auth", its field focused ("A branch of this repository,
or companion:branch"), the other branches of the table as chips.

#### Scenario: The form of Add Need
- **WHEN** the user chooses "Add Need…" of `auth`
- **THEN** a form under the row says "What lands before auth", its field focused ("A branch of this repository, or companion:branch")
- **AND** the other branches of the table are chips
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and undo the declaration`)

### Requirement: The app: Enter or Add Need writes the need

Enter or Add Need MUST write it: the rewrite `need`, queued as every rewrite, then the sync and the toast "Declared that
docs lands before auth" with Undo, the store back as it was.

#### Scenario: The need is written and undone
- **WHEN** the user presses Enter or Add Need in the form
- **THEN** the rewrite `need` runs, queued as every rewrite, then the sync
- **AND** the toast "Declared that docs lands before auth" offers Undo, which puts the store back as it was
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should declare from the menu what lands before a branch, name it on the row, and undo the declaration`)

### Requirement: The app: A refusal of the core keeps the form

A refusal of the core MUST keep the form with the typing.

#### Scenario: The core refuses the need
- **WHEN** the core refuses the need the form writes
- **THEN** the form stays open with the typing

### Requirement: The app: The branch menu offers Remove Need of each declared need

Of a branch that declared needs the menu MUST offer "Remove Need: docs", for two or more a submenu "Remove Need ›" of
them, each the rewrite `unneed`.

#### Scenario: One declared need and two
- **GIVEN** a branch that declared needs
- **WHEN** the user opens its menu
- **THEN** it offers "Remove Need: docs" for one, a submenu "Remove Need ›" of them for two or more
- **AND** each runs the rewrite `unneed`
  [@test] crates/domain-policy/src/actions_test.rs (`a_need_is_added_to_a_branch_with_commits_and_removed_while_one_is_declared`)
  [@test] tests/e2e/declared-needs/declared-needs.spec.ts (`should take back a declared need from the menu, one of two from its submenu`)

### Requirement: The app: Rust decides both actions over the needs of the branch

Rust MUST decide both actions (`ActionId::AddNeed`, `RemoveNeed`, `domain_policy::actions`) over the declared needs of
the branch, which the view carries in its structure (`BranchesGrouped::needs` → `BranchStructure.needs`).

#### Scenario: The actions of a branch
- **WHEN** `domain_policy::actions` decides the actions of a branch
- **THEN** `ActionId::AddNeed` is disabled of a branch without commits, `RemoveNeed` offered while a need is declared
  [@test] crates/domain-policy/src/actions_test.rs (`a_need_is_added_to_a_branch_with_commits_and_removed_while_one_is_declared`)

#### Scenario: The structure of a branch names its needs
- **WHEN** a sync groups the branches (`BranchesGrouped::needs`)
- **THEN** the structure of each branch names the needs it declared (`BranchStructure.needs`)
  [@test] crates/repository-model/src/derive_test.rs (`the_structure_of_a_branch_names_the_needs_it_declared`)

### Requirement: The app: The reason names the state of the needed branch

The reason MUST name the state of the needed branch in the words below, and the robot that lands it.

#### Scenario: Each state of the needed branch and a robot that lands it
- **WHEN** the reason of a declared need is worded
- **THEN** it names the state of the needed branch in the words below
- **AND** the robot that lands it
  [@test] src/presenters/landing.test.ts (`words the order the user declared by the state of the branch it names, and Start Anyway lifts it`)

#### Scenario: An inherited need
- **WHEN** the reason of an inherited need is worded
- **THEN** it says why the branch inherits it, and the unstack lands it alone
  [@test] src/presenters/landing.test.ts (`says why a branch built on the one that declared the need inherits it, and unstacks it to land it alone`)

### Requirement: The app: A need is worded as the other rewrites

A need written by the app's rewrites (`Rewrite(Need)`, `Rewrite(Unneed)`) MUST be worded as the other rewrites:
"Declaring what lands before auth", "Cannot declare that docs lands before auth", "Removed the need of auth for docs".

#### Scenario: The words of a need and of its removal
- **WHEN** the app runs `Rewrite(Need)` or `Rewrite(Unneed)`
- **THEN** it is worded "Declaring what lands before auth", "Cannot declare that docs lands before auth", "Removed the need of auth for docs"
  [@test] src/presenters/rewrite.test.ts (`words a rewrite by what it does to what it names`)

### Requirement: Companion repositories: A primary and its companions name each other in local config

A primary MUST list its companions in its local config (`branchdeck.companion.<name>.path`, the name as it is written),
each companion its primary (`branchdeck.primary`), both the paths of the main worktrees, so its leases and linked
worktrees name the same repository.

#### Scenario: A companion is added
- **WHEN** a companion is added to a primary
- **THEN** the primary lists it in `branchdeck.companion.<name>.path`, the name as it is written
- **AND** the companion names its primary in `branchdeck.primary`, both the paths of the main worktrees
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

### Requirement: Companion repositories: One writer writes both sides

`brd companion add <path> [--name]`, `brd companion remove <name>`, the section Companions of Settings → Landing (the
tab of the repository) and its Add Companion… (the folder dialog, named by the directory; none where the host has no
folder dialog, the IDE) and Remove MUST write both sides through one writer (`repository_config::write_companion`).

#### Scenario: Add and remove in brd
- **WHEN** `brd companion add <path> [--name]` or `brd companion remove <name>` runs
- **THEN** both sides are written through `repository_config::write_companion`
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

#### Scenario: Add Companion… and Remove in Settings
- **WHEN** the user picks a folder in the folder dialog of Add Companion… in the section Companions of Settings → Landing, the tab of the repository, then removes it
- **THEN** the companion is added under the name of its directory, then removed, both sides written
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)

### Requirement: Companion repositories: The list reads the companions with the problem of each path

`brd companion list` and the page MUST read the companions with the problem of each path.

#### Scenario: A companion whose path has a problem
- **WHEN** `brd companion list` runs or the page shows the companions
- **THEN** each companion comes with the problem of its path

### Requirement: Companion repositories: A companion has one primary and a valid name

A companion MUST have one primary, a primary MUST be no companion, a repository no companion of itself, and a name is
letters, digits, `-`, `_` and `.`; a removal MUST unset the primary of the companion only while it names this
repository.

#### Scenario: The writes the writer refuses
- **WHEN** a companion of a second primary, a companion of a companion, a companion of itself or a name with other characters is written
- **THEN** the writer refuses it
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

#### Scenario: A removal
- **WHEN** a companion is removed
- **THEN** its primary is unset only while it names this repository
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

### Requirement: Companion repositories: The project selector groups the linked repositories

The main-window project selector SHALL show the selected repository's links from its session: a primary labels the
group "Companions of app", a companion "Primary and companions" with the primary first.

#### Scenario: The selector of a primary
- **WHEN** the user opens the project selector of a primary
- **THEN** its companions stand in the group "Companions of app"
  [@test] src/presenters/repository-links.test.ts (`puts linked repositories after Recent and removes their duplicate recent entries`)
  [@test] tests/e2e/repository/companion-links.spec.ts (`should show and switch to a companion that has never been opened @webkit`)

#### Scenario: The selector of a companion
- **WHEN** the user opens the project selector of a companion
- **THEN** the group is "Primary and companions", the primary first
  [@test] src/presenters/repository-links.test.ts (`names a companion's primary group and keeps the primary first`)

### Requirement: Companion repositories: A linked repository appears only once

A linked repository SHALL appear only once, apart from the remaining "Recent" projects; with no links the existing list
has no labels.

#### Scenario: A linked repository that is also recent
- **WHEN** a linked repository is among the "Recent" projects
- **THEN** it appears only once, in its group, apart from the remaining "Recent" projects
  [@test] src/presenters/repository-links.test.ts (`puts linked repositories after Recent and removes their duplicate recent entries`)

#### Scenario: Two links of one path
- **WHEN** two links name the same path, written differently, and one of them is unreadable
- **THEN** the path appears once, in the linked group, with its problem
  [@test] src/presenters/repository-links.test.ts (`deduplicates equivalent paths and keeps unreadable links in the linked section`)

#### Scenario: No links
- **WHEN** the selected repository has no links
- **THEN** the existing list has no labels
  [@test] src/presenters/repository-links.test.ts (`returns one unnamed section when there are no links`)
  [@test] tests/e2e/repository/header.spec.ts (`should show the unlabelled recent-project list when there are no companions`)

### Requirement: Companion repositories: The trigger keeps the selected path and its width

The trigger of the project selector SHALL keep the selected path and its width.

#### Scenario: A repository with links is selected
- **WHEN** the selected repository has links
- **THEN** the trigger shows the selected path and keeps its width

### Requirement: Companion repositories: Opening the selector starts no Git process

Opening the pop-up SHALL start no Git process; a sync SHALL carry the linked paths and why one is unreadable.

#### Scenario: The pop-up opens
- **WHEN** the user opens the pop-up of the project selector
- **THEN** no Git process starts: it shows the links the last sync carried

#### Scenario: A sync reads the links
- **GIVEN** a linked path that is gone and one that is no Git repository
- **WHEN** a sync reads the links of the repository
- **THEN** it carries each linked path with why it is unreadable (`… is gone`, `… is no git repository`)
- **AND** it reads them in one `config` process
  [@test] crates/sync-core/src/companions_test.rs (`repository_links_report_missing_and_non_repository_paths`)

### Requirement: Companion repositories: A linked project outside Recent can be selected

A linked project outside Recent SHALL be selectable; it shows the full path under its shortened path until its first
sync, then the usual sync data.

#### Scenario: A companion that has never been opened
- **WHEN** the user selects a linked project outside Recent
- **THEN** it shows the full path under its shortened path until its first sync
- **AND** then the usual sync data
  [@test] tests/e2e/repository/companion-links.spec.ts (`should show and switch to a companion that has never been opened @webkit`)

### Requirement: Companion repositories: A linked path that is not readable is disabled

A path that is gone or no Git repository SHALL be disabled and say "Not readable: …".

#### Scenario: The companion is no Git repository any longer
- **WHEN** a linked path is gone or no Git repository
- **THEN** its entry is disabled and says "Not readable: …"
  [@test] tests/e2e/repository/companion-links.spec.ts (`should show and switch to a companion that has never been opened @webkit`)

### Requirement: Companion repositories: A need of a companion names it as name:branch

A need of a companion MUST name it as `<name>:<branch>`: in a primary one of its companions, in a companion its primary
(by the name of its directory) or another companion of that primary.

#### Scenario: The names a primary and a companion resolve
- **WHEN** a need names `<name>:<branch>`
- **THEN** in a primary the name is one of its companions
- **AND** in a companion it is its primary, by the name of its directory, or another companion of that primary
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

### Requirement: Companion repositories: A branch of a companion is read of its last sync

How a branch of a companion stands MUST be read of what the last sync of the companion left, never by a sync of it:
its config, one listing of its branches under its prefix (of its own config, else this one's) and the integration
notes of their tips, three processes per companion, none while no need names one.

#### Scenario: Three processes per companion
- **WHEN** needs name branches of a companion
- **THEN** its config, one listing of its branches under its prefix and the integration notes of their tips are read, three processes per companion
  [@test] crates/sync-core/src/companions_test.rs (`a_branch_of_a_companion_is_active_unapplied_integrated_or_missing_as_its_last_sync_left_it`)

#### Scenario: No need names a companion
- **WHEN** no need names a companion
- **THEN** nothing of the companions is read
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_that_is_gone_or_unknown_makes_its_needs_unreadable_and_nothing_fails`)

### Requirement: Companion repositories: The state of a branch of a companion

A grouped branch of a companion MUST be `active`, an unapplied one `unapplied`, one whose newest archived branch is
integrated landed, anything else `missing`.

#### Scenario: Each state its last sync left
- **WHEN** a need names a branch of a companion that is grouped, unapplied, archived as integrated, archived and not integrated, or nowhere
- **THEN** it is `active`, `unapplied`, landed, `missing` and `missing`
  [@test] crates/sync-core/src/companions_test.rs (`a_branch_of_a_companion_is_active_unapplied_integrated_or_missing_as_its_last_sync_left_it`)

#### Scenario: The status of a need of a companion
- **WHEN** `brd status` runs while a need names a branch of a companion
- **THEN** the need refuses with its state until the companion landed it
  [@test] crates/bd-remote/src/status_test.rs (`a_need_of_a_branch_of_a_companion_refuses_until_the_companion_landed_it`)

### Requirement: Companion repositories: An unreadable companion fails nothing

A companion that is gone, no git repository or no companion of this repository MUST make its needs
`companionUnreadable` with the reason, and neither `brd status` nor the sync MUST fail on it.

#### Scenario: A companion that is gone or unknown
- **WHEN** a need names a companion that is gone, no git repository or no companion of this repository
- **THEN** its state is `companionUnreadable` with the reason
- **AND** neither `brd status` nor the sync fails on it
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_that_is_gone_or_unknown_makes_its_needs_unreadable_and_nothing_fails`)

### Requirement: Companion repositories: The sync drops the landed ones, the status reads without a sync

The sync MUST read the states of the branches of companions in its detection, drop the landed ones from the store and
pass the rest to the table; `brd status` MUST read them without a sync.

#### Scenario: A sync after the companion landed the branch
- **GIVEN** the last sync of the companion archived the needed branch as integrated
- **WHEN** a sync runs here
- **THEN** it drops the landed need from the store and passes the rest to the table
  [@test] crates/bd-remote/src/status_test.rs (`a_need_of_a_branch_of_a_companion_refuses_until_the_companion_landed_it`)

#### Scenario: `brd status` without a sync
- **WHEN** `brd status` runs
- **THEN** it reads the states of the branches of companions without a sync

### Requirement: Companion repositories: `brd status --companions` adds the status of each companion

`brd status --companions` MUST add the status of every repository a need here can name, each by its own context and
its prefix as the companion reader takes it (`companions`: `name`, `path`, `status`, or the `error` why it can not be
read); without the flag nothing of the companions is read.

#### Scenario: A readable companion and one that is gone
- **WHEN** `brd status --companions` runs
- **THEN** `companions` holds `name`, `path` and `status` of each repository a need here can name, or the `error` why it can not be read
  [@test] crates/bd-remote/src/status_test.rs (`status_with_companions_tells_the_status_of_each_companion_and_why_one_can_not_be_read`)

#### Scenario: Without the flag
- **WHEN** `brd status` runs without `--companions`
- **THEN** nothing of the companions is read
  [@test] crates/bd-remote/src/status_test.rs (`status_with_companions_tells_the_status_of_each_companion_and_why_one_can_not_be_read`)

### Requirement: Companion repositories: A write of the companions syncs the main window

A write of the companions MUST sync the main window again (`repository_config_changed`, `companions`): the states of the
branches a need names come with the sync.

#### Scenario: A companion added in Settings
- **WHEN** the user adds a companion in Settings
- **THEN** the main window syncs again (`repository_config_changed`, `companions`)
- **AND** the states of the branches a need names come with that sync
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
  "No companion repositories. A need can name a branch of another repository once you add one here." (in the IDE, which
  offers no Add Companion…: "… once you add one in the Branch Deck app."); of a companion:
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
