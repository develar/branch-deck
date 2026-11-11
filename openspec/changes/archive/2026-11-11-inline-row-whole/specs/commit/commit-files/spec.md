## ADDED Requirements

### Requirement: Files section: Inline scrolls a row as far as its deleted parts reach

The one column of Inline MUST scroll sideways as far as its widest row: the row of a similar pair holds the deleted
parts of its old line too, so it is longer than both lines, and its end is never out of reach of the scroll bar.

#### Scenario: A long line whose last name changes
- **GIVEN** a long line of a similar pair whose row in Inline is wider than its column and than both its lines
- **WHEN** the user scrolls the column to its end
- **THEN** the end of the row shows
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`Inline scrolls a row of a similar pair as far as its deleted part reaches, past the widest line of both texts`)
  [@test] src/lib/diff/inline-rows.test.ts (`counts the row of a similar pair wider than both its lines, a tab to its next stop`, `counts no row of a block that is no similar pair, that the policy ignores, or that deletes nothing`)

#### Scenario: The text of the row
- **WHEN** Inline lays out the row of a similar pair, for its width or its soft wraps
- **THEN** its text is the new line with the deleted parts of the old line before the inserted parts
  [@test] src/lib/diff/inline-rows.test.ts (`is the new line with the deleted parts of the old line before the inserted parts`)
