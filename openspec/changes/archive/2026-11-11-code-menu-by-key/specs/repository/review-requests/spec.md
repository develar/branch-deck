## MODIFIED Requirements

### Requirement: Card: The menu of a row holds the actions of the review

The right-click, the context menu key and Shift+F10 MUST open the menu, the actions of the review
(`getReviewRequestActions`): Review (↵), Approve…, Request Changes…, Open Pull Request (O), Copy Pull Request Link,
Copy Branch Name (copies without a toast). A key MUST open that menu alone
([A menu key opens one menu](../../commit/symbol-navigation/spec.md#requirement-a-menu-key-opens-one-menu)).

#### Scenario: The menu from the mouse and the keyboard
- **WHEN** the user right-clicks a row, or presses the context menu key or Shift+F10 on it
- **THEN** the menu opens with Review (↵), Approve…, Request Changes…, Open Pull Request (O), Copy Pull Request Link and Copy Branch Name
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open a pull request on GitHub from its button, a ⌘-click and its menu`, `should move with the arrows, review with Enter and open the menu from the keyboard`)

#### Scenario: The release of the context menu key
- **GIVEN** the context menu key opened the menu of a row
- **WHEN** the key goes up, the focus in the menu by then
- **THEN** the release opens no menu of the engine beside it
  [@test] src/lib/menu-key.test.ts (`prevents the release of the context menu key once, wherever it lands in the page, and no other release`)
