## ADDED Requirements

### Requirement: Views: A block shows part by part, a note before the lines of each run

A view MUST show a block in the order of both its sides, cut at its runs: the lines between two runs as the block
shows without marks, and each run after a row of words, its note, which says what the lines changed in and how many
they are ("Formatting only · 3 lines"). A run that lies in no block MUST change no row.

#### Scenario: A block of several parts
- **WHEN** a block adds a line, indents three lines again and adds another line, or has two runs with a changed line between them
- **THEN** each layout shows the added line, the note of the run, the lines of the run, then the lines after it
- **AND** the lines between two runs are rows of the block, each run has its own note
  [@test] src/lib/diff/rows.test.ts (`cuts a block at its runs in the order of both sides: the lines between them show as the block does, a run after its note`)
  [@test] src/lib/diff/block-parts.test.ts (`is one part of lines for a block without a run, in the shape of its layout`, `counts the rows of each part: a note a row, a quiet run of Inline its new lines until its note is opened`, `gives each row by its part, and the row of each line of a side`, `says the lines its rows cover, the lines a note stands for with them`)

#### Scenario: The pairs of Inline
- **WHEN** a quiet run lies between similar pairs of a block, or moves the lines after it apart on the two sides
- **THEN** Inline shows a pair as one row only where the engine pairs line i of both sides, and the other lines as Unified does
  [@test] src/lib/diff/rows.test.ts (`pairs the lines of a part of Inline only where the engine pairs line i of both sides`)
  [@test] src/lib/diff/block-parts.test.ts (`lists the pairs of lines that are one row each in Inline`)

#### Scenario: A run in no block
- **WHEN** the marks hold a run before every block, one that reaches past its block, or one after the last block
- **THEN** no row changes: every line of every block shows
  [@test] src/lib/diff/rows.test.ts (`leaves a run that lies in no block out, so every line of every block shows`)
  [@test] src/lib/diff/block-parts.test.ts (`gives each block its runs, and none to a run that lies in no block`)

#### Scenario: The words of a note
- **WHEN** a note words a run of formatting, of comments or of a renamed name
- **THEN** it says the kind ("Formatting only", "Comments only", "Renamed only") and the new lines of the run ("3 lines", "1 line")
  [@test] src/presenters/diff-marks.test.ts (`says what the lines of a run changed in and how far it reaches, by the new lines`)

### Requirement: Views: Inline shows a quiet run once, by its new lines

Inline MUST show the lines of a quiet run once: its new lines alone, after its note, in the quiet colour, neither
inserted nor unchanged, with no changed part in them. Its old lines have no row.

#### Scenario: A wrapped call
- **WHEN** Inline shows the call of the `commit_marks` template that the commit wraps over three lines
- **THEN** the three new lines show once after the note "Formatting only · 3 lines", quiet, with their new line numbers alone
- **AND** no row shows the old line, and no part of the three lines is marked as changed
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)
  [@test] src/lib/diff/rows.test.ts (`shows a quiet run of Inline once, by its new lines after a note, and with its old lines once the note is opened`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line`)

### Requirement: Views: The note of a quiet run of Inline opens its old lines

The note of a quiet run of Inline MUST be a button: a click shows the old lines of the run before its new ones, all
of them quiet, and a second click hides them. ⌥-click (Alt-click) MUST open every such note of the file, on an open
note close them all. The diffs keep the focus, and a view keeps what it opened while the diff is kept, as the levels
of its folds.

#### Scenario: A click
- **WHEN** the user clicks the note of the wrapped call, then clicks it again
- **THEN** the old line shows before the three new ones, each of its type and quiet, and the note says it is expanded
- **AND** the second click hides the old line again
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)
  [@test] src/presenters/diff-marks.test.ts (`names the button of a note that opens by what a press does to the old lines`)

#### Scenario: ⌥-click
- **GIVEN** a file with two wrapped calls, each a quiet run
- **WHEN** the user clicks the first note, ⌥-clicks the second, then ⌥-clicks the first
- **THEN** the click opens its note alone, the diffs keeping the focus
- **AND** the first ⌥-click opens every note of the file, the second closes them all
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a click on a note opens it and ⌥-click opens and closes every note of its file`)

#### Scenario: Kept with the diff
- **WHEN** the notes a view opened are stored for its diff
- **THEN** they are read back by the diff, beside the levels of its folds
  [@test] src/lib/diff/rows.test.ts (`keeps the open notes of a diff beside the levels of its folds`)

#### Scenario: Kept as the scopes of the file come
- **GIVEN** a note opened, and a fold and a note of a section of a review, before the scopes of a text the caches lacked came
- **WHEN** the scopes come and the store replaces the content of the file
- **THEN** the note is as open as it was, and the section keeps its key, its fold and its note
  [@test] src/stores/file-diffs.test.ts (`keeps the notes a view opened across the late scopes of a text, and the folds and the notes of its sections`)

### Requirement: Views: Unified and Split keep every line of a quiet run, played down

Unified and Split MUST show every line of a quiet run, each of its type, in the quiet colour and with no changed
part; Split fills the shorter side of the run. Their note opens nothing.

#### Scenario: The wrapped call in Unified and Split
- **WHEN** the user switches the preview to Unified, then to Split
- **THEN** Unified shows the old line and the three new ones, deleted and inserted, all quiet; a block to read keeps its colour
- **AND** Split shows the old line beside the first new one and fills the old side beside the others
- **AND** the note is plain words in both
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Unified and Split keep every line of a wrapped call, played down, after a note that opens nothing`)
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a quiet run in Unified and Split, played down, after a note that opens nothing`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line`)

### Requirement: Views: A run that is a label has a note and is never played down

The lines of a run that is not quiet MUST show as the lines of their block, with its tints and its changed parts,
after a note that opens nothing: no layout hides a line of it.

#### Scenario: A comment worded again
- **WHEN** a view shows the comment of the `commit_marks` template that the commit words again
- **THEN** the note "Comments only · 1 line" is plain words, no button
- **AND** the comment is a similar pair of Inline, pale, its old words struck through before the new ones, never quiet
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a run of comments is a label: its note opens nothing, its lines show as any change`)
  [@test] src/lib/diff/rows.test.ts (`labels a run of comments with a note and plays nothing of it down`)

### Requirement: Views: The words of a run that starts its block join the heading of the change

In a section of a review, a run that starts a block with a heading MUST say its words in the row of that heading,
after it, and open from there: no second row of words stands under the heading. A run after the first lines of the
block keeps its own note.

#### Scenario: A heading and a run
- **WHEN** a block with a heading starts with a quiet run, or holds one after its first line
- **THEN** the first has one row of words, the heading then the words of the run, which opens the old lines
- **AND** the second has the heading, the first line, then the note of the run
  [@test] src/lib/diff/rows.test.ts (`says the words of a run that starts its block in the heading of the change, one row of words`)

### Requirement: Views: An old line shown once is found at the note that stands for it

An old line of a quiet run that Inline shows once MUST resolve to the note of its run: the scroll position keeps a
note at the top by that line, and a reveal of a name in such a line opens the note, as it expands a fold, and selects
the name in the line. A row of words that stands for no line MUST stay at the top across a new layout of the same
rows.

#### Scenario: The row of an old line
- **WHEN** the row of an old line of a quiet run is asked, the note closed, opened, and in Unified
- **THEN** it is the note while Inline shows the run once, and the row of the line where it has one
  [@test] src/lib/diff/rows.test.ts (`gives the note that stands for an old line shown once, and the row of the line once the note is open`)

#### Scenario: The scroll position
- **WHEN** a note is at the top of the view and the rows are laid out again, the note opened, or in Unified
- **THEN** the top stays at the old line the note stood for
  [@test] src/lib/diff/layout.test.ts (`keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each`)

#### Scenario: A row of words that stands for no line
- **WHEN** the note of a run whose lines all show (Unified, Split, an opened note, a label) or the heading of a change is at the top of the view, and the same rows are laid out again
- **THEN** the top stays where it was: the row is held by the line of the row after it
- **AND** in a layout where the note stands for that line, or is gone, the top is the row of the line, never the row above it
  [@test] src/lib/diff/layout.test.ts (`keeps a row of words that stands for no line at the top across a new layout, by the row after it: a note of Unified and Split, an opened note, a label, a heading`)

#### Scenario: Back to a name of a hidden line
- **GIVEN** a jump from a name of the old line of the wrapped call, its note opened, then closed again
- **WHEN** the user goes Back
- **THEN** the note opens and the name is selected in the old line
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Back to a name of an old line that a closed note stands for opens the note and selects the name in the line`)

### Requirement: Views: The find searches the old lines a note stands for

The find MUST search the old lines of a quiet run that Inline shows once, as hits of the old side before the hits of
its new lines. A step to such a hit lands on the note, which names the hits it holds ("1 match in the old line") and
marks the current one. Once the note is opened the same hits are in the old lines.

#### Scenario: The hits of a hidden line
- **WHEN** the query is in the old line of a quiet run, in Inline, the note closed and opened, and in Unified and Split
- **THEN** the hit is of the old side, before the hits of the new lines, at the row of the note while it stands for the line
  [@test] src/lib/diff/find.test.ts (`searches the old lines that Inline shows once, at the note that stands for them, before the new lines`, `finds the same hits by the rows of the old lines once the note is opened, and where every line stays`)
  [@test] src/presenters/diff-marks.test.ts (`names the hits in the old lines a note stands for`)

#### Scenario: A step lands on the note
- **GIVEN** a file of two wrapped calls below the first screen of the review
- **WHEN** the user finds a text only the old line of the first call holds, presses Enter, then opens the note
- **THEN** the step scrolls the view to the note, which names "1 match in the old line", marked as the current hit
- **AND** opened, the hit is marked in the old line, still the current one, and the note names none
- **AND** a text of both forms of the call is found at the note and in the new line
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the find searches the old line a note stands for and lands on the note; opened, the hit is in the line`)

### Requirement: Views: A copy takes the lines the view shows

A copy MUST give the lines the rows show: of a quiet run of Inline its new lines, its old lines too once its note is
opened and where every line stays. A note gives no text.

#### Scenario: A selection over a quiet run
- **WHEN** the user selects from the note of the wrapped call to the end of its last line and copies, the note closed, then opened
- **THEN** the copy is the three new lines, then the old line and the three new ones
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a copy takes what a quiet run shows: its new lines, and its old line once the note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a quiet run shows: its new lines alone in Inline, its old lines too once its note is opened or where every line stays`)

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"; a line
shown once "formatting only"), as a changed line says its type. A note that opens MUST be a button named by its words
and by what a press does, with its expanded state.

#### Scenario: The wrapped call
- **WHEN** Inline shows the wrapped call
- **THEN** its lines carry the kind of their run, which the generated content of the line reads out
- **AND** the note is the button "Formatting only, 3 lines: Show the old line", not expanded; opened, "… Hide the old line", expanded
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)
