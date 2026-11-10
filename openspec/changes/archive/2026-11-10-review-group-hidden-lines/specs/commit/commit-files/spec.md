## ADDED Requirements

### Requirement: Reading the diffs: The counts of a group mark the lines its files hide

The counts of a group of the tree (a directory, a step and its commit, a concern, a group of a kind) MUST carry the
mark of the counts of a file when its files whose diff is read hide lines, their sum in its hover and in the
description of its row ("1 line hidden by Whitespace"); a row "also in" it counts none of its file's, and a group none
of whose read files hides a line MUST have no mark.

#### Scenario: The row of a step, its commit
- **WHEN** the review of a branch tells each commit as a step, and Ignore Whitespace and Blank Lines hides a blank line
  of a file of a step
- **THEN** the counts of the row of that step, its commit, sum the lines of its files under the policy and are
  dotted-underlined, their hover and the description of the row say "1 line hidden by Whitespace"
- **AND** the step of a file that hides nothing, and every step under Do Not Ignore, has no mark
  [@test] src/lib/file-tree.test.ts (`marks the counts of the row of a step, its commit, with the lines the whitespace policy hides in its read files`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`the row of a step, its commit, marks the lines the whitespace policy hides in its files once their diffs are read`)

#### Scenario: A directory
- **WHEN** a file of a directory hides lines under the policy once its diff is read
- **THEN** the directory and each directory above it sum them, "2 files, +2 −2, 1 line hidden by Whitespace"
- **AND** before the diffs are read, and in a directory that holds only a row "also in" of such a file, no mark
  [@test] src/lib/file-tree.test.ts (`sums the lines the whitespace policy hides in the read files of each directory, and marks none without them`)
  [@test] src/lib/file-tree.test.ts (`shows a file of what no evidence joined that is also in a concern in its place under Other changes`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change, and the counts say how many lines the policy hides`)
