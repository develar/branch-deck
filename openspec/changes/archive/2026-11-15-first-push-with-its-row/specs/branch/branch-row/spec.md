## MODIFIED Requirements

### Requirement: Actions cell: No push button until the landing answers are in

Until Patronus and GitHub answered whether its landing pushes it, such a branch MUST have no button either: one shown
before their answers would leave with them and move the columns under the pointer. Its push track names that answer
(`data-waits-for`). A repository on neither Space nor GitHub MUST show its first push with its row, as its session
answers from git; elsewhere the settled layout keeps the track from the next start on.

#### Scenario: A repository on no service
- **WHEN** the repository is on neither Space nor GitHub, the answers of the services held
- **THEN** the first push of a branch comes with its row, and the status stays under the pointer, its tooltip open, as the services answer
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should show the first push of a branch with its row in a repository on no service, the status under the pointer as the services answer`)

#### Scenario: A first push that waits
- **WHEN** GitHub or Patronus has not answered whether the landing of a branch the remote lacks pushes it
- **THEN** its push track names that answer (`data-waits-for`) and holds no button, until the answer comes
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should name the answer a first push waits for on its track, and show the push with it`)

#### Scenario: The facts come back after a start
- **WHEN** the app starts and Your Turn, the pull requests and the approvals come back
- **THEN** the rows and the columns stay where the last start left them
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back`)
