## MODIFIED Requirements

### Requirement: In the App: The queries of a plan read ahead run below the app

The queries of accio of a plan read ahead MUST run below the app, at nice 19 on Unix and the below-normal priority
class on Windows, which the processes accio starts inherit; those of a plan a window asks MUST run at the priority of
the app. A child MUST never run above the app: under an app below that priority it keeps the one of the app.

#### Scenario: A child in the background
- **WHEN** a child process starts in the background
- **THEN** it runs below the app, at nice 19 on Unix and the below-normal priority class on Windows
  [@test] crates/sync-utils/src/child_process_test.rs (`a_background_child_runs_below_the_app`)

#### Scenario: A child of an app below it
- **GIVEN** the app runs below the priority a child asks for: a niceness above it, the idle class on Windows
- **WHEN** the child starts
- **THEN** it starts, at the priority of the app
  [@test] crates/sync-utils/src/child_process_test.rs (`a_child_never_runs_above_its_parent_and_starts_whatever_priority_it_asks_for`)

#### Scenario: The queries of a plan read ahead
- **WHEN** a plan is read ahead
- **THEN** its queries of accio run at the lowest priority, and the processes accio starts inherit it
- **AND** those of a plan a window asks run at the priority of the app
  [@test] crates/file-diffs/src/accio_test.rs (`a_search_in_the_background_runs_its_queries_at_the_lowest_priority`)
  [@test] crates/app-api/src/reads/review_prefetch_test.rs (`a_commit_that_moved_a_branch_a_window_asked_reads_its_plan_ahead_once_below_the_app`)
