## ADDED Requirements

### Requirement: File diffs: A merge the time cut short reaches no store

The `file_merge` store MUST NOT keep a merge whose deadline ran out (`BudgetedMerge::out_of_time`: its lines, then one
block of the three whole texts, or the words of a block), also of a text over the limits of the inner ranges; memory
keeps it for the run. A side over a size limit or a comparison of too many changes is a limit of the work, and such a
merge is stored.

#### Scenario: A merge whose lines ran out of time
- **GIVEN** a conflict in a text over the line limit of the inner ranges
- **WHEN** the views merge it with a line budget already over, then another app merges it through the same store
- **THEN** the first merge is one block of the three whole texts, and the other app gets the blocks of a merge without a limit
  [@test] crates/file-diffs/src/merge_test.rs (`a_merge_whose_lines_ran_out_of_time_reaches_no_store`)
