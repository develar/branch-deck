## MODIFIED Requirements

### Requirement: Files section: A file expands to its diff under its row

A file MUST expand to its diff under its row. A file expanded while the diffs of the commit are still read MUST have
its view at once, which says "Loading…" in a row, the word once the read took 300 ms; the rows of its diff MUST come
in that view, and what a reader has on screen below it MUST stay where it is.

#### Scenario: A file is expanded
- **WHEN** the user expands a file row
- **THEN** its diff shows under its row
  [@test] tests/e2e/commit-files/commit-files.spec.ts (`should list the files of the commit in the preview and load a diff when a file is expanded`)

#### Scenario: A file expanded while the diffs are read
- **GIVEN** a commit of a file of 122 lines and forty small ones, the read of its diffs held
- **WHEN** the user expands the first file, scrolls the preview until the row of a file further down is at its top, and the diffs arrive
- **THEN** the view of the file says "Loading…" in one row and is busy; its 122 rows come in place of that row, above the view
- **AND** the preview scrolls by as much, and the row at its top stays where it was
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a file opened while the diffs of the commit are read says so in its own view, and its rows come above the row a reader has at the top of the preview, which stays where it is`)

#### Scenario: The anchor of a file that shows no rows yet
- **WHEN** the top of a view is asked below a file whose diff is still read, and kept across the layout with its rows and the strip of their scrollbar, and back
- **THEN** the top is held from the end of the file: it moves by the rows that came and the strip, less the row that said the file is read, and returns to where it was when the rows go again
- **AND** a top in the row of the notice is held by the top of the file: the rows come below it
  [@test] src/lib/diff/layout.test.ts (`keeps a top below a file whose diff is still read from the end of the file: the rows that come, and the strip of their scrollbar, move it by as much`)
