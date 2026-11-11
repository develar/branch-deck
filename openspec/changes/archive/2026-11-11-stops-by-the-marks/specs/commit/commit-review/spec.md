## MODIFIED Requirements

### Requirement: The sidebar and the diffs: N and P show the next and the previous change

N and P MUST show the next and the previous stop: the line the reading of a block of changed lines starts at, as the
marks of its diff say it ([Quiet Changes](../quiet-changes/spec.md#views)), into the next or the previous file at the
end of one. A block with nothing to read is passed; collapsed files are skipped.

#### Scenario: N and P through the blocks and the files
- **WHEN** the user presses N or P
- **THEN** the next or the previous block of changed lines with something to read is shown, into the next or the previous file at the end of one
- **AND** collapsed files are skipped
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show the tree beside the diffs of all files and follow J, K, N, P and the tree`, `N and P skip a block the whitespace policy ignores`)

#### Scenario: A block of formatting alone
- **WHEN** a Kotlin file indents two lines again, adds a blank line and changes a value, under Do Not Ignore
- **THEN** N stops at the blank line and at the value, and passes the indentation, a quiet run
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`)

### Requirement: The sidebar and the diffs: The diffs and their texts come from the engine

The diffs SHALL come from `get_file_diffs` and their texts from `get_blob_texts`
([Commit Files](../commit-files/spec.md#file-diffs)): N and P step through the starts of the blocks, which a block
that is `Ignored` does not have.

#### Scenario: Blocks the policy ignores
- **WHEN** a file has a block of changed lines that is `Ignored`
- **THEN** N and P step past it
  [@test] src/lib/diff/rows.test.ts (`starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`N and P skip a block the whitespace policy ignores`)

### Requirement: The sidebar and the diffs: The diff search reads the model

The diff search MUST remain model-based and find what the rows of the layout show: both sides of a change in Split, in
one column a deleted row by its old line and the row of a similar pair of Inline by both its lines, the new side of an
unchanged line, and with the checkbox folded diff lines too. A text Unified or Split finds, Inline finds too, the old
lines of a quiet run at their note, but for the whitespace of an old line that a whitespace policy ignores: the row
of a pair shows the text its lines share as the new line has it.

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

#### Scenario: A pair of another indent under a whitespace policy
- **GIVEN** Trim Whitespace or Ignore Whitespace pairs a line with the line indented again and edited
- **WHEN** the query is in the old line, from its unchanged text into its deleted part
- **THEN** the hit marks that text where the new line has it, not its indent, and the struck-through part
- **AND** the hits of the row go in the order of its text
  [@test] src/lib/diff/find.test.ts (`finds in an Inline row of a whitespace policy the old line by its own text, in the order of the text of the row`)
  [@test] src/lib/diff/line-parts.test.ts (`marks a hit of the old line where the new line has the shared text, when a whitespace policy pairs lines of another indent`, `places an offset of the old line by the text the lines share, whatever whitespace a policy ignored in it`, `places every character of both lines of random pairs, and marks every hit of the old line as its text`)
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should mark a hit of the old line of an Inline row at its own text where Trim Whitespace pairs lines of another indent`)

#### Scenario: A block the engine does not pair
- **GIVEN** a call wrapped again that lost an argument, which Inline shows line by line
- **WHEN** the query is the lost argument
- **THEN** it is found in its deleted row, as Unified finds it
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should show a call wrapped again line by line in Inline, and find the argument it lost`)

#### Scenario: The old lines of a quiet run
- **WHEN** the query is in an old line of a quiet run of the marks, which Inline shows once by its new lines
- **THEN** it matches there, in the old line, and a step lands on the note that stands for the line ([Quiet Changes](../quiet-changes/spec.md#requirement-views-the-find-searches-the-old-lines-a-note-stands-for))
  [@test] src/lib/diff/find.test.ts (`searches the old lines that Inline shows once, at the note that stands for them, before the new lines`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the find searches the old line a note stands for and lands on the note; opened, the hit is in the line`)

#### Scenario: A line of very many hits
- **WHEN** a line holds more hits than a call takes arguments (a minified file, one letter typed)
- **THEN** every hit is found and the line is cut into its parts
  [@test] src/lib/diff/find.test.ts (`finds in a line more hits than a call takes arguments`)
  [@test] src/lib/diff/line-parts.test.ts (`cuts a line with more hits than a call takes arguments`)
