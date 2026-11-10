## MODIFIED Requirements

### Requirement: In the Review Window: A click on the row opens the other section

A click on the row of a block another section shows, or on its place in a fold, MUST open that section, expanded, as
the section at the top: the tree and the footer follow it.

#### Scenario: The row of the imports and the row of a repeated edit
- **WHEN** the user clicks the row of a block another section shows
- **THEN** that section opens, expanded
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`, `should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: The place of a block in a fold
- **WHEN** the user clicks the place of a block in a fold ("1 change in step 1")
- **THEN** that section opens, expanded, and the footer names it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)

### Requirement: In the Review Window: A file of the tree opens its first open section

A file of the tree MUST open its first open section (of its concern), of a file in parts its first open part, a
place of the card the section that shows its line.

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
