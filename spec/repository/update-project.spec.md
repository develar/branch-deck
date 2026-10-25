---
name: Update Project
description: Requirements for Update Project, which fetches the baseline and rebases the checked out branch onto it without touching unrelated local changes, the header state of the update, and the hand-off of a conflict to a coding agent.
style: plain-1
targets:
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/git-ops/src/paths.rs
  - ../../crates/git-ops/src/replay.rs
  - ../../crates/git-ops/src/revs.rs
  - ../../crates/git-ops/src/worktree_update.rs
  - ../../crates/update-core/src/update_project.rs
  - ../../crates/app-api/src/api_error.rs
  - ../../crates/sync-core/src/baseline.rs
  - ../../crates/sync-core/src/transfer_progress.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/sync-core/src/sync.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-executor/src/worktree_safety_test.rs
  - ../../src-tauri/src/commands/update_project.rs
  - ../../src-tauri/src/menu.rs
  - ../../src/components/repository/UpdateProjectButton.tsx
  - ../../src/components/repository/UpdateBlockedDialog.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/SyncButton.tsx
  - ../../src/components/repository/RepositorySelector.tsx
  - ../../src-tauri/src/lib.rs
  - ../../src/components/shared/RelativeTime.tsx
  - ../../src/hooks/useNow.ts
  - ../../src/stores/update-project.ts
  - ../../src/lib/errors.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/update-state.ts
  - ../../src/lib/update-progress.ts
  - ../../src/hooks/useRunProgress.ts
  - ../../src/components/shared/ProgressLine.tsx
  - ../../src/components/repository/DockStatus.tsx
  - ../../src/lib/transfer-progress.ts
  - ../../src/lib/agent-prompt.ts
  - ../../src-tauri/src/commands/agent_cli.rs
  - ../../src/lib/update-conflict.ts
  - ../../src/lib/text.ts
  - ../../crates/sync-utils/src/list_cut.rs
---

# Update Project

Status: Implemented
Date: 2026-10-15

## Summary

Update Project (`⌘T`, as in IntelliJ) fetches the baseline (e.g. `origin/master`) and brings the checked out branch
onto it: a fast-forward without local commits, otherwise a rebase of the local commits. It works with uncommitted and
staged changes as long as the upstream doesn't change those files. A conflict or an overlap changes nothing; a dialog
explains it and copies a ready prompt for a coding agent. With the app's bd, the agent resolves a conflict in a resolve
session of `bd update`, outside the working tree ([bd update](../cli/update.spec.md)).

The header shows whether an update would apply cleanly, from a check that every sync runs against the last fetch.

## Goals

- Update without leaving the app, and without stashing local work that the update doesn't touch.
- Know before updating whether it conflicts.
- Never leave a half-done rebase: either the update applies as a whole, or nothing changes.

## Non-goals

- No count of incoming commits anywhere: a monorepo gets hundreds, and only conflicts matter. The state is up to date,
  applies cleanly, conflicts in N files, or blocked by uncommitted changes. The user's own commits that the baseline
  already has are named, not counted as incoming ([Landed Commits](landed-commits.spec.md)).
- The app does not resolve conflicts and does not launch an agent; it copies a prompt. The agent resolves a conflict
  with `bd update` and its resolve session, and a blocked update goes to the user, then to `bd update`; neither
  stashes. Without a bd command, a conflict gets the git steps, which never stash either.
- No merge of the upstream (`pull --no-rebase`), no update of submodules.

## Requirements

### Check

- The check must not use the network: it compares the branch with the tracking ref of the last fetch.
- It must refuse (an error, not a state) while a rebase, merge, cherry-pick or revert is in progress, on a detached HEAD,
  and when the local commits contain a merge.
  [@test] ../../crates/update-core/src/update_project_test.rs (`check_refuses_while_a_rebase_is_in_progress`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`merges_in_the_local_commits_are_refused`)
- A local baseline has nothing to update from: no check, and an update says so.
  [@test] ../../crates/update-core/src/update_project_test.rs (`local_baseline_has_nothing_to_update`)
- A branch and an upstream without a common history must be refused as such; a history that git can't read (an
  object that is not there, a tracking ref that is gone) must be an error with git's message, never "no common
  history" or "no fork point".
  [@test] ../../crates/git-ops/src/update_branch_test.rs (`unrelated_histories_have_no_common_history`, `a_history_git_cannot_read_is_an_error_with_its_message_not_unrelated_histories`, `the_fork_point_of_a_tracking_ref_that_is_gone_is_an_error`)
- The state must be one of:
  - up to date: the upstream has nothing the branch lacks;
  - clean, as a fast-forward or a rebase;
  - conflict, with the files that likely conflict (the first 50, the ones with their commits, and the count of all),
    the local commits oldest first, and the upstream commits that change those files (the newest 50, oldest first);
    each commit has its author and author time, its committer and committer time (for an upstream commit, about when
    it reached the upstream, e.g. when a robot landed it) and the likely conflicting files it changes;
  - blocked, with the local paths that overlap the update (the first 100, and the count of all): uncommitted or
    staged changes on a file the upstream changes, an untracked file where it adds one, an ignored file where it adds
    one.
  [@test] ../../crates/update-core/src/update_project_test.rs (`same_line_conflict_reports_files_and_commits_and_changes_nothing`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`dirty_touched_path_blocks_and_changes_nothing`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`staged_touched_path_blocks`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`untracked_file_that_upstream_adds_blocks`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`ignored_file_that_upstream_adds_blocks`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`wide_conflict_reports_the_first_files_with_their_commits_and_counts_all`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`many_blocking_paths_are_counted`)
- A file that was only touched (same content, new modification time) must not block.
  [@test] ../../crates/update-core/src/update_project_test.rs (`touched_only_file_passes_thanks_to_the_refresh`)
- A path that is not UTF-8 must be checked by its bytes, its index entry and its file; the state shows it with U+FFFD
  for the bytes that are not UTF-8.
  [@test] ../../crates/git-ops/src/update_branch_test.rs (`a_path_that_is_not_utf8_names_its_index_entry`)
- Commits that an upstream force-push removed must not be replayed: the old tip of a forced fetch and the fork point of
  the tracking ref's reflog are excluded, as `git pull --rebase` does.
  [@test] ../../crates/update-core/src/update_project_test.rs (`upstream_force_push_does_not_replay_the_removed_commits`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`check_after_a_forced_fetch_elsewhere_uses_the_fork_point`)
- Every sync must run the check and report it once, beside the rest of the sync (the event can come at any point);
  a failed check is reported with its reason and must not fail the sync. A sync deferred for a detached HEAD or an
  operation in progress (`SyncDeferred`, [Onboarding](onboarding.spec.md)) runs no check and reports nothing.
  [@test] ../../crates/update-core/src/update_project_test.rs (`sync_reports_the_baseline_status`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`sync_on_a_detached_head_is_deferred_before_the_update_check`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`test_branches_grouped_precedes_per_branch_events`)
- Repeated checks of the same state must not write new objects; a check made for a sync can't be applied.
  [@test] ../../crates/update-core/src/update_project_test.rs (`preview_check_cannot_be_applied`)
- The check inside a sync must cost in proportion to the paths the update changes, not the size of the index: it
  never copies, refreshes or writes the index, reuses the replay of an unchanged HEAD and upstream, and rereads only
  files whose stat data changed. It must still find dirty and staged touched paths, must not block on a file that was
  only touched, and must block on untracked and ignored files where the upstream adds one.
  [@test] ../../crates/update-core/src/update_project_test.rs (`preview_blocks_dirty_and_staged_touched_paths_without_touching_the_index`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`preview_does_not_block_a_touched_only_file`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`preview_blocks_untracked_and_ignored_files_that_upstream_adds`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`preview_reuses_the_replay_of_an_unchanged_state`)
- A conflict found once must not be replayed again for the same HEAD, upstream and previous tips: the update after the
  sync that found it, and the sync after a conflicting update, reuse it (a conflict doesn't depend on the committer
  date; a clean outcome is never shared).
  [@test] ../../crates/update-core/src/update_project_test.rs (`update_after_the_sync_found_a_conflict_does_not_replay_again`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`sync_after_a_conflicting_update_does_not_replay_again`)
- With `branchdeck.resolveByWords` on (the default), a replay that conflicts must be tried again commit by commit, each
  conflict of git resolved by words as the sync resolves it ([Git Operations](../../docs/design/git-operations.md),
  "Resolve by words"). All or nothing: the state is a clean rebase only when every conflicting commit resolves, and
  it lists them (`resolvedByWords`: the commit, its subject, the conflicts and the files). The replay of the sync's
  check and of the update are cached apart by the mode.
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)
  [@test] ../../crates/git-ops/src/word_replay_test.rs (`a_conflict_that_resolves_by_words_is_committed_and_the_rest_replays_onto_it`, `a_commit_whose_words_conflict_too_stops_the_replay`, `a_commit_that_the_words_leave_without_a_change_is_dropped`)
- A file that the upstream replaces by a directory (or the reverse) must not block.
  [@test] ../../crates/update-core/src/update_project_test.rs (`file_replaced_by_a_directory_upstream_is_not_in_the_way`)

### Update

- The update must fetch only the baseline's remote branch, so the push lease of the virtual branches still compares
  against what the last push saw, then check again and apply a clean result. A failed fetch changes nothing.
  [@test] ../../crates/update-core/src/update_project_test.rs (`parses_the_forced_update_line_of_fetch`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`a_failed_fetch_is_fetch_failed`)
- A fetch beside another fetch of the same tracking ref (`bd update` in a lease while the app updates or fetches: one
  common dir) must not fail on the other's ref lock: a fetch that git refuses with `cannot lock ref` is run once more,
  any other failure is not.
  [@test] ../../crates/sync-core/src/baseline_fetch_test.rs (`a_fetch_beside_another_fetch_of_the_tracking_ref_succeeds`, `a_fetch_that_fails_otherwise_is_not_repeated`)
- After the fetch, the update must check (the replay, the resolution by words, the landed candidates) without the
  repository lock, on the HEAD and the upstream it read once, then take the lock only for the witness, the local
  changes on the paths it writes and the move, so the app, `bd update` and the rewrites of either never interleave
  with the move, while a regroup, a push or an agent's `bd` goes on during the check; the fetch, the counts and notes
  of what the update did and the record of what landed never run under the lock. A late record of an update that
  another update already moved past only adds what it landed: the state keeps naming the later update, whose undo
  restores the report from before it. A check that moves nothing (up to date, or a conflict, which starts no session in
  the app) is the answer at once, without waiting for the lock, unless an interrupted update left files to follow.
  A branch or an upstream moved during the check must be checked once more, under
  the lock, and a file the update writes, edited during the check, must block it. A lock held for the whole wait
  (10 s in the app) is an error with only the fetch and the check done, `locked` with the holder its record names
  (`details.holder`: `bd` or the app, its command, pid and how long it has held the lock; none once the record is
  gone): the app shows "Cannot update the project" with "`bd push auth` (pid 4242) has held the repository for 40 s"
  (the app's own holder as "Branch Deck (pid 7) …"; "Another Branch Deck operation holds the repository" without a
  record; never the lock file path), worded by the page from the code and its data, and keeps the time of the fetch.
  [@test] ../../crates/update-core/src/update_project_test.rs (`update_takes_the_repository_lock_after_the_fetch_and_says_it_waits`, `an_update_that_moves_nothing_answers_without_waiting_for_the_lock`)
  [@test] ../../crates/app-api/src/api_error_test.rs (`api_error_names_the_holder_of_a_lock_with_how_long_it_has_held_it`)
  [@test] ../../src/lib/errors.test.ts (`names the holder of a held repository lock and how long it has held it, as the lock record says it`, `says another operation holds the repository when the holder left no record, never the lock file`)
  [@test] ../../src/lib/errors.test.ts (`toasts a held repository lock with its holder, without the holder the other operation, and a plain Error with its message`)
  [@test] ../../crates/update-core/src/landed_test.rs (`a_late_record_of_an_update_keeps_the_record_of_the_update_after_it`)
  [@test] ../../crates/bd-update/src/update_test.rs (`the_update_waits_for_the_repository_lock_after_its_fetch`, `the_check_of_an_update_runs_before_the_lock`, `a_regroup_during_the_replay_of_an_update_is_done_and_the_update_checks_again`, `a_file_edited_during_the_replay_of_an_update_blocks_it`)
  [@test] ../../src/stores/update-project.test.ts (`keeps the fetch time when the update fails after its fetch, e.g. while bd holds the repository`)
- An update refused after its fetch by an error that the same request may pass (`retry: same`, a held lock) offers
  "Retry Without Fetch" on its toast: the check and the apply against the baseline that fetch got (`update_project`
  with `fetch: false`, no second `fetch_baseline`), as an update joined by a second start, and only while the main
  window shows the repository. The retry refused by the same holder updates that toast in place (an error toast is
  one per title and backend message, which says no time). An error to read again
  (`retry: after_reread`, the branch moved) offers no retry: a new Update reads it again.
  [@test] ../../src/stores/update-project.test.ts (`offers Retry Without Fetch for a held lock after the fetch, which checks and applies without a second fetch; a moved branch gets none`, `retries without a fetch only the repository the main window shows`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should name the holder of the repository lock and retry the update without a fetch`, `should offer no retry when the branch moved under the update`)
- `bd update --no-fetch` must update onto the last fetched baseline ([bd update](../cli/update.spec.md)).
  [@test] ../../crates/update-core/src/update_project_test.rs (`update_without_a_fetch_goes_onto_the_last_fetched_state`)
- A fast-forward must move the branch and update the files.
  [@test] ../../crates/update-core/src/update_project_test.rs (`fast_forward_updates_the_working_tree`)
- A rebase must keep the author of every local commit; the committer is the current user.
  [@test] ../../crates/update-core/src/update_project_test.rs (`rebase_keeps_the_author_and_makes_the_current_user_the_committer`)
- Uncommitted, staged and untracked changes on paths the update doesn't change must survive byte for byte, the staged
  version staged.
  [@test] ../../crates/update-core/src/update_project_test.rs (`dirty_and_staged_files_on_untouched_paths_survive_byte_identical`)
- A rebase that resolved conflicts of git by words must keep the author and the message of each resolved commit, and
  write a note per resolved commit on `refs/notes/branch-deck/resolve`. Its toast and its Activity entry must end with
  "Resolved 1 conflict by words in “<subject>”" (or "… in 2 commits"); the tooltip and the footer of the check say
  "Rebases onto origin/master, resolving 1 conflict by words". Undo takes it back as any update.
  [@test] ../../crates/git-ops/src/update_resolve_test.rs (`an_update_whose_every_conflict_resolves_by_words_applies_without_a_session`)
  [@test] ../../src/lib/landed.test.ts (`ends with the conflicts of git that the update resolved by words`)
  [@test] ../../src/lib/update-state.test.ts (`says that a rebase resolves conflicts of git by words: it is not clean to git`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should rebase an update whose conflict of git resolves by words, and say so`)
- A local commit whose changes are already upstream is dropped and reported, not an error. When the replay conflicts
  because a robot folded fixups into their target, the landed commits are left out if nothing local is lost
  ([Landed Commits](landed-commits.spec.md)).
  [@test] ../../crates/update-core/src/update_project_test.rs (`commit_already_upstream_is_dropped_and_reported`)
- Conflict and blocked are results: the branch, the index, the working tree and the reflog stay as they were.
- Up to date must write no reflog entry.
  [@test] ../../crates/update-core/src/update_project_test.rs (`up_to_date_writes_no_reflog_entry`)
- The branch moves with a CAS ref transaction (`branch-deck: update project`) before the files; if the files can't
  follow (one changed since the check), the branch moves back (`branch-deck: update project rolled back`) and the update
  reports that nothing was changed. The transaction updates HEAD and commits only while git holds the lock of HEAD,
  after a check that HEAD is still on the checked branch.
  [@test] ../../crates/update-core/src/update_project_test.rs (`rolls_back_when_a_file_changes_between_the_check_and_the_update`)
- The index lock of another process must make the move wait (at most 30 s per write) and an ownerless one be removed,
  as for `bd pick`; one that stays must move the branch back and answer `locked`, nothing changed.
  [@test] ../../crates/git-ops/src/pick_test.rs (`a_move_removes_an_index_lock_that_no_process_owns_and_lets_the_files_follow`)
- HEAD switched to another branch since the check, even one at the same commit, and also right before the move, must
  refuse the move (and its undo) with nothing changed.
  [@test] ../../crates/update-core/src/update_project_test.rs (`a_switch_to_another_branch_at_the_same_commit_since_the_check_is_refused`)
  [@test] ../../crates/bd-update/src/update_test.rs (`a_switch_between_the_check_and_the_move_moves_no_branch`)
- A `git switch` while the move holds the lock of HEAD must fail on `HEAD.lock`; the move commits on the checked
  branch.
  [@test] ../../crates/bd-update/src/update_test.rs (`a_switch_during_the_move_fails_on_the_head_lock`)
- The working tree is written by `crates/git-ops/src/worktree_update.rs` only, called only from `update_branch.rs`
  (the update, the undo that only `check_update_undo` builds, and the finish of a resolve session, whose tip
  `update_resolve.rs` reads from the session ref); the safety scan fails on any other `read-tree -u`, `--reset` or
  `checkout-index`, and on a change of the number of exempt argument lists.
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`scanner_finds_read_tree_updates`)

### Undo

- `bd undo update:<previous head>..<new head>` must move the branch back and let the files follow through the same
  guarded two-way update with the trees swapped, under the repository lock (`branch-deck: undo update`); local changes
  on the other paths stay.
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_moves_the_branch_and_the_files_back`)
- It must refuse, changing nothing, when local changes sit on a path it writes, an ignored file where a file that the
  update deleted comes back included, and when the branch moved since the update.
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_is_refused_when_local_changes_overlap`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_is_refused_after_the_branch_moved`)
- After a process was killed between the move and the files (HEAD at the new head, the files at the previous one),
  the undo must succeed and be the recovery.
  [@test] ../../crates/update-core/src/update_project_test.rs (`the_undo_recovers_an_update_killed_before_the_files_followed`)
- The move must create the mark `refs/branch-deck/update-pending/<worktree id>` in its ref transaction, and delete it
  once the files followed. With the mark left by a killed process, the next update or undo, the app's included, must
  let the files follow first, or change nothing with `finish_pending` when the branch moved since.
  [@test] ../../crates/bd-update/src/update_test.rs (`an_update_killed_before_its_files_followed_is_finished_by_the_next_update`, `an_undo_killed_before_its_files_followed_is_finished_by_the_next_undo`, `an_interrupted_update_of_a_branch_that_moved_since_is_reported`)
- An undo of an undone update is no error and changes nothing (`already_undone`).
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_moves_the_branch_and_the_files_back`)
- Only the last update of the branch can be undone: the last reflog entry must be that update, so a token can't move
  the branch to another commit. A token takes full ids only. The update writes that reflog entry also with
  `core.logAllRefUpdates=false`.
  [@test] ../../crates/update-core/src/update_project_test.rs (`the_undo_works_without_reflogs_because_the_update_writes_one`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_needs_the_token_of_the_last_update`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`undo_tokens_are_strict`)
- The landed report must be the one from before the update again ([Landed Commits](landed-commits.spec.md)).
  [@test] ../../crates/update-core/src/landed_test.rs (`undo_of_an_update_restores_the_report_from_before_it`)

### Header

- The Update button sits in the header before the landing indicator: incoming and the way into master together, the time of the
  last sync next to Sync. It is hidden for a local baseline and before the first sync of the repository, which runs
  when the window opens a repository synced before ([Onboarding](onboarding.spec.md)).
- The header must stay one line down to the minimum window width, 960 px (`min_inner_size`), its controls never
  overlapping: the repository selector gives way first (its path clipped from the start), the branch count shows from
  1024 px while the filter bar is hidden (from 8 branches on its "All 35" counts them), the time of the last sync from 768 px; Update, the landing indicator, Activity ([Activity](activity.spec.md)),
  Sync and its settings never shrink.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)
- While a sync runs, Sync shows its spinner, the one indicator of the sync, and its tooltip names the step,
  "Syncing branches…", without the shortcut, which does nothing then; the time of the last sync stays as it is, so
  nothing in the header moves. Before the first sync there is no time.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should not move while a sync runs`)
- One sync runs at a time in the session of a repository, whichever window or command asks for it. A resync after a
  change, its Undo or a fetch asked while one runs share one quiet sync after it, which reads what changed meanwhile; a
  focus or ⌘R while one runs checks once after it and syncs only if what a sync reads changed. A request resolves at
  the end of the run that serves it, never of the follow-ups after it; Sync keeps its spinner from the request until
  the last patch of that run reached the window, and a push pressed meanwhile waits for it. The events of a run that
  ended are dropped, and a sync records its time on its own repository.
  [@test] ../../crates/repository-model/src/session_test.rs (`a_ticket_resolves_when_its_run_ends_not_the_chain`, `the_late_events_of_an_ended_run_are_dropped`, `a_focus_during_a_run_is_one_check_after_it`)
  [@test] ../../crates/repository-model/src/sync_scheduler_test.rs (`a_ticket_resolves_when_the_run_that_serves_it_ends`, `a_focus_during_a_run_checks_the_inputs_once_after_the_chain`)
  [@test] ../../src/model/store.test.ts (`keeps Sync busy from the request to the last patch of the run that serves it, and counts the runs that ended`, `leaves a request of the session the window left to it: Sync of the next one is not busy, nothing of it is recorded`)
- Sync has the tooltip "Sync branches ⌘R" (`Ctrl+R` off macOS) and its settings (auto-sync on focus) next to it; the
  menu item and ⌘R do what the button does, nothing while it is disabled for a reason. A
  failed sync turns it `destructive` with the error in its tooltip; with the page alert, the error shows twice at most.
  The repository selector has a tooltip only without a repository ("Select a Git repository to manage branches") or
  for its error.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should name the shortcut of Sync and keep the selected repository without a tooltip`)
- It shows the state, never a count:
  - up to date: muted, no state icon, tooltip "Up to date with origin/master";
  - clean: a `success` check, tooltip "Rebases cleanly onto origin/master" ("Fast-forwards to origin/master");
  - conflict: a `warning` icon, tooltip "Conflicts in N files: …" (at most three names);
  - blocked: a `warning` icon, tooltip "Your uncommitted changes in … overlap origin/master";
  - a failed check: muted with a question mark, tooltip "Cannot check the update: <reason>".
  [@test] ../../src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)
  [@test] ../../src/lib/update-state.test.ts (`names at most three files, four of four: never 'and 1 more'`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)
- The tooltip must say when the baseline was last fetched: Branch Deck's own fetch or one elsewhere (`FETCH_HEAD`),
  whichever is newer, and the shortcut: "Fetched 5 minutes ago · fetch and update ⌘T".
  [@test] ../../src/lib/update-state.test.ts (`takes the newer of Branch Deck's fetch and one elsewhere`)
- Before the first fetch, a state would claim what nobody asked the remote: the button is muted without a state icon,
  its tooltip says "origin/master not fetched yet" over "fetch and update ⌘T", and the footer of the landing overview
  "Incoming from origin/master: not fetched yet". A conflict or a blocked update, found against the copy of the clone,
  keeps its words and its icon, then "· origin/master not fetched yet".
  [@test] ../../src/lib/update-state.test.ts (`says the baseline was not fetched yet instead of a state nobody asked the remote for`)
  [@test] ../../src/lib/update-state.test.ts (`says the same in the footer of the landing overview`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)
- A relative time must age while it is shown, without a sync or a fetch: "Last synced 1 minute ago" next to the button,
  the fetch time of the tooltip and of the footer line, "Pull requests checked" and "Patronus checked" in the overview,
  the times of the Landed island and chips, and of the commits a force push drops. One interval per interval length
  renews them every 15 s (`useNow`), so "1 minute ago" is at most a tick late; only the text re-renders.
  [@test] ../../src/hooks/useNow.test.ts (`ticks every listener of an interval length at once, with one interval that stops with the last listener`)
  [@test] ../../src/hooks/useNow.test.ts (`ages a relative time without anything else changing: "Last synced just now" becomes "1 minute ago"`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should age the last sync next to the Update button while it is shown`)
- A click, or File → Update Project (`⌘T`), runs the update. Its steps are one progress from the click to its end:
  the fetch, the check, the wait for the repository lock (only while another operation holds it and the check found
  something to apply), and, for an update that applies, the update of the files and the sync that rebuilds the rebased branches. Meanwhile the button shows a
  spinner and one word for the whole run with its percent, "Updating 62%" ("Updating…" below 1 %), never a word per
  step; the Fetch of a merge toast says "Fetching 30%". The label keeps the width of the widest one, "Updating 99%",
  from the start of the run to its end, so the header doesn't move per step or tick.
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should run from the Update Project menu item`)
  [@test] ../../src/lib/update-state.test.ts (`names the whole run with one word and its percent, the toast's Fetch as a fetch`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart while an update runs`)
- While another process holds the repository lock (an agent's `bd pick --follow`, `bd`) or the app's own push, the
  update must wait after its fetch and its check as long as the holder lives, never refused after 10 s: its tooltip
  step says whom for ("Waiting for `bd pick` (pid 4242, for 12 s)…"), and Stop Waiting beside the Update button ends
  the wait with nothing changed but the fetch ("Canceled the update before it started"). Only a holder no record
  names refuses it after 10 s; `bd update` waits 10 s, then `locked`.
  [@test] ../../src/lib/update-state.test.ts (`names whom an update waits for while another holder has the repository lock, with Stop Waiting, until its next step`)
- A bar of 2 px along the bottom edge of the button shows the progress, a sibling of the button (`progressbar`,
  `aria-valuenow`, `aria-valuetext` with the step, "Checking the update onto origin/master… 62%"); while the run is on,
  the name of the button is "Update Project: updating", without the percent. Once the run ends the button works again
  at once and the bar shows 100 % for 300 ms. While it runs in the main window, the Dock (the taskbar on Windows) shows
  the progress of the update instead of the robots' ([Robots in the Dock](robots-indicator.spec.md#dock)).
  [@test] ../../src/stores/update-project.test.ts (`is busy through the sync of an applied update, its own run of it; the toast doesn't wait for the sync`)
- The share of each step is its usual time in the repository: the median of its last 5 durations (fetch, check,
  update of the files; the sync learns from every sync of the repository, the wait for the lock never), kept in the
  state of the repository (`repositoryState`, beside the settings, never in `settings.json`), 3 s, 1.5 s,
  0.8 s and 2 s before the first. Inside a step the progress is the fetch's percent of git, or the synced commits of
  the update's own sync run, when it is ahead of the time; else the time, `1 − e^(−t/usual)`: 63 % of the step at its
  usual time, never still. A step reaches at most 95 % of its share before it ends, and at each change of step the
  rest of the bar is shared again among the steps left, from where it stands: the bar never jumps nor goes back. While
  the update waits for the lock the bar stands still. The update shares the bar among all its steps from its start,
  also when the last check found the branch up to date; a check that finds nothing to apply ends the run with its
  result, and the bar shows 100 %.
  [@test] ../../src/lib/update-progress.test.ts (`shares the bar by the usual time of each step in the repository: the median of the last five`)
  [@test] ../../src/lib/update-progress.test.ts (`moves with the time in a step that reports nothing, never standing still, and keeps the end of a step for its end`)
  [@test] ../../src/lib/update-progress.test.ts (`takes git's percent in the fetch when it is ahead of the time`)
  [@test] ../../src/lib/update-progress.test.ts (`goes on from where it stands at each change of step: no jump after a slow fetch, no stall after a fast one`)
  [@test] ../../src/lib/update-progress.test.ts (`stands still while it waits for the lock`)
  [@test] ../../src/stores/update-project.test.ts (`lists every step of an update from its start, also when the last check found the branch up to date`)
  [@test] ../../src/lib/update-progress.test.ts (`takes the commits of its own sync, capped until the sync ends`)
  [@test] ../../src/lib/update-progress.test.ts (`learns what the steps took, without the wait for the lock and the sync, which learns from every sync`)
  [@test] ../../src/stores/update-project.test.ts (`changes the step only when the phase changes, and records what the steps took in the state of the repository once the update ends`)
  [@test] ../../crates/repository-model/src/session_test.rs (`a_ticket_resolves_when_its_run_ends_not_the_chain`)
- While the update runs, its tooltip lists its steps instead of the state of the last check and "fetch and update":
  those done ("Fetched origin/master · 5.79 MiB", "Checked the update onto origin/master"), the running one ("Fetching
  origin/master…", or git's last progress line once git reports a stage; "Waiting for Branch Deck or bd to finish…",
  "Checking the update onto origin/master…", "Updating to origin/master…", "Syncing branches…") and those left,
  muted ("Check the update", "Update to origin/master", "Sync branches"). Every step is listed from the start of the
  run, the update of the files and the sync too, so one look shows the whole run: a check that finds nothing to apply
  ends the run before them. A wait that ended is no step of its own.
  [@test] ../../src/lib/update-state.test.ts (`names the running step in the tooltip, never the last state`)
  [@test] ../../src/lib/update-state.test.ts (`lists the steps done, the running one and those left, without a wait that ended`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should end the progress at the result when there is nothing to apply, its sync on Sync`)
- The backend tells the wait for the lock as a step of its own (`waiting`), once, when another operation holds the
  lock as the update asks for it, after the check (`checking`), which runs before the lock. When the branch or the
  upstream moved meanwhile, the check once more under the lock is `checking` again, so the step no longer names
  another operation. A progress message after the command answered changes nothing: the steps after it are the
  frontend's.
  [@test] ../../crates/update-core/src/update_project_test.rs (`update_takes_the_repository_lock_after_the_fetch_and_says_it_waits`, `the_check_again_after_the_wait_for_the_lock_is_the_check_step`)
  [@test] ../../src/stores/update-project.test.ts (`ignores a progress message that arrives after the command answered`)
- The step and the fetch progress belong to the repository the update (or the Fetch of a merge toast) runs in: after a
  switch, the buttons show the selected repository's, never another one's. One update or fetch runs at a time in a
  repository, and none is dropped: a second Update (⌘T, a toast) joins the running update, Update during the Fetch of
  a toast goes on from that fetch with the check and the apply (no second fetch), and a Fetch during an update joins
  it; another repository can update meanwhile. An update in a repository that is no longer selected doesn't sync the
  selected one.
  [@test] ../../src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)
  [@test] ../../src/stores/update-project.test.ts (`syncs only the selected repository: another one selected during the update is not synced for it`)
  [@test] ../../src/stores/update-project.test.ts (`runs one at a time in a repository: a second start joins it, and an update during a Fetch goes on from that fetch`)
- The fetch of an update holds nothing of the page: it runs before the update's operation, so a move, a park, an
  Unstack, a reword or an Undo during the fetch runs at once. Only the check, the apply and the sync of the update are
  a rewrite of the repository ([Running Operations](../../docs/design/operations.md)): while a regroup, a squash or
  another rewrite runs, Update stays enabled with the busy state it would wait for on hover ("Unstacking oauth…"); a
  start fetches, then its check waits for that rewrite in the queue (the step "waiting"), never refused; while the
  check and the apply run, a rewrite of a menu waits for them in the queue ("Waits for Updating the project").
  [@test] ../../src/stores/update-project.test.ts (`fetches outside the slot of the rewrites, which run meanwhile, then waits for a running rewrite in the queue for its check and apply, and the rewrites wait for those`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should let a rewrite run during the fetch of an update, which checks and applies after it`)
- Once git reports a stage of the fetch, its percent of the whole fetch moves the progress of the update, and the
  tooltip shows the last progress line of git, as IDEA shows a fetch. The percent weighs the stages as IDEA's
  `GitStandardProgressAnalyzer` does (counting 5 %, compressing 10 %, receiving or unpacking 80 %, resolving deltas
  5 %), never decreases, and reaches the window at most every 100 ms; the end of a stage always does. A push reports
  its progress the same way ([Outgoing](outgoing.spec.md)).
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`stages_have_the_shares_of_idea`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`small_fetch_unpacks_instead_of_receiving`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`percent_never_decreases`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`reporter_sends_the_first_progress_and_every_stage_end`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`fetch_reports_progress_until_the_last_stage_ends`)
- The progress is read from `--progress` lines on stderr in the C locale (git has no machine-readable progress), best
  effort: a line it doesn't know changes nothing, and nothing but the progress depends on it. The error of a failed fetch
  keeps git's messages without the progress updates.
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`other_lines_change_nothing`)
  [@test] ../../crates/sync-core/src/remote_command_test.rs (`remote_command_runs_untranslated`)
  [@test] ../../crates/git-executor/src/git_command_executor_test.rs (`failed_command_reports_messages_without_progress_updates`)
- After an applied update a toast says "Updated master: rebased 2 commits onto origin/master" or "…: fast-forwarded to
  origin/master". When commits landed, the toast leads with them instead ([Landed Commits](landed-commits.spec.md)).
  Up to date says "master is already up to date with origin/master". Either takes the place of the open toasts of the
  merges of the repository, whose Update Project it did, and every surface of a running update shows it there: the
  Update Project of such a toast says "Updating…" while it runs
  ([Patronus](../branch/patronus.spec.md#merged), [One toast per event](../../docs/guidelines/ux-writing.md#one-toast-per-event)).
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show a clean update, apply it and then be up to date`)
  [@test] ../../src/stores/update-project.test.ts (`takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date`)
  [@test] ../../tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)
- A sync follows every update whose fetch finished, without expanding or scrolling: applied, up to date, conflicting,
  blocked, or failed after the fetch. The fetch moved the baseline, so integration, the remote status and what landed
  are built against it although the branch itself didn't change. The sync of an applied update is the last step of
  its progress: the Update button stays busy until that sync run ends (a ⌘R meanwhile asks for a follow-up that is
  Sync's only), and Sync shows its spinner as for any sync. Up to date, refused or failed, the update ends with its
  result, and its sync shows on Sync alone. The toast and the dialog come with the result, never after the sync: they
  say what happened, the sync only rebuilds the rows.
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should end the progress at the result when there is nothing to apply, its sync on Sync`)
  [@test] ../../src/stores/update-project.test.ts (`syncs after every update, also an update that was up to date, conflicted or was blocked`)
  [@test] ../../src/stores/update-project.test.ts (`keeps the fetch time when the update fails after its fetch, e.g. while bd holds the repository`)
- The footer of the landing overview must show the same state in one line, with the state icon of the button, named
  as incoming and with the fetch time, "Incoming from origin/master: rebases cleanly · fetched 5 minutes ago" (up to
  date: "nothing new"), with an Update button. Below the rows of the work on its way, a bare "origin/master: up to date"
  reads as a state of that work. While the update runs, the text names its step in lower case, "Incoming from origin/master: checking the
  update…", and while the fetch receives it has the amount and the rate as git prints them, "Incoming from
  origin/master: 5.79 MiB · 32.57 MiB/s"; its button has the label of the header button, without a second bar (the
  header shows above the overview), and keeps its full colour while the update runs, as the one in the header.
  [@test] ../../src/lib/transfer-progress.test.ts (`shows the amount and the rate as git prints them`)
  [@test] ../../src/lib/update-state.test.ts (`says the step in the footer, the amount and the rate while the fetch receives`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the landing overview`)

### Conflict and blocked dialog

- A conflict or a block after a click opens a dialog in the frame of the previews: it says nothing was changed and
  when the baseline was fetched. It reads the app's bd command in the repository when it opens (`agent_cli_command`),
  so the copy needs no wait; Copy waits until it is read.
- The conflict title counts all files, "1 file conflicts with origin/master" or "6 files conflict with origin/master";
  without known files it says "Your commits conflict with origin/master".
- A conflict lists the files that likely conflict, each with the local commits ("yours", with their author time) and
  the upstream commits that change it. An upstream commit shows its author and both times when they differ by more
  than an hour, "Daniil Bubnov · authored 13 days ago · landed 2 hours ago", else one, "Daniil Bubnov, 13 days ago":
  a commit written long ago can have reached the upstream just now. The list of all local commits, oldest first, comes
  collapsed below; the commits that change a likely conflicting file are marked. Without known files that list is
  open. The line above the files says to rebase in a terminal or the IDE, or, with a bd command, that the prompt
  resolves each commit with bd, outside the working tree.
  [@test] ../../src/lib/update-conflict.test.ts (`gives each file the commits of each side that change it, in their order`)
  [@test] ../../src/lib/update-conflict.test.ts (`names when the commit was written and when it landed, if that was more than an hour later`)
  [@test] ../../src/lib/update-conflict.test.ts (`gives one time when the commit landed within an hour of being written`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)
- A large conflict stays a short list: the files the check reported (50), then "… and 10 more files"; per file at
  most 5 commits a side, yours the oldest (the first to stop the rebase) and the upstream's the newest, then
  "… and 2 more". The tooltip names three files of all, "Conflicts in 60 files: a, b, c and 57 more" (four of four). A block lists
  the paths the check reported (100), then "… and 20 more files". Every list is cut so that the count of the rest is
  never 1: one more than the limit is listed whole (51 files, 6 commits a side, 101 paths; `cutList`, and the check's
  `list_cut::kept`).
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should list the first files of a wide conflict and count the rest`)
  [@test] ../../src/lib/update-state.test.ts (`counts the files the backend left out of its list`)
  [@test] ../../crates/update-core/src/update_project_test.rs (`a_conflict_one_file_wider_than_the_list_reports_every_file_with_its_commits`, `many_blocking_paths_are_counted`)
  [@test] ../../src/lib/text.test.ts (`keeps a list one longer than its limit whole and cuts a longer one at the limit, counting the rest of all`)
- A block lists the overlapping paths and says to commit or stash them; changes to other files don't block. It says
  that the prompt asks about each file, then updates with bd, or, without a bd command, that you update again.
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain uncommitted changes that block the update`)
- The primary button "Copy Prompt for Agent" copies a prompt and confirms in place: it says "Copied" (`CopyCheckIcon`)
  for a moment, as a copy button does, and no toast covers the dialog. Both cases offer it: an overlap of uncommitted
  changes with the upstream needs the same merge as a conflict.
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)
- With a bd command, the conflict prompt must send the agent to bd's resolve session and keep it out of the working
  tree. It holds what bd does not know and leaves the protocol to bd (`bd help update`, the `hint` of each file,
  `session.next`, `retryable`): a prompt that restated it drifted from bd (it put `round_moved`, exit 3, under exit 2,
  and its "exit 3 retries" then sent the agent back to the same refused round). It names the repository path, the
  branch and the upstream; bd once, as the command to run as written (in PowerShell on Windows), with "bd is
  `<command>`; below, `bd` stands for it", "Run `bd help update` first and follow it", one JSON object per run; the
  state the user saw on one line ("State (fetched <time>): HEAD (<branch>) <id>, <upstream> <id>."), then per likely
  conflicting file "Likely conflict: <path> (mine: <ids>; upstream: <ids>)" (each round names the upstream commits of
  its files, so the prompt describes none); the steps:
  1. `bd update --no-fetch` (the upstream the user saw, no `--expect-head`, because peers commit meanwhile); with
     `session.blocked_paths`, ask the user about those files first;
  2. while it exits 1, resolve each file of `session.files` as its `hint` says (and `session.commit.hint`), only in
     the copy at `edit` and only between the markers (in PowerShell, write each copy with `[IO.File]::WriteAllText`,
     as UTF-8 without a byte order mark), then run `session.next` with the `--take` or `--delete` a hint names; a change
     the commit needs elsewhere waits for the fixup of step 3;
  3. on exit 0, build and test, and on a failure a `fixup! <subject>` commit of only the fixed files
     (`git -C <repository> commit -m 'fixup! <subject>' -- <paths>`, the repository quoted for the shell, and in
     PowerShell each `'` of the subject doubled; never the changes of other sessions) with `bd fold`, or
     `bd undo <undo_token>`;
  4. report each entry of `resolutions` and of `resolved` (the commits resolved by words) with its dropped lines.

  The rules: other sessions work in the tree, so never `git rebase`, `git stash`, `git checkout` or `git reset`, and
  never edit a conflicting file in the working tree; keep every message with its `(prefix)`, don't squash, reorder,
  split or drop commits, don't push; on an error, `retryable` goes back to step 1, exit 2 with `paths` fixes those
  copies and runs the command again, `uncommitted_overlap` and `empty_resolution` ask the user, anything else stops
  and shows the error. The prompt names no exit code above 2 and no other error code.
  [@test] ../../src/lib/agent-prompt.test.ts (`with bd, resolves a conflict in bd's session, outside the working tree`)
  [@test] ../../src/lib/agent-prompt.test.ts (`names PowerShell for a conflict on Windows`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)
- A conflict prompt stays a few kilobytes however large the conflict (hundreds of files with 50 upstream commits each
  were hundreds of kilobytes): it is a map, not the record; bd names the files and the upstream commits of each round,
  git the rest; with bd about 2 KB with one file. At most 20 files, then "… and <n> more files: bd names the
  files of each round" (without bd: "`git status` names them when the rebase stops"), counted as in the title. Per file at most 3 commits a side, by
  id: mine the oldest (the first to stop a replay), upstream the newest, then "+<n> more", "+<n> or more" when the
  backend kept only the newest 50 upstream commits. Without bd, each upstream commit is described once, at most the 10
  newest, then "… and <n> more:" with where the rest are (`git log --oneline <base>..<upstream> --
  <path>`). Without bd, more than 30 local commits list only those the files name (or the first 30, without known
  files), at most 30, with the count and `git log --reverse --oneline <base>..<head>`. A blocked prompt lists at most
  50 paths, then "… and <n> more: `git status` lists them", and asks about "these files". No list leaves out only one:
  one more than its limit is given whole (`cutList`), so the prompt never says "… and 1 more files".
  [@test] ../../src/lib/agent-prompt.test.ts (`never counts one where the item would stand: a list one longer than its limit is given whole`)
  [@test] ../../src/lib/agent-prompt.test.ts (`with bd, names the first files and a few commits of each by id`)
  [@test] ../../src/lib/agent-prompt.test.ts (`without bd, lists only the local commits the files name, with the command for the rest`)
  [@test] ../../src/lib/agent-prompt.test.ts (`counts the rest exactly when the backend did not cut the upstream list`)
  [@test] ../../src/lib/agent-prompt.test.ts (`lists the first local commits without known files`)
  [@test] ../../src/lib/agent-prompt.test.ts (`caps the paths of a blocked update`)
- Without a bd command, the conflict prompt must contain the repository path, the branch, the upstream with the ids of
  HEAD, the upstream and the merge base and the fetch time, the local commits oldest first, the likely conflicting
  files, the rules (keep every message with its `(prefix)`, don't squash, reorder, split or drop commits, don't push;
  other sessions may work in the tree, so uncommitted changes stop the agent, which asks the user and never stashes,
  resets, checks out or commits them) and the steps (rebase, resolve keeping the upstream change and the intent of the
  commit, build and test, continue, report each resolution). Each likely conflicting file names the ids of the local
  commits and the upstream commits that change it; "Upstream commits of these files" describes each upstream one once,
  with its subject, author and both dates: "(Daniil Bubnov, authored 2026-10-01, in origin/master since 2026-10-14)".
  [@test] ../../src/lib/agent-prompt.test.ts (`names the repository, the branch, the upstream and the exact ids`)
  [@test] ../../src/lib/agent-prompt.test.ts (`lists the local commits oldest first`)
  [@test] ../../src/lib/agent-prompt.test.ts (`names the commits on both sides of each likely conflicting file`)
  [@test] ../../src/lib/agent-prompt.test.ts (`states the rules and the steps`)
  [@test] ../../src/lib/agent-prompt.test.ts (`leaves out the file list when the conflicting files are unknown`)
- The blocked prompt must never stash: it lists the files, says that other sessions work in the tree, forbids to
  stash, reset, check out or commit them, and asks the agent to ask the user about each file. Then, with the app's bd,
  it runs `<bd> -C <repository> update --no-fetch` (quoted for the shell, `& '…'` in PowerShell on Windows) and
  continues as its output says; if that prints `state: conflict`, the agent runs `<bd> help update`, follows it and
  edits only the files at `edit`. Without one, it tells the user to update in Branch Deck again.
  [@test] ../../src/lib/agent-prompt.test.ts (`with bd, sends a blocked update to the user and then to bd, never to a stash`)
  [@test] ../../src/lib/agent-prompt.test.ts (`names PowerShell for the command of Windows`)
  [@test] ../../src/lib/agent-prompt.test.ts (`without bd, sends a blocked update to the user and back to Branch Deck, never to a stash`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain uncommitted changes that block the update`)

## User Experience

- The button is `ghost` `sm` with `ArrowDownIcon` and "Update" (a `Spinner` and the run's label while it runs, its
  `ProgressLine` along the bottom edge in `primary`); the state icon follows the label (`CheckIcon`
  `text-success`, `PackageCheckIcon` `text-info` when the user's commits landed, `TriangleAlertIcon` `text-warning`,
  `CircleHelpIcon` muted for a failed check). Up to date mutes the label instead of adding an icon. The footer line
  starts with the same icon at `size-3`.
- Sync is the `ghost` `sm` button with `RefreshCwIcon`, as Update (a `Spinner` at full colour while it runs), the
  shortcut a `Kbd` in its tooltip; after it, in a group of its own and in every state (no repository, an invalid
  path), Settings, the `ghost` `icon-sm` `SettingsIcon` ("Settings" and ⌘, / Ctrl+, in its tooltip,
  [Settings](settings.spec.md)). The header has no
  primary button: Sync runs by itself when the window opens the repository and after each change.
- The dialog uses `PreviewDialogFrame`: the title says what is wrong, the body starts with one line on what to do,
  then lists the files, each as its name and then its muted directory, which is clipped from its start on a narrow
  window, with a copy button on hover; the commits of each side sit below in aligned columns (side, hash, subject,
  then the time for yours and the author with the times for the upstream's, muted); the full list is a `Collapsible` with a `TriangleAlertIcon` `text-warning` mark. The footer shows
  `master ← origin/master`, `Esc Close` and the primary button.

## Testing / Local Run

- `cargo nextest run --workspace` covers the check, the update and the scan
  (`crates/update-core/src/update_project_test.rs`, `crates/git-executor/src/worktree_safety_test.rs`).
- `pnpm test:unit` covers the state words and the prompt.
- `pnpm e2e` covers the header, the dialog and the update with the templates `behind_upstream`, `update_conflict`,
  `update_conflict_wide` (60 files, 7 upstream commits each) and `update_blocked`: a clone with a bare upstream at
  `.git/upstream.git`, already fetched, so the update works offline.
  The upstream commit that conflicts landed 15 hours after it was written, so the dialog and the prompt show both
  times (`e2e_templates_have_the_expected_update_states` checks it); every template time is before the fixed clock of
  the e2e pages, 2024-01-02 10:00 UTC.

## Open Questions / Risks

- A write can still fail midway (a Git LFS smudge filter, a full disk) after git validated every path; the branch moves
  back and the written files show as uncommitted changes. See "Update project" in the design doc.
- The first check after a start or a fetch hashes the changed files the probe hasn't seen: about 60–130 µs a file on a
  loaded machine, so an update that changes tens of thousands of files costs seconds once.
- The usual time of a step is a guess from the last runs: a fetch that is much slower than usual moves slowly past its
  63 %, and the first update of a repository shares the bar by defaults. The test server forwards no progress either:
  the e2e tests send the progress messages to the page themselves (`holdWithProgress`).

## References

- [Git Operations](../../docs/design/git-operations.md): "Update project", the exception to invariant 1, and "Resolve
  session".
- [bd update](../cli/update.spec.md): the agent's update and its resolve session.
- [Outgoing Commits](outgoing.spec.md): the overview whose footer shows the update state.
