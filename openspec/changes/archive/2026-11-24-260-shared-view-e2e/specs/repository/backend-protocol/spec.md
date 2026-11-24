## MODIFIED Requirements

### Requirement: Pages: The desktop window and the IDE share one backend

The desktop window and the IDE view of one repository MUST show the one session of the backend, its check time, its
Landed island and the estimates of its robots included, and an event of a page of one connection (the Command Palette of the IDE's action) SHALL reach no window of
another.

#### Scenario: A check of the desktop window
- **WHEN** the desktop window and the IDE view show one repository, and a focus of the desktop window a minute after
  the sync finds nothing changed
- **THEN** both say "Checked just now", and the Command Palette action of the IDE opens the palette of its view and none
  of the desktop window
  [@test] tests/e2e/air/one-backend.spec.ts (`the desktop window and the IDE view of one repository say the same check time, and the Command Palette of the IDE opens no palette of the desktop window`)

#### Scenario: The Landed island of the desktop window
- **WHEN** the desktop window and the IDE view show the Landed island of an update, and the desktop window saw it long
  enough on the clock of the session
- **THEN** the next sync hides it in both
  [@test] tests/e2e/air/one-backend.spec.ts (`the desktop window and the IDE view of one repository show one Landed island, which the sight of the desktop window hides in both`)

#### Scenario: A running robot
- **WHEN** the desktop window and the IDE view show a running robot that the history of the repository estimates
- **THEN** both say the same time left of it
  [@test] tests/e2e/air/one-backend.spec.ts (`the desktop window and the IDE view of one repository say the same estimate of a running robot`)
