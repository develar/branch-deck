## MODIFIED Requirements

### Requirement: Every ready lease of the pool is listed with its holder

`active_leases` MUST list every ready lease of the pool, so a session that lost its path finds its own by `holder`,
with `holder_alive` (omitted for a lease whose get recorded no holder process) and, for one whose holder is gone,
`idle` as `brd lease status` shows it (`idle_with_work` with its work: tell the user).

#### Scenario: A session that lost its path finds its lease by holder
- **WHEN** an agent runs `brd workspace` in the shared checkout or in a lease
- **THEN** `active_leases` lists every ready lease of the pool with its `holder`
- **AND** with `holder_alive`, on Windows too, omitted only for a lease whose get recorded no holder process
  [@test] crates/bd-lease/src/workspace_test.rs (`by_default_the_agent_leases_a_worktree_picks_its_commits_and_returns_it`)

#### Scenario: The holder of a lease is gone
- **WHEN** the holder of a ready lease is gone
- **THEN** its entry has `idle` as `brd lease status` shows it
- **AND** `idle_with_work` with its work: tell the user
