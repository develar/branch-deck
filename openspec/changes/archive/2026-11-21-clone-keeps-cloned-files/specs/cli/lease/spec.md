## MODIFIED Requirements

### Requirement: Get: A clone keeps only the ignored files of its template

The clone MUST get the `.git` file of a `worktree add --no-checkout --lock`, the template's index with the stat data of
the cloned files and refreshed with the default check, then `checkout --force` to the source HEAD, the removal of the
untracked files (the discard below) and a clean `status`: the template's local changes, staged files and untracked
files never reach the lease, its ignored files do. The checkout MUST NOT write a tracked file that the clone kept
unchanged.

#### Scenario: The local work of the template
- **GIVEN** a template with local changes, staged files, untracked files and ignored files
- **WHEN** the get clones it
- **THEN** only its ignored files reach the lease
  [@test] crates/bd-lease/src/lease_test.rs (`a_new_slot_is_a_clone_of_the_source_without_its_local_work`)

#### Scenario: An unchanged tracked file
- **GIVEN** a tracked file of the template that the source HEAD does not change
- **WHEN** the get clones the template
- **THEN** the file of the slot keeps the birth time of the template, and a `status` that may write the index writes
  nothing
  [@test] crates/bd-lease/src/lease_test.rs (`a_clone_keeps_the_cloned_files_that_the_checkout_does_not_change`)

#### Scenario: The stat data without a hash
- **WHEN** the clone kept the mtime, the size and the mode of the file of an entry, and the entry is not racy
- **THEN** the entry takes the ctime, the device, the inode, the owner and the group of the file without a hash
- **AND** a racy entry, an entry of another size, a split index, a sparse index or an unknown extension is left to the
  refresh, which hashes it; the extensions that cache stat data are dropped
  [@test] crates/git-ops/src/cloned_index_test.rs (`the_stat_data_of_the_copied_files_goes_into_the_index_without_a_hash`, `a_racy_entry_and_a_changed_file_are_left_to_the_refresh`, `a_stat_cache_goes_and_an_index_it_does_not_know_stays`)

#### Scenario: A prune while the clone is away
- **WHEN** a `worktree prune` runs beside the clone, the user's or a gc's
- **THEN** it never deletes the registration, which is unlocked once the clone is at its path
  [@test] crates/git-ops/src/lease_slot_test.rs (`a_prune_while_a_cloned_slot_is_away_keeps_its_registration_until_the_unlock`)
