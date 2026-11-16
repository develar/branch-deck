## MODIFIED Requirements

### Requirement: Details: The details are three requests in parallel, cached by the backend

The details MUST be `GET /robots/{id}`, `/teamcity-checks` and `/problems` (every severity: the errors are shown, the
warning `robot_is_failing` marks the robot failing, [below](#failing)) in parallel, cached by the backend: a finished
robot for the process, a running one for a minute. Only the read ahead of a failed robot MUST keep its problems, for
the process, asked again only after a failure; the details take them or join its read, else ask on their own.

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

#### Scenario: The details and the read ahead ask at once
- **WHEN** the details of a robot that just failed are read while its read ahead asks its problems
- **THEN** one request answers both
  [@test] crates/patronus/src/actions_test.rs (`the_details_and_the_read_ahead_of_a_failed_robot_ask_its_problems_once`)

#### Scenario: The details of a robot that ran when they asked
- **GIVEN** the details of a running robot ask its problems, and the robot fails meanwhile
- **WHEN** the read ahead asks why it failed
- **THEN** it sends its own request and keeps what Patronus says of the failed robot, not the problems of the running one
  [@test] crates/patronus/src/actions_test.rs (`the_read_ahead_of_a_robot_that_failed_joins_no_problems_read_of_its_details_while_it_ran`)

#### Scenario: A warm start
- **WHEN** the app starts again with the robots of the frame, why the failed ones failed among them
- **THEN** the card shows the reason as it opens, and its first details ask the problems once more (the process kept none)

#### Scenario: The details could not read the problems
- **WHEN** Patronus fails the problems of the details of a finished robot
- **THEN** the details are not kept and keep no problems: the read ahead asks Patronus and has them, and a reopen of
  the card asks the details again
  [@test] crates/patronus/src/actions_test.rs (`problems_the_details_could_not_read_are_asked_again`)

## ADDED Requirements

### Requirement: Details: Why a failed robot failed shows as its card opens

The session MUST keep the problems of severity error it read ahead of each failed robot of the table (`problems` of its
robots, gone with the robot), and the card of the robot MUST show them as it opens, above its actions. Problems not
known as the card opened come with the details, past the actions, and so does the notice of a missing review they
bring ([Branch Row](../branch-row/spec.md)).

#### Scenario: The problems read ahead are kept
- **WHEN** the session read the problems of a failed robot of the table
- **THEN** its robots keep them by id, and drop them with the robot
  [@test] crates/repository-model/src/facts_test.rs (`the_problems_of_a_failed_robot_of_the_table_are_asked_until_known`)

#### Scenario: The card of a failed robot opens
- **GIVEN** the session read why a failed robot failed
- **WHEN** its card opens, its details still to come
- **THEN** its problems stand in it at once, above Safe Push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 700 px high`, `should show why the robot failed above Safe Push as its card opens, and keep Safe Push under the pointer as the details of its robot come, 420 px high`)

#### Scenario: Why a robot failed comes after its card opened
- **GIVEN** the card of a failed robot of pull request #7 open while its problems are still to come, the pointer on Restart
- **WHEN** they come
- **THEN** the problems and the notice of the missing review stand among the late details, and Restart keeps its box
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, 700 px high`, `should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, 420 px high`)

#### Scenario: Why a robot without a pull request failed comes after its card opened
- **GIVEN** the card of a failed robot without a pull request open while its problems are still to come, the pointer on
  its Restart
- **WHEN** they say it failed for missing approvals
- **THEN** Safe Push comes in the place of Restart, which a new Safe Push replaces, and the problems and the notice stand
  among the late details
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, 700 px high`, `should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, 420 px high`)
