## MODIFIED Requirements

### Requirement: Title bar, footer and actions: The diff view is saved by the writer of the backend

Inline/Unified/Split MUST be the setting of every diff view, saved by the writer of the backend, which owns
`settings.json` ([Distributed Settings](../../../../docs/design/distributed-settings.md)): the window sends it the
change, and keeps the choice on its answer.

#### Scenario: The window chooses a diff view
- **WHEN** the user chooses Inline, Unified or Split in the window
- **THEN** the window sends the change to the writer of the backend, which saves it in `settings.json`
- **AND** the window keeps the choice on its answer
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should save the diff view the window chooses through the writer of the backend`)
