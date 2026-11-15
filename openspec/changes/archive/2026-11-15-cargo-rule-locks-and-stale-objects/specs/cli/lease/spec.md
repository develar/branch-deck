## ADDED Requirements

### Requirement: Clean rules: A cargo rule prunes a profile dir only under the locks of cargo

A cargo rule that deletes MUST take `.cargo-build-lock` and `.cargo-lock` of each profile dir exclusive, open to read
and never created or deleted, and under them move each stale unit that no build wrote since its read. A held lock MUST
skip that profile dir with the error `the lock of cargo is held (a build or another clean)`: a return waits for it up
to 30 s first, a takeover and a reclaim do not wait. A dry run takes no lock.

#### Scenario: A build works in a profile dir
- **GIVEN** a process holds `.cargo-build-lock` of one profile dir, or its `.cargo-lock`
- **WHEN** the cargo rule prunes the target dir
- **THEN** that profile dir stays whole and the error names it, and the other profile dir is pruned
- **AND** the lock files stay, free once the rule ends
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_profile_dir_whose_cargo_lock_is_held_stays_whole`)

#### Scenario: A build between the read and the locks
- **GIVEN** a unit that the read found stale
- **WHEN** a build writes it before the rule takes the locks
- **THEN** it stays
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_unit_that_a_build_wrote_after_the_walk_is_built_again`)

#### Scenario: A return meets a held lock
- **GIVEN** another clean holds the locks of cargo of a slot
- **WHEN** the slot is returned
- **THEN** the return waits for the lock, takes it when the holder lets it go, and else frees the slot with that
  profile dir as it is, its `cleaned` naming the held lock
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_cargo_lock_that_is_released_during_the_wait_is_taken`)
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_return_that_meets_a_held_cargo_lock_frees_the_slot_and_names_it`)

#### Scenario: Another volume
- **GIVEN** the trash is on another volume than the profile dir
- **WHEN** the cargo rule prunes it
- **THEN** the stale units go into `.brd-trash/` of the profile dir under the locks, and are deleted after the release
- **AND** a dir of `.brd-trash/` that a gone run left is deleted
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_cargo_rule_on_another_volume_moves_its_units_beside_them`)

### Requirement: Clean rules: A cargo rule deletes the stale objects of rustc in its kept units

In a kept unit the cargo rule MUST delete each object of an incremental compile (`*.rcgu.o`) older than `age` whose
inode no session of `incremental/` holds: in a unit that links only an rlib, each one; in another unit, each one that
no name of the invocation of its newest object holds. An object of a compile without incremental MUST stay. A stale
object MUST be deleted in place once the slot is free, never through the trash.

#### Scenario: A unit that links only an rlib
- **GIVEN** a kept unit with an rlib, old objects of two invocations, one of them held by a session, and a young one
- **WHEN** the cargo rule prunes
- **THEN** the old objects outside the session go, and the held and the young one stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_deletes_the_objects_of_a_unit_that_links_only_an_rlib`)

#### Scenario: An executable unit
- **GIVEN** a kept test unit with objects of an older invocation and of the newest one, one inode named by both
- **WHEN** the cargo rule prunes
- **THEN** the old names go but the one of the shared inode, and the names of the newest invocation stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_keeps_the_objects_of_the_newest_invocation_of_an_executable`)

#### Scenario: Two units of one incremental dir
- **GIVEN** two test units of one crate that share one incremental dir, whose session holds a few objects of the first
  and every object of the second
- **WHEN** the cargo rule prunes
- **THEN** the newest invocation of each unit keeps all its objects
  [@test] crates/lease-pool/src/lease_clean_test.rs (`sibling_units_of_one_incremental_dir_keep_their_newest_objects`)

#### Scenario: The classic layout
- **GIVEN** a kept classic unit with objects `deps/<name>-<hash>.<cgu>.<invocation>.rcgu.o` of two invocations
- **WHEN** the cargo rule prunes
- **THEN** the objects of the old invocation go and the newest stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_deletes_the_stale_objects_of_a_classic_unit`)

#### Scenario: Objects of a compile without incremental
- **GIVEN** a registry build script named `build_script_build` beside `incremental/build_script_build-<id>/`, a
  registry library and a release executable, each with old objects `<crate>-<hash>.<crate>.<hex>-cgu.<n>.rcgu.o`
- **WHEN** the cargo rule prunes
- **THEN** every object of the three stays
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_keeps_the_objects_of_units_with_other_names`)

#### Scenario: Stale objects through a clean with the trash
- **WHEN** a clean with the trash finds stale objects
- **THEN** they stay in place until the trash is deleted, then go in place with their bytes counted
- **AND** a stop of the delete leaves the rest in place for the next clean
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_clean_with_the_trash_deletes_the_stale_objects_in_place_once_it_empties`)

## MODIFIED Requirements

### Requirement: Clean rules: A cargo rule prunes the stale units of its profile dirs

`{ cargo = "<dir>", keep = 8, age = "1h" }` (the defaults) MUST prune the profile dirs under `<dir>` (a dir with
`.cargo-lock`, at most 4 levels down, also of a cross target such as `aarch64-unknown-linux-musl/debug`): a unit stays
when it is among the `keep` newest of its group (its package and the target its fingerprint names) or younger than
`age` (`s`, `m`, `h`, `d`), and the files of a unit go together. Each profile dir MUST be pruned in the layout it has.

#### Scenario: The newest and the young units stay
- **WHEN** a cargo rule prunes a profile dir
- **THEN** the `keep` newest units of each group and the units younger than `age` stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_keeps_the_newest_and_the_young_units`)

#### Scenario: Each target of a package
- **GIVEN** a package with more current targets than `keep`: its library, its check, its test harness and its build
  script
- **WHEN** a cargo rule prunes the profile dir
- **THEN** the `keep` newest units of each target stay, as its fingerprint names it (`lib-bd_cli`, `test-bin-brd`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`each_target_of_a_package_keeps_its_own_newest_units`)

#### Scenario: The build-dir layout
- **WHEN** the profile dir has the build-dir layout
- **THEN** a unit is `build/<package>/<hash>/`, built at the newest mtime of the dir and of the children of its
  `fingerprint` dir

#### Scenario: The classic layout
- **WHEN** the profile dir has the classic layout
- **THEN** a unit is `<name>-<16 hex digits>`, its group `<name>` (`-` and `_` the same, without the `lib` of a library
  file) and the target of its dir in `.fingerprint/`
- **AND** its files are all of that name and hash in `deps/` (`lib<name>-<hash>.rlib` and `.rmeta`, `<name>-<hash>.d`,
  the object files, the test or bin executable and its `.dSYM`, `.pdb` or `.exe`), `.fingerprint/` and `build/` (a
  build script and its run, `out/`), built at the newest mtime of its files
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash`, `a_classic_unit_is_its_name_and_its_hash`)

#### Scenario: Incremental units
- **WHEN** the profile dir has `incremental/<crate>-<hash>/`, in either layout
- **THEN** it is a unit of its own, grouped by crate

#### Scenario: The uplifted artifacts
- **WHEN** the profile dir has uplifted artifacts at its top (`debug/libavl_vm.rlib`, `debug/<bin>`) and `.cargo-lock`
- **THEN** they are never touched

### Requirement: Clean rules: What a rule removes goes into the trash first

What a rule removes MUST go into the trash first, a rename into `$GIT_COMMON_DIR/branch-deck/trash/<pid>-<start
time>-<uuid>/` (on another volume a `delete` in place, a cargo rule into `.brd-trash/` of the profile dir): the paths
of a `delete` but a link, and a stale unit of a `cargo` rule as a whole, so a return frees the slot before the long
delete of a build output and a timeout of the caller never leaves a lease `returning` while files go.

#### Scenario: A return through the trash
- **WHEN** a return applies its rules
- **THEN** what they remove goes into the trash, and is deleted once the slot is free
  [@test] crates/bd-lease/src/lease_clean_test.rs (`a_return_deletes_through_the_trash_and_what_a_gone_run_left_there`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_delete_keeps_its_kept_entries_removes_a_link_and_goes_through_the_trash`)

### Requirement: Clean rules: Sizes are measured by the shared walk

`CleanMode::Measure` and `Pool::trash_bytes` MUST use `fs_walk::tree_size`, the shared filesystem walk rather than a
second recursive size implementation; its result includes directory blocks and counts links as themselves. A dry run
MUST count the stale objects by their inodes: the blocks of an inode whose every link is a stale object, once.

#### Scenario: The size of a tree
- **WHEN** a dry run measures a rule, or the pool measures its trash
- **THEN** the size comes from the walk, with directory blocks, each run of the trash measured
  [@test] crates/fs-walk/src/walk_test.rs (`tree_size_uses_the_walk_and_counts_directory_bytes`)
  [@test] crates/lease-pool/src/lease_clean_test.rs (`trash_bytes_measure_each_run_with_the_shared_walk`, `the_cargo_rule_keeps_the_newest_and_the_young_units`)

#### Scenario: The size of stale objects
- **GIVEN** stale objects, two of them names of one inode
- **WHEN** a dry run measures the cargo rule
- **THEN** it counts what the delete then frees, the shared inode once
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_deletes_the_objects_of_a_unit_that_links_only_an_rlib`)

### Requirement: Host slots: The clean rules of a host slot run when they are due

After a give-back, the clean rules of the slot's own `.branchdeck.toml` MUST run when they are due: with
`branchdeck.hostSlotMaxSize`, when what they would free passes it (the stale objects of the kept units count), or when
the volume has less free space than `branchdeck.leaseMinFree` plus it; without it, after 50 runs or 7 days since the
last clean, or when the volume has less than `branchdeck.leaseMinFree`.

#### Scenario: By size, by free space and after fifty runs
- **WHEN** a give-back ends and the rules are due by size, by free space or by the number of runs
- **THEN** the clean rules of the slot run
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_host_slot_is_cleaned_by_size_by_free_space_and_after_fifty_runs`)

#### Scenario: By the stale objects
- **GIVEN** an idle host slot whose stale objects alone pass `branchdeck.hostSlotMaxSize`, and no stale unit
- **WHEN** a give-back ends
- **THEN** its clean rules run and the objects go
  [@test] crates/bd-lease/src/host_slots_test.rs (`a_host_slot_is_due_by_the_stale_objects_of_its_kept_units`)
