## MODIFIED Requirements

### Requirement: `brd status`: Each branch and whole stack shows the `next_step` of its row

Each branch, and in `stacks` each whole stack of a stack that forks, MUST show `next_step`: the step the app's row
shows by the same rule (`domain_policy::next_step`, [Next Step](../../branch/next-step/spec.md#the-table)) over what
`brd status` read.

#### Scenario: The next step of a branch and of a whole stack
- **WHEN** `brd status` runs on Space
- **THEN** each branch and each whole stack in `stacks` has `next_step`: `row` of the table (`0` waits for a fact, `1`
  has nothing to start, `1″` `pick_remote_commits`: someone else pushed to the remote branch, `brd pick --remote`
  takes it in, … `11` a Safe Push), `step`, `waits_for` (the fact without an answer, or whose read failed) and
  `pull_request`
- **AND** why a Safe Push or a Dry Run can't start now (`safe_push_blocked`, `dry_run_blocked`, `null` when it can)
- **AND** of the root of a stack that forks, which lands as its whole stack, `hosts_whole_stack`: the app's row of the
  root shows the step of its whole stack (`next_step` of the stack in `stacks`) in its slot, and offers the root's own
  step, this `next_step`, in its menu only ([Whole Stack](../../branch/whole-stack/spec.md#requirement-the-safe-push-of-the-whole-stack-in-the-app-the-row-of-its-root-hosts-it))
  [@test] crates/bd-remote/src/landed_test.rs (`status_gives_each_branch_and_whole_stack_the_next_step_of_the_apps_row`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
