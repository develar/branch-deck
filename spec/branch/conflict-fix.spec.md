---
name: Conflict Fix
description: Requirements for the fix of a sync conflict, the regroup after which the branches that conflict (or remove a declaration that another branch's commit stops using) apply cleanly, found by trial builds and offered, never applied without the user.
style: plain-1
targets:
  - ../../crates/trial-check/src/conflict_fix.rs
  - ../../crates/trial-check/src/check.rs
  - ../../crates/trial-check/src/missing_commits.rs
  - ../../crates/history-rewrite/src/move_plan.rs
  - ../../crates/trial-check/src/dangling_references.rs
  - ../../crates/bd-rewrite/src/check_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../src-tauri/src/commands/conflict_fix.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../src/stores/conflict-fix.ts
  - ../../src/lib/conflict-fix.ts
  - ../../src/lib/conflict-fix-prompt.ts
  - ../../src/components/conflict/ConflictFixPanel.tsx
  - ../../src/components/conflict/MergeConflictViewer.tsx
  - ../../src/hooks/useAgentCliCommand.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/conflict/ConflictWindowPage.tsx
---

# Conflict Fix

Status: Implemented
Date: 2026-10-16

## Summary

A virtual branch conflicts when one of its commits changes lines that a commit of another branch changed first. The
conflict view used to say so ("45 missing commits modified the conflicting files") and to list the ways out as text.
The fix does the work: it tries regroups in trial builds (`bd check`'s, objects only) and keeps the one after which the
branches apply cleanly, with the fewest changes the user notices.

In one monorepo checkout, a commit of `IJAI-3852` (stacked on `IJAI-3828`) missed 42 commits of 12 branches, all of
the stack of `IJAI-4047`, and `IJAI-4047` itself needed a commit of a branch stacked on it. The fix found the regroup
that a person found by hand: two commits into `IJAI-4047`, one of them a whole issue by its issue group, without a
rewrite.

## Goals

- Find a regroup that fixes the conflicts of the current grouping, verified by a trial build, within a bounded time.
- Prefer the fix the user notices least: no branch that applied cleanly fails, few emptied branches, few moved commits.
- Say what the fix does in steps (move, pull in, restack), what it fixes and what it empties.
- Offer it to an agent as a plan for `bd group`.

## Non-goals

- Editing a commit. A conflict that no regroup fixes stays; `bd move-hunk` and an agent resolve it.
- Applying a fix without the user: `bd check --fix` writes no ref.

## Requirements

### What a commit needs

- What a conflicting commit needs must be the commits it depends on by its lines (`dependencies.rs`, with fixup units),
  and what those need in turn, that neither its branch nor the branches it is built on have; the missing commits by
  the changed files only when the lines name none.
- The branch whose stack holds what it needs is an owner built on every other owner (the deepest), or the only owner
  with stacked branches off.

### Candidates

- A candidate must be a move as `move_plan.rs` plans it: a whole issue by the issue groups, a part by a prefix, fixups
  with their target.
- For a conflict: move the commit and the commits of its branch that depend on it (by lines, or by a name it declares:
  the edges by symbols with `branchdeck.symbols on`, the declaration patterns for the other languages and modes)
  to the branch whose stack has what it needs; pull what it needs into the branch, at most 6 commits, from a branch
  stacked on it, from the unassigned commits, or from a branch where no commit that stays depends on them; restack the
  branch on that branch, or, when no owner is built on every other, on a chain of the owners (each restacked on the
  one with the older commits, when it stays in its stack). A branch on a parent of its own keeps it: the ancestor that
  leaves the stack of that branch is restacked instead, and a parent the `--plan` gives is replaced, never doubled.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_leaf_on_a_parent_of_its_own_keeps_it_and_the_parent_is_restacked_on_what_the_leaf_needs`, `a_restack_on_top_of_a_plan_keeps_the_parent_the_plan_gives`)
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_commit_moves_to_the_branch_whose_stack_has_what_it_needs`, `a_single_commit_branch_is_restacked_on_the_branch_it_needs`, `an_unassigned_commit_the_branch_needs_is_pulled_in`, `a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child`, `the_fixes_are_the_same_with_the_symbols_off`)
- For a proven dangling reference (the branch removes a declaration that a commit of another branch stops using): pull
  that commit into the branch, or move the removing commits to the branch whose stack has it.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)
- A candidate whose trial leaves a dangling reference or a new conflict in a branch it fills must be extended by what
  that branch then needs, and tried again.

### Verification and choice

- A candidate must be taken only when its trial build has more branches that apply cleanly and no branch that applied
  cleanly before fails. Among the verified ones the least by the failing branches, the emptied branches, the commits
  that change branch or base (a restack changes the base of every commit of its branch), the rewritten subjects.
- The direct answer must come first, in one trial, by the line dependencies alone (before the symbols are read): every
  conflicting branch whose commits all depend on the conflicting one (a move would take the whole branch, as a branch
  of one commit) restacked on the branch, or the chain of branches, that holds what it needs. The search goes on for
  what it leaves.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_leaf_whose_commit_needs_two_other_leaves_is_restacked_on_their_chain_in_one_trial`, `a_single_commit_branch_is_restacked_on_the_branch_it_needs`)
- The conflicts must be fixed before the dangling references, each in build order (parents first); a fix builds on
  the fixes before it. A trial of a conflict looks for no dangling reference, so a dangling reference never fails it;
  the dangling references of the plan so far are read once the conflicts are done and time is left (no build again),
  then searched.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)
- The search must stop at a number of trials (the app's 24, 8 per problem; `bd`'s 12 or --max-trials) or a time from
  the start of the command, its first check included (the app's 60 s; `bd`'s 180 s or --time-budget), checked before
  every branch build of a trial: a trial the time cuts is thrown away, the components are not checked in parts, and
  `search` says `budget` with the best plan so far. Nothing to fix is a complete search without trials.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_search_out_of_trials_says_so`, `a_search_out_of_budget_ends_with_the_best_plan_so_far`, `a_search_out_of_time_stops_before_its_first_trial`, `nothing_to_fix_is_a_complete_search_without_components`)
- A conflict that no candidate fixes must be in `unresolved`.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`a_commit_that_needs_two_long_branches_without_stacks_has_no_fix`)
- The steps must be grouped into components by the branches they touch; with several, each component's plan alone
  must keep what it fixes, or they become one.

### `bd check --fix`

- `bd check --fix` must add `fix` `{search, trials, components, unresolved}`, each component with its `steps`, the
  whole `plan` (the `--plan` included) for `bd group`, and what it fixes, unblocks and empties; it writes objects and
  the object cache only. It prints a line on stderr for the first check and for each trial (`trial 3/12: restack
  <branch> on <parent> … verified (41 s)`, or the conflict it leaves, the branch it breaks), the JSON stays on stdout.
  [@test] ../../crates/trial-check/src/conflict_fix_test.rs (`each_trial_is_a_line_of_progress`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`a_fix_out_of_time_says_budget_and_the_trial_limit_needs_fix`)
  [@test] ../../crates/bd-rewrite/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

### App

- The conflict view of a sync conflict must show the fix as an `info` `Callout` with `WandSparklesIcon` below the
  explanation; a conflict with the amended commit has none. The search runs when a conflict view is shown and the
  last sync is done, one at a time; a sync that starts cancels it (`cancel_conflict_fix`), and the next view looks again.
  Until it ends the callout says "Looking for a fix…" with a spinner.
  [@test] ../../tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)
- A found fix names its steps as the Move action does ("Move “Show the timeout in seconds” to api", "Move the
  unassigned “Add config” to auth", "Move 2 commits of x and y to auth", "Stack ui on api"): one step is the title,
  more are a list under "Fix in N steps". Below: the issue groups it writes ("Future IJAI-4085 commits go to IJAI-4047
  too"), and the result: what applies cleanly and syncs again, what has no commits left and where its commits went,
  the children of an emptied branch, the pull requests the next push updates, and "its pull request stays open" for
  an emptied branch with one.
  [@test] ../../src/lib/conflict-fix.test.ts (`says a step as the Move action says it`, `names what applies cleanly, syncs again and is emptied, and where the commits went`, `names the pull requests the next push changes and keeps the one of an emptied branch open`, `says that the next commits of a whole issue follow it`, `finds the component with a step for the branch, else one that fixes or unblocks it`)
- "Apply Fix" (primary `sm`) applies the component of the branch as one regroup: the toast "Fixed the conflict of ui"
  ("Fixed the conflicts of N branches") with the steps and Undo, the same entry in Activity, then a sync that shows
  the first branch the fix fills with the moved commits selected. It is disabled with the reason "A Safe Push of
  <branch> is running" while a Safe Push holds a branch the fix changes, and before what the robots hold is known: "Waits
  for the answer of Patronus" until Patronus answered with the view of its robots, "Patronus didn't tell what its
  robots hold and land (…)" while it gave none ([Next Step](next-step.spec.md#facts-without-an-answer)); never a fix on
  "nothing holds it" that the push guard of Rust refuses later. A fix the repository moved away from (HEAD, the issue
  groups or the stack parents) is looked for again without an error; any other failure is "Cannot apply the fix".
  [@test] ../../tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)
  [@test] ../../tests/e2e/conflict-fix/conflict-fix.spec.ts (`should apply the fix only once Patronus told what its robots hold`)
- Without a fix the callout says "Branch Deck found no regroup that applies this commit cleanly" (after a cut-short
  search "Branch Deck stopped looking for a fix" with Look Again) and offers "Copy Prompt for Agent": the conflict, the
  explanation, the files and bd's trial builds (`bd check --fix`, `bd check --plan`), with a `bd group` only after the
  user approves.
  [@test] ../../src/lib/conflict-fix.test.ts (`hands a conflict without a fix to bd's trial builds and asks before a rewrite`, `works without bd`)
- The menu of a conflicting branch must start with "Fix Conflict…": it opens the row at the commit that does not apply,
  where the fix is.
  [@test] ../../tests/e2e/conflict-fix/conflict-fix.spec.ts (`should open the fix from the menu of the branch`)
- The branch preview must show the fix below its conflict notice ([Branch Preview](branch-preview.spec.md)).
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)
- The conflict windows must show the steps of the fix found when they opened, read only ([Commit Row](../commit/commit-row.spec.md)).

## Testing / Local Run

- `cargo nextest run -p trial-check conflict_fix`
- `bd -C <repo> check --fix` on a repository with a conflict: read-only.
- `pnpm e2e tests/e2e/conflict-fix` (the `conflict_fix` template: `ui` needs a commit of `api`).

## Risks

- A trial build proves that commits apply cleanly, not that they compile: the dangling references are a heuristic by
  the lines and the syntax trees, without a compiler.
- A fix may empty a branch with a pull request; the step list names it.

## References

- [Agent CLI](../../docs/guidelines/agent-cli.md) (`bd check`)
- [Commit Row](../commit/commit-row.spec.md) (the conflict view)
- [Stack Parents](stack-parents.spec.md)
