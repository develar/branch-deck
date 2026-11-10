## MODIFIED Requirements

### Requirement: Timing: The row says how long the robot runs and how long it has left

The row and the lists of robots MUST say how long the robot runs and how long it has left: "18m · ~22m" (the time left
rounded up; "about 22 minutes left" in the hover and for the screen reader); past its usual time, the usual time
instead, "1h 36m · usually ~1h 14m" (the minutes after the hour as within it, the hours only from 10 hours:
"14h · usually ~1h"); without history, "18m".

#### Scenario: A robot within its usual time
- **WHEN** a robot runs within its usual time
- **THEN** the row says "18m · ~22m", and "about 22 minutes left" in the hover and for the screen reader
  [@test] src/lib/robot-eta.test.ts (`says how long it runs and how long it usually takes`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)

#### Scenario: A robot past its usual time
- **WHEN** a robot runs longer than usual
- **THEN** the row names the usual time, "55m · usually ~40m", with the minutes after the hour once it runs an hour,
  "1h 36m · usually ~1h 14m", and the hours only from 10 hours, "14h · usually ~1h"
  [@test] src/lib/robot-eta.test.ts (`names the usual time once a robot runs longer, with the minutes after the hour until it runs 10 hours`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)

#### Scenario: The time left
- **WHEN** a robot runs within its usual duration, past it, or without history
- **THEN** the time left counts down until the usual duration, indeterminate past it, and the row says "18m" without
  history
  [@test] src/lib/robot-eta.test.ts (`counts the time left until the usual duration, indeterminate past it and without history`)
