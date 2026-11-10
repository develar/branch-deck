## MODIFIED Requirements

### Requirement: In the Review Window: The keyboard opens the card of a mark by I

The keyboard MUST reach the card: `I` on a focused row with a mark MUST open its card with the focus on its first file,
on the card when it lists none; Enter on a file selects it as a click does, the focus on its row. Escape MUST close
the card alone, not the window or the review, and give the focus back to the row. The key hints MUST list `I` while a
row of the tree has the focus.

#### Scenario: I, Enter and Escape on a card
- **GIVEN** a focused row with a mark
- **WHEN** the user presses `I`
- **THEN** its card opens with the focus on its first file, on the card when it lists none
- **AND** Enter on a file selects it as a click does, the focus on its row, so one row is marked
- **AND** Escape closes the card alone, not the window or the review, and gives the focus back to the row
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should open the card of a row's mark by I on the focused row, the focus on its first file, Enter selecting it and Escape giving the focus back to the row`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should give the focus to the card of a row's mark that lists no file, its Escape the card's and not the window's`)

#### Scenario: The key hints
- **WHEN** a row of the tree has the focus
- **THEN** the key hints list `I`
