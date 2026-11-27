## ADDED Requirements

### Requirement: `brd safe-push`: A Safe Push starts beside a running one of another subject when told

A running Safe Push of another subject that lands the same commits (the whole stack, a branch built on the branch, a
branch it is built on) MUST refuse a Safe Push start with `robot_running` unless `--keep-running`, which the refusal
offers as a step that needs the approval. With it the start MUST cancel nothing and MUST push the branches that robot
only lands, never the branch a robot runs on. A robot on the last commit MUST still refuse.

#### Scenario: A branch beside the Safe Push of its whole stack
- **GIVEN** a Safe Push of the whole stack runs
- **WHEN** the Safe Push of a branch of it starts without `--keep-running`
- **THEN** it refuses with `robot_running`, the steps `brd robot <root> --whole-stack` and the start with
  `--keep-running`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_safe_push_starts_beside_a_running_robot_of_the_whole_stack_only_when_told_to_keep_it_running`)

#### Scenario: Told to keep it running
- **WHEN** the Safe Push starts with `--keep-running`
- **THEN** the robot starts from the branch, nothing is canceled, and the branches the robot of the whole stack lands
  are pushed
- **AND** `brd robot <branch>` names the new robot, `brd robot <root> --whole-stack` the robot of the whole stack
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_safe_push_starts_beside_a_running_robot_of_the_whole_stack_only_when_told_to_keep_it_running`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_kept_beside_a_robot_that_lands_it_with_more_starts_and_pushes_what_that_robot_holds_by_its_span`)

#### Scenario: With --cancel-running or a Dry Run
- **WHEN** `--keep-running` comes with `--cancel-running`, `--patronus-dry-run` or `--create-pull-request`
- **THEN** it is `invalid_arguments`, exit 2
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_safe_push_starts_beside_a_running_robot_of_the_whole_stack_only_when_told_to_keep_it_running`)

#### Scenario: A Dry Run keeps no robot running
- **WHEN** a Dry Run with the allowance would push into what a running Safe Push lands
- **THEN** it is refused as before (`held`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_kept_beside_a_robot_that_lands_it_with_more_starts_and_pushes_what_that_robot_holds_by_its_span`)

## MODIFIED Requirements

### Requirement: Output contract: The next steps of each refusal

The next steps of a refusal SHALL be the commands its code names below. The field MUST be named `next_steps`, never
`next`, whose commands (`brd workspace`, `brd update`, `brd pick`) run at once. A command with its arguments that the
message names MUST name the repository as a step does (`brd -C <repo> …`, `RepoContext::bd`; another worktree by
`in_worktree`), and where the refusal has that step, be its very command; a command named only as a pointer
(`brd sync` says what holds it) stays bare.

#### Scenario: A leased push that the remote rejects
- **WHEN** a push is `lease_rejected`
- **THEN** the step is the leased push that replaces the remote branch
  [@test] crates/bd-remote/src/push_test.rs (`a_moved_remote_branch_is_fetched_and_reported_and_force_replaces_it`)

#### Scenario: A push the remote refuses
- **WHEN** a push is `push_refused`
- **THEN** the step of a branch behind its remote is the push that replaces it, and of a branch not synced `brd sync`
  [@test] crates/bd-remote/src/push_test.rs (`all_needs_yes_and_unsynced_branches_are_refused`)

#### Scenario: A robot that runs
- **WHEN** a start is `robot_running`
- **THEN** the steps are `brd robot` of the robot and the `--cancel-running`, `--keep-running` or `--restart-running` of
  the same command
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`, `push_refuses_a_branch_whose_safe_push_runs_and_restarts_it_when_told`, `a_safe_push_starts_beside_a_running_robot_of_the_whole_stack_only_when_told_to_keep_it_running`)

#### Scenario: Missing approvals without a pull request
- **WHEN** a start is `approvals_missing` and the branch has no pull request
- **THEN** the step is the `--create-pull-request` of the start
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`)

#### Scenario: A refusal by the code a branch uses
- **WHEN** a start is `dangling_reference` or `missing_declaration`
- **THEN** the step of `dangling_reference` is the `fix.command` a trial build proved, of `missing_declaration` the
  `brd restack` it names
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)

#### Scenario: A push that dismisses the approvals
- **WHEN** a start is `push_dismisses_approvals`
- **THEN** the steps are the push and then the review request
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`)

#### Scenario: A branch that landed or whose target conflicts
- **WHEN** a refusal is `target_conflict` or `landed`
- **THEN** the step is `brd update`
  [@test] crates/bd-remote/src/landed_test.rs (`push_and_safe_push_refuse_a_branch_a_merged_safe_push_landed_whole`)

#### Scenario: A whole stack that conflicts or is not built
- **WHEN** a start is `stack_conflict` or `stack_not_built`
- **THEN** the step of `stack_conflict` is the `--dry-run` of the branch whose commit conflicts, of `stack_not_built`
  `brd sync`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)

#### Scenario: The message names the command of its step
- **WHEN** a refusal's message names a command with its arguments: the `--create-pull-request` of `approvals_missing`,
  the push and the review request of `push_dismisses_approvals`, the `brd update` and the start again of
  `target_conflict`, the robot of another branch, the `brd safe-push` of `no_pull_request`, the abort and the unapply
  of `unsupported_conflict`
- **THEN** it names `brd -C <repo>` and the arguments, the command of `next_steps` where the refusal has one
  [@test] crates/bd-remote/src/safe_push_test.rs (`missing_approvals_refuse_the_safe_push_unless_told_to_start_anyway`, `the_approvals_count_in_the_pull_request_it_opens_and_the_safe_push_starts_from_it`, `a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`, `a_target_that_moved_with_a_conflicting_change_after_the_last_sync_refuses_the_start`)
  [@test] crates/bd-remote/src/robot_test.rs (`robot_takes_a_robot_by_id_of_the_branch_only`)
  [@test] crates/bd-remote/src/request_review_test.rs (`a_branch_without_an_open_pull_request_or_a_code_owner_to_ask_is_refused`)
  [@test] crates/bd-update/src/resolve_test.rs (`binary_and_file_directory_conflicts_are_refused_and_start_no_session`, `an_unsupported_conflict_asks_the_user_when_unapply_dry_run_refuses`)
