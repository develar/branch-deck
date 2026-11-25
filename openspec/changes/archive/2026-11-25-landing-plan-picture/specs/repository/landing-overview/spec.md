## MODIFIED Requirements

### Requirement: Overview: Show Statistics opens the Landing view

"Show Plan" and "Show Statistics", on the line of the last day also without a robot of the last day, MUST close the
overview and show the Landing view at that section: what goes into master next and after what
([Landing Plan](../../branch/landing-plan/spec.md)), how the Safe Pushes went ([Main Views](../main-views/spec.md)).

#### Scenario: Show Statistics
- **WHEN** the user clicks "Show Statistics", with or without a robot of the last day
- **THEN** the overview closes and the Statistics of the Landing view show
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

#### Scenario: Show Plan
- **WHEN** the user clicks "Show Plan"
- **THEN** the overview closes and the Plan of the Landing view shows
  [@test] tests/e2e/landing/landing-plan.spec.ts (`should switch between Plan and Statistics, and open each from the landing overview`)
