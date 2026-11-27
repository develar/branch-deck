## MODIFIED Requirements

### Requirement: The Safe Push of the whole stack: brd safe-push --whole-stack is refused unless it can land

`brd safe-push <branch> --whole-stack` MUST be refused unless the stack branch is `built` (`stack_conflict`,
`stack_not_built`, exit 4), and both ways while a robot of the stack or of a branch of it runs (`robot_running`, exit 4)
([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)), unless `--keep-running` starts it beside that robot.

#### Scenario: The stack branch conflicts
- **WHEN** the stack branch is not `built`, as it conflicts
- **THEN** `brd safe-push <branch> --whole-stack` is refused with the conflict (`stack_conflict`, else `stack_not_built`), exit 4
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_whole_stack_that_conflicts_is_refused_with_the_conflict`)

#### Scenario: A robot of the stack or of a branch of it runs
- **WHEN** a robot of the whole stack runs and a branch of it is asked, or a robot of a branch of it runs and the whole stack is asked
- **THEN** the Safe Push is refused as `robot_running`, exit 4
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_whole_stack_lands_in_one_robot_from_its_stack_branch`)

#### Scenario: Told to keep the other robot running
- **WHEN** a robot of the whole stack runs and a branch of it starts with `--keep-running`, or a robot of a branch of it
  runs and the whole stack starts with `--keep-running`
- **THEN** the Safe Push starts beside it, and nothing is canceled
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_safe_push_starts_beside_a_running_robot_of_the_whole_stack_only_when_told_to_keep_it_running`)
