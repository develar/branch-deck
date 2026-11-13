## ADDED Requirements

### Requirement: The table: A merged robot lands only the commits it took

A merged Safe Push of the branch MUST land it only as the commits it took: the branch at the robot's commit, its
remote branch there with what the branch has, or all of it as it is by content (what the view says the robot landed).
A branch with other commits is new work under the same name: the robot is a past fact, the branch is not landed, and
its step MUST be the one of new work, also before any remote status, while Patronus is pending and while the sync
failed.

#### Scenario: The commits it took
- **WHEN** the robot merged on the branch's tip, on its remote branch with what the branch has, or the view says it landed all of the branch as it is by content
- **THEN** the branch is landed: row 1, "Merged by Patronus"
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)

#### Scenario: Commits it never took
- **WHEN** the branch has commits the merged robot never took, and no remote status said what the remote has (the first view of a session, `brd status`), Patronus is pending or the sync failed
- **THEN** the branch is not landed, its Status cell and its node show it as it is, and its step is the one of new work (Safe Push, a wait for the pull requests), in the app and in `brd status`
- **AND** a failed or canceled robot keeps its slot until a remote status tells that the branch changed since
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)
  [@test] crates/domain-policy/src/row_status_test.rs (`a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

## MODIFIED Requirements

### Requirement: The table: The step says why nothing of the branch is left to land

The step MUST say why nothing of the branch is left to land (`BranchNextStep.landed`, `formatLandedWhole`: the words of
the view's landed state, `formatLanded`; before the view of the robots, the robot of the row that merged the branch as
it is), also while another reason of row 1 (a sync) disables its start.

#### Scenario: The words of what landed it
- **WHEN** nothing of the branch is left to land, also while it syncs
- **THEN** the step says "Landed in origin/master", "Landed in master with oauth", "Merged by Patronus" or "Pull request #7 is merged"
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
  [@test] src/presenters/next-step.test.ts (`row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed`)
  [@test] src/presenters/reasons.test.ts (`words what landed a subject: the baseline, merged Safe Pushes, its own robot, its pull request`)
