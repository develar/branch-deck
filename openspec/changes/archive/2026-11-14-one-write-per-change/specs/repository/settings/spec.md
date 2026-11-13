## MODIFIED Requirements

### Requirement: Scope: A window closed with valid typing saves it first

A window closed with valid typing (⌘W, the close button) MUST save it first: one listener of the window waits for the
saves the fields registered (`saveBeforeClose`), then destroys the window, a failed save too: the window may destroy
itself (`core:window:allow-destroy` of `src-tauri/capabilities/settings.json`), since with that listener Tauri no longer
closes it. One change is one write, whichever of a blur, Enter and the close saves it.

#### Scenario: The window closes with typing
- **GIVEN** a field holds valid typing
- **WHEN** the window closes by ⌘W or the close button
- **THEN** the typing is saved, nothing else, and then the window is destroyed (`saveBeforeClose`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should save valid typing when the window closes, and nothing else`)

#### Scenario: The window may destroy itself
- **WHEN** the Settings window gets its close request
- **THEN** it may destroy itself (`core:window:allow-destroy` of `src-tauri/capabilities/settings.json`), after a failed save too
  [@test] src-tauri/src/title_bar_test.rs (`the_settings_window_may_destroy_itself_after_its_close_request`)

#### Scenario: The window closes as the write of a blur answers
- **GIVEN** a field was left with valid typing, and its write runs
- **WHEN** the window closes as that write answers, before the field shows the value written
- **THEN** the change is written once, and then the window is destroyed
  [@test] src/lib/field-writes.test.ts (`writes a change once when the window closes after the write of its blur answered and before the field dropped its typing`, `lets a save during a write wait for it, and writes nothing more`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write a change once when the window closes as the write of its blur answers, before the field shows it`)
