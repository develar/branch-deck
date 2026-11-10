## ADDED Requirements

### Requirement: Reading the diffs: The empty line after a final line end pairs only with its like

The comparison MUST pair the empty line after the final line end of a text, and the one line of an empty text, with no
line but the same line of the other text, under every whitespace policy: a blank line added or removed at the end of a
file is a change of that blank line, and the counts of a file differ by the lines it grew by, as git's do.

#### Scenario: A blank line and declarations after the closing brace of a class
- **WHEN** a commit adds a method to a class and, after its closing brace, a blank line and two declarations
- **THEN** the diff shows the blank line added, and the file counts +9 −0 under every whitespace policy, as
  `git diff --numstat` does
  [@test] crates/text-diff/src/model_test.rs (`the_empty_last_line_pairs_only_with_the_empty_last_line`)
  [@test] crates/trial-check/src/diff_scan_test.rs (`read_commit_diffs_gives_git_counts_beside_the_engines`)

#### Scenario: A blank line added or removed at the end of a file
- **WHEN** a commit adds a blank line at the end of a file or removes one, with any line end, or an empty file gets lines
- **THEN** the diff shows that blank line added or removed and counts it, pale under Ignore Whitespace and cut off
  under Ignore Whitespace and Blank Lines as any blank line
  [@test] crates/text-diff/src/model_test.rs (`the_empty_last_line_pairs_only_with_the_empty_last_line`)

#### Scenario: Any two texts
- **WHEN** any two texts are compared, under any whitespace policy
- **THEN** the lines of all their blocks, without the empty line after a final line end, differ by the lines the file
  grew by
  [@test] crates/text-diff/src/model_test.rs (`blocks_partition_both_texts`)
