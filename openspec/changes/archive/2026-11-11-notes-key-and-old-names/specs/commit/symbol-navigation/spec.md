## MODIFIED Requirements

### Requirement: The declaration shown is revealed with its name selected

The declaration shown MUST have its file open, the fold around it expanded, the view scrolled to it and its name
selected at its exact position on the line, once its row is painted with its text, however long its text takes, and
again when a new layout paints the row anew, until the user presses or types in the view or selects elsewhere. A name
of the old side on an unchanged line, which one column shows once by its new line, MUST be selected in the line shown,
at the columns the name has there.

#### Scenario: A declaration in a fold is revealed with its name selected
- **WHEN** a jump shows a declaration inside a fold
- **THEN** its file is open, the fold around it expanded and the view scrolled to it
- **AND** its name is selected at its exact position on the line once its row is painted with its text
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)
  [@test] crates/file-diffs/src/declarations_test.rs (`goes_outside_the_review_through_the_index_at_the_revision_of_the_side`)

#### Scenario: A new layout paints the row anew
- **GIVEN** the name of the declaration shown is selected
- **WHEN** a new layout paints its row anew, before the user presses or types in the view or selects elsewhere
- **THEN** the name is selected again at its position

#### Scenario: A name of the old side on a line one column shows by its new line
- **GIVEN** a call indented again, an unchanged line under Ignore Whitespace, and a jump from its name on the old side in Split
- **WHEN** the layout is Inline and the user goes Back
- **THEN** the name is selected in the new line of the row, four columns further than in the old line, and not the text at the columns of the old line
- **AND** its text is selected as any, though the press in Split kept the selection on the old side
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should select a name of the old side at its own columns in a line the whitespace policy ignores, which one column shows by its new line`)
  [@test] src/lib/diff/line-parts.test.ts (`places a name of the old line in the new line of a row the whitespace policy ignores, by the characters that are no whitespace`)
