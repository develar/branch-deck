## ADDED Requirements

### Requirement: Writes through the backend: Two writers on one directory keep each other's changes

Each process that shares the app data directory (the app, `brd mcp` of an IDE) MUST have one writer, and a save MUST
take turns at the lock of the directory, read the file as it is and put on top only the keys and the fields its process
changed and has not saved: a change of the other process to another key or field stays, a key it removed stays removed.

#### Scenario: Two writers change different keys and fields
- **GIVEN** two writers on one app data directory
- **WHEN** each changes other keys and other fields of one key, and both save, also at once
- **THEN** the file has every change of both
- **AND** the writer that saved second has the changes of the first in memory
  [@test] crates/app-settings/src/two_processes_test.rs (`two_writers_that_change_different_keys_and_fields_keep_every_change`, `two_writers_that_save_at_once_lose_no_change_of_the_other`)
  [@test] crates/app-settings/src/merge_test.rs (`a_save_puts_only_the_keys_and_fields_this_process_changed_on_top_of_the_file_of_the_other`, `a_save_forgets_the_changes_it_wrote_and_keeps_the_ones_made_after_it_took_its_copy`)

#### Scenario: A key the other writer removed
- **WHEN** one writer removes a key or a field and saves, and the other saves a change of another key
- **THEN** the key and the field stay removed
  [@test] crates/app-settings/src/two_processes_test.rs (`a_key_the_other_writer_removed_stays_removed_after_a_save_of_this_one`)

#### Scenario: Both change the same field
- **GIVEN** a writer has an unsaved change of a field
- **WHEN** the other writer saves another value of that field
- **THEN** the writer keeps its own value, and its save writes it
  [@test] crates/app-settings/src/two_processes_test.rs (`an_unsaved_change_of_this_writer_stays_over_the_other_writers_save_of_the_same_field`)

#### Scenario: A file another program broke
- **WHEN** the file does not parse as a writer saves
- **THEN** the writer writes it again from memory and takes nothing of it
  [@test] crates/app-settings/src/two_processes_test.rs (`a_broken_file_another_program_wrote_takes_nothing_into_memory`)

### Requirement: Writes through the backend: A change another process saved reaches every window

A writer MUST take what the other process saved into memory, when the watch of the directory sees the file renamed and
with its own next save at the latest; each key is a change of its own, with the next version and the origin
`other-process`, which every window that gets the key hears as `settings_changed` and catches up with by its version.

#### Scenario: The other process saves a setting
- **WHEN** the writer of the other process saves a change
- **THEN** the watch reads the file, and the writer tells the change once, with its next version and the origin `other-process`
- **AND** every window that gets the key takes it (`settings_changed`), a page that loads catches up with it (`changes_since`)
  [@test] crates/app-settings/src/two_processes_test.rs (`a_change_another_writer_saved_reaches_the_subscribers_with_the_next_version`)

#### Scenario: A save before the watch told it
- **WHEN** a writer saves before its watch read the change of the other process
- **THEN** the save takes the change and tells it once, before its flush returns
  [@test] crates/app-settings/src/two_processes_test.rs (`a_save_takes_the_change_of_the_other_writer_and_tells_it_once_before_its_flush_returns`)
