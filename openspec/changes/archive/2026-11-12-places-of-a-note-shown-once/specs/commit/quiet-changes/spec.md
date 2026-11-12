## MODIFIED Requirements

### Requirement: Views: The place a note names in another file shows it there

The place a note of a move between two files names MUST be a button that shows the other end in its file, the note
there marked. In a review window the card of that file opens and the end the jump left is kept for Back; where each
file has a view of its own (the preview of a commit, the uncommitted changes) the diff of that file opens and shows
it, once: a diff of that file that mounts again later MUST show its top. G and the menu of the code MUST take the
note as the note of any move.

#### Scenario: A click in the preview of a commit
- **GIVEN** the preview of the commit of the template with the diff of Ledger.kt open and the one of Report.kt closed
- **WHEN** the user clicks "to Report.kt:18", then "from Ledger.kt:55" in the diff that opened, then "to Report.kt:18" again
- **THEN** the row of Report.kt opens its diff, which shows the note where the lines stand, marked; the second click shows the note where they left, marked; the third the first again
- **AND** the buttons are named "Go to line 18 of src/Report.kt, where the lines stand now" and "Go to line 55 of src/Ledger.kt, where the lines were", and name no key
- **AND** back at the top of the preview, the directory of the files folded and unfolded and then the diffs read again under another whitespace policy, the preview stays at its top and no row is marked: a place is shown once; asked again by its note, it is shown again
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
words as plain text: no button, with the file and the line in full on a hover, or of the file that declares a name
"connectStore is declared in src/Store.kt". G MUST pass such a note and the menu of the code MUST have no item of
its move and no declaration to go to. Shown again, the place is a button.

#### Scenario: A file the search of a review hides
- **GIVEN** the review of a commit of eight files that moves a function from Accounts.kt to Names.kt
- **WHEN** the user types "Accounts" into the search of the files, presses G, opens the menu of the code on a moved line, then clears the search
- **THEN** the note in Accounts.kt still says "Moved to Names.kt:5", plain words with the hover "Line 5 of src/Names.kt, where the lines stand now"; no place of the diffs is a button, G moves nothing and the menu has no item of the move
- **AND** with every file shown the place is a button again, and a click shows the note in Names.kt
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note names the other end of a move in plain words while the review does not show its file: no button, no key, no item of the menu; shown again, it is a button`)

#### Scenario: The file that declares a name, hidden by the search
- **GIVEN** the review of a commit of eight files in which Accounts.kt renames its function `openAccounts` to `loadAccounts` and Names.kt, which calls it, renames a local on the same line
- **WHEN** the user types "Names" into the search of the files, clicks "declared in Accounts.kt", opens the menu of the code on the note, then clears the search
- **THEN** the note in Names.kt says "Renamed list → found · 2 sites · openAccounts → loadAccounts · 1 site · loadAccounts declared in Accounts.kt" both times: the new name before the words says which of its two renames they speak of
- **AND** while the search hides Accounts.kt the words are no button, their hover "loadAccounts is declared in src/Accounts.kt"; the click goes nowhere and Go to Declaration of the menu is disabled
- **AND** with every file shown they are the button "Go to the declaration of loadAccounts in src/Accounts.kt", which shows the declaration with the name selected, and Back returns to the call
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note names the file that declares a name in plain words while the review does not show that file: no button, nothing to go to; shown again, it is a button, in a note of two renames beside the name it speaks of`)

#### Scenario: The words and the key
- **WHEN** a note is worded whose file the view does not show, and the note of the key is asked of a layout with such notes
- **THEN** the words are the same and say that they show nothing; the key takes the next note in view whose other end the view shows
  [@test] src/presenters/diff-marks.test.ts (`says the same words of an end in a file the view does not show, as words that show nothing`)
  [@test] src/lib/diff/layout.test.ts (`passes a note of a move whose other end the view cannot show: the key takes the next one in view that it can, at the end the view stands at too`)

### Requirement: Views: A name another file declares is named once where it is used, with that file

A file that uses a name another file of the subject declares and renamed MUST name the rename once, at its first
site in the rows, with "declared in Store.kt" after its sites, and draw the name at each site as any renamed name.
Where a view shows that file the words MUST be a button that shows the declaration, the new name selected; Back
MUST return to the site the note names the rename at, never an earlier one its rows do not show. Else plain words.

#### Scenario: The calls of the report
- **WHEN** the preview shows Report.kt of the template, and the user clicks "declared in Store.kt"
- **THEN** one note stands above the first call, "Renamed openStore → connectStore · 2 sites · declared in Store.kt", and each call has the name in the mark of a renamed name
- **AND** the click opens the diff of Store.kt at the declaration with `connectStore` selected; the note there says "Renamed openStore → connectStore · 3 sites in 2 files" and names no file
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the note of a name another file declares names that file, and a click on it opens the diff of that file at the declaration, the new name selected`)

#### Scenario: The declaration in a review, and Back
- **WHEN** the user clicks "declared in Store.kt" in the review of the template, then presses Back and Forward
- **THEN** the declaration shows in the card of Store.kt with `connectStore` selected, the diffs focused; Back selects the name at the first call in Report.kt, Forward the declaration again
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a click on the file that declares a name shows the declaration there with the new name selected, and Back returns to the use`)

#### Scenario: The site Back returns to
- **WHEN** the notes of a file are made that names a function another file declares in an import and in a call, with every row shown, in a review that plays the imports down, and in a section that does not show the block of the first site; and a note of two renames
- **THEN** the jump from the note leaves the site the note stands at: the import where every row shows, the call where the import is played down or not shown
- **AND** each rename of a note of two is named at its own site, and a name the file declares itself has no declaration elsewhere to jump to
  [@test] src/lib/diff/block-parts.test.ts (`names each rename of a note at its site in the rows, the place a jump to its declaration leaves: never an earlier site that a review plays down or a section does not show`)

#### Scenario: The note of a used name
- **WHEN** the notes of the renames of a file are made whose two calls of a name another file declares are runs of a renamed name, in a section that shows them all and in one that does not show the first
- **THEN** the name has one note, at its first site in the rows, with the file and the line of its declaration; both runs are named by it and have no note of their own
- **AND** a local of the file named in the same block shares the note, each rename with what the subject says of it
  [@test] src/lib/diff/block-parts.test.ts (`gives a name another file declares one note, at its first site in the rows, with the file and the line of its declaration and what the index proved`, `says of a name the file declares what the index proved of it and how far it reaches in the subject, by its number among the renames of the file, and nothing of a local`)
  [@test] src/presenters/diff-marks.test.ts (`says after a rename which file of the subject declares the name, a button that shows it where the view shows that file`)

### Requirement: Views: The note of a rename says how far the index proved it

The note of a rename that is not local MUST say after its words what the index of the repository answered of its
uses, where the name is declared and where it is used: "all uses in this change" for a proof, else "uses outside
this change", "changes what another name means", "more than the name changed" or "uses not all checked". It MUST
say nothing while the index is still to be asked and where none answers.

#### Scenario: Every use in the change
- **GIVEN** the template, the index answering that the declaration of the store and its two calls are every use of the name in both trees
- **WHEN** the preview shows Report.kt and Store.kt, once the index answered
- **THEN** the notes say "Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change" and "Renamed openStore → connectStore · 3 sites in 2 files · all uses in this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says after its words that every use of the name is in the change, where the name is declared and where it is used`)

#### Scenario: A use the change left alone
- **GIVEN** the same, the index listing a call of the old name in a file the commit did not change
- **WHEN** the preview shows the two files
- **THEN** both notes end with "uses outside this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says that the name is used outside the change where the index lists a use the change left alone`)

#### Scenario: No index
- **WHEN** the index of the repository answers nothing of the name
- **THEN** the notes are what they were before it was asked, and say nothing of the uses
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says nothing of the uses of a name where no index answers: the rename is named as before it was asked`)

#### Scenario: The words of each answer
- **WHEN** a note of one rename, of two and of three is worded with each answer of the index
- **THEN** each answer has its words, none for a name still to be asked, without an index or that the index is not asked of
- **AND** in a note of several renames the words name the rename they are about by its new name; a rename the note counts says them in its line of the hover and to a screen reader
  [@test] src/presenters/diff-marks.test.ts (`says how far the index proved a rename: every use in the change, uses outside it, another meaning, more than the name, not all checked`, `tells of which rename of a note of several the subject speaks, by its new name, and of a counted one in its line of the hover`)

### Requirement: Views: The row at the top of a view stays where it is as marks come late

A view MUST keep the row at its top where it is while the marks of a subject add rows above it. A view that scrolls
itself keeps its anchor; a view that is a block of another scroller MUST move that scroller by the rows that came
above its top row, by all of them when the view lies above the viewport, each view by its own part. The engine's
own scroll anchoring MUST be off in that scroller only while it holds such a view.

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

#### Scenario: The scroller once its last diff is closed
- **WHEN** the preview of the template opens the diff of Ledger.kt, then the one of Report.kt, closes the first, closes the second, and opens it again
- **THEN** the engine's scroll anchoring is off in the preview while a diff is open, with one as with two, on again once the last one is closed, and off again with the diff that opens
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the scroller around the diffs of a preview is kept by its views while it holds one, and is the engine's to keep again once its last diff is closed`)

#### Scenario: The anchor of a view
- **WHEN** the row of a line is asked before and after the notes came above it, and the top of a view below the rows of its last file is kept across a layout with more rows
- **THEN** the line has the row it had plus the notes above it, never a note; a top below the rows is held from their end, so it moves by the rows that came
  [@test] src/lib/diff/rows.test.ts (`keeps the row at the top of a view at its line while the notes come above it`)
  [@test] src/lib/diff/layout.test.ts (`keeps a top below the rows of the last file from the end of its rows: rows that come above it move it by as much, and nothing of the file is brought back into view`)

## ADDED Requirements

### Requirement: Views: Go to Declaration on the note of a name another file declares shows that declaration

Go to Declaration of the menu of the code, opened on the note of a rename that names the file declaring a name,
where a view shows that file, MUST show the declaration the note names, as a click on its place does; Find Usages
has no name there. On any other note the item has no name to go to. The place has no key of its own.

#### Scenario: The menu of the code on the note
- **WHEN** the menu of the code opens on the note "Renamed openStore → connectStore · 2 sites · declared in Store.kt" in the review of the template, and on the note of Store.kt
- **THEN** on the first its Go to Declaration is enabled and shows the declaration in Store.kt with `connectStore` selected, Back returns to the call, and Find Usages is disabled: the row holds no name
- **AND** on the note of the file that declares the name the item is disabled
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`Go to Declaration of the menu of the code on the note of a name another file declares shows the declaration the note names, and Back returns to the use`)

#### Scenario: The item of the menu
- **WHEN** the items of the menu are built on a note that names a declaration, on a name of such a row, and on a note that names none
- **THEN** on the note the item goes where the note says, with the name of its place as its hover; on a name it is the name's; else it is disabled
  [@test] src/lib/diff/code-menu.test.ts (`goes to the declaration the note of a rename names where the menu opened on that note, a row of no name`)
