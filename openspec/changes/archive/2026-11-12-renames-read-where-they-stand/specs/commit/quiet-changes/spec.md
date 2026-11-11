## MODIFIED Requirements

### Requirement: Views: A rename has one note in the rows of a file, at its declaration

A view MUST name a rename once in the rows of a file: a note right above the line of its declaration, or of its
first site where the rows do not show that block; a block of the imports alone that a review plays down has it only
when no other block holds a site. The lines of the block that stand before that line, on either side, MUST stay
above the note. Its other sites MUST have no note, and a run of a renamed name that the rename names none of its
own.

#### Scenario: The local of the template
- **WHEN** the preview of the `commit_marks` template shows Ledger.kt, whose local `total` is `sum` at three lines, in Inline, Unified and Split
- **THEN** one note, "Renamed total → sum · 3 sites", stands before the line of its declaration, plain words that open nothing
- **AND** the two other sites stand after an unchanged line, with no note
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`, `Unified and Split keep both lines of each site of a rename, the old name and the new one in the mark of a renamed name`)
  [@test] src/lib/diff/rows.test.ts (`names a rename once in the rows of a file, before the lines of its declaration, and gives its other sites no note`)
  [@test] src/lib/diff/block-parts.test.ts (`gives a rename one note, at its declaration, with the old lines of its quiet runs to open; its runs have no note of their own`)

#### Scenario: A name that is not local
- **WHEN** the preview shows Store.kt of the template, whose function `openStore` is `connectStore`, declared there
- **THEN** the note "Renamed openStore → connectStore · 1 site" stands before the line of the declaration, which is read
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a name that is not local is named at its declaration, which is read; a file that only uses the name shows its change as any`)

#### Scenario: A section of a review that does not show the declaration
- **WHEN** the rows are those of a section of a review that shows two sites of a rename and not the block of its declaration
- **THEN** the note stands at the first of those sites, and names no site of a block the section does not show
- **AND** rows that show no site of the rename name it nowhere
  [@test] src/lib/diff/rows.test.ts (`names the rename at its first site in a section of a review that does not show its declaration, and opens the runs of that section alone`)
  [@test] src/lib/diff/block-parts.test.ts (`stands the note at the first site the rows show where they do not show the declaration, and opens the runs they show alone`)

#### Scenario: A block that is played down
- **WHEN** the declaration of a rename is in a block of the imports alone, in a review with its plan and outside one
- **THEN** with the plan the block stays pale with no note, and the note stands at the next site; without it the note is at the declaration
  [@test] src/lib/diff/rows.test.ts (`keeps the note of a rename out of a block of the imports alone that a review plays down while another block holds a site`)

#### Scenario: The place of the note in its block
- **WHEN** the site of the note is on a line after a quiet run of formatting of its block, or on the first lines of a block with a heading
- **THEN** the note is the row before the lines that hold the site, after the lines of the run; in the row of the heading where those lines start the block
  [@test] src/lib/diff/block-parts.test.ts (`stands the note before the lines that hold its site: after a run that comes first, in the row of the heading where those lines start the block`)

#### Scenario: A declaration inside the lines of a block
- **WHEN** the declaration of a rename is the fourth of five changed lines of a block, in lines the engine pairs and in lines it does not, in Inline, Unified and Split
- **THEN** the note is the row right above the line of the declaration: the lines before it, the old ones and the new ones, stay above the note, and its old line and its new one follow, in one row in Split
- **AND** a site on the first line of its lines cuts nothing, the lines of a quiet run that Inline shows once are cut by their new lines, and the reading of the block starts at its first row
  [@test] src/lib/diff/block-parts.test.ts (`cuts the lines that hold the site of the note at the site, each side at its line, so the note is the row right above it`)

#### Scenario: A declaration after a line that really changed, in a browser
- **GIVEN** a change that renames two locals of a function, the first declared on the second changed line of its block, after a line that really changed
- **WHEN** the preview shows the file in Inline, then in Split
- **THEN** the note is the row right above the old line of the declaration, and the row above the note is the new line of the change before it; in Split the two lines of the declaration are one row under the note
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a note words both renames of a block that declares two, the first of three with the count of the others, and stands right above the line of its declaration`)

#### Scenario: A run no rename names
- **WHEN** the marks hold a run of a renamed name and no rename with a site in it, which no engine gives, or the rows name no rename
- **THEN** the run has the note of its kind, "Renamed only", which opens it as the note of any quiet run
  [@test] src/lib/diff/block-parts.test.ts (`keeps the note of a run of a renamed name that the marks name no rename for, which opens it as any quiet run`)
  [@test] src/presenters/diff-marks.test.ts (`words a run of the kind renamed that names no rename by its kind, as any run`)

### Requirement: Views: The note of a rename says its two spellings and its sites

The note of a rename MUST say "Renamed", the old and the new spelling of the name with an arrow between them, and
how many places hold the name ("Renamed total → sum · 12 sites", "1 site"). Renames whose notes stand in one block
MUST share one note, at the first of their sites, worded as a cut list of the app: two are both worded, never "and 1
more"; of three or more the first, the others counted ("and 2 more"), listed on a hover and said to a screen reader.

#### Scenario: The words
- **WHEN** a note words a rename of twelve sites, and one of one site
- **THEN** it says "Renamed", the two spellings, and "12 sites" or "1 site"; as one sentence "Renamed total to sum, 12 sites"
  [@test] src/presenters/diff-marks.test.ts (`says the two spellings of a renamed name and how many places hold it, one site or many`)

#### Scenario: Several renames at one block
- **WHEN** two renames are declared on one line and a third in another block, and a note names two, three or four renames
- **THEN** the two share one note at the first of their sites, the third has its own
- **AND** a note of two words both; a note of three or four words the first and says "and 2 more" or "and 3 more", each of the others told for the hover ("count → size · 3 sites"), for a screen reader (": count to size, 3 sites; flag to enabled, 1 site") and in the name of the note
  [@test] src/lib/diff/block-parts.test.ts (`names the renames whose notes stand in one block in one note, at the first of their sites, and any other in its own`)
  [@test] src/presenters/diff-marks.test.ts (`words both renames of a note of two, and of three or more the first with the count of the others, each told for a hover, to a screen reader and in the name of the note`)

#### Scenario: Several renames at one block, in a browser
- **GIVEN** a change that renames three locals declared on the lines of one block, and two declared in another
- **WHEN** the preview shows the file
- **THEN** the note of the three reads "Renamed low → small · 2 sites · and 2 more", plain words; the hover of its count lists "high → large · 2 sites" and "both → all · 2 sites", and the words of the two are in the text of the note after the count, where no eye sees them
- **AND** the note of the two reads "Renamed left → a · 2 sites · right → b · 2 sites" and counts none
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a note words both renames of a block that declares two, the first of three with the count of the others, and stands right above the line of its declaration`)

### Requirement: Views: A renamed name is drawn in a mark of its own, never as a change

The name at a site of a rename MUST be drawn in the mark of a renamed name on both sides, in every layout, on a
quiet line, a line that is read and a line with another change, whose other ranges stay. No range of the engine
inside the name is drawn; on a line of a run of a renamed name the names MUST be the only ranges. Its hover MUST say
what it was ("Renamed from total") or, on the old side, what it became. A name the file does not declare is no
rename of its marks and shows as any change.

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
- **WHEN** the preview shows Report.kt of the template, which calls `connectStore` and declares none of it
- **THEN** no note of it words a rename and no name is in the mark of one: each call is a change, `openStore` struck through before `connectStore`
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a name that is not local is named at its declaration, which is read; a file that only uses the name shows its change as any`)

### Requirement: Views: The row of a pair of Inline strikes the old name of a rename through, in its mark

Where Inline shows a similar pair as one row, the row MUST show the old name of a site struck through before the new
one, each as a whole name in the mark of a renamed name, whatever letters the engine marked inside them; the other
changes show as before, and a range of the engine that reaches out of the name MUST be cut at it. Nothing of a line
that is read is left out. Lines of a pair that hold a name on two lines the engine does not pair MUST show apart,
each with the name in its mark.

#### Scenario: The lines of the template
- **WHEN** Inline shows the three lines of `sum` of the template
- **THEN** each is one row: `total` struck through before `sum`, both in the mark of a renamed name, and no other changed part
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)
  [@test] src/lib/diff/row-cells.test.ts (`gives the row of a pair of Inline the name of both lines as one range, so the row strikes the whole old name through before the new one`)

#### Scenario: The ranges of the pair
- **WHEN** the engine marked the name as a whole, as three ranges inside it, as a range that reaches out of it or over two names, and the pair has other changes
- **THEN** the name is one range of both lines, what is left of a range outside it a range of its own (inserted or deleted where a side of it is empty), and both lines keep as many ranges as each other
- **AND** ranges that are no pair, and a pair without a site, stay as they are
  [@test] src/lib/diff/rename-sites.test.ts (`makes the name of both lines one range of the pair, whatever the engine marked inside it`, `cuts a range that reaches out of a name at the name: what is left is a range of its own, both lists as long as each other`, `leaves ranges that are no pair, and a pair without a site, as they are`)

#### Scenario: The text of the row
- **WHEN** a pair renames a name twice and changes its last words
- **THEN** the text of the row is the new line with each old name struck through before the new one and the old last words before the new ones
- **AND** an offset of an old name is a place of its struck-through part, where a reveal selects it and a hit of the find is marked
  [@test] src/lib/diff/line-parts.test.ts (`strikes the old name through before the new one, each a part of the kind of a renamed name, and hides nothing of the old line`, `places and marks the old name as any deleted part: a reveal selects it and a hit of the find is in it`)

#### Scenario: A name on two lines the engine does not pair
- **WHEN** a wrap moved a renamed name to the next line, in a block whose lines the engine pairs line by line
- **THEN** Inline shows those lines as Unified does, no row of a pair, the old name in its mark in its old line and the new one in its new line, what is left of the range of the engine beside each; Split marks both in the row of the two lines
- **AND** such lines are no pair of the rows Inline measures, and a block whose name stays on its line keeps its rows of pairs
  [@test] src/lib/diff/block-parts.test.ts (`shows the lines of a pair apart where a renamed name stands on two lines the engine does not pair, so both lines mark it`)
  [@test] src/lib/diff/row-cells.test.ts (`marks a name that stands on two lines the engine does not pair on both of them in Inline, as Unified and Split do, never as a deletion in one row and an insertion in another`)

#### Scenario: A removed declaration that was renamed
- **GIVEN** a function renamed at its declaration, whose uses outside keep the old name, in a review with its plan
- **WHEN** the old name is shown from Possible Breakage
- **THEN** it is selected in the struck-through part of the row, copied and read by Find Usages as the name of the old line
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

### Requirement: Views: Inline shows a quiet run of a renamed name once, and the note of its rename opens it

Inline MUST show a quiet run of a renamed name once, by its new lines, played down, the name in its mark, with no
note of its own. The note of its rename MUST then be the button that shows the old lines of its quiet runs and hides
them again; ⌥-click and H take it as any note. A run MUST open with one note alone, the note of the first rename
with a site in it; a note with no run to open is plain words. Unified and Split MUST keep both lines of each site,
played down; their note opens nothing.

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
- **THEN** the note of the first rename of the run opens it, and the other note, opened, leaves it closed: each note says whether the lines it opens show
- **AND** a rename declared on a line that is read, whose one quiet line is the run of another note, has a note of plain words
  [@test] src/lib/diff/block-parts.test.ts (`opens a quiet run by one note alone, the note of the first rename with a site in it; a note with no run of its own is plain words`, `names the renames whose notes stand in one block in one note, at the first of their sites, and any other in its own`)

#### Scenario: A rename whose runs are labels
- **WHEN** the engine gives the runs of the renamed local of the template as labels, as it does until their kind is switched on
- **THEN** no line of them is shown once or played down, and the note is plain words
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)
  [@test] src/lib/diff/block-parts.test.ts (`shows a run of a rename that is a label as the lines of its block, the note before the lines of the declaration alone`)

### Requirement: Views: The find for an old name lands on the new name that stands for it

Where Inline shows a quiet run of a renamed name once, the find MUST read every old line of the run and find each
hit at the text of the new lines that stands for it, on the row of that line: a hit that holds a part of an old name
at the new name of its place, one hit at the first such name; any other hit at the same code, marked there, unless
the new lines hold that hit themselves. Where the old name shows (struck through in the row of a pair, in an old
line), the hits are in it.

#### Scenario: The hits of the old name
- **WHEN** the query is the old name, the new name, text both lines have, text that reaches into the old name, and text over two old names of a line, in Inline with the runs labels and quiet
- **THEN** the old name is one hit a site, of the old line, at the row that shows its new line; the new name a hit of the new line; the text both lines have one hit, of the new line
- **AND** a hit that reaches into the old name is the old line's, and one over two names is one hit, at the first
  [@test] src/lib/diff/find.test.ts (`finds the old name where the row of a pair of Inline strikes it through, and at the row of a quiet line shown once, whose new name stands for it`)

#### Scenario: The mark of a hit on a line shown once
- **WHEN** a hit of a hidden old line stands at a new name, two hits lie in one old name, or the hit is the current one
- **THEN** the new line carries one mark on the new name, as the current hit where either hit is, beside its own hits, and the old line that no row shows carries none
  [@test] src/lib/diff/find.test.ts (`gives the painter the hits of the hidden old lines as marks of the new lines that stand for them, the current one marked, two in one name as one`)

#### Scenario: A run wrapped otherwise
- **WHEN** a quiet run is one old line and two new ones, the name on the second, and the query is the old name, text only the old line holds in one piece, text both sides hold, and whitespace
- **THEN** the old name is found at the row of the new line that holds the new name, never at the first row of the run
- **AND** text only the old line holds in one piece is one hit, at the code of the new lines that stands for it, as far as its first line holds it, marked there; text both sides hold at one place is one hit, of the new line; whitespace before code that the new lines have whitespace before is the new line's, other whitespace is found before the code after it
- **AND** under the opened note every hit of the old line is in its own row
  [@test] src/lib/diff/find.test.ts (`finds the old name at the row whose new line holds the new name, not at the first row of the run`, `finds text that only the hidden old line holds in one piece at the code of the new lines that stands for it, and text both sides hold once`, `finds every hit of the old line in its own row once the note of the rename is opened`)
  [@test] src/lib/diff/rename-sites.test.ts (`reads each side as its characters outside whitespace and each renamed name as one unit: the same units on both sides, however the lines are wrapped`, `gives the units a range of a line holds a part of, and the unit at or after a place`)

#### Scenario: Two sides of other code
- **WHEN** the old lines of a run are not the code of its new lines but for the names, which no engine gives
- **THEN** every hit of the old lines outside a name still counts, at the first row of the run, with nothing marked
  [@test] src/lib/diff/find.test.ts (`counts every hit of an old line whose code is not the code of the new lines, which no engine gives, at the first row of the run`)

#### Scenario: A step in the review, a run wrapped otherwise
- **GIVEN** the review of a change whose renamed local is the argument of a call that the change wraps over two lines, with the kind `renamed` switched on
- **WHEN** the user finds the old name and steps twice, finds `(extra, `, a text the wrap took the space of, steps, finds `log(extra`, then opens the note of the rename
- **THEN** the old name counts three results, and the second step marks the new name on the third row of the run, the row of its new line
- **AND** `(extra, ` is one result, marked on `(extra,` of the row that shows the call; `log(extra` is one result, of the new line; the note counts none of them
- **AND** under the opened note `(extra, ` is marked in the old line, and no new line holds a hit
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`the find reads the hidden old lines of a run wrapped otherwise: the old name at the row of its new name, text only the old line holds at the code that stands for it`)

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

### Requirement: Views: H shows the old lines of the notes in view

In a review window H MUST show the old lines of every note in view that opens, and hide them again once they all
show, from the diffs and from the tree of the files; the view stays where it is and the focus where it was. A note
is in view with its row or with a line it hides or shows; one with neither MUST stay as it is. A note MUST name the
key after its words, and to a screen reader, only where the key works: the preview of a commit shows none.

#### Scenario: The key in the diffs and in the tree
- **GIVEN** a file with two wrapped calls, each a quiet run, below the first screen of the review
- **WHEN** the user presses H at the top of the review, shows the file, presses H twice, clicks the first note and presses H, then presses H with the focus on a row of the tree
- **THEN** the press at the top opens no note: both are closed once the file shows
- **AND** with both notes in view the first press shows the old line of each call, the second hides them, the diffs keeping the focus
- **AND** with one note open the press opens the other, and the press from the tree closes both, the row keeping the focus
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`H shows the old lines of the notes in view and hides them again, from the diffs and from the tree of the files`)

#### Scenario: The hint of the key
- **WHEN** a note that opens shows in a review window, and in the preview of a commit
- **THEN** the note of the review names the key H, the note of the preview names none
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`H shows the old lines of the notes in view and hides them again, from the diffs and from the tree of the files`, `Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)

#### Scenario: A line whose note is out of view
- **GIVEN** the review of a change whose renamed local has its last place forty added lines below its declaration, with the kind `renamed` switched on
- **WHEN** the view shows that last place, the note of the rename out of view, and the user presses H twice
- **THEN** the first press shows the old line of the place above its new one, the second hides it again, the diffs keeping the focus
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`H shows and hides the old line of a quiet site whose note is out of view`)

#### Scenario: The notes of the rows in view
- **WHEN** the notes of a range of rows are asked: rows around a quiet line of a rename forty lines below its note, the old line under the opened note, unchanged rows alone, and the rows of a wrapped call and of the lines a move took
- **THEN** a line shown once and a line under an opened note give the note that hides or shows them, a note among the rows itself, each once; unchanged rows, lines that are read and a layout that hides no line give none
- **AND** the rows in view are those whole below the header stuck at the top and above the end of the view
  [@test] src/lib/diff/rows.test.ts (`takes for the key of the notes the note of a line it hides or shows, wherever that note stands, beside the notes among the rows`)
  [@test] src/lib/diff/layout.test.ts (`gives the rows of a file that are in view: whole below the header stuck at the top, and above the end of the view`)
  [@test] src/lib/diff/block-parts.test.ts (`opens a quiet run by one note alone, the note of the first rename with a site in it; a note with no run of its own is plain words`)

#### Scenario: The rows of the notes
- **WHEN** the rows of a file with two quiet runs are built, the first note closed and opened, and in Unified
- **THEN** each note that opens has its row, the second one a row lower once the first is open, and a layout that hides no line has none
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a quiet run in Unified and Split, played down, after a note that opens nothing`)
  [@test] src/lib/diff/block-parts.test.ts (`counts the rows of each part: a note a row, a quiet run of Inline its new lines until its note is opened`)

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"), and a
renamed name "renamed" before it, once a name. A note that opens MUST be a button named by its words and by what a
press does, with its expanded state; a note of renames MUST say the renames it counts. A line of a move MUST say
which end of it the line is at and that it was edited on the way, and the place its note names is a button named by
where it goes.

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

### Requirement: Views: An old line shown once is found at the note that stands for it

An old line of a quiet run that Inline shows once, and one a move took, MUST resolve to the note that stands for it:
the scroll position keeps a note at the top by that line, a reveal of a name in it opens the note, as a fold, and
selects the name. An old line of a quiet run of a renamed name MUST resolve to a row of the new lines that stand for
it; a reveal in it opens the note of its rename. A row of words that stands for no line MUST stay at the top across
a new layout of the same rows.

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

#### Scenario: Lines that one column shows by the new ones alone
- **WHEN** the row of an old line of a block the whitespace policy ignores is asked, with as many lines on both sides and with fewer new ones, and of a quiet run of a renamed name wrapped over one line more
- **THEN** it is the row of the new line at its place where both sides have as many lines, else the first row of those lines
  [@test] src/lib/diff/block-parts.test.ts (`gives an old line of lines that one column shows by the new ones alone the row of the new line at its place, and the first row where the sides differ in their lines`, `shows a quiet run of a rename once in Inline, without a note of its own, until the note of the rename is opened; Unified and Split keep both lines`)
  [@test] src/lib/diff/rows.test.ts (`finds an old line of a block the policy ignores at the row that shows its new line, and at the first row of the block where the sides differ in their lines`)

#### Scenario: A line of a renamed name
- **WHEN** the row of an old line of a quiet run of a renamed name is asked, the note of the rename closed and opened, and the note that hides the line
- **THEN** it is the row of its new line while Inline shows the run once, far from the note, and the row of the old line once the note is opened
- **AND** the note that hides it is the note of the rename, for every line of its runs, and none for another line
  [@test] src/lib/diff/rows.test.ts (`shows the lines of a quiet rename once in Inline, which N and P pass, and opens them all by the one note of the rename`)
  [@test] src/lib/diff/block-parts.test.ts (`shows a quiet run of a rename once in Inline, without a note of its own, until the note of the rename is opened; Unified and Split keep both lines`)
