## ADDED Requirements

### Requirement: Backend: The system's stop of the app without a window is remembered

An end of the app that the app did not ask for while it has no window MUST be recorded in its data directory
(`backend-killed.json`, the time and the build) before it ends, on macOS, where Background Task Management quits such an
app about 30 s after it went windowless when Allow in the Background is off. A SIGTERM MUST quit the app as Quit does.

#### Scenario: macOS quits the app without a window
- **GIVEN** Allow in the Background is off for Branch Deck, and the app runs without a window
- **WHEN** loginwindow quits it about 30 s later
- **THEN** the app records the kill with its build before it ends
  [@test] crates/app-api/src/app/life_test.rs (`an_end_the_app_did_not_ask_for_without_a_window_is_the_systems_kill`)
  [@test] crates/backend-host/src/background_kill_test.rs (`the_record_is_read_as_the_app_wrote_it_and_a_missing_or_cut_one_is_none`)

#### Scenario: An end with a window or one the app asked for
- **WHEN** the app ends by Quit, its deadline, a restart, the close of its last window, or with a window open
- **THEN** nothing is recorded
  [@test] crates/app-api/src/app/life_test.rs (`an_end_the_app_did_not_ask_for_without_a_window_is_the_systems_kill`)

### Requirement: Bridge: The bridge starts no app that the system stops in the background

For a day after a recorded kill of its build, the bridge MUST NOT start the app without a window: it MUST serve the IDE
with its own backend and still join an app that runs. The first bridge that serves for a record MUST tell the IDE
once, "Cannot keep Branch Deck running in the background", with Open Login Items, which opens Login Items &
Extensions of System Settings.

#### Scenario: The IDE opens a project after a kill
- **GIVEN** the system stopped the app without a window 10 minutes ago
- **WHEN** the bridge finds no backend
- **THEN** it serves the IDE with its own backend and starts no app
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)
  [@test] crates/backend-host/src/background_kill_test.rs (`a_bridge_starts_the_app_but_within_a_day_of_a_kill_of_its_build`)

#### Scenario: The app the bridge joined is killed
- **WHEN** the system stops the app a bridge joined
- **THEN** the bridge ends with 1, the IDE starts it again once, and the new bridge serves the IDE with its own backend
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)

#### Scenario: The user hears it once
- **WHEN** two IDEs serve themselves for one kill
- **THEN** the first one tells "Cannot keep Branch Deck running in the background" with Open Login Items, the second tells nothing
  [@test] crates/backend-host/src/background_kill_test.rs (`only_the_first_claim_of_a_record_tells_it_and_a_new_record_is_told_again`)
  [@test] crates/ide-host/src/server_test.rs (`the_notice_of_the_start_of_the_bridge_reaches_the_ide_once_it_initialized`)

#### Scenario: The user opens the app after a kill
- **GIVEN** the bridge serves the IDE itself after a kill
- **WHEN** the user opens the app
- **THEN** the bridge goes back to it, as from any fallback
  [@test] crates/ide-host/src/start_test.rs (`the_bridge_joins_waits_starts_the_app_or_serves_alone_by_what_it_finds`)
