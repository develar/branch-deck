## MODIFIED Requirements

### Requirement: Copying: Start in Air is the primary way to fix the failure

"Start in Air" SHALL be the primary way to fix the failure (it opens the New Session tab in the IDE that has the
repository open); the prompts are for a user who copies them into an agent elsewhere.

#### Scenario: Start in Air comes before the prompts
- **GIVEN** a failed robot
- **WHEN** its actions are offered
- **THEN** "Start in Air" comes before both prompts
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

### Requirement: The failed toast: The toast offers Start in Air first and the prompt second

The toast "Safe Push of auth failed" MUST offer "Start in Air" first (it opens the New Session tab in the IDE to fix
the failure), with "Copy Prompt for Agent" as the second option (for a user who copies the prompt into an agent
elsewhere). "Start in Air" shows a toast of its own ([Start in Air](../start-in-air/spec.md), "Opening in Air…"); the
copy turns the toast in place into "Copied the prompt for auth" / "Paste it into your coding agent".

#### Scenario: The toast of a failed robot
- **WHEN** the Safe Push of `auth` fails
- **THEN** the toast "Safe Push of auth failed" offers "Start in Air" first, with "Copy Prompt for Agent" as the second option
  [@test] src/stores/patronus.test.ts (`tells once that a robot failed, with Start in Air and Copy Prompt for Agent beside it, and the failed checks, once its problems are read`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and offer Start in Air in the toast, with Copy Prompt for Agent as the second option`)

#### Scenario: Start in Air from the toast
- **WHEN** the user chooses "Start in Air" in the toast
- **THEN** a toast of its own says "Opening in Air…", and then what the IDE answered

#### Scenario: The toast turns in place after the copy
- **WHEN** the user chooses "Copy Prompt for Agent" in the toast of `auth`
- **THEN** it turns in place into "Copied the prompt for auth" / "Paste it into your coding agent"
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and offer Start in Air in the toast, with Copy Prompt for Agent as the second option`)
