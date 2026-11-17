## MODIFIED Requirements

### Requirement: The page: An auto-merge is said until GitHub merged it

An auto-merge MUST be "Auto-Merge of #7 is on", its Status cell "Merges when checks pass" (a wait, no step of the slot), its node landing, Cancel Auto-Merge
in the menus ("Canceled the auto-merge of #7"); once GitHub merged it the same toast says "Merged #7 into master" with
Update Project.

#### Scenario: The auto-merge is on, then GitHub merged it
- **WHEN** the start enabled the auto-merge while the required checks run
- **THEN** the toast says "Auto-Merge of #7 is on", its Status cell "Merges when checks pass", its node landing
- **AND** once GitHub merged it the same toast says "Merged #7 into master" with Update Project
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should enable the auto-merge while the required checks run, and offer Update Project once GitHub merged it`)
  [@test] src/presenters/outcome.test.ts (`says what GitHub merged, an auto-merge it waits with, and its refusal in its words`)

#### Scenario: Cancel Auto-Merge in the menus
- **WHEN** the user chooses Cancel Auto-Merge in the menu of the row
- **THEN** the auto-merge is canceled, "Canceled the auto-merge of #7"
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should cancel the auto-merge from the menu of the row`)
