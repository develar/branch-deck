## RENAMED Requirements

- FROM: `### Requirement: Starting a Safe Push or a Dry Run: A branch only rebuilt on a newer baseline is not pushed`
- TO: `### Requirement: Starting a Safe Push or a Dry Run: A branch only rebuilt on a newer baseline is pushed first`

## MODIFIED Requirements

### Requirement: Starting a Safe Push or a Dry Run: A branch only rebuilt on a newer baseline is pushed first

A branch whose remote has the same commits, only rebuilt since on a newer baseline (Update Project), MUST be pushed
before the start, on every route, so the robot and the builds of its pull request test the current one: the subject
only, never a branch it is built on, as any push under the rules of the approvals. While a running Safe Push holds it or
a branch it is built on, the start MUST go without that push. One reworded since MUST be pushed.

#### Scenario: The remote branch has the commits on an older baseline
- **WHEN** the remote branch has the commits of the branch, only rebuilt on a newer baseline
- **THEN** the start pushes the branch first, leased, without a question, then starts the robot
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push a branch rebuilt on a newer baseline before its Safe Push`)
  [@test] crates/domain-policy/src/push_test.rs (`a_start_pushes_its_branch_rebuilt_on_a_newer_baseline_unless_held`)
  [@test] crates/domain-policy/src/land_test.rs (`the_whole_stack_says_its_build_first_and_its_own_robot_holds_or_is_replaced`)

#### Scenario: The remote branch has the commits under other hashes
- **WHEN** the remote branch has an earlier build of the same commits on the same baseline
- **THEN** the start runs without a push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a branch whose remote branch has its commits under other hashes without a push`)

#### Scenario: A hold, an ancestor, an approved pull request
- **WHEN** a running Safe Push of a branch built on it holds it, or only its parent is on an older baseline too, or its pull request has approvals the rules dismiss on a push
- **THEN** a Dry Run starts without that push, the start of the branch pushes it alone, and the start into the approved pull request is refused (`dismissesApprovals`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A commit reworded since
- **WHEN** a commit of the branch was reworded since the push
- **THEN** the branch is pushed

### Requirement: Restart and cancel: What a restart does is one rule of Rust

What a restart does MUST be one rule of Rust (`domain_policy::holds::restart_action`, the view's `restarts` by robot
id): a plain restart while the remote has the branch, a push first while the branch has work the remote lacks ("Push
and Restart", `PushAndRestart`, also of a running robot, which the push cancels first; on every surface it is named
"Restart Safe Push" too), or has it only on an older baseline; the page only presents it.

#### Scenario: The remote has the branch, or lacks its work
- **WHEN** a restart is offered
- **THEN** it restarts on the remote while the remote has the branch, else pushes first
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)

#### Scenario: A robot of a branch only rebuilt on a newer baseline
- **GIVEN** a robot took the remote branch, which has the commits of the branch on an older baseline
- **WHEN** it failed, was canceled or a required check of it failed
- **THEN** its restart pushes the branch first, so the robot tests the current baseline
- **AND** a healthy running robot and a Dry Run that passed on the remote's commit offer no restart
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_of_a_robot_on_an_older_baseline_pushes_first_and_a_healthy_one_stays`)
