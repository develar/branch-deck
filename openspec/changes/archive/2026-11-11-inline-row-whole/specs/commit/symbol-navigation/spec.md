## ADDED Requirements

### Requirement: A name of the old line is selected in the Inline row that shows it

A name of the old line whose line Inline shows as the row of a similar pair MUST be selected where that row shows it:
in the text the row shares with the new line, or struck through in a deleted part, which is selectable for as long as
the name is selected. A name found by its word is found in the line itself, never in the text of the row.

#### Scenario: A removed name the row strikes through
- **GIVEN** a declaration whose name changed, its old and its new line one row in Inline
- **WHEN** the old name is shown (a removed declaration of Possible Breakage)
- **THEN** it is selected in the struck-through part of the row, and not the name that replaced it
- **AND** after a press in the diffs the deleted part is left out of a selection again
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

#### Scenario: A name after a deleted part of its row
- **GIVEN** a declaration whose line lost a modifier before its name, one row in Inline with the modifier struck through
- **WHEN** the name is shown by its word
- **THEN** the name is selected, not the text as many columns further as the deleted part is long
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold a widening under the verdict that no use left outside breaks, never under Possible Breakage, and name it opened wider at its declaration`)
