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
- **THEN** the step waits for Patronus (row 0), or is row 1 of the failure, and nothing of the branch is landed by the robot; the step names the robot of an earlier state as the facts there are say it, also while Patronus is pending
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_lands_only_the_commits_it_took_also_before_a_remote_status`)

#### Scenario: The whole stack
- **WHEN** a merged robot of the whole stack took an earlier tip of the stack branch, and no remote status said what the remote has
- **THEN** the whole stack is not landed and takes the step of new work, unless the view says by content that the robot landed every branch of it
  [@test] crates/domain-policy/src/next_step_test.rs (`a_merged_robot_of_the_whole_stack_lands_only_the_stack_branch_it_took`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A view of an older table
- **WHEN** the Patronus view the session shows was derived for another table (stale while the view of this one is derived) and doesn't know a commit a branch has now: one added since, or the same changes under new ids (a reword, a squash or a move of an older commit rewrites the commits above it; an update)
- **THEN** Patronus is pending for the branch and for every branch built on it but one the view knows landed whole (row 1): row 0, no start, not landed; what that view says landed it holds no push of it (its push alone, Push Stack, Push with Children, the push an action needs and its start: every reader of a hold reads it of the branch as it is, `derive::hold_now`), lands nothing a start, the count of a Safe Push or Push with Children reads, offers Push Up to Here of every commit the remote lacks, and a merge closes no pull request of it nor names it in its toast
- **AND** a whole stack with such a branch is pending too, and what is left of it, whether its own robot landed it and whether its stack branch is held as landed (`derive::stack_hold_now`) are read of the table as it is
- **AND** once the derivation of the view of this table failed (`Stale` with its reason), the step of such a branch says the failure of the view ("Refresh Patronus"), not a wait
- **AND** the page reads what landed a branch, what holds its push, the count of its Safe Push, what is left of a whole stack and what a merge landed only of Rust's answer of this table (`landedStates`, `NextStepView.hold`, `NextStep.lands`, `NextStepView.rest`, the news of the merge): the types it gets have no `landedBy`, `landedByRobot`, `StackLanding.rest` or hold of a branch of the view, so its rows, menus, popovers, push buttons and preview say no "Landed with" of such a branch, push it, and the Safe Push of the whole stack counts its commits as they are
  [@test] crates/domain-policy/src/landed_test.rs (`a_view_of_another_table_says_nothing_of_a_branch_whose_commits_it_does_not_know`)
  [@test] crates/repository-model/src/derive_test.rs (`a_merged_robot_of_earlier_commits_lands_nothing_of_the_branch_before_a_remote_status`)
  [@test] crates/repository-model/src/derive_test.rs (`a_view_of_another_table_leaves_pending_the_branches_built_on_one_it_does_not_know_and_their_whole_stack`)
  [@test] crates/repository-model/src/derive_test.rs (`a_failed_view_of_this_table_shows_its_failure_of_the_branches_the_view_before_it_does_not_know`)
  [@test] crates/domain-policy/src/next_step_test.rs (`a_subject_the_view_does_not_know_waits_for_the_view_or_shows_its_failure`)
  [@test] src/hooks/useBranchContextActions.test.ts (`names as many branches as the next step of the branch counts of the table as it is, never its own count of what landed`)
  [@test] src/hooks/useBranchContextActions.test.ts (`says what is left of the whole stack as Rust decides it of the table as it is, never what the view of the robots names`)
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed as Rust says it of the branches as they are, also one first seen merged`)
  [@test] crates/repository-model/src/derive_test.rs (`a_push_reads_no_hold_of_what_a_view_of_another_table_says_landed`)
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_as_landed_only_while_every_branch_of_it_is_what_landed`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say what is left of the whole stack and who landed a branch, and push it, of the table as it is while the view of the robots is of the table before`)
  [@test] crates/repository-model/src/derive_test.rs (`a_start_reads_no_landing_and_no_hold_of_a_branch_whose_commits_changed_since_the_view`)
  [@test] crates/repository-model/src/derive_test.rs (`push_up_to_here_reads_what_landed_only_of_a_view_that_knows_the_branch`)
  [@test] crates/repository-model/src/action_view_test.rs (`push_with_children_is_landed_once_merged_safe_pushes_landed_the_branch_and_every_branch_stacked_on_it`)
  [@test] crates/repository-model/src/facts_test.rs (`a_merge_closes_no_pull_request_of_a_branch_the_view_does_not_know`)

#### Scenario: Commits named after what landed that the branch no longer has
- **WHEN** the view knows a branch and names commits after what landed that the branch no longer has (dropped, moved to another branch)
- **THEN** every commit it has landed, and every rule reads it landed whole: what landed it, the reason of its start, its own robot
  [@test] crates/domain-policy/src/landed_test.rs (`a_view_of_another_table_says_nothing_of_a_branch_whose_commits_it_does_not_know`)
