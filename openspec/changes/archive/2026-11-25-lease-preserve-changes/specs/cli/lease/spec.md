## ADDED Requirements

### Requirement: Get: The quiet local changes of an idle lease are preserved in its notes

A get and `brd lease reclaim` MUST preserve the local changes of an idle lease as patches in its notes and take its
slot when the changes are its only work (every commit landed, no git operation, no resolve session) and quiet. A get
MUST give the clones lock up before the preserve and keep its place in the queue. Each answer that names the lease MUST
name the patches and when the notes expire (`preserved`), and so MUST the message of `lease_reclaimed`.

#### Scenario: Quiet local changes are the only work
- **GIVEN** an idle lease whose commits landed, with a staged, an unstaged and an untracked change, a link and a
  deletion, all quiet
- **WHEN** a get runs
- **THEN** it takes the lease over, and `took_over.preserved` names the directory, the base commit, the number of the
  changes and `staged`
- **AND** the slot is clean at the HEAD of the source, and the record of the slot and of the notes names the patches
- **AND** `git apply` of `changes.patch` on its base gives the files back, and `git apply --cached` of `staged.patch`
  the version the index held
  [@test] crates/bd-lease/src/preserve_test.rs (`a_get_preserves_the_quiet_local_changes_of_an_idle_lease_and_takes_its_slot`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_preserves_the_quiet_changes_of_an_idle_lease`)

#### Scenario: The known limits
- **GIVEN** an idle lease with quiet local changes
- **THEN** the ignored files are no change and go with the clean rules, as for a return
- **AND** a slot with a `reclaimed` record, whose resumed session never becomes the holder, is preserved too: the quiet
  time without a change, no process in the slot and the patches in the notes stand for that session

### Requirement: Get: Local changes are quiet once no changed path moved for the quiet time

The local changes of an idle lease MUST be quiet once no changed path moved for `branchdeck.leaseIdleChangesAfter` (git
config, `12h` by default, `0` never, `0s` at once): the mtime of each path, a link not followed, of a deleted one the
nearest directory above it that exists, each untracked file listed by itself (`--untracked-files=all`). A lease whose
changes are not quiet MUST name when they are (`quiet_at`).

#### Scenario: Fresh changes, a commit beside them, the preserve off
- **WHEN** a changed path was touched within the quiet time, the lease has a commit its home branch lacks, or the
  key is `0`
- **THEN** the lease stays `idle_with_work`; fresh changes name when they are quiet (`quiet_at`)
  [@test] crates/bd-lease/src/preserve_test.rs (`the_local_changes_of_an_idle_lease_stay_while_fresh_beside_a_commit_or_with_the_preserve_off`)
  [@test] crates/lease-pool/src/idle_changes_test.rs (`the_quiet_time_and_the_bound_of_a_preserve_come_from_the_config`, `the_latest_change_of_a_deleted_path_is_the_mtime_of_the_nearest_directory_above_it`)

### Requirement: Get: Local changes that a patch can't carry stay in the slot

Quiet local changes MUST stay in the slot, the lease `idle_with_work` with why (`not_preserved`), for an unmerged entry,
a gitlink or a nested repository, a path with a `filter` attribute (Git LFS), an entry of the index with
assume-unchanged or skip-worktree (`ls-files -v`), a lease without notes, or patches past
`branchdeck.leasePreserveMax` (`64M` by default).

#### Scenario: Changes that a patch can't carry
- **WHEN** the quiet changes hold an unmerged entry, a staged gitlink, a nested repository, a path with a filter or an
  entry with skip-worktree, or the lease has no notes
- **THEN** the lease stays `idle_with_work` with `not_preserved`, and the slot keeps its changes
  [@test] crates/bd-lease/src/preserve_test.rs (`quiet_local_changes_that_a_patch_cannot_carry_keep_the_lease`)

### Requirement: Get: The patches of local changes carry the files and the index

The patches MUST be `changes.patch`, the tree of HEAD to the tree of `add --all` (`core.safecrlf=true`) on a copy of the
index of the slot, and `staged.patch`, the tree of HEAD to the tree of that index where it is neither, in `preserved/`
of the notes of the lease, found by its id. Their proof MUST be an `apply --cached` of `changes.patch` on a new index of
HEAD that gives the tree of `add --all`. Every git of a private index MUST run with `core.splitIndex=false`.

#### Scenario: A split index and a staged version
- **GIVEN** a slot with a split index, a file whose index holds another version than the file, and an untracked file
- **WHEN** its changes are preserved
- **THEN** `changes.patch` carries the file and the untracked file, `staged.patch` the version of the index, and the
  index of the slot and its git dir stay as they were
- **AND** patches past the bound write no file
  [@test] crates/git-ops/src/lease_slot_test.rs (`the_patches_of_a_slot_carry_its_files_and_its_index_and_leave_its_split_index_as_it_was`)

### Requirement: Get: A preserve stops at a change and marks the slot dirty before the discard

A preserve MUST only read the slot until its commit point, and stop when a `status` after the patches finds a path, its
stat or the slot moved since the stat before them. The record of the notes (`preserved`, `preserved_at`) MUST come
before the commit point, a compare-and-swap under the pool lock that names the patches in the record of the slot and
marks the slot dirty. The discard MUST come after it and after the close of the notes, and the lease MUST never come
back.

#### Scenario: An edit while the patches are written
- **WHEN** a changed file is edited between the first `status` and the second
- **THEN** the get keeps the lease and adds a slot, the reclaim names it `failed`, and the files stay
  [@test] crates/bd-lease/src/preserve_test.rs (`an_edit_while_the_changes_are_preserved_keeps_the_lease_and_its_files`)

#### Scenario: A discard that fails after the commit point
- **WHEN** the discard of a get or of a reclaim fails, as a process killed there leaves it
- **THEN** the slot is free and dirty, its record and the notes name the patches, the files stay
- **AND** the next get passes over the slot as dirty without a `status`
  [@test] crates/bd-lease/src/preserve_test.rs (`a_discard_that_fails_after_the_commit_point_leaves_a_dirty_slot_and_the_patches`)

## MODIFIED Requirements

### Requirement: Get: A get takes over only a lease whose work is on its home branch

A get MUST take the lease over only when every commit of the lease that its home branch lacks (`HEAD
^refs/heads/<home> ^<leased commit>`, whatever other branch, its own `-b` one too, holds it) is proven on that branch
by the landed rule of the return ([Return](#return)), the slot has no local changes but quiet ones that the get
preserved in the notes of the lease (Get: The quiet local changes of an idle lease are preserved in its notes), no git
operation in progress and no open resolve session. A lease without a home branch MUST be taken over only without
commits of its own.

#### Scenario: Commits landed, one with a conflict resolution
- **WHEN** every commit the home branch lacks is proven on it by the landed rule of the return
- **THEN** the get takes the lease over
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_takes_over_an_idle_lease_whose_commits_landed`, `a_get_takes_over_an_idle_lease_whose_commit_landed_with_a_conflict_resolution`)

#### Scenario: An idle lease on its own branch
- **GIVEN** the commits of the lease are held by its own `-b` branch
- **WHEN** a get checks it
- **THEN** it takes it over only when home has its commits
  [@test] crates/bd-lease/src/takeover_test.rs (`an_idle_lease_on_its_own_branch_is_taken_over_only_when_home_has_its_commits`)

#### Scenario: A git operation in progress
- **WHEN** the git dir of the slot holds `rebase-merge`, `rebase-apply`, `MERGE_HEAD`, `CHERRY_PICK_HEAD`,
  `REVERT_HEAD`, `sequencer` or `BISECT_LOG`, or the slot has local changes that are not preserved or an open resolve
  session
- **THEN** the get does not take the lease over

#### Scenario: A lease without a home branch
- **GIVEN** a lease without a home branch
- **WHEN** a get checks it
- **THEN** it takes it over only without commits of its own

### Requirement: Get: An idle lease with work its home branch lacks is never taken over

An idle lease with work its home branch lacks MUST NOT be taken over: commits, a git operation, or local changes that
are not quiet or that a patch can't carry. The get skips it (`skipped`, `reason: "idle_with_work"`, `idle` with its
`commits`, `paths` or `operation`, and for local changes `quiet_at` or `not_preserved`), records the check in the lease
(`idle_check`) and adds a slot, and `brd lease status` shows it as `idle_with_work`.

#### Scenario: An idle lease with unlanded work
- **GIVEN** an idle lease with commits its home branch lacks, fresh local changes or a git operation in progress
- **WHEN** a get runs
- **THEN** it skips the lease, records `idle_check` and adds a slot
- **AND** `brd lease status` shows it as `idle_with_work`
  [@test] crates/bd-lease/src/takeover_test.rs (`an_idle_lease_with_unlanded_work_is_kept_and_named`)

### Requirement: Get: A kept idle lease is checked again only when its verdict moved

A later get MUST check a kept idle lease again only when what the verdict rests on moved: the slot (its `HEAD`, the
length and the mtime of its reflog), for commits the home tip too, for local changes the stat of their first 8 paths
(as the proof of a dirty slot) and their `quiet_at`; else it costs one `rev-parse`. One get MUST check at most two idle
leases in full for its takeover and two for its reclaim below the minimum, the ones checked longest ago first; the
keeper checks every one.

#### Scenario: Neither the slot nor home moved
- **WHEN** a later get finds the slot and the home tip as the check found them
- **THEN** it keeps the verdict for one `rev-parse`
  [@test] crates/bd-lease/src/takeover_test.rs (`a_kept_idle_lease_is_checked_again_only_when_its_slot_or_home_moved`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_that_passes_over_an_idle_lease_it_checked`)

#### Scenario: Local changes restored by hand
- **GIVEN** an idle lease whose fresh local changes a get found
- **WHEN** they are restored by hand, which moves the stat of their paths but not the slot, and a get runs
- **THEN** it checks the lease again and takes it over
  [@test] crates/bd-lease/src/preserve_test.rs (`local_changes_restored_by_hand_are_checked_again_and_the_slot_is_taken_over`)

#### Scenario: Many idle leases
- **WHEN** a get finds more than two idle leases to check
- **THEN** it checks at most two of them in full

#### Scenario: The reclaim below the minimum
- **GIVEN** an idle lease whose work a get before found
- **WHEN** a later get below the minimum free space reclaims, and neither the slot nor home moved
- **THEN** it keeps the verdict for one `rev-parse`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_get_below_the_minimum_passes_over_an_idle_lease_it_checked`)

#### Scenario: Many idle leases below the minimum
- **WHEN** a get below the minimum free space finds more than two idle leases whose verdict moved
- **THEN** it checks two of them in full, the ones checked longest ago first, and a later get checks the rest
  [@test] crates/bd-lease/src/takeover_test.rs (`a_get_below_the_minimum_checks_two_idle_leases_in_full_and_the_next_get_the_rest`)

### Requirement: Reclaim: Reclaim applies the rule of the takeover to every idle lease

`brd lease reclaim --idle [--dry-run] [--keep <n>] [--age <duration>]` MUST apply the rule of the takeover to every idle
lease of the pool, each checked anew: one whose work landed is returned as the takeover returns it, its quiet local
changes preserved first, under a CAS on its lease, its holder still gone and its slot as the check found it, its slot
free with its record, returned as the coldest free slot of its pool. The output is `lease_reclaim` of `brd schema`,
exit 0.

#### Scenario: An idle lease whose work landed
- **WHEN** reclaim finds an idle lease whose work landed
- **THEN** it returns it as the takeover returns it (`reclaimed`, each with `landed`, `stopped_servers`, `cleaned`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_lease_reclaim`)

#### Scenario: An idle lease whose only work is quiet local changes
- **WHEN** reclaim finds an idle lease whose only work is quiet local changes, with `--dry-run` or not
- **THEN** it preserves them and returns the lease (`reclaimed` with `preserved`), the slot free and clean
- **AND** a dry run names the directory and the number of the changes, writes nothing and no patch
  [@test] crates/bd-lease/src/preserve_test.rs (`reclaim_preserves_the_quiet_local_changes_of_an_idle_lease_and_a_dry_run_names_them`)

#### Scenario: An idle lease with work
- **WHEN** reclaim finds an idle lease with work
- **THEN** it keeps the lease and records its check (`kept`, as `idle_with_work`)
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_returns_the_idle_leases_whose_work_landed_and_keeps_the_rest`)

#### Scenario: The other free slots after a reclaim
- **GIVEN** free slots returned after an idle lease went quiet, measured by `brd lease disk`
- **WHEN** reclaim returns the idle lease
- **THEN** each of them keeps its `returned` and its epoch
- **AND** the next `brd lease disk` takes their bytes from its cache and measures only the reclaimed slot
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaim_leaves_the_measures_of_the_other_free_slots_valid`)

#### Scenario: A lease it cannot judge
- **WHEN** a lease is not quiet for an hour, busy, changed meanwhile or its check failed
- **THEN** it is `skipped` with its reason

#### Scenario: A lease whose holder is unknown
- **GIVEN** a `ready` lease whose get recorded no holder process
- **WHEN** reclaim runs, with `--dry-run` or not
- **THEN** it is `skipped` as `holder_unknown`, checked by nothing, and stays `ready`
  [@test] crates/bd-lease/src/takeover_test.rs (`reclaim_names_a_lease_without_a_holder_as_holder_unknown`)

#### Scenario: A dry run
- **WHEN** reclaim runs with `--dry-run`
- **THEN** it checks the same and writes nothing

#### Scenario: The return time of a reclaimed slot
- **GIVEN** free slots of the pool, some returned before an idle lease went quiet and some after
- **WHEN** reclaim returns the idle lease
- **THEN** its `returned` is below that of every other free slot of the pool and is not its number before the lease
- **AND** `last_returned` keeps the highest number of the pool
- **AND** its `returned_at` is the earliest of the time its lease went quiet (the later of its get and the last change
  of the reflog of the slot) and the times of the other free slots, none when one of them has none
- **AND** a brd that orders by the number alone and a brd that orders by the time first both take it last for a get
  and first for the trim and the shrink
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaimed_slot_is_the_coldest_free_slot_of_its_pool_by_number_and_by_time`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_below_the_minimum_reclaims_an_idle_lease_and_shrinks_its_slot_before_the_hot_one`)

#### Scenario: A number of an earlier free period
- **GIVEN** a reclaim gave its slot a number that a measure of `brd lease disk` of an earlier free period has
- **WHEN** `brd lease disk` runs
- **THEN** it measures the reclaimed slot again, as its measure names the reclaimed lease
- **AND** the other free slots keep their measures
  [@test] crates/pool-disk/src/disk_test.rs (`a_reclaimed_slot_with_the_number_of_an_earlier_free_period_is_measured_again`)

#### Scenario: No number left below the other free slots
- **GIVEN** a pool whose lowest free slot has the number 0, or 1 while the reclaimed slot had 0
- **WHEN** reclaim returns an idle lease
- **THEN** the other free slots move up by `RECLAIM_ROOM`, and the reclaimed slot takes the number below them
- **AND** a leased slot keeps its number
  [@test] crates/lease-pool/src/lease_test.rs (`a_reclaim_with_no_number_left_below_moves_the_free_slots_up_once`)

### Requirement: Notes: The notes of a returned lease expire

The notes of a returned lease MUST expire `branchdeck.leaseNotesKeep` after the return (`14d` by default, `0` for ever;
without a return from its get), and not before that time after a preserve of its changes (`preserved_at`), deleted by
`brd lease trim` (`expired_notes`; `--plan` only names them) and the keeper of a host in every place of the notes,
never while the pool holds the lease, where its state can't be read, nor anything else of the pool directory. A
directory without a record MUST expire by its modification time. `brd lease disk` MUST count them.

#### Scenario: The trim deletes expired notes
- **GIVEN** the notes of a lease returned 15 days ago and of a lease held for 15 days
- **WHEN** `brd lease trim` runs, after a `--plan`, and with `branchdeck.leaseNotesKeep` `0` or `30d` before
- **THEN** only the trim with the default deletes the notes of the returned lease; the plan names them, and the held
  lease keeps its notes
  [@test] crates/bd-lease/src/notes_test.rs (`the_notes_of_a_returned_lease_expire_through_the_trim_and_never_while_held`)
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_expire_after_the_return_and_never_while_the_lease_is_held`)

#### Scenario: The notes of preserved changes
- **GIVEN** the notes of a lease whose changes were preserved, returned or not
- **THEN** they expire `branchdeck.leaseNotesKeep` after the later of the return and the preserve
  [@test] crates/lease-pool/src/lease_notes_test.rs (`the_notes_of_preserved_changes_expire_after_the_preserve_too`)

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
