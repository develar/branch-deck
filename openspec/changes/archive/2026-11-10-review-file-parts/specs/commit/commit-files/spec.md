## ADDED Requirements

### Requirement: Reading the diffs: The counts of a file mark the lines the policy ignores

Once the diff of a file is read, its counts in the tree MUST mark the lines of the blocks the whitespace policy
ignores (of each the most of a side), a dotted underline whose hover says "1 line hidden by Whitespace", as the
description of the row does; a file of no such block, one whose diff is not read and the review as a whole MUST have
no mark.

#### Scenario: A blank line under Ignore Whitespace and Blank Lines
- **WHEN** a file of the tree holds a blank line that Ignore Whitespace and Blank Lines ignores
- **THEN** its counts are dotted-underlined, their hover and the description of the row say "1 line hidden by
  Whitespace"
- **AND** under Do Not Ignore, and under a policy that compares the lines it ignores as equal (Trim Whitespace), no
  mark
  [@test] src/lib/diff/view-files.test.ts (`counts the lines the whitespace policy hides once the diff is read: of each ignored block the most of a side`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change, and the counts say how many lines the policy hides`)
