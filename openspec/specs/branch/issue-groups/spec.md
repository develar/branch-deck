---
name: Issue Groups
description: Requirements for issue groups, a stored issue key to branch name map that puts the commits of several issues into one branch without a history rewrite, written only by a regroup in the ref transaction that moves HEAD.
style: plain-1
targets:
  - crates/history-rewrite/src/issue_groups.rs
  - crates/history-rewrite/src/store_lines.rs
  - crates/history-rewrite/src/snapshot.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/history-rewrite/src/commit_ids.rs
  - crates/sync-core/src/sync.rs
  - crates/sync-core/src/grouping.rs
  - crates/trial-check/src/check.rs
  - crates/history-rewrite/src/autosquash.rs
  - crates/git-ops/src/autosquash_targets.rs
  - crates/sync-core/src/landed.rs
  - crates/update-core/src/update_project.rs
  - crates/operations/src/rewrite.rs
  - crates/history-rewrite/src/move_commits.rs
  - crates/history-rewrite/src/move_plan.rs
  - crates/git-ops/src/commit_writer.rs
  - crates/git-ops/src/ref_transaction.rs
  - crates/git-ops/src/reword_commits.rs
  - crates/git-ops/src/squash_commits.rs
  - crates/domain-types/src/lib.rs
  - crates/bd-rewrite/src/rewrite.rs
  - crates/bd-remote/src/status.rs
  - crates/bd-rewrite/src/suggest.rs
  - crates/commit-clustering/src/clustering.rs
  - crates/domain-types/src/error.rs
  - crates/app-api/src/reads/commits.rs
  - src/lib/stack.ts
  - src/stores/branch-sync.ts
  - src/hooks/useCommitMoveList.ts
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useStackActions.ts
  - src/hooks/useMoveCommits.ts
  - src/components/branch/InlineRenameBranchInput.tsx
  - src/components/commit/PrefixChangeHint.tsx
---

# Issue Groups

Status: Draft
Date: 2026-10-14

## Purpose

An issue group puts the commits of several issue keys into one branch, without a history rewrite. The repository
stores an issue key to branch name map. A commit without a prefix whose issue key is in the map belongs to the branch
of its key. So `ABC-1 Parse lists` and `ABC-2 Parse maps` go into `parser`, and both subjects stay as they are. Only a
regroup writes the map, under the repository lock, in the ref transaction that moves HEAD. A sync only reads it. In the
app, a move of a whole issue writes it ([Move Commits](../move-commits/spec.md)): dragging the branches of `ABC-1` and
`ABC-2` onto `parser`.

## Goals

- Group the commits of related issues into one branch, and keep the rule "issue id plus one sentence" of the subjects.
- Change nothing but one ref of the repository: no commit, index or working-tree change.
- Make every change of the map safe: compare-and-swap, one transaction with HEAD, an undo.

## Non-goals

- An automatic cleanup of the keys that group no commit.

## Requirements

### Requirement: Store: The map lives in `refs/branch-deck/issue-groups`

The map MUST live in `refs/branch-deck/issue-groups`: a commit whose tree holds the file `issue-groups`, one
`KEY<TAB>name` line per key, sorted by key. A missing ref is an empty map.

#### Scenario: No ref
- **GIVEN** no `refs/branch-deck/issue-groups`
- **WHEN** the map is read
- **THEN** it is an empty map
  [@test] crates/sync-core/src/issue_groups_test.rs (`reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction`)

#### Scenario: The written map
- **WHEN** a write of the map lands
- **THEN** the ref names a commit whose tree holds the file `issue-groups`, one `KEY<TAB>name` line per key, sorted by key
  [@test] crates/sync-core/src/issue_groups_test.rs (`reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction`)

### Requirement: Store: Each write is a new commit on the previous one

Each write of the map MUST be a new commit on the previous one.

#### Scenario: A write on the map
- **GIVEN** the map ref names a commit
- **WHEN** a write of the map lands
- **THEN** the ref names a new commit whose parent is the previous one
  [@test] crates/sync-core/src/issue_groups_test.rs (`reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction`)

### Requirement: Store: One git process reads the map and the stack parents

One `cat-file --batch` MUST read the map and the stack parents ([Stack Parents](../stack-parents/spec.md)): one git
process for both.

#### Scenario: Both maps are read
- **WHEN** the grouping reads the map and the stack parents
- **THEN** one `cat-file --batch` reads both: one git process
  [@test] crates/sync-core/src/stack_parents_test.rs (`both_maps_cost_one_git_process`)

### Requirement: Store: A bad line is a problem and never used

A line whose key is not an issue key (`[A-Z]+-\d+`), whose name is not a branch name without `/`, or whose key is
listed twice MUST be a problem: it is reported and never used.

#### Scenario: Lines the store can't use
- **GIVEN** a line whose key is not an issue key (`[A-Z]+-\d+`), one whose name is not a branch name without `/`, and a key listed twice
- **WHEN** the map is read
- **THEN** each of those lines is a problem, reported and never used
  [@test] crates/sync-core/src/issue_groups_test.rs (`bad_lines_are_problems_and_never_used`)

### Requirement: Store: A write keeps a bad line unless it sets or ungroups its key

A write MUST keep a line that is a problem as it is, so that a line of a newer format survives, unless the write sets
or ungroups its key.

#### Scenario: A write that names other keys
- **GIVEN** a line the store can't use
- **WHEN** a write sets or ungroups other keys
- **THEN** the line is kept as it is
  [@test] crates/sync-core/src/issue_groups_test.rs (`unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines`)

#### Scenario: A write that ungroups the key of the line
- **GIVEN** a key listed twice
- **WHEN** a write ungroups that key
- **THEN** its lines are gone
  [@test] crates/sync-core/src/issue_groups_test.rs (`unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines`)

### Requirement: Store: A write changes only the keys it names

The ref is shared by every branch and worktree. So a write MUST change only the keys that it names; a key that groups
no commit on this HEAD MUST stay, because it may group the commits of another branch. Only `ungroup_issues` removes a
key.

#### Scenario: A write on one worktree keeps the groups of another
- **GIVEN** keys that only the commits of another worktree carry
- **WHEN** a write on this worktree names other keys
- **THEN** those keys stay in the map, and group the commits there
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_write_on_one_worktree_keeps_the_groups_of_another`)

#### Scenario: A key is removed
- **WHEN** a key leaves the map
- **THEN** it is by `ungroup_issues`, never by a write that does not name it

### Requirement: Grouping: Prefix, then issue group, then issue key

The group of a commit MUST be its prefix (the leaf of a stack path), else the issue group of its issue key, else its
issue key, else none.

#### Scenario: The rule of a subject
- **GIVEN** a map that puts `ABC-1` into `parser`
- **WHEN** commits are grouped
- **THEN** a prefixed commit goes to its prefix (the leaf of a stack path), a commit of `ABC-1` without a prefix to `parser`, a commit of another key to its key, and a commit without a key to none
  [@test] crates/sync-core/src/issue_groups_test.rs (`the_rule_is_prefix_then_issue_group_then_issue_key`)
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`)

### Requirement: Grouping: A fixup follows its target into the group

A fixup with the key of its target MUST follow it into the group.

#### Scenario: A fixup of a commit in an issue group
- **GIVEN** a map that puts `ABC-1` into `parser`
- **WHEN** a `fixup! ABC-1 …` commit is grouped
- **THEN** it goes to `parser`, with its target
  [@test] crates/sync-core/src/issue_groups_test.rs (`the_rule_is_prefix_then_issue_group_then_issue_key`)

### Requirement: Grouping: Every reader groups with the same map

The sync, `brd status`, `brd commits`, `brd check`, `brd suggest`, the regroup, the fixup plan and the landed commits
MUST group with the same map.

#### Scenario: The sync groups by the map the regroup wrote
- **WHEN** a regroup wrote the map and the sync groups the commits
- **THEN** the commits of the keys go into the branch the map names
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)

### Requirement: Grouping: A branch an issue group fills has the origin `IssueGroup`

A branch that an issue group fills, without a prefix commit, MUST have the origin `IssueGroup`.

#### Scenario: A branch of grouped issues
- **GIVEN** a map that puts two keys into one branch, and no prefix commit of it
- **WHEN** the commits are grouped
- **THEN** that branch has the origin `IssueGroup`
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)

### Requirement: Grouping: The key of a commit is read as the grouper reads it

The key of a commit MUST be read as the grouper reads it, so an issue branch with a `fixup! KEY` commit stays `Issue`.

#### Scenario: An issue branch with a fixup
- **GIVEN** a commit of an issue key and a `fixup! KEY` commit of it
- **WHEN** the commits are grouped
- **THEN** their branch stays `Issue`
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_issue_branch_with_a_fixup_is_an_issue_branch`)

### Requirement: Grouping: An issue group can be stacked without a rewrite

Branch Deck SHALL allow an issue group to be the parent of a stacked branch, and to be stacked by the stack parents,
without a rewrite ([Stack Parents](../stack-parents/spec.md)).

#### Scenario: An issue group is restacked and is a parent
- **WHEN** an issue group is stacked on a branch, or a branch on it
- **THEN** no commit is rewritten and the issue group is a parent like any branch
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_issue_group_is_restacked_and_can_be_a_parent`)

#### Scenario: A plan stacks an issue group without a prefix
- **WHEN** a plan stacks an issue group whose commit is in the group already
- **THEN** only the stack parents move
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_stacks_an_issue_group_without_a_prefix`)

### Requirement: Writing the map: `brd group` reads the issue groups of the plan

`brd group` MUST read `issue_groups: {name: [keys]}` and `ungroup_issues: [keys]` in the plan.

#### Scenario: A plan of issue groups
- **WHEN** `brd group` gets a plan with `issue_groups: {name: [keys]}` or `ungroup_issues: [keys]`
- **THEN** it writes those keys to the map
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`)

### Requirement: Writing the map: A plan without commit moves moves only the map ref

A plan without commit moves MUST rewrite no commit, honour `--expect-head`, and move only the map ref, with a check
that HEAD did not move.

#### Scenario: A plan of issue groups only
- **WHEN** a plan without commit moves runs
- **THEN** no commit is rewritten and HEAD stays
- **AND** only the map ref moves, with a check that HEAD did not move
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)

#### Scenario: A stale `--expect-head`
- **WHEN** a plan without commit moves runs with an `--expect-head` that is not HEAD
- **THEN** it is refused and the map is not written
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)

### Requirement: Writing the map: A mixed plan moves HEAD and the map in one transaction

A plan with commit moves and map changes MUST move HEAD and the map ref in one `update-ref --stdin`, with
compare-and-swap on both, under the repository lock.

#### Scenario: A rewrite and a change of the map
- **WHEN** a plan with commit moves and map changes runs
- **THEN** HEAD and the map ref move in one `update-ref --stdin`, with compare-and-swap on both, under the repository lock
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

### Requirement: Writing the map: A plan is validated with the map after it

The plan MUST be validated with the map as it is after the plan. A key in two groups of the plan or in a group and in
`ungroup_issues`, a key to ungroup that is in no group and in no line of the store, an invalid key or name, a key that
no commit without a prefix carries afterwards, and a fixup that the map would separate from its target are
`invalid_plan`.

#### Scenario: A key in two groups
- **WHEN** a key is in two groups of the plan, or in a group and in `ungroup_issues`
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)

#### Scenario: A key to ungroup that is in no group
- **WHEN** a key to ungroup is in no group and in no line of the store
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)

#### Scenario: An invalid key or name
- **WHEN** the plan names an invalid key or name
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)

#### Scenario: A key no commit carries
- **WHEN** no commit without a prefix carries a key of the plan afterwards
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)

#### Scenario: A fixup the map would separate from its target
- **WHEN** the map would separate a fixup from its target, as `fixup! <id>`, which has no issue key
- **THEN** the plan is `invalid_plan`
  [@test] crates/sync-core/src/issue_groups_test.rs (`invalid_issue_group_plans_are_refused`)

### Requirement: Writing the map: `unassign` falls back to the issue group of the key

`unassign` of a prefixed commit MUST put it into the issue group of its key.

#### Scenario: Unassign of a prefixed commit
- **GIVEN** a map that puts the key of a prefixed commit into an issue group
- **WHEN** the commit is in `unassign`
- **THEN** it goes into the issue group of its key
  [@test] crates/sync-core/src/issue_groups_test.rs (`unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines`)

### Requirement: Writing the map: A sync never writes the map

A sync MUST NOT write the map.

#### Scenario: A sync with issue groups
- **GIVEN** a map
- **WHEN** a sync runs
- **THEN** it builds the branches of the map and the map ref stays as it was
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_sync_reads_the_map_and_never_writes_it`)

### Requirement: Rename and restack: A rename of an issue group renames its map entries only

`brd rename` of an issue group MUST rename its map entries only, with no rewrite.

#### Scenario: Rename of an issue group
- **WHEN** `brd rename` renames an issue group
- **THEN** its map entries take the new name, and no commit is rewritten
  [@test] crates/sync-core/src/issue_groups_test.rs (`rename_changes_the_entries_of_an_issue_group_and_of_a_prefix_branch`)

### Requirement: Rename and restack: A rename of a prefix branch renames its map entries too

A rename of a prefix branch MUST also rename the map entries of its name, in the same transaction.

#### Scenario: Rename of a prefix branch whose name the map holds
- **GIVEN** a prefix branch whose name the map holds
- **WHEN** it is renamed
- **THEN** its prefixes and the map entries of its name change in the same transaction
  [@test] crates/sync-core/src/issue_groups_test.rs (`rename_changes_the_entries_of_an_issue_group_and_of_a_prefix_branch`)

### Requirement: Rename and restack: A restack of an issue group writes the stack parents only

`brd restack` and `brd unstack` of a branch that the map fills MUST write the stack parents, with no rewrite.

#### Scenario: Restack of an issue group
- **WHEN** `brd restack` stacks a branch that the map fills
- **THEN** the stack parents are written, and no commit is rewritten
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_issue_group_is_restacked_and_can_be_a_parent`)
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

#### Scenario: Unstack of an issue group
- **WHEN** `brd unstack` unstacks a branch that the map fills
- **THEN** the stack parents are written, and no commit is rewritten
  [@test] crates/sync-core/src/stack_parents_test.rs (`an_issue_group_is_restacked_and_unstacked_without_a_rewrite`)

### Requirement: Undo: The undo token names the refs that moved

The undo token MUST be `H1..H2` for a rewrite only, `groups:G1..G2` for a change of the map only, and
`H1..H2+groups:G1..G2` for both, followed by `+stacks:S1..S2` when the stack parents moved too
([Stack Parents](../stack-parents/spec.md#writing-the-map)); a `G1` of zeros means that the map did not exist.

#### Scenario: The forms of the token
- **WHEN** a rewrite only, a change of the map only, or both ran, with or without a move of the stack parents
- **THEN** the token is `H1..H2`, `groups:G1..G2` or `H1..H2+groups:G1..G2`, followed by `+stacks:S1..S2` when the stack parents moved too
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)

#### Scenario: A map that did not exist
- **WHEN** the change of the map wrote the first map
- **THEN** its `G1` is zeros
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)

### Requirement: Undo: The undo token names full ids

The ids of the undo token MUST be full (40 or 64 hex digits), and anything else is `invalid_arguments`.

#### Scenario: A token of short ids or names
- **WHEN** a token holds a short id, a ref name or anything but full ids in its forms
- **THEN** it is `invalid_arguments`
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_token_is_read_strictly`)

### Requirement: Undo: `brd undo` moves both refs back in one transaction

`brd undo` MUST move both refs back in one transaction.

#### Scenario: Undo of a change of the map only
- **WHEN** `brd undo` runs with the token of a change of the map
- **THEN** the map is as it was and HEAD stays
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`)

#### Scenario: Undo of a mixed plan
- **WHEN** `brd undo` runs with the token of a rewrite and a change of the map
- **THEN** HEAD and the map move back in one transaction
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`)

### Requirement: Undo: The map commit of the token is a write on the previous one

`G2` MUST be a write of the map on `G1`: its parent, or no parent when `G1` is zeros; else the token is
`invalid_arguments`.

#### Scenario: A map commit that is no write on the previous one
- **WHEN** the token's `G2` does not have `G1` as its parent, or has a parent while `G1` is zeros
- **THEN** the token is `invalid_arguments` and nothing moves
  [@test] crates/sync-core/src/issue_groups_test.rs (`an_undo_of_the_map_needs_the_written_commit_on_its_previous_one`)
  [@test] crates/bd-rewrite/src/rewrite_test.rs (`issue_groups_plan_and_undo`)

### Requirement: Undo: The undo is refused when HEAD or the map moved

`brd undo` MUST refuse with `head_moved` when HEAD moved, and with `groups_moved` (exit 3, retryable) when the map
moved, also when a ref moves between its checks and its transaction.

#### Scenario: HEAD moved
- **WHEN** HEAD moved since the token
- **THEN** the undo refuses with `head_moved` and changes nothing
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

#### Scenario: The map moved
- **WHEN** the map moved since the token
- **THEN** the undo refuses with `groups_moved` (exit 3, retryable) and changes nothing
  [@test] crates/sync-core/src/issue_groups_test.rs (`a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store`, `a_mixed_plan_moves_head_and_the_store_in_one_transaction`)

#### Scenario: A ref moves between the checks and the transaction
- **WHEN** HEAD or the map moves between the checks of the undo and its transaction
- **THEN** the undo refuses with `head_moved` or `groups_moved` and changes nothing

### Requirement: `brd status` and `brd suggest`: `brd status` prints the issue groups

`brd status` MUST print the `origin` of each branch, the `issue_keys` of an issue group, `idle_issue_groups` (the keys
that group no commit on this HEAD; they may group the commits of another branch) and `issue_group_problems`.

#### Scenario: Status of a repository with issue groups
- **GIVEN** a map with an issue group, a key that groups no commit on this HEAD, and a line the store can't use
- **WHEN** `brd status` runs
- **THEN** it prints the `origin` of each branch, the `issue_keys` of the issue group, the idle key in `idle_issue_groups` and the line in `issue_group_problems`
  [@test] crates/bd-remote/src/status_test.rs (`issue_groups_in_status`)

### Requirement: `brd status` and `brd suggest`: `brd suggest` groups whole issue branches

`brd suggest` MUST put a cluster of whole issue branches into `issue_groups` of its plan, so that it rewrites no
commit.

#### Scenario: A cluster of whole issue branches
- **WHEN** `brd suggest` finds a cluster of whole issue branches
- **THEN** its plan puts them into `issue_groups`, and the plan rewrites no commit
  [@test] crates/bd-rewrite/src/suggest_test.rs (`whole_issue_branches_become_an_issue_group`)

### Requirement: `brd status` and `brd suggest`: `--rewrite-issues` gives the commits a prefix

With `--rewrite-issues`, `brd suggest` SHALL give the commits of such a cluster a prefix instead.

#### Scenario: `brd suggest --rewrite-issues`
- **WHEN** `brd suggest --rewrite-issues` finds a cluster of whole issue branches
- **THEN** its plan gives the commits a prefix instead of an issue group

### Requirement: `brd status` and `brd suggest`: An issue group is an anchor

For `brd suggest`, an issue group MUST be an anchor, as a prefix is.

#### Scenario: A suggestion after the issue group was written
- **GIVEN** an issue group of the cluster
- **WHEN** `brd suggest` runs again
- **THEN** it keeps the cluster as it keeps a prefixed branch, and suggests nothing more
  [@test] crates/bd-rewrite/src/suggest_test.rs (`whole_issue_branches_become_an_issue_group`)

### Requirement: App: An issue group shows as a branch

The app MUST show an issue group as a branch, and nest a single-key issue group under its issue.

#### Scenario: An issue group of several keys
- **WHEN** the app shows an issue group
- **THEN** it is a branch row of its commits
  [@test] tests/e2e/issue-groups/issue-groups.spec.ts (`should rename an issue group without a rewrite and undo the rename`)

#### Scenario: A single-key issue group
- **WHEN** an issue group holds a single key
- **THEN** the app nests it under its issue

### Requirement: App: An issue group is renamed through the rename core

The app MUST rename an issue group through the rename core; naming an issue branch MUST write the map, not a prefix.

#### Scenario: Rename of an issue group
- **WHEN** the user renames an issue group
- **THEN** the rename core renames it, with no rewrite
  [@test] tests/e2e/issue-groups/issue-groups.spec.ts (`should rename an issue group without a rewrite and undo the rename`)

#### Scenario: Naming an issue branch
- **WHEN** the user names an issue branch
- **THEN** the map is written, not a prefix

### Requirement: App: A move of a whole issue writes the map

A move of a whole issue MUST write the map, a move back of one MUST remove its key (`plan_move`, see
[Move Commits](../move-commits/spec.md)); "Ungroup" of an issue group moves all its issues back.

#### Scenario: A whole issue joins a branch
- **WHEN** a whole issue moves to a branch
- **THEN** the map is written
  [@test] crates/sync-core/src/move_commits_test.rs (`whole_issues_join_a_branch_without_a_rewrite_and_their_next_commits_follow`)

#### Scenario: An issue moves back, and "Ungroup"
- **WHEN** an issue of a group moves back, or "Ungroup" of an issue group runs
- **THEN** the key of that issue is removed, and "Ungroup" moves all the issues of the group back
  [@test] crates/sync-core/src/move_commits_test.rs (`moving_an_issue_group_reassigns_its_keys_and_moving_it_back_ungroups_them`)

### Requirement: App: The undo of a move, a rename and a restack is `brd undo`

The undo of a move, a rename and a restack MUST be `brd undo` of the token of the result (the rewrite `undo` of the
app): HEAD and the map in one transaction.

#### Scenario: Undo of a rename of an issue group
- **WHEN** the user undoes the rename of an issue group
- **THEN** `brd undo` of the token of the result moves HEAD and the map back in one transaction, and the old name is back
  [@test] tests/e2e/issue-groups/issue-groups.spec.ts (`should rename an issue group without a rewrite and undo the rename`)

### Requirement: App: The commit editor names the issue group of a key

The commit editor and "Move Back to …" MUST name the issue group of a key.

#### Scenario: Commits that fall back to an issue group
- **WHEN** the commit editor or "Move Back to …" lists where commits fall back to
- **THEN** it names the issue group of their key
  [@test] src/hooks/useCommitMoveList.test.ts (`names the issue group the commits fall back to`)

## Testing / Local Run

- `pnpm test:backend`: `crates/sync-core/src/issue_groups_test.rs`, `crates/bd-remote/src/status_test.rs`,
  `crates/bd-rewrite/src/suggest_test.rs`.
- `pnpm test:unit`: `src/hooks/useCommitMoveList.test.ts`.
- `pnpm e2e`: `tests/e2e/issue-groups/issue-groups.spec.ts`, on the `issue_groups` template;
  `tests/e2e/move-commits/combine-branches.spec.ts`, on the `issue_combine` template.

## Open Questions / Risks

- git keeps no reflog for `refs/branch-deck/issue-groups`. The parent chain of its commits is the history of the map.
- A move of a whole issue writes a key that the other worktrees share: its commits there move too.

## References

- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- [Git Operations](../../../../docs/design/git-operations.md)
- [Agent CLI](../../cli/agent-cli/spec.md)
