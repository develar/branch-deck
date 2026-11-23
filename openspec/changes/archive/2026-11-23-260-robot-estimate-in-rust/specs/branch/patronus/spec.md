## RENAMED Requirements

- FROM: `### Requirement: Timing: The main window keeps a history of the finished robots`
- TO: `### Requirement: Timing: The sessions keep a history of the finished robots of the repository`

## MODIFIED Requirements

### Requirement: Timing: The sessions keep a history of the finished robots of the repository

The sessions of a repository MUST keep the finished robots they saw (the robot, its profile when known, the push mode,
the target, the start, the finish, the status) in its object cache, at most 200 and none that finished more than 30
days before the newest, so the estimate survives a restart and learns beyond the 7 days of a list. Every process writes
it, merged by robot id; the first session moves `robotHistory` of `settings.json` into it and removes the key.

#### Scenario: An answer with finished robots
- **WHEN** an answer names finished robots
- **THEN** they are added to the history, never a running one
- **AND** a profile an answer named since is learnt, and a later list, which names none, keeps it
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`learns_the_finished_robots_of_an_answer_never_a_running_one_and_a_profile_an_answer_named_since`)

#### Scenario: The limits of the history
- **WHEN** the history grows
- **THEN** it keeps at most 200 robots and none that finished more than 30 days before the newest, the newest first
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`keeps_at_most_200_robots_and_none_that_finished_more_than_30_days_before_the_newest_the_newest_first`)

#### Scenario: The history of a repository
- **WHEN** a session opens, and a robot it lists finishes
- **THEN** it reads the history of its repository, and the host keeps the robot in it, merged with what other
  processes wrote
  [@test] crates/repository-model/src/session_test.rs (`the_view_estimates_the_running_robots_over_the_history_it_opened_with_and_keeps_the_robots_that_finish`)
  [@test] crates/repository-model/src/robot_history_test.rs (`the_history_of_the_repository_merges_the_writes_of_every_process_by_robot_id`)

#### Scenario: The history of an older main window
- **GIVEN** `robotHistory` in `settings.json`
- **WHEN** the first session opens
- **THEN** its robots move into the history of each repository, and the key goes
  [@test] crates/app-settings/src/robot_history_test.rs (`the_history_of_the_main_window_is_read_by_repository_and_then_forgotten`)

### Requirement: Timing: How long a robot usually runs is the median of its profile

How long a robot usually runs MUST be the median duration of the successful robots of its merge profile (those without
one for a robot without one) and push mode started in the 30 days before it, of the listed robots and the history, when
3 or more finished; only those into its target when 3 or more (`domain_policy::robot_estimate`). The view MUST say it of
each running robot (`robotEstimates`).

#### Scenario: Successful robots of the push mode
- **WHEN** successful robots of the push mode started in the 30 days before the robot
- **THEN** the usual time is their median
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`takes_the_median_of_the_successful_robots_of_the_push_mode_started_in_the_last_30_days`)

#### Scenario: Enough robots of the merge profile
- **WHEN** three robots of the same merge profile finished
- **THEN** the median takes those, and those without one for a robot without one
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`takes_the_robots_of_the_same_merge_profile_once_three_of_them_finished_and_those_without_one_for_a_robot_without_one`)
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`takes_the_sample_that_knows_the_profile_of_a_robot_both_listed_and_in_the_history`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

#### Scenario: Enough robots into the same target
- **WHEN** there are 3 or more of them into the target branch of the robot
- **THEN** only those count
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`counts_only_the_robots_into_the_same_target_once_there_are_enough_of_them`)

#### Scenario: Every window counts the same time
- **WHEN** a robot runs
- **THEN** the view of its session says its estimate, over the robots listed and the history, the same in every window
  of the app and of the IDE; a finished robot has none
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`estimates_each_running_robot_over_the_listed_robots_and_the_history`)
  [@test] crates/repository-model/src/session_test.rs (`the_view_estimates_the_running_robots_over_the_history_it_opened_with_and_keeps_the_robots_that_finish`)

### Requirement: Timing: Without the profile the estimate is the rough one of the push mode

While the profile of the robot is not known, or fewer than 3 robots of it finished, the estimate MUST be the rough one
of its push mode. A failed robot stopped early and MUST NOT count. Without such a robot the run MUST be indeterminate.

#### Scenario: The profile is not known or too few of its robots finished
- **WHEN** the profile of the robot is not known, or fewer than 3 robots of it finished
- **THEN** the estimate falls back to the push mode, roughly
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`falls_back_to_the_push_mode_roughly_while_the_profile_is_not_known_or_too_few_of_its_robots_finished`)

#### Scenario: No finished robot to learn from
- **WHEN** no successful robot finished
- **THEN** the run is indeterminate
  [@test] crates/domain-policy/src/robot_estimate_test.rs (`is_indeterminate_without_a_finished_robot_to_learn_from`)
