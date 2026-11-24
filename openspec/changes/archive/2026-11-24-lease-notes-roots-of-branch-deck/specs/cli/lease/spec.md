## MODIFIED Requirements

### Requirement: Notes: The notes of a brd before are read where they are

The root of a new lease MUST follow from its slot, `<pool dir>/<n>/<name>` → `<pool dir>/notes`, in a pool directory of
Branch Deck (the config's, one of `~/.branch-deck/pool`, a marked root), else `notes` of the get's pool directory. A
reader MUST look there, then in `branch-deck/notes/` of the common dir of a brd before, read and expired, never moved.
A fetch of brd-host MUST write through `notes.local` only into notes whose record names its checkout as the slot.

#### Scenario: The root of a slot
- **WHEN** a get leases a slot `<pool dir>/<n>/<name>`, or a worktree outside the pool directory
- **THEN** the notes are in `notes` of that pool directory, or of the pool directory of the get
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_root_of_the_notes_follows_from_a_slot_of_a_pool_directory_only`)

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

## ADDED Requirements

### Requirement: Notes: The expiry deletes only the notes of Branch Deck of its repository

The expiry MUST delete a directory without a record only in a root Branch Deck made (its marker `.branch-deck-notes`,
the old place in the common dir), and only the notes of its own repository (`repository` of the record, else the one
of its slot), so neither a user's `notes` beside an adopted worktree nor the held lease of another repository that
shares the pool directory loses a file. `brd lease notes` lists only the notes of its repository.

#### Scenario: A user's notes beside an adopted worktree
- **GIVEN** a worktree adopted where it is, `~/work/2026/repo-wt`, and a `~/work/notes` of the user with old files
- **WHEN** the notes are read and expired
- **THEN** that directory is no place of the notes, and nothing in it is deleted
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_of_a_user_beside_an_adopted_worktree_are_never_read_nor_deleted`)

#### Scenario: Two repositories in one pool directory
- **GIVEN** a pool directory that two repositories share, a returned lease of one and a held lease of the other
- **WHEN** the first expires its notes past their keep
- **THEN** only its own returned notes go; the held notes of the other stay, and it lists none of them
  [@test] crates/lease-pool/src/lease_notes_test.rs (`a_shared_pool_directory_expires_only_the_notes_of_its_own_repository`)
