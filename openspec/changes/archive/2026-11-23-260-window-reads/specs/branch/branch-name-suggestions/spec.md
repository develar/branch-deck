## MODIFIED Requirements

### Requirement: A newer request: A newer request kills the running claude

When a request of a window starts while an older one of the same window runs (the selection changed), the older
`claude` MUST be killed within 20 ms and answer `superseded`, which the frontend ignores; the close of the window
supersedes its request too, and a request of another window (the IDE beside the app) SHALL never supersede it.

#### Scenario: The selection changes while claude runs
- **GIVEN** a request runs `claude`
- **WHEN** a newer request starts, as the selection changed
- **THEN** the older `claude` is killed within 20 ms and answers `superseded`
- **AND** the frontend ignores it
  [@test] crates/sync-core/src/branch_name_suggestion_test.rs (`a_newer_request_kills_the_running_one`)
  [@test] crates/sync-utils/src/child_process_test.rs (`kills_the_child_when_superseded`)

#### Scenario: A superseded request another window waits for
- **GIVEN** the frontend still waits for a request, as another window started the newer one
- **WHEN** it answers `superseded`
- **THEN** it asks again next time

#### Scenario: A request of another window
- **WHEN** the main view of the IDE asks branch names while a request of the main window of the app runs
- **THEN** both run, and neither answers `superseded`
  [@test] crates/app-api/src/session/windows_test.rs (`the_main_windows_of_two_connections_never_supersede_each_other`)
