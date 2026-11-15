## MODIFIED Requirements

### Requirement: The page: A push that landed is one toast that follows its update

A push that landed MUST be one toast that follows the update after it in place: "Pushed JIRA-456 into master · 3
commits", "Rebuilding on the new master…", then what the update did, or Update Project in the toast when the update did
not move the branch. The toast stays while the update runs, however long it takes; a start that fails after the push
ends the update in it with the failure, as a merge's does.

#### Scenario: The push and its update
- **WHEN** a push landed and its update runs
- **THEN** one toast says "Pushed JIRA-456 into master · 3 commits", "Rebuilding on the new master…", then what the update did ("JIRA-456 landed in origin/master · Updated master: …")
- **AND** Update Project in the toast when the update did not move the branch (local changes, a conflict, a failure, an update that did not run)
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`, `says the update that follows it: running, what it did, or Update Project where it did not move the branch`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push the copy into master from the slot of the row, the popover asking what CI did not pass, and update after it`)

#### Scenario: An update after the push that runs past the time of a toast
- **WHEN** the update after the push runs longer than the time of a toast
- **THEN** the toast of the push stays, "Rebuilding on the new master…", and the end of the update comes into it, never into a second toast
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should keep the toast of the push while the update after it waits, past the time of a toast, and tell the update's end in it`)

### Requirement: The page: A target that moved is a notice that follows its update

A target that moved MUST be a notice in the same way: "master moved: JIRA-456 is rebuilt on the new commits", nothing
pushed, the update, then "Push to master again", which the step offers. The notice stays while the update runs, however
long it takes, as the toast of a push does.

#### Scenario: The target moved
- **WHEN** the push found the target moved
- **THEN** the notice says "master moved: JIRA-456 is rebuilt on the new commits", nothing pushed, then the update
- **AND** then "Push to master again", which the step offers
  [@test] src/presenters/outcome.test.ts (`says what it pushed, a target that moved, the words of the server that refused it, and the push again of a plan that changed`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should say that master moved, push nothing, and offer the push of the copy rebuilt on it`)

#### Scenario: An update after a moved target that runs past the time of a toast
- **WHEN** the update after the push that found the target moved runs longer than the time of a toast
- **THEN** the notice stays, "Nothing was pushed · Rebuilding on the new master…", and the end of the update and "Push to master again" come into it, never into a second toast
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should keep the notice that master moved while the update after it waits, past the time of a toast, and offer the push again in it`)
