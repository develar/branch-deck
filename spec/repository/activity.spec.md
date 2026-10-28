---
name: Activity
description: Requirements for the log of what Branch Deck did in a repository (pushes, updates, landed reports, Patronus robots, pull request retargets, history rewrites, errors) and the review requests it told about, opened from the header, with the Undo of a rewrite while its toast's lasts, in a side panel of the main window or a window of its own, with the chips of the kinds and a search.
style: plain-1
targets:
  - ../../src/stores/activity.ts
  - ../../src/presenters/activity.ts
  - ../../crates/operations/src/journal.rs
  - ../../crates/operations/src/registry_watch.rs
  - ../../crates/app-settings/src/journals.rs
  - ../../crates/app-settings/src/file.rs
  - ../../crates/domain-types/src/activity.rs
  - ../../src-tauri/src/commands/activity.rs
  - ../../src-tauri/src/commands/session.rs
  - ../../crates/test-server/src/activity_routes.rs
  - ../../src/components/repository/ActivityButton.tsx
  - ../../src/components/repository/ActivityLog.tsx
  - ../../src/components/shared/FilterChips.tsx
  - ../../src/pages/ActivityPage.tsx
  - ../../src/components/repository/ActivityPanel.tsx
  - ../../src/components/repository/ActivityPanelContent.tsx
  - ../../src/hooks/useActivityPanel.ts
  - ../../src/stores/activity-panel.ts
  - ../../src/lib/input-modality.ts
  - ../../src/pages/IndexPage.tsx
  - ../../src/stores/settings/appSettings.ts
  - ../../src/components/shared/SearchField.tsx
  - ../../src/lib/activity-window.ts
  - ../../src/lib/activity-window-link.ts
  - ../../src/hooks/useActivityWindowLink.ts
  - ../../src/hooks/useActivityReads.ts
  - ../../src/hooks/useMainWindowLink.ts
  - ../../src/lib/main-menu.ts
  - ../../src-tauri/src/menu.rs
  - ../../src-tauri/src/zoom.rs
  - ../../src/lib/zoom.ts
  - ../../src-tauri/capabilities/sub-windows.json
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/LandedCard.tsx
  - ../../src/components/branch/DroppedCommitList.tsx
  - ../../src/lib/command.ts
  - ../../src/lib/errors.ts
  - ../../src-tauri/src/operation.rs
  - ../../crates/domain-types/src/error.rs
  - ../../crates/app-api/src/api_error.rs
  - ../../src/lib/landed.ts
  - ../../src/lib/push-run.ts
  - ../../src/presenters/pull-request-url.ts
  - ../../src/lib/robot-run.ts
  - ../../src/presenters/actions.ts
  - ../../src/lib/push-summary.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/review-requests.ts
  - ../../src/lib/review-requests.ts
  - ../../src/stores/update-project.ts
  - ../../src/stores/landed.ts
  - ../../src/lib/run-intent.ts
  - ../../src/lib/remote-status.ts
---

# Activity

Status: Implemented
Date: 2026-10-15

## Summary

What Branch Deck did lives in toasts of 5 to 10 seconds: the push that dropped remote commits, the update, what landed,
the Safe Push the user started, the squash and its Undo. Activity keeps them: a `HistoryIcon` button in the header opens
a side panel beside the branch tables (or a window of its own) with the log of the selected repository, newest first
by day, each entry with what happened, the detail and when; chips of the kinds and a search find an entry in a month of
them. A branch name shows its row, a landed entry
has the report of the island, a rewrite has its Undo while the toast's lasts. The log is read from journals: the
operations of the remote, of the app and of `bd`, are the journal of their repository, the rest the page records in the
journal of the app data directory; no window keeps it. It is the history the landing overview leaves out: the overview
tells the present, its line of the last day opens Activity at its Robots.

## Goals

- Answer "what did I push yesterday", "what did the update do", "which robot did I start" after the toast is gone.
- One record of each event, in the words of its toast: Activity adds no news of its own.
- Undo from Activity exactly as from the toast, for as long, once.

## Non-goals

- No history of the repository (that is `git reflog`): only what this app and `bd` did, as the app says it.
- No sync and no fetch of their own in the log: they change nothing the user asked for, a failure is an error entry.
- No Undo after the toast's window or after a restart: the rewrite it would revert may be pushed or rewritten again.

## Requirements

### Recording

- What Branch Deck did is read from journals, never kept by a window. Every operation of the remote, of the app and of
  `bd` alike (a push, a start of a Safe Push or a Dry Run, a restart or a cancel of a robot, a retarget, a pull request
  created, closed or reviewed), is the journal of its repository (`operations::journal`,
  `$GIT_COMMON_DIR/branch-deck/journal.jsonl`): an entry once it ended, with each effect and what it answered. What only
  the page knows (an error it told, an update, what landed, a rewrite, a review request, an approval, the news of a
  robot) the main window records in the journal of the app data directory (`record_activity`, the same writer), always
  with its repository: an error in the repository the main window shows when it is told, but one that names an
  operation of a journal shows where that operation ran, in the projects of its journal; a sub-window records nothing.
  [@test] ../../crates/operations/src/journal_test.rs (`the_page_records_into_its_journal_and_clear_log_forgets_the_log_of_a_repository_in_both`)
  [@test] ../../src/stores/activity.test.ts (`reads the journals of the repositories and the page's, each repository's log apart, newest first`, `records nothing without a repository`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should read the log from the journal of the repository after a reload`)
- Each event is an entry in the words its toast had: the toast and the entry take their title and description from
  one function (`describePush`, `describeRobot`, `presentLandOutcome`, `appliedUpdateToast`; the entries of an
  operation `presentActivity` of `src/presenters/activity.ts`). An operation is an entry per event it made: each branch
  it pushed (of a stack push each, of a whole stack its stack branch), each pull request it retargeted, the robot it
  started; a push that is a step of another action (a Safe Push start, a merge request) too, in the words its toast
  would have. A toast whose later states replace it in place ([One toast per event](../../docs/guidelines/ux-writing.md#one-toast-per-event):
  the merge, then the landing; the failure of a robot, then its restart) leaves an entry per state: Activity is the log
  of the events, the toasts are one per event.
  [@test] ../../src/presenters/activity.test.ts (`puts the operations of a journal in every project of its repository, and the page's entries in theirs, newest first`)
- A push: "Pushed feature-auth" / "3 commits", "4 commits · created the remote branch"; a force push "Force pushed
  ABC-123" / "1 commit · dropped 1 remote commit" with the dropped remote commits (`DroppedCommitList`), or "replaced the
  rebased copies", "replaced 1 amended commit", "replaced the parent's older commits"; a remote that moved since its
  status says so ("auth changed on the remote" / "… Nothing was pushed."). The words come from the remote status the
  push found, which the journal keeps with the push (`EffectRecord.remote`: the status the session showed, of Push Up
  to Here the part it sent, the one it went over once more, what the remote has now when it moved).
  [@test] ../../src/lib/push-summary.test.ts (`says what the push did from the status it was made against`)
  [@test] ../../src/presenters/activity.test.ts (`says each branch a push sent from the status it went over, a force push with the remote commits it dropped`, `says a remote that moved under the push of a branch, nothing pushed`)
  [@test] ../../crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`)
- An update: applied, with the toast's words ("Updated master: rebased 1 commit onto origin/master", or what landed
  first, with the report); refused, "Didn't update master" / "Conflicts in 1 file: src/app.txt. Nothing was changed.";
  an update with nothing to do is no entry.
  [@test] ../../src/stores/activity.test.ts (`records a refused update, not one with nothing to do`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`, `should record a refused update`)
- A landed report is recorded once: by the update that brought it, or by the sync after an update made elsewhere (the
  commits no entry has yet, "Found by a sync"), also while the record of the update is on its way.
  [@test] ../../src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)
- A Patronus robot started ("Safe Push started" / "auth → master · profile Large Scale Change", or the comment Patronus
  accepted), restarted, canceled ("Safe Push is canceling" / "auth → master"), with "Open in Patronus": the operation
  of the journal. Failing ("A required check of auth failed"), failed ("Safe Push of auth failed" / "Fast Tests · auth
  → master", [Patronus](../branch/patronus.spec.md#failed)) or merged ("auth merged into master" / "Safe Push"): the
  news of the session, which the page records.
  [@test] ../../src/presenters/activity.test.ts (`says the robot a start started in the words of its toast, and the comment Patronus accepted`, `says a restart and a cancel of a robot, and the pull request a creation opened`)
  [@test] ../../src/stores/activity.test.ts (`records the news of a robot with its page`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and copy the prompt for an agent from the toast`)
- A new review request ([Review Requests](review-requests.spec.md#new-requests)), one entry per request also when one
  toast told several: "dave requested your review on #104" / its title, kind `review`, with "Open Pull Request", the
  name of the action everywhere ([UX Writing](../../docs/guidelines/ux-writing.md#actions)).
  [@test] ../../src/stores/review-requests.test.ts (`tells the requests the session tells new in one toast with Review, unread until opened, recorded in Activity`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)
- An approval of the user's pull request that made a branch ready ([Review Requests](review-requests.spec.md#approval-news)):
  "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform", kind `review`, with "Open Pull Request".
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should tell the approval that arrives with a poll, with Safe Push of the branch first`)
- A pull request the app retargeted from a virtual branch to the baseline ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)),
  "Retargeted #802 to master" / "login · it went into auth"; one it created ("Created pull request #12" / "auth →
  master · review requested from bob"), one the user closed ("Closed pull request #12" / "#12 no longer matches the air
  stack: auth landed"), a review the user submitted ("Approved #7"); kind `review`, with "Open Pull Request", a link to
  the pull request in the GitHub repository of the repository. The closes of a merged robot are states of its merge.
  [@test] ../../src/presenters/activity.test.ts (`says the retarget of a pull request out of a virtual branch, with the pull request on GitHub`, `says the close the user asked and a review, not the closes of a merged robot`)
  [@test] ../../crates/test-server/src/github_host_test.rs (`a_push_retargets_the_pull_request_of_its_branch_out_of_a_virtual_branch_first`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)
- A history rewrite with an Undo toast (squash, fixups, move, ungroup, rename, stack, unstack, reword), in the
  repository it rewrote, and every error toast (`notifyError`, with its reason). An operation of the remote that failed
  is the entry of its error toast, which names it (`operationId`), in the projects of the journal of the operation,
  whichever repository the main window showed when the toast came (a Safe Push of one repository that fails while
  another is shown is in the log of the first only); one no toast told (of `bd`) says what it was for and why it failed
  ("Cannot push auth" / "The remote refused the push: …"); a canceled one is no entry. A rewrite or a pick of `bd`
  (its journal entry, `tool: bd`), which no toast told, is an entry of kind `rewrite` in the words of the rewrite
  ("Unstacked beta", "Picked commits"; `presentRewrite`), an agent's marked as any other; an update of `bd` tells
  nothing of what it moved and is no entry. Clear Log of a repository forgets the errors the page told of the
  operations of its journal too.
  [@test] ../../src/stores/activity.test.ts (`records the errors the user was told about, in the repository the main window shows`)
  [@test] ../../src/presenters/activity.test.ts (`says an operation that failed unless the page recorded the error of its toast, and leaves out the canceled`)
  [@test] ../../src/presenters/activity.test.ts (`puts the error the page told of an operation where the operation ran, whichever repository the window showed then`)
  [@test] ../../src/presenters/activity.test.ts (`says a rewrite and a pick of bd, which no toast told, an agent's marked, and not the rewrites of the app nor an update of bd`)
  [@test] ../../crates/operations/src/journal_test.rs (`clear_log_forgets_the_errors_the_page_told_of_the_operations_of_the_journal`)
  [@test] ../../src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`)
- The entry of an error a command returned keeps the operation it names as `operationId`, the key to the command's
  lines in the app's log: every command that can fail runs its body as an operation, the `operation{operation_id=…}`
  span of `src-tauri/src/operation.rs`, and its error carries the id (`ApiError.operation_id`); the app's log keeps
  the lines for days (the retention as the comment of `LOG_FILE_BYTES` in `src-tauri/src/lib.rs` describes it). The page
  log line of the error carries it as `operation_id`. A page step that throws what a command returned throws it as a
  `commandFailure` (`src/lib/errors.ts`), which keeps the id up to the `notifyError` of its caller, and a review write
  hands the error of its effect itself to its feedback. An entry of an operation of the remote is its id (the
  `operation_id` of its log lines, of its entry in the journal, of the JSON of `bd`). Copy Diagnostics of the entry
  copies it (below); an error raised outside a command has none.
  [@test] ../../src/stores/activity.test.ts (`keeps the operation a failed command names, for its lines in the log`)
  [@test] ../../src/stores/review-requests.test.ts (`hands the error of a failed approval to its feedback as the engine answered it, and starts nothing`)
  [@test] ../../src/lib/errors.test.ts (`throws what a command returned with its message, its code and the operation it names`)
  [@test] ../../src/lib/errors.test.ts (`is what the page throws for a failed command: no new Error of its message drops the operation`)
  [@test] ../../src/lib/errors.test.ts (`returns the operation an ApiError names, and nothing for any other error`)
  [@test] ../../src/lib/errors.test.ts (`passes the operation an ApiError names to the backend log line, as operation_id`)
  [@test] ../../src-tauri/src/command_scan_test.rs (`every_command_has_a_span_and_a_fallible_one_runs_as_an_operation`)
  [@test] ../../src-tauri/src/command_scan_test.rs (`a_body_is_one_operation_only_without_work_outside_it`)
  [@test] ../../crates/git-executor/src/thread_span_scan_test.rs (`every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller`)
  [@test] ../../crates/git-executor/src/thread_span_scan_test.rs (`the_scan_reads_the_argument_of_a_spawn_past_strings_nested_calls_and_line_breaks`)
  [@test] ../../src-tauri/src/tracing_log_test.rs (`the_async_lines_of_a_command_and_its_blocking_step_name_one_operation`)
  [@test] ../../src-tauri/src/log_rotation_test.rs (`the_log_retention_comment_describes_the_plugin_version_the_lockfile_pins`)

### Undo

- A rewrite entry shows "Undo" (`xs`) for the 10 s of its toast. Either Undo, the toast's or the entry's, reverts the
  rewrite once (a second press while it runs does nothing), and once it reverted it the toast closes and the entry says
  "undone"; after the window the button goes, and the entry says what was done, as its title. An Undo pressed while
  another rewrite of the repository runs waits for it in the queue ([Running Operations](../../docs/design/operations.md)),
  never refused; an undo that failed (its error toast says why) leaves the toast and the entry as they were, with their
  Undo, which runs it again.
  [@test] ../../src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`, `marks a rewrite undone from its toast once its undo undid it`, `keeps the Undo of a rewrite whose undo failed, in its toast and in Activity: a second press runs it again`)
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should run the Undo of a move pressed while another rewrite runs after it, the toast and its Undo kept until it undid`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts, in the window`, `should offer the Undo of a squash while the toast's lasts, in the panel`)
- The undo itself lives in the main window, by the id of the entry, not in the journal: after a restart no entry has
  an Undo.
  [@test] ../../src/stores/activity.test.ts (`has no Undo for a rewrite of an earlier session`)

### Log

- The main window reads the log (`read_activity`): the journal of the repository of each recent project, which
  `subscribe_view` remembered for the project when a window opened its session (`journalPaths` of the settings, by the
  path of the project, written by Rust; no window writes it), so the read runs no git process; each journal once for
  the projects that share it (the worktrees of one repository), all in parallel on the blocking pool, each waited for
  200 ms (`ACTIVITY_READ_TIMEOUT`), then shown as it answered last: one whose repository is gone (its
  `$GIT_COMMON_DIR` is not there: a removed disk, a deleted repository), that can't be read or that never answered (a
  disk that hangs) is listed unavailable and the others show; a journal its repository has not written yet is empty.
  One read of a journal runs at a time; a read asked while one runs waits for the next, which begins once that one
  answered (the one that runs may have read the journal before the change the read was asked for), and the reads asked
  meanwhile share it; one that answers after the wait with other entries tells the windows (`activity-changed`), so a
  journal of a slow disk shows once it answered, never "unavailable" for good because the machine was loaded. The journal of the page comes with them. The main window and
  the Activity window each read the log themselves (`useActivityReads`): when they open, and again when a journal
  changes (`activity-changed`: a write of the page, or the watch of the operations registry of a repository a window
  shows sees its journal change, an operation of the app or of `bd` ended); the main window also after each operation
  and each write of its own. The log of a settings file of before (the key `activity`) is dropped by the load.
  [@test] ../../crates/operations/src/journal_test.rs (`activity_reads_the_journal_of_each_repository_once_and_lists_one_it_cannot_read`, `a_journal_that_answers_after_the_bound_shows_once_it_answered`, `a_read_asked_while_one_runs_shows_the_journal_as_it_is_after_that_one`, `the_journal_of_a_repository_that_is_gone_is_unavailable_and_one_not_written_yet_is_empty`)
  [@test] ../../crates/app-settings/src/journals_test.rs (`the_recent_projects_have_the_journals_remembered_for_them_and_one_opened_before_has_none`, `no_recent_projects_have_no_journals`)
  [@test] ../../crates/app-settings/src/file_test.rs (`the_activity_log_of_a_file_of_before_is_dropped_by_the_load_and_the_next_save`)
  [@test] ../../src/lib/run-intent.test.ts (`runs the plan Rust prepared on its fingerprint, then waits for the view of what it changed; Activity reads the journal it ended in`)
- The operations of the agents, whose worktree is a slot of the lease pool of the repository (`leaseSlot` of the entry,
  by `leases.json` when it ended), are entries as any other, marked "Agent" (its worktree in the tooltip), and show
  only behind the Agents chip (Window).
  [@test] ../../src/presenters/activity.test.ts (`says an operation of an agent as any other, with the worktree of its slot of the lease pool`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)
- A journal of the repository the read could not read is a mark above the log, in the window and in the panel: "Cannot
  read the journal of the repository" / "Its operations show once it answers again." and the path of the journal; a
  journal of another repository marks nothing. Without other entries the mark stands for "Nothing yet", and the header
  button stays.
  [@test] ../../src/stores/activity.test.ts (`lists the journals of a repository its read could not read, and none of another repository`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should mark an unavailable journal`)
- Each journal keeps at most 200 entries, none older than 30 days; the oldest go first.
  [@test] ../../crates/operations/src/journal_test.rs (`the_journal_keeps_the_newest_entries_of_the_last_thirty_days`)
- "Clear Log" in the footer of the window forgets the whole log of the repository, whatever the filter shows (its
  tooltip says it): the journal of its repository empties (`clear_activity`, under `operations.lock`) and the entries
  of the page of it leave the journal of the page; the other repositories keep theirs. The window then says "Nothing
  yet: what Branch Deck does in the repository shows here", Clear Log disabled, and the header button goes; a clear
  that fails is an error toast in the window.
  [@test] ../../crates/operations/src/journal_test.rs (`the_page_records_into_its_journal_and_clear_log_forgets_the_log_of_a_repository_in_both`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should clear the log of the repository`)

### Side panel

- The header button, View → Activity of ☰ (custom title bar) and the native Window → Activity (macOS) or View →
  Activity (Windows and Linux) toggle the side panel of the main window; "Show in Activity" of the landing overview
  opens it at its chip. Every entry point asks the main window (`openActivity`, the app command `open_activity`), which
  shows Activity. An open names its chip ("Show in Activity": Robots; the others none, All) and starts over with it and
  an empty search. The panel shows the log of the selected repository only, with the chips, the Agents chip and the
  marks of the window, and follows a switch of it.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`, `should toggle the side panel from the View menu of ☰ and from the native menu`, `should mark an unavailable journal`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)
- The panel opens only where the area under the header has room for it beside the islands: their minimum (768 CSS px,
  `MIN_ISLANDS_WIDTH`) plus the panel's (320 px). Without the room an entry point opens the window `activity` instead
  (or brings the open one to the front), and a panel left open shows again once the window is wide enough.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should undo a reword from Activity`)
- Open or closed is a setting (`appSettings.activityPanel`), kept across starts; the width of the panel is kept once the
  user dragged its divider (`useDefaultLayout`, as the sidebar of a review).
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`)
- The focus goes into the log of the panel only when the keyboard opened it (the last input of the main window was a
  key, `isKeyboardInput`); a click and the native menu leave it where it is. Escape closes the panel only while the
  focus is in it (the search takes its own Escape first) and gives the focus back where it was before it came in; Close
  (`XIcon`, "Close Activity") does the same.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should take the focus into the panel only when the keyboard opened it, and give it back on Escape`)
- What the panel does runs where it is: a branch name reveals the row (`requestReveal`), the panel stays; Undo and
  Dismiss as in the window. Open in Window closes the panel and opens the window at the chip of the open.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`, `should offer the Undo of a squash while the toast's lasts, in the panel`, `should keep the landed report after a sync hid the island, in the panel`)

### Window

- Open in Window of the side panel, and every entry point where the panel has no room, open the window `activity`, or
  bring the open one to the front; an open starts over with its chip and an empty search.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)
- The window shows the log of the repository the main window shows, and follows a switch of it; its title names the
  repository ("Activity — branch-deck"). It reads the log itself (Log) and shows once its first read answered. The
  main window tells it what only the main window knows: the branches of its table, the commits of the undismissed
  landed report, the links of the landed branches, the GitHub repository the pull requests of the entries link in and
  the Undo of this session's rewrites by entry id, whenever one of them or the selection changes
  (`@/lib/activity-window-link`, the window link of [Branch Review](../branch/branch-review.spec.md)); no entry goes
  over the link.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should follow the repository the main window shows`, `should clear the log of the repository`)
  [@test] ../../src/stores/activity.test.ts (`gives the Activity window the Undo of the main window's rewrites by entry id`)
- What the window does runs in the main window, which refuses a request of a repository it no longer shows: a branch
  name brings the main window to the front and reveals the row (`requestReveal`), the window stays; Undo runs the undo
  of this session's rewrite (`canUndoActivity`: "Cannot undo …: its time ran out" past the window); Dismiss and Clear
  Log. A refusal is an error toast in the window.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`, `should offer the Undo of a squash while the toast's lasts, in the window`)
- The chips All, Robots (Patronus), Pushes, Updates (an update and what it landed), Reviews, Rewrites and Errors, with
  their counts of the entries the search keeps, one at a time; the chosen one again goes back to All; a chip shows
  while it counts an entry or is chosen. The search keeps the entries whose title or description has its text,
  ignoring case (`filterActivity`). Nothing kept: "No match for “…”" (or "Nothing of this kind") with Clear Filter.
  [@test] ../../src/lib/activity-window.test.ts (`keeps the entries of the chip whose title or description has the text, ignoring case`, `counts the entries of each chip that have the text, and of all`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)
- After them the Agents chip, on or off, off at each open: the operations of the agents show only while it is on,
  whatever chip is chosen. It shows while it counts an entry or is on; its count is the agents' entries the chosen chip
  and the search keep (what a press adds), and the other chips count the agents' only while it is on. A log of only
  the agents' operations says "Only the operations of agents" with Show Agents, which turns the chip on.
  [@test] ../../src/lib/activity-window.test.ts (`shows the operations of the agents only behind their chip, which counts what its press adds`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)
- An entry of an operation (`operationId`) has Copy Diagnostics: it copies the title, the description, the time it
  ended with `operation_id=<id>`, the field of its lines in the app's log, and the worktree of an agent's.
  [@test] ../../src/lib/activity-window.test.ts (`copies the words of an entry, when it ended and its operation as the lines of the log name it, with the worktree of an agent`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)

### Landed entries

- A landed entry keeps the report: "What landed in origin/master" expands to the rows of the Landed island
  (`LandedBranchRow`), each expanding to its commits. While its commits are in the undismissed report (their chips and
  the "New" marks show, the island may be hidden since, see [Landed Commits](landed-commits.spec.md)), "Dismiss" clears
  them as the island's Dismiss.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

## User Experience

- The button is a ghost `icon-sm` `HistoryIcon` after the landing indicator, named "Activity" with the tooltip
  "Activity" (an icon-only button, [UI Theme](../../docs/design/ui-theme.md)); it is hidden while the log of the
  repository is empty. The header stays on one line from 960 px.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)
- The side panel is an island (`Card`) on the canvas beside the islands of the main window, 8px from them, from the
  window edge and from the bottom, under the header: 24rem until its divider (the 8px gap) is dragged, at least 320 px,
  at most half the area. Its 40px header has "Activity", Open in Window (`AppWindowIcon`, `xs`) and Close; under it the
  search (`w-full`, without `/`, which stays the search of the view that shows) and the chips, then the log, which scrolls.
- The window opens at 560 × 720, at least 440 × 400 (wider above a zoom of 110%: `sub_window_min_size`); "Activity" in
  its title bar. On the canvas the search (`SearchField`, `w-60`, `/` focuses it), the chips (`FilterChips`, the
  stock `ToggleGroup`, as the filter bar of the branch table) and the Agents chip (`FilterToggle`, a `Toggle` in their
  look, its tooltip "The operations of agents, in the worktrees of the lease pool"), below them the log in one `Card`
  island, a mark of an unavailable journal first (`TriangleAlertIcon` in `warning`, the path in `font-mono`), the entries newest first grouped by day
  ("Today", "Yesterday", the date) with the day headings sticky on `bg-card`, then on the canvas the footer with Clear
  Log and "Esc Close".
  [@test] ../../src/stores/activity.test.ts (`groups by local day: today, yesterday, then the date`)
  [@test] ../../src/lib/zoom.test.ts (`is smaller still for Activity, and wider above 110%`)
- An entry: the icon of its kind (`UploadIcon` push, `ArrowDownIcon` update, `ShieldCheckIcon` robot, `PenLineIcon`
  rewrite, `UserCheckIcon` review request), `PackageCheckIcon` in `info` for what landed, `TriangleAlertIcon` in `warning` for a refusal, a dropping
  push or a moved remote, `OctagonXIcon` in `destructive` for an error; the title, the description muted, the relative
  time (`RelativeTime`); before the time an agent's "Agent" (`Badge` `outline`, muted) and, on hover or keyboard
  focus, Copy Diagnostics (`CopyButton`, `CopyIcon`, "Copied" in its tooltip once it copied). Commit subjects show
  through `CommitSubjectLine`, with their issue links.
- A word of the title or description that names a branch of the table is a button: it reveals the row in the main
  window, as a branch name of the landing overview.
- The rows of a landed report have the pull request ("#7", "Open Pull Request") and the robot (`ShieldCheckIcon`, "Open
  in Patronus") of the branch as links, the actions of the chips of the island, whose state is the main window's.

## Testing / Local Run

- `pnpm test:unit`: `src/stores/activity.test.ts` (over the journals in memory, `src/test-support/fake-activity.ts`),
  `src/presenters/activity.test.ts` (the words of the entries of the journal), `src/lib/activity-window.test.ts` (the
  chips and the search), `src/lib/push-summary.test.ts` (the words of a push).
- `cargo nextest run -p operations -E 'test(/journal_test/)'` and `-p app-settings -E 'test(/journals_test/)'`: the
  journals, their reads and Clear Log.
- `pnpm e2e`: `tests/e2e/activity/activity.spec.ts` with the templates `issue_links` (pushes to the remote of the repository,
  a force push over someone else's commit there, a squash), `simple` (a second repository), `behind_upstream`, `update_conflict` and `landed_upstream`. The window is a
  linked page of its own (`openActivityWindow` of `tests/e2e/helpers/activity-helpers.ts`, `openLinkedSubWindow`), opened
  by Open in Window of the side panel unless a test opens it otherwise. Test ids: `activity-button`, `activity-panel`,
  `activity-panel-window`, `activity-panel-close`, `activity-window`, `activity-filter`, `activity-list`, `activity-empty`,
  `activity-day`, `activity-entry` with `activity-title`, `activity-description`, `activity-branch`, `activity-agent`,
  `activity-undo`, `activity-undone`, `activity-landed-toggle`, `activity-landed-dismiss`, `activity-landed-report`;
  `activity-unavailable` with `activity-unavailable-path`, `activity-clear`. The fixtures of the test server journal an agent's operation in a slot of
  the pool of the repository (`journalAgentOperation`, through the registry as `bd` does) and remember the journal of
  the repository on a disk that is gone (`loseJournalDisk`); the page mock tells the pages `activity-changed` after
  `record_activity`, `clear_activity` and `execute`, as the backend does, and a test fires it for an operation of `bd`.

## Open Questions / Risks

- A journal is read whole at each read of Activity: 200 entries with landed reports stay small (a few hundred KB at
  worst), but a much larger cap would not. A line this version can't read (a newer version's) is skipped, never the
  journal.
- A read of a journal that takes longer than 200 ms under load shows what the journal answered before (unavailable
  when it never answered) until its answer comes, which tells the windows to read again.
- A cleared log records a still undismissed report again at the next sync, as "Found by a sync".

## References

- [Landed Commits](landed-commits.spec.md): the island and its lifetime.
- [Outgoing Commits](outgoing.spec.md), [Update Project](update-project.spec.md), [Patronus](../branch/patronus.spec.md),
  [Squash](../commit/squash.spec.md).
- [Distributed Settings](../../docs/design/distributed-settings.md): persistent and main-only stores.
