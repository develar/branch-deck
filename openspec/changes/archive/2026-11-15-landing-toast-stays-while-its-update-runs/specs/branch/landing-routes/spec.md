## MODIFIED Requirements

### Requirement: The page: A push that landed is one toast that follows its update

A push that landed MUST be one toast that follows the update after it in place: "Pushed JIRA-456 into master · 3
commits", "Updating the project…", then what the update did, or Update Project in the toast when the update did not
move the branch. The toast stays while the update runs, however long it takes.

#### Scenario: The push and its update
- **WHEN** a push landed and its update runs
- **THEN** one toast says "Pushed JIRA-456 into master · 3 commits", "Updating the project…", then what the update did ("JIRA-456 landed in origin/master · Updated master: …")
- **AND** Update Project in the toast when the update did not move the branch (local changes, a conflict, a failure, an update that did not run)
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`, `says the update that follows it: running, what it did, or Update Project where it did not move the branch`)

### Requirement: The page: A merge is one toast that follows its update

A merge MUST be one toast that follows the update after it in place, as a push's: "Merged #7 of JIRA-456 into master ·
3 commits", then what the update did. The toast stays while the update runs, however long it takes.

#### Scenario: The merge and its update
- **WHEN** the merge succeeded and its update runs
- **THEN** one toast says "Merged #7 of JIRA-456 into master · 3 commits", then what the update did
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should merge the approved green pull request from the slot of the row by rebase, and update after it`)
  [@test] src/presenters/outcome.test.ts (`says what GitHub merged, an auto-merge it waits with, and its refusal in its words`)

#### Scenario: An update that runs past the time of a toast
- **WHEN** the update after the merge runs longer than the time of a toast
- **THEN** the toast of the merge stays, "Rebuilding on the new master…", and the end of the update comes into it, never into a second toast
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should keep the toast of the merge while the update after it waits, past the time of a toast, and tell the update's end in it`)
