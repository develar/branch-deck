## MODIFIED Requirements

### Requirement: Socket: The stale side yields to a client of another build

An `initialize` whose `clientInfo.build` is not the host's MUST be `-32005 otherBuild` with the host's build and
`exits`. The side whose executable on disk is no longer its build yields: a stale backend without a window exits after
the answer, a stale backend with a window stays and its main window MUST offer Restart to Update once, and a stale
bridge exits with 1.

#### Scenario: The app updated in place, no window
- **WHEN** a bridge of the new build connects to a backend of the old one that has no window
- **THEN** the answer is `otherBuild` with `exits: true`, and the backend exits after it
  [@test] crates/backend-host/src/connection_test.rs (`a_stale_backend_without_a_window_exits_after_its_answer`)
  [@test] crates/backend-host/src/identity_test.rs (`an_executable_replaced_on_disk_is_stale`)

#### Scenario: The app updated in place, a window open
- **WHEN** a bridge of the new build connects to a backend of the old one that shows a window
- **THEN** the answer is `otherBuild` with `exits: false`, and the backend stays and tells Restart to Update
  [@test] crates/backend-host/src/connection_test.rs (`a_stale_backend_with_a_window_stays_and_tells_restart_to_update`)

#### Scenario: Restart to Update
- **GIVEN** the binary of the app was replaced on disk while its main window is open
- **WHEN** a bridge of the new build asks the backend, once or again
- **THEN** the main window shows one toast "A newer Branch Deck is installed", and its Restart to Update restarts the
  app with its windows as they are
  [@test] tests/e2e/repository/restart-to-update.spec.ts (`a binary of the app replaced while its window is open is one toast of the main window, whose Restart to Update restarts the app`)
  [@test] crates/app-api/src/app/life_test.rs (`a_replaced_binary_is_told_to_the_main_window_once`)
