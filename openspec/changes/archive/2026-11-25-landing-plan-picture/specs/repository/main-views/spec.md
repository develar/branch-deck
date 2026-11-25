## ADDED Requirements

### Requirement: Landing: Plan and Statistics are its sections, the plan first

Landing MUST have two sections, Plan (the plan of what goes into master ahead, [Landing Plan](../../branch/landing-plan/spec.md))
and Statistics (how the Safe Pushes went back), switched by the option set after its heading; every start of the
window shows Plan first, and only the user changes the section (the switch, Show Plan or Show Statistics of the
landing overview, a Status cell that opens an edge of the plan).

#### Scenario: The switch of the sections
- **WHEN** the user opens Landing, presses Statistics, then Plan
- **THEN** the plan shows first, then the statistics, then the plan
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should switch between Plan and Statistics, and open each from the landing overview`)

## MODIFIED Requirements

### Requirement: Switching: Show Statistics of the landing overview shows Landing

"Show Statistics" of the landing overview SHALL close it and show the Statistics of Landing; "Show Plan" its Plan.

#### Scenario: Show Statistics
- **WHEN** the user presses "Show Statistics" in the landing overview
- **THEN** the landing overview closes and the Statistics of Landing show
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

#### Scenario: Show Plan
- **WHEN** the user presses "Show Plan" in the landing overview
- **THEN** the landing overview closes and the Plan of Landing shows
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should switch between Plan and Statistics, and open each from the landing overview`)

### Requirement: Landing: The statistics are read only while Landing shows

The statistics MUST be read while the Statistics of Landing show: when they show and when the window gets the focus;
a hidden Landing, or its Plan, reads nothing; a change of the period reads its numbers.

#### Scenario: A focus of the window while the branches show
- **GIVEN** Landing is hidden
- **WHEN** the window gets the focus
- **THEN** no statistics are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: The Plan of Landing shows
- **GIVEN** the Plan of Landing shows
- **WHEN** the window gets the focus
- **THEN** no statistics are read until the user shows the Statistics
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: Landing shows and the window gets the focus
- **WHEN** the Statistics of Landing show, and again when the window gets the focus while they show
- **THEN** the statistics are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should read nothing while the view is hidden, and ask again on Refresh`)

#### Scenario: A change of the period
- **WHEN** the user chooses another Period
- **THEN** the numbers of that period are read
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)
