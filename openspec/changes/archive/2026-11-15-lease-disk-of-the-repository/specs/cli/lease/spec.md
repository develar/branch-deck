## RENAMED Requirements

- FROM: `### Requirement: Disk: The disk report names the bytes of every slot of the machine`
- TO: `### Requirement: Disk: The disk report names the bytes of every slot of the pool of the repository`

## MODIFIED Requirements

### Requirement: Disk: The disk report names the bytes of every slot of the pool of the repository

`brd lease disk [--all] [--cached] [--fresh]` MUST report the pool of the repository it runs in (`-C`, else the
current directory) and its orphans. With `--all` it MUST report every pool and every orphan of this machine, as the
trim finds them. Without a `du`, per slot it MUST name `state`, `holder`, `returned`, the bytes its builds wrote
(`build`, `output_bases`), their sum (`bytes`), `measured_at` and `cached`.

#### Scenario: The bytes of each slot
- **WHEN** `brd lease disk` runs
- **THEN** each slot names its build directories and output bases with their bytes, and keeps them until the slot is
  leased
  [@test] crates/bd-lease/src/trim_test.rs (`the_disk_report_names_the_bytes_of_each_slot_and_keeps_them_until_the_slot_is_leased`)

#### Scenario: The build directories
- **WHEN** the clean rules of a slot name nested directories
- **THEN** the report names the outermost of them, `build`
  [@test] crates/pool-disk/src/disk_test.rs (`the_build_dirs_of_a_slot_are_the_outermost_dirs_its_clean_rules_name`)

#### Scenario: The pool of the repository and every pool
- **WHEN** `brd lease disk` runs in a repository, and the machine also has the pool of another repository
- **THEN** the report names the pool of the repository and its orphans only
- **AND** `--all` names every pool and every orphan of the machine, and `--all --cached` the latest measure of each
  orphan
  [@test] crates/bd-lease/src/trim_test.rs (`the_disk_report_names_the_pool_of_the_repository_and_with_all_every_pool`)
