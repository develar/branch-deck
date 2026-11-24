## ADDED Requirements

### Requirement: App: The app menu holds Unattended Mode, checked while it is on

The menu bar MUST hold the check item "Unattended Mode", after Settings… (the app menu of macOS, File on Windows and
Linux, File of ☰ under a custom title bar), checked while the mode is on. Choosing it MUST switch the mode through the
item of the palette, with its toasts and errors; the main window MUST check it for every value of the setting, from
any surface or `brd unattended`, and a switch on that fails MUST leave it unchecked.

#### Scenario: The item of the native menu
- **GIVEN** the native menu, built with the item checked as the stored mode
- **WHEN** the user chooses Unattended Mode, then again
- **THEN** the item checks itself back, the main window gets `unattended_mode` and turns the mode on, then off with the toast of a switch off, and checks the item each time
  [@test] src-tauri/src/menu_test.rs (`unattended_mode_is_checked_as_the_stored_mode`, `a_click_of_unattended_mode_shows_the_mode_again_and_sends_the_switch_to_the_main_window`)
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the app menu, with the toasts of the mode, and check the item as the mode is, whichever surface switched it`)

#### Scenario: Another surface, a failed switch
- **WHEN** the palette turns the mode on, Settings → General turns it off, then a switch on from the menu fails
- **THEN** the item is checked, then unchecked, and stays unchecked after the error toast
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the app menu, with the toasts of the mode, and check the item as the mode is, whichever surface switched it`)

#### Scenario: The main menu of a custom title bar
- **WHEN** the user chooses Unattended Mode in File of ☰, then again
- **THEN** the mode turns on, the item reads checked, and the mode turns off
  [@test] tests/e2e/unattended/unattended-mode.spec.ts (`should switch the mode from Unattended Mode of the main menu of a custom title bar, checked as the mode is`)
  [@test] src/lib/main-menu.test.ts (`disables Sync with the reason of the Sync button, checks Auto-sync and Unattended Mode, offers updates in a release build`)
