## MODIFIED Requirements

### Requirement: Notes: A lease has notes outside its worktree that outlive the return

The get of an agent's lease MUST make its notes, a directory beside the state of the pool (`branch-deck/notes/<lease
id>/` of the common dir, with the record `<lease id>.json`), and link `notes.local` of the worktree to it. Every return
MUST take the link out of the worktree first and keep the notes; a get MUST set aside what an earlier lease left at
`notes.local` (`left-<uuid>`), so the next holder starts empty. A clone leaves `notes.local` out. A get whose notes
fail MUST leave none.

#### Scenario: The get links the notes
- **WHEN** an agent gets a lease
- **THEN** `notes.local` of its worktree links to the notes of the lease, which the answer names (`notes`), and what
  the agent writes there is in that directory
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_lease_outlive_its_return_and_the_next_holder_of_the_slot_starts_empty`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_of_a_lease_outlive_its_return_and_the_next_lease_starts_empty`)

#### Scenario: The return keeps the notes
- **WHEN** the lease is returned and the slot leased again
- **THEN** the link left the worktree, the notes stay with the return in their record, and the next holder's
  `notes.local` is an empty one of its own
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_lease_outlive_its_return_and_the_next_holder_of_the_slot_starts_empty`)

#### Scenario: What an earlier lease left
- **GIVEN** a `notes.local` that an earlier lease left in a slot (a brd before the notes, a return that could not move
  it)
- **WHEN** a get leases the slot
- **THEN** it goes into notes of its own, returned, and the new `notes.local` is empty
  [@test] crates/lease-pool/src/lease_notes_test.rs (`what_an_earlier_lease_left_in_notes_local_is_set_aside`)

#### Scenario: A get whose notes fail
- **GIVEN** a get whose notes fail after their directory is made (their record can't be written)
- **THEN** neither the directory nor the link stays, and the get goes on without notes
  [@test] crates/lease-pool/src/lease_notes_test.rs (`a_get_whose_notes_fail_leaves_none`)

### Requirement: Notes: The notes are no change of the worktree

The get MUST add `/notes.local` to `info/exclude` of the repository where the file lacks that line, once, after what it
holds, and no status of the pool (the return, the takeover, the dirty check of a get) MAY count an untracked
`notes.local`, or a file in it, as a local change, also where `info/exclude` lost the line.

#### Scenario: The exclude
- **WHEN** gets lease slots of a repository
- **THEN** `info/exclude` holds `/notes.local` once, after its own lines
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_are_excluded_once`)

#### Scenario: A return without the exclude
- **GIVEN** a lease with notes whose `info/exclude` lost the line
- **WHEN** the lease is returned
- **THEN** the return succeeds without a discard, and the notes stay
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_are_no_local_change_of_the_worktree`)

#### Scenario: Every untracked file shown
- **GIVEN** `notes.local` is a directory (no link), `info/exclude` lost the line, and `status.showUntrackedFiles=all`,
  so git names each file in it
- **WHEN** the lease is returned
- **THEN** the return succeeds without a discard, and the files are in the notes of the lease
  [@test] crates/bd-lease/src/notes_test.rs (`a_directory_of_notes_is_no_local_change_with_every_untracked_file_shown`)

### Requirement: Notes: The notes of a returned lease expire

The notes of a returned lease MUST expire `branchdeck.leaseNotesKeep` after the return (`14d` by default, `0` for ever;
a lease that left the pool without a return counts from its get), deleted by `brd lease trim` (`expired_notes`;
`--plan` only names them) and by the keeper of a host, never while the pool holds the lease nor where its state can't
be read. A directory without a record MUST expire by its modification time. `brd lease disk` MUST count them.

#### Scenario: The trim deletes expired notes
- **GIVEN** the notes of a lease returned 15 days ago and of a lease held for 15 days
- **WHEN** `brd lease trim` runs, after a `--plan`, and with `branchdeck.leaseNotesKeep` `0` or `30d` before
- **THEN** only the trim with the default deletes the notes of the returned lease; the plan names them, and the held
  lease keeps its notes
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_returned_lease_expire_through_the_trim_and_never_while_held`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_expire_after_the_return_and_never_while_the_lease_is_held`)

#### Scenario: The disk report
- **WHEN** a lease has notes and `brd lease disk` runs
- **THEN** its pool counts them in `notes_bytes`
  [@test] crates/bd-lease/src/notes_test.rs (`the_disk_report_counts_the_notes_of_the_pool`)

#### Scenario: A directory without a record
- **GIVEN** a directory of the notes without a record (a get that stopped before it wrote one)
- **THEN** it expires `branchdeck.leaseNotesKeep` after its modification time, never with `0` nor for a held lease
  [@test] crates/lease-pool/src/lease_notes_test.rs (`a_directory_without_a_record_expires_by_its_age`)

#### Scenario: The state can't be read
- **GIVEN** the notes of a lease past their keep, and a state of the pool that can't be read
- **WHEN** the keeper expires the notes
- **THEN** it deletes none, since no lease is known to be held; without the state file it deletes them
  [@test] crates/lease-pool/src/lease_notes_test.rs (`no_notes_expire_where_the_state_of_the_pool_cannot_be_read`)

## ADDED Requirements

### Requirement: Notes: The notes move across volumes

Where the slot and the notes are on two volumes (a `branchdeck.leasePool` on another disk, on Windows a repository on
another drive than its pool), every move of the notes (the set-aside of a get, the return of a directory) MUST copy
what no rename moves, then remove the source; a copy that fails MUST be removed, the source kept whole.

#### Scenario: Two volumes
- **GIVEN** a slot on another volume than the notes, which no rename crosses
- **WHEN** a get sets aside what an earlier lease left, and a return moves a directory of notes
- **THEN** the files are copied into the notes with their content, permissions and modification time, the slot keeps
  no `notes.local`, and no copy is left beside the notes; a copy that cannot be put in place is removed and the source
  stays
  [@test] crates/lease-pool/src/lease_notes_test.rs (`what_an_earlier_lease_left_in_notes_local_is_set_aside`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`without_a_link_the_return_moves_the_notes_out_of_the_worktree`)
  [@test] crates/sync-utils/src/fs_retry_test.rs (`a_move_across_two_volumes_copies_the_tree_and_removes_the_source`)
  [@test] crates/sync-utils/src/fs_retry_test.rs (`a_move_across_two_volumes_that_fails_keeps_the_source_whole`)
