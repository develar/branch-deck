## ADDED Requirements

### Requirement: Views: A move between two files has a note at each end that names the other file

A view MUST draw a move between two files of a subject at both its ends: a note that says "Moved to Report.kt:18"
where the lines left and "Moved from Ledger.kt:55" where they stand now, then the lines, in the colour of a move.
Every layout MUST show every line of both ends, Inline too, and the note opens nothing. The file is named by its
name, with as many of its directories as tell it from another file of the subject of that name.

#### Scenario: The function that moved to the report
- **WHEN** the preview shows Ledger.kt and Report.kt of the `commit_marks` template in Inline, in Unified and in Split
- **THEN** the four lines `reportTitle` left show after the note "Moved to Report.kt:18", deleted lines of the move, none pale, and the four lines it stands at after the note "Moved from Ledger.kt:55"
- **AND** neither note is a button that opens; in Split each lies over the column of its lines, the other column filled beside the lines
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the lines that moved to another file show at both ends in every layout, in the colour of a move, each end under a note that names the file and the line of the other`)

#### Scenario: The ends of a block
- **WHEN** the marks of a subject name lines of a file that left for another file, or came from one, and the parts of their block are made in each layout
- **THEN** the block has an end of the move with its lines, the lines of the other end and the file they are in; the note stands before the lines, which show whatever notes are open, each old line at a row of its own
- **AND** lines no block holds whole, no line, and lines of a block the policy ignores give no end; beside a move within the file the ends of a side stand in the order of their lines, and a block is cut at such an end as at an end of any move
- **AND** a block of the imports alone that a review plays down draws no such end: its lines stay the pale lines they are
  [@test] src/lib/diff/block-parts.test.ts (`gives the block of the lines an end of the move, which names the file and the lines of the other end; none for lines no block holds whole`, `shows every line of both ends in every layout, Inline too, after a note that opens nothing`, `cuts the lines of a block at an end of a move between two files as at an end of any move: each side up to it, the note, its lines, then the rest`, `draws no end of a move between two files in a block a view plays down, and every end elsewhere`)
  [@test] src/lib/diff/rows.test.ts (`reads the lines that left for another file where they left: a stop of N and P, every line a row after the note, in Inline too`)

#### Scenario: The cells of the lines
- **WHEN** the cells of the lines of a move between two files are asked in each layout, in blocks that have inner ranges of their own
- **THEN** each is a line of a move in the full colour, at the end it is at, with no range; in Split the other side is filled
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move between two files as a line of a move at both ends in every layout, in the full colour, with no range`)

#### Scenario: The words of the note
- **WHEN** a note at an end of a move between two files is worded, and files of one name in several directories are named
- **THEN** it says "Moved" and "to Report.kt:18" or "from Ledger.kt:55", the first line of the other end as a reader counts, and counts no line
- **AND** a file is "Report.kt", or "api/Report.kt" beside "impl/Report.kt": the shortest end of its path that no other path of the subject ends with
  [@test] src/presenters/diff-marks.test.ts (`names the file and the first line of the other end at both ends, and counts no line: every line of the move shows`, `names a file among the files of its subject by its name, with the directories that tell it from another of that name`)

#### Scenario: The files a note names
- **WHEN** the diffs of a subject are read, the content of its second file first, then its scopes and the marks of its subject
- **THEN** each file has the paths of the files of the subject in the order of the stream, one list for all of them, which the marks of the subject name another file by, whatever content came first
  [@test] src/stores/file-diffs.test.ts (`gives each file the paths of the files of its subject in the order of the stream, whatever content came first and whatever came after it`)

### Requirement: Views: The place a note names in another file shows it there

The place a note of a move between two files names MUST be a button that shows the other end in its file, the note
there marked. In a review window the card of that file opens and the end the jump left is kept for Back; where each
file has a view of its own (the preview of a commit, the uncommitted changes) the diff of that file opens and shows
it. G and the menu of the code MUST take the note as the note of any move.

#### Scenario: A click in the preview of a commit
- **GIVEN** the preview of the commit of the template with the diff of Ledger.kt open and the one of Report.kt closed
- **WHEN** the user clicks "to Report.kt:18", then "from Ledger.kt:55" in the diff that opened, then "to Report.kt:18" again
- **THEN** the row of Report.kt opens its diff, which shows the note where the lines stand, marked; the second click shows the note where they left, marked; the third the first again
- **AND** the buttons are named "Go to line 18 of src/Report.kt, where the lines stand now" and "Go to line 55 of src/Ledger.kt, where the lines were", and name no key
- **AND** the diff of Report.kt, closed and opened again by its row, shows its top: the place a note asked of it before is asked no more
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a click on the file and the line a note of a move names opens the diff of that file under its row and shows the other end there, and the note there shows the first again`)

#### Scenario: A click in a review window
- **GIVEN** the review of the template with the card of Report.kt collapsed
- **WHEN** the user clicks "to Report.kt:18" in Ledger.kt, presses Back and Forward, clicks "from Ledger.kt:55" and presses Back
- **THEN** the card of Report.kt opens with the note where the lines stand marked, the diffs focused; Back returns to the note in Ledger.kt, Forward goes there again, and the jump back is a jump of its own
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a click on the place a note names jumps to the other end in the other file, into a card that is collapsed too; Back returns and Forward goes there again`)

#### Scenario: The menu of the code
- **WHEN** the menu of the code opens on a line `reportTitle` stands at in Report.kt, and on the note where its lines left in Ledger.kt
- **THEN** it has "Go to Other End of Move", with the key of the review where the key takes that note, and the item shows the other end in the other file
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the menu of the code on a line that moved between two files shows its other end in the other file`)
  [@test] src/presenters/diff-marks.test.ts (`names the action that shows the other end of a move, and says where it goes from each end`)

#### Scenario: The uncommitted changes
- **GIVEN** a working tree that moves a function from Store.kt to the end of Report.kt, in the list of the uncommitted changes with Store.kt open
- **WHEN** the user clicks "to Report.kt:23", then "from Store.kt:19"
- **THEN** the item of Report.kt opens and its diff shows the note where the lines stand, marked; the second click shows the note where they left
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a click on the place a note of a move names in another file of the list opens that file and shows the other end in its diff, and the note there shows the first again`)

### Requirement: Views: A place in a file no view shows is plain words

Where no view shows the file a note names (the search or the owner of a review hides it), the note MUST say the same
words as plain text: no button, with the file and the line in full on a hover. G MUST pass such a note and the menu
of the code MUST have no item of its move. Shown again, the place is a button.

#### Scenario: A file the search of a review hides
- **GIVEN** the review of a commit of eight files that moves a function from Accounts.kt to Names.kt
- **WHEN** the user types "Accounts" into the search of the files, presses G, opens the menu of the code on a moved line, then clears the search
- **THEN** the note in Accounts.kt still says "Moved to Names.kt:5", plain words with the hover "Line 5 of src/Names.kt, where the lines stand now"; no place of the diffs is a button, G moves nothing and the menu has no item of the move
- **AND** with every file shown the place is a button again, and a click shows the note in Names.kt
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note names the other end of a move in plain words while the review does not show its file: no button, no key, no item of the menu; shown again, it is a button`)

#### Scenario: The words and the key
- **WHEN** a note is worded whose file the view does not show, and the note of the key is asked of a layout with such notes
- **THEN** the words are the same and say that they show nothing; the key takes the next note in view whose other end the view shows
  [@test] src/presenters/diff-marks.test.ts (`says the same words of an end in a file the view does not show, as words that show nothing`)
  [@test] src/lib/diff/layout.test.ts (`passes a note of a move whose other end the view cannot show: the key takes the next one in view that it can, at the end the view stands at too`)

### Requirement: Views: A name another file declares is named once where it is used, with that file

A file that uses a name another file of the subject declares and renamed MUST name the rename once, at its first
site in the rows, with "declared in Store.kt" after its sites, and draw the name at each site as any renamed name.
Where a view shows that file the words MUST be a button that shows the declaration, the new name selected, the site
kept for Back; else plain words.

#### Scenario: The calls of the report
- **WHEN** the preview shows Report.kt of the template, and the user clicks "declared in Store.kt"
- **THEN** one note stands above the first call, "Renamed openStore → connectStore · 2 sites · declared in Store.kt", and each call has the name in the mark of a renamed name
- **AND** the click opens the diff of Store.kt at the declaration with `connectStore` selected; the note there says "Renamed openStore → connectStore · 1 site" and names no file
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the note of a name another file declares names that file, and a click on it opens the diff of that file at the declaration, the new name selected`)

#### Scenario: The declaration in a review, and Back
- **WHEN** the user clicks "declared in Store.kt" in the review of the template, then presses Back and Forward
- **THEN** the declaration shows in the card of Store.kt with `connectStore` selected, the diffs focused; Back selects the name at the first call in Report.kt, Forward the declaration again
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a click on the file that declares a name shows the declaration there with the new name selected, and Back returns to the use`)

#### Scenario: The note of a used name
- **WHEN** the notes of the renames of a file are made whose two calls of a name another file declares are runs of a renamed name, in a section that shows them all and in one that does not show the first
- **THEN** the name has one note, at its first site in the rows, with the file and the line of its declaration; both runs are named by it and have no note of their own
- **AND** a local of the file named in the same block shares the note, each rename with what the subject says of it
  [@test] src/lib/diff/block-parts.test.ts (`gives a name another file declares one note, at its first site in the rows, with the file and the line of its declaration and what the index proved`, `says of a name the file declares what the index proved of it, by its number among the renames of the file, and nothing of a local`)
  [@test] src/presenters/diff-marks.test.ts (`says after a rename which file of the subject declares the name, a button that shows it where the view shows that file`)

### Requirement: Views: The note of a rename says how far the index proved it

The note of a rename that is not local MUST say after its words what the index of the repository answered of its
uses, where the name is declared and where it is used: "all uses in this change" for a proof, else "uses outside
this change", "changes what another name means", "more than the name changed" or "uses not all checked". It MUST
say nothing while the index is still to be asked and where none answers.

#### Scenario: Every use in the change
- **GIVEN** the template, the index answering that the declaration of the store and its two calls are every use of the name in both trees
- **WHEN** the preview shows Report.kt and Store.kt, once the index answered
- **THEN** the notes say "Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change" and "Renamed openStore → connectStore · 1 site · all uses in this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says after its words that every use of the name is in the change, where the name is declared and where it is used`)

#### Scenario: A use the change left alone
- **GIVEN** the same, the index listing a call of the old name in a file the commit did not change
- **WHEN** the preview shows the two files
- **THEN** both notes end with "uses outside this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says that the name is used outside the change where the index lists a use the change left alone`)

#### Scenario: No index
- **WHEN** the index of the repository answers nothing of the name
- **THEN** the notes are what they were before it was asked, and say nothing of the uses
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says nothing of the uses of a name where no index answers: the rename is named, and each use is read`)

#### Scenario: The words of each answer
- **WHEN** a note of one rename, of two and of three is worded with each answer of the index
- **THEN** each answer has its words, none for a name still to be asked, without an index or that the index is not asked of
- **AND** in a note of several renames the words name the rename they are about by its new name; a rename the note counts says them in its line of the hover and to a screen reader
  [@test] src/presenters/diff-marks.test.ts (`says how far the index proved a rename: every use in the change, uses outside it, another meaning, more than the name, not all checked`, `tells of which rename of a note of several the subject speaks, by its new name, and of a counted one in its line of the hover`)

### Requirement: Views: A run the index proved is passed and keeps every line

A quiet run of a renamed name that the index of the repository proved (`SubjectMarks.confirmed`) MUST be played
down and passed by N and P, and MUST keep every line in every layout: Inline shows the row of its pair with both
names in their mark, as it shows a label, never the line once, and no note opens it. The same run reads the same
whenever its proof came.

#### Scenario: The rows of a proven run
- **WHEN** the parts and the cells of two calls of a name another file declares are made before the index answered, as labels, and after, as quiet runs the subject lists as proven, in each layout
- **THEN** the rows are the same rows of the same lines, in the quiet tint; in Inline the row of the pair, whose ranges are the two names alone
- **AND** no note opens them, the note of their rename hides none of their lines, and the widest row of Inline counts their pairs
  [@test] src/lib/diff/block-parts.test.ts (`keeps every line of the run in every layout, played down, with no note that opens: the row of the pair in Inline too`)
  [@test] src/lib/diff/row-cells.test.ts (`draws the names alone in the row of a pair of a quiet run that Inline keeps whole, as in the lines of the run elsewhere`)

#### Scenario: A quiet run the pair proved
- **WHEN** the same runs are quiet without the list of the subject, a local the scopes of the file prove
- **THEN** Inline shows each line once under the note that opens them, as before
  [@test] src/lib/diff/block-parts.test.ts (`shows a quiet run of the same kind once in Inline where the index did not prove it late: the pair says it with the content`)

### Requirement: Views: Marks that come after the rows of a file take no row of a line

The marks of a subject that come after the rows of a file were shown (what its files say of each other, once they
were compared; the answer of the index) MUST add rows of words and tints alone: every line that had a row keeps
one, in every layout, and nothing of them opens. A block they cut shows part by part, as any block the marks cut:
its lines may group anew around the note, each with its row.

#### Scenario: The rows before and after
- **WHEN** the rows of a file are built from its content alone, then with the marks of its subject, then with the answer of the index that proves its two calls, in each layout
- **THEN** the rows of the lines are the same each time, in the same order, and two rows of words came: the note of the rename above its first call and the note where the moved lines stand
- **AND** no note of them is a note that opens
  [@test] src/lib/diff/rows.test.ts (`adds rows of words alone to the rows the content of the file gave: every row of a line stays, the same line in the same order, in every layout`, `stops N and P at each use of the name while the index proved nothing, and passes the uses it proved, which stay on screen played down`)

#### Scenario: A block a mark of the subject cuts
- **WHEN** three lines became three others the engine does not pair, and the second old line stands in another file now
- **THEN** one column shows the first old line and the new lines, then the note and the line that left, then the last old line, where it showed the old lines and then the new ones
- **AND** every line of the block has the one row it had, in each layout
  [@test] src/lib/diff/rows.test.ts (`regroups the lines of a block that a mark of the subject cuts around its note, part by part, each line with the one row it had`)

### Requirement: Views: The row at the top of a view stays where it is as marks come late

A view MUST keep the row at its top where it is while the marks of a subject add rows above it. A view that scrolls
itself keeps its anchor; a view that is a block of another scroller MUST move that scroller by the rows that came
above its top row, by all of them when the view lies above the viewport, each view by its own part.

#### Scenario: A review window
- **GIVEN** the review of the template whose Report.kt says nothing of its subject until the index answers, a line of `reportTitle` at the top of the diffs
- **WHEN** the answer comes with the marks of the file, the two calls proven
- **THEN** the diffs scroll by the two rows of the notes that came above, and the line stays where it was; the file counts one change of three to read
- **AND** at the top of the file both notes show, and each call is the row of two lines it was, played down
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the notes they add come above the row a reader has at the top of a review, which stays where it is, and the uses the index proved keep their rows, played down`)

#### Scenario: A diff of a preview above the view
- **GIVEN** the preview of the commit with the diffs of its three files open, scrolled to its end, so the diff of Report.kt lies above the view
- **WHEN** the marks of Report.kt come
- **THEN** the preview scrolls by the two rows the diff above grew by, and the line of Store.kt at its top stays where it was
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the notes they add to a diff of a preview that lies above the one a reader has on screen leave the row at the top of the view where it is`)

#### Scenario: The diff of a preview a reader has on screen
- **GIVEN** the same preview with a line of `reportTitle` in Report.kt at its top
- **WHEN** the marks of Report.kt come
- **THEN** the preview scrolls by the two rows of the notes, and the line stays where it was, a line of the move now
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the notes they add to the diff a reader has on screen in a preview come above its top row, which stays where it is`)

#### Scenario: The anchor of a view
- **WHEN** the row of a line is asked before and after the notes came above it, and the top of a view below the rows of its last file is kept across a layout with more rows
- **THEN** the line has the row it had plus the notes above it, never a note; a top below the rows is held from their end, so it moves by the rows that came
  [@test] src/lib/diff/rows.test.ts (`keeps the row at the top of a view at its line while the notes come above it`)
  [@test] src/lib/diff/layout.test.ts (`keeps a top below the rows of the last file from the end of its rows: rows that come above it move it by as much, and nothing of the file is brought back into view`)
