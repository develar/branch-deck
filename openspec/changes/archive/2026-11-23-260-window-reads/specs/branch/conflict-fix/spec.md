## MODIFIED Requirements

### Requirement: App: The search runs when a conflict view shows, one at a time

The search MUST run when a conflict view is shown and the last sync is done, one at a time in its window; a sync that
starts MUST cancel it (`cancel_conflict_fix`), and the next view looks again; a search of another window (the IDE beside
the app) SHALL never stop it. Until it ends the callout says "Looking for a fix…" with a spinner.

#### Scenario: A conflict view shows after the sync
- **GIVEN** the last sync is done
- **WHEN** a conflict view shows
- **THEN** the search runs, and until it ends the callout says "Looking for a fix…" with a spinner
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

#### Scenario: A new search or a sync while one runs
- **WHEN** a new search starts or a sync starts while a search runs
- **THEN** the new search stops the one in flight, and a sync cancels it with `cancel_conflict_fix`
- **AND** the next view looks again
  [@test] crates/app-api/src/reads/conflict_fix_test.rs (`a_new_search_of_the_fix_stops_the_one_in_flight_and_a_cancel_the_latest`)

#### Scenario: A search of another window
- **WHEN** the IDE searches a fix, cancels it, or closes its view while a search of the app runs
- **THEN** the search of the app goes on
  [@test] crates/app-api/src/reads/conflict_fix_test.rs (`the_search_of_a_window_of_another_connection_goes_on`)
