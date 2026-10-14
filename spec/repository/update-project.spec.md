---
name: Update Project
description: Requirements for Update Project, which fetches the baseline and rebases the checked out branch onto it without touching unrelated local changes, the header state of the update, and the hand-off of a conflict to a coding agent.
style: plain-1
targets:
  - ../../crates/git-ops/src/update_branch.rs
  - ../../crates/git-ops/src/worktree_update.rs
  - ../../crates/sync-core/src/update_project.rs
  - ../../crates/sync-core/src/transfer_progress.rs
  - ../../crates/sync-core/src/remote_command.rs
  - ../../crates/sync-core/src/sync.rs
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
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/update-state.ts
  - ../../src/lib/transfer-progress.ts
  - ../../src/lib/agent-prompt.ts
  - ../../src/lib/update-conflict.ts
---

# Update Project

Status: Implemented
Date: 2026-10-14

## Summary

Update Project (`⌘T`, as in IntelliJ) fetches the baseline (e.g. `origin/master`) and brings the checked out branch
onto it: a fast-forward without local commits, otherwise a rebase of the local commits. It works with uncommitted and
staged changes as long as the upstream doesn't change those files. A conflict or an overlap changes nothing; a dialog
explains it and copies a ready prompt for a coding agent that does the rebase.

The header shows whether an update would apply cleanly, from a check that every sync runs against the last fetch.

## Goals

- Update without leaving the app, and without stashing local work that the update doesn't touch.
- Know before updating whether it conflicts.
- Never leave a half-done rebase: either the update applies as a whole, or nothing changes.

## Non-goals

- No count of incoming commits anywhere: a monorepo gets hundreds, and only conflicts matter. The state is up to date,
  applies cleanly, conflicts in N files, or blocked by uncommitted changes. The user's own commits that the baseline
  already has are named, not counted as incoming ([Landed Commits](landed-commits.spec.md)).
- Branch Deck does not resolve conflicts and does not launch an agent; it copies a prompt.
- No merge of the upstream (`pull --no-rebase`), no update of submodules.

## Requirements

### Check

- The check must not use the network: it compares the branch with the tracking ref of the last fetch.
- It must refuse (an error, not a state) while a rebase, merge, cherry-pick or revert is in progress, on a detached HEAD,
  and when the local commits contain a merge.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`check_refuses_while_a_rebase_is_in_progress`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`merges_in_the_local_commits_are_refused`)
- A local baseline has nothing to update from: no check, and an update says so.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`local_baseline_has_nothing_to_update`)
- The state must be one of:
  - up to date: the upstream has nothing the branch lacks;
  - clean, as a fast-forward or a rebase;
  - conflict, with the files that likely conflict, the local commits oldest first, and the upstream commits that
    change those files (the newest 50, oldest first); each commit has its author and author time, its committer and
    committer time (for an upstream commit, about when it reached the upstream, e.g. when a robot landed it) and the
    likely conflicting files it changes;
  - blocked, with the local paths that overlap the update: uncommitted or staged changes on a file the upstream
    changes, an untracked file where it adds one, an ignored file where it adds one.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`same_line_conflict_reports_files_and_commits_and_changes_nothing`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`dirty_touched_path_blocks_and_changes_nothing`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`staged_touched_path_blocks`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`untracked_file_that_upstream_adds_blocks`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`ignored_file_that_upstream_adds_blocks`)
- A file that was only touched (same content, new modification time) must not block.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`touched_only_file_passes_thanks_to_the_refresh`)
- Commits that an upstream force-push removed must not be replayed: the old tip of a forced fetch and the fork point of
  the tracking ref's reflog are excluded, as `git pull --rebase` does.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`upstream_force_push_does_not_replay_the_removed_commits`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`check_after_a_forced_fetch_elsewhere_uses_the_fork_point`)
- Every sync must run the check and report it once, beside the rest of the sync (the event can come at any point);
  a failed check is reported with its reason and must not fail the sync.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`sync_reports_the_baseline_status`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`sync_reports_a_failed_check_without_failing`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`test_branches_grouped_precedes_per_branch_events`)
- Repeated checks of the same state must not write new objects; a check made for a sync can't be applied.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`preview_check_cannot_be_applied`)
- The check inside a sync must cost in proportion to the paths the update changes, not the size of the index: it
  never copies, refreshes or writes the index, reuses the replay of an unchanged HEAD and upstream, and rereads only
  files whose stat data changed. It must still find dirty and staged touched paths, must not block on a file that was
  only touched, and must block on untracked and ignored files where the upstream adds one.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`preview_blocks_dirty_and_staged_touched_paths_without_touching_the_index`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`preview_does_not_block_a_touched_only_file`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`preview_blocks_untracked_and_ignored_files_that_upstream_adds`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`preview_reuses_the_replay_of_an_unchanged_state`)
- A file that the upstream replaces by a directory (or the reverse) must not block.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`file_replaced_by_a_directory_upstream_is_not_in_the_way`)

### Update

- The update must fetch only the baseline's remote branch, so the push lease of the virtual branches still compares
  against what the last push saw, then check again and apply a clean result.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`parses_the_forced_update_line_of_fetch`)
- A fast-forward must move the branch and update the files.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`fast_forward_updates_the_working_tree`)
- A rebase must keep the author of every local commit; the committer is the current user.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`rebase_keeps_the_author_and_makes_the_current_user_the_committer`)
- Uncommitted, staged and untracked changes on paths the update doesn't change must survive byte for byte, the staged
  version staged.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`dirty_and_staged_files_on_untouched_paths_survive_byte_identical`)
- A local commit whose changes are already upstream is dropped and reported, not an error. When the replay conflicts
  because a robot folded fixups into their target, the landed commits are left out if nothing local is lost
  ([Landed Commits](landed-commits.spec.md)).
  [@test] ../../crates/sync-core/src/update_project_test.rs (`commit_already_upstream_is_dropped_and_reported`)
- Conflict and blocked are results: the branch, the index, the working tree and the reflog stay as they were.
- Up to date must write no reflog entry.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`up_to_date_writes_no_reflog_entry`)
- The branch moves with a CAS ref transaction (`branch-deck: update project`) before the files; if the files can't
  follow (one changed since the check), the branch moves back (`branch-deck: update project rolled back`) and the update
  reports that nothing was changed.
  [@test] ../../crates/sync-core/src/update_project_test.rs (`rolls_back_when_a_file_changes_between_the_check_and_the_update`)
- The working tree is written by `crates/git-ops/src/worktree_update.rs` only; the safety scan fails on any other
  `read-tree -u`, `--reset` or `checkout-index`, and on a change of the number of exempt argument lists.
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`production_code_never_writes_the_working_tree`)
  [@test] ../../crates/git-executor/src/worktree_safety_test.rs (`scanner_finds_read_tree_updates`)

### Header

- The Update button sits in the header before the outgoing indicator: incoming and outgoing together, the time of the
  last sync next to Sync. It is hidden for a local baseline and before the first sync.
- The header must stay one line down to the minimum window width, 960 px (`min_inner_size`), its controls never
  overlapping: the repository selector gives way first (its path clipped from the start), the branch count shows from
  1024 px, the time of the last sync from 768 px; Update, the outgoing indicator, Activity ([Activity](activity.spec.md)),
  Sync and its settings never shrink.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should keep its controls apart on one line at the minimum window width`)
- While a sync runs, the time of the last sync gives its place to "Syncing…", so nothing in the header moves.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should not move while a sync runs`)
- Sync has the tooltip "Sync branches ⌘R" (`Ctrl+R` off macOS) and its settings (auto-sync on focus) next to it. A
  failed sync turns it `destructive` with the error in its tooltip; with the page alert, the error shows twice at most.
  The repository selector has a tooltip only without a repository ("Select a Git repository to manage branches") or
  for its error.
  [@test] ../../tests/e2e/repository/header.spec.ts (`should name the shortcut of Sync and keep the selected repository without a tooltip`)
- It shows the state, never a count:
  - up to date: muted, no state icon, tooltip "Up to date with origin/master";
  - clean: a `success` check, tooltip "Rebases cleanly onto origin/master" ("Fast-forwards to origin/master");
  - conflict: a `warning` icon, tooltip "Conflicts in N files: …" (at most three names);
  - blocked: a `warning` icon, tooltip "Your uncommitted changes in … overlap origin/master";
  - a failed check: muted with a question mark, tooltip "Can't check the update: <reason>".
  [@test] ../../src/lib/update-state.test.ts (`names the state, never a count of upstream commits`)
  [@test] ../../src/lib/update-state.test.ts (`names at most three files`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)
- The tooltip must say when the baseline was last fetched: Branch Deck's own fetch or one elsewhere (`FETCH_HEAD`),
  whichever is newer, and the shortcut: "Fetched 5 minutes ago · fetch and update ⌘T".
  [@test] ../../src/lib/update-state.test.ts (`takes the newer of Branch Deck's fetch and one elsewhere`)
- Before the first fetch, a state would claim what nobody asked the remote: the button is muted without a state icon,
  its tooltip says "origin/master not fetched yet" over "fetch and update ⌘T", and the footer of the outgoing overview
  "Incoming from origin/master: not fetched yet". A conflict or a blocked update, found against the copy of the clone,
  keeps its words and its icon, then "· origin/master not fetched yet".
  [@test] ../../src/lib/update-state.test.ts (`says the baseline was not fetched yet instead of a state nobody asked the remote for`)
  [@test] ../../src/lib/update-state.test.ts (`says the same in the footer of the outgoing overview`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should be muted when up to date`)
- A relative time must age while it is shown, without a sync or a fetch: "Last synced 1 minute ago" next to the button,
  the fetch time of the tooltip and of the footer line, "Pull requests checked" and "Patronus checked" in the overview,
  the times of the Landed island and chips, of the commits a force push drops and of the commit that merged an archived
  branch. One interval per interval length renews them every 15 s (`useNow`), so "1 minute ago" is at most a tick late;
  only the text re-renders.
  [@test] ../../src/hooks/useNow.test.ts (`ticks every listener of an interval length at once, with one interval that stops with the last listener`)
  [@test] ../../src/hooks/useNow.test.ts (`ages a relative time without anything else changing: "Last synced just now" becomes "1 minute ago"`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should age the last sync next to the Update button while it is shown`)
- A click, or File → Update Project (`⌘T`), runs the update; meanwhile the button shows a spinner and the step:
  "Fetching…", "Checking…", "Updating…", and its tooltip names only the step, without the state of the last check and
  without "fetch and update": "Fetching origin/master…", "Checking the update onto origin/master…", "Updating to
  origin/master…".
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should run from the Update Project menu item`)
  [@test] ../../src/lib/update-state.test.ts (`names the running step in the tooltip, never the last state`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should name the running step in the tooltip, not the last state`)
- The step and the fetch progress belong to the repository the update (or the Fetch of a merge toast) runs in: after a
  switch, the buttons show the selected repository's, never another one's. One update or fetch runs at a time in a
  repository; another repository can update meanwhile. An update applied in a repository that is no longer selected
  doesn't sync the selected one.
  [@test] ../../src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)
  [@test] ../../src/stores/update-project.test.ts (`runs one at a time in a repository`)
- Once git reports a stage of the fetch, the button shows the percent of the whole fetch, "Fetching 45%", and its
  tooltip the last progress line of git, as IDEA shows a fetch. The percent weighs the stages as IDEA's
  `GitStandardProgressAnalyzer` does (counting 5 %, compressing 10 %, receiving or unpacking 80 %, resolving deltas
  5 %), never decreases, and reaches the window at most every 100 ms; the end of a stage always does. A push reports
  its progress the same way ([Outgoing](outgoing.spec.md)).
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`stages_have_the_shares_of_idea`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`small_fetch_unpacks_instead_of_receiving`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`percent_never_decreases`)
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`reporter_sends_the_first_progress_and_every_stage_end`)
  [@test] ../../crates/sync-core/src/update_project_test.rs (`fetch_reports_progress_until_the_last_stage_ends`)
  [@test] ../../src/lib/update-state.test.ts (`shows the percent of the fetch once git reports a stage`)
- The progress is read from `--progress` lines on stderr in the C locale (git has no machine-readable progress), best
  effort: a line it doesn't know changes nothing, and nothing but the label depends on it. The error of a failed fetch
  keeps git's messages without the progress updates.
  [@test] ../../crates/sync-core/src/transfer_progress_test.rs (`other_lines_change_nothing`)
  [@test] ../../crates/sync-core/src/remote_command_test.rs (`remote_command_runs_untranslated`)
  [@test] ../../crates/git-executor/src/git_command_executor_test.rs (`failed_command_reports_messages_without_progress_updates`)
- After an applied update a toast says "Updated master: rebased 2 commits onto origin/master" or "…: fast-forwarded to
  origin/master", and a sync follows without expanding or scrolling. When commits landed, the toast leads with them
  instead ([Landed Commits](landed-commits.spec.md)).
  Up to date says "master is already up to date with origin/master".
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show a clean update, apply it and then be up to date`)
- The footer of the outgoing overview must show the same state in one line, with the state icon of the button, named
  as incoming and with the fetch time, "Incoming from origin/master: rebases cleanly · fetched 5 minutes ago" (up to
  date: "nothing new"), with an Update button. Below the push rows, a bare "origin/master: up to date" reads as nothing
  to push. While the fetch receives, the text has the amount and the rate as git prints them, "Incoming from
  origin/master: 5.79 MiB · 32.57 MiB/s", and the button the percent; the button keeps its full colour while the
  update runs, as the one in the header.
  [@test] ../../src/lib/transfer-progress.test.ts (`shows the amount and the rate as git prints them`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the outgoing overview`)

### Conflict and blocked dialog

- A conflict or a block after a click opens a dialog in the frame of the previews: it says nothing was changed and
  when the baseline was fetched.
- The conflict title counts the files, "1 file conflicts with origin/master" or "6 files conflict with origin/master";
  without known files it says "Your commits conflict with origin/master".
- A conflict lists the files that likely conflict, each with the local commits ("yours", with their author time) and
  the upstream commits that change it. An upstream commit shows its author and both times when they differ by more
  than an hour, "Daniil Bubnov · authored 13 days ago · landed 2 hours ago", else one, "Daniil Bubnov, 13 days ago":
  a commit written long ago can have reached the upstream just now. The list of all local commits, oldest first, comes
  collapsed below; the commits that change a likely conflicting file are marked. Without known files that list is
  open.
  [@test] ../../src/lib/update-conflict.test.ts (`names when the commit was written and when it landed, if that was more than an hour later`)
  [@test] ../../src/lib/update-conflict.test.ts (`gives one time when the commit landed within an hour of being written`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain a conflict and copy a prompt for an agent`)
- A block lists the overlapping paths and says to commit or stash them; changes to other files don't block.
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should explain uncommitted changes that block the update`)
- The primary button "Copy Prompt for Agent" copies a prompt and confirms with the toast "Prompt copied". Both cases
  offer it: an overlap of uncommitted changes with the upstream needs the same merge as a conflict.
- The conflict prompt must contain the repository path, the branch, the upstream with the ids of HEAD, the upstream
  and the merge base and the fetch time, the local commits oldest first, the likely conflicting files, the rules (keep
  every message with its `(prefix)`, don't squash, reorder, split or drop commits, don't push, stash untracked changes
  first and pop them at the end) and the steps (rebase, resolve keeping the upstream change and the intent of the
  commit, build and test, continue, report each resolution). Each likely conflicting file names the local commits and
  the upstream commits that change it, each upstream one with its subject, author and both dates: "(Daniil Bubnov,
  authored 2026-10-01, in origin/master since 2026-10-14)".
  [@test] ../../src/lib/agent-prompt.test.ts (`names the repository, the branch, the upstream and the exact ids`)
  [@test] ../../src/lib/agent-prompt.test.ts (`lists the local commits oldest first`)
  [@test] ../../src/lib/agent-prompt.test.ts (`names the commits on both sides of each likely conflicting file`)
  [@test] ../../src/lib/agent-prompt.test.ts (`states the rules and the steps`)
  [@test] ../../src/lib/agent-prompt.test.ts (`leaves out the file list when the conflicting files are unknown`)
- The blocked prompt asks to stash, update, pop and merge, keeping the uncommitted changes uncommitted.
  [@test] ../../src/lib/agent-prompt.test.ts (`asks to stash, update and restore uncommitted changes that block the update`)

## User Experience

- The button is `ghost` `sm` with `ArrowDownIcon` and "Update"; the state icon follows the label (`CheckIcon`
  `text-success`, `PackageCheckIcon` `text-info` when the user's commits landed, `TriangleAlertIcon` `text-warning`,
  `CircleHelpIcon` muted for a failed check). Up to date mutes the label instead of adding an icon. The footer line
  starts with the same icon at `size-3`.
- Sync is the `default` `sm` button with `RefreshCwIcon` (a `Spinner` at full colour while it runs), the shortcut a
  `Kbd` in its tooltip; its settings are the `ghost` `icon-sm` `Settings2Icon` after it.
- The dialog uses `PreviewDialogFrame`: the title says what is wrong, the body starts with one line on what to do,
  then lists the files, each as its name and then its muted directory, which is clipped from its start on a narrow
  window, with a copy button on hover; the commits of each side sit below in aligned columns (side, hash, subject,
  then the time for yours and the author with the times for the upstream's, muted); the full list is a `Collapsible` with a `TriangleAlertIcon` `text-warning` mark. The footer shows
  `master ← origin/master`, `Esc Close` and the primary button.

## Testing / Local Run

- `cargo nextest run --workspace --exclude model-ai` covers the check, the update and the scan
  (`crates/sync-core/src/update_project_test.rs`, `crates/git-executor/src/worktree_safety_test.rs`).
- `pnpm test:unit` covers the state words and the prompt.
- `pnpm e2e` covers the header, the dialog and the update with the templates `behind_upstream`, `update_conflict` and
  `update_blocked`: a clone with a bare upstream at `.git/upstream.git`, already fetched, so the update works offline.
  The upstream commit that conflicts landed 15 hours after it was written, so the dialog and the prompt show both
  times (`e2e_templates_have_the_expected_update_states` checks it); every template time is before the fixed clock of
  the e2e pages, 2024-01-02 10:00 UTC.

## Open Questions / Risks

- A write can still fail midway (a Git LFS smudge filter, a full disk) after git validated every path; the branch moves
  back and the written files show as uncommitted changes. See "Update project" in the design doc.
- The first check after a start or a fetch hashes the changed files the probe hasn't seen: about 60–130 µs a file on a
  loaded machine, so an update that changes tens of thousands of files costs seconds once.
- The e2e mock does not forward the progress channel: the page sees only the fetch before git reports a stage (the test
  holds the request); the percent and the later steps are tested in the units.

## References

- [Git Operations](../../docs/design/git-operations.md): "Update project", the exception to invariant 1.
- [Outgoing Commits](outgoing.spec.md): the overview whose footer shows the update state.
