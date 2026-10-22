---
name: Issue Groups
description: Requirements for issue groups, a stored issue key to branch name map that puts the commits of several issues into one branch without a history rewrite, written only by a regroup in the ref transaction that moves HEAD.
style: plain-1
targets:
  - ../../crates/history-rewrite/src/issue_groups.rs
  - ../../crates/history-rewrite/src/store_lines.rs
  - ../../crates/history-rewrite/src/snapshot.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../crates/history-rewrite/src/regroup.rs
  - ../../crates/history-rewrite/src/commit_ids.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/sync-core/src/grouping.rs
  - ../../crates/sync-core/src/check.rs
  - ../../crates/history-rewrite/src/autosquash.rs
  - ../../crates/sync-core/src/landed.rs
  - ../../crates/sync-core/src/update_project.rs
  - ../../crates/sync-core/src/stack_operations.rs
  - ../../crates/history-rewrite/src/move_commits.rs
  - ../../crates/history-rewrite/src/move_plan.rs
  - ../../crates/git-ops/src/commit_writer.rs
  - ../../crates/git-ops/src/ref_transaction.rs
  - ../../crates/git-ops/src/reword_commits.rs
  - ../../crates/git-ops/src/squash_commits.rs
  - ../../crates/domain-types/src/lib.rs
  - ../../crates/bd-rewrite/src/rewrite.rs
  - ../../crates/bd-remote/src/status.rs
  - ../../crates/bd-rewrite/src/suggest.rs
  - ../../crates/bd-clustering/src/clustering.rs
  - ../../crates/domain-types/src/error.rs
  - ../../src-tauri/src/commands/stack_operations.rs
  - ../../src/lib/commit.ts
  - ../../src/lib/stack.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/hooks/useMoveCommits.ts
  - ../../src/components/branch/InlineRenameBranchInput.tsx
  - ../../src/components/commit/PrefixChangeHint.tsx
---

# Issue Groups

Status: Draft
Date: 2026-10-14

## Summary

An issue group puts the commits of several issue keys into one branch, without a history rewrite. The repository
stores an issue key to branch name map. A commit without a prefix whose issue key is in the map belongs to the branch
of its key. So `ABC-1 Parse lists` and `ABC-2 Parse maps` go into `parser`, and both subjects stay as they are. Only a
regroup writes the map, under the repository lock, in the ref transaction that moves HEAD. A sync only reads it. In the
app, a move of a whole issue writes it ([Move Commits](move-commits.spec.md)): dragging the branches of `ABC-1` and
`ABC-2` onto `parser`.

## Goals

- Group the commits of related issues into one branch, and keep the rule "issue id plus one sentence" of the subjects.
- Change nothing but one ref of the repository: no commit, index or working-tree change.
- Make every change of the map safe: compare-and-swap, one transaction with HEAD, an undo.

## Non-goals

- An automatic cleanup of the keys that group no commit.

## Requirements

### Store

- The map must live in `refs/branch-deck/issue-groups`: a commit whose tree holds the file `issue-groups`, one
  `KEY<TAB>name` line per key, sorted by key. Each write must be a new commit on the previous one. A missing ref is an
  empty map.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction`)
- One `cat-file --batch` must read the map and the stack parents ([Stack Parents](stack-parents.spec.md)), in the same
  phase as the stack setting: two git processes for all three.
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`the_stack_setting_and_both_maps_cost_two_git_processes`)
- A line whose key is not an issue key (`[A-Z]+-\d+`), whose name is not a branch name without `/`, or whose key is
  listed twice must be a problem: it is reported and never used. A write must keep it as it is, so that a line of a
  newer format survives, unless the write sets or ungroups its key.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`bad_lines_are_problems_and_never_used`, `unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines`)
- The ref is shared by every branch and worktree. So a write must change only the keys that it names; a key that groups
  no commit on this HEAD must stay, because it may group the commits of another branch. Only `ungroup_issues` removes
  a key.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_write_on_one_worktree_keeps_the_groups_of_another`)

### Grouping

- The group of a commit must be its prefix (the leaf with stacked branches on), else the issue group of its issue key,
  else its issue key, else none. A fixup with the key of its target follows it into the group.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`the_rule_is_prefix_then_issue_group_then_issue_key`)
  [@test] ../../src/lib/commit.test.ts (`groupFor with issue groups`)
- The sync, `bd status`, `bd commits`, `bd check`, `bd suggest`, the regroup, the fixup plan and the landed commits
  must group with the same map. A branch that an issue group fills, without a prefix commit, must have the origin
  `IssueGroup`. The key of a commit is read as the grouper reads it, so an issue branch with a `fixup! KEY` commit stays
  `Issue`.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`, `an_issue_branch_with_a_fixup_is_an_issue_branch`)
- An issue group can be the parent of a stacked branch, and be stacked by the stack parents, without a rewrite
  ([Stack Parents](stack-parents.spec.md)).
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_issue_group_is_restacked_and_can_be_a_parent`, `a_plan_stacks_an_issue_group_without_a_prefix`)

### Writing the map

- `bd group` must read `issue_groups: {name: [keys]}` and `ungroup_issues: [keys]` in the plan. A plan without commit
  moves must rewrite no commit, honour `--expect-head`, and move only the map ref, with a check that HEAD did not move.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)
- A plan with commit moves and map changes must move HEAD and the map ref in one `update-ref --stdin`, with
  compare-and-swap on both, under the repository lock.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)
- The plan must be validated with the map as it is after the plan. These are `invalid_plan`:
  - a key in two groups of the plan, or in a group and in `ungroup_issues`;
  - a key to ungroup that is in no group and in no line of the store;
  - an invalid key or name;
  - a key that no commit without a prefix carries afterwards;
  - a fixup that the map would separate from its target, as `fixup! <id>`, which has no issue key.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)
- `unassign` of a prefixed commit must put it into the issue group of its key.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines`)
- A sync must never write the map.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_sync_reads_the_map_and_never_writes_it`)

### Rename and restack

- `bd rename` of an issue group must rename its map entries only, with no rewrite. A rename of a prefix branch must
  also rename the map entries of its name, in the same transaction.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`rename_changes_the_entries_of_an_issue_group_and_of_a_prefix_branch`)
- `bd restack` and `bd unstack` of a branch that the map fills must write the stack parents, with no rewrite.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_issue_group_is_restacked_and_can_be_a_parent`)
  [@test] ../../crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

### Undo

- The undo token must be `H1..H2` for a rewrite only, `groups:G1..G2` for a change of the map only, and
  `H1..H2+groups:G1..G2` for both, followed by `+stacks:S1..S2` when the stack parents moved too
  ([Stack Parents](stack-parents.spec.md#writing-the-map)); a `G1` of zeros means that the map did not exist. The ids must be full (40 or 64
  hex digits), and anything else is `invalid_arguments`.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)
- `bd undo` must move both refs back in one transaction. `G2` must be a write of the map on `G1`: its parent, or no
  parent when `G1` is zeros; else the token is `invalid_arguments`. It must refuse with `head_moved` when HEAD moved,
  and with `groups_moved` (exit 3, retryable) when the map moved, also when a ref moves between its checks and its
  transaction.
  [@test] ../../crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`, `a_mixed_plan_moves_head_and_the_store_in_one_transaction`, `an_undo_of_the_map_needs_the_written_commit_on_its_previous_one`)
  [@test] ../../crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`)

### `bd status` and `bd suggest`

- `bd status` must print the `origin` of each branch, the `issue_keys` of an issue group, `idle_issue_groups` (the
  keys that group no commit on this HEAD; they may group the commits of another branch) and `issue_group_problems`.
  [@test] ../../crates/bd-remote/src/status_test.rs (`issue_groups_in_status`)
- `bd suggest` must put a cluster of whole issue branches into `issue_groups` of its plan, so that it rewrites no
  commit; `--rewrite-issues` gives the commits a prefix instead. An issue group is an anchor, as a prefix is.
  [@test] ../../crates/bd-rewrite/src/suggest_test.rs (`whole_issue_branches_become_an_issue_group`)

### App

- The app must show an issue group as a branch, nest a single-key issue group under its issue, and rename it through
  the rename core; naming an issue branch must write the map, not a prefix.
- A move of a whole issue must write the map, a move back of one must remove its key (`plan_move`, see
  [Move Commits](move-commits.spec.md)); "Ungroup" of an issue group moves all its issues back.
  [@test] ../../crates/sync-core/src/move_commits_test.rs (`whole_issues_join_a_branch_without_a_rewrite_and_their_next_commits_follow`, `moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them`)
- The undo of a move, a rename and a restack must be `bd undo` of the token of the result (`undo_regroup`): HEAD and
  the map in one transaction.
  [@test] ../../tests/e2e/issue-groups/issue-groups.spec.ts (`should rename an issue group without a rewrite and undo the rename`)
- The commit editor and "Move Back to …" must name the issue group of a key.
  [@test] ../../src/hooks/useCommitMoveList.test.ts (`names the issue group the commits fall back to`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/issue_groups_test.rs`, `crates/bd-remote/src/status_test.rs`,
  `crates/bd-rewrite/src/suggest_test.rs`.
- `pnpm test:unit`: `src/lib/commit.test.ts`, `src/hooks/useCommitMoveList.test.ts`.
- `pnpm e2e`: `tests/e2e/issue-groups/issue-groups.spec.ts`, on the `issue_groups` template;
  `tests/e2e/move-commits/combine-branches.spec.ts`, on the `issue_combine` template.

## Open Questions / Risks

- git keeps no reflog for `refs/branch-deck/issue-groups`. The parent chain of its commits is the history of the map.
- A move of a whole issue writes a key that the other worktrees share: its commits there move too.

## References

- [Virtual Branches](../../docs/design/virtual-branches.md)
- [Git Operations](../../docs/design/git-operations.md)
- [Agent CLI](../cli/agent-cli.spec.md)
