## ADDED Requirements

### Requirement: Log: Escape closes the window from the disabled Clear Log too

Escape MUST close the window from every control of it, the disabled Clear Log too, once the tooltip its focus opened
closed; Enter on the disabled Clear Log MUST press nothing.

#### Scenario: Escape on the disabled Clear Log
- **GIVEN** an empty log, the focus on the disabled Clear Log, its tooltip open
- **WHEN** the user presses Enter, then Escape twice
- **THEN** Enter presses nothing, the first Escape closes the tooltip and the window stays
- **AND** the second Escape closes the window
  [@test] tests/e2e/activity/activity.spec.ts (`should close on Escape from its disabled Clear Log once its tooltip closed, which Enter does not press`)
