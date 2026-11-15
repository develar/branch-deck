## MODIFIED Requirements

### Requirement: Indicator: The button shows while work is on its way or a robot ran

The header MUST show the button after Update Project while a unit is on its way to master or in master, or a robot
ran in the last day; otherwise, while a selected repository is valid, it MUST keep its place as an empty, disabled
button that shows nothing, so the first work that comes moves no control of the header (Update, its Stop Waiting) from
under a still pointer. Without a selected repository (the Welcome screen) there MUST be no button.

#### Scenario: Work on its way to master
- **GIVEN** units on their way to master and one in master
- **WHEN** the header shows
- **THEN** the button is there after Update Project
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: A pushed branch still waits for its pull request
- **GIVEN** a lone branch whose step is its push, "On the way to master: 1 move is yours"
- **WHEN** the user pushes it and it waits for its pull request
- **THEN** the button stays, "On the way to master: 1 waiting"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should keep the button while a pushed branch waits for its pull request`)

#### Scenario: Nothing on the way, nothing in master, no robot of the last day
- **WHEN** nothing is on the way to master, nothing is in master and no robot ran in the last day
- **THEN** the overview is empty ("Nothing on the way to master") and the button keeps its place, disabled, with nothing to open
- **AND** it shows nothing: its node is hidden in its box, unlike the node of rows that still load ("checking…")
- **AND** the first work that comes fills that place: Update stays under the pointer, its tooltip open
  [@test] src/lib/landing-overview.test.ts (`is empty with nothing on the way, nothing in master and no robot of the last day`)
  [@test] tests/e2e/repository/header.spec.ts (`should keep Update under the pointer, its tooltip open, as the first work on the way comes and fills the place of the landing indicator`)

#### Scenario: No repository selected
- **WHEN** the Welcome screen asks for a repository
- **THEN** the header has no landing indicator: the work on its way is of a selected repository, as Activity is
- **AND** the repository selected, the indicator is there, empty and disabled
  [@test] tests/e2e/repository/browse.spec.ts (`should successfully select valid git repository`, `should show error for non-git directory`)
