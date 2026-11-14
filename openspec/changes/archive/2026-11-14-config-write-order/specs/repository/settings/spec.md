## MODIFIED Requirements

### Requirement: Scope: A window closed with valid typing saves it first

A window closed with valid typing (⌘W, the close button) MUST save it first (`saveBeforeClose`), waiting at most 3 s,
then destroy itself, a failed save too; the main window MUST tell a save that failed or did not answer by then. One
change is one write, whichever of a blur, Enter and the close saves it, and no edit is lost: one saved while a write
runs is written after it, the one of the close at once.

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
- **WHEN** the user types another value and presses Enter before that write answered
- **THEN** the first value is written, then the second, each once, and the field keeps the second
  [@test] src/lib/field-writes.test.ts (`writes an edit saved during a write after it, the latest of such saves, and the save waits for both`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write an edit typed and entered during the write of a blur once that write answered, and keep it in the field`)

#### Scenario: The window closes with an edit during the write of a blur
- **GIVEN** a field was left with valid typing, and its write runs
- **WHEN** the user types another value and the window closes before that write answered
- **THEN** the second value is written at once, numbered after the first, an edit queued before it never, and the
  window closes after both answers; the second is the value of the repository, whichever write the writer got first
- **AND** a close while the write of the same value runs writes nothing more
  [@test] src/lib/field-writes.test.ts (`writes the value of the close at once beside the write that runs, the newer one taken whichever answers first, and resolves with the failure a write answered`)
  [@test] src/stores/settings/persistent-store.test.ts (`sends the changes queued behind an unanswered patch at once with it when the window closes, numbered after it, so the writer supersedes the older`)
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should write an edit typed during the write of a blur at once when the window closes, beside that write, and keep the edit`)

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
- **THEN** its save fails with the error, the edit queued behind it is written all the same, and the next save of the
  field writes
  [@test] src/lib/field-writes.test.ts (`writes again after a write that threw: the save rejects with its error, and the next save of the field writes`, `writes the edit queued behind a write that threw, and the save of that write rejects with its error`)

#### Scenario: A close whose save gets no answer
- **WHEN** the window closes with typing whose write the writer never answers
- **THEN** the window is destroyed after 3 s all the same, and the main window tells it: the title of the failure of
  the write ("Cannot change the branch prefix of every repository"), "Branch Deck did not answer within 3 seconds",
  and Open Settings, which opens Settings on the field
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`closes Settings after a while when the save of the prefix typed gets no answer, and the main window tells it, its Open Settings on the field`)

#### Scenario: A close whose save fails
- **WHEN** the window closes with typing whose write fails
- **THEN** the window is destroyed, and the main window tells the failure by the title of the write
  (`writeErrorTitle`) and its error, with Open Settings on the field
  [@test] tests/e2e/settings/repository-settings.spec.ts (`should tell a write that fails as the window closes in the main window, whose Open Settings shows the field again`)

## ADDED Requirements

### Requirement: Scope: One writer of the git config of a repository

The app MUST write each git config file through its one writer (`config_writer`): the writes of Settings and of the
companions of a repository, or of the global config, one after the other in the order they came, never a lock held
across git. A `config.lock` of another process MUST be waited for up to a second, then fail with the reason.

#### Scenario: Two writes of the app at once
- **WHEN** the app writes two keys of a repository at once
- **THEN** both are on disk, and no write was tried again for a lock of the other
  [@test] crates/app-api/src/app/config_writer_test.rs (`two_writes_of_different_keys_at_once_are_both_on_disk`)

#### Scenario: A `config.lock` of another process
- **GIVEN** another process holds `config.lock`
- **WHEN** the app writes the config
- **THEN** the write waits for it up to a second, and fails on a lock that stays: "Cannot write branchdeck.… : another
  git process held the lock of the config file (config.lock) for more than 1 s"
  [@test] crates/sync-core/src/repository_config_test.rs (`a_write_waits_for_a_config_lock_another_process_holds`, `a_write_fails_on_a_config_lock_that_stays`)

### Requirement: Scope: The writes of a page apply newest first

Each write of a page of Settings (`set_repository_config`, `set_setting`) MUST name its page, an id drawn at its load,
and its number among the writes of the page (`order`). Its writer MUST apply it only when it is newer than the last
write of its key of that page it decided; an older one MUST write nothing and answer null, taken without an error.

#### Scenario: Two writes of one key in reverse order
- **WHEN** the writes numbered 2 and 1 of one key of the page arrive in that order
- **THEN** 2 is on disk, and 1 answers null and writes nothing
  [@test] crates/app-api/src/app/config_writer_test.rs (`an_older_write_of_a_key_of_the_page_is_superseded_and_writes_nothing`)
  [@test] crates/app-settings/src/service_test.rs (`an_older_write_of_a_key_of_the_page_is_superseded_and_changes_nothing`)
  [@test] crates/sync-utils/src/page_writes_test.rs (`an_older_write_of_a_key_of_a_page_is_superseded`)

#### Scenario: A window loaded again
- **WHEN** the page of Settings loads again, or Settings opens again, and writes a key
- **THEN** its write numbered 1 applies after the writes of the page before, whatever the host knew of the load, and
  the writes of the two pages apply in the order they come
  [@test] crates/sync-utils/src/page_writes_test.rs (`a_page_loaded_again_counts_anew_and_the_writes_of_two_pages_apply_as_they_come`)
  [@test] crates/app-settings/src/service_test.rs (`an_older_write_of_a_key_of_the_page_is_superseded_and_changes_nothing`)
  [@test] tests/e2e/inline-branch-creator-ai/inline-branch-creator-ai.spec.ts (`should turn AI on and off in Settings`)

#### Scenario: The page takes a superseded write
- **WHEN** a write of the page answers null
- **THEN** the page shows no error, takes no settings of it, and the field keeps the newer value
  [@test] src/hooks/useConfigWrite.test.ts (`numbers each write after the one before, and answers a superseded one without an error or the settings`, `tells a failed write by its title and answers its error`)
  [@test] src/lib/field-writes.test.ts (`takes nothing of a write the backend superseded, and tells no failure of it`)

## REMOVED Requirements

### Requirement: Scope: Two writes at once both land

**Reason**: The writes of the app no longer share a mutex per file held across git; each config file has its one
writer, which runs them one after the other, and a lock of another process fails with its reason.

**Migration**: "Scope: One writer of the git config of a repository".
