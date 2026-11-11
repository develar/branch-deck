## MODIFIED Requirements

### Requirement: Files section: Inline shows a similar pair of a block as one row

Inline MUST show each pair of a block that the diff engine pairs as one row: the new line with the deleted
parts of the old line before the inserted parts, each part in the syntax colours of its own side, the deleted parts
left out of a copy, the line number of the new line.

#### Scenario: A lightly edited line
- **WHEN** Inline shows a block whose old and new line are similar
- **THEN** they are one row, the new line with the deleted parts of the old line before the inserted parts
- **AND** each part has the syntax colours of its own side, the deleted parts are left out of a copy, the line number is the new line's
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a lightly edited line as one row`)
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline shows a re-indented line as one pale row, its indentation in the full colour, and the whitespace policy hides it`)
  [@test] src/lib/diff/line-parts.test.ts (`puts the deleted part of the old line before the inserted part, each with the runs of its side`)

## REMOVED Requirements

### Requirement: Files section: A line of a pair without a range has an empty one

**Reason**: The engine marks the empty side of an inserted and of a deleted change in the line of its pair, so the
two lines of a pair always have as many ranges; the page no longer makes up an empty range for a line without one.

**Migration**: "Files section: The row of a pair hides no text of its lines".

## ADDED Requirements

### Requirement: Files section: The row of a pair hides no text of its lines

The row of a pair MUST show all of both its lines: the text they share once, as the new line has it, each deleted
part of the old line and each inserted part of the new one. The empty side of an inserted or a deleted change needs
no marker in the row. Two lines whose ranges do not align, which no engine pairs, MUST show whole: the old line
struck through before the new line.

#### Scenario: A pure insertion and a pure deletion
- **WHEN** a pair is a pure insertion, or a pure deletion, the engine marking its empty side
- **THEN** the row is the new line with the inserted part marked, or the new line with the deleted part where it stood, and no marker
  [@test] src/lib/diff/line-parts.test.ts (`shows a pure insertion as the new line: the engine marks the empty old side, which the row needs no marker for`)

#### Scenario: Ranges that do not align
- **WHEN** the two lines of a row have ranges that do not align by index
- **THEN** the row is the whole old line struck through before the whole new line
  [@test] src/lib/diff/line-parts.test.ts (`takes the ranges of the engine, which align, and shows any other two lines whole`)
  [@test] src/lib/diff/inline-rows.test.ts (`is the new line with the deleted parts of the old line before the inserted parts`)

### Requirement: Files section: The engine pairs only lines that one row shows whole

The engine MUST pair line i of the two sides of a block only when the sides have as many lines, each pair is similar,
each inner change lies within line i of both sides for one i, and between its changes a pair is the same text on both
sides but for whitespace. A block of other lines MUST show in Inline line by line, as in Unified, with its ranges.

#### Scenario: A call wrapped again
- **GIVEN** a call over two lines whose argument goes up a line while its last argument goes
- **WHEN** Inline shows its block
- **THEN** the block shows line by line, as in Unified, with its ranges: the change of the line end has its sides in lines apart, so the second lines are no pair
- **AND** the deleted row shows the argument the old line lost
  [@test] crates/text-diff/src/model_test.rs (`a_change_across_a_line_end_leaves_a_block_without_inline_pairs`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show a call wrapped again line by line in Inline, and find the argument it lost`)

#### Scenario: A line end that moves under Ignore Whitespace
- **GIVEN** a block under Ignore Whitespace whose word goes up a line without a change of its own
- **WHEN** the engine pairs its lines
- **THEN** the block has no pairs, though the ranges of its lines align: the text between them is not the same
  [@test] crates/text-diff/src/model_test.rs (`a_line_end_that_moves_under_ignore_leaves_a_block_without_inline_pairs`)

#### Scenario: Another indent under Trim Whitespace
- **GIVEN** a line indented again and edited, under Trim Whitespace
- **WHEN** the engine pairs its lines
- **THEN** they are a pair, the text before its change longer in the new line
  [@test] crates/text-diff/src/model_test.rs (`a_reindented_and_edited_line_is_a_pair_under_trim`)

#### Scenario: Every pair of the engine
- **WHEN** the engine pairs the lines of any block
- **THEN** line i of both sides has as many ranges, of the same kinds, and between them the same text but for whitespace, the same text under Do Not Ignore
  [@test] crates/text-diff/src/model_test.rs (`blocks_partition_both_texts`)
