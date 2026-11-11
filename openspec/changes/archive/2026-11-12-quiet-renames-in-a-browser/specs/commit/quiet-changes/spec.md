## ADDED Requirements

### Requirement: Views: A note pressed right after a scroll stays where it is

A note that opens MUST stay where it was pressed: the view keeps the row at its top as it stood at the press, also
when the press comes before the event of the scroll that brought the note into view.

#### Scenario: A press right after a scroll
- **GIVEN** the note of a rename that opens, below the first screen of a review, and the note of a wrapped call above it
- **WHEN** a scroll brings the note into view and ⌥-click on it follows at once, which opens the note above it too, and again to close them
- **THEN** the note stays in view where it was pressed, though the notes above it took rows and gave them back
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the note of a quiet rename shows the old line of every site by a click, by ⌥-click with the other notes of its file, and by H`)

## MODIFIED Requirements

### Requirement: Views: Inline shows a quiet run of a renamed name once, and the note of its rename opens it

Inline MUST show a quiet run of a renamed name once, by its new lines, played down, the name in its mark, with no
note of its own. The note of its rename MUST then be a button that shows the old lines of every such run of the
rename in the rows, and hides them again; ⌥-click and H take it as any note that opens, and a run with sites of two
notes opens with either. Unified and Split MUST keep both lines of each site, played down; their note opens nothing.

#### Scenario: A quiet rename
- **WHEN** the three runs of a renamed local are quiet, in Inline with the note closed and opened, in Unified and in Split
- **THEN** Inline shows each line once, quiet, with the name in its mark and no other range, and the one note before the declaration is the note that opens, with every old line it hides
- **AND** opened, every old line shows before its new one, in its block and in the others; Unified and Split show both lines of each site, quiet
  [@test] src/lib/diff/rows.test.ts (`shows the lines of a quiet rename once in Inline, which N and P pass, and opens them all by the one note of the rename`)
  [@test] src/lib/diff/block-parts.test.ts (`shows a quiet run of a rename once in Inline, without a note of its own, until the note of the rename is opened; Unified and Split keep both lines`)
  [@test] src/lib/diff/row-cells.test.ts (`draws the name on a line of a quiet run, which shows no other range: once in Inline, on both lines elsewhere and under an opened note`)

#### Scenario: The local of the template, its kind switched on
- **GIVEN** the answers of the engine for the `commit_marks` template with the kind `renamed` switched on: the three runs of `sum` quiet, their blocks without a start
- **WHEN** the preview shows Ledger.kt in Inline, the note is clicked twice, then in Unified and in Split
- **THEN** Inline shows each of the three lines once, quiet, `sum` in its mark and nothing struck through, under the one note, a button named "Renamed total to sum, 3 sites: Show the 3 old lines" that names no key
- **AND** the click shows the old line of each site before its new one, quiet, `total` in its mark, in its block and in the two others; the second click hides them
- **AND** Unified and Split show both lines of each site, quiet, the name in its mark on both, under a note of plain words
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline shows each line of a quiet rename once, the new name in its mark, and the one note of the rename shows and hides the old line of every site`, `Unified and Split keep both lines of each site of a quiet rename, played down, the name in its mark on both, under a note that opens nothing`)

#### Scenario: The note in a review
- **GIVEN** the review of the template with the kind `renamed` switched on
- **WHEN** the note of the rename is clicked twice, ⌥-clicked twice, and H is pressed twice with it in view
- **THEN** the note names the key H; each first press shows the three old lines, each second hides them, the diffs keeping the focus
- **AND** the ⌥-click opens the note of the wrapped call with it, and closes both
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the note of a quiet rename shows the old line of every site by a click, by ⌥-click with the other notes of its file, and by H`)

#### Scenario: The words of the button
- **WHEN** the note of a rename opens four old lines of three runs, or one line, and when every line of the rename is read
- **THEN** its button says "Show the 4 old lines", "Hide the 4 old lines", "Show the old line"; a note that opens nothing is no button
  [@test] src/presenters/diff-marks.test.ts (`names the button of a note that opens by the old lines of every quiet run of its renames, one line or many`)

#### Scenario: A run with sites of two notes
- **WHEN** a quiet run holds a site of a rename named in its own block and a site of one named in another block
- **THEN** either note, opened, shows the old lines of the run
  [@test] src/lib/diff/block-parts.test.ts (`names the renames whose notes stand in one block in one note, at the first of their sites, and any other in its own`)

#### Scenario: A rename whose runs are labels
- **WHEN** the engine gives the runs of the renamed local of the template as labels, as it does until their kind is switched on
- **THEN** no line of them is shown once or played down, and the note is plain words
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)
  [@test] src/lib/diff/block-parts.test.ts (`shows a run of a rename that is a label as the lines of its block, the note before the lines of the declaration alone`)

### Requirement: Views: The find for an old name lands on the new name that stands for it

Where Inline shows a quiet line of a renamed name once, the find MUST find the old name at that row: a hit of the old
line that holds a part of the old name, one hit at the first such name, in the order of the text of the row, marked
on the new name that stands for it. The other text of such an old line is found in the new line alone. Where the old
name shows (struck through in the row of a pair, in an old line), the hits are in it.

#### Scenario: The hits of the old name
- **WHEN** the query is the old name, the new name, text both lines have, text that reaches into the old name, and text over two old names of a line, in Inline with the runs labels and quiet
- **THEN** the old name is one hit a site, of the old line, at the row that shows its new line; the new name a hit of the new line; the text both lines have one hit, of the new line
- **AND** a hit that reaches into the old name is the old line's, and one over two names is one hit, at the first
  [@test] src/lib/diff/find.test.ts (`finds the old name where the row of a pair of Inline strikes it through, and at the row of a quiet line shown once, whose new name stands for it`, `gives the painter the hits of the old lines by line, the current one marked, for the new names that stand for them`)
  [@test] src/lib/diff/rename-sites.test.ts (`finds a hit of an old line at the first name it holds a part of, so a hit over two names is one hit`)

#### Scenario: The mark of a hit on a line shown once
- **WHEN** a hit of an old line holds a part of an old name, two hits lie in one old name, or the hit is the current one
- **THEN** the new name of the site is marked as a hit, once, as the current hit where either hit is
  [@test] src/lib/diff/rename-sites.test.ts (`marks the new name of a site where a hit holds a part of its old name, the current hit as the current one`)

#### Scenario: Where the old lines show
- **WHEN** the query is the old name under the opened note of a quiet rename, in Unified and on the old side of Split
- **THEN** each hit is in the row of its old line, and the text both lines have is found in both
  [@test] src/lib/diff/find.test.ts (`finds the old name in its own lines where they show: under the opened note of Inline, in Unified and on the old side of Split`)

#### Scenario: A step in the review
- **WHEN** the user finds `total` in the review of the template in Inline, presses Enter twice, finds text of a new line, then switches to Unified
- **THEN** the find counts three results, each marked in the struck-through `total` of its row; the first step marks the first as the current hit, in view, the second the next, and no `sum` is a hit
- **AND** in Unified the three results are marked in the old lines
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the find for the old name of a rename marks it where a row shows it: struck through in the rows of Inline, in the old lines of Unified, and never in the new name`)

#### Scenario: A step in the review, the runs quiet
- **GIVEN** the review of the template with the kind `renamed` switched on
- **WHEN** the user finds `total`, presses Enter twice, opens the note of the rename, closes it, then finds text of a new line
- **THEN** the find counts three results, each marked on the `sum` of the line shown once, whose hover says what it was; the first step marks the first as the current hit, in view, the second the next, and the note counts no hit
- **AND** under the opened note the three hits are in `total` of the old lines, none in a new line; the text of a new line is found in the line shown once
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the find for the old name of a quiet rename lands on the row that shows its new line and marks the new name; under the opened note the hit is in the old line`)

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

#### Scenario: The lines of a renamed name
- **WHEN** the user presses N through Ledger.kt of the `commit_marks` template, whose renamed local is three runs that are labels, and when the rows of such runs are built quiet
- **THEN** each of the three lines is a stop, at the line: the first under the note of the rename, the others after an unchanged line
- **AND** quiet, none of them is a stop, and the three blocks still count among the changes of the file
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`)
  [@test] src/lib/diff/rows.test.ts (`names a rename once in the rows of a file, before the lines of its declaration, and gives its other sites no note`, `shows the lines of a quiet rename once in Inline, which N and P pass, and opens them all by the one note of the rename`)

#### Scenario: The lines of a renamed name, quiet
- **GIVEN** the review of the template with the kind `renamed` switched on
- **WHEN** the user presses N through it, then P
- **THEN** N stops five times in Ledger.kt, at no line of the name: from `describe`, which moved, straight to the comment worded again; then three times in Report.kt and once in Store.kt
- **AND** P goes back through the same stops, and the three lines of the name show once, played down
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass the lines of a renamed local once its runs are quiet, and the file counts three changes fewer to read`)

### Requirement: Views: A file says how many of its changes are left to read

Where a file shows its counts (its row in the tree of the files, the header of its card in a review), it MUST say how
many of its blocks of changed lines are stops of N and P, where they are fewer than all: "10 to read", "Nothing to
read", in full in its hover and its description ("10 of 11 changes to read"). A card counts the stops of its section.

#### Scenario: The row of a file
- **WHEN** the preview of the `commit_marks` template lists its files
- **THEN** Ledger.kt says how many of its eleven changes are left to read ("8 to read"), fewer than all, and in full in its hover and its description ("8 of 11 changes to read"): the wrapped call and the two blocks its moved functions left are none of them
- **AND** Report.kt, whose every block is a stop, says nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the row of a file says how many of its changes are left to read, and nothing where every change is read`)
  [@test] src/lib/diff/view-files.test.ts (`counts the stops of N and P among the changes of a file where some change is quiet, and nothing where every change is read`, `counts no block without a line: the stand-in of a use a change left outside is no change`)

#### Scenario: The header of a card
- **WHEN** the review of the template, and of a file of quiet runs alone, shows its cards
- **THEN** the header of Ledger.kt and its row in the tree say the same count ("8 to read"), the header and the row of the other file "Nothing to read"
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`, `N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: A rename that is quiet
- **GIVEN** the answers of the engine for the `commit_marks` template with the kind `renamed` switched on
- **WHEN** the preview lists its files, and the review shows its cards
- **THEN** the row of Ledger.kt, its row in the tree of the review and the header of its card say "5 to read", in full "5 of 11 changes to read": the three blocks of the name are no more left to read
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline shows each line of a quiet rename once, the new name in its mark, and the one note of the rename shows and hides the old line of every site`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass the lines of a renamed local once its runs are quiet, and the file counts three changes fewer to read`)

#### Scenario: A review with its plan
- **WHEN** a file has a quiet run and a block of the imports alone, in a review with its plan
- **THEN** neither is left to read, as N and P pass both there
  [@test] src/lib/diff/view-files.test.ts (`passes a block of the imports alone too in a review with its plan, as N and P do there`)

#### Scenario: The words
- **WHEN** ten of eleven changes, one of two, none of three and none of one are left to read
- **THEN** the count is "10 to read", "1 to read", "Nothing to read", and in full "10 of 11 changes to read", "None of the 3 changes needs reading", "The change needs no reading"
  [@test] src/presenters/diff-marks.test.ts (`says how many changes of a file are left to read, short beside its counts and in full for a hover`)
