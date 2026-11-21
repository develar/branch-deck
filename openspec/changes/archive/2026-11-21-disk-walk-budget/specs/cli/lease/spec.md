## MODIFIED Requirements

### Requirement: Disk: Every byte counted is what a delete frees

Every byte counted MUST be what a delete frees: on macOS the private size of a file (`ATTR_CMNEXT_PRIVATESIZE` of
`getattrlistbulk`, the blocks no APFS clone shares, one call per batch of entries), elsewhere its blocks (Linux) or its
length (Windows). A file of `n` hard links counts `1/n` of its bytes at each link, so the bytes of the parts of a tree
add up whatever else is walked; a link counts as itself, a directory nothing of its own; the walks of the call run side
by side (`fs_walk::walk_bytes`, 8 threads over one queue of directories taken from the earliest walk).

#### Scenario: A clone
- **WHEN** a file of a slot is an APFS clone
- **THEN** it counts only the blocks it shares with no other file
  [@test] crates/pool-disk/src/walk_test.rs (`a_clone_counts_only_the_blocks_it_shares_with_no_other_file`)

#### Scenario: Roots side by side and hard links
- **WHEN** the walks of a call run
- **THEN** each root is counted side by side, what counts apart is skipped, and a file of two links counts half at each,
  once in a root that holds both
  [@test] crates/fs-walk/src/walk_test.rs (`a_walk_counts_each_root_side_by_side_skips_what_counts_apart_and_a_hard_link_its_share`, `the_parts_of_a_tree_are_the_subtrees_of_at_most_the_node_entries_and_the_directories_above_them`)

### Requirement: Disk: The bytes of a slot are kept and refreshed by what changed

The bytes of a slot MUST be kept in `$GIT_COMMON_DIR/branch-deck/disk/`, a file a slot, with its `returned`, the lease
that a reclaim or a takeover took from it, whether it was leased, its mark of the history and the parts of its roots;
`pool.json` there keeps the output bases of the main worktree, the orphans and the trash the same way. On macOS the
default refresh MUST ask the history what changed since that mark: an unchanged slot stays cached regardless of its
age or lease; a changed path walks its deepest saved part. A slot that left the pool leaves `disk/`. The `disk.json`
of brd before 1.20, which that brd still writes, MUST stay: a slot that `disk/` has not saved shows its bytes from it,
with their age, until its first walk.

#### Scenario: A changed file
- **WHEN** the history names a changed file of a slot
- **THEN** the refresh walks its deepest saved part, and a path no saved part holds selects its root
- **AND** the report streams the cached overview, then each finished slot
  [@test] crates/pool-disk/src/disk_test.rs (`disk_usage_streams_the_saved_report_then_each_finished_slot`, `a_changed_path_selects_the_deepest_saved_part_of_the_deepest_root`)

#### Scenario: A full walk
- **WHEN** the history lost events, the mark is none or of another volume, nothing was saved, or `--fresh` is given
- **THEN** the full slot is walked; a new root alone is walked whole

#### Scenario: Linux, Windows and the output bases of the main worktree
- **WHEN** the report runs on Linux or Windows
- **THEN** a slot keeps its returned/leased cache policy, with 15 minutes for a leased slot, and the output bases of the
  main worktree and the orphans hold for 15 minutes
- **AND** on macOS the history checks the output bases of the main worktree, the orphans and the trash as it checks a
  slot

#### Scenario: The saved slots for a shrink
- **WHEN** a shrink orders the leased slots by the bytes of their build dirs
- **THEN** it reads them from `disk/` without a lock or a walk
  [@test] crates/pool-disk/src/disk_test.rs (`cached_build_bytes_are_the_build_roots_of_the_saved_slots`)

#### Scenario: The bytes of brd before 1.20
- **WHEN** a slot that `disk/` has not saved has bytes in the `disk.json` of brd before 1.20
- **THEN** it shows them with their age until its first walk, which they cannot spare, and the file stays
  [@test] crates/pool-disk/src/disk_test.rs (`the_bytes_of_brd_before_show_until_the_first_walk_of_a_slot`)

### Requirement: Disk: Cached reads the saved sizes without a walk

`--cached` MUST read the saved slot, trash and orphan sizes without a walk and conflict with `--fresh`; normal output
MUST name where the bytes of each slot come from (`walk`), their measurement age and the elapsed milliseconds. The
report writes nothing else, and counts as a read (`brd help`).

#### Scenario: Cached and fresh together
- **WHEN** `brd lease disk --cached --fresh` runs
- **THEN** it is refused, as the flags are mutually exclusive
  [@test] crates/bd-lease/src/disk_command.rs (`cached_and_fresh_disk_flags_are_mutually_exclusive`)

#### Scenario: The lines of the report
- **WHEN** the report prints a slot
- **THEN** its line names the cache source, the measurement age and the elapsed time
  [@test] crates/pool-disk/src/report_test.rs (`lines_name_the_slot_cache_source_age_and_elapsed_time`)

### Requirement: Disk: The report tells each step on stderr

`brd lease disk` MUST print a line on stderr when the report from `disk/` is ready, with the count of slots to check or
measure. On macOS, when a slot waits for the check of the history, it MUST print a line after the check with how long
the replay took and how many slots keep their bytes as it came late. It MUST then print a line for each walked slot,
with its bytes, its count and why it was walked (`2 changed part(s) again`, `in full: nothing saved`), and a last line
that names the slots the walk budget or another report left saved; stdout MUST stay one JSON object.

#### Scenario: The lines of a walk
- **WHEN** `brd lease disk --fresh` walks the three slots of a pool
- **THEN** the steps are the start with three slots to check or measure, one walked step for each slot, counted 1 to
  3, then the end
- **AND** a run with `--cached` tells only the start, with no slot to check or measure, and the end
  [@test] crates/bd-lease/src/trim_test.rs (`the_disk_report_tells_each_step_on_stderr`)

#### Scenario: The count after the FSEvents check
- **WHEN** the check of the history finds one of two measured slots changed
- **THEN** the check tells one slot to walk, and the walked step of that slot counts 1 of 1
  [@test] crates/pool-disk/src/disk_test.rs (`a_slot_event_counts_the_slots_that_the_history_check_left`)
