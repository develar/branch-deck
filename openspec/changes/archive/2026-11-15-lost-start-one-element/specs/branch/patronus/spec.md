## MODIFIED Requirements

### Requirement: Pushing into a branch whose Safe Push runs: The row of every window shows a lost start

The record of a lost start MUST stay after its run, as one of a process that was killed between its push and its start:
the row of every window MUST show "Start Unknown" in its slot as a status: the one button of the slot, disabled and not
dimmed, its reason its description, with "Open in Patronus" ("Open Pull Request #7" for a comment) and "Forget This
Start" beside it.

#### Scenario: The row of a lost start
- **GIVEN** a start whose answer was lost
- **WHEN** a window shows its row
- **THEN** the row shows "Start Unknown" in its slot ("Cannot tell whether the Safe Push of auth started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start; Patronus may still run it"), with "Open in Patronus" ("Open Pull Request #7" for a comment) and "Forget This Start" beside it; S says it can't start, and a start of `brd` or the app that needs what it holds is `busy` naming it ("Cannot tell whether the Safe Push of auth that `brd` (pid 4242) sent started", `brd operation forget <op>`)
- **AND** "Start Unknown" is the button of the slot with a ? icon, disabled and not dimmed: its reason shows on the hover and on the focus of the keyboard, and is its accessible description
  [@test] crates/domain-policy/src/actions_test.rs (`a_start_whose_answer_was_lost_offers_open_in_patronus_and_forget_this_start`)
  [@test] src/presenters/operations.test.ts (`words a start whose answer was lost: whether it started is not known, and how it lets go`)
  [@test] src/presenters/actions.test.ts (`offers Open in Patronus where its robot would be and Forget This Start, which says Patronus may still run it`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show a start whose answer was lost on its row, refuse a start of it, and start again once it is forgotten`)

#### Scenario: A start that turns lost under the focus
- **GIVEN** the keyboard on the slot of a Safe Push that `brd` runs
- **WHEN** its answer is lost
- **THEN** the slot says "Start Unknown" and keeps the focus, in both engines: it is the same element
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should keep the focus on the slot as the Safe Push bd runs turns into a start whose answer was lost`)

#### Scenario: A lost comment start
- **GIVEN** a comment start whose answer was lost
- **WHEN** the user looks at its row
- **THEN** "Open Pull Request #7" is offered for it
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should open the pull request of a comment start whose answer was lost`)

#### Scenario: A process killed between its push and its start
- **GIVEN** a `brd` was killed between its push and its start
- **WHEN** the app reads the operations registry
- **THEN** the record stays as a lost start, which the app settles and starts once
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once`)
