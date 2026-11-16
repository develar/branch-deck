## MODIFIED Requirements

### Requirement: Details: The details are three requests in parallel, cached by the backend

The details MUST be `GET /robots/{id}`, `/teamcity-checks` and `/problems` (every severity: the errors are shown, the
warning `robot_is_failing` marks the robot failing, [below](#failing)) in parallel, cached by the backend: a finished
robot for the session, a running one for a minute. The problems of a finished robot read ahead MUST NOT be asked
again.

#### Scenario: The details of a robot
- **WHEN** the details of a robot are read
- **THEN** they have the profile, the checks and the error problems
  [@test] crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)

#### Scenario: The warning that the robot is failing
- **WHEN** the problems of a running robot hold the warning `robot_is_failing`
- **THEN** the robot is marked failing ([below](#failing))
  [@test] crates/patronus/src/client.rs (`a_failed_check_of_a_running_robot_reads_as_patronus_answers_it`)

#### Scenario: The details of a finished robot
- **WHEN** the details of a finished robot are demanded again
- **THEN** they are asked once
- **AND** its problems are its errors
  [@test] crates/patronus/src/actions_test.rs (`details_of_a_finished_robot_are_asked_once`)
  [@test] crates/patronus/src/actions_test.rs (`the_problems_of_a_finished_robot_are_its_errors`)

#### Scenario: The card of a failed robot whose problems were read ahead
- **GIVEN** the session read the problems of a failed robot of the table ahead
- **WHEN** its card opens and its details are read
- **THEN** they ask its robot and its checks, not its problems
- **AND** its failure (the prompt for an agent) asks no problems either
  [@test] crates/patronus/src/actions_test.rs (`the_card_of_a_failed_robot_asks_no_problems_again`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 700 px high`)

## ADDED Requirements

### Requirement: Details: Why a failed robot failed shows as its card opens

The session MUST keep the problems of severity error it read ahead of each failed robot of the table (`problems` of its
robots, gone with the robot), and the card of the robot MUST show them as it opens, above its actions. Problems not
known as the card opened come with the details, past the actions ([Branch Row](../branch-row/spec.md)).

#### Scenario: The problems read ahead are kept
- **WHEN** the session read the problems of a failed robot of the table
- **THEN** its robots keep them by id, and drop them with the robot
  [@test] crates/repository-model/src/facts_test.rs (`the_problems_of_a_failed_robot_of_the_table_are_asked_until_known`)

#### Scenario: The card of a failed robot opens
- **GIVEN** the session read why a failed robot failed
- **WHEN** its card opens, its details still to come
- **THEN** its problems stand in it at once, above Safe Push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 700 px high`, `should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 420 px high`)
