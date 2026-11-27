## ADDED Requirements

### Requirement: Output contract: A printed command carries the global options that change what it acts on

Every command brd prints for a worktree (`next`, `next_steps`, `push`, the commands a message names) MUST carry the
global options of its command line that change what a command acts on, `--prefix`, after its `-C`, so that run as
printed it acts on what the command that printed it did; one without them on the command line MUST name none.

#### Scenario: `--prefix` over the prefix of the config
- **GIVEN** the config of the repository has `branchdeck.branchPrefix=develar`
- **WHEN** `brd --prefix slc sync` runs, then `brd sync`
- **THEN** the first builds `slc/virtual/auth` and names its push `brd -C <repo> --prefix slc push auth`, the second
  `brd -C <repo> push auth`
  [@test] crates/bd-cli/src/sync_test.rs (`the_push_a_sync_names_carries_the_prefix_of_its_command_line`)
  [@test] crates/bd-cli/src/context_test.rs (`the_brd_of_a_context_names_its_repository_once`)

#### Scenario: A command of another worktree
- **WHEN** `brd --prefix test-user lease pick` conflicts in a lease, or refuses a message that the commit rules reject
- **THEN** the `update --continue`, `lease pick` and `reword` it names in the lease carry `--prefix test-user`
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_that_conflicts_opens_a_resolve_session_in_the_lease_and_lands_after_it`, `a_lease_pick_refuses_a_rejected_message_until_the_reword_in_the_lease`)
