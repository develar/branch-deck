## MODIFIED Requirements

### Requirement: Rounds: `next` names this binary in this repository

`next` MUST name this binary in this repository, as the app's prompt does, with one `-C`
(`<bd> -C <repo> update --continue --round <n>`, `<bd>` the path of this binary or `brd` when it is unknown).

#### Scenario: The command that continues
- **WHEN** a round opens
- **THEN** `next` is `<bd> -C <repo> update --continue --round <n>` with the path of this binary, or `brd` when the
  binary is unknown, and `-C` once
  [@test] crates/bd-update/src/resolve_test.rs (`a_conflict_opens_a_round_and_the_edited_copy_finishes_the_update`)
  [@test] crates/bd-cli/src/context_test.rs (`the_brd_of_a_context_names_its_repository_once`)

### Requirement: In a lease: A conflict of the update starts a resolve session in the lease

A conflict of the update that `brd lease pick` runs MUST start a resolve session as below, with `upstream` the branch;
the finish MUST print `next`, `brd -C <lease> lease pick`, its `-C` once.

#### Scenario: A lease pick that conflicts
- **WHEN** the pick of a lease conflicts
- **THEN** a resolve session opens in the lease with `upstream` the branch
- **AND** its finish prints `next`, exactly `brd -C <lease> lease pick`, which lands the lease
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`)

#### Scenario: The processes of a lease pick that conflicts
- **WHEN** the pick of a lease conflicts and its update opens the session
- **THEN** each runs the git processes of its budget
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_pick_that_conflicts`)
