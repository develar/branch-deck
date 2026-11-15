## ADDED Requirements

### Requirement: Clean rules: The object pass keeps what a running build reads

A build may run during the object pass of a cargo rule, after the release of the locks of cargo. `age`, measured from
the start of the run, MUST keep each object that it compiles. The sessions of every dir of `incremental/` at the pass,
kept or stale, MUST keep each object that it links again from one, with the old mtime of its inode. A session that
vanishes during the read MUST be read again by a new list. When a read fails at each try, every object of the profile
dir MUST stay.

#### Scenario: A stale incremental dir that a build wrote after the walk
- **GIVEN** a stale incremental dir whose session holds an old object of a kept unit that links only an rlib, and an
  old object that no session holds
- **WHEN** a build writes the dir between the walk and the locks of cargo, and the cargo rule prunes
- **THEN** the dir stays, as built again, and so does the object that its session holds
- **AND** the object that no session holds goes
  [@test] crates/lease-pool/src/lease_clean_test.rs (`an_object_that_a_stale_incremental_dir_built_again_holds_stays`)

#### Scenario: A session that rustc renames during the read
- **WHEN** rustc renames its working session and deletes the old one between the list of the sessions and their read
- **THEN** a new list reads the renamed session, and its inodes are held
- **AND** a session that cannot be read at each try gives no answer, so the objects stay
  [@test] crates/lease-pool/src/lease_clean_test.rs (`a_session_that_rustc_renames_during_the_read_is_read_under_its_new_name`)
