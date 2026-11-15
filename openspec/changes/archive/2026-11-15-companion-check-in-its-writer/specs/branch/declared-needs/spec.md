## MODIFIED Requirements

### Requirement: Companion repositories: A companion has one primary and a valid name

A companion MUST have one primary, checked with the write of its `branchdeck.primary`, a primary MUST be no companion,
a repository no companion of itself, and a name is letters, digits, `-`, `_` and `.`; an add that does not name the
companion in its primary MUST take back the primary it named; a removal MUST unset the primary of the companion only
while it names this repository.

#### Scenario: The writes the writer refuses
- **WHEN** a companion of a second primary, a companion of a companion, a companion of itself or a name with other characters is written
- **THEN** the writer refuses it
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

#### Scenario: Two primaries add one companion at once
- **GIVEN** two primaries add one companion at once, each past the check of its own config
- **WHEN** the side of each is written in the writer of the companion's config, one after the other
- **THEN** the first names its primary and the second is refused (a companion has one primary), and the primary of the
  second names no companion, whatever the order of the two
  [@test] crates/sync-core/src/companions_test.rs (`of_two_primaries_that_add_one_companion_at_once_the_side_of_the_second_is_refused`)
  [@test] crates/app-api/src/app/config_writer_test.rs (`of_two_primaries_that_add_one_companion_at_once_the_second_is_refused_in_the_writer_of_the_companion`)

#### Scenario: An add whose key of the primary is not written
- **GIVEN** a companion named its primary for an add
- **WHEN** the key of the primary is not written: the primary became a companion meanwhile, or another process holds the
  lock of its config past the wait of a write
- **THEN** the add fails and the primary of the companion is taken back
  [@test] crates/sync-core/src/companions_test.rs (`an_add_whose_primary_became_a_companion_meanwhile_takes_back_its_claim`, `an_add_whose_key_of_the_primary_cannot_be_written_takes_back_the_primary_of_the_companion`)
  [@test] crates/app-api/src/app/config_writer_test.rs (`an_add_whose_primary_became_a_companion_meanwhile_takes_back_the_primary_of_the_companion`)

#### Scenario: A removal
- **WHEN** a companion is removed
- **THEN** its primary is unset only while it names this repository
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)
