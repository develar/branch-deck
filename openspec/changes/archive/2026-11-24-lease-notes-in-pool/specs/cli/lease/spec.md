## MODIFIED Requirements

### Requirement: Notes: A lease has notes outside its worktree that outlive the return

The get of an agent's lease MUST make its notes in the pool directory (`notes/<lease id>/`, the record `<lease
id>.json` beside) and link `notes.local` of the worktree to them. A return MUST take the link out first and keep the
notes; a get MUST set aside what an earlier lease left at `notes.local` (`left-<uuid>`). A clone leaves `notes.local`
out; a get whose notes fail MUST leave none. No slot scan, trim, disk report or status MAY take `notes/` for a slot.

#### Scenario: The get links the notes
- **WHEN** an agent gets a lease
- **THEN** `notes.local` of its worktree links to the notes of the lease in the pool directory, which the answer names
  (`notes`), and what the agent writes there is in that directory
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

#### Scenario: The notes are no slot
- **GIVEN** the notes of a returned lease beside the slots, and a worktree an agent made in notes
- **WHEN** a trim deletes every free slot of the pool, and the pools of the machine are listed
- **THEN** the notes stay whole and listed, no status names a slot in them, and the worktree in them names no pool
  [@test] crates/bd-lease/src/notes_test.rs (`a_trim_of_every_slot_keeps_the_notes_in_the_pool_directory`)
  [@test] crates/lease-pool/src/trim_test.rs (`a_worktree_in_the_notes_of_a_pool_directory_names_no_pool`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_expiry_deletes_nothing_beside_the_notes_in_the_pool_directory`)

### Requirement: Notes: brd lease notes finds the notes of a lease

`brd lease notes` MUST name the notes of `--lease-id`, of every lease of `--holder` (got last first), else of the lease
of the worktree of `-C`, else of every lease of the pool: holder, slot, branch, directory, the times of the get and the
return, whether it is held, when it expires, its bytes, with `--list` its files (the first 1000). It is read only.
`brd lease status` and the return MUST name the notes of each lease, wherever they are.

#### Scenario: The notes of the own lease and of a returned one
- **WHEN** `brd lease notes` runs in a leased worktree, and `brd lease notes --holder <name> --list` after its return
- **THEN** the first names the held lease, which never expires; the second the returned one, with its files and when
  it expires; the status and the return name the directory
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_lease_outlive_its_return_and_the_next_holder_of_the_slot_starts_empty`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_files_of_the_notes_are_listed_by_path`)

### Requirement: Notes: The notes of a returned lease expire

The notes of a returned lease MUST expire `branchdeck.leaseNotesKeep` after the return (`14d` by default, `0` for ever;
without a return from its get), deleted by `brd lease trim` (`expired_notes`; `--plan` only names them) and the keeper
of a host in every place of the notes, never while the pool holds the lease, where its state can't be read, nor
anything else of the pool directory. A directory without a record MUST expire by its modification time. `brd lease
disk` MUST count them.

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

### Requirement: Notes: The notes of a brd before are read where they are

The root of the notes of a new lease MUST follow from its slot: `<pool dir>/<n>/<name>` → `<pool dir>/notes`. A
reader MUST look there for each slot of the state and the pool directory it knows, then in `branch-deck/notes/` of the
common dir of a brd before, whose notes stay there, read and expired, never moved. A fetch of brd-host MUST write
through `notes.local` only into notes whose record names its checkout as the slot.

#### Scenario: The root of a slot
- **WHEN** a get leases a slot `<pool dir>/<n>/<name>`, or a worktree outside the pool directory
- **THEN** the notes are in `notes` of that pool directory, or of the pool directory of the get
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_root_of_the_notes_follows_from_the_slot`)

#### Scenario: The notes of a brd before
- **GIVEN** the notes of a lease in `branch-deck/notes/` of the common dir
- **WHEN** `brd lease notes --lease-id` runs, the lease is returned, and its notes pass their keep
- **THEN** they are named there with their files, nothing moves them, the return closes them where the link points,
  and the trim deletes them there
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_brd_before_are_named_where_they_are_and_expire_there`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_of_a_brd_before_are_read_where_they_are_and_expire_there`)

#### Scenario: A fetch of brd-host
- **WHEN** a fetch of brd-host writes through `notes.local` to notes of a lease in the pool directory
- **THEN** it writes there while their record names the checkout as the slot, and takes no write while it names
  another
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_writes_through_notes_local_only_into_the_notes_of_the_lease_of_its_checkout`)
