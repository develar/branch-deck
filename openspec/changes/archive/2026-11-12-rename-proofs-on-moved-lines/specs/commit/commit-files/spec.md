## MODIFIED Requirements

### Requirement: File diffs: The result does not depend on the load of the machine

The result MUST NOT depend on the load of the machine: only a pathological file (100 ms for its inner ranges, 1 s for
its lines) is compared by line, and an answer of the index that its query cut short MUST NOT outlive its question.

#### Scenario: A file out of time
- **WHEN** a file takes more than 100 ms for its inner ranges or more than 1 s for its lines
- **THEN** it is compared by line
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_out_of_time_is_by_line_and_not_stored`)

#### Scenario: An answer of the index that its query cut short
- **WHEN** the index answers the renames of a subject without getting to the end of a declaration: its budget ran out before it, or its answer is not complete
- **THEN** the answer is that of its question alone, and the next question of the same subject asks the index again
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)
