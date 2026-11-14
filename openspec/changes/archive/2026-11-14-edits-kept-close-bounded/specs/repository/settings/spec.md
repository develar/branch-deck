## MODIFIED Requirements

### Requirement: Scope: A window closed with valid typing saves it first

A window closed with valid typing (⌘W, the close button) MUST save it first: one listener of the window waits for the
saves the fields registered (`saveBeforeClose`) for at most 3 s (`CLOSE_SAVE_LIMIT_MS`), then destroys the window, a
failed save too (it may destroy itself, `core:window:allow-destroy`). One change is one write, whichever of a blur,
Enter and the close saves it, and no edit is lost: one saved while a write runs is written after it.

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
  [@test] src/lib/field-writes.test.ts (`writes a change once when the window closes after the write of its blur answered and before the field dropped its typing`, `lets a save of the same value during a write wait for it, and writes nothing more`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write a change once when the window closes as the write of its blur answers, before the field shows it`)

#### Scenario: An edit during the write of a blur
- **GIVEN** a field was left with valid typing, and its write runs
- **WHEN** the user types another value and presses Enter, or the window closes, before that write answered
- **THEN** the first value is written, then the second, each once; the field keeps the second, and the window closes
  after both
  [@test] src/lib/field-writes.test.ts (`writes an edit saved during a write after it, the latest of such saves, and the save waits for both`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write an edit typed during the write of a blur once that write answered, when the window closes`, `should write an edit typed and entered during the write of a blur once that write answered, and keep it in the field`)

#### Scenario: An edit back to no value during the write of a blur
- **GIVEN** a field was left with "acme/one", and its write runs
- **WHEN** the user empties the field and presses Enter before that write answered
- **THEN** "acme/one" is written, then no value, and the field is empty
  [@test] src/lib/field-writes.test.ts (`writes a value saved during a write back to the one the field showed before it, which the field still shows then`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write the field emptied and entered during the write of a blur once that write answered, back to no value`)

#### Scenario: An edit unsaved during the write of a blur
- **WHEN** the user types another value while the write of a blur runs, and saves nothing
- **THEN** the field keeps that typing once the write answered, and only the first value is written
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should keep an edit typed during the write of a blur in the field once that write answered, unsaved`)

#### Scenario: A write that threw
- **WHEN** a write of a field throws
- **THEN** its save fails with the error, and the next save of the field writes
  [@test] src/lib/field-writes.test.ts (`writes again after a write that threw: the save rejects with its error, and the next save of the field writes`)

#### Scenario: A close whose save gets no answer
- **WHEN** the window closes with typing whose write the writer never answers
- **THEN** the window is destroyed after 3 s all the same
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`closes Settings after a while when the save of the prefix typed gets no answer, as after a failed save`)
