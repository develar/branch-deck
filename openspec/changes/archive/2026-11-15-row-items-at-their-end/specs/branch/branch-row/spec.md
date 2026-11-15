## ADDED Requirements

### Requirement: Columns: An item that comes or goes moves only what stands before it

The items of Status and Actions MUST stand at the end of their cell, also where they are narrower than the head of the
column, as the row stands at its end: an item that comes or goes in a row moves only what stands before it, and "⋯"
stays at the end of the row whatever the rows of the view show.

#### Scenario: The last push of the view ends
- **GIVEN** one row of the view shows a push, no row a step, and the pointer rests on "⋯" of that row
- **WHEN** its push ends and the push track takes no room
- **THEN** "⋯" stays where it was, under the pointer
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`keeps ⋯ at the end of the row under the pointer as the last push of the view ends`)
