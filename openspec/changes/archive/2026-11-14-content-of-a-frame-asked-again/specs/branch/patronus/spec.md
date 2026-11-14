## MODIFIED Requirements

### Requirement: Merged: The content is asked once per state of the branches

The landing by content MUST be asked once per state of the branches (`ContentMemo`, keyed by the robot and the
commits it covers, locked only to look up and to keep answers, never while git runs), on the table the last sync left.
The memo MUST keep the latest answer of each merged robot (what the hashes told included), which a view of a state
not asked yet reads, judged against the branches as they are, and the frame MUST keep the answers its table's view reads.

#### Scenario: The memo while git runs
- **WHEN** git tells the content of a landing
- **THEN** the memo is free: it is locked only to look up and to keep answers
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`the_memo_is_free_while_git_tells_the_content`)

#### Scenario: The processes of a landing by content
- **WHEN** the landing of a merged robot is asked after a change below its commit
- **THEN** it runs within its process budget
  [@test] crates/sync-core/src/process_budget_test.rs (`the_landing_of_a_merged_robot_after_a_change_below_its_commit`)

#### Scenario: A state not asked yet
- **GIVEN** a merged robot landed a branch, by the hashes or by content, and the branch changed since
- **WHEN** the view of the branches as they are is made before a derivation of them answers
- **THEN** it says what the robot landed of the branch as it was, which the view does not know as the branch is (pending), and nothing of a robot without an answer
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_view_reads_what_the_content_told_of_a_robot_by_its_id_and_the_commits_it_covered`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say what is left of the whole stack and who landed a branch, and push it, of the table as it is while the view of the robots is of the table before`)

#### Scenario: The memo over a start
- **WHEN** a session starts from its frame
- **THEN** the memo answers what the frame kept, and the first view of the robots as they are reads it
  [@test] crates/repository-model/src/warm_start_test.rs (`a_frame_keeps_what_only_rust_reads_of_the_patronus_view_beside_its_wire`)

#### Scenario: What a frame keeps of the content
- **WHEN** the session writes its frame
- **THEN** it keeps the answers of the questions of the listed robots over the table of the frame and the latest answer of each listed robot, none of a state the table left or of a robot no longer listed
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_frame_keeps_the_answers_the_view_of_its_table_reads`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_frame_of_the_large_table_keeps_the_content_of_its_table_only`)

#### Scenario: The cost of a view made again
- **WHEN** the view of the robots as they are is made again over 141 rows and 46 robots
- **THEN** it takes at most its budget (about 2 ms in a debug build), and a publish of the same inputs and answers reads the view it kept
  [@test] crates/repository-model/src/derive_budget_test.rs (`the_view_of_the_robots_as_they_are_is_made_again_within_its_budget`)

#### Scenario: An answer since the view
- **WHEN** the memo keeps an answer after the view of the robots as they are was made, and their inputs stay
- **THEN** the next publish makes the view again with that answer
  [@test] crates/repository-model/src/facts_test.rs (`a_publish_of_the_same_inputs_reads_what_the_memo_kept_since`)

### Requirement: Pushing into a branch whose Safe Push runs: The hold is one rule of Rust for the app and brd

The hold MUST be a rule of Rust, one for the app and `brd` (`domain_policy::holds::branch_hold`, the view's
`landing.hold`): a landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push
that lands it, over the view of the robots, the table and the remote branches as they are (`patronus_view::view_of`),
which every reader reads against the branch as its row has it (`derive::hold_now`). The page MUST only word it
(`src/presenters/reasons.ts`).

#### Scenario: The order of the holds
- **WHEN** a branch is landed whole, its own Safe Push runs and another running Safe Push lands it
- **THEN** the hold is the landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push that lands it
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)

#### Scenario: The app and brd answer alike
- **WHEN** the app and `brd` decide the hold of the same branch
- **THEN** they answer alike
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A hold the view decided of a robot that ended
- **GIVEN** the view the session shows was derived while the robot that holds the branch ran (its own, or one that lands it), and the robot ended since
- **WHEN** a start of the branch, its step (`NextStepView.hold`, which the page words) or a push reads the hold
- **THEN** nothing holds the branch; while the robot runs, it is held (`derive::hold_now`: a hold while its robot holds pushes)
- **AND** a landing holds the branch only while its robot merged and every commit of the branch is one it landed
  [@test] crates/repository-model/src/derive_test.rs (`a_start_reads_the_holds_of_the_view_as_every_reader_does`)
  [@test] crates/repository-model/src/derive_test.rs (`a_push_reads_no_hold_of_what_a_view_of_another_table_says_landed`)

#### Scenario: A landing of the branch as it was
- **GIVEN** the latest answer of the content of a merged robot is of the branch as it was, and the branch has a commit since
- **WHEN** the view of the branches as they are is made
- **THEN** the view names what landed the branch as it was, and its hold is decided of the branch as the table has it: the landing holds nothing
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_merged_robot_holds_as_landed_only_the_branch_as_it_landed_it`)

#### Scenario: A robot started since the view
- **GIVEN** a Safe Push of a branch, or of its whole stack, started after the view the session shows was derived (`brd safe-push`, `brd land`), and its derivation runs or failed
- **WHEN** the robots are listed
- **THEN** the view of the robots as they are holds the push of its branch, and the robot of the whole stack its stack branch, at once
  [@test] crates/repository-model/src/derive_test.rs (`a_robot_started_since_the_view_holds_its_branch_and_the_stack_branch_before_a_derivation_answers`)
  [@test] crates/repository-model/src/session_test.rs (`a_robot_started_while_the_derivation_of_the_view_runs_holds_its_branch_at_once`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should hold the header's push and the push of a branch once their Safe Pushes started, while the view of the robots is of the robots before`)

#### Scenario: A robot that ended since the view
- **GIVEN** the robot that held a branch or refused its start ran when the view was derived, and ended since
- **WHEN** the robots are listed, before a derivation of them answers
- **THEN** it holds nothing, the step refuses no start by it, and a failed one offers its restart
  [@test] crates/repository-model/src/derive_test.rs (`a_robot_that_ended_since_the_view_refuses_no_start_and_offers_its_restart`)

#### Scenario: A table changed since the view
- **GIVEN** a sync added branches since the view was derived
- **WHEN** the table commits, before a derivation of it answers
- **THEN** what the robots land, the nearest ancestor that lands a branch and the branch of a whole stack that lands a part of it are of the table as it is
  [@test] crates/repository-model/src/derive_test.rs (`the_view_reads_the_branches_of_the_table_as_it_is_before_a_derivation_of_it_answers`)

## ADDED Requirements

### Requirement: Merged: An answer of a frame is asked again by the first derivation

An answer of the content a frame of an earlier run seeded MUST be read by the views and asked of git again by the first
derivation that needs it, so an answer of the rules of another binary lasts only until then; the answer of that
derivation MUST be kept.

#### Scenario: A frame's answer the rules of this binary contradict
- **GIVEN** a session seeded from a frame whose answer says the robot lands a branch its commit doesn't have
- **WHEN** the views of the robots are made, then the first derivation runs
- **THEN** the views read the answer of the frame, the derivation asks git again and answers by the rules of this binary, and the next asks nothing
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`an_answer_of_a_frame_is_read_by_the_views_and_asked_again_by_the_first_derivation`)

### Requirement: Merged: A content git does not tell fails the derivation

When git fails to tell the content of a robot's commit, the derivation MUST fail with the error (`Stale` with why,
`Unavailable` without an earlier view), the answers that came kept, and the next answer of Patronus MUST derive it
again; meanwhile the view MUST be of the robots as they are by what the memo has. A one-shot command of `brd` MUST
read that view.

#### Scenario: Git fails
- **WHEN** git fails to tell what a merged robot landed
- **THEN** the derivation fails with the error and the view of what the memo has, keeping what the hashes told
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_derivation_git_fails_to_tell_the_content_to_fails_with_the_view_of_what_the_memo_has`)

#### Scenario: The session over a failed content
- **WHEN** the derivation of the session fails so over an earlier view
- **THEN** the view is `Stale` with why, of the robots as they are, and the next answer of Patronus derives it again
  [@test] crates/repository-model/src/session_test.rs (`a_derivation_git_fails_to_tell_the_content_to_is_stale_and_derived_again_after_the_next_answer_of_patronus`)
