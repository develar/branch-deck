## ADDED Requirements

### Requirement: Backend: A start with --background builds no window

The app started with `--background` (the start of the bridge of an IDE) MUST build no window and, on macOS, show no
Dock icon (`ActivationPolicy::Accessory`). With no bridge connected, it MUST exit 30 s after its start.

#### Scenario: The bridge starts the app
- **WHEN** the app starts with `--background`
- **THEN** it builds no window, and the Dock shows no icon of it
- **AND** with no bridge connected it exits 30 s after its start
  [@test] crates/app-api/src/app/life_test.rs (`without_a_window_and_a_connection_the_process_exits_after_the_grace_of_the_later_of_its_windowless_start_and_its_last_connection`)
  [@test] crates/app-api/src/app/life_test.rs (`only_the_background_argument_starts_without_a_window_and_a_second_start_with_it_shows_no_main_window`)

### Requirement: Backend: The desktop app stays while an IDE is connected

When the last window closes or the user chooses Quit of the app menu (⌘Q, an item of the app, not `terminate:`) while
a bridge is connected, the windows MUST close and the process MUST stay without a window (no Dock icon on macOS). With
no window and no connection it MUST exit 30 s after the later of the moment it became windowless and the end of its
last connection. Quit of the Dock MUST still end the process.

#### Scenario: The last window closes while an IDE is connected
- **GIVEN** a bridge is connected
- **WHEN** the last window closes, or the user chooses Quit (⌘Q)
- **THEN** the windows close, the process stays, and the Dock shows no icon of it
  [@test] crates/app-api/src/app/life_test.rs (`the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected`)

#### Scenario: Quit with no IDE connected
- **GIVEN** no bridge is connected
- **WHEN** the last window closes, or the user chooses Quit (⌘Q)
- **THEN** the app quits, as an app does
  [@test] crates/app-api/src/app/life_test.rs (`the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected`)

#### Scenario: The last connection ends
- **GIVEN** the app runs without a window for an IDE
- **WHEN** the last bridge disconnects
- **THEN** the app exits 30 s later, unless a bridge connects or a window opens meanwhile
  [@test] crates/app-api/src/app/life_test.rs (`without_a_window_and_a_connection_the_process_exits_after_the_grace_of_the_later_of_its_windowless_start_and_its_last_connection`)

#### Scenario: Quit of the Dock
- **WHEN** the user chooses Quit of the menu of the Dock icon
- **THEN** the process ends, as AppKit's `terminate:` ends it with no event to keep it; the bridge of a connected IDE starts the app again or runs its fallback

### Requirement: Backend: Reopen and showMain show the main window

Reopen of macOS with no window shown (a click on the Dock icon, an open of the running app) and
`x-branch-deck/showMain` of a second start MUST build the main window, or bring it to the front, and put the app back
in the Dock. A second start with `--background` MUST show no window.

#### Scenario: The app is opened again
- **GIVEN** the app runs without a window
- **WHEN** the user opens it again, or a second start sends `x-branch-deck/showMain`
- **THEN** the main window opens and the Dock shows the app
  [@test] crates/app-api/src/app/life_test.rs (`only_the_background_argument_starts_without_a_window_and_a_second_start_with_it_shows_no_main_window`)

### Requirement: Backend: A restart keeps the windows as they are

A restart of the app (Restart of a setting, Install and Restart of an update) MUST start the new process without
`--background` when a window is open and with it when none is, whatever the arguments of the first start were.

#### Scenario: A start for an IDE shows its window, then restarts
- **GIVEN** the app started with `--background` and its main window opened since
- **WHEN** it restarts to install an update
- **THEN** the new process starts with its main window
  [@test] crates/app-api/src/app/life_test.rs (`a_restart_starts_with_a_window_when_one_is_open_and_without_one_when_none_is`)

### Requirement: Backend: The app without a window checks for updates

While no main window is open, the app MUST check for an update itself, as the page of the main window does: 3 s
after its start, then every six hours, never in a development build. The update it finds is downloaded, and Setup of
the next main window offers it.

#### Scenario: An update while the app has no window
- **GIVEN** the app runs without a window for an IDE
- **WHEN** six hours pass with an update published
- **THEN** the app downloads it, and the main window that opens next offers it in Setup
