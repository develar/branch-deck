## MODIFIED Requirements

### Requirement: Header: The fetch of an update holds no HEAD

The fetch of an update MUST hold no HEAD: a move, a park, an Unstack, a reword or an Undo during the fetch runs at
once. A Safe Push or a restart pressed during the fetch MUST wait behind the update, as its sync rebuilds the branches
they push ([Patronus](../../branch/patronus/spec.md)).

#### Scenario: A rewrite during the fetch
- **GIVEN** an update fetches
- **WHEN** a move, a park, an Unstack, a reword or an Undo starts
- **THEN** it runs at once, and the update checks and moves what it left
  [@test] crates/operations/src/update_test.rs (`a_rewrite_during_the_fetch_of_an_update_runs_at_once_and_the_update_moves_what_it_left`)
  [@test] tests/e2e/update-project/update-project.spec.ts (`should let a rewrite run during the fetch of an update, which checks and applies after it`)

#### Scenario: A start during the fetch
- **GIVEN** an update fetches
- **WHEN** a Safe Push starts
- **THEN** it waits behind the update and decides on the table of the sync the update asked
  [@test] crates/operations/src/update_test.rs (`a_start_during_the_fetch_of_an_update_waits_for_it_and_decides_on_the_table_of_its_sync`)
