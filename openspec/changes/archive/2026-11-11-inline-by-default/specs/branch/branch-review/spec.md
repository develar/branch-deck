## MODIFIED Requirements

### Requirement: Title bar, footer and actions: The title bar shows the stack bar, the diff view and the actions

The title bar of the window ([Title Bar](../../repository/title-bar/spec.md)) MUST show the stack bar and after it the
robot that lands the target branch while it runs, then at its other end the Inline/Unified/Split toggle and the
actions on the target branch.

#### Scenario: The title bar of the review window
- **WHEN** the review window of a branch shows
- **THEN** its title bar shows the stack bar, then at its other end the Inline/Unified/Split toggle and the actions on the target branch
- **AND** after the stack bar the robot that lands the target branch while it runs
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)
