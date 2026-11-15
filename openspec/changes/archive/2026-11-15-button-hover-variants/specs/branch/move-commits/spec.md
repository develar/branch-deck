## MODIFIED Requirements

### Requirement: Move operation: A wait a surface started is heard and reached by key

A surface that started a rewrite which waits for the lock MUST tell a screen reader whom it waits for, once per
holder, in a polite live region there before the wait; its Stop Waiting MUST take the focus the press left on the
surface, so Enter or Space stops the wait without a Tab, and as the wait ends the focus MUST go back where it came
from, else to the button that waited, which stays focusable while busy and takes no hover and no press.

#### Scenario: A screen reader hears whom for
- **WHEN** a rewrite a surface started waits for the lock
- **THEN** its live region says whom it waits for without how long, and without the backticks of the command line, which a screen reader reads aloud ("Waiting for brd pick (pid 4242)"), the same words while the holder stays
- **AND** the region is empty once the wait ended
  [@test] src/presenters/lock-wait.test.ts (`tells a screen reader whom an operation waits for without how long, so the words change with the holder only`)
  [@test] tests/e2e/amend-changes/amend-changes.spec.ts (`should take the keys to Stop Waiting in the progress row of a form whose amend waits for the lock, and give them back to the form`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should say whom a drop waits for on the line of its buttons, Drop under the pointer, and stop it`, `should say whom a move to a branch waits for below its input, Move under the pointer, and stop it`, `should say whom a squash of fixups waits for on the line of its buttons, Squash under the pointer, and stop it`, `should say whom a squash waits for in the footer of its dialog, Squash under the pointer, and stop it`, `should say whom a reword waits for in the footer of the preview, Save under the pointer, and stop it`)

#### Scenario: Stop Waiting takes the keys
- **GIVEN** the press left the focus on the pressed button, which stays focusable while busy (a press of the pointer in Chromium, a key), elsewhere in its form, dialog or popover, or nowhere (a press of the pointer in WebKit)
- **WHEN** the wait shows
- **THEN** Stop Waiting has the focus, and Enter or Space stops the wait
- **AND** the focus goes back where the press left it, else to the button that waited: the next press needs no Tab, in both engines
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should take the keys to Stop Waiting as a move pressed by Enter waits for the lock, and give them back to its input as Space stops it`, `should stop the wait of a squash with Stop Waiting in its dialog, squash nothing, and squash on the next press`, `should say whom a drop waits for on the line of its buttons, Drop under the pointer, and stop it`, `should say whom a squash waits for in the footer of its dialog, Squash under the pointer, and stop it`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should say whom a drop waits for on the line of its buttons, Drop under the pointer, and stop it`, `should say whom a move to a branch waits for below its input, Move under the pointer, and stop it`, `should say whom a squash of fixups waits for on the line of its buttons, Squash under the pointer, and stop it`, `should say whom a squash waits for in the footer of its dialog, Squash under the pointer, and stop it`, `should say whom a reword waits for in the footer of the preview, Save under the pointer, and stop it`)

#### Scenario: The progress row of a form
- **GIVEN** a form that its progress row replaces as its rewrite runs (the amend of the uncommitted changes), the focus of the form on the row
- **WHEN** the rewrite waits for the lock
- **THEN** the Stop Waiting of the row has the focus, and Enter stops the wait
- **AND** the form comes again with the focus in it, never on the page, in both engines
  [@test] tests/e2e/amend-changes/amend-changes.spec.ts (`should take the keys to Stop Waiting in the progress row of a form whose amend waits for the lock, and give them back to the form`)

#### Scenario: A busy button under the pointer
- **GIVEN** the pressed button busy, its wait for the lock shown
- **WHEN** the pointer hovers it and presses it
- **THEN** its look stays: no hover background, no shift of a press, not dimmed (its spinner says that it is busy), in both engines
- **AND** a hover its caller gives (the colour of a push) comes neither: the button keeps its background at rest
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should change nothing of a busy button under the pointer, neither by its hover nor by a press`, `should give a busy push no hover of its colour, its background at rest kept`)
