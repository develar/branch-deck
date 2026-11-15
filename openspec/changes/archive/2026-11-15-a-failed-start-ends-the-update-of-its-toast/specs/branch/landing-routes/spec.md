## MODIFIED Requirements

### Requirement: The page: A merge is one toast that follows its update

A merge MUST be one toast that follows the update after it in place, as a push's: "Merged #7 of JIRA-456 into master ·
3 commits", then what the update did. The toast stays while the update runs, however long it takes. A start that fails
after the merge, by the error it answers or by its call, MUST end the update in that toast with the failure and Update
Project, never keep "Rebuilding on the new master…" until the user closes it.

#### Scenario: The merge and its update
- **WHEN** the merge succeeded and its update runs
- **THEN** one toast says "Merged #7 of JIRA-456 into master · 3 commits", then what the update did
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge the approved green pull request from the slot of the row by rebase, and update after it`)
  [@test] src/presenters/outcome.test.ts (`says what GitHub merged, an auto-merge it waits with, and its refusal in its words`)

#### Scenario: An update that runs past the time of a toast
- **WHEN** the update after the merge runs longer than the time of a toast
- **THEN** the toast of the merge stays, "Rebuilding on the new master…", and the end of the update comes into it, never into a second toast
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should keep the toast of the merge while the update after it waits, past the time of a toast, and tell the update's end in it`)

#### Scenario: A start that fails after the merge
- **WHEN** the start answers an error after the end of the merge, or its call fails then, before the update told its end
- **THEN** the toast of the merge says "Cannot update the project: …" with Update Project in place of "Rebuilding on the new master…", and goes in its time
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should end the toast of the merge with the error its start answered after the merge, never keep it rebuilding`)
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should end the toast of the merge with the failure of the call of its start after the merge, never keep it rebuilding`)
