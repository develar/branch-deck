## RENAMED Requirements

- FROM: `### Requirement: `brd safe-push`: A branch only rebuilt on a newer baseline is not pushed`
- TO: `### Requirement: `brd safe-push`: A branch only rebuilt on a newer baseline is pushed first`

## MODIFIED Requirements

### Requirement: `brd push`: A branch the remote has as it is is listed up to date

A named branch, or a child `--with-children` covers, that the remote has as it is (by content: `remote.up_to_date`,
a branch only rebuilt on a newer baseline too, then with `older_base: true`) MUST be listed `up_to_date` with its lease
and its pull request draft, and nothing of it pushed, as the app's push sends nothing of it; its pull request MUST NOT
be retargeted.

#### Scenario: A branch on the remote as it is
- **WHEN** a named branch is on the remote as it is
- **THEN** it is listed `up_to_date` with its lease and its pull request draft, and nothing of it is pushed
  [@test] crates/bd-remote/src/push_test.rs (`pushes_with_a_lease_and_prints_the_pull_request`)

#### Scenario: A branch only rebuilt on a newer baseline
- **WHEN** a branch was only rebuilt on a newer baseline
- **THEN** it is `up_to_date` by content and not pushed again, with `older_base: true`
  [@test] crates/bd-remote/src/push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_is_not_pushed_again`)

### Requirement: `brd safe-push`: A branch only rebuilt on a newer baseline is pushed first

A branch whose remote branch has the same commits, only rebuilt since on a newer baseline, MUST be pushed before the
start and before `--create-pull-request` opens its pull request (`push_needed`, `pushed`), so the robot and the builds
of the pull request test the current baseline: the branch itself only, never a branch it is built on, and not while a
running Safe Push holds it; one reworded since MUST be pushed too, as the robot would land the old messages.

#### Scenario: Rebuilt on a newer baseline
- **GIVEN** the remote branch has the same commits, only rebuilt since on a newer baseline
- **WHEN** the Safe Push starts or `--create-pull-request` runs
- **THEN** the branch is pushed first, leased and without `--force` (`pushed`)
- **AND** the robot starts from the pushed branch, and the pull request is opened over it
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_only_rebuilt_on_a_newer_baseline_is_pushed_before_its_start`, `the_pull_request_of_a_branch_only_rebuilt_on_a_newer_baseline_is_opened_after_its_push`)

#### Scenario: Reworded since its push
- **GIVEN** a commit of the branch was reworded since its push
- **WHEN** the Safe Push starts
- **THEN** the branch is pushed before the start
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_branch_reworded_since_its_push_is_pushed_before_the_start`)

### Requirement: `brd safe-push`: A robot running on the remote branch refuses the start

A robot running on the remote branch of a branch only rebuilt on a newer baseline since (it tests the same commits)
MUST refuse the start as one on the last commit does, before anything is pushed.

#### Scenario: A robot on the remote branch the start would start from
- **GIVEN** a robot runs on the remote branch the robot would start from
- **WHEN** the Safe Push starts
- **THEN** it is refused as for a robot on the last commit, and nothing is pushed
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_robot_on_the_last_commit_refuses_and_one_on_an_older_commit_is_canceled_when_told`, `a_branch_only_rebuilt_on_a_newer_baseline_is_pushed_before_its_start`)

## ADDED Requirements

### Requirement: `brd push`: `--rebased` pushes the branches only rebuilt on a newer baseline

`--rebased` MUST push a named branch whose remote has it only on an older baseline (`older_base`), and the branches it
is built on that are too, parents first, in the one atomic push, each leased on the remote head the sync compared with,
without `--force`: nothing is dropped, its pull request is not retargeted. It MUST NOT go with `--unapplied`; an agent
runs it only once the user approved, as any push.

#### Scenario: A stack rebuilt on a newer baseline
- **GIVEN** a stack pushed, then rebuilt on a newer baseline
- **WHEN** `brd push <child> --rebased` runs
- **THEN** the parent, then the child are `forced`, leased on the remote heads, and the next `--rebased` push lists them `up_to_date` without `older_base`
  [@test] crates/bd-remote/src/push_test.rs (`rebased_pushes_a_branch_only_rebuilt_on_a_newer_baseline_parents_first_without_force`)
