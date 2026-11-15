## MODIFIED Requirements

### Requirement: The app: "Park Branch…" is disabled while a robot lands the branch

"Park Branch…" MUST be disabled while a robot lands the branch; a merged robot that landed it disables nothing. The
plan and the run of a park MUST refuse it alike (`robot_running`, nothing parked), for whoever asks: the robot lands the
branch whatever a park says, and a park would say otherwise.

#### Scenario: A robot lands the branch
- **GIVEN** a robot that runs lands the branch
- **WHEN** the user opens its menu
- **THEN** "Park Branch…" is disabled with the hold of the robot
- **AND** a park asked anyway (a window of a state before, another client) is refused by its plan and its run: "Nothing was parked: A Safe Push of JIRA-456 is running (…): it lands JIRA-456 whatever a park says"
  [@test] crates/domain-policy/src/actions_test.rs (`a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should refuse in the backend a park of a branch a running Safe Push holds, which lands it whatever a park says`)

#### Scenario: A merged robot landed the branch
- **GIVEN** a merged robot landed the branch
- **WHEN** the user opens its menu
- **THEN** "Park Branch…" is enabled
  [@test] crates/domain-policy/src/actions_test.rs (`a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks`)
