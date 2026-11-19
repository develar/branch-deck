## MODIFIED Requirements

### Requirement: `brd sync`: It refuses during an operation, on a detached HEAD or a managed branch

`brd sync` MUST refuse, changing nothing, while a rebase, merge, cherry-pick or revert is in progress
(`operation_in_progress`), HEAD is detached (`detached_head`), a branch under the prefix is checked out
(`managed_branch`) or the history ahead does not build on the baseline (`unrelated_history`, the root in
`error.commits`, `ask_user`; exit 4 all four): the sync would archive the branches the half-done history, or the
history of one group, lacks, or read the whole history of HEAD.

#### Scenario: A rebase in progress
- **WHEN** a rebase is in progress
- **THEN** `brd sync` is refused with `operation_in_progress`, exit 4, and nothing changes
  [@test] crates/bd-cli/src/sync_test.rs (`waits_while_a_rebase_is_in_progress`)

#### Scenario: A branch under the prefix checked out
- **WHEN** a branch under the prefix is checked out
- **THEN** the sync is refused with `managed_branch`, exit 4, and nothing changes
  [@test] crates/sync-core/src/sync_test.rs (`sync_waits_while_a_branch_under_the_prefix_is_checked_out`)

#### Scenario: A detached HEAD
- **WHEN** HEAD is detached
- **THEN** the sync is refused with `detached_head`, exit 4, and nothing changes

#### Scenario: A history unrelated to the baseline
- **WHEN** the baseline is a history unrelated to HEAD
- **THEN** `brd sync` and a rewrite (`brd reword`) are refused with `unrelated_history`, exit 4, the root of HEAD in
  `error.commits` and `ask_user`, and nothing changes
  [@test] crates/bd-cli/src/sync_test.rs (`refuses_a_head_unrelated_to_the_baseline`)
