## MODIFIED Requirements

### Requirement: Get: The answer names the home of the lease and the next commands

The answer of a get MUST name where the commits of the lease land, `home`: `checkout` (the source checkout, its top
level) and `branch` (the branch it had checked out, `null` for a detached HEAD), its `notes` ("Notes" below), and
`next`, what the session runs in order.

#### Scenario: A lease with a home branch
- **WHEN** a get leases a slot from a checkout with a branch checked out
- **THEN** `next` is `brd -C <path> lease pick` after each commit that builds and passes its tests, then
  `brd lease return --lease-id <id>` once the task is done, then `brd lease notes --lease-id <id>`, which finds the
  notes of the lease after the return
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)

#### Scenario: No branch in home
- **WHEN** `home` has no branch
- **THEN** there is nothing to pick onto, and `next` is the return and the notes command
  [@test] crates/bd-lease/src/lease_pick_test.rs (`the_get_names_the_home_of_the_lease_and_the_commands_of_the_session`)

## ADDED Requirements

### Requirement: Notes: A lease has notes outside its worktree that outlive the return

The get of an agent's lease MUST make its notes, a directory beside the state of the pool (`branch-deck/notes/<lease
id>/` of the common dir, with the record `<lease id>.json`), and link `notes.local` of the worktree to it. Every return
MUST take the link out of the worktree first and keep the notes; a get MUST set aside what an earlier lease left at
`notes.local` (`left-<uuid>`), so the next holder starts empty. A clone of a template slot leaves `notes.local` out.

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

### Requirement: Notes: The notes are no change of the worktree

The get MUST add `/notes.local` to `info/exclude` of the repository where the file lacks that line, once, after what it
holds, and no status of the pool (the return, the takeover, the dirty check of a get) MAY count an untracked
`notes.local` as a local change, also where `info/exclude` lost the line.

#### Scenario: The exclude
- **WHEN** gets lease slots of a repository
- **THEN** `info/exclude` holds `/notes.local` once, after its own lines
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_are_excluded_once`)

#### Scenario: A return without the exclude
- **GIVEN** a lease with notes whose `info/exclude` lost the line
- **WHEN** the lease is returned
- **THEN** the return succeeds without a discard, and the notes stay
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_are_no_local_change_of_the_worktree`)

### Requirement: Notes: brd lease notes finds the notes of a lease

`brd lease notes` MUST name the notes of `--lease-id`, of every lease of `--holder` (got last first), else of the lease
of the worktree of `-C`, else of every lease of the pool: holder, slot, branch, directory, the times of the get and the
return, whether the pool holds it, when it expires, its bytes, and with `--list` its files (the first 1000); none is an
empty list. It is read only. `brd lease status` and the return MUST name the notes of each lease.

#### Scenario: The notes of the own lease and of a returned one
- **WHEN** `brd lease notes` runs in a leased worktree, and `brd lease notes --holder <name> --list` after its return
- **THEN** the first names the held lease, which never expires; the second the returned one, with its files and when
  it expires; the status and the return name the directory
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_lease_outlive_its_return_and_the_next_holder_of_the_slot_starts_empty`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_files_of_the_notes_are_listed_by_path`)

### Requirement: Notes: The notes of a returned lease expire

The notes of a returned lease MUST expire `branchdeck.leaseNotesKeep` after the return (`14d` by default, `0` for ever;
a lease that left the pool without a return counts from its get), deleted by `brd lease trim` (`expired_notes`;
`--plan` only names them) and by the keeper of a host, never while the pool holds the lease. `brd lease disk` MUST
count them (`notes_bytes`).

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

### Requirement: Notes: Without a link the notes are a directory the return moves

Where the system makes no link (Windows without the right to make one), `notes.local` MUST be a directory of the
worktree, the record MUST say so (`linked: false`), and the return MUST move its entries into the notes of the lease.

#### Scenario: No link
- **GIVEN** a system that makes no link
- **WHEN** a lease writes into `notes.local` and is returned
- **THEN** `notes.local` leaves the worktree and its files are in the notes of the lease
  [@test] crates/lease-pool/src/lease_notes_test.rs (`without_a_link_the_return_moves_the_notes_out_of_the_worktree`)
