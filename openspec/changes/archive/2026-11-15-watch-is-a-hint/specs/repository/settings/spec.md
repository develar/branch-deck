## MODIFIED Requirements

### Requirement: Writes through the backend: A change another process saved reaches every window

A writer MUST take what the other process saved into memory, when the watch of the directory sees the file renamed,
within 2 s by its look when the watch tells nothing, and with its own next save at the latest; each key is a change of its own, with the next version and the origin
`other-process`, which every window that gets the key hears as `settings_changed` and catches up with by its version.

#### Scenario: The other process saves a setting
- **WHEN** the writer of the other process saves a change
- **THEN** the watch reads the file, and the writer tells the change once, with its next version and the origin `other-process`
- **AND** every window that gets the key takes it (`settings_changed`), a page that loads catches up with it (`changes_since`)
  [@test] crates/app-settings/src/two_processes_test.rs (`a_change_another_writer_saved_reaches_the_subscribers_with_the_next_version_when_its_watch_tells_the_save`)
  [@test] crates/app-settings/src/watch_test.rs (`an_event_asks_for_its_file_and_one_that_may_be_a_change_of_either_asks_for_both`)

#### Scenario: The watch tells nothing
- **GIVEN** the watch of the directory tells no event, as under an overloaded fseventsd
- **WHEN** the writer of the other process saves a change
- **THEN** the look of the writer reads the file within 2 s and tells the change once, as at an event
  [@test] crates/app-settings/src/two_processes_test.rs (`a_change_another_writer_saved_reaches_the_subscribers_at_the_next_look_with_no_event_of_the_watch`)

#### Scenario: A save before the watch told it
- **WHEN** a writer saves before its watch read the change of the other process
- **THEN** the save takes the change and tells it once, before its flush returns
  [@test] crates/app-settings/src/two_processes_test.rs (`a_save_takes_the_change_of_the_other_writer_and_tells_it_once_before_its_flush_returns`)
