## ADDED Requirements

### Requirement: Views: A rename has one note in the rows of a file, at its declaration

A view MUST name a rename once in the rows of a file: a note before the lines that hold its declaration, or its
first site where the rows do not show that block; a block of the imports alone that a review plays down has it only
when no other block holds a site. Its other sites MUST have no note, and a run of a renamed name that the rename
names none of its own.

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

#### Scenario: A run no rename names
- **WHEN** the marks hold a run of a renamed name and no rename with a site in it, which no engine gives, or the rows name no rename
- **THEN** the run has the note of its kind, "Renamed only", which opens it as the note of any quiet run
  [@test] src/lib/diff/block-parts.test.ts (`keeps the note of a run of a renamed name that the marks name no rename for, which opens it as any quiet run`)
  [@test] src/presenters/diff-marks.test.ts (`words a run of the kind renamed that names no rename by its kind, as any run`)

### Requirement: Views: The note of a rename says its two spellings and its sites

The note of a rename MUST say "Renamed", the old and the new spelling of the name with an arrow between them, and
how many places hold the name ("Renamed total → sum · 12 sites", "1 site"). Renames whose notes stand in one block
MUST share one note, at the first of their sites: the first is worded, the others are counted ("and 2 more") and
told on a hover.

#### Scenario: The words
- **WHEN** a note words a rename of twelve sites, and one of one site
- **THEN** it says "Renamed", the two spellings, and "12 sites" or "1 site"; as one sentence "Renamed total to sum, 12 sites"
  [@test] src/presenters/diff-marks.test.ts (`says the two spellings of a renamed name and how many places hold it, one site or many`)

#### Scenario: Several renames at one block
- **WHEN** two renames are declared on one line and a third in another block, and a note names two or three renames
- **THEN** the two share one note at the first of their sites, the third has its own
- **AND** the note words the first rename and says "and 1 more" or "and 2 more", each of the others told for the hover ("count → size · 3 sites") and in the name of the note
  [@test] src/lib/diff/block-parts.test.ts (`names the renames whose notes stand in one block in one note, at the first of their sites, and any other in its own`)
  [@test] src/presenters/diff-marks.test.ts (`words the first rename of a note that names several and counts the others, each told for a hover and in the name of the note`)

### Requirement: Views: A renamed name is drawn in a mark of its own, never as a change

The name at a site of a rename MUST be drawn in the mark of a renamed name on both sides, in every layout: on a
quiet line, on a line that is read and on a line with another change, whose other ranges stay. No range of the
engine inside the name is drawn. Its hover MUST say what it was ("Renamed from total") or, on the old side, what it
became ("Renamed to sum"). A name the file does not declare is no rename of its marks and shows as any change.

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

#### Scenario: The hover of a name
- **WHEN** the pointer rests on a renamed name of a new line, and of an old line
- **THEN** the hover says "Renamed from total", and "Renamed to sum"
  [@test] src/presenters/diff-marks.test.ts (`says what a renamed name of a line was on the new side, and what it became on the old one`)
  [@test] src/lib/diff/row-cells.test.ts (`names the sites of a line of a side, for the hover of a name and the find; a merge has none`)
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline names a rename once, before the line of its declaration, and shows each site as one row, the old name struck through before the new one, both in the mark of a renamed name`)

#### Scenario: A file that only uses the name
- **WHEN** the preview shows Report.kt of the template, which calls `connectStore` and declares none of it
- **THEN** no note of it words a rename and no name is in the mark of one: each call is a change, `openStore` struck through before `connectStore`
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a name that is not local is named at its declaration, which is read; a file that only uses the name shows its change as any`)

### Requirement: Views: The row of a pair of Inline strikes the old name of a rename through, in its mark

Where Inline shows a similar pair as one row, the row MUST show the old name of a site struck through before the new
one, each as a whole name in the mark of a renamed name, whatever letters the engine marked inside them; the other
changes of the pair show as before, and a range of the engine that reaches out of the name MUST be cut at the name.
Nothing of a line that is read is left out: the old name is selected, found and clicked there as any deleted part.

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

#### Scenario: A removed declaration that was renamed
- **GIVEN** a function renamed at its declaration, whose uses outside keep the old name, in a review with its plan
- **WHEN** the old name is shown from Possible Breakage
- **THEN** it is selected in the struck-through part of the row, copied and read by Find Usages as the name of the old line
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

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

### Requirement: Views: The mark of a renamed name does not rest on its colour

The mark of a renamed name MUST be a colour of its own, made of the quiet colour of each theme and so no colour of a
change, with a dotted line along its lower edge that no changed range has. It MUST be no text decoration: the syntax
underlines a reassigned variable.

#### Scenario: The light and the dark theme
- **WHEN** a renamed name shows on its inserted line in the light theme, and in the dark one
- **THEN** its colour is neither the colour of its line nor the colour of an inserted range nor that of an unchanged line, light in the light theme and dark in the dark one
- **AND** the dotted line is a layer of its background, which a changed range has none of
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`a renamed name has a colour of its own and a dotted line under it in the light theme, never the colour of a change`, `a renamed name has a colour of its own and a dotted line under it in the dark theme, never the colour of a change`)
