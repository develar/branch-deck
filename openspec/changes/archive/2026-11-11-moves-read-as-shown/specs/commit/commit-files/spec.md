## MODIFIED Requirements

### Requirement: Files section: The tints of a block are IntelliJ's

The tints MUST be IntelliJ's: a block with inner ranges draws its lines pale and its ranges in the full colour; a
block without them (line-level only, or ranges dropped) draws its lines in the full colour; a whitespace-only block
draws its lines pale with nothing strong. The lines of a quiet run of the marks are our own: in the quiet colour,
with no range; so are the lines of a move, in its colour, pale where edited on the way
([Quiet Changes](../quiet-changes/spec.md#views)).

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

#### Scenario: The lines of a move
- **WHEN** lines of a block with inner ranges are lines of a move, one of them edited on the way
- **THEN** they are cells of a move in every layout, full as they were and pale where edited, with the ranges of the move and none of the block
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move as its own cell at both ends in every layout, and draws the ranges of the move in an edited line alone`)
  [@test] src/lib/diff/rows.test.ts (`shows the lines of a move once in Inline, where they stand now, and one note that opens where they left`)
