## MODIFIED Requirements

### Requirement: Robot of a row: A robot of an earlier state gives way to the git status

For a robot of an earlier state of the branch (a merged one included: the same name with new work) the Status cell
MUST show the git status of the branch as it is, also where no start is offered. The robot MUST stay in the branch
preview and the landing overview, with the dot and the notice of what changed, as Rust decides it
(`NextStep.earlierRobot`). A merged robot MUST be of an earlier state unless it is proven to have taken the branch as
it is, also before a remote status.

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

#### Scenario: Where no start is offered
- **GIVEN** a failed or canceled robot of an earlier state of the branch, its remote moved past the robot's commit
- **WHEN** the sync of the branch failed, Patronus is pending, or the branch has no commits
- **THEN** the cell names the failure of the sync alone, or nothing of the robot: never the robot beside it
  [@test] crates/domain-policy/src/row_status_test.rs (`a_finished_robot_of_an_earlier_state_gives_way_where_no_start_is_offered`)

#### Scenario: The preview of a merged robot of an earlier state
- **GIVEN** a merged robot of the branch that took other commits than the branch has now
- **WHEN** the preview or the landing overview shows it
- **THEN** its chip says "Merged" with the warning dot, and the details say that the merge didn't land the newest commits, while the row offers a Safe Push
- **AND** of a merge whose commit Patronus didn't name, the details say that it may not have landed the newest commits
- **AND** while Patronus is pending for the branch, the chip has the dot of the verdict the facts there are give
  [@test] src/lib/patronus.test.ts (`marks a merged robot of an earlier state as Rust finds it, whatever the remote status loaded so far says`)
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)
  [@test] src/lib/patronus.test.ts (`says that a merge of an earlier state didn't land the newest commits, and nothing of a merge of the branch as it is`)
  [@test] src/lib/robot-overview.test.ts (`marks the merged robot of a whole stack that Rust found of an earlier state of it, and no other`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should keep a merged robot of earlier commits in the preview with its dot and what it didn't land, and offer a new Safe Push`)
