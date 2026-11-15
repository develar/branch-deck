## MODIFIED Requirements

### Requirement: Companion repositories: A companion has one primary and a valid name

A companion MUST have one primary, a primary MUST be no companion, a repository no companion of itself, a name is
letters, digits, `-`, `_` and `.`, one name names one companion and one companion has one name in its primary; an add
that does not name the companion in its primary MUST take back the primary it named; a removal MUST unset the primary
of the companion only while it names this repository.

#### Scenario: The writes the writer refuses
- **WHEN** a companion of a second primary, a companion of a primary that can not be read, a companion of a companion, a companion of itself, a name with other characters, a name of another companion or a companion listed under another name is written
- **THEN** the writer refuses it
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)
  [@test] crates/sync-core/src/companion_write_test.rs (`a_companion_of_a_primary_whose_config_fails_to_read_is_not_taken_over`)

#### Scenario: Two primaries add one companion at once
- **GIVEN** two primaries add one companion at once, in one process or from two
- **WHEN** the second waits for the companion lock the first holds
- **THEN** it reads what the first wrote and is refused (a companion has one primary), and its primary names no
  companion, whatever the order of the two
  [@test] crates/sync-core/src/companion_write_test.rs (`of_two_primaries_that_add_one_companion_from_two_processes_the_second_reads_the_first_and_is_refused`)
  [@test] crates/app-api/src/app/config_writer_test.rs (`of_two_primaries_that_add_one_companion_at_once_the_second_is_refused`)

#### Scenario: An add whose key of the primary is not written
- **GIVEN** a companion named its primary for an add
- **WHEN** the key of the primary is not written: another process holds the lock of its config past the wait of a write
- **THEN** the add fails and the primary of the companion is taken back
  [@test] crates/sync-core/src/companions_test.rs (`an_add_whose_key_of_the_primary_cannot_be_written_takes_back_the_primary_of_the_companion`)

#### Scenario: A removal
- **WHEN** a companion is removed
- **THEN** its primary is unset only while it names this repository
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

### Requirement: Companion repositories: A half link is the problem of the side that has it

A link that only one side names MUST be the problem of that side in Settings → Landing and `brd companion list`: a
companion that names another primary, or none, on its row in the primary; a primary that is gone, can not be read or
does not list the companion, in the companion, which MUST offer Unlink. The next write of the pair MUST repair it, but a
primary that can not be read is unknown, not absent: its companion MUST NOT be taken over. Needs of a half-linked
companion are read all the same.

#### Scenario: A companion that names no primary, or another one
- **WHEN** a companion of the primary names no primary, or another primary that does not list it
- **THEN** its row says so, the companion says its primary does not list it, and an add links both sides again
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should show a companion that names no primary as a half link, and link both sides again with Add Companion…`)

#### Scenario: A primary that does not list the companion
- **WHEN** a companion names a primary that can be read and does not list it
- **THEN** Settings of the companion say so under "A companion of …", and an add of it to any primary takes it over
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`, `a_removal_whose_companion_moved_before_its_locks_writes_nothing`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should show the problem of a primary that does not list this companion, and unlink it`)

#### Scenario: A primary that can not be read
- **WHEN** a companion names a primary whose path is gone (a disk not mounted now), holds no git repository, or whose
  config fails to read, and another primary adds it
- **THEN** the add is refused with the primary it names and how to unlink it (remove it there once it can be read, or
  Unlink in the companion), and the companion keeps its primary
  [@test] crates/sync-core/src/companion_write_test.rs (`a_companion_of_a_primary_whose_config_fails_to_read_is_not_taken_over`, `a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`)

#### Scenario: Unlink in the companion
- **WHEN** the user picks Unlink in Settings → Landing of a companion whose link is half or broken, or an agent runs
  `brd companion unlink` there
- **THEN** its `branchdeck.primary` goes and it is a repository of no primary; refused while the primary lists it
  (remove it there) and in a repository that names none
  [@test] crates/sync-core/src/companion_write_test.rs (`unlink_takes_back_the_side_of_a_half_link_on_the_companion_only`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should show the problem of a primary that does not list this companion, and unlink it`)

#### Scenario: A removal of a half link
- **WHEN** a companion that names another primary is removed
- **THEN** the primary unlists it and the companion keeps the primary it names
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`)
