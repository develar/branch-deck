## MODIFIED Requirements

### Requirement: Robot of a row: The slot shows the next step of the branch

The slot MUST show the next step of the branch, the first row of the table of [Next Step](../next-step/spec.md) that
matches (`domain_policy::next_step`), from the robot, the pull request and the readiness Patronus answers
([below](#code-owner-review)), and every other "Safe Push" MUST be disabled with the same reason.

#### Scenario: No robot, a robot of an earlier state, a dry run
- **WHEN** the branch has no robot, one of an earlier state of the branch, or a dry run
- **THEN** the slot is "Safe Push" ([below](#starting-a-safe-push-or-a-dry-run))
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
  [@test] crates/domain-policy/src/next_step_test.rs (`row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`)

#### Scenario: A failed or canceled robot of the branch as it is
- **WHEN** the robot of the branch as it is failed or was canceled
- **THEN** the slot is "Restart Safe Push", the action of its details ([below](#restart-and-cancel)), disabled with the
  reason where a start is not possible
- **AND** "Safe Push" where Patronus can't restart it (a Safe Push of a temporary ref)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)

#### Scenario: A robot that failed for missing code-owner approvals
- **WHEN** the robot failed for missing code-owner approvals
- **THEN** the slot is "Restart Safe Push" only once Patronus says they count in the pull request it was started from
- **AND** else "Open Pull Request" (or "Create Pull Request…") while nobody can tell yet, or the wait for the approvals
  [@test] src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)

#### Scenario: The wait for the approvals that count
- **WHEN** the branch waits for the approvals that count
- **THEN** the slot is "Request Review…", "Create Pull Request…" or "Retarget #7" (to master on hover); a wait is the
  Status cell's: "Awaiting review" (whom on hover), "Changes requested", "Draft"
- **AND** a stacked branch's by its own pull request

#### Scenario: Nothing to offer
- **WHEN** a robot runs, after a merge, for a landed branch and a branch a Safe Push holds
- **THEN** the slot shows nothing (Cancel is in "⋯", the details and the landing overview)
- **AND** both starts are disabled while the Status cell says why
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)
