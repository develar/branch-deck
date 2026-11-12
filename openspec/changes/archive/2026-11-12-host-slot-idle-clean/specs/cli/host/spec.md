## MODIFIED Requirements

### Requirement: serve: The keeper wakes once a minute and at each start

The keeper MUST wake once a minute and at each `Start` of a repository, and run `ensure_host_git`, `reclaim`,
`reclaim_refs` (`lease_pool::host_runs::reclaim_all`) and `fill` in process, one repository at a time, then one
relief (`keep_free`) over the repositories, each repository's preceded by the clean of its host slots idle since their
last run (`clean_idle_slots`).

#### Scenario: A round of the keeper
- **WHEN** a minute passed, or a `Start` of a repository came
- **THEN** the keeper runs `ensure_host_git`, `reclaim`, `reclaim_refs` and `fill` in process for each repository, one
  at a time, then one relief over the repositories
  [@test] crates/host-agent/tests/serve/lend.rs (`the_keeper_fills_the_slots_and_backs_off_after_a_failed_fill`)
