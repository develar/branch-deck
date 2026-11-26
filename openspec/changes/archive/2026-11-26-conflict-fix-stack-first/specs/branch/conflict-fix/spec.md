## MODIFIED Requirements

### Requirement: Candidates: A branch on a parent of its own keeps it

A branch on a parent of its own MUST keep it: the ancestor that leaves the stack of that branch is restacked instead,
and a parent the `--plan` gives the restacked branch, or its unstack, is replaced, never doubled.

#### Scenario: A leaf on a parent of its own
- **GIVEN** a conflicting branch stacked on a parent of its own that needs the lines of a sibling of the parent
- **WHEN** the fix makes its candidates
- **THEN** the branch keeps its parent, and the ancestor that leaves the stack of that branch is restacked instead
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_leaf_on_a_parent_of_its_own_keeps_it_and_the_parent_is_restacked_on_what_the_leaf_needs`)

#### Scenario: A restack on top of a plan
- **GIVEN** a `--plan` that stacks the conflicting branch on a parent of its own
- **WHEN** the fix restacks on top of it
- **THEN** the branch keeps the parent the plan gives, and that parent is restacked instead
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_restack_on_top_of_a_plan_keeps_the_parent_the_plan_gives`)

#### Scenario: A restacked branch the plan gave a parent
- **GIVEN** a `--plan` that gives a branch a parent
- **WHEN** the fix restacks that branch on another parent
- **THEN** a parent the `--plan` gives is replaced by the restack, never doubled: the commits of the branch go to the new path only

#### Scenario: A restacked branch the plan unstacks
- **GIVEN** a `--plan` that unstacks a branch, which then conflicts
- **WHEN** the fix restacks that branch
- **THEN** the restack replaces the unstack of the plan, never both
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_restack_on_top_of_a_plan_that_unstacks_the_branch_replaces_the_unstack`)

### Requirement: Candidates: A conflict moves the commit to the branch whose stack has what it needs

For a conflict, a candidate SHALL move the commit and the commits of its branch that depend on it (by lines, or by a
name it declares: the edges by symbols with `branchdeck.symbols on`, the declaration patterns for the other languages
and modes) to the branch whose stack has what it needs. It is taken only where no restack fixes the conflict (the least of the
verified candidates).

#### Scenario: The commit moves to the branch whose stack has what it needs
- **GIVEN** a conflicting commit whose branch keeps other commits
- **WHEN** the fix makes its candidates
- **THEN** one moves the commit and the commits of its branch that depend on it to the branch whose stack has what it needs
- **AND** the fix takes it only where no restack fixes the conflict
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_moves_only_where_no_restack_fixes_the_conflict`)

#### Scenario: A parent that needs a commit of its child
- **GIVEN** a commit of a parent that needs a commit of a branch stacked on it
- **WHEN** the fix makes its candidates
- **THEN** the parent's commit moves to the child
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child`)

#### Scenario: The dependents with the symbols off
- **GIVEN** `branchdeck.symbols` is off
- **WHEN** the fix makes its candidates
- **THEN** the commits that depend on the moved one by a name it declares come from the declaration patterns, and the fixes are the same as with the symbols on
  [@test] crates/trial-check/src/conflict_fix_test.rs (`the_fixes_are_the_same_with_the_symbols_off`)

### Requirement: Candidates: A conflict restacks the branch on what it needs

For a conflict, a candidate SHALL restack the branch on the branch whose stack has what it needs, or, when no owner
is built on every other, on a chain of the owners (each restacked on the one with the older commits, when it stays in
its stack). The restack MUST be the candidate tried first, then the pull in, then the move, so a search the budget
cuts keeps the groups.

#### Scenario: A branch of one commit
- **GIVEN** a conflicting branch of one commit
- **WHEN** the fix makes its candidates
- **THEN** one restacks the branch on the branch it needs
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_single_commit_branch_is_restacked_on_the_branch_it_needs`)

#### Scenario: No owner built on every other
- **GIVEN** no owner of what the commit needs is built on every other
- **WHEN** the fix makes its candidates
- **THEN** one restacks the branch on a chain of the owners, each restacked on the one with the older commits when it stays in its stack

### Requirement: Verification and choice: The least of the verified candidates is chosen

Among the verified candidates the fix SHALL choose the least by the failing branches, then the emptied branches, the
commits that leave the group the user gave them (a move or a pull in of a commit of a branch), the rewritten subjects,
the commits whose base changes (every commit of a restacked branch and of the branches built on it, and the unassigned
commits a branch takes), each deciding only between candidates equal by the ones before. A dependency is an order of
landing, which a stack says: a commit leaves its group only where no restack fixes the conflict.

#### Scenario: Several verified candidates
- **GIVEN** several verified candidates
- **WHEN** the fix chooses one
- **THEN** it takes the least by the failing branches, then the emptied branches, the commits that leave their group, the rewritten subjects, the commits whose base changes, in this order
- **AND** a restack counts every commit of its branch and of the branches built on it as changing its base, and no commit as leaving its group
- **AND** an unassigned commit a branch takes counts as changing its base, not as leaving a group

#### Scenario: A restack of a branch that keeps other commits over a move of its commit
- **GIVEN** a conflicting commit whose branch keeps other commits, which a restack of the branch fixes as a move of the commit does
- **WHEN** the fix chooses one
- **THEN** it restacks the branch: its commits stay in the group the user gave them
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_whose_branch_keeps_other_commits_restacks_its_branch_on_what_it_needs`)

#### Scenario: A move where no restack fixes the conflict
- **GIVEN** a parent that needs a commit of its child, which no restack fixes, and another conflict that a restack fixes
- **WHEN** the fix chooses
- **THEN** the parent's commit moves to the child, and the other branch is restacked
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_moves_only_where_no_restack_fixes_the_conflict`)

#### Scenario: A move that keeps both branches over one that empties a branch
- **GIVEN** a parent that needs a commit of its child
- **WHEN** moving the parent's commit to the child keeps both branches and pulling the child's commit down would empty the child
- **THEN** the fix moves the parent's commit
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child`)

### Requirement: Verification and choice: The direct answer comes first, in one trial

The direct answer MUST come first, in one trial, by the line dependencies alone (before the symbols are read): every
conflicting branch whose needed commits all have an owner restacked on the branch, or the chain of branches, that holds
what it needs, but for a restack onto a stack with a conflict that the answer leaves. Of an answer that fixes only some
of the branches, the restacks of those MUST be tried again alone, so the search goes on for the others on the base they
have.

#### Scenario: A branch of one commit
- **GIVEN** a conflicting branch of one commit
- **WHEN** the search starts
- **THEN** its first trial restacks the branch on the branch that holds what it needs
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_single_commit_branch_is_restacked_on_the_branch_it_needs`)

#### Scenario: A commit that needs two other branches
- **GIVEN** a conflicting branch whose commit needs two branches, none of which holds both
- **WHEN** the search starts
- **THEN** its first trial restacks the branch on the chain of those branches
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_leaf_whose_commit_needs_two_other_leaves_is_restacked_on_their_chain_in_one_trial`)

#### Scenario: A branch that keeps commits the conflicting one doesn't need
- **GIVEN** a conflicting branch of two commits, only the last of which needs another branch
- **WHEN** the search starts
- **THEN** its first trial restacks the branch on the branch that holds what it needs, and verifies
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_whose_branch_keeps_other_commits_restacks_its_branch_on_what_it_needs`)

#### Scenario: What the direct answer leaves
- **WHEN** the direct answer leaves conflicts
- **THEN** the search goes on for them
- **AND** of the restacks of the answer only those of the branches it fixed stay
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_budget_ends_with_the_best_plan_so_far`)

### Requirement: App: Apply Fix applies the component of the branch as one regroup

"Apply Fix" (primary `sm`) MUST apply the component of the branch as one regroup, the rewrite `applyConflictFix` of
the engine named on the table the window shows: a sync that shows the first branch the fix fills with the moved
commits selected (the restacked branch of a restack), then the toast "Fixed the conflict of ui" ("Fixed the conflicts of N branches") with the steps and
Undo (the rewrite `undo` of its token), the same entry in Activity.

#### Scenario: The component of the branch
- **GIVEN** a fix of several components
- **WHEN** the conflict view of a branch shows it
- **THEN** it applies the component with a step for the branch, else one that fixes or unblocks it
- **AND** its toast is "Fixed the conflict of ui", or "Fixed the conflicts of N branches"
  [@test] src/lib/conflict-fix.test.ts (`finds the component with a step for the branch, else one that fixes or unblocks it`)

#### Scenario: Apply Fix and Undo
- **WHEN** the user clicks "Apply Fix"
- **THEN** a sync shows the first branch the fix fills, the moved commits selected, or the restacked branch
- **AND** the toast "Fixed the conflict of ui" has the steps and Undo, the same entry in Activity
- **AND** Undo puts the grouping back
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the stack a trial build verified, apply it and undo it`)

### Requirement: App: A Safe Push that lands what the fix brings is the wait, the fix below it

While a running Safe Push that will not fail holds a holder of the fix (`FixComponent.holders`), the callout MUST be
titled "Wait for the Safe Push of <its subject>", say that Update Project applies the fixed branches cleanly once it
merges, and offer the fix below it ("Or fix it now: <the step>") with Apply Fix as the other choice, beside the
robot and its row, never its cancel.

#### Scenario: A Safe Push of the branch the fix stacks on
- **GIVEN** a fix that stacks ui on api, and a running Safe Push of api
- **WHEN** the conflict view of ui shows the fix
- **THEN** it is titled "Wait for the Safe Push of api" and says "api has what ui needs: once its Safe Push merges, Update Project applies ui cleanly, without a fix."
- **AND** below it "Or fix it now: Stack ui on api", Apply Fix enabled, Open in Patronus opening the robot, Show api opening its row, no Cancel Safe Push
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
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_whose_branch_keeps_other_commits_restacks_its_branch_on_what_it_needs`)
  [@test] crates/trial-check/src/conflict_fix_test.rs (`an_unassigned_commit_the_branch_needs_is_pulled_in`)

### Requirement: App: A failing Safe Push of what the fix brings leaves the fix the way

While the running Safe Push that holds a branch that holds what the fix brings will fail (a required check failed),
the fix MUST stay the title with Apply Fix primary, a line MUST say "The Safe Push of <its subject> is failing, so
<the branch> doesn't land with it: fix the conflict now, or restart the Safe Push.", and the robot's Restart Safe
Push, Cancel Safe Push and Open in Patronus MUST stand beside Apply Fix.

#### Scenario: The Safe Push of api is failing
- **GIVEN** a fix that stacks ui on api, and a running Safe Push of api that will fail
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
- **GIVEN** a fix that stacks ui on api, and the Safe Push of api merged
- **WHEN** the conflict view of ui shows it
- **THEN** it says "api is merged by Patronus" and "Update Project applies ui cleanly, without a fix.", without Apply Fix
- **AND** Update Project updates the project
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer Update Project in place of the fix once a Safe Push landed what it brings`)
  [@test] src/lib/conflict-fix.test.ts (`says that Update Project brings what landed, in place of the fix`)
