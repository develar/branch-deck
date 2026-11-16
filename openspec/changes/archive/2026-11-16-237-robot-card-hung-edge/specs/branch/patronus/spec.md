## MODIFIED Requirements

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
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, 700 px high`, `should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, 420 px high`, `should keep Restart of the robot card under the pointer as why the robot failed comes after the card opened, over its chip`)

#### Scenario: Why a robot without a pull request failed comes after its card opened
- **GIVEN** the card of a failed robot without a pull request open while its problems are still to come, the pointer on
  its Restart
- **WHEN** they say it failed for missing approvals
- **THEN** Safe Push comes in the place of Restart, which a new Safe Push replaces, and the problems and the notice stand
  among the late details
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, 700 px high`, `should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, 420 px high`, `should put Safe Push in the place of Restart under the pointer as why the robot without a pull request failed comes after the card opened, over its chip`)
