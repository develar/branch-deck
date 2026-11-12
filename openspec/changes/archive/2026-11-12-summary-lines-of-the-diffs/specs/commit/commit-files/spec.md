## MODIFIED Requirements

### Requirement: File diffs: A hunk header names the declaration around it

The hunk headers MUST name the declaration around each hunk, `Outer › inner`, from the scopes of the diff. The lines
of a scope MUST be those of the diff, where a lone `\r` ends a line too, never those of git that the syntax summary
counts.

#### Scenario: A member and a Rust impl member
- **WHEN** a hunk changes a member, or a member of a Rust impl
- **THEN** its header names it with its containers, `Outer › inner`, a Rust impl member with its type
  [@test] crates/file-diffs/src/scopes_test.rs (`a_member_is_labelled_with_its_containers`, `a_rust_impl_member_is_labelled_with_its_type`)

#### Scenario: A lone `\r` above a declaration
- **GIVEN** a text whose comment above its declarations holds a lone `\r`, so the diff counts one line more than git above them
- **WHEN** the scopes of the text are read from its summary
- **THEN** each scope starts and ends at the lines the diff shows its declaration at
  [@test] crates/file-diffs/src/service_test.rs (`the_imports_and_the_scopes_of_a_text_with_a_lone_cr_are_at_the_lines_of_the_diffs`)
  [@test] crates/text-diff/src/text_lines_test.rs (`the_lines_of_a_summary_are_named_the_same_way_among_the_lines_of_the_diffs`, `the_lines_of_a_text_are_those_of_the_diffs`)
