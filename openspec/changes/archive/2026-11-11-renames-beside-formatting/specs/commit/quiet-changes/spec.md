## ADDED Requirements

### Requirement: Renames: A run of a renamed name takes no line of formatting

Lines that are the same tokens with the names of a rename read as one and hold no place MUST be the run of formatting
they are without the rename, quiet whatever the rename is, with the blank lines beside them. The second reading of a
block MUST take no run: a run the first reading found MUST lie in a run of its kind, and a run of the second reading
that shares a line with it without holding it, or crosses it, is none.

#### Scenario: Lines of formatting beside the name
- **WHEN** a call is wrapped between or before the lines of a renamed local, two calls are wrapped each before a line of it, or twelve statements are indented inside a new `if`, one of them a use of the local
- **THEN** the wrapped calls and the indented lines are quiet runs of formatting, each row of lines of the name between them is a run of the kind `renamed`, and the reading of the last block starts at the `if`
- **AND** a blank line put before or after the wrapped call is in its run of formatting
- **AND** beside a rename that is a label for good (the use of a member, a parameter a call may name, a local of a C# file with an `#if`, a local of a text with a syntax error) the wrapped call is a quiet run of formatting too, and the reading of the block starts at the line of the name
  [@test] crates/file-diffs/src/marks_renames_test.rs (`lines_of_formatting_beside_a_renamed_name_are_a_run_of_their_own`)

#### Scenario: The two readings pair the lines otherwise
- **WHEN** a line with three places of a renamed local changes places with a call that is wrapped: the tokens of the texts pair the call, and with the names read as one the line of the places pairs with itself
- **THEN** the wrapped call is the quiet run of formatting the first reading found, and the line of the places is in no run and is read
- **AND** of the runs of two readings, a run of the second that holds a run of the first in its kind stays with it, and one that shares a line with a run of the first it does not hold, or crosses it, is none
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_run_of_the_first_reading_stays_where_the_second_aligns_the_lines_otherwise`)

#### Scenario: The runs of the first reading
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and the blocks of each diff are read once more from the tokens of the texts as they are
- **THEN** every run that reading finds lies in a run of its kind of the marks
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)
