## MODIFIED Requirements

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"), and a
renamed name "renamed" before it, once a name. A note that opens MUST be a button named by its words and by what a
press does, with its expanded state; a note of renames MUST say the renames it counts. A line of a move MUST say
its end, and that it was edited on the way or that its other end is in another file; the place its note names is a
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

#### Scenario: A rename
- **WHEN** a view shows the sites of a rename, and its note opens the old lines of its quiet runs
- **THEN** each name carries the kind of a renamed name and, on the part it starts with, the mark of its start, where the generated content reads "renamed" out; a line of a quiet run says "renamed only" after its type
- **AND** the note is the button "Renamed total to sum, 12 sites: Show the 4 old lines", with its expanded state; of two renames "Renamed total to sum, 12 sites, and count to size, 3 sites", of three "… and 2 more: count to size, 3 sites; flag to enabled, 1 site"
  [@test] src/presenters/diff-marks.test.ts (`says the two spellings of a renamed name and how many places hold it, one site or many`, `words both renames of a note of two, and of three or more the first with the count of the others, each told for a hover, to a screen reader and in the name of the note`, `names the button of a note that opens by the old lines of every quiet run of its renames, one line or many`)
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)

#### Scenario: A name the find cuts
- **WHEN** the find for text that holds the end of a renamed name marks it in the review, so the name is two parts
- **THEN** the first part alone carries the mark of the start of the name, and every name of the rows carries one
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the find for the old name of a rename marks it where a row shows it: struck through in the rows of Inline, in the old lines of Unified, and never in the new name`)

#### Scenario: The renames a note counts
- **WHEN** a note of plain words names three renames in the preview
- **THEN** its text holds the two it counts after the count, in words no eye sees, as the name of a note that opens holds them
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a note words both renames of a block that declares two, the first of three with the count of the others, and stands right above the line of its declaration`)

#### Scenario: A move between two files
- **WHEN** the preview shows the lines `reportTitle` left in Ledger.kt and the lines it stands at in Report.kt
- **THEN** their lines carry the end of the move and that the other end is in another file, which the generated content reads out: "moved to another file", "moved here from another file"
- **AND** the places are the buttons "Go to line 18 of src/Report.kt, where the lines stand now" and "Go to line 55 of src/Ledger.kt, where the lines were"; the file that declares a name is the button "Go to the declaration of connectStore in src/Store.kt"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the lines that moved to another file show at both ends in every layout, in the colour of a move, each end under a note that names the file and the line of the other`, `a click on the file and the line a note of a move names opens the diff of that file under its row and shows the other end there, and the note there shows the first again`, `the note of a name another file declares names that file, and a click on it opens the diff of that file at the declaration, the new name selected`)
  [@test] src/presenters/diff-marks.test.ts (`names the file and the first line of the other end at both ends, and counts no line: every line of the move shows`, `names the action that shows the other end of a move, and says where it goes from each end`)

### Requirement: Views: N and P stop where the reading of a block starts

N and P MUST stop only at the starts of a diff: the first row of lines of what is read from the line `starts`
names. Rust cuts a block by its quiet runs alone, so lines deleted before a label are read from their first, and
one column shows the old lines of a part first. Lines a move took within the file end what is read before them;
lines that left for another file are read. A block without a start is passed, as are the imports alone of a review
with its plan and a block another section shows.

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

#### Scenario: The lines that moved between two files
- **WHEN** the rows of a block whose lines left for another file are built in each layout, of a block that is those lines alone, and of the block the lines stand in
- **THEN** the block is read from its first line, a moved one too: the stop is the row of that line, under the note where the block starts with the move, and the lines that came from another file are read where they stand
- **AND** in the review of the template the last stop of Report.kt is the block the lines of `reportTitle` stand in
  [@test] src/lib/diff/block-parts.test.ts (`reads the lines that left for another file: the reading of their block starts at its first line, a moved one too`)
  [@test] src/lib/diff/rows.test.ts (`reads the lines that left for another file where they left: a stop of N and P, every line a row after the note, in Inline too`, `stops N and P at each use of the name while the index proved nothing, and passes the uses it proved, which stay on screen played down`)

#### Scenario: The uses of a name the index proved
- **GIVEN** the review of the template with the kind `renamed` switched on, the index of the repository answering that every use of the function the store renamed is a place of the commit
- **WHEN** the user presses N through it, then P
- **THEN** N stops five times in Ledger.kt, once in Report.kt, at the block the lines of `reportTitle` stand in and at no call of the name, and once in Store.kt, at the declaration
- **AND** P goes back through the same stops, and the two calls show as they were, each the row of its two lines, played down, both names in their mark, under a note that opens nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass the uses of a name another file declares once the index proved them and their runs are quiet, and the file counts them out; the lines stay on screen`)

### Requirement: Views: G shows the other end of a move in view

In a review window G MUST show the other end of a move from one note of an end in view: the note of the end the view
stands at while it is in view (where a jump, Back or Forward landed, or the note N or P stopped under), else the first
one, so two presses go there and back. Only the button of the place of that note MUST name the key, the key MUST move
nothing with no such note in view, and it MUST be the key of no action of the window.

#### Scenario: The key from the tree and from the diffs
- **GIVEN** the review of the `commit_marks` template at the top of Ledger.kt, the focus on its row of the tree
- **WHEN** the user presses G twice, then Back, then shows Store.kt and presses G
- **THEN** the first press shows the note where `accounts()` left, marked, and gives the diffs the focus; the second returns to the note where it stands now; Back returns along the second jump
- **AND** at Store.kt, which has no note of a move, with no such note in view, G moves nothing
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

#### Scenario: A move between two files
- **GIVEN** the review of the template at Report.kt, where the note of the lines that came from Ledger.kt is the one note of a move in view
- **WHEN** the user presses G twice, then Back
- **THEN** the button "from Ledger.kt:55" names the key; the first press shows the note where the lines left in Ledger.kt, marked, whose button "to Report.kt:18" names the key then; the second returns to Report.kt; Back returns along the second jump
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`G jumps from the note of a move between two files to its other end in the other file and back`)

#### Scenario: A note the key cannot take
- **WHEN** the note of the key is asked of a layout whose first note in view names an end the view cannot show, or whose every note does, and with the view standing at such an end
- **THEN** it is the next note in view whose other end the view shows, of the next file where the first has none, and none where no note can be taken
  [@test] src/lib/diff/layout.test.ts (`passes a note of a move whose other end the view cannot show: the key takes the next one in view that it can, at the end the view stands at too`)

### Requirement: Views: A renamed name is drawn in a mark of its own, never as a change

The name at a site of a rename MUST be drawn in the mark of a renamed name on both sides, in every layout, on a
quiet line, a line that is read and a line with another change, whose other ranges stay. No range of the engine
inside the name is drawn; on a line of a run of a renamed name the names MUST be the only ranges. Its hover MUST say
what it was ("Renamed from total") or, on the old side, what it became. A name another file of the subject declares
is drawn the same at each of its uses.

#### Scenario: The sites in every layout
- **WHEN** the preview shows the three sites of `total` of the template in Inline, Unified and Split
- **THEN** each new line has `sum` in the mark of a renamed name, each old line that shows has `total` in it, the row of a pair of Inline `total` struck through before `sum`, and no other part of those lines is a changed range
- **AND** the real change of the file, `isEmpty()` to `none()`, is drawn as before
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`, `Unified and Split keep both lines of each site of a rename, the old name and the new one in the mark of a renamed name`)
  [@test] src/lib/diff/row-cells.test.ts (`gives a renamed name a range of its own kind in place of what the engine marked in it, on both sides, and keeps the other changes of its line`)

#### Scenario: The ranges of a line with a site
- **WHEN** the engine marked a name as a whole, only the letters that differ, a range over two names and the text between them, or an empty side at an end of the name
- **THEN** each name is one range of its own kind, what a range of the engine holds outside the names stays a range of its kind, and a marker that touches a name goes with it
- **AND** a place a rename names on one side alone is no site
  [@test] src/lib/diff/rename-sites.test.ts (`gives the sites of every rename by side and line, in the order of their columns, each with the same site of the other side`, `leaves out a place a rename names on one side alone, which no engine gives`, `makes each renamed name a range of its own kind and leaves out what the engine marked inside it`, `keeps what a range of the engine holds outside a name, before it, after it and between two names`, `takes the marker of an empty side that touches a name with the name, and keeps one apart from it`)

#### Scenario: A line of a run of a renamed name
- **WHEN** the engine marked the three names of a line and the text between them as one range, on a line of a run of a renamed name, a label and a quiet one, and on a line of no run
- **THEN** the names alone are the ranges of the line of the run, in Unified, in Split and in the row of a pair of Inline; on the line of no run what the engine marked beside the names stays a range
- **AND** in a browser the line `val all = small + large` of a run has its three names in their mark and no changed part
  [@test] src/lib/diff/row-cells.test.ts (`marks the names alone on a line of a run of a renamed name, a label too: what the engine marked between two names there is the same code`)
  [@test] src/lib/diff/rename-sites.test.ts (`finds the run of a renamed name that holds a new line, never a run of another kind`)
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a note words both renames of a block that declares two, the first of three with the count of the others, and stands right above the line of its declaration`)

#### Scenario: A place that makes more of its name than its binding
- **WHEN** a place of a rename is a shorthand property, on a line Rust keeps out of every run, below a quiet line of the name
- **THEN** that line is the stop of its block, a row of its two lines, and its name is in the mark as on a line with another change, with no range of the engine: the marks list the place and do not say that it is more than the name (Open Questions)
  [@test] src/lib/diff/row-cells.test.ts (`draws the name of a place on a line of no run in its mark and keeps that line a stop, whatever the place makes of the name: the marks name no such place`)

#### Scenario: The hover of a name
- **WHEN** the pointer rests on a renamed name of a new line, and of an old line
- **THEN** the hover says "Renamed from total", and "Renamed to sum"
  [@test] src/presenters/diff-marks.test.ts (`says what a renamed name of a line was on the new side, and what it became on the old one`)
  [@test] src/lib/diff/row-cells.test.ts (`names the sites of a line of a side and the run of a renamed name that holds a line, for the hover of a name and the find; a merge has none`)
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)

#### Scenario: A file that only uses the name
- **WHEN** the preview shows Report.kt of the template, which calls `connectStore` twice and declares none of it, while Store.kt of the same commit declares it
- **THEN** one note words the rename, "Renamed openStore → connectStore · 2 sites", and each call has `connectStore` in the mark of a renamed name, `openStore` struck through before it in the same mark, and no other changed part
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a name that is not local is named at its declaration, which is read, and once in a file that only uses it, each use in the mark of a renamed name`)
  [@test] src/lib/diff/rename-sites.test.ts (`are renames of the file after those it declares, in one list, the same list whenever it is asked`, `have their sites as any rename, by their number in that list, and their lines show the names alone`)
