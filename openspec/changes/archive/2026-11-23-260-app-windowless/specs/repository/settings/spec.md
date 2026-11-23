## MODIFIED Requirements

### Requirement: The window: It closes with the main window

The window MUST close with the main window: when the main window is destroyed, every sub-window is (`close_sub_windows`
in `src-tauri/src/lib.rs`), and the app quits with the last window unless an IDE is connected to it, when it stays
without a window (openspec/specs/repository/ide-embedding/spec.md, "Backend: The desktop app stays while an IDE is
connected"). Without the main window a sub-window would wait for the answers of the main window.

#### Scenario: The main window is destroyed
- **GIVEN** the Settings window is open
- **WHEN** the main window is destroyed
- **THEN** every sub-window is destroyed too (`close_sub_windows` in `src-tauri/src/lib.rs`)
- **AND** the app quits with the last window, or stays without a window while an IDE is connected
  [@test] crates/app-api/src/app/life_test.rs (`the_windows_close_and_the_process_stays_only_while_a_bridge_is_connected`)

### Requirement: Writes through the backend: A failed save keeps the settings and is tried again

A failed save MUST keep the settings in memory and be tried again by itself (the delay doubling up to a minute) and by
the next change, and the main window MUST say so once per streak of failures ("Cannot save the settings",
`settings_save_failed`). The app MUST save what is unsaved when it is asked to quit (`RunEvent::ExitRequested`, also
when it stays without a window for an IDE), at its exit (`RunEvent::Exit`) and before Restart of the title bar setting
(`restart_app`).

#### Scenario: The directory of the settings is not writable
- **WHEN** a save fails
- **THEN** memory keeps the value, and the error is reported once a streak of failures
- **AND** the main window says "Cannot save the settings" (`settings_save_failed`)
  [@test] crates/app-settings/src/service_test.rs (`an_unwritable_directory_reports_the_error_once_a_streak_and_memory_keeps_the_value`)
  [@test] src/stores/settings/persistent-store-main.test.ts (`tells the user in the main window that the settings could not be saved`)

#### Scenario: The directory is writable again
- **GIVEN** a save failed
- **WHEN** the directory is writable again
- **THEN** the save is tried again by itself, the delay doubling up to a minute, or by the next change
  [@test] crates/app-settings/src/service_test.rs (`a_failed_save_is_retried_by_itself_once_the_directory_is_writable_again`)

#### Scenario: The app quits or restarts
- **WHEN** the app is asked to quit (`RunEvent::ExitRequested`), exits (`RunEvent::Exit`) or restarts by Restart of the title bar setting (`restart_app`)
- **THEN** it saves what is unsaved first, without waiting for the debounce
  [@test] crates/app-settings/src/service_test.rs (`a_flush_soon_after_a_patch_saves_it_without_waiting_for_the_debounce`)
