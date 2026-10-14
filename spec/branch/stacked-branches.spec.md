---
name: Stacked Branches
description: Requirements for stacked branches, where a `(parent/child)` prefix builds the child's virtual branch on its parent's, with problems and blocked parents reported per branch and the setting opt-in per repository.
style: plain-1
targets:
  - ../../crates/sync-core/src/stack.rs
  - ../../crates/sync-core/src/commit_grouper.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/branch_processor.rs
  - ../../crates/sync-core/src/autosquash.rs
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/branch-integration/src/detector.rs
  - ../../crates/sync-types/src/stack.rs
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/BranchActions.tsx
---

# Stacked Branches

Status: Draft
Date: 2026-10-14

## Summary

A commit `(auth/login) Add form` belongs to the branch `login`, and `login` stacks on `auth`. Its virtual branch is
built on the new head of `auth`'s virtual branch, so a pull request for `login` contains only its own commits on top
of `auth`. Deeper prefixes declare the ancestry (`(auth/login/oauth)`), and a bare `(login)` joins `login` wherever
its parent is declared. A branch that can't be placed, or whose parent can't be built on, is reported and not rebuilt;
the other branches still sync. The backend is done; the branch table does not show the stack yet.

## Goals

- Review a feature in dependent steps, each a branch with its own commits.
- Keep linear development on the main branch: the prefix is the only stack state.
- Never re-base a branch silently: a child whose parent is missing stays where it was.

## Non-goals

- Rename, ungroup or unapply a parent together with its children (later).
- Show the stack in the branch table, or set the base of a pull request (later).
- Stacked branches in the `bd` CLI (later).

## Requirements

### Opt-in

- Stacks must be on only when `git config branchdeck.stacks` is true. Without the setting, a `/` in a prefix must stay
  part of a flat branch name and every event must be as before, with a null `stack`.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`without_stacks_a_slash_is_part_of_a_flat_name`, `stacks_are_off_unless_configured`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`without_stacks_branches_grouped_has_no_stack`)

### Prefix syntax

- The last level of a prefix must name the branch and the level before it its parent. Each level must be sanitized
  like a flat branch name. Empty levels at the ends must be dropped; an empty level in the middle (`(a//b)`) is a
  syntax problem of the last level's branch.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`stack_prefixes_split_into_sanitized_levels`, `an_empty_stack_level_is_a_syntax_error`, `a_syntax_error_is_reported_on_the_last_level`)
- Commits must be grouped by the leaf, and every level pair must declare an edge: `(auth/login/oauth)` also declares
  `auth` as the parent of `login`. An issue-key branch is a root.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`groups_by_leaf_and_records_the_declared_parents`, `a_parent_without_commits_is_missing`)

### Problems

- These problems must mark only the affected branch, with the `Error` status, naming the commits; the branch is not
  rebuilt:
  - commits declare different parents;
  - the parents form a cycle (every branch of the cycle);
  - `(a/a)`;
  - two branches differ only in case (both);
  - a syntax problem.
  [@test] ../../crates/sync-core/src/commit_grouper_test.rs (`conflicting_parents_name_their_commits`, `a_branch_can_not_be_its_own_parent`, `a_cycle_marks_each_of_its_branches`, `leaves_that_differ_only_in_case_collide`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_missing_parent_or_a_problem_is_not_built`)

### Placement

- A child must be built on the new head of its active parent's virtual branch, also when a child commit comes before
  every parent commit on the main branch (`childPrecedesParent`).
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_child_is_built_on_its_active_parent`)
- A parent without commits must be `Unapplied` with an unapplied branch of its name, `Integrated` when the detection
  note of its newest archived branch says so, else `Missing`.
  [@test] ../../crates/sync-core/src/stack_test.rs (`reads_the_state_of_parents_without_commits_from_their_branches`)
- An integrated parent must move the child to its nearest active ancestor, or the baseline, with `prBaseChanged`.
  [@test] ../../crates/sync-core/src/stack_test.rs (`an_integrated_parent_moves_the_child_to_the_nearest_active_ancestor`)
- A missing or unapplied parent, a parent with a problem and a blocked parent must block the child: the `Blocked`
  status, the reason as its text, its commits blocked, its virtual branch left as it was.
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_missing_or_unapplied_parent_blocks_the_child_and_its_descendants`, `a_parent_with_a_problem_blocks_its_children`)
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`an_unapplied_parent_is_reported_before_the_sync_starts`)
- A parent that conflicts or fails while it is built must block its descendants the same way.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`a_parent_conflict_blocks_its_descendants`)

### Sync

- A child must start once its parent's head is written, without holding a slot of the git process limit while it
  waits. Ready branches must start longest chain first.
- A new parent commit must move the children onto the parent's new head; an unchanged stack must reuse every commit.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`builds_each_branch_on_its_parent`)

### Remote status

- The unpushed commits of a child must exclude its parent's commits (`^<parent ref>` next to the baseline exclusion),
  after a sync and after a push of the child.
  [@test] ../../crates/sync-core/src/stack_sync_test.rs (`the_unpushed_commits_of_a_child_exclude_its_parent`)

### Integration detection

- A rebuilt child must get a stack note (`refs/notes/branch-deck/stack`) on its tip naming the commit it was built on,
  written with the mapping notes in one `fast-import` run.
- Detection of an archived child must look only at the commits after that base: `^<base>` in the cherry-mark scans and
  the merge count, the base as the merge base of the squash check. A child whose own commits are upstream is
  integrated even if its parent is not.
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`an_archived_child_is_integrated_when_its_own_commits_are`)
- Only an `Integrated` result of a stacked child may be cached; another one is detected again on the next sync.
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`an_archived_child_counts_only_its_own_commits_and_is_not_cached_until_integrated`)

### Fixups

- With stacks on, a fixup's prefix must agree with its target's when both name the same leaf, so `fixup! (login) X`
  finds `(auth/login) X`.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`, `with_stacks_another_leaf_does_not_agree`, `the_plan_reads_the_stacks_setting_of_the_repository`)

### UI

- The branch table must show each branch under the branch it stacks on, depth-first and newest first among siblings,
  with one rule per level under the icon of the row above; its expanded commits follow the depth. A branch whose
  parent has no row, or whose parents form a cycle, stays at the top level.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a branch whose parent has no row, or whose parents form a cycle, at the top level`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
- Issue families must be formed from the stack roots only. A stacked branch of another issue than its parent shows its
  issue as a chip. Without stacked branches the table is as before.
  [@test] ../../src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)
- A blocked branch must show a muted `blocked by <parent>` status with the reason as its tooltip, a disabled push
  button, and, expanded, an info alert with a link that reveals the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_test.rs`, `stack_sync_test.rs`, `commit_grouper_test.rs`,
  `autosquash_test.rs`.
- `git config branchdeck.stacks true` in a repository, then commit with `(parent/child)` prefixes.

## Open Questions / Risks

- An integrated parent is recognized only once its archived branch has a detection note, so the first sync after the
  parent's merge blocks the child (`Missing`); the next one moves it.

## References

- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Git Operations](../../docs/design/git-operations.md)
- [Fixups](../commit/fixups.spec.md)
