## MODIFIED Requirements

### Requirement: The fetch, the outputs and the changes: A write of a fetch follows no link and is whole

A write of a fetch MUST NOT follow a local link, but `notes.local` at the root of the checkout while it links to the
notes of a lease of the same repository: an absolute target without `.` or `..`, a directory and no link, right in
`$GIT_COMMON_DIR/branch-deck/notes/`. Each file MUST go to `<name>.bd-host-part` first and take its name only at the
size that the agent named.

#### Scenario: A local link on the path of a file
- **WHEN** a path of the fetch goes through a local link: to a directory outside the notes of the repository, to a link,
  by a target with `..` or a relative one, to the notes under another name than `notes.local` at the root, or a link
  inside the notes of the lease
- **THEN** no write follows the link
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_takes_only_what_the_record_names_and_never_writes_through_a_link`, `a_link_that_is_no_notes_of_a_lease_of_the_repository_takes_no_write`, `a_link_inside_the_notes_of_a_lease_takes_no_write`, `a_relative_link_to_the_notes_of_a_lease_takes_no_write`)

#### Scenario: The notes of a lease
- **WHEN** a run of a lease passes `--fetch notes.local`, and `notes.local` links to the notes of that lease
- **THEN** the files of `notes.local` of the slot go into the notes of the lease
  [@test] crates/host-client/src/fetch_test.rs (`a_fetch_writes_through_notes_local_into_the_notes_of_a_lease_of_its_repository`)

#### Scenario: A file that comes
- **WHEN** a file of the fetch comes
- **THEN** it goes to `<name>.bd-host-part` first, and takes its name only at the size that the agent named
