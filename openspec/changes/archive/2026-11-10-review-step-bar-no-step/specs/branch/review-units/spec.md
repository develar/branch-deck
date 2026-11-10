## MODIFIED Requirements

### Requirement: In the Review Window: A block another section shows is one row

A block that another section shows MUST be one row in the section of its file, "Shown in Imports only": a group by its
name, on each of its files, never the count of its heading; no change for N and P.

#### Scenario: A block of the imports in its file's section
- **WHEN** the section of a file holds a block that the group of the imports shows
- **THEN** the block is one row there, "Shown in Imports only", without the count of its heading "Imports only (1)"
- **AND** N and P pass it, no change of that section
  [@test] src/lib/diff/rows.test.ts (`makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows`)
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)
