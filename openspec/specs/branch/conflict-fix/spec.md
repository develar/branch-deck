---
name: Conflict Fix
description: Requirements for the fix of a sync conflict, the regroup after which the branches that conflict (or remove a declaration that another branch's commit stops using) apply cleanly, found by trial builds and offered, never applied without the user.
style: plain-1
targets:
  - crates/trial-check/src/conflict_fix.rs
  - crates/trial-check/src/check.rs
  - crates/trial-check/src/missing_commits.rs
  - crates/history-rewrite/src/move_plan.rs
  - crates/trial-check/src/dangling_references.rs
  - crates/bd-rewrite/src/check_command.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/app-api/src/reads/conflict_fix.rs
  - src/stores/conflict-fix.ts
  - crates/operations/src/rewrite.rs
  - src/lib/rewrite-run.ts
  - src/lib/conflict-fix.ts
  - src/lib/conflict-fix-prompt.ts
  - src/components/conflict/ConflictFixPanel.tsx
  - src/components/conflict/MergeConflictViewer.tsx
  - src/hooks/useAgentCliCommand.ts
  - src/hooks/useBranchContextActions.ts
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/conflict/ConflictWindowPage.tsx
  - src/presenters/rewrite.ts
  - src/hooks/useConflictBase.ts
  - crates/git-ops/src/conflict_analysis.rs
  - crates/git-ops/src/merge_conflict.rs
  - crates/domain-types/src/conflict.rs
---

# Conflict Fix

Status: Implemented
Date: 2026-10-16

## Purpose

A virtual branch conflicts when one of its commits changes lines that a commit of another branch changed first. The
conflict view used to say so ("45 missing commits modified the conflicting files") and to list the ways out as text.
The fix does the work: it tries regroups in trial builds (`brd check`'s, objects only) and keeps the one after which the
branches apply cleanly, with the fewest changes the user notices.

In one monorepo checkout, a commit of `IJAI-3852` (stacked on `IJAI-3828`) missed 42 commits of 12 branches, all of
the stack of `IJAI-4047`, and `IJAI-4047` itself needed a commit of a branch stacked on it. The fix found the regroup
that a person found by hand: two commits into `IJAI-4047`, one of them a whole issue by its issue group, without a
rewrite.

## Goals

- Find a regroup that fixes the conflicts of the current grouping, verified by a trial build, within a bounded time.
- Prefer the fix the user notices least: no branch that applied cleanly fails, few emptied branches, few moved commits.
- Say what the fix does in steps (move, pull in, restack), what it fixes and what it empties.
- Offer it to an agent as a plan for `brd group`.

## Non-goals

- Editing a commit. A conflict that no regroup fixes stays; `brd move-hunk` and an agent resolve it.
- Applying a fix without the user: `brd check --fix` writes no ref.

## Requirements

### Requirement: What a commit needs: The commits it depends on that its stack lacks

What a conflicting commit needs MUST be the commits it depends on by its lines (`dependencies.rs`, with fixup units),
and what those need in turn, that neither its branch nor the branches it is built on have; the missing commits by
the changed files only when the lines name none.

#### Scenario: The lines name the commits it depends on
- **GIVEN** a conflicting commit whose lines depend on commits that neither its branch nor the branches it is built on have
- **WHEN** the fix reads what it needs
- **THEN** it needs those commits, with fixup units, and what those need in turn

#### Scenario: The lines name none
- **GIVEN** a conflicting commit whose lines name no commit it depends on
- **WHEN** the fix reads what it needs
- **THEN** it needs the missing commits by the changed files

### Requirement: What a commit needs: The owner built on every other owner holds what it needs

The branch whose stack holds what a commit needs SHALL be an owner built on every other owner (the deepest).

#### Scenario: The commits it needs have several owners
- **GIVEN** the commits a conflicting commit needs belong to several branches
- **WHEN** the fix looks for the branch whose stack holds them
- **THEN** it is the owner built on every other owner, the deepest

### Requirement: Candidates: A candidate is a move as move_plan.rs plans it

A candidate MUST be a move as `move_plan.rs` plans it: a whole issue by the issue groups, a part by a prefix, fixups
with their target.

#### Scenario: The commits a candidate moves
- **WHEN** the fix makes a candidate
- **THEN** it moves a whole issue by the issue groups, a part by a prefix, and fixups with their target

### Requirement: Candidates: A conflict moves the commit to the branch whose stack has what it needs

For a conflict, a candidate SHALL move the commit and the commits of its branch that depend on it (by lines, or by a
name it declares: the edges by symbols with `branchdeck.symbols on`, the declaration patterns for the other languages
and modes) to the branch whose stack has what it needs.

#### Scenario: The commit moves to the branch whose stack has what it needs
- **GIVEN** a conflicting commit whose branch keeps other commits
- **WHEN** the fix makes its candidates
- **THEN** one moves the commit and the commits of its branch that depend on it to the branch whose stack has what it needs
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_commit_moves_to_the_branch_whose_stack_has_what_it_needs`)

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

### Requirement: Candidates: A conflict pulls in what the commit needs, at most 6 commits

For a conflict, a candidate SHALL pull what the commit needs into the branch, at most 6 commits, from a branch
stacked on it, from the unassigned commits, or from a branch where no commit that stays depends on them.

#### Scenario: An unassigned commit the branch needs
- **GIVEN** a conflicting commit that needs an unassigned commit
- **WHEN** the fix makes its candidates
- **THEN** one pulls the unassigned commit into the branch
  [@test] crates/trial-check/src/conflict_fix_test.rs (`an_unassigned_commit_the_branch_needs_is_pulled_in`)

#### Scenario: More than 6 commits needed
- **WHEN** the commit needs more than 6 commits
- **THEN** no candidate pulls them in

#### Scenario: Where the pulled commits come from
- **WHEN** the commit needs at most 6 commits
- **THEN** a candidate pulls them from a branch stacked on the branch, from the unassigned commits, or from a branch where no commit that stays depends on them

### Requirement: Candidates: A conflict restacks the branch on what it needs

For a conflict, a candidate SHALL restack the branch on the branch whose stack has what it needs, or, when no owner
is built on every other, on a chain of the owners (each restacked on the one with the older commits, when it stays in
its stack).

#### Scenario: A branch of one commit
- **GIVEN** a conflicting branch of one commit
- **WHEN** the fix makes its candidates
- **THEN** one restacks the branch on the branch it needs
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_single_commit_branch_is_restacked_on_the_branch_it_needs`)

#### Scenario: No owner built on every other
- **GIVEN** no owner of what the commit needs is built on every other
- **WHEN** the fix makes its candidates
- **THEN** one restacks the branch on a chain of the owners, each restacked on the one with the older commits when it stays in its stack

### Requirement: Candidates: A branch on a parent of its own keeps it

A branch on a parent of its own MUST keep it: the ancestor that leaves the stack of that branch is restacked instead,
and a parent the `--plan` gives is replaced, never doubled.

#### Scenario: A leaf on a parent of its own
- **GIVEN** a conflicting branch stacked on a parent of its own that needs the lines of a sibling of the parent
- **WHEN** the fix makes its candidates
- **THEN** the branch keeps its parent, and the ancestor that leaves the stack of that branch is restacked instead
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_leaf_on_a_parent_of_its_own_keeps_it_and_the_parent_is_restacked_on_what_the_leaf_needs`)

#### Scenario: A restack on top of a plan
- **GIVEN** a `--plan` that gives the branch its parent
- **WHEN** the fix restacks on top of it
- **THEN** the branch keeps the parent the plan gives
- **AND** a parent the `--plan` gives is replaced by the restack, never doubled
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_restack_on_top_of_a_plan_keeps_the_parent_the_plan_gives`)

### Requirement: Candidates: A dangling reference pulls in the use or moves the removal

For a proven dangling reference (the branch removes a declaration that a commit of another branch stops using), a
candidate SHALL pull that commit into the branch, or move the removing commits to the branch whose stack has it.

#### Scenario: A removal of a declaration another branch stops using
- **GIVEN** a branch removes a declaration that a commit of another branch stops using
- **WHEN** the fix makes its candidates
- **THEN** one pulls that commit into the branch, another moves the removing commits to the branch whose stack has it
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)

### Requirement: Candidates: A candidate that leaves a new problem is extended and tried again

A candidate whose trial leaves a dangling reference or a new conflict in a branch it fills MUST be extended by what
that branch then needs, and tried again.

#### Scenario: A trial leaves a new conflict in a branch it fills
- **WHEN** the trial of a candidate leaves a dangling reference or a new conflict in a branch it fills
- **THEN** the candidate is extended by what that branch then needs and tried again

### Requirement: Verification and choice: A candidate is taken only when its trial build is better

A candidate MUST be taken only when its trial build has more branches that apply cleanly and no branch that applied
cleanly before fails.

#### Scenario: A trial that fixes a branch and breaks none
- **WHEN** the trial build of a candidate has more branches that apply cleanly and no branch that applied cleanly before fails
- **THEN** the candidate is verified

#### Scenario: A trial that breaks a clean branch
- **WHEN** the trial build of a candidate fails a branch that applied cleanly before
- **THEN** the candidate is not taken

### Requirement: Verification and choice: The least of the verified candidates is chosen

Among the verified candidates the fix SHALL choose the least by the failing branches, the emptied branches, the
commits that change branch or base (a restack changes the base of every commit of its branch), the rewritten
subjects.

#### Scenario: Several verified candidates
- **GIVEN** several verified candidates
- **WHEN** the fix chooses one
- **THEN** it takes the least by the failing branches, the emptied branches, the commits that change branch or base, the rewritten subjects
- **AND** a restack counts every commit of its branch as changing its base

#### Scenario: A move that keeps both branches over one that empties a branch
- **GIVEN** a parent that needs a commit of its child
- **WHEN** moving the parent's commit to the child keeps both branches and pulling the child's commit down would empty the child
- **THEN** the fix moves the parent's commit
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child`)

### Requirement: Verification and choice: The direct answer comes first, in one trial

The direct answer MUST come first, in one trial, by the line dependencies alone (before the symbols are read): every
conflicting branch whose commits all depend on the conflicting one (a move would take the whole branch, as a branch
of one commit) restacked on the branch, or the chain of branches, that holds what it needs. The search goes on for
what it leaves.

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

#### Scenario: What the direct answer leaves
- **WHEN** the direct answer leaves conflicts
- **THEN** the search goes on for them

### Requirement: Verification and choice: The conflicts are fixed before the dangling references

The conflicts MUST be fixed before the dangling references, each in build order (parents first); a fix builds on the
fixes before it.

#### Scenario: Conflicts and dangling references
- **GIVEN** conflicts and dangling references
- **WHEN** the search runs
- **THEN** it fixes the conflicts first, each in build order, parents first, each fix built on the fixes before it
- **AND** then the dangling references
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)

### Requirement: Verification and choice: A trial of a conflict looks for no dangling reference

A trial of a conflict MUST look for no dangling reference, so a dangling reference never fails it; the dangling
references of the plan so far are read once the conflicts are done and time is left (no build again), then searched.

#### Scenario: The dangling references after the conflicts
- **WHEN** the conflicts are done and time is left
- **THEN** the dangling references of the plan so far are read without a build again, then searched
- **AND** no trial of a conflict failed on a dangling reference
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)

### Requirement: Verification and choice: The search stops at a number of trials or a time

The search MUST stop at a number of trials (the app's 24, 8 per problem; `brd`'s 12 or --max-trials) or a time from
the start of the command, its first check included (the app's 60 s; `brd`'s 180 s or --time-budget), checked before
every branch build of a trial.

#### Scenario: Out of trials
- **WHEN** the search has used its trials
- **THEN** it stops
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_trials_says_so`)

#### Scenario: Out of time
- **WHEN** the time from the start of the command, its first check included, is up
- **THEN** the search stops before its next branch build, here before its first trial
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_time_stops_before_its_first_trial`)

### Requirement: Verification and choice: A cut-short search says budget with the best plan so far

A trial the time cuts MUST be thrown away, the components are not checked in parts, and `search` MUST say `budget`
with the best plan so far.

#### Scenario: The budget ends the search
- **WHEN** the trials or the time end the search
- **THEN** `search` says `budget` with the best plan so far
- **AND** a trial the time cut is thrown away, the components not checked in parts
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_trials_says_so`, `a_search_out_of_budget_ends_with_the_best_plan_so_far`)

### Requirement: Verification and choice: Nothing to fix is a complete search without trials

Nothing to fix MUST be a complete search without trials.

#### Scenario: No conflict and no dangling reference
- **WHEN** the check finds nothing to fix
- **THEN** the search is complete, without trials and without components
  [@test] crates/trial-check/src/conflict_fix_test.rs (`nothing_to_fix_is_a_complete_search_without_components`)

### Requirement: Verification and choice: A conflict no candidate fixes is unresolved

A conflict that no candidate fixes MUST be in `unresolved`.

#### Scenario: No candidate fixes the conflict
- **WHEN** no candidate fixes a conflict
- **THEN** its branch is in `unresolved`
  [@test] crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_trials_says_so`)

### Requirement: Verification and choice: The steps are grouped into components

The steps MUST be grouped into components by the branches they touch; with several, each component's plan alone
MUST keep what it fixes, or they become one.

#### Scenario: Steps that touch other branches
- **WHEN** the steps of the fix touch separate sets of branches
- **THEN** they are grouped into one component per set
- **AND** a component whose plan alone does not keep what it fixes becomes one with the others

### Requirement: `brd check --fix`: The fix in the JSON of the check

`brd check --fix` MUST add `fix` `{search, trials, components, unresolved}`, each component with its `steps`, the
whole `plan` (the `--plan` included) for `brd group`, and what it fixes, unblocks and empties.

#### Scenario: A check with a fix
- **WHEN** `brd check --fix` runs on a grouping that conflicts
- **THEN** its JSON has `fix` `{search, trials, components, unresolved}`
- **AND** each component has its `steps`, the whole `plan` for `brd group` with the `--plan` included, and what it fixes, unblocks and empties
  [@test] crates/bd-rewrite/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)
  [@test] crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

#### Scenario: A fix out of time
- **WHEN** the time of `brd check --fix` ends before a trial
- **THEN** `fix` says `search` `budget` and its conflicts in `unresolved`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_fix_out_of_time_says_budget_and_the_trial_limit_needs_fix`)

### Requirement: `brd check --fix`: The check with a fix writes objects only

`brd check --fix` MUST write objects and the object cache only.

#### Scenario: No ref changes
- **WHEN** `brd check --fix` finds a fix
- **THEN** the refs of the repository are as before; only objects and the object cache are written
  [@test] crates/bd-rewrite/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)

### Requirement: `brd check --fix`: Each trial is a line of progress on stderr

`brd check --fix` MUST print a line on stderr for the first check and for each trial, and the JSON MUST stay on
stdout.

#### Scenario: The progress of the search
- **WHEN** the search runs its trials
- **THEN** stderr has a line for the first check and one for each trial (`trial 3/12: restack
  <branch> on <parent> … verified (41 s)`, or the conflict it leaves, the branch it breaks)
- **AND** the JSON stays on stdout
  [@test] crates/trial-check/src/conflict_fix_test.rs (`each_trial_is_a_line_of_progress`)

### Requirement: App: The conflict view shows the fix as a callout

The conflict view of a sync conflict MUST show the fix as an `info` `Callout` with `WandSparklesIcon` below the
explanation; a conflict with the amended commit has none.

#### Scenario: A sync conflict
- **WHEN** the conflict view of a sync conflict shows
- **THEN** the fix is an `info` `Callout` with `WandSparklesIcon` below the explanation
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

#### Scenario: A conflict with the amended commit
- **WHEN** the conflict view of a conflict with the amended commit shows
- **THEN** it has no fix callout

### Requirement: App: The search runs when a conflict view shows, one at a time

The search MUST run when a conflict view is shown and the last sync is done, one at a time; a sync that starts MUST
cancel it (`cancel_conflict_fix`), and the next view looks again. Until it ends the callout says "Looking for a fix…"
with a spinner.

#### Scenario: A conflict view shows after the sync
- **GIVEN** the last sync is done
- **WHEN** a conflict view shows
- **THEN** the search runs, and until it ends the callout says "Looking for a fix…" with a spinner
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

#### Scenario: A new search or a sync while one runs
- **WHEN** a new search starts or a sync starts while a search runs
- **THEN** the new search stops the one in flight, and a sync cancels it with `cancel_conflict_fix`
- **AND** the next view looks again
  [@test] crates/app-api/src/reads/conflict_fix_test.rs (`a_new_search_of_the_fix_stops_the_one_in_flight_and_a_cancel_the_latest`)

### Requirement: App: A found fix holds while the commits of the table stay

A found fix MUST hold while the commits of the table stay (`commitsRevision`): a push or the deletion of an archived
branch changes only their sections, and looks for nothing again.

#### Scenario: A push or a deleted archived branch
- **GIVEN** a found fix
- **WHEN** a push or the deletion of an archived branch changes only the sections of the table
- **THEN** the fix holds and nothing is looked for again
- **AND** a change of the commits of the table looks again
  [@test] src/stores/conflict-fix.test.ts (`looks for the fix once per commits of the table: a push or a deleted archived branch looks for nothing again`)

### Requirement: App: A found fix names its steps as the Move action does

A found fix MUST name its steps as the Move action does ("Move “Show the timeout in seconds” to api", "Move the
unassigned “Add config” to auth", "Move 2 commits of x and y to auth", "Stack ui on api"): one step is the title,
more are a list under "Fix in N steps".

#### Scenario: The steps of a fix
- **WHEN** a fix is found
- **THEN** each step is said as the Move action says it ("Move “Show the timeout in seconds” to api", "Move the unassigned “Add config” to auth", "Move 2 commits of x and y to auth", "Stack ui on api")
- **AND** one step is the title, more are a list under "Fix in N steps"
  [@test] src/lib/conflict-fix.test.ts (`says a step as the Move action says it`)

### Requirement: App: Below the steps, the issue groups the fix writes and its result

Below the steps the fix MUST name the issue groups it writes ("Future IJAI-4085 commits go to IJAI-4047 too"), and the
result: what applies cleanly and syncs again, what has no commits left and where its commits went, the children of an
emptied branch, the pull requests the next push updates, and "its pull request stays open" for an emptied branch with
one.

#### Scenario: The issue groups the fix writes
- **WHEN** a fix moves a whole issue by its issue group
- **THEN** below the steps it says "Future IJAI-4085 commits go to IJAI-4047 too"
  [@test] src/lib/conflict-fix.test.ts (`says that the next commits of a whole issue follow it`)

#### Scenario: What applies cleanly and what is emptied
- **WHEN** a fix is found
- **THEN** its result names what applies cleanly and syncs again, what has no commits left and where its commits went, and the children of an emptied branch
  [@test] src/lib/conflict-fix.test.ts (`names what applies cleanly, syncs again and is emptied, and where the commits went`)

#### Scenario: The pull requests
- **WHEN** a fix changes branches with pull requests
- **THEN** its result names the pull requests the next push updates
- **AND** says "its pull request stays open" for an emptied branch with one
  [@test] src/lib/conflict-fix.test.ts (`names the pull requests the next push changes and keeps the one of an emptied branch open`)

### Requirement: App: Apply Fix applies the component of the branch as one regroup

"Apply Fix" (primary `sm`) MUST apply the component of the branch as one regroup, the rewrite `applyConflictFix` of
the engine named on the table the window shows: a sync that shows the first branch the fix fills with the moved
commits selected, then the toast "Fixed the conflict of ui" ("Fixed the conflicts of N branches") with the steps and
Undo (the rewrite `undo` of its token), the same entry in Activity.

#### Scenario: The component of the branch
- **GIVEN** a fix of several components
- **WHEN** the conflict view of a branch shows it
- **THEN** it applies the component with a step for the branch, else one that fixes or unblocks it
- **AND** its toast is "Fixed the conflict of ui", or "Fixed the conflicts of N branches"
  [@test] src/lib/conflict-fix.test.ts (`finds the component with a step for the branch, else one that fixes or unblocks it`)

#### Scenario: Apply Fix and Undo
- **WHEN** the user clicks "Apply Fix"
- **THEN** a sync shows the first branch the fix fills, the moved commits selected
- **AND** the toast "Fixed the conflict of ui" has the steps and Undo, the same entry in Activity
- **AND** Undo puts the grouping back
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

### Requirement: App: Apply Fix waits for the repository lock with Stop Waiting

While another holder has the repository lock, the panel MUST say whom it waits for, the step of its operation, with
Stop Waiting (`cancel_operation`), which changes nothing; the search of a fix (`find_conflict_fix`,
`cancel_conflict_fix`) stays a read.

#### Scenario: Another holder has the repository lock
- **GIVEN** another holder has the repository lock
- **WHEN** the user applies the fix
- **THEN** the panel says whom it waits for and the step of its operation, with Stop Waiting
- **AND** Stop Waiting (`cancel_operation`) changes nothing

#### Scenario: The search under the lock
- **WHEN** a search of a fix (`find_conflict_fix`, `cancel_conflict_fix`) runs while another holder has the repository lock
- **THEN** it stays a read

### Requirement: App: Apply Fix is disabled while a Safe Push holds a branch it changes

"Apply Fix" MUST be disabled with the reason "A Safe Push of <branch> is running" while a Safe Push holds a branch the
fix changes.

#### Scenario: A Safe Push holds a branch of the fix
- **GIVEN** a Safe Push holds a branch the fix changes
- **WHEN** the fix shows
- **THEN** "Apply Fix" is disabled with the reason "A Safe Push of <branch> is running"

### Requirement: App: Apply Fix waits until what the robots hold is known

"Apply Fix" MUST be disabled before what the robots hold is known: "Waits for the answer of Patronus" until Patronus
answered with the view of its robots, "Patronus didn't tell what its robots hold and land (…)" while it gave none
([Next Step](../next-step/spec.md#facts-without-an-answer)); never a fix on "nothing holds it" that the push guard of
Rust refuses later.

#### Scenario: Patronus has not answered yet
- **GIVEN** Patronus has not answered with the view of its robots
- **WHEN** the fix shows
- **THEN** "Apply Fix" is disabled with "Waits for the answer of Patronus"
- **AND** once Patronus answered, it is enabled and applies the fix
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should apply the fix only once Patronus told what its robots hold`)

#### Scenario: Patronus gave no view of its robots
- **GIVEN** Patronus answered without the view of its robots
- **WHEN** the fix shows
- **THEN** "Apply Fix" is disabled with "Patronus didn't tell what its robots hold and land (…)"

### Requirement: App: A fix the repository moved away from is looked for again

A fix the repository moved away from (HEAD, the issue groups or the stack parents) MUST be looked for again without
an error; any other failure MUST be "Cannot apply the fix".

#### Scenario: The repository moved away from the fix
- **WHEN** HEAD, the issue groups or the stack parents changed since the fix was found and the user applies it
- **THEN** the fix is looked for again without an error

#### Scenario: Another failure
- **WHEN** the apply fails for another reason
- **THEN** it says "Cannot apply the fix"

### Requirement: App: Without a fix the callout says so

Without a fix the callout MUST say "Branch Deck found no regroup that applies this commit cleanly" (after a cut-short
search "Branch Deck stopped looking for a fix" with Look Again).

#### Scenario: A complete search without a fix
- **WHEN** a complete search finds no fix
- **THEN** the callout says "Branch Deck found no regroup that applies this commit cleanly"

#### Scenario: A cut-short search without a fix
- **WHEN** a search cut short finds no fix
- **THEN** the callout says "Branch Deck stopped looking for a fix" with Look Again

### Requirement: App: Without a fix the callout offers a prompt for an agent

Without a fix the callout MUST offer "Copy Prompt for Agent": the conflict, the explanation, the files and brd's
trial builds (`brd check --fix`, `brd check --plan`), with a `brd group` only after the user approves.

#### Scenario: The prompt with brd
- **GIVEN** a conflict without a fix
- **WHEN** the user clicks "Copy Prompt for Agent"
- **THEN** the prompt has the conflict, the explanation, the files and brd's trial builds (`brd check --fix`, `brd check --plan`)
- **AND** a `brd group` only after the user approves
  [@test] src/lib/conflict-fix.test.ts (`hands a conflict without a fix to brd's trial builds and asks before a rewrite`)

#### Scenario: The prompt without brd
- **GIVEN** no `brd`
- **WHEN** the user copies the prompt
- **THEN** it asks the agent to show its change before a rewrite
  [@test] src/lib/conflict-fix.test.ts (`works without brd`)

### Requirement: App: The menu of a conflicting branch starts with Fix Conflict

The menu of a conflicting branch MUST start with "Fix Conflict…": it opens the row at the commit that does not apply,
where the fix is.

#### Scenario: Fix Conflict from the menu
- **WHEN** the user chooses "Fix Conflict…" in the menu of a conflicting branch
- **THEN** its row opens at the commit that does not apply, where the fix is
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should open the fix from the menu of the branch`)

### Requirement: App: The branch preview shows the fix below its conflict notice

The branch preview MUST show the fix below its conflict notice ([Branch Preview](../branch-preview/spec.md)).

#### Scenario: The preview of a conflicting branch
- **WHEN** the branch preview of a conflicting branch shows
- **THEN** the fix is below its conflict notice
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

### Requirement: App: The conflict windows show the steps read only

The conflict windows MUST show the steps of the fix found when they opened, read only
([Commit Row](../../commit/commit-row/spec.md)).

#### Scenario: A conflict window opens
- **GIVEN** a fix was found
- **WHEN** a conflict window opens
- **THEN** it shows the steps of that fix, read only

## Testing / Local Run

- `cargo nextest run -p trial-check conflict_fix`
- `brd -C <repo> check --fix` on a repository with a conflict: read-only.
- `pnpm e2e tests/e2e/conflict-fix` (the `conflict_fix` template: `ui` needs a commit of `api`).

## Risks

- A trial build proves that commits apply cleanly, not that they compile: the dangling references are a heuristic by
  the lines and the syntax trees, without a compiler.
- A fix may empty a branch with a pull request; the step list names it.

## References

- [Agent CLI](../../../../docs/contributing/agent-cli.md) (`brd check`)
- [Commit Row](../../commit/commit-row/spec.md) (the conflict view)
- [Stack Parents](../stack-parents/spec.md)
