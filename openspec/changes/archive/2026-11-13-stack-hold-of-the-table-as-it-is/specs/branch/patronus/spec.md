## MODIFIED Requirements

### Requirement: Pushing into a branch whose Safe Push runs: A running Safe Push holds the branch it lands

A branch MUST be held (`PushHold`) while a Safe Push that pushes into the target runs on it (started on the branch, not
of a temporary ref) or on a branch built on it whose commit has commits of it, which lands them ("lands with oauth"). A
push of a held branch won't land: Patronus merges the commit it took at the start, and a push into its own branch fails
it at the end ("branch was modified"). A dry run and a canceling robot MUST hold nothing.

#### Scenario: Its own Safe Push runs on the branch
- **GIVEN** a Safe Push that pushes into the target runs on the branch, started on the branch, not of a temporary ref
- **WHEN** the holds of the branch are decided
- **THEN** the branch is held, as a push into it would fail that Safe Push at the end ("branch was modified")
- **AND** it is held no longer once the robot no longer runs
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)
  [@test] src/stores/patronus.test.ts (`holds a branch whose Safe Push runs and the ancestors it lands; not once the robot no longer runs`)

#### Scenario: The Safe Push of a branch built on it lands its commits
- **GIVEN** a Safe Push runs on a branch built on the branch, and its commit has commits of the branch
- **WHEN** the holds of the branch are decided
- **THEN** the branch is held, and its reason says that the robot lands it ("lands with oauth")
  [@test] src/presenters/reasons.test.ts (`says why a held branch offers no push: its own Safe Push, one that lands it with the commits it counts`)

#### Scenario: A dry run or a canceling robot
- **GIVEN** the robot on the branch, or on a branch built on it, is a dry run or is canceling
- **WHEN** the holds of the branch are decided
- **THEN** it holds nothing
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_dry_run_lands_nothing`)

### Requirement: Pushing into a branch whose Safe Push runs: The hold is one rule of Rust for the app and brd

The hold MUST be a rule of Rust, one for the app and `brd` (`domain_policy::holds::branch_hold`, the view's
`landing.hold`): a landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push
that lands it. Every reader MUST read the hold the view decided of the branch and the robots as they are
(`derive::hold_now`). The page MUST only word it (`src/presenters/reasons.ts`).

#### Scenario: The order of the holds
- **WHEN** a branch is landed whole, its own Safe Push runs and another running Safe Push lands it
- **THEN** the hold is the landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push that lands it
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)

#### Scenario: The app and brd answer alike
- **WHEN** the app and `brd` decide the hold of the same branch
- **THEN** they answer alike
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A hold the view decided of a robot that ended
- **GIVEN** the view the session shows was derived while the robot that holds the branch ran (its own, or one that lands it), and the robot ended since
- **WHEN** a start of the branch, its step (`NextStepView.hold`, which the page words) or a push reads the hold
- **THEN** nothing holds the branch; while the robot runs, it is held (`derive::hold_now`: a hold while its robot holds pushes)
- **AND** a landing holds the branch only while its robot merged and every commit of the branch is one it landed
  [@test] crates/repository-model/src/derive_test.rs (`a_start_reads_the_holds_of_the_view_as_every_reader_does`)
  [@test] crates/repository-model/src/derive_test.rs (`a_push_reads_no_hold_of_what_a_view_of_another_table_says_landed`)

### Requirement: Pushing into a branch whose Safe Push runs: The stack branch of a whole stack is held as a branch is

The stack branch of a whole stack ([Whole Stack](../whole-stack/spec.md)) MUST be held as a branch is, by one rule of
Rust for the app and `brd push --with-children` (`domain_policy::holds::stack_branch_hold`), in this order: a parked
member; merged Safe Pushes that landed every branch of the stack whole; its own running robot; a stack branch the sync
did not build. Every reader MUST decide it of the table and the robots as they are (`derive::stack_hold_now`).

#### Scenario: A parked member
- **GIVEN** a member of the whole stack is parked
- **WHEN** the hold of the stack branch is decided
- **THEN** the parked member keeps it local: "Holds parked login: land the stack in pieces"
  [@test] crates/domain-policy/src/holds_test.rs (`a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build`)
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)

#### Scenario: Merged Safe Pushes landed every branch of the stack
- **GIVEN** merged Safe Pushes, of pieces or of the whole stack, landed every branch of the stack whole
- **WHEN** the hold of the stack branch is decided
- **THEN** the stack branch is held, and the reason says who landed them, each robot once, in the order they merged: "Landed in master with deploy and side-test: nothing of it is left to push"
- **AND** "Merged by Patronus: …" when its own robot alone landed them
  [@test] crates/domain-policy/src/holds_test.rs (`every_member_landed_by_the_robot_of_the_whole_stack_is_its_own_landing`)
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says`)
  [@test] crates/operations/src/pull_request_test.rs (`the_pull_request_of_a_whole_stack_pushes_its_stack_branch_by_the_table_as_it_is`)
  [@test] src/presenters/reasons.test.ts (`says that nothing is left to push of a stack branch merged Safe Pushes landed, and nothing for no hold`)

#### Scenario: The landed stack branch is not pushed with what is left
- **GIVEN** every branch of the stack landed, and commits are left above it
- **WHEN** the user pushes the stack
- **THEN** the push of the whole stack branch is held, and what is left pushes without the landed ones
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: Its own running robot
- **GIVEN** the Safe Push of the whole stack runs on the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held: "A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack"
- **AND** the step of the whole stack names that robot as what holds it (`NextStepView.hold`), which the page reads
  [@test] crates/repository-model/src/derive_test.rs (`the_step_of_a_whole_stack_names_its_own_robot_only_while_it_holds_the_stack_branch`)
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)

#### Scenario: A stack branch the sync did not build
- **GIVEN** the sync did not build the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held

#### Scenario: A view of another table
- **GIVEN** the view the session shows was derived for another table or other robots: it says every branch of the stack landed and a branch has a commit since, it holds nothing and a branch was parked since, or it says the robot of the whole stack runs and the robot ended since
- **WHEN** the stack branch is pushed with the children of the root, alone, or first by the creation of the pull request of the whole stack, or its step or its start reads the hold
- **THEN** it is held as the table and the robots are now: it goes while nothing holds it now, and stays local while a branch of it is parked now
- **AND** the view gives only what it alone knows: what landed each branch, the robot of the whole stack
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says`)
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)
  [@test] crates/operations/src/pull_request_test.rs (`the_pull_request_of_a_whole_stack_pushes_its_stack_branch_by_the_table_as_it_is`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should take the whole stack branch along in the header's push once its own robot ended, while the view of the robots says it runs`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should leave the whole stack branch out of the header's push once a branch of it is parked, while the view of the robots is of the table before`)

### Requirement: Pushing into a branch whose Safe Push runs: A held stack branch is left out and its push refused

While the stack branch is held, Push with Children MUST leave it out, and its push alone (the push step of the restart
of its robot, of Request Review… and of the creation of its pull request, `pushWholeStack`) MUST be refused with that
reason.

#### Scenario: Push with Children and the push of the stack branch alone
- **GIVEN** the stack branch of a whole stack is held
- **WHEN** the user pushes with children, or an action pushes the stack branch alone (`pushWholeStack`: the restart of its robot, Request Review…, the creation of its pull request)
- **THEN** Push with Children leaves it out
- **AND** its push alone is refused with the reason of the hold
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says`)

### Requirement: Pushing into a branch whose Safe Push runs: The push an action needs is refused by a branch of its chain

The push an action needs (a pull request, a review request, the push a refused start offers) MUST be refused before it
is made, of the table and the holds as they are (`derive::action_push_of`, `BranchPush.actionBlocker`): by the branch
itself that stays local, is held, was not built or drops remote commits, else by the nearest branch of its chain up to
the first held ancestor, past one that landed, that the sync did not build or whose push drops remote commits
(`ActionPush::stop`).

#### Scenario: An unbuilt or dropping branch of the chain
- **GIVEN** a branch of the chain up to the first held ancestor that the sync did not build, or whose push drops remote commits
- **WHEN** an action that needs a push is taken
- **THEN** the push is refused before it is made
  [@test] crates/repository-model/src/derive_test.rs (`the_push_of_an_action_is_refused_by_its_chain_as_the_table_is`)
  [@test] crates/domain-policy/src/push_test.rs (`the_push_of_an_action_is_stopped_by_the_nearest_branch_of_its_chain_not_built_or_dropping_commits`)
  [@test] src/lib/push-run.test.ts (`is refused in its words of what Rust refuses of the table as it is, before anything reaches the backend; else it pushes the branch`)

#### Scenario: Past an ancestor that landed
- **GIVEN** an ancestor in the chain landed: a merged Safe Push landed it whole, or the fetched baseline has every commit of it
- **WHEN** the push of the action is decided
- **THEN** the chain goes past it, as the decision of a start passes it
  [@test] crates/repository-model/src/derive_test.rs (`the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed`)
  [@test] crates/repository-model/src/branch_table_test.rs (`the_table_of_a_sync_carries_the_branches_the_fetched_baseline_has_whole`)
