## MODIFIED Requirements

### Requirement: Views: N and P stop where the reading of a block starts

N and P MUST stop only at the starts of a diff: at the first row of lines of what is read from the line `starts`
names. Rust cuts a block by its quiet runs alone, so lines deleted before a label are read from the first of them,
and one column shows the old lines of a part first; the lines a move took end what is read before them. A block
without a start is passed, and so are a block of the imports alone in a review with its plan and a block another
section shows.

#### Scenario: The stops of the template
- **WHEN** the user presses N through the review of the `commit_marks` template, then P
- **THEN** N stops in Ledger.kt as many times as its header says is left to read, fewer than its eleven blocks, which hold the wrapped call, then three times in Report.kt and once in Store.kt
- **AND** each stop is a changed line four rows below the header of its file, after a row that is none, further down than the stop before it; no line of the wrapped call is one
- **AND** `accounts()` and `describe`, which moved, are stops where they stand now, each at its first line under the note of its move, and no stop where they left
- **AND** P goes back through the same stops, each at the row N showed
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`)

#### Scenario: A file of quiet runs alone
- **GIVEN** a file whose two changes are wrapped calls, between a file changed before it and one after it
- **WHEN** the user presses N, then P
- **THEN** N goes from the first file to the third, and P back, past the file
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: The row of a stop
- **WHEN** a block starts at an added line before a quiet run, at a deleted line after one, at a run of comments, or is quiet as a whole
- **THEN** the stop is the first row of lines since the last quiet run before that line: an old line where one column shows the old lines of the part first
- **AND** a block of quiet lines alone has no stop, and a start that names no line of its block stops at the last row of the block
  [@test] src/lib/diff/rows.test.ts (`stops N and P where Rust starts the reading of a block, at the first row of that part, never at a block of quiet lines alone`, `starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`, `plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any`)
  [@test] src/lib/diff/block-parts.test.ts (`reads the stops as Rust gives them, and counts them against the blocks of changed lines`, `gives each row by its part, and the row of each line of a side`)

#### Scenario: Lines deleted before a label
- **GIVEN** a block that deletes six lines and words the comment of the line after them again, a run of comments, so Rust names the comment, the first new line of the block
- **WHEN** the rows are built in each layout, and the user presses N in the review
- **THEN** the stop is the first deleted line, where the block stopped before it had a label, in Inline, Unified and Split, also under a heading and after a quiet run
- **AND** N shows the first deleted line four rows below the header of its file, the deletion in view from its start
  [@test] src/lib/diff/rows.test.ts (`stops N and P where Rust starts the reading of a block, at the first row of that part, never at a block of quiet lines alone`)
  [@test] src/lib/diff/block-parts.test.ts (`starts the reading of a block at its first row of lines to read, in every layout, whatever labels cut it`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N stops at the first line a block deletes before a comment worded again, never at the comment below them`)

#### Scenario: A file without a grammar
- **WHEN** a file no grammar reads indents two lines again, between a file changed before it and one after it
- **THEN** its lines are pale with the indentation in the full colour, without a note or a quiet line, and N stops at them
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`stays the pale stop it was: no note, no quiet line, and N stops at it`)

#### Scenario: The lines of a move
- **WHEN** a block is the lines of a move where they stand now, a block lost every old line to a move, and a block lost some of its lines, in each layout
- **THEN** the first stops at the first line of the move, under its note; the second has no stop; the third stops at a line that stayed, one column at the old line of that part first
- **AND** a block that starts with lines a move took is read from the line after them, the lines shown or behind their note
  [@test] src/lib/diff/rows.test.ts (`stops N and P where the lines stand now and never where they left, and lists the notes of both ends`)
  [@test] src/lib/diff/block-parts.test.ts (`cuts the lines of no run at the ends of the moves, each on its own side, and reads a block from its first line no move took`)

### Requirement: Views: The find searches the old lines a note stands for

The find MUST search the old lines a note of Inline stands for (of a quiet run shown once, before the hits of its new
lines; of a move, where its lines left), as hits of the old side. A step to such a hit lands on the note, which names the hits it holds ("1 match in the old line") and
marks the current one; once the note is opened the same hits are in the old lines. The row of an edited line of a
move MUST be searched by its new line alone: a hit of its old line is one hit, where the line left.

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

#### Scenario: The lines a move took
- **WHEN** the query is in a line of a move, in Inline with the note where the lines left closed and opened, and in Unified and Split
- **THEN** the line is found where it stands now and, a hit of the old side, at the note that stands for it where it left, or in its own row once the note is opened and where every line stays
  [@test] src/lib/diff/find.test.ts (`searches the lines that left at the note that stands for them in Inline, after the lines where they stand now`, `finds the lines that left by their own rows once the note is opened, and where every line stays`)

#### Scenario: The word an edited line of a move lost
- **WHEN** the query is the word an edited line of a move lost, and text both its lines have, the note where the lines left closed and opened
- **THEN** the word is one hit, of the old line, at the row of the note, which counts one match, or at the row of the old line once the note is opened; never a second hit in the row of the two lines
- **AND** the text both lines have is found in the new line of the row and in the old line where it left, no hit twice
- **AND** the row of the two lines marks of its old line the struck-through parts alone, the current hit among them
  [@test] src/lib/diff/find.test.ts (`searches the lines that left at the note that stands for them in Inline, after the lines where they stand now`, `finds the lines that left by their own rows once the note is opened, and where every line stays`)
  [@test] src/lib/diff/line-parts.test.ts (`keeps of the hits of the old line of a pair its own alone, the current one among them, for the row of an edited line of a move`)

#### Scenario: A step to the word an edited line lost
- **GIVEN** a commit that moves a function from the end of its class to its start, a word of one line another, every function between the ends changed
- **WHEN** the user finds the old word in the review, presses Enter twice, then opens the note
- **THEN** the find counts one result, marked in the row of the two lines where the line stands now, struck through; the step scrolls to the note where the lines left, which names "1 match in the old lines", the current hit, and a second step stays there
- **AND** opened, the hit is marked in the old line, still the one result
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the find counts the word an edited line of a move lost once, marks it in the row of its two lines and lands on the note where the line left`)

#### Scenario: A step lands on the note of a move
- **WHEN** the user finds "fun accounts" in the review of the `commit_marks` template, presses Enter twice, then opens the note
- **THEN** the first step marks the line where it stands now, the second goes to the note where the lines left, which names "1 match in the old lines", the current hit
- **AND** opened, the hit is marked in the old line, still the current one, and the note names none
- **AND** "val text", which both lines of the edited line of `describe` have, is two results: the first step marks it in the row of the two lines, the second at the note where the lines left, the row marking it as a hit and no more as the current one
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the find searches the lines a move took at the note that stands for them and lands on it; opened, the hit is in the line`)

### Requirement: Views: A copy takes the lines the view shows

A copy MUST give the lines the rows show: of a quiet run of Inline its new lines, its old lines too once its note is
opened and where every line stays; of a move the lines once, where they stand now, and where they left too once
their note is opened and where every line stays. A note gives no text.

#### Scenario: A selection over a quiet run
- **WHEN** the user selects from the note of the wrapped call to the end of its last line and copies, the note closed, then opened
- **THEN** the copy is the three new lines, then the old line and the three new ones
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a copy takes what a quiet run shows: its new lines, and its old line once the note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a quiet run shows: its new lines alone in Inline, its old lines too once its note is opened or where every line stays`)

#### Scenario: The lines of a move
- **WHEN** the user copies the side of Ledger.kt of the `commit_marks` template in a review, the notes closed, then with the note where `accounts()` left opened
- **THEN** the first copy holds the first line of `accounts()` once and no old line of `describe`, and no words of a note
- **AND** the second holds the line twice, where it stands now and where it left
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a copy takes the lines of a move once, where they stand now, and the lines that left too once their note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a move shows: its lines once where they stand now in Inline, the lines that left too once their note is opened or where every line stays`)

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"; a line
shown once "formatting only"), as a changed line says its type. A note that opens MUST be a button named by its words
and by what a press does, with its expanded state. A line of a move MUST say which end of it the line is at and
that it was edited on the way ("moved away", "moved here, edited on the way"), and the place its note names is a
button named by where it goes.

#### Scenario: The wrapped call
- **WHEN** Inline shows the wrapped call
- **THEN** its lines carry the kind of their run, which the generated content of the line reads out
- **AND** the note is the button "Formatting only, 3 lines: Show the old line", not expanded; opened, "… Hide the old line", expanded
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)

#### Scenario: A move
- **WHEN** Inline shows the moves of the `commit_marks` template
- **THEN** their lines carry the end of the move they are at, which the generated content of the line reads out
- **AND** the note where `accounts()` left is the button "5 lines moved to line 6: Show the 5 old lines", not expanded; opened, "… Hide the 5 old lines", expanded
- **AND** the places are the buttons "Go to line 6, where the lines stand now" and "Go to line 22, where the lines were"
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`, `a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

### Requirement: Views: The rows of the marks keep the layout arithmetic

A note MUST be one row, and a line shown once as many visual lines as its new line takes, so every offset of the
view is known before a paint. The one column of Inline MUST scroll sideways no further for a pair that a quiet run
shows once, and as far as the row of an edited line of a move, the pair of its two lines, and as far as the pair of a
block in which a review with its plan draws no move.

#### Scenario: Heights
- **WHEN** a quiet run of Inline is laid out, with soft wraps off and on
- **THEN** its note and each of its lines are a row, a wrapped line as many visual lines as its new line
  [@test] src/lib/diff/layout.test.ts (`keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each`)

#### Scenario: The widest row of Inline
- **WHEN** a pair of a block lies in a quiet run
- **THEN** the widest row of Inline counts the row of the other pairs alone
  [@test] src/lib/diff/inline-rows.test.ts (`counts no row of a pair in a quiet run, which Inline shows once by its new line`)

#### Scenario: The rows of a move
- **WHEN** a move is laid out in Inline, with soft wraps off and on
- **THEN** the note where its lines left and the note where they stand now are a row each, a line of the move as many visual lines as its text takes
  [@test] src/lib/diff/layout.test.ts (`keeps the note of the lines a move took at the top by the old line it stands for, and the note of where they stand by the line after it; each a row`)

#### Scenario: The row of an edited line of a move
- **WHEN** a line of a move was edited on the way, and when it moved as it was
- **THEN** the widest row of Inline counts the first, its new line with the deleted part of its old line, and no row of the second
  [@test] src/lib/diff/inline-rows.test.ts (`counts the row of an edited line of a move, the pair of its old line and its new one where the line stands now`)
  [@test] src/lib/diff/block-parts.test.ts (`lists the edited lines of the moves that stand in a block as rows of two lines, with the ranges of their move`)

#### Scenario: The widest row of Inline in a review with its plan
- **WHEN** the engine pairs the two lines of a block of the imports alone and the marks call its old line a move, or the move stands in such a block
- **THEN** the widest row of Inline counts the row of the pair, which a review with its plan shows since it draws no move there, though outside a review the move takes the old line of the pair
  [@test] src/lib/diff/inline-rows.test.ts (`counts the pairs of a block of the imports alone as a review with its plan shows it too, which draws no move there`)

#### Scenario: The scroll of the column in a review with its plan
- **GIVEN** a commit that gives a long import a longer name in place and adds its old form again below, in a review with its plan, in Inline
- **WHEN** the section of the imports is opened and the scroll bar of its column is at its end
- **THEN** the row of the two lines of the import, wider than its column, shows its end
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the row of two lines of an import that a review plays down is reached by the scroll of its column, though its old line is a move to the engine`)

### Requirement: Views: An old line shown once is found at the note that stands for it

An old line of a quiet run that Inline shows once, and one a move took, MUST resolve to the note that stands for it: the scroll position keeps a
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

#### Scenario: A line a move took
- **WHEN** the row of an old line of a move is asked, the note where the lines left closed and opened, and of the old line of an edited line of the move
- **THEN** it is the note while it stands for the line, and the row of the line where it left once it has one, never the row of its pair where the lines stand now
- **AND** a note at the top of the view stays there by that line across a new layout, the note opened or in Unified; the note where the lines stand now by the first of them
  [@test] src/lib/diff/rows.test.ts (`finds an old line of a move at the note that stands for it, and lands a jump to an end on its note`)
  [@test] src/lib/diff/layout.test.ts (`keeps the note of the lines a move took at the top by the old line it stands for, and the note of where they stand by the line after it; each a row`)

### Requirement: Views: The words of a run that starts its block join the heading of the change

In a section of a review, a run or an end of a move that starts a block with a heading MUST say its words in the row of that heading,
after it, and open from there: no second row of words stands under the heading. A run after the first lines of the
block keeps its own note.

#### Scenario: A heading and a run
- **WHEN** a block with a heading starts with a quiet run, or holds one after its first line
- **THEN** the first has one row of words, the heading then the words of the run, which opens the old lines
- **AND** the second has the heading, the first line, then the note of the run
  [@test] src/lib/diff/rows.test.ts (`says the words of a run that starts its block in the heading of the change, one row of words`)

#### Scenario: A review with its plan
- **GIVEN** a function whose first statement is wrapped over three lines and whose result changes, one block, in a review with its plan
- **WHEN** the review shows the block, and the user clicks the words of the run
- **THEN** one row of words stands before it, "total — body changed · Formatting only · 3 lines"
- **AND** the click shows the old line of the statement, and no second row of words comes
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a run that starts its block says its words in the heading of the change, and opens its old line from there`)

#### Scenario: A heading and an end of a move
- **WHEN** a block with a heading is the lines a move took, in Inline and in Unified
- **THEN** one row of words stands before it, the heading then the words of the move, which opens the old lines in Inline; no second row of words comes
  [@test] src/lib/diff/rows.test.ts (`says the words of an end of a move that starts its block in the heading of the change, and leaves a move with an end in no block out`)
