## ADDED Requirements

### Requirement: Views: A note at each end of a move says where its other end is

A view MUST show a note before the lines of a move at each of its ends: where they left, how many lines moved and to
which line ("14 lines moved to line 210"); where they stand now, from which line, whether they were indented again
and how many were edited on the way ("Moved from line 120 · re-indented · 1 line edited"). The line is the first line
of the other end, the blank lines that moved with the move among them. A move with an end in no block MUST change no
row.

#### Scenario: The words
- **WHEN** a note words where fourteen lines left, where they stand now, and a move that was indented again with two lines edited
- **THEN** it says "14 lines moved" "to line 210" and "Moved" "from line 120", then "re-indented · 2 lines edited", nothing where the lines moved as they were
- **AND** the line is the first line of the other end as a reader counts, a blank line that moved with the lines among them
  [@test] src/presenters/diff-marks.test.ts (`says where the lines that left stand now, by the first line of that end, and what its buttons do`, `says where the lines that stand now were, whether they were indented again and how many of them were edited on the way`)

#### Scenario: The moves of the template
- **WHEN** the preview of the `commit_marks` template shows Ledger.kt, whose `accounts()` moved up in its class as it is and whose `describe` moved into the class, indented, a space added in one line
- **THEN** the notes where they stand now say "Moved from line 22" and "Moved from line 46 · re-indented · 1 line edited"
- **AND** the notes where they left say "5 lines moved to line 6" and "7 lines moved to line 19"
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`)

#### Scenario: The ends of a block
- **WHEN** the marks hold a move, one with blank lines beside it, two whose old lines are in another order than their new ones, and one with an end in no block or in a block the policy ignores
- **THEN** each block has the ends of its moves in the order of its lines, the blank lines among the lines of an end and never out of the block
- **AND** the move with an end in no block changes no row: every line of every block shows
  [@test] src/lib/diff/block-parts.test.ts (`gives each block the ends of its moves, the blank lines that moved with them among their lines, and none of a move with an end in no block`)
  [@test] src/lib/diff/rows.test.ts (`says the words of an end of a move that starts its block in the heading of the change, and leaves a move with an end in no block out`)

### Requirement: Views: Inline shows the lines of a move once, where they stand now

Inline MUST show the lines of a move once: where they stand now, after their note, in the colour of a move, an edited
line as the row of its old line and its new one with the ranges of the move. Where they left it MUST show the note
alone, which stands for the old lines and opens as the note of a quiet run does: a click, ⌥-click and H show them
after it, in the colour of a move.

#### Scenario: The moves of the template
- **WHEN** Inline shows Ledger.kt of the `commit_marks` template
- **THEN** the lines of `accounts()` and of `describe` show once after their notes, new lines of the move with their new numbers alone
- **AND** the edited line of `describe` is one row with both its numbers, pale, the space it has more in the full colour and nothing struck through
- **AND** where the lines left a note stands alone, no row of them
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`)
  [@test] src/lib/diff/rows.test.ts (`shows the lines of a move once in Inline, where they stand now, and one note that opens where they left`)
  [@test] src/lib/diff/block-parts.test.ts (`shows the lines of a move once where they stand now, after a note, an edited one as the pair of its two lines in Inline`)

#### Scenario: The note where the lines left opens
- **WHEN** the user clicks the note where `accounts()` left, then clicks it again; and in a review window presses H with the note where `describe` left in view, clicks it, presses H twice, then ⌥-clicks it, and ⌥-clicks the note where `accounts()` left
- **THEN** the click shows the blank line and the four lines after the note, deleted rows in the colour of a move, and the second click hides them
- **AND** H shows the seven old lines of `describe`, the edited one pale, the click hides them, H shows and hides them again, the diffs keeping the focus
- **AND** ⌥-click opens every note of the file that opens, the notes where both functions left, and no note where lines stand now; ⌥-click on an open one closes them all
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`, `H and a click show the lines a move took after the note that stands for them, and hide them again`)
  [@test] src/lib/diff/block-parts.test.ts (`stands a note for the lines that left in Inline until it is opened, and shows them after it elsewhere`)

#### Scenario: The cells of a move
- **WHEN** the cells of a move are drawn, a line as it was and an edited one, at both ends, in a block with ranges of its own
- **THEN** each is a cell of a move, full as it was and pale where it was edited, with the ranges of the move and never those of its block
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move as its own cell at both ends in every layout, and draws the ranges of the move in an edited line alone`)

### Requirement: Views: Unified and Split keep every line of a move at both ends

Unified and Split MUST show every line of a move at both ends, in the colour of a move, an edited line pale with the
ranges of its side of the move, after notes that open nothing. Split MUST show the lines that left on the old side
and the lines that stand now on the new side, the other side filled, each note over the column of its lines.

#### Scenario: The moves of the template in Unified and Split
- **WHEN** the user switches the preview of Ledger.kt to Unified, then to Split
- **THEN** Unified shows the lines of both moves as inserted rows of the move where they stand and as deleted rows of the move where they left, the edited line pale at both ends; the notes are plain words
- **AND** Split shows the lines that stand now on the new side and the lines that left on the old side, the other side filled beside them, each note over the column of its lines
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Unified and Split keep every line of a move at both ends, in the colour of a move, after notes that open nothing`)
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a move in Unified and Split, both ends rows of the move after a note that opens nothing`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move as its own cell at both ends in every layout, and draws the ranges of the move in an edited line alone`)

### Requirement: Views: The lines of no move beside a move show as the block does

A view MUST cut the lines of a block between its runs at the ends of its moves, each end on its own side, and show
the lines of no move as the block shows without marks. In lines the engine pairs, a cut of one side MUST cut the
other at the same line, so the lines beside a move keep their pairs. Every line of a block MUST show once whatever
the marks say: by its own row, or behind the note that stands for it.

#### Scenario: A block that lost some of its lines
- **WHEN** a block changes a line, loses three lines to a move and deletes two more
- **THEN** each layout shows the changed line as the block does, the note of the move (with its lines where the layout keeps them), then the two deleted lines
  [@test] src/lib/diff/block-parts.test.ts (`cuts the lines of no run at the ends of the moves, each on its own side, and reads a block from its first line no move took`)

#### Scenario: Lines the engine pairs
- **WHEN** a block of four similar pairs loses its third old line to a move, or a line of a move stands at its third new line
- **THEN** Inline shows the pairs before and after as a row each, and the line whose pair moved alone
  [@test] src/lib/diff/block-parts.test.ts (`keeps the pairs of the lines beside a move in a block the engine pairs, and shows the line whose pair moved alone`)

#### Scenario: Any marks
- **WHEN** blocks get runs and moves at random, valid or not: overlapping each other, reaching past their block
- **THEN** in every layout, the notes closed and opened, each line of the block shows once: by its own row, or behind the note that stands for it
  [@test] src/lib/diff/block-parts.test.ts (`shows every line of a block once whatever the marks say: by its own row, or behind the note that stands for it`)

### Requirement: Views: A move with an end in a block that is played down is drawn at neither end

A view MUST draw a move at both its ends or at neither. A block of the imports alone that a review with its plan
plays down MUST draw no end of a move: its lines show as they did, pale, every one of them, without a note, and it is
no stop. At the other end of such a move, in a block that is not played down, the lines of the move MUST be lines of
their block, each with its row in every layout, with no note: none is hidden, and an edited one shows what it was.

#### Scenario: Imports that were sorted
- **WHEN** an import moves within the blocks of the imports alone, in a review with its plan and outside one
- **THEN** in the review both blocks show their lines pale, none left out, with no note of a move and no stop; outside it the move shows with its notes and its stop
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)

#### Scenario: An import that moves into a block with code
- **WHEN** an import leaves a block of the imports alone for a block with code beside it, in a review with its plan, in Inline, Unified and Split
- **THEN** the block that is played down shows its line pale, and where the line stands now it is an inserted line of its block, with no note
- **AND** the block with code is read from its first row, as a block without a move
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)

#### Scenario: Imports that leave a block with code, one edited on the way
- **WHEN** four imports leave a block with code for a block of the imports alone, the second edited on the way, in a review with its plan, in Inline, Unified and Split
- **THEN** the block they left shows them as its deleted lines, each with its row, the old line of the edited import among them: no note stands for a line
- **AND** where they stand now they are pale inserted lines, with no note, no row of two lines and no stop; the one stop is the first row of the block with code
- **AND** a move between two blocks with code in the same review shows with its notes
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)
  [@test] src/lib/diff/block-parts.test.ts (`leaves a move with an end in a block a view plays down out of both its blocks, and no other move`)

#### Scenario: A review with its plan
- **GIVEN** a commit that puts four imports of a file in order, two of them moves to the engine, and changes a line of its body
- **WHEN** the review with its plan shows the section of the imports, opened
- **THEN** the imports that left and where they stand now are deleted and inserted lines, pale, none in the colour of a move, and no note of a move stands in the review
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`imports that were sorted stay the pale lines of a block the review plays down, with no note of a move`)

#### Scenario: Imports that left the block of a comment, in a review with its plan
- **GIVEN** a commit that puts four imports before the others, one of them from another package now, and deletes the comment that stood before them
- **WHEN** the review with its plan shows the section of the file and the section of the imports, opened
- **THEN** the comment and the four imports as they were are deleted lines of the section of the file, every one shown
- **AND** the four imports as they are now are pale inserted lines of the section of the imports; no note of a move, no line in its colour and no row of two lines stands in the review
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`imports that left a block with code for the imports, one edited on the way, stay the deleted lines of their block, none behind a note of a move`)

### Requirement: Views: The place a note of a move names shows its other end

The place a note of a move names ("to line 210", "from line 120") MUST be a button: a click shows the note at the
other end a third of the view below its top, marked as where the view landed, its file open. In a review window the
end the jump started from MUST be kept for Back, and Forward goes there again, the diffs holding the focus; the
preview of a commit keeps no history and moves no focus.

#### Scenario: A click in the preview
- **WHEN** the user clicks "from line 22" of the note where `accounts()` stands now, then "to line 6" of the note the view went to
- **THEN** the first click shows the note where the lines left, marked, and the second the note where they stand now, the first one marked no more
- **AND** in Unified and in Split, where the lines stay at both ends, the same clicks go there and back
- **AND** the buttons are named by where they go ("Go to line 22, where the lines were", "Go to line 6, where the lines stand now") and name no key
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

#### Scenario: A click, Back and Forward in a review window
- **WHEN** the user clicks the place of the note where `accounts()` stands now, presses Back, Forward, clicks the place of the note there, and presses Back
- **THEN** the click shows the note where the lines left, marked, the diffs holding the focus; Back returns to the note where they stand, Forward goes to the other again
- **AND** the click there is a jump of its own: Back returns from it to where the lines left
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note names jumps to the other end of the move, Back returns and Forward goes there again`)

#### Scenario: In a review with its plan
- **GIVEN** a commit that moves a function to the start of its class with one line edited, and one that moves it as it is, each in a review with its plan
- **WHEN** the user clicks the place in the heading of the change of the first, then presses G twice; and in the second expands the folded section of the moves, clicks the place of a note, presses G and Back, folds the section again and presses Forward
- **THEN** the heading says the words of the move after the change ("compute — body changed · Moved from line 121 · 1 line edited") and names the key; the click shows the note where the lines left, marked, G returns to the row of the heading, and G goes there again
- **AND** in the section of the moves the jump goes there and back, and Forward to a place in the section folded again opens it
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a move with an edited line says its words in the heading of the change: a click on its place and G go to where the lines left and back to the heading`, `a move as it was is in the folded section of the moves: a jump in it goes there and back, and Forward opens the section folded again`)

#### Scenario: The row a jump lands on
- **WHEN** the row of the first line of an end of a move is asked for a jump, the note where the lines left closed and opened, and of any other line; and of the line a note names where blank lines of the move reach out of the block of its other end
- **THEN** it is the note of that end: the row of words right above its first line, or the note that stands for the line; any other line is its own row
- **AND** the line a note names is the first line the other end shows in its block, so the jump lands on the note there in every layout
  [@test] src/lib/diff/rows.test.ts (`finds an old line of a move at the note that stands for it, and lands a jump to an end on its note`, `lands a jump on the note of the other end by the line that end has, also where blank lines of the move reach out of its block`)
  [@test] src/lib/diff/block-parts.test.ts (`gives each block the ends of its moves, the blank lines that moved with them among their lines, and none of a move with an end in no block`)

### Requirement: Views: G shows the other end of a move in view

In a review window G MUST show the other end of a move from one note of an end in view: the note of the end the view
stands at while it is in view (where a jump, Back or Forward landed, or the note N or P stopped under), else the first
one, so two presses go there and back. Only the button of the place of that note MUST name the key, the key MUST move
nothing with no such note in view, and it MUST be the key of no action of the window.

#### Scenario: The key from the tree and from the diffs
- **GIVEN** the review of the `commit_marks` template at the top of Ledger.kt, the focus on its row of the tree
- **WHEN** the user presses G twice, then Back, then shows Report.kt and presses G and N
- **THEN** the first press shows the note where `accounts()` left, marked, and gives the diffs the focus; the second returns to the note where it stands now; Back returns along the second jump
- **AND** in Report.kt, which has no note of a move, G moves nothing: N goes on from the top of the file
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`G jumps from the note of a move in view to its other end and back, from the tree of the files and from the diffs`)

#### Scenario: The hint of the key
- **WHEN** the notes of two moves show in a review window, the user clicks the place of one, and in the preview of a commit
- **THEN** in the review one button of a place names the key G: of the first note of a move in view, then of the note the click landed on; in the preview none does
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note names jumps to the other end of the move, Back returns and Forward goes there again`, `a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

#### Scenario: One note at a time
- **GIVEN** the review of a commit that moves two lines of a file up, each to a place of its own, so both ends of both moves are in view at once
- **WHEN** the user presses N twice, G twice, clicks the place of the first move, presses Back, then G
- **THEN** one button names the key at a time: of the first note of a move in view, then of the note N stopped under though the first is still in view above it, then of each note a jump or Back landed on
- **AND** G goes from the note that names it: from the note N stopped under to where its line left and back, and after Back from the note Back returned to, never from the one the click landed on
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`one note of a move names the key G at a time: the first one in view, the one N stopped under, the one a jump or Back landed on; the menu names the key on that end alone`)

#### Scenario: The note the key takes
- **WHEN** the note of the key is asked of a layout of two files with notes of moves: the view at the top, scrolled, and standing at an end whose note is in view or out of it
- **THEN** it is the note at the end the view stands at while its row is whole below the stuck header and above the end of the view, else the first note of a move in view, of the next file where the first has none; none with no note in view
  [@test] src/lib/diff/layout.test.ts (`gives the key of the moves one note: the one at the end the view stands at while it is in view, else the first note of a move in view`)

#### Scenario: No key of an action
- **WHEN** the keys of the review are compared with the keys that the actions of a branch and of a pull request name in a review window
- **THEN** G is the key of none of them, and the review answers it by its physical key
  [@test] src/lib/review-keys.test.ts (`takes no letter that an action of a window of the review names as its key: O stays the pull request's`, `answers the action of each of its keys by the physical key, and none for another key`)

#### Scenario: The notes of the moves of a file
- **WHEN** the rows of a file with a move are built in each layout
- **THEN** the notes at both ends are listed with their rows, in the order of the rows
  [@test] src/lib/diff/rows.test.ts (`stops N and P where the lines stand now and never where they left, and lists the notes of both ends`)

### Requirement: Views: The menu of the code on a line of a move shows its other end

The menu of the code MUST have the item "Go to Other End of Move" on a line of a move and on the note at its end,
which shows the other end as a click on the place of the note does; opened by a menu key it is on the line the
selection starts in. The item MUST show the key of the review only where the key takes the note of that end, and on
any other row the menu MUST have no such item.

#### Scenario: The item on a line and on a note
- **WHEN** the user opens the menu of the code on a line of `accounts()` where it stands now and chooses the item, then on the note where the lines left, then on a line of `describe`, the other move in view, then on an unchanged line
- **THEN** the first has the item with the key G and shows the note where the lines left, marked, the second the note where they stand now
- **AND** the item on the line of `describe` names no key: the key takes the note the view landed on
- **AND** the menu of the unchanged line has "Go to Declaration" and no such item
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the menu of the code on a line of a move and on the note at its end shows the other end`)

#### Scenario: A menu key
- **WHEN** the user selects a word of a line of `accounts()` where it stands now, presses Shift+F10 and chooses the item, then presses Shift+F10 again
- **THEN** the menu opens with the item and its key, which shows the note where the lines left, marked, the diffs holding the focus
- **AND** the jump selected nothing, so the second menu has "Go to Declaration" and no such item, and the note the view landed on names the key
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the menu of the code a key opens on a line of a move shows the other end, and has no such item once the jump left nothing selected`)

#### Scenario: The key of the item
- **GIVEN** the review of a commit with two moves, every note of them in view
- **WHEN** the user opens the menu on a line of each move, before and after N stops under the note of the second
- **THEN** the item names the key on the line of the move whose note takes the key, and none on the line of the other
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`one note of a move names the key G at a time: the first one in view, the one N stopped under, the one a jump or Back landed on; the menu names the key on that end alone`)

#### Scenario: The item
- **WHEN** the items of the menu are built on a row of no move, on an end of a move with the key, and on one without it
- **THEN** the first has no such item; the others have it, with the key only where one is given, and say where it goes from that end
  [@test] src/lib/diff/code-menu.test.ts (`offers the other end of a move on a line of a move or the note at its end, with the key of the view where it has one`)
  [@test] src/presenters/diff-marks.test.ts (`names the action that shows the other end of a move, and says where it goes from each end`)

### Requirement: Views: The colour of a move is two tokens of both themes

A line of a move and the note at each of its ends MUST be drawn in the colour of the token `--diff-moved` of each
theme, a line edited on the way in `--diff-moved-pale`: the colour of no other line of a diff. A rail down the start
of the lines of a move and of its notes, which no other line has, MUST mark a move without its colour.

#### Scenario: The light and the dark theme
- **WHEN** the moves of the template show in the light theme, and in the dark one
- **THEN** their lines and their notes have the colour `--diff-moved` names, the edited line the one `--diff-moved-pale` names, light in the light theme and dark in the dark one
- **AND** no token of an inserted, a deleted, a modified or a quiet line, pale or full, names either colour
- **AND** the lines and the notes of a move have the rail; an inserted line, an unchanged one and the note of a quiet run have none
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the lines of a move and the notes at its ends have the moved colour of the light theme, which is the colour of no other line`, `the lines of a move and the notes at its ends have the moved colour of the dark theme, which is the colour of no other line`)
