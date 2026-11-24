## MODIFIED Requirements

### Requirement: After the update: The toast of what landed has no action

The toast that took the place of a merge toast MUST have no action but the start of the subject that waited for the
merge ([Landing Plan](../../branch/landing-plan/spec.md#merge-toast)): the update is done, the island has the rest.

#### Scenario: The merge toast after the update
- **WHEN** the toast of a merge turned into what the update landed
- **THEN** it has no action, unless a start waited for the merge: "Start flow"
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed`, `should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should offer the start of the branch that waited in the toast of the merge once its Update Project landed it, and start it on the press`)
