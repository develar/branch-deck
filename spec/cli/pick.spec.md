---
name: bd pick
description: Requirements for bd pick, which lands the commits of another worktree on the branch checked out here without git cherry-pick, moves that worktree along with --follow, and takes both back with bd undo.
style: plain-1
targets:
  - ../../crates/bd-update/src/pick_command.rs
  - ../../crates/bd-cli/src/agent_help.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/bd-rewrite/src/rewrite.rs
  - ../../crates/bd-update/src/undo_command.rs
  - ../../crates/bd-cli/src/schema.rs
  - ../../crates/update-core/src/pick.rs
  - ../../crates/history-rewrite/src/stack.rs
  - ../../crates/trial-check/src/dependencies.rs
  - ../../crates/git-ops/src/pick.rs
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/git-ops/src/ref_transaction.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/operations/src/rewrite.rs
  - ../../src/lib/git/pick-remote.ts
---

# bd pick

Status: Implemented
Date: 2026-10-18

## Summary

Several agent sessions share one checkout. Each one commits in a worktree of its own (a `bd lease` worktree, a detached
HEAD that shares the git dir), and its work counts once it is on the branch of the shared checkout, which one owner
rewrites with `bd update`, `bd fold` and `bd group`. `bd pick --from <worktree>` lands the commits of that worktree on
the branch checked out at `-C`. It replays them in memory with `git replay` and leaves out a commit whose change the
branch has already. It checks before the repository lock, which every worktree shares, and under it only reads again
what the check read; then it moves the branch, and the files follow through the guarded working-tree update. `--follow` moves the source worktree onto the new tip the same way. A conflict
or local changes in the way change nothing; `bd undo` takes the pick back, the worktree too. A lease of the pool lands
its commits with `bd lease pick`, this pick run in the checkout it was cut from, onto the branch it had checked out
([bd lease](lease.spec.md)). How it works: "Pick" in
[Git Operations](../../docs/design/git-operations.md). Working beside other sessions: `bd help worktrees`
([Agent CLI](agent-cli.spec.md)).

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
  Commits](../repository/outgoing.spec.md#remote-commits-of-others)).

## Requirements

### Pick

- `bd pick --from <dir>` must land the commits that the HEAD of the worktree of `dir` has and the checked out branch
  lacks: `picked` with each commit, its copy (`new_id`), its subject, the `group` it lands in and what decides it
  (`grouped_by`: `prefix`, `issue_group`, `issue`, or `none` for a subject without a prefix or an issue key). The
  copies keep their author; the committer is the current user. It must print `state: "picked"`, the previous and the
  new HEAD, the `base` of the range and the undo token. The branch's reflog must name the move `branch-deck: pick`.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows`)
- A range that starts at the branch's HEAD (a worktree cut at the tip, the branch not moved since) must land as it is,
  a fast-forward: every `new_id` is its commit, and the worktree of `--follow` has nowhere to move.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_worktree_cut_at_the_tip_lands_its_commits_as_they_are`)
- The committer date of the copies must be the newest committer date of the range and of the branch's HEAD, not the
  time of the pick: the same pick writes the same commits (the `new_head` of `--dry-run` is the one the pick writes),
  a copy is never older than its parent, and the dates of the worktree's commits carry over. A `GIT_COMMITTER_DATE`
  the user sets wins.
  [@test] ../../crates/git-ops/src/pick_test.rs (`the_copies_get_the_newest_committer_date_so_the_check_names_the_commits_the_pick_writes`)
- A commit whose change the branch has already must be listed in `already_present`, with `on`, the branch's commit of
  the same patch id, or `null` when the branch has the change in another form, and `by`: `patch_id` or `empty`; it
  must never become an empty commit. When every commit is present, the state must be `up_to_date` and nothing must
  move.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_commit_the_target_has_already_is_present_with_the_target_commit_of_its_patch`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_dry_run_writes_no_ref_and_a_pick_again_finds_every_commit_present`)
- A commit that the shared branch has with another message (a `bd reword` after the pick) must be present by its
  patch id; after a follow the old tip is the fork point, so a later pick lands only the commits after it.
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_pick_without_follow_finds_the_reworded_commit_present_by_patch_id`, `a_session_rewords_its_landed_commit_from_its_lease`)
- A commit of the range with the patch id of a branch commit above the base must be left out before the replay
  (`by: "patch_id"`; the range's `log --cherry-mark --right-only <branch>...<source HEAD> ^<base> ^<upstream>`), so a
  commit the branch has in a form that no longer applies (the branch changed the same lines since) never conflicts.
  The rest must be replayed in segments between the left out commits, each onto the tip of the one before, so the
  child of a left out commit lands on the copy before it: one chain, never a fork onto the branch's HEAD (as
  `git replay --cherry-pick` would replay it). A commit the replay drops as empty is `by: "empty"`. Without a left out
  commit the pick runs one `replay`, one more per segment otherwise.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_commit_with_the_patch_id_of_a_target_commit_is_left_out_and_its_child_lands_on_the_copy_before_it`)
- A conflict after a left out commit must be searched in its segment only, list the left out commits in
  `already_present` and not in `clean`; `conflict.next` must say never to `git cherry-pick` in the shared checkout (it
  leaves `CHERRY_PICK_HEAD`).
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_conflict_after_a_commit_left_out_by_its_patch_id_names_the_commit_and_lists_the_left_out_one`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_conflict_changes_nothing_and_says_what_to_do`)
- A pick that names the branch the target must have checked out (`bd lease pick`) must refuse another one as
  `home_branch_switched` (exit 4), checked with the branch of the witness under the lock; nothing moves.
  [@test] ../../crates/bd-lease/src/lease_pick_test.rs (`a_lease_pick_refuses_a_home_that_switched_its_branch`)
- A picked commit whose prefix names a branch the branch has already must be listed in `dependencies` when that
  branch is not built on every branch whose commits its hunks depend on (the edges by lines, no symbols), with the
  copy, `prefix_branch` and `depends_on`, in a `--dry-run` too. `plan`, the `bd group` plan that moves the commit
  into the branch of the stack that holds all of them, must make the next `bd sync` clean; a commit that also needs
  its own branch gets `restack`, `bd restack <prefix_branch> <parent>` onto a branch that holds them and its current
  parent. The pick never applies either. A prefix of a new branch is not checked: the check reads the diffs only for a
  prefix that names a branch the branch has.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_picked_commit_that_needs_another_branch_names_it_with_the_plan_that_moves_it`, `a_picked_commit_that_needs_its_own_and_another_branch_offers_the_restack`)
- A `(parent/leaf)` prefix of a picked commit on a branch the branch has as a root (or on another parent) must stack
  the whole branch on `parent`, and the pick must say so in `reparented` (`{"leaf": "parent"}`), in a `--dry-run`
  too; the next `bd sync` must build the stack.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_parent_leaf_prefix_on_a_root_group_stacks_the_group_and_says_so`)
- `groups`, `dependencies` and `reparented` decide nothing: they must be read after the moves and the release of the
  repository lock, from the head the check froze, so the `log` of the commits ahead and the diffs of the dependency
  check never make another writer wait. A failure of that read after a move must keep the pick and its undo token.
  [@test] ../../crates/bd-update/src/pick_test.rs (`the_landing_of_a_pick_is_read_after_the_lock`)
- The range of a worktree must be `<source HEAD> ^<branch> ^<upstream> ^<base>`, the base being the fork point of the
  source in the reflog of the branch, so that a branch folded, regrouped or redated after the worktree was cut gives
  none of its old commits back.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_target_rewritten_after_the_worktree_was_cut_from_it_gives_back_none_of_its_old_commits`)
- A value that a move of bd (a pick, an update, an unapply, a reapply) put into the reflog of the branch and that its
  rollback or its undo took back must be no fork point: a fast-forward pick that rolled back leaves the source's own
  tip there, which would make its range empty and a `--follow` drop its commits. The base must be the value before
  that move, when the move only added commits to it; a value that any other entry put there (the user, a rewrite, a
  rollback or an undo itself, the current tip) stays a fork point, and so does one whose next entry `log -g` skipped.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_fast_forward_pick_that_rolled_back_is_no_fork_point_and_the_next_pick_lands_the_commit`, `a_worktree_that_went_on_from_a_rolled_back_tip_lands_the_commits_below_it_too`)
  [@test] ../../crates/git-ops/src/pick_test.rs (`an_undone_fast_forward_pick_is_no_fork_point`, `only_a_value_whose_every_move_was_rolled_back_or_undone_gives_way_to_the_value_before`)
  [@test] ../../crates/bd-lease/src/lease_test.rs (`a_return_after_a_pick_that_rolled_back_refuses_the_commit`)
- Without a fork point, a shared history that is local (not the upstream's) must be refused as `range_unknown`
  (exit 4), and `bd pick <rev>` or `<a>..<b>` must pick the commits it names. A worktree that stands on a newer
  baseline than the branch must be refused as `source_on_newer_baseline` (exit 4).
  [@test] ../../crates/git-ops/src/pick_test.rs (`without_a_fork_point_a_local_shared_history_is_refused_and_a_range_names_the_commits`, `a_worktree_on_a_newer_baseline_is_refused`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_worktree_on_a_newer_baseline_is_refused`)
- `--dry-run` must take no lock and write at most git objects; it is the check whether the branch has the worktree's
  commits by content, not by date.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_dry_run_writes_no_ref_and_a_pick_again_finds_every_commit_present`)
  [@test] ../../crates/bd-cli/src/agent_help_test.rs (`read_only_commands_write_no_ref_config_index_or_working_tree`)
- A commit that does not apply must print `state: "conflict"` (exit 1) with the first conflicting commit (a binary
  search over the prefixes of the range), the commits before it (`clean`), its files and `next`, what to do; nothing
  must move.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_conflict_names_the_first_conflicting_commit_and_moves_nothing`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_conflict_changes_nothing_and_says_what_to_do`)
- Local changes on a path that the pick or the follow writes must refuse it as `uncommitted_overlap` (exit 4) with
  the `paths`, the message naming the worktree; local changes on other paths must stay.
  [@test] ../../crates/git-ops/src/pick_test.rs (`local_changes_on_a_path_the_pick_writes_block_it_in_either_worktree`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`local_changes_in_the_way_and_a_wrong_source_are_refused`)
- The move of a check whose branch moved since must be `head_moved`, nothing moved; a HEAD other than `--expect-head`
  must be `head_moved` (exit 3).
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_target_that_moved_since_the_check_is_head_moved_with_nothing_changed`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`expect_head_refuses_a_pick_onto_another_head`)
- The source must be another worktree of the same repository; the target worktree itself, a directory outside the
  repository, a symmetric range, no source and `--follow` without `--from` must be `invalid_arguments` (exit 2).
  [@test] ../../crates/git-ops/src/pick_test.rs (`the_source_is_another_worktree_of_this_repository`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`local_changes_in_the_way_and_a_wrong_source_are_refused`)
- A pick while a `bd update` resolve session is open on the branch must land, and the finish of the session must carry
  the picked commits as a peer's.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_pick_during_an_open_update_session_is_carried_by_its_finish`)

### Remote commits

Someone else pushed to the remote branch of a virtual branch (a fix of a reviewer, a commit of a colleague): a push
would drop it. `bd pick --remote <branch>` takes it into the checked out branch instead.

- `bd pick --remote <branch>` must sync, fetch the remote branch of `<branch>` alone (before the lock) and pick the
  commits a push of it would drop (`commits_behind`, all of them: no older version of a commit the branch has, no
  landed one), oldest first, onto the checked out branch; the copies of those whose subject groups elsewhere get the
  `(<branch>)` prefix, so they land in `<branch>` and its push then drops nothing (no `--force`). The author stays.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_remote_pick_takes_in_the_commit_someone_pushed_and_the_push_then_drops_nothing`)
  [@test] ../../crates/git-ops/src/pick_test.rs (`listed_commits_land_alone_with_the_prefix_of_their_branch`)
- Only the listed commits are replayed, each run of them after its own parent, never the commits they were made on:
  a commit between two listed ones stays out.
  [@test] ../../crates/git-ops/src/pick_test.rs (`listed_commits_apart_replay_each_after_its_parent`)
- `--expect-remote <id>`: a fetched remote branch at another commit must pick nothing, `lease_rejected` (exit 4), as
  the lease of a push. A remote branch with no commit the branch lacks is `nothing_to_pick` (exit 4).
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_remote_pick_from_another_head_than_the_one_seen_picks_nothing`, `a_remote_pick_takes_in_the_commit_someone_pushed_and_the_push_then_drops_nothing`)
- Its processes are one sync, one `fetch`, the walks of the remote status, one `cat-file` and one `fast-import` of the
  prefixed copies besides those of a pick.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_remote_pick`)

### The lock

- The check (the range, the replay, the dry runs of both worktrees on copies of their index) must run before the
  repository lock, and another writer must get the lock meanwhile. Under it only the marks of interrupted moves, the
  witness (the target's branch, both HEADs, the upstream), the local changes on the paths the moves write (by their
  index entries and content, the exact dry run only where that finds one) and the two moves. A check that moves nothing
  (a conflict, local changes in the way, every commit present) must answer without the lock.
  [@test] ../../crates/bd-update/src/pick_test.rs (`the_check_of_a_pick_runs_before_the_lock`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`another_writer_takes_the_lock_while_a_pick_replays`)
- A pick must never wait for a sync: another agent's `bd sync` of the target, held in its merges, takes no repository
  lock, so the pick checks, moves both worktrees and ends meanwhile.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_pick_beside_a_held_sync_does_not_wait_for_it`)
- A target or a source that moved between the check and the lock must make the pick check again without the lock and
  land on the new tip, the source following there; after two such checks the third must run under the lock.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_commit_on_the_target_during_the_replay_of_a_pick_is_kept_and_the_pick_checks_again`)
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_target_that_moves_during_every_check_is_checked_under_the_lock_at_last`)
- A file a move writes, changed between the check and the lock, must refuse the pick as `uncommitted_overlap`, nothing
  moved.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_file_edited_during_the_replay_of_a_pick_blocks_it`)
- The index lock of another process in the target or the source (an IDE's `git status`) must make the pick wait,
  before the repository lock and in the move, at most 30 s per write, never roll it back; a lock that no process owns
  (left by a killed git) must be removed after two checks 2 s apart. A lock that stays must answer `locked`
  (retryable, exit 3) with the lock file, its age and the processes that may own it (`processes`), nothing changed;
  in the source it must make `followed.state` `blocked`, the target picked.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_pick_waits_for_the_index_lock_of_another_process_and_then_lands_with_its_files`, `a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)
  [@test] ../../crates/git-ops/src/index_lock_test.rs (`a_lock_that_a_process_has_open_stays_and_the_answer_names_the_process`, `a_lock_that_no_process_owns_is_removed_and_the_step_runs_again`)
  [@test] ../../crates/git-ops/src/index_lock_test.rs (`a_step_blocked_by_the_lock_of_another_process_runs_again_once_the_lock_is_gone`, `a_step_that_fails_on_its_own_returns_its_error_after_one_run_although_a_younger_lock_exists`)
  [@test] ../../crates/git-ops/src/index_lock_test.rs (`a_lock_file_that_took_the_place_of_the_checked_one_is_not_removed`)
  [@test] ../../crates/bd-update/src/error_code_test.rs (`bd_answers_the_code_of_each_refusal_of_a_move_an_unapply_and_a_reapply_that_the_app_answers`)

### `--follow`

- `--follow` must move the source worktree onto the new tip with its files, a detached HEAD staying detached
  (`option no-deref`, and under the locks HEAD must still be detached) and a branch moving the branch; its reflog must
  name the move `branch-deck: pick`. A rebase, merge, cherry-pick or revert in progress in the source must refuse it
  (`operation_in_progress`); without `--follow` the source is only read.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_pick_lands_the_commits_of_a_worktree_and_the_worktree_follows`, `a_source_on_a_branch_moves_its_branch_and_one_with_a_cherry_pick_in_progress_is_refused`)
- Both moves must be checked before either moves. A source that moved or switched after the check must leave it where
  it is while the branch moves: `followed.state` `head_moved` (or `blocked`), exit 0; the same `bd pick` again finds
  every commit present and moves only the source.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_source_that_switched_after_the_check_stays_and_the_same_pick_again_moves_it`, `a_source_that_switched_to_a_branch_at_another_commit_after_the_check_is_attached_and_stays`)
- A pick or a follow killed between the move of HEAD and its files must leave the mark
  `refs/branch-deck/update-pending/<worktree id>`; the next pick, update or undo must let the files follow first, a
  detached HEAD by the reflog of `HEAD`.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_pick_and_a_follow_killed_before_their_files_followed_are_finished`)

### Undo

- The undo token must be `pick:<previous head>..<new head>` with full ids, with `+follow:<worktree id>:<previous>..<new>`
  when the source moved. `bd undo` must check both moves, then move the branch and the source back with their files;
  `follow_undone` says whether the source moved back. Undoing again must be no error (`already_undone`), and the
  worktree's commits must be pickable again.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_pick_lands_the_worktree_commits_with_their_groups_and_the_undo_moves_both_back`)
  [@test] ../../crates/update-core/src/pick_test.rs (`an_undo_token_prints_and_parses_the_target_and_the_worktree_that_followed`)
- The last entry of each reflog must be that pick, else the undo is refused; a branch that moved on is `undo_stale`.
  [@test] ../../crates/git-ops/src/pick_test.rs (`the_undo_moves_the_target_and_the_detached_source_back`)
- The checks of both moves must run before the repository lock, which holds only the witness of each worktree (its
  HEAD and the mark of an interrupted move, read in its own directory) and the moves back; a remote pick fetches and
  checks before it as every pick. A worktree with such a mark before the check lets its files follow under the lock
  first.
  [@test] ../../crates/bd-update/src/pick_test.rs (`a_remote_pick_and_its_undo_lock_only_the_witness_and_the_move`)

### Error codes

| Exit | Codes |
|---|---|
| 1 | a commit conflicts (`state: "conflict"`): an output, not an error |
| 2 | `invalid_arguments` |
| 3 | `head_moved`, `locked` (retryable) |
| 4 | `range_unknown`, `source_on_newer_baseline`, `home_branch_switched`, `uncommitted_overlap`, `operation_in_progress`, `detached_head`, `finish_pending`, `undo_stale`; with --remote `lease_rejected`, `nothing_to_pick` |

- The outputs must match `bd schema` (`pick`, `undo`).
  [@test] ../../crates/bd-update/src/pick_test.rs (`pick_outputs_match_the_schema`)
  [@test] ../../crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`)
- A pick must run a fixed number of git processes: one `replay`, the witness, the two moves with their marks, one read
  of the grouping after them (the inputs and one `log` of the commits ahead); the diffs of the dependency check only
  for a prefix that names a branch the branch has.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_pick`)

## Testing / Local Run

- `cargo nextest run -p git-ops pick_test` runs the operation on `TestRepo` fixtures with a linked worktree;
  `cargo nextest run -p bd-cli pick_test` runs the command end to end.

## Open Questions / Risks

- The dependency check sees lines only, not symbols: a picked commit that calls a function another branch adds in
  another file is not named; `bd check` names it after the pick (`missing_declarations`).

- A source directly on the branch's HEAD, picked in the same second as its commits were made, replays into the same
  commits: the branch fast-forwards to them and the source does not move (`at_tip`).
- The range of revisions (`<a>..<b>`) excludes the branch's history but not by the reflog: it names exactly the
  commits to pick.
- The patch ids are compared with the branch's commits above the base and the upstream only (`^<upstream>` limits
  both sides of the range): a commit that already reached the upstream in another form is not found by its patch id.
  It drops as empty at the replay (`by: "empty"`) or conflicts.

## References

- [Git Operations](../../docs/design/git-operations.md): "Pick" and "Update project".
- [Agent CLI guide](../../docs/contributing/agent-cli.md): `bd pick`.
