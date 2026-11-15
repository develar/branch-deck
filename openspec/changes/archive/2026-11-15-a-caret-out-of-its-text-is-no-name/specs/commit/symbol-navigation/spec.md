## MODIFIED Requirements

### Requirement: A name is under the pointer only where the pointer is on its text

The name under the pointer MUST be a name whose text the pointer is on. Beside the text of a line (its number, the
space before its first character, the blank end of its row) the pointer is on no name, though the nearest text is
one: the modifier MUST underline nothing there, and the menu of the code MUST have no name. A caret the engine gives
for the point outside its text (WebKit's `caretRangeFromPoint` past the end of a text, over code drawn anew) MUST be
no name, never an error of the page.

#### Scenario: The pointer between the number of a line and its first word
- **GIVEN** a line that starts with a word, and the modifier held with the pointer on a name, which is underlined
- **WHEN** the pointer moves into the space between the number of that line and the word
- **THEN** nothing is underlined and the pointer is no hand
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)

#### Scenario: The menu of the code on the number of a line
- **WHEN** the user opens the menu of the code on the number of a line that starts with a word
- **THEN** its items are disabled: the word is not under the pointer
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: A caret past the end of its text
- **GIVEN** the modifier held with the pointer on a name, which is underlined
- **WHEN** the engine gives the caret of the point past the end of its text
- **THEN** nothing is underlined, the pointer is no hand, and the page logs no error
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline no name where the engine gives the caret of the point past the end of its text, as WebKit did over code drawn anew`)
