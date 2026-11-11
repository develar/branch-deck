## MODIFIED Requirements

### Requirement: A name of the old line is selected in the Inline row that shows it

A name of the old line whose line Inline shows as the row of a similar pair MUST be selected where that row shows it:
in the text the row shares with the new line, or struck through in a deleted part, which is selectable for as long as
the selection is that name. A name found by its word is found in the line itself, never in the text of the row.

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

#### Scenario: A name of a pair of another indent
- **GIVEN** a whitespace policy pairs a line with the line indented again and edited
- **WHEN** a name of the old line is placed in the row
- **THEN** it stands at its own characters, whatever the indent of the new line
  [@test] src/lib/diff/line-parts.test.ts (`places an offset of the old line by the text the lines share, whatever whitespace a policy ignored in it`, `gives the place of an offset of each line in the text of the row`)

## ADDED Requirements

### Requirement: The selected name of the old line is copied and read by the keys of the code

While the name of the old line that a reveal selected in its Inline row is selected, a copy MUST take it, and ⌘B, ⌥F7
and Back MUST read it as the name of the old line, on the old side. A modifier alone and the copy MUST NOT let the
name go.

#### Scenario: A copy and Find Usages of the selected old name
- **GIVEN** the old name of a row is selected in its struck-through part
- **WHEN** the user copies, then presses ⌥F7
- **THEN** the copy is the old name, which stays selected after the keys of the copy
- **AND** the usages are asked at the name of the old line, on the old side, not at the new name that stands after it
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should select the name of a removed declaration in the Inline row that strikes it through, until a press in the diffs`)

### Requirement: A struck-through name of an Inline row is the name of the old line

A name in a deleted part of the row of a similar pair in Inline MUST be the name of the old line, on the old side:
with the modifier held its underline is that part alone, and a ⌘-click (Ctrl-click off macOS) and the menu of the
code MUST act on it, never on the word of the new line beside it. The underline of a name of the new line MUST NOT
reach into a struck-through part before it.

#### Scenario: The old name under the pointer
- **GIVEN** a call whose name changed, its old and its new line one row in Inline, the old name struck through before the new one
- **WHEN** the pointer is on the old name with the modifier held, then on the new name
- **THEN** the underline is the old name alone, then the new name alone
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

#### Scenario: A click on the old name
- **WHEN** the user ⌘-clicks the old name, then goes Back
- **THEN** the declaration of the old name is asked on the old side and shown
- **AND** Back selects the old name in its struck-through part
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

### Requirement: The menu of the code acts on the name it opened on

The menu of the code MUST act on the name it opened on, whatever is selected elsewhere, and on the name at the
selection where it opened on no name (a key opens it at the caret).

#### Scenario: The menu on a struck-through name after a click elsewhere
- **GIVEN** a click left the caret in another row
- **WHEN** the user opens the menu of the code on the struck-through old name of an Inline row and chooses "Find Usages"
- **THEN** the usages listed are those of the old name
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)
