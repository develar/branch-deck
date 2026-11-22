# Proposal

## Why

A section of no change of its own, such as the place two functions left, showed the rows of the blocks another section
shows one under the other, "Shown in step 2" twice beside each other. And the note of the marks of a move at its new
place ("from Orders.kt:7") opened the first section of the file of the other end, a row that names another section,
not the section whose row stands for the place the code left: that row is a block of `elsewhere` of its section too, so
no section counted as showing the line.

## What Changes

- Rows side by side of the same words, the same words in full and the same section are one row, which stands for the
  lines of all their blocks (`rows.ts`, `joinedRows`); rows of two sections stay two.
- A section says which of its rows are its own (`SectionUnits.held`, the place a move left): a reveal of a line of such
  a block takes that section, so the jump from the other end of a move lands on its row, its fold opened.
- The section a reveal shows (a jump of the marks, Go to Declaration, Back and Forward) is the one the tree and the
  footer name, as after a click on a row: the view's own scroll reported no section at its top until the user scrolled.
- Words and layout of the page only.

Capabilities: `branch/review-units`.
