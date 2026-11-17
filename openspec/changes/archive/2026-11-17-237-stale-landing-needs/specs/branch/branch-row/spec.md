## MODIFIED Requirements

### Requirement: Status cell: A sync renders only what it changed

A sync MUST render only what it changed. A branch whose status alone changed renders the status of its row, nothing
else of the row; a branch whose commits, texts or remote status changed renders its row; a next step that changed
renders the step of its row; no other row renders, nor the branch table or a stack island whose rows stay.

#### Scenario: A sync that changes one branch, and one that changes nothing
- **WHEN** a sync changes the commits, texts or remote status of one branch
- **THEN** only the row of that branch renders
- **AND** of a sync that changes nothing (each branch `Created` to `Unchanged`; each step of the progress of a sync), only the status of each row renders
  [@test] tests/e2e/branch-row-layout/row-renders.spec.ts (`should render only the row of the branch a sync changed, and of a sync that changes nothing the status of each row`)

#### Scenario: The next steps keep the needs of the tips before while those of a sync are computed
- **GIVEN** Patronus answers, so the session computes the landing needs of the tips after each sync that moved one
- **WHEN** a sync finds one more commit on one branch, and the next step of every branch keeps the needs of the tips before meanwhile, then has the new ones, the same
- **THEN** of the steps, the rows and their status cells only those of that branch render
- **AND** neither the branch table nor a stack island renders: a row reads of the step the ring of its node, a status cell the status and what landed the branch, an island the hold of its root, each kept while it says the same
  [@test] tests/e2e/branch-row-layout/row-renders.spec.ts (`should render the step of the branch a sync of one more commit changed alone while the landing needs of its new tip are computed, no other row, no card`)
