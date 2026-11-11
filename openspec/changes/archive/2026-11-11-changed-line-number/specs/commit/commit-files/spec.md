## MODIFIED Requirements

### Requirement: Files section: The colour of a line fills its line number

The colour of a line MUST fill its line number; no bar stands beside the code. The number of a changed line MUST read
on that colour, at least 3:1 against it in the light and in the dark scheme; the number of an unchanged line keeps the
quiet colour of the editor's line numbers.

#### Scenario: A changed line
- **WHEN** a diff shows a changed line
- **THEN** its colour fills its line number, and no bar stands beside the code

#### Scenario: The number of a deleted, an added and a paired line
- **WHEN** Inline shows a deleted row, an added row and the row of a similar pair, in the light or the dark scheme
- **THEN** the number of each is at least 3:1 against the colour of its line
- **AND** it is another colour than the number of an unchanged line
  [@test] tests/e2e/commit-files/diff-font.spec.ts (`shows the number of a changed line in a colour that reads on the colour of its line, light and dark`)
