## MODIFIED Requirements

### Requirement: File diffs: The result does not depend on the load of the machine

The result MUST NOT depend on the load of the machine: in a view only a pathological file (100 ms for its inner
ranges, 1 s for its lines) is compared by line, a scan (`blocks_of`: the hunks of a commit, its patch, the pairs of a
review plan) MUST compare without a deadline, bounded by its work alone, and MUST NOT take a diff of memory that the
time of a view may have cut short; an answer of the index that its query cut short MUST NOT outlive its question.

#### Scenario: A file out of time
- **WHEN** a file takes more than 100 ms for its inner ranges or more than 1 s for its lines
- **THEN** it is compared by line
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

#### Scenario: A scan under any load
- **GIVEN** a commit that changes a short file and one over the line limit of the inner ranges
- **WHEN** an instance whose budgets are already over scans it, before and after its views of the commit, which the time cut short
- **THEN** the scan has the blocks of a comparison without a limit both times, and the store has none of the views' diffs
  [@test] crates/file-diffs/src/blocks_test.rs (`a_scan_has_the_blocks_of_no_deadline_and_a_diff_the_time_cut_short_reaches_no_store`)

#### Scenario: An answer of the index that its query cut short
- **WHEN** the index answers the renames of a subject without getting to the end of a declaration: its budget ran out before it, or its answer is not complete
- **THEN** the answer is that of its question alone, and the next question of the same subject asks the index again
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)
