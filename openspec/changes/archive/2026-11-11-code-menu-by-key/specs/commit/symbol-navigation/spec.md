## MODIFIED Requirements

### Requirement: The menu of the code acts on the name it opened on

The menu of the code opened by the pointer MUST act on the name under the pointer, whatever is selected elsewhere.
Where the pointer is on no name its items MUST be disabled, with the reason, also while a name is selected elsewhere.

#### Scenario: The menu on a struck-through name after a click elsewhere
- **GIVEN** a click left the caret in another row
- **WHEN** the user opens the menu of the code on the struck-through old name of an Inline row and chooses "Find Usages"
- **THEN** the usages listed are those of the old name
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a struck-through name of an Inline row alone under the pointer with the modifier, and go to the declaration of that old name on a click`)

#### Scenario: The pointer on no name while a name is selected elsewhere
- **GIVEN** a name is selected in the code
- **WHEN** the user opens the menu of the code with the pointer on the number of another line
- **THEN** "Go to Declaration" and "Find Usages" are disabled
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

## ADDED Requirements

### Requirement: A name is under the pointer only where the pointer is on its text

The name under the pointer MUST be a name whose text the pointer is on. Beside the text of a line (its number, the
space before its first character, the blank end of its row) the pointer is on no name, though the nearest text is
one: the modifier MUST underline nothing there, and the menu of the code MUST have no name.

#### Scenario: The pointer between the number of a line and its first word
- **GIVEN** a line that starts with a word, and the modifier held with the pointer on a name, which is underlined
- **WHEN** the pointer moves into the space between the number of that line and the word
- **THEN** nothing is underlined and the pointer is no hand
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should underline a name under the pointer with the modifier, go to its declaration in a fold on a click, and back`)

#### Scenario: The menu of the code on the number of a line
- **WHEN** the user opens the menu of the code on the number of a line that starts with a word
- **THEN** its items are disabled: the word is not under the pointer
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

### Requirement: The context menu key and Shift+F10 open the menu of the code on the name at the selection

While the view has the focus, the context menu key and Shift+F10 MUST open the menu of the code on the name at the
selection (a selected name, or the name the caret is in), under the start of that name, wherever the pointer is.
Where the selection holds no name the menu MUST open at the selection, or at the start of the view, its items
disabled. A menu key on a focused control of the view (the header of a file) is that control's.

#### Scenario: The context menu key at a caret
- **GIVEN** a click left the caret in a name, nothing selected, and the focus on the view
- **WHEN** the user presses the context menu key
- **THEN** the menu of the code opens under the start of that name, not at the middle of the view, "Go to Declaration" and "Find Usages" enabled
- **AND** Escape closes it and gives the focus back to the view
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: Shift+F10 and the keys of the menu
- **WHEN** the user presses Shift+F10, then ↓, ↓ and Enter
- **THEN** the menu opens under the name and the usages of that name are listed
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: A selected name while the pointer is on another
- **GIVEN** a name is selected and the pointer is over another name
- **WHEN** the user presses the context menu key and chooses "Go to Declaration"
- **THEN** the menu opens under the selected name and acts on it
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: A selection out of the view, or none
- **WHEN** a scroll took the selection out of the view, or nothing is selected in the view
- **THEN** the menu opens inside the view: at the edge nearest to the selection, or at the start of the view
  [@test] src/lib/diff/code-menu.test.ts (`opens for a menu key under the start of the name at the selection`, `opens for a menu key inside the view when a scroll took the selection out of it`, `opens for a menu key at the start of the view where nothing is selected in it`)

### Requirement: A menu key opens one menu

A menu key that opens a menu of the app (the menu of the code, the menu of a row of pull requests) MUST open no menu
of the engine beside it: its press is prevented, and so is the next release of the context menu key, at which Windows
opens the menu of that key at whatever has the focus by then. A key held down MUST open the menu once.

#### Scenario: The keys that ask for the menu
- **WHEN** a key is pressed
- **THEN** the context menu key and Shift+F10 ask for the menu; F10 alone and a key with Alt, Ctrl or Meta do not
  [@test] src/lib/menu-key.test.ts (`takes the context menu key and Shift+F10 for the menu, and no key with another modifier`)

#### Scenario: The release of the context menu key
- **GIVEN** the context menu key opened a menu of the app, which took the focus
- **WHEN** the key goes up
- **THEN** that release is prevented, wherever it lands in the window, and no other release is
- **AND** a window that lost the focus meanwhile waits for no release
  [@test] src/lib/menu-key.test.ts (`prevents the release of the context menu key once, wherever it lands in the page, and no other release`, `stops waiting for a release when the window loses the focus, not when an element of it does`)

#### Scenario: Shift+F10
- **WHEN** Shift+F10 opens a menu of the app
- **THEN** the menu opens at the point given, as a right click there, the press is prevented and no release is
  [@test] src/lib/menu-key.test.ts (`opens the menu of the trigger at the point, as a right click there, and prevents the press`, `prevents no release after Shift+F10, whose menu the engine opens as the key goes down`)

#### Scenario: A key held down
- **WHEN** the key repeats
- **THEN** the menu opens once, and no repeat opens a menu of the engine
  [@test] src/lib/menu-key.test.ts (`opens the menu once for a key held down`)

### Requirement: The view keeps its selection while a popup of it has the focus

The selection of the code MUST be as it was once the menu of the code or a list of declarations or usages closes and
the focus returns to the view, also in an engine that clears the selection of the page as the focus leaves it
(WebKit), so that the keys of the code read the same name again.

#### Scenario: The menu closed by Escape
- **GIVEN** the caret is in a name and the context menu key opened the menu on it
- **WHEN** the user presses Escape, then a menu key again
- **THEN** the menu opens on the same name again
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)

#### Scenario: An item that jumps nowhere
- **GIVEN** a name is selected and a menu key opened the menu on it
- **WHEN** "Go to Declaration" finds no declaration and the menu has closed
- **THEN** the name is selected as it was
  [@test] tests/e2e/commit-files/symbol-navigation.spec.ts (`should open the menu of the code by the context menu key and Shift+F10 on the name at the selection, under that name, and by the pointer on no name selected elsewhere`)
