## MODIFIED Requirements

### Requirement: The stack and the commits: A moved head or another part keeps the review on screen

A sync that moves a head of the part, or another part chosen in the stack bar, MUST keep the review on screen (the
tree with the steps of its plan and their commits, the column, the files, the diffs, the file on screen and the
position of the footer) until the files, the diffs and the plan of the new changes are read, which the window reads
ahead as it reads the neighbours of a commit. A switch of the review plan is no new review: the plan of the review on
screen MUST show as it is read.

#### Scenario: A sync moved the head
- **WHEN** a sync moves a head of the part, or the user chooses another part in the stack bar
- **THEN** the review on screen stays (the tree with the steps of its plan and their commits, the column, the files, the diffs, the file on screen, the position of the footer)
- **AND** it stays until the files, the diffs and the plan of the new changes are read
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should keep the review on screen after a sync that moved the head until the files, the diffs and the plan of the new head are read, then show them at once`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should review a stacked branch on its parent, then the whole stack, by owner`)
  [@test] src/lib/review-plan-answer.test.ts (`ends the read of a plan whole, disabled, a failure or no plan, never with a plan of other changes or one that still waits for the index`)

#### Scenario: The review plan told after the review showed
- **WHEN** the session of the window tells it the repository turns the review plan on only after the window read and showed its review without the plan
- **THEN** the plan of the review on screen shows as it is read: "Finding the tests of the change…" and the uses left outside while the tests of its steps are asked, the body of the review not `aria-busy`
- **AND** then the plan whole, with the tests that reach each step
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show the plan that waits for the tests of its steps in a window whose session told the review plan after it showed the review`)
