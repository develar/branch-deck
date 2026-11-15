## MODIFIED Requirements

### Requirement: Companion repositories: One writer writes both sides

`brd companion add <path> [--name]`, `brd companion remove <name>`, the section Companions of Settings → Landing (the
tab of the repository) and its Add Companion… (the folder dialog, named by the directory; none where the host has no
folder dialog, the IDE) and Remove MUST write both sides through one writer (`companion_write::write_companion`).

#### Scenario: Add and remove in brd
- **WHEN** `brd companion add <path> [--name]` or `brd companion remove <name>` runs
- **THEN** both sides are written through `companion_write::write_companion`
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

#### Scenario: Add Companion… and Remove in Settings
- **WHEN** the user picks a folder in the folder dialog of Add Companion… in the section Companions of Settings → Landing, the tab of the repository, then removes it
- **THEN** the companion is added under the name of its directory, then removed, both sides written
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it`)

### Requirement: Companion repositories: A companion has one primary and a valid name

A companion MUST have one primary, a primary MUST be no companion, a repository no companion of itself, a name is
letters, digits, `-`, `_` and `.`, one name names one companion and one companion has one name in its primary; an add
that does not name the companion in its primary MUST take back the primary it named; a removal MUST unset the primary
of the companion only while it names this repository.

#### Scenario: The writes the writer refuses
- **WHEN** a companion of a second primary, a companion of a companion, a companion of itself, a name with other characters, a name of another companion or a companion listed under another name is written
- **THEN** the writer refuses it
  [@test] crates/sync-core/src/companions_test.rs (`a_companion_writes_both_sides_and_resolves_the_primary_and_its_siblings`)

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

## ADDED Requirements

### Requirement: Companion repositories: A write of a companion is one write of both configs

A write of a companion MUST read and write both configs, the primary's and the companion's, under the companion locks
of both repositories (`branch-deck/companions.lock` in each common git directory, taken in the order of the
directories), and MUST ask under them which companion it touches, writing nothing when that changed; the app MUST hold
the writers of both configs for the whole write, taken in the order of their files.

#### Scenario: An add and a removal of one pair at once
- **GIVEN** a linked companion
- **WHEN** an add of it again and its removal run at once, in one process or from two, in either order
- **THEN** they run one after the other and both sides agree: unlinked after the add and the removal, linked after the
  removal and the add
  [@test] crates/app-api/src/app/config_writer_test.rs (`an_add_and_a_removal_of_one_linked_pair_at_once_run_one_after_the_other`)
  [@test] crates/sync-core/src/companion_write_test.rs (`an_add_that_waits_for_a_removal_of_another_process_links_both_sides_after_it`, `a_removal_that_waits_for_an_add_of_another_process_unsets_both_sides_after_it`)

#### Scenario: A write of Settings during a write of a companion
- **WHEN** Settings writes the config of the primary or of the companion while a write of the companion runs
- **THEN** that write runs after it, and none is tried again for a `config.lock` the other held
  [@test] crates/app-api/src/app/config_writer_test.rs (`a_write_together_holds_the_writers_of_its_configs_until_it_ends`, `a_companion_write_holds_the_writers_of_both_configs`)

#### Scenario: Two writes of one pair that name it in opposite orders
- **WHEN** two writes hold the writers of one pair of configs at once, each naming them in another order
- **THEN** both end: each takes the writers in the order of their files
  [@test] crates/app-api/src/app/config_writer_test.rs (`two_writes_together_of_one_pair_named_in_opposite_orders_both_end`)

#### Scenario: The companion of a removal changes before its locks
- **WHEN** the primary names another path for the companion of a removal before the removal took its locks
- **THEN** the removal writes nothing under them and asks again which companion it touches
  [@test] crates/sync-core/src/companion_write_test.rs (`a_removal_whose_companion_moved_before_its_locks_writes_nothing`)

### Requirement: Companion repositories: A half link is the problem of the side that has it

A link that only one side names MUST be the problem of that side in Settings → Landing and `brd companion list`: a
companion that names another primary, or none, on its row in the primary; a primary that is gone or does not list the
companion, in the companion. The next write of the pair MUST repair it. Needs of a half-linked companion are read all
the same.

#### Scenario: A companion that names no primary, or another one
- **WHEN** a companion of the primary names no primary, or another primary that does not list it
- **THEN** its row says so, the companion says its primary does not list it, and an add links both sides again
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should show a companion that names no primary as a half link, and link both sides again with Add Companion…`)

#### Scenario: A primary that does not list the companion
- **WHEN** a companion names a primary that is gone or does not list it
- **THEN** Settings of the companion say so under "A companion of …", and an add of it to any primary takes it over
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`, `a_removal_whose_companion_moved_before_its_locks_writes_nothing`)
  [@test] tests/e2e/settings/companions-settings.spec.ts (`should show the problem of a primary that does not list this companion`)

#### Scenario: A removal of a half link
- **WHEN** a companion that names another primary is removed
- **THEN** the primary unlists it and the companion keeps the primary it names
  [@test] crates/sync-core/src/companion_write_test.rs (`a_half_link_is_the_problem_of_the_side_that_has_it_and_the_next_write_repairs_it`)
