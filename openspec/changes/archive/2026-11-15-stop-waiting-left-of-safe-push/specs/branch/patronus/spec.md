## ADDED Requirements

### Requirement: Pushing into a branch whose Safe Push runs: What comes beside the step leaves the button under the pointer

Stop Waiting and the actions of a start whose answer was lost MUST come beside the Safe Push button on the side away
from the edge of its place that stands, so the button stays under the pointer that pressed it: on its left in the step
track of a row and of an island header (both stand at the end of the row) and in the footer of the preview, on its
right among the buttons of the robot details.

#### Scenario: Stop Waiting comes beside a pressed Safe Push
- **GIVEN** the pointer that pressed the Safe Push of a row rests on its button
- **WHEN** the start begins to wait for another holder of the repository lock, and later its push goes
- **THEN** Stop Waiting comes on the left of the button and goes again, the button where it was and under the pointer, its hover saying whom it waits for
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show Stop Waiting where the Safe Push button stays under the pointer that pressed it, its hover saying whom it waits for`)
