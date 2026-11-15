## MODIFIED Requirements

### Requirement: Move operation: The wait of a move shows on the row and Stop Waiting ends it

A move started from a menu MUST show its spinner on the "⋯" of its row, whose hover says whom it waits for and whose
click offers Stop Waiting (`cancel_operation`), which ends the wait with nothing moved; the surface that started it
MUST say the same, where it moves no button the pointer pressed: on the line of its buttons, before them, or below them.

#### Scenario: The wait of a move started from a menu
- **WHEN** a move started from a menu waits for the lock
- **THEN** the "⋯" of its row shows its spinner, whose hover says whom it waits for ("Waiting for `brd pick` (pid 4242, for 12 s)")
- **AND** its click offers Stop Waiting (`cancel_operation`), which ends the wait with nothing moved ("Canceled moving 1 commit to docs before it started")
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved`)
  [@test] crates/test-server/src/invoke_test.rs (`stop_waiting_ends_the_wait_of_a_rewrite_with_nothing_changed`)
  [@test] src/presenters/lock-wait.test.ts (`offers Stop Waiting of an operation of the session only while it waits for a holder, and stops it by its id`, `says that an action stopped while it waited for the repository lock changed nothing`)

#### Scenario: The surface that started it says the same
- **WHEN** a rewrite of the engine waits
- **THEN** the surface that started it says whom it waits for, and its Stop Waiting stops the wait by its operation
- **AND** the notice comes where the button the pointer pressed stays: on the line of the buttons of the drop and of the squash of the fixups, before them, below the input of the move to a branch
  [@test] src/lib/rewrite-run.test.ts (`shows whom a rewrite of the engine waits for and stops the wait by its operation`)
  [@test] src/lib/rewrite-run.test.ts (`says behind which rewrite of this window it waits in the queue of the session`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should say whom a drop waits for on the line of its buttons, Drop under the pointer, and stop it`, `should say whom a move to a branch waits for below its input, Move under the pointer, and stop it`, `should say whom a squash of fixups waits for on the line of its buttons, Squash under the pointer, and stop it`)
