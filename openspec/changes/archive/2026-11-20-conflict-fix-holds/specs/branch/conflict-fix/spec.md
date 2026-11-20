## ADDED Requirements

### Requirement: App: A Safe Push that lands what the fix brings is the wait, the fix below it

While a running Safe Push that will not fail holds a holder of the fix (`FixComponent.holders`), the callout MUST be
titled "Wait for the Safe Push of <its subject>", say that Update Project applies the fixed branches cleanly once it
merges, and offer the fix below it ("Or fix it now: <the step>") with Apply Fix as the other choice, beside the
robot and its row, never its cancel.

#### Scenario: A Safe Push of the branch the commit moves to
- **GIVEN** a fix that moves a commit of ui to api, and a running Safe Push of api
- **WHEN** the conflict view of ui shows the fix
- **THEN** it is titled "Wait for the Safe Push of api" and says "api has what ui needs: once its Safe Push merges, Update Project applies ui cleanly, without a fix."
- **AND** below it "Or fix it now: Move “Show the timeout in seconds” to api", Apply Fix enabled, Open in Patronus opening the robot, Show api opening its row, no Cancel Safe Push
- **AND** Apply Fix applies the fix while the robot runs
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should say to wait for the Safe Push that lands what the fix brings, and still apply the fix`)
  [@test] src/lib/conflict-fix.test.ts (`says the wait for a robot that lands what the fix brings, the fix below it`)

#### Scenario: What the hold of a branch means for a fix
- **WHEN** the view derives the hold of a branch
- **THEN** a merged Safe Push that landed all of it is `landed`, a running one whose required check failed `failing`, any other running one `lands`
  [@test] crates/domain-policy/src/holds_test.rs (`a_hold_of_a_branch_of_a_fix_is_a_wait_for_its_robot_unless_it_fails_or_landed`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_says_whether_the_robot_that_holds_a_branch_of_a_fix_lands_it_or_fails`)

#### Scenario: The branches that hold what a fix brings
- **WHEN** the fix moves a commit, restacks a branch or pulls commits in
- **THEN** its holders are the target of the move or the restack and the branches a pull in takes the commits from, none for unassigned commits
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_moves_to_the_branch_whose_stack_has_what_it_needs`)
  [@test] crates/trial-check/src/conflict_fix_test.rs (`an_unassigned_commit_the_branch_needs_is_pulled_in`)

### Requirement: App: A failing Safe Push of what the fix brings leaves the fix the way

While the running Safe Push that holds a branch that holds what the fix brings will fail (a required check failed),
the fix MUST stay the title with Apply Fix primary, a line MUST say "The Safe Push of <its subject> is failing, so
<the branch> doesn't land with it: fix the conflict now, or restart the Safe Push.", and the robot's Restart Safe
Push, Cancel Safe Push and Open in Patronus MUST stand beside Apply Fix.

#### Scenario: The Safe Push of api is failing
- **GIVEN** a fix that moves a commit of ui to api, and a running Safe Push of api that will fail
- **WHEN** the conflict view of ui shows the fix
- **THEN** its title is the step, the line says the Safe Push is failing, Apply Fix is primary
- **AND** Cancel Safe Push cancels the robot, Restart Safe Push restarts it
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should make the fix the way while the Safe Push that holds what it brings is failing, its restart and cancel beside it`)
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should restart the failing Safe Push that holds what the fix brings`)
  [@test] src/lib/conflict-fix.test.ts (`says that a failing robot lands nothing, the fix the way`)

### Requirement: App: What a merged Safe Push landed is brought by Update Project, in place of the fix

When a merged Safe Push landed all of a branch that holds what the fix brings, the callout MUST say "<branch> landed
with <its subject>" ("<branch> is merged by Patronus" of its own robot) and "Update Project applies <the fixed
branches> cleanly, without a fix.", with Update Project in place of the steps and Apply Fix.

#### Scenario: The Safe Push of api merged
- **GIVEN** a fix that moves a commit of ui to api, and the Safe Push of api merged
- **WHEN** the conflict view of ui shows it
- **THEN** it says "api is merged by Patronus" and "Update Project applies ui cleanly, without a fix.", without Apply Fix
- **AND** Update Project updates the project
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer Update Project in place of the fix once a Safe Push landed what it brings`)
  [@test] src/lib/conflict-fix.test.ts (`says that Update Project brings what landed, in place of the fix`)

## REMOVED Requirements

### Requirement: App: Apply Fix is disabled while a Safe Push holds a branch it changes

**Reason**: a hold refuses a push, not a rewrite: the fix is local and never disturbs the robot, which lands the remote copy it took; the disable left the user with neither a way nor a next step

**Migration**: `App: A Safe Push that lands what the fix brings is the wait, the fix below it`, `App: A failing Safe Push of what the fix brings leaves the fix the way`, `App: What a merged Safe Push landed is brought by Update Project, in place of the fix`
