## MODIFIED Requirements

### Requirement: `brd status`: Each branch shows the `status` its Status cell says

The `next_step` of a branch MUST show `status`, what its Status cell in the app says by the same rule
(`domain_policy::row_status`), the main fact first: `robot_running` before `blocked`, so a step settled by a running
robot says it.

#### Scenario: The status of a branch
- **WHEN** `brd status` gives the next step of a branch
- **THEN** its `status` is one of `robot_running`, `blocked`, `robot_finished`, `robot_failed`, `landed`,
  `sync_error`, `merge_conflict`, `analyzing_conflict`, `parked`, `waits` (the step is a state it waits in), the main
  fact first
- **AND** it is omitted when only the push state is to say
  [@test] crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)
