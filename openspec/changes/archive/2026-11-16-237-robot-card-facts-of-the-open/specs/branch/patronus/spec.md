## MODIFIED Requirements

### Requirement: Timing: The open details show the estimate of the CI in the row Progress

While the details of a running robot are open, they MUST show the estimate of the CI in the row "Progress": the time
left of the running check that ends last (its estimate minus its running time) and its name and percent, "about 9 min
left · Unit Tests 93%"; "1 check queued" when checks wait in the queue; "Overtime 4 min" when every running check ran
past its estimate; "probably hanging" in `warning` when the CI says so; until the CI tells it, the time left from history.

#### Scenario: The slowest check and the queued ones
- **WHEN** the CI estimates the running checks of the robot
- **THEN** the row "Progress" says "about 9 min left · Unit Tests 93%" for the check that ends last
- **AND** "1 check queued" when checks wait in the queue
  [@test] src/lib/robot-eta.test.ts (`names the time left, the check the robot waits for, and the queued checks`)

#### Scenario: Every running check ran past its estimate
- **WHEN** every running check ran past its estimate
- **THEN** the row says "Overtime 4 min"
  [@test] crates/patronus/src/actions_test.rs (`a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did`)

#### Scenario: A check probably hangs, or the CI tells nothing
- **WHEN** the CI says a check probably hangs, or tells none of it
- **THEN** the row says "probably hanging" in `warning`, or the time left from history
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should restart it from its details: canceled first, then started again on the commit it tested`)

### Requirement: Timing: The time of the row counts down what the open details read

While the details are open, the time and the progress line of the row MUST count down the time left it read meanwhile
("20m · ~9m left"), and the start line of the details says only the usual time ("usually ~1 h 50 min"); once the
details closed, the time and the line MUST take the estimate from history again. A running robot row of the landing
overview MUST show the check and the queued checks too.

#### Scenario: The details are open
- **WHEN** the details read the estimate of the CI
- **THEN** the slot counts down the time left it read, "20m · ~9m left"
  [@test] src/lib/robot-eta.test.ts (`counts down the time left it read while the details were open, for the slot`)

#### Scenario: The details close
- **WHEN** the details close
- **THEN** the time and the line take the estimate from history again
  [@test] tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)

### Requirement: Timing: The details say what the estimate is of

The details of a running robot MUST say the usual time after the start time, with what it is of, as the card knew it
as it opened (the time left is in the row Progress); the slot doesn't name it. The details MUST have the profile row
when the profile is known, also before the details answered, and the start line names no profile that row names; a
robot row of the landing overview names it on hover of its time: "Safe Push · Large Scale Change".

#### Scenario: The estimates of the details
- **WHEN** the details of a running robot show
- **THEN** the start line says "Today 14:02 · usually ~1 h 50 min for Large Scale Change", "… without a merge profile",
  the rough "… · usually ~40 min for Safe Push", or "… · no Safe Push finished in the last 30 days to estimate from"
- **AND** the row Progress the time left, "about 1 h 32 min left", "55 min, longer than usual" or "running 18 min"
  [@test] src/lib/robot-eta.test.ts (`explains the estimate and what it is of in the details, the time left apart`)

#### Scenario: The profile row names the profile
- **WHEN** the profile of the robot is known, also before the details answered
- **THEN** the details have the profile row, and the start line says "Today 14:02 · usually ~1 h 50 min"
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`, `should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

#### Scenario: A robot row of the landing overview
- **WHEN** the user hovers the time of a robot row of the landing overview
- **THEN** it names the profile, "Safe Push · Large Scale Change"

#### Scenario: The details of a running robot come after its card opened
- **GIVEN** the card of a running robot open under its chip, the pointer on its Cancel, its details still to come
- **WHEN** they come with the estimate of the CI and the checks
- **THEN** the row Progress says the estimate of the CI, the checks come past the actions, the start line and the
  profile known as the card opened stay as they were, and Cancel keeps its box, in both engines
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should keep Cancel of the card of a running robot under the pointer as the details of its robot come, its start line as it opened`)
