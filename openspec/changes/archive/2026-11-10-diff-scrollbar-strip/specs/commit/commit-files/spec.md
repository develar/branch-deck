## ADDED Requirements

### Requirement: Files section: The scrollbar of a long line never lies over a row

The sticky horizontal scrollbar of a file whose side is wider than its column MUST take a strip of its own below the
rows of the file, so at the end of the file it lies under the last row and a control of that row (a fold, a place in
it) takes a click anywhere on it; while the rows go on below the view, it sticks at the bottom of the view.

#### Scenario: A click on the lower half of the fold at the end of a card
- **WHEN** a card whose old side is narrower than its longest line ends with a fold, and the user clicks the lower half
  of a place in that fold
- **THEN** the scrollbar stands below the fold, and the click shows the section the place names
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should take a click on the lower half of the fold at the end of a card, the scrollbar of its long lines in a strip of its own below it`)
  [@test] src/lib/diff/layout.test.ts (`keeps a strip for the scrollbar of a file wider than its columns below its rows, above the line at the end, which the offsets count`)
