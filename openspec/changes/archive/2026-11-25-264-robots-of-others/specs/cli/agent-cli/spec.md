## ADDED Requirements

### Requirement: `brd robot`: A robot someone else started on the branch is reported with who started it

`brd robot <branch>` MUST list, whoever started them, the robots of the pushed branches whose robot builds the branch
(it, the branches stacked on it and the stack branch of its stack): a Safe Merge another person started from the merge
request of the branch is its robot, and `robot.started_by` names that person; the user's own robot has no `started_by`.

#### Scenario: A reviewer's Safe Merge
- **GIVEN** the branch is pushed, and another person started Safe Merge from its merge request
- **WHEN** `brd robot <branch>` runs
- **THEN** it prints that robot, its `started_by` the person
  [@test] crates/bd-remote/src/robot_test.rs (`robot_reports_a_safe_merge_someone_else_started_on_the_branch`)
