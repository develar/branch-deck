## ADDED Requirements

### Requirement: Runs: Blocks the comparison cut out of one change are read as one

Blocks in a row that keep a start after their own reading, at most eight, with text and none cut off, MUST be read as
one region with the lines between them where their characters but for the blanks are the same on both sides: one
change the comparison cut. Its runs are cut at the ends of its blocks, a part with lines on
one side alone where a block has no other. The region MUST be taken only when the parts leave no line of its blocks to
read, the smallest from each block on.

#### Scenario: A function indented again with its lambda wrapped
- **WHEN** a function of Kotlin is indented from four columns to two and the body of its lambda is put on a line of its own, and the comparison pairs the old `    }` that ends the function with the new `    }` that ends the lambda
- **THEN** its two blocks, the second only the `}` the function ends with now, are a quiet run of formatting each, and neither has a start
- **AND** a file of ten members reformatted so has nothing to read; its review plan folds every block into the unit of whitespace, and its reading has nothing to read, by formatting
  [@test] crates/file-diffs/src/marks_test.rs (`blocks_the_comparison_cut_out_of_one_change_are_read_as_one`)
  [@test] crates/review-plan/src/reading_test.rs (`a_reformat_the_comparison_cut_in_two_leaves_nothing_to_read_and_one_unit_of_whitespace`)

#### Scenario: What a region does not prove
- **WHEN** a change stands inside such blocks, a space inside a string makes the same characters other tokens, or the blocks are blank lines alone
- **THEN** each block is read alone, and keeps its start
  [@test] crates/file-diffs/src/marks_test.rs (`blocks_the_comparison_cut_out_of_one_change_are_read_as_one`)

#### Scenario: Edited at random
- **WHEN** files of every grammar are edited at random and compared with themselves
- **THEN** a run of formatting whose lines are not the same tokens alone is the quiet part of a run of blocks in a row whose lines are
- **AND** when no block of a diff that ignores nothing has a start, the two texts have the same tokens
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`)

## MODIFIED Requirements

### Requirement: Runs: A block is cut at the line boundaries both sides share

A block of several changes MUST be cut where both sides have a line boundary, crossed by no token, with the same
tokens of the code before it. Between the cuts, the longest rows of lines whose tokens are the same are runs of
formatting, the blank lines inside them with them; lines between such rows that differ in their comments alone are a
run of comments. A run MUST lie inside one block, with lines on both sides but as a part of blocks read as one.

#### Scenario: Lines wrapped into a block, a change beside lines indented again
- **WHEN** lines are wrapped into a new `if` and indented, or indented lines stand beside a line that really changed
- **THEN** the indented lines are a quiet run, and the reading of the block starts at the `if`, at the changed line, at a comment added between them, or at the old line removed between them
- **AND** a comment worded again among them is a run of comments of its own
- **AND** a comment that moved to a line of its own is in the run of formatting, and comments worded again around an added comment are one run of comments
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: No boundary both sides share
- **WHEN** a wrapped call and a changed call stand on one line of a side
- **THEN** the block has no run
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: The template of the E2E tests
- **WHEN** the commit of the `commit_marks` template is read
- **THEN** each of its cases is a block of its own: the wrapped call a quiet run, the comment a label, the renamed names, the real change and the lines that left for another file blocks with a start
- **AND** `accounts` is a move that counts the blank line that moved with it, and `describe` a move indented by two columns with one edited line: the blocks they stand in have a start, the blocks they left have none
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

### Requirement: Never marked: A side of no lines, blank lines alone and a block the policy cut off

A block with a side of no lines (added lines, deleted lines, an added comment), lines that are blank on both sides
and a block the policy cut off MUST have no run: a quiet run needs tokens that are the same on both sides. The first
two MUST stay stops. A block with a side of no lines MAY be the part of a run of the blocks the comparison cut out of
one change, read as one: the tokens of the region are the same.

#### Scenario: Added and deleted lines, blank lines
- **WHEN** a block adds lines, deletes lines, adds a comment on a line of its own, adds a blank line, or has blank lines alone on both sides
- **THEN** it has no run and its reading starts at its first line, under the policies that keep it a block
- **AND** a blank line `IgnoreBlankLines` cut off has no run and no start
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_with_a_side_of_no_lines_or_of_blank_lines_alone_is_never_marked`)
