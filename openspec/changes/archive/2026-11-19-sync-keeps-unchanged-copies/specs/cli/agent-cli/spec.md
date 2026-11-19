## ADDED Requirements

### Requirement: `brd sync`: A rewrite below a branch keeps the copies whose content stays

A sync MUST keep the commit of an existing virtual branch on the parent a copy goes onto when it has the tree, the
message and the author the copy gets, while every copy below it is kept, whatever the original's mapping note says or
without one; it is `Unchanged`, and the note names it anew. A commit with another tree, message or author MUST be
copied again.

#### Scenario: A fold of another branch's fixup below the branch
- **GIVEN** a fixup of `auth` above the commit of `cache`, and both branches synced
- **AND** the fold gives the replayed commit of `cache` new trees and carries no note to it
- **WHEN** the fixup is folded and the branches are synced again
- **THEN** the copy of `cache` is kept, `Unchanged`, and the note of its replayed original names it
- **AND** `auth` is copied again
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_fold_below_a_branch_keeps_the_copies_whose_content_stays`)
  [@test] crates/sync-core/src/process_budget_test.rs (`a_sync_after_a_fold_below_a_branch_writes_only_the_folded_branch`)

#### Scenario: The notes are gone
- **WHEN** the notes ref is deleted and the branches are synced again
- **THEN** every copy is kept, `Unchanged`
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_rebuild_without_notes_writes_the_same_commits`)

#### Scenario: A note carried to a commit with another tree
- **WHEN** a note is copied onto a commit whose copy gets another tree
- **THEN** the commit is copied again
  [@test] crates/sync-core/src/branch_processor_test.rs (`a_note_copied_to_a_commit_with_another_tree_is_stale`)
