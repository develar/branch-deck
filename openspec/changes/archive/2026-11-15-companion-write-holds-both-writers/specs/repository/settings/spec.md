## MODIFIED Requirements

### Requirement: Scope: One writer of the git config of a repository

The app MUST write each git config file through its one writer (`config_writer`): the writes of Settings and of the
companions of a repository, or of the global config, one after the other in the order they came, never a lock held
across git; a write of a companion MUST hold the writers of both configs for its whole span, taken in the order of
their files. A `config.lock` of another process MUST be waited for up to a second, then fail with the reason.

#### Scenario: Two writes of the app at once
- **WHEN** the app writes two keys of a repository at once
- **THEN** both are on disk, and no write was tried again for a lock of the other
  [@test] crates/app-api/src/app/config_writer_test.rs (`two_writes_of_different_keys_at_once_are_both_on_disk`)

#### Scenario: One writer for every spelling and for each side of a companion
- **WHEN** the app writes the config of a repository through two spellings of its path, or adds a companion
- **THEN** one writer runs the writes of each config file: both spellings through one; the add holds the writer of the
  primary's config and the writer of the companion's until it ends, and a write of either waits for it
  [@test] crates/app-api/src/app/config_writer_test.rs (`two_spellings_of_one_repository_write_through_one_task`, `a_companion_write_holds_the_writers_of_both_configs`, `a_write_together_holds_the_writers_of_its_configs_until_it_ends`)

#### Scenario: Two writes of companions of one pair at once
- **WHEN** two writes hold the writers of one pair of configs at once, each naming them in another order
- **THEN** both end: each takes the writers in the order of their files
  [@test] crates/app-api/src/app/config_writer_test.rs (`two_writes_together_of_one_pair_named_in_opposite_orders_both_end`)

#### Scenario: A `config.lock` of another process
- **GIVEN** another process holds `config.lock`
- **WHEN** the app writes the config
- **THEN** the write waits for it up to a second, and fails on a lock that stays: "Cannot write branchdeck.… : another
  git process held the lock of the config file (config.lock) for more than 1 s"
  [@test] crates/sync-core/src/repository_config_test.rs (`a_write_waits_for_a_config_lock_another_process_holds`, `a_write_fails_on_a_config_lock_that_stays`)
