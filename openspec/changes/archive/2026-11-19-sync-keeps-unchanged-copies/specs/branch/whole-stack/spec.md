## MODIFIED Requirements

### Requirement: The stack branch: It holds every commit of the stack as a flat build does

It MUST hold the original commits of all its branches in the order of the checked out branch's history, on the base of
the sync, copied as a flat virtual branch of those commits is (the prefix stripped, `branch-deck` the committer, the
deterministic dates), so a copy on the same parent is the same commit as the flat build's. As a virtual branch does, it
MUST keep a commit it holds on the same parent with the tree, the message and the author of the copy while every copy
below it is kept.

#### Scenario: A forking stack builds its commits in history order
- **WHEN** the sync builds the stack branch of a stack that forks
- **THEN** it holds the original commits of all its branches in the order of the checked out branch's history, on the base of the sync
- **AND** each is copied as a flat build copies it (the prefix stripped, `branch-deck` the committer, the deterministic dates), so a copy on the same parent is the same commit as the flat build's
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_forking_stack_builds_its_commits_in_history_order_as_one_branch`)

#### Scenario: A fold below the stack
- **GIVEN** a fixup of `docs` above commits of the stack, and the stack branch built
- **WHEN** the fixup is folded and the branches are synced again
- **THEN** the stack branch and its members keep their commits
  [@test] crates/sync-core/src/stack_sync_test.rs (`a_fold_below_a_stack_keeps_the_stack_branch_and_its_members`)

### Requirement: The stack branch: An unchanged stack costs no git process

A stack with the same commits on the same base MUST cost no git process: the outcome of a build is remembered by a
hash of the base and the original commit ids, in the memory of the process and in the object cache (`stack_branch`).
Without either it MUST be built again, to the same commit, and its ref MUST NOT move.

#### Scenario: The same commits on the same base
- **GIVEN** a stack branch built by an earlier sync, remembered in the memory of the process or in the object cache
- **WHEN** a sync finds the same commits on the same base
- **THEN** it reuses the stack branch without a git process
  [@test] crates/sync-core/src/stack_sync_test.rs (`an_unchanged_stack_reuses_its_stack_branch_without_a_git_process`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_first_sync_of_a_forking_stack`, `an_unchanged_sync_of_a_forking_stack`)

#### Scenario: The object cache keeps the builds by their version
- **WHEN** a build is remembered in the object cache (`stack_branch`)
- **THEN** it is read back by its hash, and a build of another version is never read
  [@test] crates/object-cache/src/cache_test.rs (`stack_builds_are_read_by_their_version`)

#### Scenario: Neither the memory nor the object cache has it
- **GIVEN** a new process without the object cache
- **WHEN** the sync finds the same commits on the same base
- **THEN** it builds the stack branch again from the commits it holds, writes no commit, and its ref doesn't move
  [@test] crates/sync-core/src/stack_sync_test.rs (`an_unchanged_stack_reuses_its_stack_branch_without_a_git_process`)
