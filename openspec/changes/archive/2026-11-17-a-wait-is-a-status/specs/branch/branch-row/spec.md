## ADDED Requirements

### Requirement: Status cell: The state the next step waits in is a status, never an action

Where no fact of the branch's own says more, the first column MUST say the state its next step waits in
(`RowFact::Wait`, [Next Step](../next-step/spec.md)), and the slot MUST show no step of it: the Status column says
where the work stands and whose move it is, the Actions column only what the user can do.

#### Scenario: A branch awaiting a review
- **WHEN** the next step of a branch waits for the reviewers of its pull request, and nothing of its own says more
- **THEN** the first column says "Awaiting review" ("Changes requested" in `destructive`, "Draft", "Into release-252", "In #12", "Merges when checks pass"), muted, its icon where the dot is, truncated to the track; its hover the whole state
- **AND** the slot of the Actions shows no step, and a click on the status opens the pull request
  [@test] crates/domain-policy/src/row_status_test.rs (`a_wait_is_the_state_of_the_cell_where_nothing_of_its_own_says_more`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

## MODIFIED Requirements

### Requirement: Status cell: Work in flight never hides behind the state of the row

Work in flight MUST NOT hide behind the state of the row, nor the state behind it: the first that applies is the first
column, a running robot, then what landed all of it ("landed", its own merged robot "Merged"), a failed sync ("merge
conflict", "analyzing conflict…"), parked, a blocked stacked branch ("blocked by auth"), the state its next step
waits in where its robot finished on the branch as it is covers nothing ("Awaiting review"), its robot finished on the
branch as it is.

#### Scenario: The order of the first column
- **WHEN** several of these facts apply to a branch
- **THEN** the first that applies is the first column
- **AND** only a Safe Push that lands it outranks a block, and nothing outranks a fix but master
  [@test] crates/domain-policy/src/row_status_test.rs (`only_a_safe_push_that_lands_it_outranks_a_block_and_nothing_outranks_a_fix_but_master`)

### Requirement: Actions cell: On Space the Safe Push slot shows the next step

On Space (Patronus answered), the Safe Push slot MUST show the next step of the branch, the first row of the table of
[Next Step](../next-step/spec.md) that matches. The robot itself is in the Status cell. Nothing in the slot changes on
the row hover.

#### Scenario: The steps of the slot
- **WHEN** Patronus answered and the slot shows the next step of a branch
- **THEN** it is "Safe Push" for a branch with a push button and nothing in its way (a click starts it at once when it has nothing to say, else opens its popover at the slot,
  [Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run))
- **AND** the step after a robot ("Restart Safe Push")
- **AND** no step of a wait ("Awaiting review", "Changes requested", "Draft"): the Status cell says it
- **AND** the action that helps ("Create Pull Request…", "Request Review…", "Retarget #7")
- **AND** a stacked branch's own step ("Safe Push · 3"), its popover at its own row
- **AND** nothing while a robot runs, after a merge, for a landed or held branch ([Patronus](../patronus/spec.md#robot-of-a-row))
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
