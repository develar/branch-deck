## MODIFIED Requirements

### Requirement: Robot of a row: A robot of an earlier state gives way to the git status

For a robot of an earlier state of the branch (the warning dot below, a merged robot included: the same name with new
work) the Status cell MUST show the git status of the branch as it is. The robot MUST stay in the branch preview and the
landing overview, with the notice of what changed. A merged robot MUST be of an earlier state unless it is proven to
have taken the branch as it is, also before a remote status.

#### Scenario: A finished robot of an earlier state
- **GIVEN** a finished robot of an earlier state of the branch, a merged one included
- **WHEN** the row shows
- **THEN** the cell shows the git status of the branch as it is
  [@test] crates/domain-policy/src/row_status_test.rs (`a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs`)

#### Scenario: A merged robot before a remote status
- **GIVEN** a merged robot of the branch that took other commits than the branch has, and no remote status yet
- **WHEN** the row shows, in the app or in `brd status`
- **THEN** the cell shows the branch as it is, not the robot, and nothing of the branch is landed by it ([Next Step](../next-step/spec.md#requirement-the-table-a-merged-robot-lands-only-the-commits-it-took): the branch at the robot's commit, its remote branch there with what the branch has, or what the view says it landed by content prove it)
  [@test] crates/domain-policy/src/row_status_test.rs (`a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)
