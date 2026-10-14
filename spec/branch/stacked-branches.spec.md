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
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/sync-types/src/stack.rs
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/components/branch/InlineUngroupConfirm.tsx
  - ../../src/components/branch/InlineRenameBranchInput.tsx
  - ../../crates/sync-core/src/stack_operations.rs
  - ../../src/lib/commit.ts
  - ../../src/lib/stack.ts
  - ../../src/components/commit/PrefixChangeHint.tsx
  - ../../src/components/shared/InlineBranchTarget.tsx
  - ../../src/hooks/useBranchNameValidation.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/git/push.ts
  - ../../src/components/branch/StackBaseLabel.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/lib/git/stacked-push-toast.tsx
---

# Stacked Branches

Status: Draft
Date: 2026-10-14

## Summary

A commit `(auth/login) Add form` belongs to the branch `login`, and `login` stacks on `auth`. Its virtual branch is
built on the new head of `auth`'s virtual branch, so a pull request for `login` contains only its own commits on top
of `auth`. Deeper prefixes declare the ancestry (`(auth/login/oauth)`), and a bare `(login)` joins `login` wherever
its parent is declared. A branch that can't be placed, or whose parent can't be built on, is reported and not rebuilt;
the other branches still sync. The branch table shows the stacks as a tree, and the branch menu stacks, unstacks and
renames branches.

## Goals

- Review a feature in dependent steps, each a branch with its own commits.
- Keep linear development on the main branch: the prefix is the only stack state.
- Never re-base a branch silently: a child whose parent is missing stays where it was.

## Non-goals

- Create or retarget the pull request itself: Branch Deck copies the `gh` command.
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
- A parent without commits must be `Unapplied` with an unapplied branch of its name, `Integrated` when its branch is
  integrated, else `Missing`. Its branch is its virtual branch, which the sync is about to archive, else its newest
  archived branch; a detection note says whether it is integrated.
  [@test] ../../crates/sync-core/src/stack_test.rs (`reads_the_state_of_parents_without_commits_from_their_branches`)
- A virtual branch of a parent without commits and without a note must be detected before the branches are placed, so
  the first sync after the parent's merge already moves the child. The sync writes the note, and its archiving reports
  the parent from it; `bd status` and `bd check` detect without writing.
  [@test] ../../crates/sync-core/src/stack_test.rs (`a_parent_whose_virtual_branch_lost_its_commits_is_detected_before_the_placement`)
  [@test] ../../crates/integration-detection-tests/src/stack_tests.rs (`the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline`)
  [@test] ../../crates/bd-cli/src/status_test.rs (`a_parent_integrated_since_the_last_sync_is_integrated_before_the_next_one`)
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

### Pull requests

- The pull request base of a stacked branch must be its parent's pushed branch; of a root, the baseline as the remote
  names it (`master`). "Copy PR Base" in the branch menu must copy it.
  [@test] ../../src/lib/stack.test.ts (`targets the parent's branch, or the baseline on the remote`)
- After a push, the toast must name the base and offer to copy `gh pr create --base … --head …`. An open pull request
  whose base (`baseRefName`) is not the expected one, e.g. after its parent was integrated or the branch was
  restacked, must get "retarget it" with `gh pr edit … --base …` in the toast and "Copy Retarget Command" in the
  branch menu.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to retarget a pull request whose base is not the parent`)
  [@test] ../../crates/sync-core/src/pull_requests_test.rs (`lists_pull_requests_of_the_push_remote_repository`)
- The push button of a branch whose parent has work for the remote must stay as it is (`Create Remote (1)`, enabled):
  pushing the branch alone is legitimate. Its description must make the choice explicit: below the push's own lines,
  after a rule, "login isn't on the remote yet. Pushing oauth alone works; a pull request includes login's commits
  until login is pushed." ("login has commits the remote lacks. … includes them until login is pushed." for a parent
  on the remote), then a `Push Stack (3 branches)` button, named as in the branch menu. In this state only, the Status
  cell shows the muted "on login" in its third column.
  [@test] ../../src/hooks/usePushButton.test.ts (`offers Push Stack and says what a push of the branch alone does while the parent has work for the remote`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and name the pull request base`)
- "Push Stack" (in that description or the branch menu) must push the ancestors with unpushed work, root first, then
  the branch, and stop at a failure.
  [@test] ../../src/lib/stack.test.ts (`pushes the ancestors that have work for the remote first, root first`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and name the pull request base`)

### Messages

- `groupFor` must read a prefix as the backend does: the leaf, its parent and an empty level.
  [@test] ../../src/lib/commit.test.ts (`reads %j as the backend stack_path does`, `keeps a slash in a flat name without stacked branches`)
- The commit message editor must say when an edit restacks the branch, and warn when it would make the branch's
  parents conflict or name a parent without commits.
  [@test] ../../src/lib/stack.test.ts (`names the new branch and its parent`, `warns about a parent the other commits of the branch don't declare`, `says a single-commit branch is restacked or unstacked`, `warns about a parent without commits`)

### Fixups

- With stacks on, a fixup's prefix must agree with its target's when both name the same leaf, so `fixup! (login) X`
  finds `(auth/login) X`.
  [@test] ../../crates/sync-core/src/autosquash_test.rs (`with_stacks_a_fixup_names_the_leaf_of_its_target`, `with_stacks_another_leaf_does_not_agree`, `the_plan_reads_the_stacks_setting_of_the_repository`)

### UI

- The branch table must show each branch under the branch it stacks on, depth-first and newest first among siblings,
  with one rule per level under the icon of the row above; its expanded commits follow the depth, and the rail of a
  parent's commits runs into the rule of its children ([Branch Row](branch-row.spec.md)). A branch whose parent has no
  row, or whose parents form a cycle, stays at the top level.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a branch whose parent has no row, or whose parents form a cycle, at the top level`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should continue the commit rail of a parent into the rule of its children`)
- Issue families must be formed from the stack roots only. A stacked branch of another issue than its parent shows its
  issue as a chip. Without stacked branches the table is as before.
  [@test] ../../src/stores/branch-table-rows.test.ts (`forms issue families from the stack roots only and marks a child of another issue`)
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)
- The branch menu must offer "Stack On…", a submenu of the other branches without the branch's descendants and its
  parent, and "Unstack from <parent>" for a stacked branch (`restack_branch`, with an undo).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should stack a branch on another one and unstack it`, `should not offer a branch's own descendants to stack on`)
  [@test] ../../crates/sync-core/src/stack_operations_test.rs (`restacks_and_unstacks_a_branch`, `renaming_a_parent_renames_it_in_its_children`)
- Rename of a parent must keep its children stacked on it; Ungroup and Unapply of a parent must name the children
  they block. See [Move Commits](move-commits.spec.md).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should rename a parent with its children following`, `should confirm ungrouping a parent and list its children`)
- A blocked branch must show a muted `blocked by <parent>` status after a `warning` dot with the reason as its tooltip,
  a disabled push button (in the row, the Outgoing overview and the preview), no share in the outgoing counts, and,
  expanded, a `warning` alert with a link that reveals the parent.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] ../../src/lib/stack.test.ts (`leaves out a blocked branch: it offers no push`)
- Where the tree isn't visible, a stacked branch must say what it is built on ("on auth"): the Outgoing overview and the
  branch preview. The branch row says it only while the parent has work for the remote ([Pull requests](#pull-requests)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
- A push of a stacked branch must count only its own commits as unpushed afterwards (`parentBranch`).

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/stack_test.rs`, `stack_sync_test.rs`, `commit_grouper_test.rs`,
  `autosquash_test.rs`.
- `git config branchdeck.stacks true` in a repository, then commit with `(parent/child)` prefixes.

## Open Questions / Risks

- A parent integrated by a squash merge is `Missing` under the default rebase detection, so its child stays blocked
  until the user restacks it.

## References

- [Branch Row](branch-row.spec.md): the tree rules and the blocked row
- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Git Operations](../../docs/design/git-operations.md)
- [Fixups](../commit/fixups.spec.md)
