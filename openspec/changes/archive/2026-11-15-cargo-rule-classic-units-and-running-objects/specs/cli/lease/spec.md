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
- **THEN** a unit is the hash of `<name>-<16 hex digits>`, its group the package that its dir in `.fingerprint/` names
  (`-` and `_` the same) and the target of that dir; without that dir, `<name>` without the `lib` of a library file
- **AND** its files are all of that hash in `deps/` (`lib<name>-<hash>.rlib` and `.rmeta`, `<name>-<hash>.d`, the
  object files, the test or bin executable and its `.dSYM`, `.pdb` or `.exe`), `.fingerprint/` and `build/` (a build
  script and its run, `out/`), built at the newest mtime of its files
  [@test] crates/lease-pool/src/lease_clean_test.rs (`the_cargo_rule_prunes_the_classic_layout_by_units_of_name_and_hash`, `a_classic_unit_is_its_name_and_its_hash`)

#### Scenario: A bin named apart from its package
- **GIVEN** the classic layout with a bin `pbrd` of the package `probe-app`: `deps/pbrd-<hash>` beside
  `.fingerprint/probe-app-<hash>/`, two builds of the bin and one of its test harness
- **WHEN** a cargo rule with `keep` 1 prunes the profile dir
- **THEN** the old build of the bin goes whole, and the files of the test harness stay beside its fingerprint dir
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_bin_named_apart_from_its_package_is_one_unit_with_its_fingerprint`)

#### Scenario: Incremental units
- **WHEN** the profile dir has `incremental/<crate>-<hash>/`, in either layout
- **THEN** it is a unit of its own, grouped by crate

#### Scenario: The uplifted artifacts
- **WHEN** the profile dir has uplifted artifacts at its top (`debug/libavl_vm.rlib`, `debug/<bin>`) and `.cargo-lock`
- **THEN** they are never touched

### Requirement: Clean rules: A cargo rule deletes the stale objects of rustc in its kept units

After the release of the locks of cargo, the cargo rule MUST delete in a kept unit each incremental object
(`*.rcgu.o`) older than `age` and than the newest rlib, rmeta or executable of the unit, whose inode no session holds:
in a unit that links only an rlib, each one; in another, each one that no name of the invocation of its newest object
holds. Other objects MUST stay. They go in place, never through the trash, when the caller empties its trash, also in a leased slot.

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

#### Scenario: An object of a compile under way
- **GIVEN** a kept unit that links only an rlib, with an object older than the rlib and two objects newer than it but
  older than `age`, and a unit with objects of two invocations and no output yet
- **WHEN** the cargo rule prunes after the release of the locks of cargo
- **THEN** only the object older than the rlib goes
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_object_newer_than_the_outputs_of_its_unit_stays_whatever_its_age`)

#### Scenario: Stale objects through a clean with the trash
- **WHEN** a clean with the trash finds stale objects
- **THEN** they stay in place until the trash is deleted, then go in place with their bytes counted
- **AND** a stop of the delete leaves the rest in place for the next clean
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_clean_with_the_trash_deletes_the_stale_objects_in_place_once_it_empties`)

### Requirement: Get: The answer names the slots the shrink cleaned

The answer of a get that shrank MUST name each slot it cleaned with what its rules freed (`shrunk`), and a
`low_disk_space` after it MUST name how many of these slots freed bytes and the bytes they freed.

#### Scenario: Still below the minimum after the shrink and the eviction
- **WHEN** a get is still below the minimum after the shrink and the eviction
- **THEN** its `low_disk_space` names how many slots it cleaned and the bytes they freed
  [@test] crates/bd-lease/src/lease_test.rs (`a_get_still_below_the_minimum_after_the_shrink_and_the_eviction_refuses_and_names_what_it_freed`)

#### Scenario: A leased slot that freed nothing
- **GIVEN** a leased slot with no stale unit and two free slots with their build output
- **WHEN** a get is still below the minimum after the shrink, which pruned all three
- **THEN** its `low_disk_space` counts the two free slots only
  [@test] crates/bd-lease/src/lease_test.rs (`the_refusal_counts_only_the_shrunk_slots_that_freed_bytes`)
