## MODIFIED Requirements

### Requirement: Failed: A failure is an error toast with the next action

The failure MUST be an `error` toast "Safe Push of auth failed" / "Fast Tests · auth → master" (the checks that failed
from its details, else what it tested), with "Start in Air" first and "Copy Prompt for Agent" second, in the place of
"Open in Patronus", or "Restart Safe Push" (the action of the robot, `getRobotActionItems`, where a restart is
possible) when its builds never started
([Safe Push Failure Prompt](../safe-push-failure-prompt/spec.md#the-failed-toast)).

#### Scenario: A running Safe Push failed
- **WHEN** a running Safe Push failed
- **THEN** the `error` toast tells it once with the failed checks, and its first action, "Start in Air", opens the New Session tab in the IDE, with "Copy Prompt for Agent" as the second option
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and offer Start in Air in the toast, with Copy Prompt for Agent as the second option`)
  [@test] src/stores/patronus.test.ts (`tells once that a robot failed, with Start in Air and Copy Prompt for Agent beside it, and the failed checks, once its problems are read`)

#### Scenario: Its builds never started
- **WHEN** a Safe Push failed whose builds never started
- **THEN** the toast offers "Restart Safe Push", which restarts it, and "Open in Patronus" below the text
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart from the toast a Safe Push whose builds never started`)

#### Scenario: Start in Air is not offered
- **WHEN** a Safe Push failed and the robot offers no "Start in Air"
- **THEN** the toast offers "Copy Prompt for Agent" first, and "Open in Patronus" below the text
