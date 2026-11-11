## MODIFIED Requirements

### Requirement: The sidebar and the diffs: The diff search reads the model

The diff search MUST remain model-based and find what the rows of the layout show: both sides of a change in Split, in
one column a deleted row by its old line and the row of a similar pair of Inline by both its lines, the new side of an
unchanged line, and with the checkbox folded diff lines too. A text Unified or Split finds, Inline finds too.

#### Scenario: The rows, the sides and the folds
- **WHEN** the query is in the diff rows
- **THEN** it matches in the lines the rows show, ignoring case, in the order of the rows
- **AND** both sides of a change in Split and the new side of an unchanged line, and with the checkbox the folded lines too
  [@test] src/lib/diff/find.test.ts (`finds the query in the lines the rows show, ignoring case, in the order of the rows`, `searches both sides of a change in Split and the new side of an unchanged line, and the folds on request`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the diffs from the model, also in the folded lines, with ⌘F, Enter and Escape`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text on both sides of a change in Split`)

#### Scenario: A deleted row of one column
- **WHEN** the query is in a deleted line that Unified, or Inline for a block of no similar pair, shows as a row, or in a line of a deleted file
- **THEN** it matches there, in the old line
  [@test] src/lib/diff/find.test.ts (`searches the deleted rows of Unified and of Inline by their old line, and the lines of a deleted file`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the deleted parts of an Inline row, struck through, and in the deleted rows of Unified`)

#### Scenario: The deleted parts of an Inline row
- **WHEN** the query is in the old line of a similar pair that Inline shows as one row
- **THEN** a hit that holds a deleted part, or a place where the new line inserts text, is a hit of its own, marked in the struck-through part and on in the text the row shows from the new line
- **AND** a hit in the text the two lines share counts once, as the hit of the new line
- **AND** the hits of the row go in the order of its text, a deleted part before the inserted one
  [@test] src/lib/diff/find.test.ts (`searches an Inline row in its new line and in the deleted parts of its old line, in the order of the text of the row`, `finds in an Inline row a text of the old line that the new line cuts with an insertion, or has lost with a deletion`)
  [@test] src/lib/diff/line-parts.test.ts (`marks a hit of the old line in its deleted part and on in the text the row shows from the new line`, `marks a hit of the old line around the text the new line inserts into it`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should find a text in the deleted parts of an Inline row, struck through, and in the deleted rows of Unified`)
