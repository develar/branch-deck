## ADDED Requirements

### Requirement: Title bar, footer and actions: The title bar shows the robot that lands the branch

The title bar MUST show after the stack bar the robot that lands what the review shows while it runs, as the Status
cell of its row does: the ring, the time, the line under it; its hover the popover of its details with "Open in
Patronus", a click opening it. Of a branch it is its own robot, else the running Safe Push that lands it with more
("lands with session"); of the whole stack its robot. Nothing shows while none runs, nor for a kept branch, a list of
commits or a pull request.

#### Scenario: The running Safe Push of the branch
- **WHEN** the review window shows a branch whose Safe Push runs
- **THEN** its title bar shows after the stack bar the ring, "18m · ~22m" and the line, as the row of the branch
- **AND** its hover is the popover of its details, whose "Open in Patronus" opens it, as a click on it does
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

#### Scenario: A robot that will fail
- **WHEN** a required check of the running robot failed
- **THEN** its ring and its line are `destructive`, its time "18m · Failing"
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the robot that will fail in the colour of a failure, Failing in place of the time left`)

#### Scenario: The Safe Push of a branch built on it lands it
- **WHEN** the review shows a branch that the running Safe Push of a branch built on it lands
- **THEN** the title bar shows that robot, "lands with session" after it, its details on hover
- **AND** a branch that no running robot lands shows none
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of a branch built on the branch that lands it, and nothing on a branch it doesn't land`)

#### Scenario: The robot of the whole stack
- **WHEN** the robot of the whole stack runs
- **THEN** the review of the whole stack shows it, and the review of a branch of it with "lands with the whole air stack"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the running Safe Push of the whole stack after the stack bar of its review, and of each branch of it, which it lands`)

#### Scenario: The narrowest window
- **WHEN** the stack bar and the actions leave the robot little room
- **THEN** the branches keep their names: the robot shows how long it runs first, "lands with session" before the time
  left, and with no room the ring and the line alone, the name of the robot saying its time
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

## MODIFIED Requirements

### Requirement: Title bar, footer and actions: The title bar shows the stack bar, the diff view and the actions

The title bar of the window ([Title Bar](../../repository/title-bar/spec.md)) MUST show the stack bar and after it the
robot that lands the target branch while it runs, then at its other end the Split/Unified/Inline toggle and the
actions on the target branch.

#### Scenario: The title bar of the review window
- **WHEN** the review window of a branch shows
- **THEN** its title bar shows the stack bar, then at its other end the Split/Unified/Inline toggle and the actions on the target branch
- **AND** after the stack bar the robot that lands the target branch while it runs
  [@test] tests/e2e/repository/title-bar.spec.ts (`should start the row of a review window after the traffic lights, its empty parts dragging the window`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should show the running Safe Push of the branch after its stack bar as its row does, its details with Open in Patronus on hover, a click opening it`)

### Requirement: Title bar, footer and actions: Open Pull Request is an action of a branch with one

`O Open Pull Request #42` MUST be an action of the title bar when the branch has a pull request: the item of its
menu, the number saying which one it opens.

#### Scenario: A branch with a pull request
- **WHEN** the target branch has a pull request
- **THEN** the title bar offers `O Open Pull Request #42`
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should build the actions of the title bar of each part from the window's own session, and open the pull request itself`)
