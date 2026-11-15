## ADDED Requirements

### Requirement: Git and the identity: The alert of a failed sync stays while the next sync runs

The alert of a failed sync (the error alert, the warning of the name and email) MUST stay where it is while the next
sync runs, as "Last synced" stays, and go only once a sync ended without a failure: it never goes and comes back for
each sync of a repository whose syncs fail, so the rows below it and its "Sync Again" stay under a still pointer.

#### Scenario: The next sync fails again
- **GIVEN** the alert of a failed sync shows above the rows, and the pointer rests on a row
- **WHEN** the next sync runs and fails again
- **THEN** the alert stays throughout, and the row stays where it was
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`keeps the alert of a failed sync above the rows while the next sync runs and fails again, and drops it once one syncs`)
  [@test] src/model/store.test.ts (`keeps the failure the alerts show while the next sync runs, and drops it once a sync ended without one`)

#### Scenario: Sync Again of the warning of the name and email
- **GIVEN** the warning of the name and email shows
- **WHEN** the user presses "Sync Again", and its sync runs and fails again
- **THEN** the warning stays throughout, its "Sync Again" under the pointer that pressed it
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`keeps the git alert and its Sync Again under the pointer while the sync it asked for runs and fails again`)

#### Scenario: A sync ends without a failure
- **WHEN** a sync ends without a failure
- **THEN** the alert goes
  [@test] tests/e2e/onboarding/onboarding.spec.ts (`keeps the alert of a failed sync above the rows while the next sync runs and fails again, and drops it once one syncs`)
  [@test] src/model/store.test.ts (`keeps the failure the alerts show while the next sync runs, and drops it once a sync ended without one`)
