## MODIFIED Requirements

### Requirement: Files section: The tints of a block are IntelliJ's

The tints MUST be IntelliJ's: a block with inner ranges draws its lines pale and its ranges in the full colour; a
block without them (line-level only, or ranges dropped) draws its lines in the full colour; a whitespace-only block
draws its lines pale with nothing strong. The lines of a quiet run of the marks are our own: in the quiet colour,
with no range ([Quiet Changes](../quiet-changes/spec.md#views)).

#### Scenario: Blocks with and without inner ranges
- **WHEN** a diff has a block with inner ranges and a block without them
- **THEN** the first draws its lines pale and its ranges in the full colour, the second its lines in the full colour
  [@test] src/lib/diff/rows.test.ts (`draws the lines of a block with inner ranges pale and the lines of a block without them in the full tint, as IntelliJ`)

#### Scenario: A whitespace-only block
- **WHEN** a diff has a whitespace-only block
- **THEN** its lines are pale with nothing strong
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`)

#### Scenario: A quiet run
- **WHEN** lines of a block with inner ranges are a quiet run of the marks
- **THEN** they are quiet in every layout and show no range, and the other lines of the block keep their tint and their ranges
  [@test] src/lib/diff/row-cells.test.ts (`types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line`)

### Requirement: Whitespace: Under Do Not Ignore a change of indentation has inner ranges

Under Do Not Ignore a change of indentation MUST be a change with inner ranges, as in IntelliJ. In a file no grammar
reads its lines are pale, and the indentation is in the full colour. In a file of a grammar whose tokens prove the
change formatting alone, the lines are a quiet run ([Quiet Changes](../quiet-changes/spec.md)): quiet, no range drawn.

#### Scenario: A change of indentation under Do Not Ignore
- **WHEN** a file no grammar reads changes only its indentation, under Do Not Ignore
- **THEN** its lines are pale, and the indentation is in the full colour
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a re-indented line of a file without a grammar as one pale row, its indentation in the full colour, and the whitespace policy hides it`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`stays the pale stop it was: no note, no quiet line, and N stops at it`)

#### Scenario: A Kotlin file
- **WHEN** a Kotlin file indents two lines again, under Do Not Ignore, in Split
- **THEN** the lines are quiet on both sides after the note "Formatting only · 2 lines", none of their indentation marked
- **AND** the file says that two of its three changes are left to read
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Ignore Whitespace hides an indentation-only change, and the counts say how many lines the policy hides`)

### Requirement: Whitespace: A whitespace-only block is drawn pale and counts

A block that differs only in whitespace MUST count, and MUST be drawn pale and be a stop of N and P unless the
tokens of its file prove lines of it a quiet run ([Quiet Changes](../quiet-changes/spec.md)): a file no grammar
reads, and a block with a side of no lines, keep the pale stop.

#### Scenario: A whitespace-only block
- **WHEN** a diff has a block that differs only in whitespace, without a run of the marks
- **THEN** it is drawn pale, it counts, and N and P stop at it
  [@test] src/lib/diff/rows.test.ts (`draws a whitespace-only block in the whitespace tint, and a block the policy ignores as unchanged lines inside the context`, `starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)

#### Scenario: A blank line added under Ignore Whitespace
- **WHEN** a Kotlin file adds a blank line, under Ignore Whitespace
- **THEN** the block is pale and N stops at it: a side of no lines is never a run
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)
