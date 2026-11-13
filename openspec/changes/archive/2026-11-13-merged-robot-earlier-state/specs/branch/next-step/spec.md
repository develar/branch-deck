## MODIFIED Requirements

### Requirement: The table: A merged robot lands only the commits it took

A merged Safe Push of the branch MUST land it only as the commits it took: the branch at the robot's commit, its
remote branch there with what the branch has, or all of it as it is by content (what the view says the robot landed).
A branch with other commits, or whose robot names no commit, is new work under the same name: the robot is a past
fact, the branch is not landed, and its step MUST be the one of new work, also before any remote status.

#### Scenario: The commits it took
- **WHEN** the robot merged on the branch's tip, on its remote branch with what the branch has, or the view says it landed all of the branch as it is by content
- **THEN** the branch is landed: row 1, "Merged by Patronus"
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)

#### Scenario: Commits it never took
- **WHEN** the branch has commits the merged robot never took, or Patronus named no commit of it, and no remote status said what the remote has (the first view of a session, `brd status`)
- **THEN** the branch is not landed, its Status cell and its node show it as it is, and its step is the one of new work (Safe Push, a wait for the pull requests), in the app and in `brd status`; the step names the robot of an earlier state (`NextStep.earlierRobot`)
- **AND** a failed or canceled robot keeps its slot until a remote status tells that the branch changed since
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)
  [@test] crates/domain-policy/src/row_status_test.rs (`a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: Patronus pending, or a failed sync
- **WHEN** such a branch's Patronus answer is pending, or its sync failed
- **THEN** the step waits for Patronus (row 0), or is row 1 of the failure, and nothing of the branch is landed by the robot
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)

#### Scenario: The whole stack
- **WHEN** a merged robot of the whole stack took an earlier tip of the stack branch, and no remote status said what the remote has
- **THEN** the whole stack is not landed and takes the step of new work, unless the view says by content that the robot landed every branch of it
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_of_the_whole_stack_lands_only_the_stack_branch_it_took`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A view of an older table
- **WHEN** the Patronus view the session shows was derived for an older table (stale while the view of the new one is derived) and says a branch landed whole, which has a commit since
- **THEN** the branch is not landed whole: the commit since is work left, its start is not refused as landed, and the hold of what landed holds its push no more
  [@test] crates/domain-policy/src/landed_test.rs (`a_view_of_an_older_table_lands_no_commit_added_since`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)
