## MODIFIED Requirements

### Requirement: The app: The read waits for no sync

`get_robot_stats` MUST wait for no sync: shown during one, the Statistics of Landing read the statistics at once.

#### Scenario: The Landing view shown during a held sync
- **GIVEN** a sync is held
- **WHEN** the Statistics of the Landing view are shown
- **THEN** they read the statistics at once, and the sync ends once
  [@test] tests/e2e/landing/statistics.spec.ts (`should read the statistics of the Landing view shown during a held sync at once, the sync ending once`)

### Requirement: The app: The Landing view shows the statistics

The Statistics of the Landing view of the main window MUST show the statistics, in the period of its header
([Main Views](../main-views/spec.md)).

#### Scenario: How the Safe Pushes went
- **WHEN** the Statistics of the Landing view are shown
- **THEN** they show the merges against everyone else, the time lost, the failure causes and the hours
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

#### Scenario: The chosen period
- **WHEN** the user chooses the period in the header of the Landing view
- **THEN** the view counts the chosen period: its robots and its weeks
- **AND** the choice is kept
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)
