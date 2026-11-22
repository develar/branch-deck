## MODIFIED Requirements

### Requirement: In the Review Window: A block another section shows is one row

A block that another section shows MUST be one row in the section of its file, "Shown in Imports only": a group by its
name, on each of its files, never the count of its heading; no change for N and P. A block of a slice that holds the
move alone (`moved_out`) MUST be such a row that says what moved and where ("Moved formatTotal to Report.kt"), another
step by its number; in a fold the words of the row. Rows side by side of the same words and section MUST be one row.

#### Scenario: A block of the imports in its file's section
- **WHEN** the section of a file holds a block that the group of the imports shows
- **THEN** the block is one row there, "Shown in Imports only", without the count of its heading "Imports only (1)"
- **AND** N and P pass it, no change of that section
  [@test] src/lib/diff/rows.test.ts (`makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows`)
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: A block of the place a declaration left
- **WHEN** the section of `Orders.kt` holds the block `parseTotal` and `parseSum` left for the new `Parsing.kt`, the one
  `formatTotal` left for `Report.kt` in another step, and the section of the deleted `Legacy.kt` the block `legacyTotal`
  left for `Orders.kt`
- **THEN** each is one row, "Moved parseTotal and parseSum to Parsing.kt", "Moved formatTotal to Report.kt in step 1",
  "Moved legacyTotal to Orders.kt", none of their lines shown
- **AND** N and P pass it, no change of that section; a fold that hides it says its words
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names the step of where a declaration went in the row of the place it left, when it is another, and in a fold the row itself`)

#### Scenario: Rows side by side
- **WHEN** a section has no change of its own, and two blocks of its file that a section of step 2 shows stand beside
  the place two functions left
- **THEN** it shows "Shown in step 2" once, for both blocks, and "Moved parseTotal and parseSum to Parsing.kt"
- **AND** two blocks that two sections of step 2 show stay two rows, and a fold names each place once
  [@test] src/lib/diff/rows.test.ts (`makes the rows side by side of a section of no change of its own of the same words and section one row, and keeps those of another section apart`)
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)

### Requirement: In the Review Window: A file of the tree opens its first open section

A file of the tree MUST open its first open section (of its concern), of a file in parts its first open part, a
place of the card the section that shows its line; a line of a block a section's own row stands for (the place a move
left) is shown by that section, never by a row of another section that names it. The section a jump shows is the one
the tree and the footer name.

#### Scenario: A file of the tree whose change is in two sections
- **GIVEN** a file whose own change is in one section and whose repeated edit is in the group of the edit
- **WHEN** the user clicks its row in the tree
- **THEN** its first open section (of its concern) opens
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: A file in parts
- **WHEN** the user clicks the row of a file the story shows in three parts
- **THEN** its first part opens, "part 1 of 3" in the footer
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

#### Scenario: A place of the card
- **WHEN** the user opens a place of the card
- **THEN** the section that shows its line opens

#### Scenario: The other end of a move
- **WHEN** the user clicks "from Orders.kt:7", the note of the marks of the move of `formatTotal` in `Report.kt`, whose
  place in `Orders.kt` is the row "Moved formatTotal to Report.kt in step 1" in the fold of the section of step 2
- **THEN** the fold opens and the jump lands on that row, marked, not on the row "Shown in step 2" of the section of
  step 1; the tree and the footer follow the section a jump shows, as they follow the click of a row
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
