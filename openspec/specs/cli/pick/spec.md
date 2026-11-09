---
name: brd pick
description: Requirements for brd pick, which lands the commits of another worktree on the branch checked out here without git cherry-pick, moves that worktree along with --follow, and takes both back with brd undo.
style: plain-1
targets:
  - crates/bd-update/src/pick_command.rs
  - crates/lease-pool/src/check_record.rs
  - crates/commit-rules/src/lib.rs
  - crates/git-ops/src/commit_list.rs
  - crates/git-ops/src/copy_commit.rs
  - crates/bd-cli/src/agent_help.rs
  - crates/domain-types/src/error.rs
  - crates/bd-rewrite/src/rewrite.rs
  - crates/bd-update/src/undo_command.rs
  - crates/bd-cli/src/schema.rs
  - crates/update-core/src/pick.rs
  - crates/history-rewrite/src/stack.rs
  - crates/trial-check/src/dependencies.rs
  - crates/git-ops/src/pick.rs
  - crates/git-ops/src/update_branch.rs
  - crates/git-ops/src/ref_transaction.rs
  - crates/git-ops/src/replay.rs
  - crates/sync-core/src/remote_status.rs
  - crates/operations/src/rewrite.rs
  - src/lib/git/pick-remote.ts
---

# brd pick

Status: Implemented
Date: 2026-10-18

## Purpose

Several agent sessions share one checkout. Each one commits in a worktree of its own (a `brd lease` worktree, a detached
HEAD that shares the git dir), and its work counts once it is on the branch of the shared checkout, which one owner
rewrites with `brd update`, `brd fold` and `brd group`. `brd pick --from <worktree>` lands the commits of that worktree on
the branch checked out at `-C`. It replays them in memory with `git replay` and leaves out a commit whose change the
branch has already. It checks before the repository lock, which every worktree shares, and under it only reads again
what the check read; then it moves the branch, and the files follow through the guarded working-tree update. `--follow` moves the source worktree onto the new tip the same way. A conflict
or local changes in the way change nothing; `brd undo` takes the pick back, the worktree too. A lease of the pool lands
its commits with `brd lease pick`, this pick run in the checkout it was cut from, onto the branch it had checked out
([brd lease](../lease/spec.md)). How it works: "Pick" in
[Git Operations](../../../../docs/design/git-operations.md). Working beside other sessions: `brd help worktrees`
([Agent CLI](../agent-cli/spec.md)).

## Goals

- No `git cherry-pick` in a shared checkout: no `CHERRY_PICK_HEAD` left behind by an empty pick, and no state that a
  fold or an update of the owner meets halfway.
- A commit that the branch already has, by its patch or by its content, is reported, never picked again as an empty
  commit.
- The worktree's range sees through the owner's rewrites (a fold, a regroup, new dates): the old commits of the branch
  never come back.
- The agent sees at once which branch each picked commit lands in, and a commit written without a prefix.

## Non-goals

- A resolve session: a conflict is resolved in the source worktree, on top of the branch, where the commits were made.
- A `(cherry picked from commit …)` trailer: the output maps every commit to its copy, and the undo token names both
  heads.
- Merges in the range: refused, as for an update.
- A command of the app for the pick of a worktree: the app sees the moved branch through its watcher and syncs. Only
  the pick of the remote commits of a branch is the app's too (Pick Remote Commits, [Outgoing
  Commits](../../repository/outgoing/spec.md#remote-commits-of-others)).
- A check of the remote commits of `--remote`: they are someone else's, pushed and seen by the user, and no worktree
  of this machine built them.

## Concepts

### The check of the tree

The check of the tree gates the pick and not only the lease because an agent that ran
`brd pick --from <lease> --follow`, which the help named as what `brd lease pick` runs, landed a tree that no build saw.

### The commit rules

An agent landed a subject that Patronus rejects later, on CI; the pick is where every agent lands, so the pick checks it
first.

### Remote commits

Someone else pushed to the remote branch of a virtual branch (a fix of a reviewer, a commit of a colleague): a push
would drop it. `brd pick --remote <branch>` takes it into the checked out branch instead.

## Requirements

### Requirement: Pick: A pick from a worktree lands the commits the branch lacks

`brd pick --from <dir>` MUST land the commits that the HEAD of the worktree of `dir` has and the checked out branch
lacks. The copies MUST keep their author; the committer is the current user. The branch's reflog MUST name the move
`branch-deck: pick`.

#### Scenario: The commits of a worktree land on the checked out branch
- **GIVEN** the HEAD of the worktree of `dir` has commits that the checked out branch lacks
- **WHEN** `brd pick --from <dir>` runs
- **THEN** copies of those commits land on the checked out branch, each keeping its author, the committer the current
  user
- **AND** the branch's reflog names the move `branch-deck: pick`
  [@test] crates/git-ops/src/pick_test.rs (`a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows`)
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)

### Requirement: Pick: The output names each picked commit, its group and the undo token

The pick MUST print `state: "picked"`, the previous and the new HEAD, the `base` of the range and the undo token, and
`picked` with each commit, its copy (`new_id`), its subject, the `group` it lands in and what decides it
(`grouped_by`).

#### Scenario: The output of a pick
- **WHEN** a pick lands its commits
- **THEN** it prints `state: "picked"`, the previous and the new HEAD, the `base` of the range and the undo token
- **AND** `picked` lists each commit with its copy (`new_id`), its subject, the `group` it lands in and `grouped_by`:
  `prefix`, `issue_group`, `issue`, or `none` for a subject without a prefix or an issue key
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)

### Requirement: Pick: A range that starts at the branch's HEAD lands as it is

A range that starts at the branch's HEAD (a worktree cut at the tip, the branch not moved since) MUST land as it is, a
fast-forward: every `new_id` is its commit, and the worktree of `--follow` has nowhere to move.

#### Scenario: A worktree cut at the tip
- **GIVEN** a worktree cut at the tip of the branch, the branch not moved since
- **WHEN** the pick runs
- **THEN** the commits land as they are, a fast-forward: every `new_id` is its commit
- **AND** the worktree of `--follow` has nowhere to move
  [@test] crates/git-ops/src/pick_test.rs (`a_worktree_cut_at_the_tip_lands_its_commits_as_they_are`)

### Requirement: Pick: The copies get the newest committer date of the range and the HEAD

The committer date of the copies MUST be the newest committer date of the range and of the branch's HEAD, not the time
of the pick: the same pick writes the same commits, a copy is never older than its parent, and the dates of the
worktree's commits carry over. A `GIT_COMMITTER_DATE` the user sets MUST win.

#### Scenario: The same pick writes the same commits
- **WHEN** a pick and its `--dry-run` copy the same range
- **THEN** each copy gets the newest committer date of the range and of the branch's HEAD, never older than its parent
- **AND** the `new_head` of `--dry-run` is the one the pick writes, the dates of the worktree's commits carrying over
  [@test] crates/git-ops/src/pick_test.rs (`the_copies_get_the_newest_committer_date_so_the_check_names_the_commits_the_pick_writes`)

#### Scenario: The user sets the committer date
- **GIVEN** the user sets a `GIT_COMMITTER_DATE`
- **WHEN** the pick copies the commits
- **THEN** the date the user set wins

### Requirement: Pick: A commit the branch has already is listed in already_present

A commit whose change the branch has already MUST be listed in `already_present`, with `on`, the branch's commit of the
same patch id, or `null` when the branch has the change in another form, and `by`: `patch_id` or `empty`; it MUST
never become an empty commit.

#### Scenario: The branch has a commit of the same patch id
- **GIVEN** the branch has the change of a commit of the range already
- **WHEN** the pick runs
- **THEN** the commit is listed in `already_present` with `on`, the branch's commit of the same patch id, or `null` when
  the branch has the change in another form, and `by`: `patch_id` or `empty`
- **AND** it never becomes an empty commit
  [@test] crates/git-ops/src/pick_test.rs (`a_commit_the_target_has_already_is_present_with_the_target_commit_of_its_patch`)

### Requirement: Pick: A pick whose every commit is present moves nothing

When every commit is present, the state MUST be `up_to_date` and nothing MUST move.

#### Scenario: A pick again
- **GIVEN** the branch has every commit of the range already
- **WHEN** the pick runs
- **THEN** the state is `up_to_date` and nothing moves
  [@test] crates/bd-update/src/pick_test.rs (`a_dry_run_writes_no_ref_and_a_pick_again_finds_every_commit_present`)

### Requirement: Pick: A commit reworded on the shared branch is present by its patch id

A commit that the shared branch has with another message (a `brd reword` after the pick) MUST be present by its patch
id.

#### Scenario: A pick without a follow after a reword
- **GIVEN** the shared branch has a picked commit with another message, after a `brd reword`
- **WHEN** the worktree's commits are picked again
- **THEN** the commit is present by its patch id
  [@test] crates/bd-lease/src/lease_test.rs (`a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`)

### Requirement: Pick: After a follow the old tip is the fork point

After a follow the old tip MUST be the fork point, so a later pick lands only the commits after it.

#### Scenario: A later pick after a follow and a reword
- **GIVEN** a pick with `--follow`, and the landed commit reworded since
- **WHEN** a later pick of the worktree runs
- **THEN** it lands only the commits after the old tip
  [@test] crates/bd-lease/src/lease_test.rs (`a_session_rewords_its_landed_commit_from_its_lease`)

### Requirement: Pick: A commit with the patch id of a branch commit is left out before the replay

A commit of the range with the patch id of a branch commit above the base MUST be left out before the replay
(`by: "patch_id"`; the range's `log --cherry-mark --right-only <branch>...<source HEAD> ^<base> ^<upstream>`), so a
commit the branch has in a form that no longer applies (the branch changed the same lines since) never conflicts.

#### Scenario: The branch has the commit in a form that no longer applies
- **GIVEN** the branch has a commit of the range by its patch id, and changed the same lines since
- **WHEN** the pick runs
- **THEN** the commit is left out before the replay, `by: "patch_id"`, and never conflicts
  [@test] crates/git-ops/src/pick_test.rs (`a_commit_with_the_patch_id_of_a_target_commit_is_left_out_and_its_child_lands_on_the_copy_before_it`)

### Requirement: Pick: The rest is replayed in segments as one chain

The rest MUST be replayed in segments between the left out commits, each onto the tip of the one before, so the child
of a left out commit lands on the copy before it: one chain, never a fork onto the branch's HEAD (as
`git replay --cherry-pick` would replay it). Without a left out commit the pick MUST run one `replay`, one more per
segment otherwise.

#### Scenario: The child of a left out commit
- **GIVEN** a commit of the range left out by its patch id
- **WHEN** the rest is replayed
- **THEN** its child lands on the copy before it, one chain, never a fork onto the branch's HEAD
  [@test] crates/git-ops/src/pick_test.rs (`a_commit_with_the_patch_id_of_a_target_commit_is_left_out_and_its_child_lands_on_the_copy_before_it`)

#### Scenario: The replays of a pick
- **WHEN** no commit of the range is left out
- **THEN** the pick runs one `replay`
- **AND** with left out commits, one more per segment
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick`)

### Requirement: Pick: A commit the replay drops as empty is present by empty

A commit the replay drops as empty MUST be listed as `by: "empty"`.

#### Scenario: The replay drops a commit as empty
- **WHEN** the replay drops a commit of the range as empty
- **THEN** it is `by: "empty"`

### Requirement: Pick: A conflict after a left out commit is searched in its segment

A conflict after a left out commit MUST be searched in its segment only, and MUST list the left out commits in
`already_present` and not in `clean`.

#### Scenario: A conflict after a commit left out by its patch id
- **GIVEN** a commit left out by its patch id, and a later commit that conflicts
- **WHEN** the pick runs
- **THEN** the conflict is searched in the segment after the left out commit only and names the conflicting commit
- **AND** the left out commit is listed in `already_present` and not in `clean`
  [@test] crates/git-ops/src/pick_test.rs (`a_conflict_after_a_commit_left_out_by_its_patch_id_names_the_commit_and_lists_the_left_out_one`)

### Requirement: Pick: The next step of a conflict is never git cherry-pick

`conflict.next` MUST say never to `git cherry-pick` in the shared checkout (it leaves `CHERRY_PICK_HEAD`).

#### Scenario: What to do after a conflict
- **WHEN** a pick conflicts
- **THEN** `conflict.next` says never to `git cherry-pick` in the shared checkout
  [@test] crates/bd-update/src/pick_test.rs (`a_conflict_changes_nothing_and_says_what_to_do`)

### Requirement: Pick: A pick that names the home branch refuses another one

A pick that names the branch the target must have checked out (`brd lease pick`) MUST refuse another one as
`home_branch_switched` (exit 4), checked with the branch of the witness under the lock; nothing moves.

#### Scenario: The home switched its branch
- **GIVEN** a pick that names the branch the target must have checked out, as `brd lease pick` does
- **WHEN** the target has another branch checked out, by the branch of the witness under the lock
- **THEN** the pick is refused as `home_branch_switched` (exit 4) and nothing moves
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)

### Requirement: Pick: A picked commit that needs another branch is listed in dependencies

A picked commit whose prefix names a branch the branch has already MUST be listed in `dependencies` when that branch
is not built on every branch whose commits its hunks depend on (the edges by lines, no symbols), with the copy,
`prefix_branch` and `depends_on`, in a `--dry-run` too.

#### Scenario: The hunks of a picked commit depend on another branch
- **GIVEN** a picked commit whose prefix names a branch the branch has, and whose hunks depend on the commits of a
  branch that branch is not built on
- **WHEN** the pick or its `--dry-run` runs
- **THEN** `dependencies` lists the commit with the copy, `prefix_branch` and `depends_on`
  [@test] crates/bd-update/src/pick_test.rs (`a_picked_commit_that_needs_another_branch_names_it_with_the_plan_that_moves_it`)

### Requirement: Pick: A dependency names the plan or the restack, never applied

`plan`, the `brd group` plan that moves the commit into the branch of the stack that holds all of them, MUST make the
next `brd sync` clean; a commit that also needs its own branch gets `restack`, `brd restack <prefix_branch> <parent>`
onto a branch that holds them and its current parent. The pick MUST NOT apply either.

#### Scenario: The plan moves the commit into the branch that holds them
- **WHEN** a picked commit is listed in `dependencies`
- **THEN** its `plan` is the `brd group` plan that moves it into the branch of the stack that holds all of them, which
  makes the next `brd sync` clean
- **AND** the pick does not apply it
  [@test] crates/bd-update/src/pick_test.rs (`a_picked_commit_that_needs_another_branch_names_it_with_the_plan_that_moves_it`)

#### Scenario: A commit that also needs its own branch
- **WHEN** a listed commit also needs its own branch
- **THEN** it gets `restack`, `brd restack <prefix_branch> <parent>` onto a branch that holds them and its current
  parent
- **AND** the pick does not apply it
  [@test] crates/bd-update/src/pick_test.rs (`a_picked_commit_that_needs_its_own_and_another_branch_offers_the_restack`)

### Requirement: Pick: Only a prefix of a branch the branch has is checked for dependencies

A prefix of a new branch MUST NOT be checked: the check reads the diffs only for a prefix that names a branch the branch
has.

#### Scenario: A prefix of a new branch
- **WHEN** a picked commit's prefix names a branch the branch does not have
- **THEN** the dependency check reads no diffs for it
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick`)

#### Scenario: A prefix of a branch the branch has
- **WHEN** a picked commit's prefix names a branch the branch has
- **THEN** the dependency check reads the diffs of the commits ahead and of the copy
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick_into_a_branch_the_target_has`)

### Requirement: Pick: A parent/leaf prefix on a root stacks the whole branch

A `(parent/leaf)` prefix of a picked commit on a branch the branch has as a root (or on another parent) MUST stack the
whole branch on `parent`, and the pick MUST say so in `reparented` (`{"leaf": "parent"}`), in a `--dry-run` too; the
next `brd sync` MUST build the stack.

#### Scenario: A parent/leaf prefix on a root group
- **GIVEN** the branch has `leaf` as a root, or on another parent
- **WHEN** a picked commit with the `(parent/leaf)` prefix is picked, or its `--dry-run` runs
- **THEN** the whole branch is stacked on `parent` and `reparented` says `{"leaf": "parent"}`
- **AND** the next `brd sync` builds the stack
  [@test] crates/bd-update/src/pick_test.rs (`a_parent_leaf_prefix_on_a_root_group_stacks_the_group_and_says_so`)

### Requirement: Pick: A parent that is the baseline or the checked out branch lands nothing

A prefix of a picked commit whose parent, a level above its leaf, is the branch of the baseline (`master` of
`origin/master`) or the checked out branch MUST land nothing, a `--dry-run` too: `baseline_parent` (exit 4), its
`commits` the ids. Neither is a virtual branch, and a branch without a parent is built on the baseline already. No flag
lands it as it is. `--remote` MUST NOT be checked: its commits are others'.

#### Scenario: The parent is the baseline
- **WHEN** a picked commit's prefix names `master` of `origin/master` as the parent of its leaf
- **THEN** nothing lands, a `--dry-run` too: `baseline_parent` (exit 4), its `commits` the ids
  [@test] crates/bd-update/src/pick_test.rs (`a_parent_that_is_the_baseline_is_refused_and_nothing_lands`)

#### Scenario: The parent is the checked out branch
- **WHEN** a picked commit's prefix names the checked out branch as the parent of its leaf
- **THEN** nothing lands: `baseline_parent` (exit 4)
  [@test] crates/bd-update/src/pick_test.rs (`a_parent_that_is_the_checked_out_branch_is_refused_and_a_deeper_path_keeps_its_levels_below`)

#### Scenario: A remote pick
- **WHEN** `brd pick --remote` picks a commit with such a prefix
- **THEN** the prefix is not checked

### Requirement: Pick: The next step of a baseline parent is the reword in the worktree

For `--from` and `brd lease pick` each commit's `next_steps` entry MUST be
`brd -C <worktree> reword <id> --subject '(<levels below it>) <rest>'`, without approval: the agent's own commit in its
own worktree.

#### Scenario: A deeper path keeps its levels below
- **WHEN** a pick of `--from` or `brd lease pick` is refused as `baseline_parent`
- **THEN** each commit's `next_steps` entry is `brd -C <worktree> reword <id> --subject '(<levels below it>) <rest>'`,
  without approval
- **AND** a deeper path keeps the levels below the parent in the subject
  [@test] crates/bd-update/src/pick_test.rs (`a_parent_that_is_the_baseline_is_refused_and_nothing_lands`, `a_parent_that_is_the_checked_out_branch_is_refused_and_a_deeper_path_keeps_its_levels_below`)

### Requirement: Pick: The landing of a pick is read after the lock

`groups`, `dependencies` and `reparented` decide nothing: they MUST be read after the moves and the release of the
repository lock, from the head the check froze, so the `log` of the commits ahead and the diffs of the dependency check
never make another writer wait. A failure of that read after a move MUST keep the pick and its undo token.

#### Scenario: The read of the landing after the moves
- **WHEN** a pick moves the branch
- **THEN** `groups`, `dependencies` and `reparented` are read after the moves and the release of the repository lock,
  from the head the check froze
  [@test] crates/bd-update/src/pick_test.rs (`the_landing_of_a_pick_is_read_after_the_lock`)

#### Scenario: The read fails after a move
- **WHEN** that read fails after a move
- **THEN** the pick and its undo token are kept
  [@test] crates/bd-update/src/pick_test.rs (`the_landing_of_a_pick_is_read_after_the_lock`)

### Requirement: Pick: The range of a worktree stops at its fork point in the reflog

The range of a worktree MUST be `<source HEAD> ^<branch> ^<upstream> ^<base>`, the base being the fork point of the
source in the reflog of the branch, so that a branch folded, regrouped or redated after the worktree was cut gives none
of its old commits back.

#### Scenario: A branch rewritten after the worktree was cut
- **GIVEN** a branch folded, regrouped or redated after the worktree was cut from it
- **WHEN** the worktree's commits are picked
- **THEN** none of the branch's old commits come back
  [@test] crates/git-ops/src/pick_test.rs (`a_target_rewritten_after_the_worktree_was_cut_from_it_gives_back_none_of_its_old_commits`)

### Requirement: Pick: A value a rolled back or undone move of brd put is no fork point

A value that a move of brd (a pick, an update, an unapply, a reapply) put into the reflog of the branch and that its
rollback or its undo took back MUST be no fork point. The base MUST be the value before that move, when the move only
added commits to it.

#### Scenario: A fast-forward pick that rolled back
- **GIVEN** a fast-forward pick that rolled back, which leaves the source's own tip in the reflog of the branch
- **WHEN** the next pick runs
- **THEN** that tip is no fork point, which would make its range empty and a `--follow` drop its commits
- **AND** the base is the value before that move, and the commit lands
  [@test] crates/git-ops/src/pick_test.rs (`a_fast_forward_pick_that_rolled_back_is_no_fork_point_and_the_next_pick_lands_the_commit`, `a_worktree_that_went_on_from_a_rolled_back_tip_lands_the_commits_below_it_too`)
  [@test] crates/bd-lease/src/lease_test.rs (`a_return_after_a_pick_that_rolled_back_refuses_the_commit`)

#### Scenario: An undone fast-forward pick
- **GIVEN** a fast-forward pick that its undo took back
- **WHEN** the next pick runs
- **THEN** its value is no fork point
  [@test] crates/git-ops/src/pick_test.rs (`an_undone_fast_forward_pick_is_no_fork_point`)

### Requirement: Pick: A value that another entry put into the reflog stays a fork point

A value that any other entry put into the reflog of the branch (the user, a rewrite, a rollback or an undo itself, the
current tip) MUST stay a fork point, and so MUST one whose next entry `log -g` skipped.

#### Scenario: Only a value whose every move was taken back gives way
- **GIVEN** values of the reflog that the user, a rewrite, a rollback or an undo itself put there, the current tip, and
  one whose next entry `log -g` skipped
- **WHEN** the pick looks for the fork point
- **THEN** each of them stays a fork point; only a value whose every move was rolled back or undone gives way to the
  value before
  [@test] crates/git-ops/src/pick_test.rs (`only_a_value_whose_every_move_was_rolled_back_or_undone_gives_way_to_the_value_before`)

### Requirement: Pick: Without a fork point a local shared history is refused

Without a fork point, a shared history that is local (not the upstream's) MUST be refused as `range_unknown`
(exit 4).

#### Scenario: A local shared history without a fork point
- **GIVEN** a worktree without a fork point in the reflog of the branch, whose shared history with it is local
- **WHEN** `brd pick --from` the worktree runs
- **THEN** it is refused as `range_unknown` (exit 4)
  [@test] crates/git-ops/src/pick_test.rs (`without_a_fork_point_a_local_shared_history_is_refused_and_a_range_names_the_commits`)

### Requirement: Pick: A revision or a range picks the commits it names

`brd pick <rev>` or `<a>..<b>` MUST pick the commits it names.

#### Scenario: A range names the commits
- **WHEN** `brd pick <rev>` or `<a>..<b>` runs
- **THEN** it picks the commits it names
  [@test] crates/git-ops/src/pick_test.rs (`without_a_fork_point_a_local_shared_history_is_refused_and_a_range_names_the_commits`)

### Requirement: Pick: A worktree on a newer baseline is refused

A worktree that stands on a newer baseline than the branch MUST be refused as `source_on_newer_baseline` (exit 4).

#### Scenario: The worktree stands on a newer baseline
- **GIVEN** a worktree that stands on a newer baseline than the branch
- **WHEN** its commits are picked
- **THEN** the pick is refused as `source_on_newer_baseline` (exit 4)
  [@test] crates/git-ops/src/pick_test.rs (`a_worktree_on_a_newer_baseline_is_refused`)
  [@test] crates/bd-update/src/pick_test.rs (`a_worktree_on_a_newer_baseline_is_refused`)

### Requirement: Pick: A dry run takes no lock and writes at most git objects

`--dry-run` MUST take no lock and write at most git objects; it is the check whether the branch has the worktree's
commits by content, not by date.

#### Scenario: A dry run and a pick again
- **WHEN** `--dry-run` runs
- **THEN** it writes no ref, and says whether the branch has the worktree's commits by content, not by date
  [@test] crates/bd-update/src/pick_test.rs (`a_dry_run_writes_no_ref_and_a_pick_again_finds_every_commit_present`)

#### Scenario: Read-only commands
- **WHEN** a read-only command of brd runs, `brd pick --dry-run` among them
- **THEN** it writes no ref, config, index or working tree
  [@test] crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)

### Requirement: Pick: A commit that does not apply is a conflict and moves nothing

A commit that does not apply MUST print `state: "conflict"` (exit 1) with the first conflicting commit (a binary search
over the prefixes of the range), the commits before it (`clean`), its files and `next`, what to do; nothing MUST move.

#### Scenario: The first conflicting commit
- **WHEN** a commit of the range does not apply
- **THEN** the pick prints `state: "conflict"` (exit 1) with the first conflicting commit, found by a binary search over
  the prefixes of the range, the commits before it (`clean`), its files and `next`, what to do
- **AND** nothing moves
  [@test] crates/git-ops/src/pick_test.rs (`a_conflict_names_the_first_conflicting_commit_and_moves_nothing`)
  [@test] crates/bd-update/src/pick_test.rs (`a_conflict_changes_nothing_and_says_what_to_do`)

### Requirement: Pick: Local changes on a path the pick writes refuse it

Local changes on a path that the pick or the follow writes MUST refuse it as `uncommitted_overlap` (exit 4) with the
`paths`, the message naming the worktree; local changes on other paths MUST stay.

#### Scenario: Local changes in the way in either worktree
- **GIVEN** local changes on a path that the pick or the follow writes, in the target or the source
- **WHEN** the pick runs
- **THEN** it is refused as `uncommitted_overlap` (exit 4) with the `paths`, the message naming the worktree
  [@test] crates/git-ops/src/pick_test.rs (`local_changes_on_a_path_the_pick_writes_block_it_in_either_worktree`)
  [@test] crates/bd-update/src/pick_test.rs (`local_changes_in_the_way_and_a_wrong_source_are_refused`)

#### Scenario: Local changes on other paths
- **GIVEN** local changes on paths that neither the pick nor the follow writes
- **WHEN** the pick lands
- **THEN** those changes stay

### Requirement: Pick: A branch that moved since the check is head_moved

The move of a check whose branch moved since MUST be `head_moved`, nothing moved; a HEAD other than `--expect-head`
MUST be `head_moved` (exit 3).

#### Scenario: The target moved since the check
- **WHEN** the branch of a check moved since, before its move
- **THEN** the move is `head_moved`, nothing moved
  [@test] crates/git-ops/src/pick_test.rs (`a_target_that_moved_since_the_check_is_head_moved_with_nothing_changed`)

#### Scenario: Another HEAD than the expected one
- **WHEN** the HEAD is other than `--expect-head`
- **THEN** the pick is `head_moved` (exit 3)
  [@test] crates/bd-update/src/pick_test.rs (`expect_head_refuses_a_pick_onto_another_head`)

### Requirement: Pick: The source is another worktree of the same repository

The source MUST be another worktree of the same repository; the target worktree itself, a directory outside the
repository, a symmetric range, no source and `--follow` without `--from` MUST be `invalid_arguments` (exit 2).

#### Scenario: A wrong source
- **WHEN** the source is the target worktree itself, a directory outside the repository, a symmetric range, or no
  source, or `--follow` comes without `--from`
- **THEN** the pick is `invalid_arguments` (exit 2)
  [@test] crates/git-ops/src/pick_test.rs (`the_source_is_another_worktree_of_this_repository`)
  [@test] crates/bd-update/src/pick_test.rs (`local_changes_in_the_way_and_a_wrong_source_are_refused`)

### Requirement: Pick: A pick during an open update session is carried by its finish

A pick while a `brd update` resolve session is open on the branch MUST land, and the finish of the session MUST carry
the picked commits as a peer's.

#### Scenario: A pick beside an open resolve session
- **GIVEN** a `brd update` resolve session open on the branch
- **WHEN** a pick runs
- **THEN** it lands, and the finish of the session carries the picked commits as a peer's
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_during_an_open_update_session_is_carried_by_its_finish`)

### Requirement: The check of the tree: The setting is a gate of the pick, not of the lease

`pick_requires_check = true` in `[lease]` of the committed `.branchdeck.toml` (the gate of
[brd lease](../lease/spec.md#pick)) MUST be a gate of the pick, not of the lease: `brd pick --from <lease> --follow`,
which `brd lease pick` runs, MUST meet it as `brd lease pick` does.

#### Scenario: A pick from a lease without brd lease pick
- **GIVEN** `pick_requires_check = true` in `[lease]` of the committed `.branchdeck.toml`
- **WHEN** an agent runs `brd pick --from <lease> --follow` instead of `brd lease pick`
- **THEN** the pick meets the gate of the check of the tree
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

### Requirement: The check of the tree: A pick from a worktree lands only a tree a green check recorded

With the setting in the checkout the pick runs in (`-C`), `brd pick --from <dir>` MUST land only a tree of HEAD of the
worktree of `dir` that a green record of its `bd-checks.jsonl` names, as `brd lease pick` does: else nothing lands,
`unchecked_tree` (exit 4).

#### Scenario: A tree that no green record names
- **GIVEN** the setting in the checkout of `-C`, and a tree of HEAD of the worktree of `dir` that no green record of its
  `bd-checks.jsonl` names
- **WHEN** `brd pick --from <dir>` runs
- **THEN** nothing lands, `unchecked_tree` (exit 4), whose message starts `brd pick:`, names the tree, HEAD and the
  last check
- **AND** the message names both ways on with this pick again: `brd pick --from <dir> [--follow]`, then the same with
  `--allow-unchecked '<reason>'`
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

#### Scenario: A tree that a green record names
- **GIVEN** the setting, and a green record of `bd-checks.jsonl` that names the tree of HEAD of the worktree
- **WHEN** `brd pick --from <dir>` runs
- **THEN** the tree lands
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

### Requirement: The check of the tree: The pick expects the source's HEAD it checked

The pick MUST expect the source's HEAD to be the commit it checked (`head_moved` otherwise).

#### Scenario: The source moved after its check
- **WHEN** the source's HEAD is not the commit the check of the tree checked
- **THEN** the pick is `head_moved`

### Requirement: The check of the tree: The check reads git once, nothing without the setting

The check MUST be one `rev-parse` of the git dir, HEAD and its tree, and for a stale tree one `diff-tree`; without the
setting and without --dry-run the pick MUST read neither the records nor git for it.

#### Scenario: The processes of the tree check
- **WHEN** a pick with the setting checks the tree
- **THEN** it runs one `rev-parse` of the git dir, HEAD and its tree, and one `diff-tree` for a stale tree
  [@test] crates/bd-cli/src/process_budget_test.rs (`the_tree_check_of_a_lease_pick`)

#### Scenario: The check of a dry run without a record
- **WHEN** `--dry-run` of `--from` reports the check of a worktree that has no record
- **THEN** the check runs one `rev-parse` of the git dir, HEAD and its tree, and no `diff-tree`
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick_into_a_branch_the_target_has`)

#### Scenario: A pick without the setting
- **WHEN** a pick without the setting and without --dry-run runs
- **THEN** it reads neither the records nor git for the check
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick`)

### Requirement: The check of the tree: Revisions with the setting land nothing

`brd pick <rev>` and `<a>..<b>` with the setting MUST land nothing, `unchecked_tree` (exit 4): revisions name no
worktree whose records brd could read; the message names the pick from the worktree that built them and
`--allow-unchecked`.

#### Scenario: A pick of revisions under the gate
- **GIVEN** the setting in the checkout the pick runs in
- **WHEN** `brd pick <rev>` or `<a>..<b>` runs
- **THEN** nothing lands, `unchecked_tree` (exit 4)
- **AND** the message names the pick from the worktree that built them and `--allow-unchecked`
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

### Requirement: The check of the tree: A dry run of revisions reports no check and never refuses

`--dry-run` MUST report no `check` for revisions and MUST never refuse.

#### Scenario: A dry run of revisions under the gate
- **GIVEN** the setting in the checkout the pick runs in
- **WHEN** `--dry-run` of revisions runs
- **THEN** it reports no `check` and does not refuse
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

### Requirement: The check of the tree: A dry run of a worktree reports the check

`--dry-run` of `--from` MUST report the check in `check` (`state` `ok`, `stale` or `missing`, `record`), with or without
the setting, and MUST never refuse for it.

#### Scenario: A dry run of a worktree
- **WHEN** `--dry-run` of `--from` runs, with or without the setting
- **THEN** it reports the check in `check`: its `state` `ok`, `stale` or `missing`, and the `record`
- **AND** it does not refuse for it
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_from_a_worktree_lands_only_a_tree_that_a_green_check_recorded`)

### Requirement: The check of the tree: --allow-unchecked lands without a check and says why

`--allow-unchecked <reason>` MUST land without a check, of a worktree or of revisions: the reason is required (a blank
one is `invalid_arguments`), the output repeats it (`unchecked`) and stderr names it.

#### Scenario: A pick with a reason
- **WHEN** a pick of a worktree or of revisions runs with `--allow-unchecked <reason>`
- **THEN** it lands without a check, the output repeats the reason (`unchecked`) and stderr names it
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)

#### Scenario: A blank reason
- **WHEN** the reason of `--allow-unchecked` is blank
- **THEN** the pick is `invalid_arguments`
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)

### Requirement: The check of the tree: --unchecked is the same flag as --allow-unchecked

`--unchecked`, its name in `brd lease pick` before, MUST be the same flag.

#### Scenario: The old name of the flag
- **WHEN** a pick runs with `--unchecked <reason>`
- **THEN** it lands without a check as with `--allow-unchecked`
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)

### Requirement: The check of the tree: --allow-unchecked with --remote is invalid

With `--remote` it MUST be `invalid_arguments`: no check gates the commits someone else pushed, which the user saw
(`--expect-remote`), and the app's Pick Remote Commits has no gate either.

#### Scenario: A remote pick with the flag
- **WHEN** `brd pick --remote` runs with `--allow-unchecked`
- **THEN** it is `invalid_arguments`
  [@test] crates/bd-update/src/pick_test.rs (`allow_unchecked_lands_a_pick_without_a_check_and_says_why`)

### Requirement: The commit rules: [commit] check names the rules a message must pass

`[commit] check` of the committed `.branchdeck.toml` MUST name the rules a message must pass (`commit_rules`):
`"patronus"`, the `commitMessageFormat` of `.patronus/config.yaml`, or `["<regex>", ...]`, one of which must match.

#### Scenario: The rules of the monorepo
- **GIVEN** `[commit] check = "patronus"`
- **WHEN** the rules are read
- **THEN** they are the `commitMessageFormat` of `.patronus/config.yaml`
  [@test] crates/commit-rules/src/lib_test.rs (`the_commit_table_names_the_rules_and_a_bad_value_is_an_error`)

#### Scenario: The own regexes of a repository
- **GIVEN** `[commit] check = ["<regex>", ...]`
- **WHEN** a message is checked
- **THEN** it passes when one of the regexes matches, and is STRICT otherwise
  [@test] crates/commit-rules/src/lib_test.rs (`the_own_regexes_of_a_repository_are_strict`)

### Requirement: The commit rules: The pick checks the message of each commit before it lands

With `[commit] check` in the checkout the pick runs in (`-C`), `brd pick` of a worktree or of revisions and
`brd lease pick` MUST check the message of each picked commit before anything lands: the message that the baseline
gets, without its `(branch)` prefix (`virtual_message_of`) and without the leading `fixup! `, `squash! ` and `amend! `,
which `brd fold` removes before a push.

#### Scenario: The message the baseline gets
- **GIVEN** `[commit] check` in the checkout of `-C`
- **WHEN** `brd pick` of a worktree or of revisions, or `brd lease pick`, runs
- **THEN** the message of each picked commit is checked before anything lands, without its `(branch)` prefix
  (`virtual_message_of`) and without the leading `fixup! `, `squash! ` and `amend! `
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_rejected_message_until_the_reword_in_the_lease`)
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_refuses_a_message_that_the_commit_rules_reject`)

### Requirement: The commit rules: A rule matches the whole message

A rule MUST match the whole message, as the safe-push plugin of the IntelliJ monorepo matches (`(?ms)\A(?:<rule>)\z`):
one of `rules` MUST match, none of `negativeRules` may.

#### Scenario: A subject without a ticket or a label
- **WHEN** a message is checked against the rules of the monorepo
- **THEN** it passes only when one of `rules` matches the whole message and none of `negativeRules` does
  [@test] crates/commit-rules/src/lib_test.rs (`the_rules_of_the_monorepo_refuse_a_subject_without_a_ticket_or_a_label`)

### Requirement: The commit rules: The paths of a commit decide its severity

The severity of a rejected message MUST follow `severityRules`: a rule applies when every path the commit changes
matches one of its globs (`/fleet/**`), the strictest of those wins, else `severityDefault`; `warningOnly` makes every
one a warning at most. `IGNORE` passes.

#### Scenario: The severity rules of the paths
- **WHEN** a message is rejected
- **THEN** a rule of `severityRules` applies when every path the commit changes matches one of its globs (`/fleet/**`),
  the strictest of those wins, else `severityDefault`
- **AND** `IGNORE` passes
  [@test] crates/commit-rules/src/lib_test.rs (`the_paths_of_a_commit_decide_its_severity`)

#### Scenario: The default severity and warningOnly
- **WHEN** no rule of the paths applies, or the format sets `warningOnly`
- **THEN** the severity is `severityDefault`, a warning at most with `warningOnly`
  [@test] crates/commit-rules/src/lib_test.rs (`only_the_top_level_block_of_the_format_is_read`)

### Requirement: The commit rules: A STRICT message lands nothing

A STRICT one MUST land nothing: `message_format` (exit 4), its `commits` the ids, `message_format` the commits with
their `subject` and `severity`, the `hint` and the `documentation` of the rules. The message MUST name the reword where
each commit was made, the pick again, and `--allow-message-format`.

#### Scenario: A rejected message refuses the pick
- **WHEN** the rules reject the message of a picked commit as STRICT
- **THEN** nothing lands: `message_format` (exit 4), its `commits` the ids, `message_format` the commits with their
  `subject` and `severity`, the `hint` and the `documentation` of the rules
- **AND** the message names the reword where each commit was made (in the lease
  `brd -C <lease> reword <id> --subject '<subject>'`, for `--from` in that worktree, for revisions where they live), the
  pick again, and `--allow-message-format`
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_refuses_a_message_that_the_commit_rules_reject`)

### Requirement: The commit rules: A WARNING message lands and is named

A WARNING one MUST land, named in `message_warnings`.

#### Scenario: A warning lands
- **WHEN** the rules reject the message of a picked commit as a WARNING
- **THEN** the commit lands, named in `message_warnings`
  [@test] crates/bd-update/src/pick_test.rs (`allow_message_format_lands_a_rejected_message_and_a_warning_lands_with_it`)

### Requirement: The commit rules: A dry run reports the rejected messages and never refuses

`--dry-run` MUST report both in `message_warnings` and MUST never refuse.

#### Scenario: A dry run of rejected messages
- **WHEN** `--dry-run` picks commits whose messages are STRICT or a WARNING
- **THEN** it reports both in `message_warnings` and does not refuse
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_refuses_a_message_that_the_commit_rules_reject`)

### Requirement: The commit rules: --allow-message-format lands a STRICT message

`--allow-message-format <reason>` of `brd pick` and `brd lease pick` MUST land a STRICT one: the reason is required (a
blank one is `invalid_arguments`) and the output repeats it (`message_format_allowed`). So a gap between brd and
Patronus never blocks an agent.

#### Scenario: A rejected message with a reason
- **WHEN** `brd pick` or `brd lease pick` runs with `--allow-message-format <reason>` on a STRICT message
- **THEN** it lands and the output repeats the reason (`message_format_allowed`)
  [@test] crates/bd-update/src/pick_test.rs (`allow_message_format_lands_a_rejected_message_and_a_warning_lands_with_it`)
  [@test] crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_rejected_message_until_the_reword_in_the_lease`)

#### Scenario: A blank reason
- **WHEN** the reason of `--allow-message-format` is blank
- **THEN** the pick is `invalid_arguments`
  [@test] crates/bd-update/src/pick_test.rs (`allow_message_format_lands_a_rejected_message_and_a_warning_lands_with_it`)

### Requirement: The commit rules: --allow-unchecked does not skip the message check

`--allow-unchecked` MUST NOT skip this check.

#### Scenario: A pick without the check of the tree
- **WHEN** a pick with `--allow-unchecked` has a STRICT message
- **THEN** the message check still refuses it

### Requirement: The commit rules: A remote pick is not checked

`--remote` MUST NOT be checked, as for the check of the tree.

#### Scenario: The messages of remote commits
- **WHEN** `brd pick --remote` picks commits
- **THEN** their messages are not checked

### Requirement: The commit rules: A bad [commit] is an error of the file

A bad `[commit]` (`check = 5`, an unknown key, a regex that does not compile, an empty array) MUST be an error of the
file (`invalid_arguments`): its owner wrote it, and a typo MUST NOT drop the check without a word.

#### Scenario: A bad commit table
- **WHEN** `[commit]` is `check = 5`, has an unknown key, a regex that does not compile, or an empty array
- **THEN** the pick is an error of the file, `invalid_arguments`
  [@test] crates/bd-update/src/pick_test.rs (`a_bad_commit_table_is_an_error_and_missing_rules_skip_the_check`)
  [@test] crates/commit-rules/src/lib_test.rs (`the_commit_table_names_the_rules_and_a_bad_value_is_an_error`)

### Requirement: The commit rules: A Patronus config that gives no rules skips the check

The content of `.patronus/config.yaml` belongs to another team and MUST fail open: a missing file, no
`commitMessageFormat`, a block that does not parse, a rule that does not compile or an unknown severity skips the check,
`message_check_skipped` and stderr say why, and Patronus still checks the push.

#### Scenario: Missing rules skip the check
- **WHEN** `.patronus/config.yaml` is missing, has no `commitMessageFormat`, a block that does not parse, a rule that
  does not compile or an unknown severity
- **THEN** the check is skipped, `message_check_skipped` and stderr say why
  [@test] crates/bd-update/src/pick_test.rs (`a_bad_commit_table_is_an_error_and_missing_rules_skip_the_check`)
  [@test] crates/commit-rules/src/lib_test.rs (`a_patronus_config_that_cannot_give_rules_skips_the_check_with_its_cause`)

### Requirement: The commit rules: The check reads one block and one log before the lock

The check MUST read only the top-level `commitMessageFormat` block of `.patronus/config.yaml` (the file of the IntelliJ
monorepo has 750 KB of other checks), and the messages and the paths of all picked commits with one
`log --no-walk=unsorted --stdin -z --name-only`, before the lock; a check made again on the same commits MUST read no
git.

#### Scenario: Only the top-level block
- **WHEN** `.patronus/config.yaml` has other blocks besides the top-level `commitMessageFormat`
- **THEN** the check reads only that block
  [@test] crates/commit-rules/src/lib_test.rs (`only_the_top_level_block_of_the_format_is_read`)

#### Scenario: One log for the messages and the paths
- **WHEN** the check reads the picked commits
- **THEN** it reads their messages and paths with one `log --no-walk=unsorted --stdin -z --name-only`, before the lock
- **AND** a check made again on the same commits reads no git
  [@test] crates/git-ops/src/commit_list_test.rs (`messages_and_paths_of_commits_come_from_one_log`)
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick_with_the_commit_rules`)

### Requirement: The commit rules: Branch Deck's rules take its conventional subjects

The `.branchdeck.toml` of Branch Deck MUST name the conventional form of its `git log` (`feat(scope): …`, `Revert "…`),
which release-please reads.

#### Scenario: The subjects of this repository
- **WHEN** the rules of the `.branchdeck.toml` of Branch Deck check its conventional subjects
- **THEN** they pass
  [@test] crates/commit-rules/src/lib_test.rs (`the_rules_of_this_repository_take_its_conventional_subjects`)

### Requirement: Remote commits: A remote pick takes in the commits a push would drop

`brd pick --remote <branch>` MUST sync, fetch the remote branch of `<branch>` alone (before the lock) and pick the
commits a push of it would drop (`commits_behind`, all of them: no older version of a commit the branch has, no landed
one), oldest first, onto the checked out branch. The author MUST stay.

#### Scenario: Someone pushed a commit to the remote branch
- **GIVEN** someone else pushed a commit to the remote branch of `<branch>`
- **WHEN** `brd pick --remote <branch>` runs
- **THEN** it syncs, fetches the remote branch of `<branch>` alone before the lock, and picks the commits a push of it
  would drop (`commits_behind`), oldest first, onto the checked out branch, the author kept
- **AND** the push of `<branch>` then drops nothing
  [@test] crates/bd-update/src/pick_test.rs (`a_remote_pick_takes_in_the_commit_someone_pushed_and_the_push_then_drops_nothing`)

### Requirement: Remote commits: The copies get the prefix of their branch

The copies of those whose subject groups elsewhere MUST get the `(<branch>)` prefix, so they land in `<branch>` and its
push then drops nothing (no `--force`).

#### Scenario: A remote commit whose subject groups elsewhere
- **WHEN** a picked remote commit's subject groups elsewhere
- **THEN** its copy gets the `(<branch>)` prefix and lands in `<branch>`, whose push then needs no `--force`
  [@test] crates/git-ops/src/pick_test.rs (`listed_commits_land_alone_with_the_prefix_of_their_branch`)

### Requirement: Remote commits: Only the listed commits are replayed

Only the listed commits MUST be replayed, each run of them after its own parent, never the commits they were made on: a
commit between two listed ones stays out.

#### Scenario: Listed commits apart
- **GIVEN** a commit between two listed ones
- **WHEN** the listed commits are replayed
- **THEN** each run of them is replayed after its own parent and the commit between them stays out
  [@test] crates/git-ops/src/pick_test.rs (`listed_commits_apart_replay_each_after_its_parent`)

### Requirement: Remote commits: A remote branch at another commit than expected picks nothing

`--expect-remote <id>`: a fetched remote branch at another commit MUST pick nothing, `lease_rejected` (exit 4), as the
lease of a push.

#### Scenario: The remote branch moved since the user saw it
- **WHEN** `brd pick --remote <branch> --expect-remote <id>` fetches the remote branch at another commit
- **THEN** it picks nothing, `lease_rejected` (exit 4)
  [@test] crates/bd-update/src/pick_test.rs (`a_remote_pick_from_another_head_than_the_one_seen_picks_nothing`)

### Requirement: Remote commits: A remote branch with nothing to pick is nothing_to_pick

A remote branch with no commit the branch lacks MUST be `nothing_to_pick` (exit 4).

#### Scenario: The branch has every remote commit
- **WHEN** the remote branch has no commit the branch lacks
- **THEN** the remote pick is `nothing_to_pick` (exit 4)
  [@test] crates/bd-update/src/pick_test.rs (`a_remote_pick_takes_in_the_commit_someone_pushed_and_the_push_then_drops_nothing`)

### Requirement: Remote commits: The processes of a remote pick

Its processes MUST be one sync, one `fetch` (with the `for-each-ref` of the tip it moves the remote-tracking ref from and
that `update-ref`), the walks of the remote status, one `cat-file` and one `fast-import` of the prefixed copies besides
those of a pick.

#### Scenario: The process budget of a remote pick
- **WHEN** `brd pick --remote <branch>` runs
- **THEN** it runs one sync, one `fetch` with the `for-each-ref` and the `update-ref` of the remote-tracking ref, the
  walks of the remote status, one `cat-file` and one `fast-import`, besides the processes of a pick
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_remote_pick`)

### Requirement: The lock: The check runs before the repository lock

The check (the range, the replay, the dry runs of both worktrees on copies of their index) MUST run before the
repository lock, and another writer MUST get the lock meanwhile.

#### Scenario: The replay and the dry runs before the lock
- **WHEN** a pick with `--follow` runs
- **THEN** the range, the replay and the dry runs of both worktrees on copies of their index run before the repository
  lock
  [@test] crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)

#### Scenario: Another writer during the replay
- **WHEN** another writer asks for the repository lock while a pick replays
- **THEN** it gets the lock meanwhile
  [@test] crates/bd-update/src/pick_test.rs (`another_writer_takes_the_lock_while_a_pick_replays`)

### Requirement: The lock: Under the lock only the witness, the local changes and the moves

Under the repository lock the pick MUST run only the marks of interrupted moves, the witness (the target's branch, both
HEADs, the upstream), the local changes on the paths the moves write (by their index entries and content, the exact dry
run only where that finds one) and the two moves.

#### Scenario: The processes under the lock
- **WHEN** a pick with `--follow` holds the repository lock
- **THEN** it reads the marks of interrupted moves, the witness, and the local changes on the paths the moves write by
  their index entries and content, and runs the two moves, nothing else
  [@test] crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)

### Requirement: The lock: A check that moves nothing answers without the lock

A check that moves nothing (a conflict, local changes in the way, every commit present) MUST answer without the lock.

#### Scenario: A pick that moves nothing
- **WHEN** the check finds a conflict, local changes in the way, or every commit present
- **THEN** the pick answers without taking the repository lock

### Requirement: The lock: A pick never waits for a sync

A pick MUST never wait for a sync: another agent's `brd sync` of the target, held in its merges, takes no repository
lock, so the pick checks, moves both worktrees and ends meanwhile.

#### Scenario: A pick beside a held sync
- **GIVEN** another agent's `brd sync` of the target, held in its merges
- **WHEN** a pick runs
- **THEN** it checks, moves both worktrees and ends meanwhile
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_beside_a_held_sync_does_not_wait_for_it`)

### Requirement: The lock: A worktree that moved before the lock makes the pick check again

A target or a source that moved between the check and the lock MUST make the pick check again without the lock and land
on the new tip, the source following there; after two such checks the third MUST run under the lock.

#### Scenario: A commit on the target during the replay
- **WHEN** the target gets a commit between the check and the lock
- **THEN** the pick checks again without the lock, keeps that commit and lands on the new tip, the source following
  there
  [@test] crates/bd-update/src/pick_test.rs (`a_commit_on_the_target_during_the_replay_of_a_pick_is_kept_and_the_pick_checks_again`)

#### Scenario: A target that moves during every check
- **WHEN** the target moves during two checks
- **THEN** the third check runs under the lock
  [@test] crates/bd-update/src/pick_test.rs (`a_target_that_moves_during_every_check_is_checked_under_the_lock_at_last`)

### Requirement: The lock: A file edited before the lock blocks the pick

A file a move writes, changed between the check and the lock, MUST refuse the pick as `uncommitted_overlap`, nothing
moved.

#### Scenario: A file edited during the replay
- **WHEN** a file a move writes changes between the check and the lock
- **THEN** the pick is refused as `uncommitted_overlap`, nothing moved
  [@test] crates/bd-update/src/pick_test.rs (`a_file_edited_during_the_replay_of_a_pick_blocks_it`)

### Requirement: The lock: The pick waits for the index lock of another process

The index lock of another process in the target or the source (an IDE's `git status`) MUST make the pick wait, before
the repository lock and in the move, at most 30 s per write, and MUST never roll it back.

#### Scenario: The index lock of another process
- **GIVEN** the index lock of another process in the target or the source, as of an IDE's `git status`
- **WHEN** the pick runs
- **THEN** it waits, before the repository lock and in the move, at most 30 s per write, and lands with its files once
  the lock is gone
  [@test] crates/git-ops/src/pick_test.rs (`a_pick_waits_for_the_index_lock_of_another_process_and_then_lands_with_its_files`)
  [@test] crates/git-ops/src/index_lock_test.rs (`a_step_blocked_by_the_lock_of_another_process_runs_again_once_the_lock_is_gone`)

#### Scenario: A step that fails on its own
- **GIVEN** an index lock that another process took after a step of the pick started
- **WHEN** that step fails for a cause of its own
- **THEN** it returns its error after one run, without a wait for the lock: only a lock that was there when the step
  started blocked it
  [@test] crates/git-ops/src/index_lock_test.rs (`a_step_that_fails_on_its_own_returns_its_error_after_one_run_although_a_younger_lock_exists`)

### Requirement: The lock: An index lock that appears under the repository lock sends the pick back

An index lock that appears while the pick takes the repository lock MUST make it release the repository lock and send
the pick back to check and wait outside it.

#### Scenario: An index lock appears while the pick takes the repository lock
- **WHEN** an index lock appears while the pick takes the repository lock
- **THEN** the pick releases it, checks again and waits for the index lock outside it

### Requirement: The lock: An index lock that no process owns is removed

A lock that no process owns (left by a killed git) MUST be removed after two checks 2 s apart.

#### Scenario: A lock left by a killed git
- **GIVEN** an index lock that no process owns, left by a killed git
- **WHEN** the pick meets it
- **THEN** it is removed after two checks 2 s apart, and the step runs again and the files follow
  [@test] crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)
  [@test] crates/git-ops/src/index_lock_test.rs (`a_lock_that_no_process_owns_is_removed_and_the_step_runs_again`)

#### Scenario: Another lock file took the place of the checked one
- **WHEN** a new lock file takes the place of the one the two checks saw
- **THEN** it is not removed
  [@test] crates/git-ops/src/index_lock_test.rs (`a_lock_file_that_took_the_place_of_the_checked_one_is_not_removed`)

### Requirement: The lock: An index lock that stays answers locked

A lock that stays MUST answer `locked` (retryable, exit 3) with the lock file, its age and the processes that may own it
(`processes`), nothing changed; in the source it MUST make `followed.state` `blocked`, the target picked.

#### Scenario: A lock that a process has open
- **GIVEN** an index lock that a process has open and keeps
- **WHEN** the wait ends
- **THEN** the answer is `locked` (retryable, exit 3) with the lock file, its age and the processes that may own it
  (`processes`), nothing changed
  [@test] crates/git-ops/src/index_lock_test.rs (`a_lock_that_a_process_has_open_stays_and_the_answer_names_the_process`)
  [@test] crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`)

#### Scenario: A lock that stays in the source
- **WHEN** the lock that stays is in the source
- **THEN** `followed.state` is `blocked` and the target is picked

### Requirement: `--follow`: The source worktree moves onto the new tip

`--follow` MUST move the source worktree onto the new tip with its files, a detached HEAD staying detached
(`option no-deref`, and under the locks HEAD MUST still be detached) and a branch moving the branch; its reflog MUST
name the move `branch-deck: pick`.

#### Scenario: A detached source follows
- **GIVEN** a source worktree with a detached HEAD
- **WHEN** a pick with `--follow` lands
- **THEN** the source moves onto the new tip with its files, its HEAD still detached (`option no-deref`)
- **AND** its reflog names the move `branch-deck: pick`
  [@test] crates/git-ops/src/pick_test.rs (`a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows`)

#### Scenario: A source on a branch follows
- **GIVEN** a source worktree on a branch
- **WHEN** a pick with `--follow` lands
- **THEN** its branch moves onto the new tip
  [@test] crates/git-ops/src/pick_test.rs (`a_source_on_a_branch_moves_its_branch_and_one_with_a_cherry_pick_in_progress_is_refused`)

### Requirement: `--follow`: An operation in progress in the source refuses the follow

A rebase, merge, cherry-pick or revert in progress in the source MUST refuse it (`operation_in_progress`).

#### Scenario: A cherry-pick in progress in the source
- **GIVEN** a rebase, merge, cherry-pick or revert in progress in the source
- **WHEN** a pick with `--follow` runs
- **THEN** it is refused as `operation_in_progress`
  [@test] crates/git-ops/src/pick_test.rs (`a_source_on_a_branch_moves_its_branch_and_one_with_a_cherry_pick_in_progress_is_refused`)

### Requirement: `--follow`: Without it the source is only read

Without `--follow` the source MUST only be read.

#### Scenario: A pick without --follow
- **WHEN** a pick without `--follow` lands
- **THEN** the source worktree is only read, its HEAD and files as they were

### Requirement: `--follow`: A source that moved after the check stays where it is

Both moves MUST be checked before either moves. A source that moved or switched after the check MUST be left where it
is while the branch moves: `followed.state` `head_moved` (or `blocked`), exit 0; the same `brd pick` again finds every
commit present and moves only the source.

#### Scenario: A source that switched after the check
- **WHEN** the source moves or switches after the check
- **THEN** the branch moves and the source stays where it is: `followed.state` `head_moved` (or `blocked`), exit 0
- **AND** the same `brd pick` again finds every commit present and moves only the source
  [@test] crates/git-ops/src/pick_test.rs (`a_source_that_switched_after_the_check_stays_and_the_same_pick_again_moves_it`)

#### Scenario: A source that switched to a branch at another commit
- **WHEN** the source switches to a branch at another commit after the check
- **THEN** the branch of the target moves and the source stays where it is, attached to that branch: `followed.state`
  `head_moved`
  [@test] crates/git-ops/src/pick_test.rs (`a_source_that_switched_to_a_branch_at_another_commit_after_the_check_is_attached_and_stays`)

### Requirement: `--follow`: A pick or a follow killed before its files is finished

A pick or a follow killed between the move of HEAD and its files MUST leave the mark
`refs/branch-deck/update-pending/<worktree id>`; the next pick, update or undo MUST let the files follow first, a
detached HEAD by the reflog of `HEAD`.

#### Scenario: A move killed before its files followed
- **WHEN** a pick or a follow is killed between the move of HEAD and its files
- **THEN** the mark `refs/branch-deck/update-pending/<worktree id>` is left
- **AND** the next pick, update or undo lets the files follow first, a detached HEAD by the reflog of `HEAD`
  [@test] crates/git-ops/src/pick_test.rs (`a_pick_and_a_follow_killed_before_their_files_followed_are_finished`)

### Requirement: Undo: The undo token names both moves with full ids

The undo token MUST be `pick:<previous head>..<new head>` with full ids, with `+follow:<worktree id>:<previous>..<new>`
when the source moved.

#### Scenario: The token of a pick that the source followed
- **WHEN** a pick lands and the source follows
- **THEN** its undo token is `pick:<previous head>..<new head>+follow:<worktree id>:<previous>..<new>` with full ids
  [@test] crates/update-core/src/pick_test.rs (`an_undo_token_prints_and_parses_the_target_and_the_worktree_that_followed`)
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)

### Requirement: Undo: brd undo moves the branch and the source back

`brd undo` MUST check both moves, then move the branch and the source back with their files; `follow_undone` says
whether the source moved back.

#### Scenario: The undo of a pick with a follow
- **WHEN** `brd undo` runs with the token of a pick that the source followed
- **THEN** it checks both moves, then moves the branch and the source back with their files
- **AND** `follow_undone` says the source moved back
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)

### Requirement: Undo: Undoing again is no error and the commits are pickable again

Undoing again MUST be no error (`already_undone`), and the worktree's commits MUST be pickable again.

#### Scenario: The undo of an undone pick
- **WHEN** `brd undo` runs again with the same token
- **THEN** it is no error, `already_undone`
- **AND** the worktree's commits are pickable again
  [@test] crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)
  [@test] crates/git-ops/src/pick_test.rs (`the_undo_moves_the_target_and_the_detached_source_back`)

### Requirement: Undo: The undo is refused unless the pick is the last reflog entry

The last entry of each reflog MUST be that pick, else the undo is refused as `invalid_arguments`; a branch that moved
on MUST be `undo_stale`.

#### Scenario: The undo of a pick that is the last entry
- **WHEN** the last entry of each reflog is that pick
- **THEN** the undo moves the target and the detached source back
  [@test] crates/git-ops/src/pick_test.rs (`the_undo_moves_the_target_and_the_detached_source_back`)

#### Scenario: A branch that moved on
- **WHEN** the branch moved on since the pick
- **THEN** the undo is refused as `undo_stale`
  [@test] crates/git-ops/src/pick_test.rs (`the_undo_moves_the_target_and_the_detached_source_back`)
  [@test] crates/bd-update/src/error_code_test.rs (`bd_undo_answers_the_code_of_each_refusal_that_the_app_answers_and_a_moved_head_makes_it_stale`)

#### Scenario: The last move is another one
- **GIVEN** the branch at the new head of the pick, and the last entry of its reflog another move
- **WHEN** `brd undo` runs with the token of the pick
- **THEN** the undo is refused as `invalid_arguments`
  [@test] crates/bd-update/src/error_code_test.rs (`bd_undo_answers_the_code_of_each_refusal_that_the_app_answers_and_a_moved_head_makes_it_stale`)

### Requirement: Undo: The undo locks only the witness and the moves back

The checks of both moves MUST run before the repository lock, which holds only the witness of each worktree (its HEAD
and the mark of an interrupted move, read in its own directory) and the moves back; a remote pick MUST fetch and check
before it as every pick. A worktree with such a mark before the check MUST let its files follow under the lock first.

#### Scenario: A remote pick and its undo
- **WHEN** a remote pick runs
- **THEN** it fetches and checks before the repository lock, as every pick
- **AND** its undo checks before the repository lock, which holds only the witness of each worktree, read in its own
  directory, and the moves back
  [@test] crates/bd-update/src/pick_test.rs (`a_remote_pick_and_its_undo_lock_only_the_witness_and_the_move`)

#### Scenario: A worktree with the mark of an interrupted move
- **GIVEN** a worktree with the mark of an interrupted move before the check of the undo
- **WHEN** the undo takes the lock
- **THEN** it lets the files of that worktree follow under the lock first

### Requirement: Error codes: A conflict exits 1, an output and not an error

A pick whose commit conflicts MUST exit 1 with `state: "conflict"`: an output, not an error.

#### Scenario: The exit of a conflict
- **WHEN** a commit of the pick conflicts
- **THEN** `brd pick` prints `state: "conflict"` and exits 1
  [@test] crates/bd-update/src/pick_test.rs (`a_conflict_changes_nothing_and_says_what_to_do`)

### Requirement: Error codes: invalid_arguments exits 2

`invalid_arguments` MUST exit 2.

#### Scenario: The exit of invalid arguments
- **WHEN** `brd pick` fails with `invalid_arguments`
- **THEN** it exits 2
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Error codes: head_moved and locked exit 3, retryable

`head_moved` and `locked` MUST exit 3, retryable.

#### Scenario: The exit of a moved head or a lock
- **WHEN** `brd pick` fails with `head_moved` or `locked`
- **THEN** it exits 3, retryable
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Error codes: The refusals exit 4

`range_unknown`, `source_on_newer_baseline`, `home_branch_switched`, `uncommitted_overlap`, `operation_in_progress`,
`detached_head`, `finish_pending`, `undo_stale`, `unchecked_tree`, `message_format`, `baseline_parent`, and with
--remote `lease_rejected` and `nothing_to_pick`, MUST exit 4.

#### Scenario: The exit of a refusal
- **WHEN** `brd pick` or its undo is refused with one of these codes
- **THEN** it exits 4
  [@test] crates/bd-cli/src/error_test.rs (`every_error_code_has_the_exit_code_and_the_retry_of_its_class`)

### Requirement: Error codes: The outputs match brd schema

The outputs MUST match `brd schema` (`pick`, `undo`).

#### Scenario: The outputs of a pick and its undo
- **WHEN** `brd pick` or `brd undo` prints its output
- **THEN** it matches its output of `brd schema`
  [@test] crates/bd-update/src/pick_test.rs (`pick_outputs_match_the_schema`)

#### Scenario: Every output in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists every output
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)

### Requirement: Error codes: A pick runs a fixed number of git processes

A pick MUST run a fixed number of git processes: one `replay`, the witness, the two moves with their marks, one read of
the grouping after them (the inputs and one `log` of the commits ahead); the diffs of the dependency check only for a
prefix that names a branch the branch has.

#### Scenario: The process budget of a pick
- **WHEN** a pick lands
- **THEN** it runs one `replay`, the witness, the two moves with their marks and one read of the grouping after them,
  the inputs and one `log` of the commits ahead
- **AND** the diffs of the dependency check only for a prefix that names a branch the branch has
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_pick`)

## Testing / Local Run

- `cargo nextest run -p git-ops pick_test` runs the operation on `TestRepo` fixtures with a linked worktree;
  `cargo nextest run -p bd-cli pick_test` runs the command end to end.

## Open Questions / Risks

- The dependency check sees lines only, not symbols: a picked commit that calls a function another branch adds in
  another file is not named; `brd check` names it after the pick (`missing_declarations`).

- A source directly on the branch's HEAD, picked in the same second as its commits were made, replays into the same
  commits: the branch fast-forwards to them and the source does not move (`at_tip`).
- The range of revisions (`<a>..<b>`) excludes the branch's history but not by the reflog: it names exactly the
  commits to pick.
- The patch ids are compared with the branch's commits above the base and the upstream only (`^<upstream>` limits
  both sides of the range): a commit that already reached the upstream in another form is not found by its patch id.
  It drops as empty at the replay (`by: "empty"`) or conflicts.

## References

- [Git Operations](../../../../docs/design/git-operations.md): "Pick" and "Update project".
- [Agent CLI guide](../../../../docs/contributing/agent-cli.md): `brd pick`.
