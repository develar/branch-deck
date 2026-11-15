## MODIFIED Requirements

### Requirement: The sidebar and the diffs: N and P show the next and the previous change

N and P MUST show the next and the previous stop: the line the reading of a block of changed lines starts at, as the
marks of its diff say it ([Quiet Changes](../quiet-changes/spec.md#views)), into the next or the previous file at the
end of one. A block with nothing to read is passed; collapsed files are skipped. They step from the last stop they
showed, else from the file at the top, else from the file the tree selects.

#### Scenario: N and P through the blocks and the files
- **WHEN** the user presses N or P
- **THEN** the next or the previous block of changed lines with something to read is shown, into the next or the previous file at the end of one
- **AND** collapsed files are skipped
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`, `N and P skip a block the whitespace policy ignores`)

#### Scenario: N and P from the selected file
- **WHEN** the user presses N or P after Show All Files of a chip whose one file stays selected below the files it shows again, before the diffs showed a change or scrolled
- **THEN** N and P step from the selected file: P shows the last change of the file before it
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should step N and P from the selected file once Show All Files shows the files above it, before the diffs showed a change`)

#### Scenario: A block of formatting alone
- **WHEN** a Kotlin file indents two lines again, adds a blank line and changes a value, under Do Not Ignore
- **THEN** N stops at the blank line and at the value, and passes the indentation, a quiet run
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`)
