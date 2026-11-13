## MODIFIED Requirements

### Requirement: Merged: The content is asked once per state of the branches

The landing by content MUST be asked once per state of the branches (`ContentMemo`, keyed by the robot and the
commits it covers, locked only to look up and to keep answers, never while git runs), on the table the last sync left.
The memo MUST keep the latest answer of each merged robot (what the hashes told included), which a view of a state
not asked yet reads, judged against the branches as they are, and the frame of the session MUST keep the memo.

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
- **THEN** the memo answers what it answered before, and the first view of the robots as they are reads it
  [@test] crates/repository-model/src/warm_start_test.rs (`a_frame_keeps_what_only_rust_reads_of_the_patronus_view_beside_its_wire`)

#### Scenario: The cost of a view made again
- **WHEN** the view of the robots as they are is made again over 141 rows and 46 robots
- **THEN** it takes at most its budget (about 2 ms in a debug build), and a publish of the same inputs and answers reads the view it kept
  [@test] crates/repository-model/src/derive_budget_test.rs (`the_view_of_the_robots_as_they_are_is_made_again_within_its_budget`)

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

### Requirement: Pushing into a branch whose Safe Push runs: The stack branch of a whole stack is held as a branch is

The stack branch of a whole stack ([Whole Stack](../whole-stack/spec.md)) MUST be held as a branch is, by one rule of
Rust for the app and `brd push --with-children` (`domain_policy::holds::stack_branch_hold`), in this order: a parked
member; merged Safe Pushes that landed every branch of the stack whole; its own running robot; a stack branch the sync
did not build. Every reader MUST decide it of the table and the robots as they are (`derive::stack_hold_now`).

#### Scenario: A parked member
- **GIVEN** a member of the whole stack is parked
- **WHEN** the hold of the stack branch is decided
- **THEN** the parked member keeps it local: "Holds parked login: land the stack in pieces"
  [@test] crates/domain-policy/src/holds_test.rs (`a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build`)
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)

#### Scenario: Merged Safe Pushes landed every branch of the stack
- **GIVEN** merged Safe Pushes, of pieces or of the whole stack, landed every branch of the stack whole
- **WHEN** the hold of the stack branch is decided
- **THEN** the stack branch is held, and the reason says who landed them, each robot once, in the order they merged: "Landed in master with deploy and side-test: nothing of it is left to push"
- **AND** "Merged by Patronus: …" when its own robot alone landed them
  [@test] crates/domain-policy/src/holds_test.rs (`every_member_landed_by_the_robot_of_the_whole_stack_is_its_own_landing`)
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says`)
  [@test] crates/operations/src/pull_request_test.rs (`the_pull_request_of_a_whole_stack_pushes_its_stack_branch_by_the_table_as_it_is`)
  [@test] src/presenters/reasons.test.ts (`says that nothing is left to push of a stack branch merged Safe Pushes landed, and nothing for no hold`)

#### Scenario: The landed stack branch is not pushed with what is left
- **GIVEN** every branch of the stack landed, and commits are left above it
- **WHEN** the user pushes the stack
- **THEN** the push of the whole stack branch is held, and what is left pushes without the landed ones
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: Its own running robot
- **GIVEN** the Safe Push of the whole stack runs on the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held: "A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack"
- **AND** the step of the whole stack names that robot as what holds it (`NextStepView.hold`), which the page reads
  [@test] crates/repository-model/src/derive_test.rs (`the_step_of_a_whole_stack_names_its_own_robot_only_while_it_holds_the_stack_branch`)
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)

#### Scenario: A stack branch the sync did not build
- **GIVEN** the sync did not build the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held

#### Scenario: A view of another table
- **GIVEN** the view the session shows was derived for another table or other robots: it says every branch of the stack landed and a branch has a commit since, it holds nothing and a branch was parked since, it says a branch is parked and the branch was unparked since, it says the sync did not build the stack branch and the sync built it since, or it says the robot of the whole stack runs and the robot ended since
- **WHEN** the stack branch is pushed with the children of the root, alone, or first by the creation of the pull request of the whole stack, or its step or its start reads the hold
- **THEN** it is held as the table and the robots are now: it goes while nothing holds it now, and stays local while a branch of it is parked now
- **AND** the view gives only what it alone knows: what landed each branch, the robot of the whole stack
  [@test] crates/repository-model/src/derive_test.rs (`the_stack_branch_is_held_of_the_table_as_it_is_never_as_a_view_of_another_one_says`)
  [@test] crates/operations/src/push_test.rs (`the_stack_branch_goes_by_the_branches_and_the_robots_as_they_are_whatever_a_view_of_another_table_held_it_by`)
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)
  [@test] crates/operations/src/pull_request_test.rs (`the_pull_request_of_a_whole_stack_pushes_its_stack_branch_by_the_table_as_it_is`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should take the whole stack branch along in the header's push once its own robot ended, while the view of the robots says it runs`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should leave the whole stack branch out of the header's push once a branch of it is parked, while the view of the robots is of the table before`)
