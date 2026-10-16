---
name: Conflict Fix
description: Requirements for the fix of a sync conflict, the regroup after which the branches that conflict (or remove a declaration that another branch's commit stops using) apply cleanly, found by trial builds and offered, never applied without the user.
style: plain-1
targets:
  - ../../crates/sync-core/src/conflict_fix.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/sync-core/src/missing_commits.rs
  - ../../crates/sync-core/src/move_plan.rs
  - ../../crates/sync-core/src/dangling_references.rs
  - ../../crates/bd-cli/src/check_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
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
- For a conflict: move the commit and the commits of its branch that depend on it (by lines, or by a name it declares)
  to the branch whose stack has what it needs; pull what it needs into the branch, at most 6 commits, from a branch
  stacked on it, from the unassigned commits, or from a branch where no commit that stays depends on them; restack the
  branch on that branch.
  [@test] ../../crates/sync-core/src/conflict_fix_test.rs (`a_commit_moves_to_the_branch_whose_stack_has_what_it_needs`, `a_single_commit_branch_is_restacked_on_the_branch_it_needs`, `an_unassigned_commit_the_branch_needs_is_pulled_in`, `a_parent_that_needs_a_commit_of_its_child_gives_its_own_commit_to_the_child`)
- For a proven dangling reference (the branch removes a declaration that a commit of another branch stops using): pull
  that commit into the branch, or move the removing commits to the branch whose stack has it.
  [@test] ../../crates/sync-core/src/conflict_fix_test.rs (`a_dangling_reference_moves_the_removal_to_the_branch_that_fixes_the_use`)
- A candidate whose trial leaves a dangling reference or a new conflict in a branch it fills must be extended by what
  that branch then needs, and tried again.

### Verification and choice

- A candidate must be taken only when its trial build has more branches that apply cleanly and no branch that applied
  cleanly before fails. Among the verified ones the least by the failing branches, the emptied branches, the commits
  that change branch or base (a restack changes the base of every commit of its branch), the rewritten subjects.
- The conflicts must be fixed before the dangling references, each in build order (parents first); a fix builds on
  the fixes before it.
- The search must stop at a number of trials (24, 8 per problem) or a time (20 s of trials); then `search` says
  `budget`. Nothing to fix is a complete search without trials.
  [@test] ../../crates/sync-core/src/conflict_fix_test.rs (`a_search_out_of_trials_says_so`, `nothing_to_fix_is_a_complete_search_without_components`)
- A conflict that no candidate fixes must be in `unresolved`.
  [@test] ../../crates/sync-core/src/conflict_fix_test.rs (`a_commit_that_needs_two_long_branches_without_stacks_has_no_fix`)
- The steps must be grouped into components by the branches they touch; with several, each component's plan alone
  must keep what it fixes, or they become one.

### `bd check --fix`

- `bd check --fix` must add `fix` `{search, trials, components, unresolved}`, each component with its `steps`, the
  whole `plan` (the `--plan` included) for `bd group`, and what it fixes, unblocks and empties; it writes objects and
  the object cache only.
  [@test] ../../crates/bd-cli/src/check_test.rs (`fix_finds_the_regroup_after_which_the_plan_applies_and_writes_nothing`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`outputs_match_schema`)

## Testing / Local Run

- `cargo nextest run -p sync-core conflict_fix`
- `bd -C <repo> check --fix` on a repository with a conflict: read-only.

## Risks

- A trial build proves that commits apply cleanly, not that they compile: the dangling references are a heuristic by
  the lines.
- A fix may empty a branch with a pull request; the step list names it.

## References

- [Agent CLI](../../docs/guidelines/agent-cli.md) (`bd check`)
- [Commit Row](../commit/commit-row.spec.md) (the conflict view)
- [Stack Parents](stack-parents.spec.md)
