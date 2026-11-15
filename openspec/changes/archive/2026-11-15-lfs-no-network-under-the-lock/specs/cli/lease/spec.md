## MODIFIED Requirements

### Requirement: Get: The files of Git LFS of a new slot are downloaded after the clones lock

Where the smudge of Git LFS downloads, the checkout of a new slot under the clones lock MUST write the pointers of its
files of LFS, so no filter of LFS runs under a lock of the pool, and the get MUST pull their content (`git lfs pull`,
killed after 10 min without progress) after the release of the clones lock, where the repository keeps objects of LFS.
A pull that fails MUST fail the get and leave the slot dirty, unless the repository skips download errors: then
`git lfs checkout` MUST follow.

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
  [@test] crates/git-ops/src/lfs_test.rs (`the_smudge_of_git_lfs_downloads_unless_it_skips`)
  [@test] crates/git-ops/src/lfs_test.rs (`the_objects_of_git_lfs_are_where_lfs_storage_puts_them`)

#### Scenario: A repository that skips download errors
- **GIVEN** `lfs.skipdownloaderrors` is true, so the smudge writes the pointer of an object it can not download
- **WHEN** the `git lfs pull` of a new slot fails
- **THEN** the get logs the failure, `git lfs checkout` writes the content of what the store has, no lock of the pool
  held, the get goes on with the pointers of the rest, and the slot is reused, not dirty
  [@test] crates/bd-lease/src/lfs_test.rs (`a_repository_that_skips_download_errors_keeps_the_pointers_of_a_failed_pull`)
  [@test] crates/git-ops/src/lfs_test.rs (`a_repository_skips_download_errors_by_its_config_or_the_environment`)

#### Scenario: A pull that hangs
- **WHEN** a `git lfs pull` writes no line of progress for 10 minutes
- **THEN** it is killed with the processes it started, and the get fails as for a pull that failed
- **AND** a slow pull that writes its progress runs to its end, however long
  [@test] crates/git-ops/src/lfs_test.rs (`a_hung_lfs_pull_is_killed_once_it_makes_no_progress`)
  [@test] crates/git-ops/src/lfs_test.rs (`a_slow_lfs_pull_that_makes_progress_is_not_killed`)
