## ADDED Requirements

### Requirement: Pick: Each step goes to stderr as it starts and into the answer

`brd pick` and `brd lease pick` MUST print each step on stderr as it starts, with the time since the pick began
(`brd pick: [<time>] <step>`): each check with its round, the wait for the repository lock, the moves, the read of the
landing, and of `brd lease pick` the update of the lease after a conflict. The answer MUST list the same steps with
when each started (`steps`, `at_ms`). Nothing MUST be printed between the move of the branch and the end of the moves.

#### Scenario: The steps of a pick
- **WHEN** a pick with `--follow` lands its commits
- **THEN** stderr gets the check, the moves of the branch and both worktrees and the read of the landing, in order
- **AND** the answer lists them in that order, each with when it started; a dry run lists only the check
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_tells_each_step_with_when_it_started`)

#### Scenario: A step names when it started
- **WHEN** a step starts 3 min 12 s after the pick began
- **THEN** its line is `brd lease pick: [3 min 12 s] <step>`, so a slow step shows in the gap to the next line
  [@test] crates/bd-update/src/pick_test.rs (`a_step_on_stderr_names_when_it_started`)

#### Scenario: The steps of a lease pick that conflicts
- **WHEN** `brd lease pick` conflicts and updates the lease onto the tip of its home branch
- **THEN** its steps are the check and the update of the lease onto that tip
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)
