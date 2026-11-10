## ADDED Requirements

### Requirement: `brd sync`: A copy of an older resolve by words is checked once

A copy whose mapping note names an older resolve by words MUST be checked once by its tree: the same tree keeps the
commit, with no new copy and no move of its branch, and its note MUST take the current resolve, so that no later sync
checks it again; another tree MUST copy the commit again, and a conflict that the current resolve leaves MUST be the
conflict of git.

#### Scenario: The same bytes
- **WHEN** the current resolve gives the tree of the copy
- **THEN** the copy stays, unchanged, and its note names the current resolve
- **AND** the next sync runs the processes of an unchanged sync
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_the_same_bytes_keeps_its_commit_and_takes_this_resolve`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_copy_of_an_older_resolve_is_checked_once`)

#### Scenario: Other bytes
- **WHEN** the current resolve gives another tree
- **THEN** the commit is copied again with the tree of the current resolve, and its note names the new copy
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_with_other_bytes_is_copied_again`)

#### Scenario: A conflict the current resolve leaves
- **WHEN** the current resolve leaves the conflict of git
- **THEN** the commit is that conflict, and a copy noted by the current resolve is taken as it is
  [@test] crates/trial-check/src/word_resolve_sync_test.rs (`a_copy_of_an_older_resolve_that_this_resolve_leaves_is_the_conflict_of_git`)
