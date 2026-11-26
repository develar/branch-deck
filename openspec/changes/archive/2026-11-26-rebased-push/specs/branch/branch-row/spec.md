## MODIFIED Requirements

### Requirement: Branch menu: The push item is the push button of the row

The push item MUST be the push button of the row as a menu item (`getPushButtonNow`), also where the row has no button for
a first push: its name, what it pushes (`pushOrConfirm`: the stack only where the button pushes it, the question in the
button's description before a push that drops remote commits, Push Rebased Branch of a branch only rebuilt on a newer
baseline), enabled as that button (the `push` Rust decides), else disabled with the reason.

#### Scenario: The name and the push of the item
- **WHEN** the menu of a branch with a push opens
- **THEN** the push item is named as the push button ("Push Branch (2)", "Force Push Branch (1)", "Push Stack (3 branches)" with "Pushes auth, login and oauth, parents first" on
  hover), and pushes the stack as far as the button does
- **AND** above an ancestor whose push drops remote commits it pushes the branch alone: that one asks first in its own row
  [@test] src/hooks/useBranchContextActions.test.ts (`is one item named as the push button, pushing the stack as far as the button does`, `pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

#### Scenario: A push that can't run
- **WHEN** the push of the branch can't run
- **THEN** the item is disabled with the reason ("The remote branch is up to date", the blocked parent, "The branch failed to sync: merge conflict" for a
  branch the last sync did not build, the running Safe Push that holds it, "Wait for the sync to finish" while a sync
  rebuilds the branch)
  [@test] src/hooks/useBranchContextActions.test.ts (`is disabled with the reason Rust gives: a branch the last sync did not build names the status of its row, not up to date`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_push_waits_for_parked_blocked_failed_held_a_rebuild_and_nothing_to_push_in_this_order`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

#### Scenario: A running push
- **WHEN** its push runs in the session, or another push that sends the branch holds it
- **THEN** the item is running ("Pushing branch login…"), or disabled ("Pushing branch login…" of the stack push of a branch built on it)
  [@test] src/hooks/useBranchContextActions.test.ts (`runs while its push of the stack runs in the session, and waits for another push that holds the branch`)

#### Scenario: A branch only rebuilt on a newer baseline
- **WHEN** the menu of a branch opens whose remote has its commits on an older master
- **THEN** the push item is "Push Rebased Branch", saying what it moves onto the current master, and pushes at once
- **AND** once pushed it is disabled again with "The remote branch is up to date"
  [@test] src/hooks/useBranchContextActions.test.ts (`pushes a branch only rebuilt on a newer baseline as Push Rebased Branch, at once, and says what it moves`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push a branch rebuilt on a newer baseline from the branch menu and with P, its row staying quiet`)

## ADDED Requirements

### Requirement: Branch menu: A branch on an older baseline is pushed only from its push item

A branch whose remote has its commits only on an older baseline (`older_base`) MUST keep a quiet row: no mark, no chip,
no next step, no count. Its push button, its push item and `P` MUST be Push Rebased Branch, a force push with a lease
and without a question, the branches it is built on that are on an older baseline too first (`BranchPush::rebased`),
its pull request not retargeted.

#### Scenario: The row after Update Project
- **GIVEN** two pushed branches, then Update Project rebuilt them on a newer master
- **WHEN** the table shows them
- **THEN** their rows show no push chip
- **AND** "Push Rebased Branch" of the menu and `P` push each, leased on the remote head its row shows, and its toast says "Force pushed"
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push a branch rebuilt on a newer baseline from the branch menu and with P, its row staying quiet`)
  [@test] src/presenters/push-button.test.ts (`is Push Rebased Branch of a branch only rebuilt on a newer baseline, saying what it moves onto the current master`)
  [@test] src/lib/push-run.test.ts (`pushes a branch only rebuilt on a newer baseline from its push button, leased on the head its row shows, and says it moved it`, `is no push of a branch the view orders none of: its button pushes as before`)
  [@test] src/lib/push-summary.test.ts (`says a push of branches only rebuilt on a newer baseline force pushed them, alone and together`)

#### Scenario: The details of its remote status
- **WHEN** the details of the remote status of such a branch show
- **THEN** they say "The remote has these commits on an older master: Push Rebased Branch moves them onto the current one."
  [@test] src/lib/remote-status.test.ts (`says that the remote has the commits of a branch only rebuilt on a newer baseline on an older one, which Push Rebased Branch moves`)

#### Scenario: With the branches it is built on
- **GIVEN** a branch and its parent only rebuilt on a newer baseline
- **WHEN** its push button pushes
- **THEN** one push sends the parent, then the branch, each leased, and asks nothing
  [@test] crates/domain-policy/src/push_test.rs (`a_rebased_push_sends_the_branch_and_its_rebuilt_ancestors_root_first_only_with_consent`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_sends_the_rebased_push_of_a_branch_only_rebuilt_on_a_newer_baseline`)
  [@test] crates/operations/src/push_test.rs (`a_rebased_push_sends_the_branches_only_rebuilt_on_a_newer_baseline_with_a_lease_and_without_a_question`)
  [@test] src/lib/push-run.test.ts (`pushes the branches it is built on that are only rebuilt too in one push of the stack, parents first`)

#### Scenario: Its push is offered, a hold still keeps it
- **WHEN** the push of a branch only rebuilt on a newer baseline is asked of Rust
- **THEN** it is enabled, and a running Safe Push that holds it disables it as for any push
  [@test] crates/domain-policy/src/actions_test.rs (`the_push_waits_for_parked_blocked_failed_held_a_rebuild_and_nothing_to_push_in_this_order`)
