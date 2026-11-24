## MODIFIED Requirements

### Requirement: Status cell: A copy with a generated file merged by words says regenerate

A branch whose copy holds a generated file the words merged in a conflict of its sync (`RowFact::Regenerate`,
`domain_policy::regenerate`; its own or of a branch it is built on) MUST say "regenerate Cargo.lock" ("regenerate 2
files") in `warning`, its hover naming each file, why it is generated, the copy that merged it, the commit the copy
lacks and the fix; a branch in master or parked MUST NOT, since it lands nothing.

#### Scenario: A copy that merged a lock file by words
- **WHEN** the copy of a branch holds a lock file the words merged, and the copy lacks a commit of `core`
- **THEN** the cell says "regenerate Cargo.lock" in `warning`
- **AND** its hover names `Cargo.lock`, "Lock file", the commit whose copy merged it, the commit of `core` it lacks and
  "Stack feature on core to land it too.", and that it clears once `core` lands or a commit of the branch writes the
  file
  [@test] crates/domain-policy/src/regenerate_test.rs (`the_cell_and_the_question_name_the_files_unless_the_branch_lands_nothing`)
  [@test] crates/domain-policy/src/row_status_test.rs (`a_copy_to_regenerate_is_said_in_the_cell_unless_it_landed`)
  [@test] src/presenters/row-status.test.ts (`says regenerate with each file, why it is generated and the commit its copy lacks`)
  [@test] tests/e2e/outgoing/regenerate.spec.ts (`should mark a branch whose copy merged a generated file by words and ask before its push`)

#### Scenario: The fix
- **WHEN** the branch of the commit the copy lacks is not built on what lands
- **THEN** the fix restacks the root of what lands onto that branch
- **AND** for a branch built on what lands it is the Safe Push of that branch, when it is built on the branch whose
  copy merged the file, else of the whole stack: it lands both, never a move
- **AND** for an unassigned commit it groups the commit into the branch whose copy merged the file
  [@test] crates/domain-policy/src/regenerate_test.rs (`the_fix_restacks_the_root_onto_the_branch_of_the_commit_the_copy_lacks_else_starts_what_lands_both_or_groups_the_commit`)

#### Scenario: A branch in master or parked
- **WHEN** the branch is in master, by the baseline, a Safe Push of another branch or its own merged robot, or parked
- **THEN** the cell says no regenerate
  [@test] crates/domain-policy/src/row_status_test.rs (`a_copy_to_regenerate_is_said_in_the_cell_unless_it_landed`)
  [@test] crates/domain-policy/src/regenerate_test.rs (`the_cell_and_the_question_name_the_files_unless_the_branch_lands_nothing`)

#### Scenario: A whole stack
- **WHEN** the stack branch of a whole stack holds such a file
- **THEN** its push and its start ask first, and its row, which has no Status cell, says nothing
