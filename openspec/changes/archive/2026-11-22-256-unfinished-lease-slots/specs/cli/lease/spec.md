## ADDED Requirements

### Requirement: Get: A slot whose add never finished is removed by the next get that adds a slot

A free slot whose add never finished (no name, which only the end of a get writes, never returned, and no lease or one
whose process is gone) holds no work: a get that failed or was killed after it registered the slot left it, and its
tree can be half written. No get MUST reserve it as free or run a `status` in it, no wait for the clones lock MUST end
for it, and no get MUST clone it as a template. A get that holds the clones lock MUST remove each such slot of its pool
before it adds one, as the trim removes a free slot (its tree into the trash, its registration and its entry dropped,
the lock of a killed clone too), and name them (`removed_unfinished`); one with a process in it MUST stay, skipped as
`busy`. Neither the shrink nor the eviction MUST keep it as the slot the next get takes. An adopt MUST record the name
of a slot in its first write, so an adopt killed during its move leaves no such slot; one killed by a brd before it
does, and the get removes that worktree too.

#### Scenario: An unfinished slot beside a clean free slot
- **GIVEN** a free slot whose add never finished, returned last, and a clean free slot
- **WHEN** a get runs
- **THEN** it reuses the clean slot and runs nothing in the unfinished one
  [@test] crates/bd-lease/src/unfinished_test.rs (`a_slot_whose_add_never_finished_is_removed_by_the_next_get_that_adds_a_slot`)

#### Scenario: No free slot left
- **WHEN** a get adds a slot and a slot whose add never finished is in the pool
- **THEN** it removes that slot, its directory, its registration and its entry, names it in `removed_unfinished`, and
  adds a slot
  [@test] crates/bd-lease/src/unfinished_test.rs (`a_slot_whose_add_never_finished_is_removed_by_the_next_get_that_adds_a_slot`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_removes_an_unfinished_slot`)

#### Scenario: A process in the unfinished slot
- **WHEN** a process works in the unfinished slot
- **THEN** the slot stays, skipped as `busy`, and the new slot is not cloned from it
  [@test] crates/bd-lease/src/unfinished_test.rs (`an_unfinished_slot_with_a_process_in_it_stays_and_is_no_template`)

#### Scenario: The next slot of the pool
- **WHEN** the shrink or the eviction looks for the slot the next get takes
- **THEN** it passes over a slot whose add never finished
  [@test] crates/lease-pool/src/lease_test.rs (`a_slot_whose_add_never_finished_is_not_the_next_free_slot`)

### Requirement: Get: The state keeps the first 100 local changes of a slot

The verdict of a dirty free slot and the check of an idle lease MUST keep the first 100 of their local changes in the
state, and the count of the rest (`paths_omitted`); a state written before the cut MUST be read cut. A get that skips
a dirty slot and `brd lease status` MUST name the same first paths and the count of the rest. A pool of 30 slots MUST
keep a state under 1 MB with a whole tree dirty in one of them: two half-written slots of idea-1 listed 3.3 million
paths, and every change of the pool read and wrote 380 MB under its lock, longer than a return waited for it.

#### Scenario: A whole tree dirty
- **GIVEN** a pool of 30 slots, a free slot of them dirty and an idle lease with work, 100,000 local changes each
- **WHEN** the state is written
- **THEN** it is under 1 MB, and each keeps the first 100 paths and the count of the rest
  [@test] crates/lease-pool/src/lease_test.rs (`the_state_of_thirty_slots_stays_small_with_a_whole_tree_dirty`)

#### Scenario: A state of an older brd
- **WHEN** a state lists every local change of a slot
- **THEN** it is read with the first 100 and the count of the rest, and the next change of the pool writes it small
  [@test] crates/lease-pool/src/lease_test.rs (`a_state_written_before_the_cut_is_read_cut_and_written_small`)

## MODIFIED Requirements

### Requirement: Get: The files of Git LFS of a new slot are downloaded after the clones lock

Where the smudge of Git LFS downloads, the checkout of a new slot under the clones lock MUST write the pointers of its
files of LFS, so no filter of LFS runs under a lock of the pool, and the get MUST pull their content (`git lfs pull`,
killed after 10 min without progress) after the release of the clones lock, where the repository keeps objects of LFS.
A pull that fails MUST fail the get and leave the slot unfinished, unless the repository skips download errors: then
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
- **THEN** the get fails, deletes the index of the slot and gives it back unfinished, so no get moves it by the diff,
  which would keep the pointers as the content, and the next get that adds a slot removes it
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

### Requirement: Status: The status lists every slot from the state alone

`brd lease status` MUST list every slot from the state alone, with its `path`, `state` (`free`, `preparing`, `ready`,
`templating`, `returning`, `cleaning`, `abandoned`, `unfinished`), `dirty`, `missing`, `lease` and `reclaimed`, the `free_bytes` of
the volume of the pool and the `trash_bytes` a killed run left; it MUST run no `git status`. `--processes` MUST add the
processes in each slot. It writes nothing.

#### Scenario: The status of the pool
- **WHEN** `brd lease status` runs
- **THEN** it lists every slot from the state without a `git status`
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_reads_the_state_without_a_git_status`)

#### Scenario: A slot whose add never finished
- **WHEN** a free slot has no name and was never returned
- **THEN** its `state` is `unfinished`
  [@test] crates/bd-lease/src/unfinished_test.rs (`a_slot_whose_add_never_finished_is_removed_by_the_next_get_that_adds_a_slot`)

#### Scenario: The holder of a lease
- **WHEN** a slot has a lease
- **THEN** `lease` has `holder_pid`, `holder_alive`, `idle` for a ready lease whose holder is gone (`idle`, or
  `idle_with_work` with what a check found), and `taking_over` while a get checks one
  [@test] crates/bd-lease/src/lease_test.rs (`the_status_tells_whether_the_holder_of_a_lease_is_alive`)

#### Scenario: A read
- **WHEN** `brd lease status` runs, with or without `--processes`
- **THEN** it writes no ref, config, index or working tree
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
