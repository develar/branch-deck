## MODIFIED Requirements

### Requirement: Commits a running robot covers: The line of the robot runs over the commits it covers

On the rail of the branch, the line of the robot MUST run over the commits it covers: a line of
`--bd-lane-span-width` in `--bd-robot-line`, the width and the colour of its landing span, the colour of its ring,
from the node of the branch while it covers the first row, else from the newest covered commit, down to the last
covered one. The dots of the covered commits MUST be in that colour; a robot that will fail draws them `destructive`.

#### Scenario: Commits added since the robot started
- **WHEN** commits were added or changed since the robot started
- **THEN** the line of the robot runs over the commits it covers
- **AND** the newest covered commit below the added ones gets a dot in the colour of the line, also in a list that is
  oldest first, and the branch row keeps the plain line down from its node
  [@test] src/lib/robot-coverage.test.ts (`runs the line of the robot over the commits it covers and marks the ones added or changed since`, `finds the newest covered commit below the added ones also in a list that is oldest first`)
  [@test] src/lib/robot-coverage.test.ts (`gives the dots of the commits the robot took the colour of its line, and starts no line above a first row it lacks`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over the commits it took, and mark the one added after it started`)

#### Scenario: The robot covers every commit
- **WHEN** the robot covers every commit
- **THEN** the line runs from the top of the rail down to the last dot, every dot in its colour
  [@test] src/lib/robot-coverage.test.ts (`draws the line from the top of the rail down to the last dot while the robot covers every commit`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over every commit while it took the branch as it is`)

#### Scenario: The line starts at the node
- **WHEN** the robot covers the first expanded commit of the branch: its own, one a robot below lands (a side branch
  through the fork curve), or a branch of the whole stack
- **THEN** the line of the branch row from the node down is the robot's, one line from the ring into the rail, in the
  same device pixels on both sides of the border of the row, the ring, the line and the dots in one colour
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should start the line of the robot at the ring of its node, in the colour of the ring`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should run the line of the robot of a side branch from the fork through its node into its commits`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should start the line of the robot of the whole stack at the node of each expanded branch`)

#### Scenario: A robot that will fail
- **WHEN** a required check of the running robot failed
- **THEN** its line, its span in the graph and its dots are `destructive`, as its ring
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line and the dots of a robot that will fail in the colour of its ring`)
  [@test] src/lib/stack-lanes.test.ts (`marks the span of a robot that will fail, the lanes passing the rows between too, and leaves the others`)
