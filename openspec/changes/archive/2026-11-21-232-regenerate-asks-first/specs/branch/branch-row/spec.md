## ADDED Requirements

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
- **AND** for an unassigned commit, or a branch built on what lands, it moves the commit into the branch whose copy
  merged the file
  [@test] crates/domain-policy/src/regenerate_test.rs (`the_fix_restacks_the_root_onto_the_branch_of_the_commit_the_copy_lacks_else_moves_the_commit`)

#### Scenario: A branch in master or parked
- **WHEN** the branch is in master, by the baseline, a Safe Push of another branch or its own merged robot, or parked
- **THEN** the cell says no regenerate
  [@test] crates/domain-policy/src/row_status_test.rs (`a_copy_to_regenerate_is_said_in_the_cell_unless_it_landed`)
  [@test] crates/domain-policy/src/regenerate_test.rs (`the_cell_and_the_question_name_the_files_unless_the_branch_lands_nothing`)

#### Scenario: A whole stack
- **WHEN** the stack branch of a whole stack holds such a file
- **THEN** its push and its start ask first, and its row, which has no Status cell, says nothing

## MODIFIED Requirements

### Requirement: Status cell: Work in flight never hides behind the state of the row

Work in flight MUST NOT hide behind the state of the row, nor the state behind it: the first that applies is the first
column, a running robot, then what landed all of it ("landed", its own merged robot "Merged"), a failed sync ("merge
conflict", "analyzing conflict…"), parked, a blocked stacked branch ("blocked by auth"), a copy to regenerate
("regenerate Cargo.lock"), the state its next step waits in where its robot finished on the branch as it is covers
nothing ("Awaiting review"), its robot finished on the branch as it is.

#### Scenario: The order of the first column
- **WHEN** several of these facts apply to a branch
- **THEN** the first that applies is the first column
- **AND** only a Safe Push that lands it outranks a block, and nothing outranks a fix but master
  [@test] crates/domain-policy/src/row_status_test.rs (`only_a_safe_push_that_lands_it_outranks_a_block_and_nothing_outranks_a_fix_but_master`)

#### Scenario: A copy to regenerate among the facts
- **WHEN** the copy of a blocked branch whose robot failed holds a generated file the words merged
- **THEN** the block is the first column, the regenerate and the failed robot stay beside it
- **AND** beside a running robot, which tests that copy, the regenerate stays too
  [@test] crates/domain-policy/src/row_status_test.rs (`a_copy_to_regenerate_is_said_in_the_cell_unless_it_landed`, `no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`)

### Requirement: Status cell: The facts the first column covers stay beside it as a dot

The exceptional facts the first column covers (a failed sync, a block, what landed it, parked, a copy to regenerate, a
failed robot) MUST stay beside it as a dot in the colour of the strongest (`destructive` for a failed sync or robot,
`warning` for a block, parked or a copy to regenerate), each a line of its hover and of the hover of the node.

#### Scenario: A running Safe Push of a branch whose parent conflicts
- **WHEN** a Safe Push runs of a branch whose parent conflicts
- **THEN** the row shows the robot with "Blocked: parent auth has a merge conflict" beside it
  [@test] crates/domain-policy/src/row_status_test.rs (`a_safe_push_of_a_blocked_branch_is_landing_and_its_block_stays_beside_the_robot`)
  [@test] src/presenters/row-status.test.ts (`shows the robot Rust puts first, a block and a conflict beside it as a mark that names them, and a sync over all of it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show a running Safe Push of a blocked branch first, its block beside it, and its parent's conflict as the fix`)

#### Scenario: A finished robot beside another fact
- **WHEN** a finished robot neither failed nor landed all of it, and another fact applies
- **THEN** the robot covers nothing beside that fact
  [@test] crates/domain-policy/src/row_status_test.rs (`a_finished_robot_shows_alone_and_beside_another_fact_only_when_it_failed`)

### Requirement: Status cell: One table pins every combination of the facts of a row

Every combination MUST be pinned by one table (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`): no
fact drops from the row, the node and the cell tell one story, and a row whose next step is settled says why.

#### Scenario: Every combination of facts
- **WHEN** any combination of the facts of a row applies
- **THEN** no fact drops from the row, the node and the cell tell one story, and a row whose next step is settled says why
  [@test] crates/domain-policy/src/row_status_test.rs (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`)

#### Scenario: A copy to regenerate in every combination
- **WHEN** the copy of the row holds a generated file the words merged, with any other fact
- **THEN** the cell says it, first or beside the first, unless the branch is in master or parked
  [@test] crates/domain-policy/src/row_status_test.rs (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`)
