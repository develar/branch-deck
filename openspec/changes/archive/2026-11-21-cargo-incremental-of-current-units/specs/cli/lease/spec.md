## MODIFIED Requirements

### Requirement: Clean rules: A cargo rule prunes the stale units of its profile dirs

`{ cargo = "<dir>", keep = 8, age = "1h" }` MUST prune the profile dirs under `<dir>` (a dir with `.cargo-lock`, at
most 4 levels down, also of a cross target, `<triple>/debug`): a unit stays when it is among the `keep` newest of its
group (its package and the target its fingerprint names) or younger than `age` (`s`, `m`, `h`, `d`); an incremental
dir stays when it is the newest of its crate, younger than `age`, or while a current unit of its crate (the last built
of its group, or one younger than `age`) wrote its session; the files of a unit go together. Each profile dir MUST be pruned in
the layout it has.

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
- **WHEN** the profile dir has `incremental/<crate>-<id>/`, in either layout
- **THEN** it is a unit of its own, grouped by crate, and only the newest dir of the crate and the young ones stay by
  themselves
- **AND** it also stays while a current unit of its crate wrote its newest session: the lock of the session
  (`s-<id>.lock`) is not older than `invoked.timestamp` of the unit and not newer than the newest file of its
  fingerprint dir
- **AND** an older unit that `keep` holds keeps no dir
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_incremental_dir_stays_while_the_current_unit_that_wrote_it_stays`, `a_target_names_the_crate_of_its_incremental_dir`, `the_cargo_rule_keeps_the_newest_and_the_young_units`)

#### Scenario: The uplifted artifacts
- **WHEN** the profile dir has uplifted artifacts at its top (`debug/libavl_vm.rlib`, `debug/<bin>`) and `.cargo-lock`
- **THEN** they are never touched
