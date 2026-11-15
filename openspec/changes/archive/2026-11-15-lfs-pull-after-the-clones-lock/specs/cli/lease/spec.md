## ADDED Requirements

### Requirement: Get: The files of Git LFS of a new slot are downloaded after the clones lock

Where the smudge filter of Git LFS downloads, the checkout of a new slot under the clones lock MUST write the pointers
of the files of LFS, so that no filter of LFS runs under a lock of the pool, and the get MUST pull their content
(`git lfs pull`) after it released the clones lock, where the repository keeps objects of LFS (`lfs/objects` of
`$GIT_COMMON_DIR`, or of `lfs.storage`). A pull that fails MUST fail the get and leave the slot dirty.

#### Scenario: A new slot in a repository of Git LFS
- **GIVEN** a repository whose smudge filter of LFS downloads and that keeps objects of LFS
- **WHEN** a get adds a slot
- **THEN** no smudge filter runs while the clones lock, the lock of the pool's `worktree` commands or the pool lock is
  held, and `git lfs pull` runs once, after the release of the clones lock, before the seed
- **AND** a get that reuses the slot runs the smudge filter in its move, without a lock of the pool, and no pull
  [@test] crates/bd-lease/src/lfs_test.rs (`a_new_slot_gets_the_content_of_its_lfs_files_after_the_clones_lock`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_of_a_new_slot_with_files_of_git_lfs`)

#### Scenario: A pull that fails
- **WHEN** the `git lfs pull` of a new slot fails
- **THEN** the get fails and deletes the index of the slot, so the next get skips the slot as dirty, as after a checkout
  that failed, and adds another: a move by the diff would keep the pointers as the content
  [@test] crates/bd-lease/src/lfs_test.rs (`a_new_slot_whose_lfs_pull_failed_is_never_reused`)

#### Scenario: A repository without objects of Git LFS
- **GIVEN** the filter of LFS is configured for every repository (the system config of Git for Windows), and the
  repository keeps no objects of LFS
- **WHEN** a get adds a slot
- **THEN** its checkout runs no filter of LFS and no pull follows
  [@test] crates/bd-lease/src/lfs_test.rs (`a_repository_without_objects_of_git_lfs_pulls_nothing`)

#### Scenario: A filter that skips
- **WHEN** neither `filter.lfs.process` nor, without it, `filter.lfs.smudge` names a command, the command has
  `--skip`, or git's environment has a `GIT_LFS_SKIP_SMUDGE` that is true
- **THEN** the smudge of LFS does not download, and the checkout of a new slot runs the filter as git does
  [@test] crates/git-ops/src/lease_slot_test.rs (`the_smudge_of_git_lfs_downloads_unless_it_skips`)
  [@test] crates/git-ops/src/lease_slot_test.rs (`the_objects_of_git_lfs_are_where_lfs_storage_puts_them`)

## MODIFIED Requirements

### Requirement: Get: A get prints each step on stderr as it starts

A get MUST print each step on stderr as it starts (`brd lease get: <step>`): the reserve of a free slot, under the
clones lock the reserve of a slot freed meanwhile or a new one, the wait for the clones lock, the stop of the Bazel
servers, the `status` of a slot, the wait for a slot whose return or clean runs, the delete of the trash, the clone of
a template, the refresh of its index, the checkout, the download of the files of Git LFS and the seed; the JSON goes to
stdout at the end.

#### Scenario: The steps of a get
- **WHEN** a get runs
- **THEN** its steps arrive on stderr in order and the JSON on stdout at the end
- **AND** the core tells the same steps to its caller (`LeaseGetParams::progress`)
  [@test] crates/bd-lease/src/clones_lock_test.rs (`the_steps_of_a_get_arrive_in_order`)
