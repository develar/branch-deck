---
name: Activity
description: Requirements for the log of what Branch Deck did in a repository (pushes, updates, landed reports, Patronus robots, pull request retargets, history rewrites, errors) and the review requests it told about, opened from the header, with the Undo of a rewrite while its toast's lasts, in a side panel of the main window or a window of its own, with the chips of the kinds and a search.
style: plain-1
targets:
  - src/stores/activity.ts
  - src/presenters/activity.ts
  - crates/operations/src/journal.rs
  - crates/operations/src/registry_watch.rs
  - crates/app-settings/src/journals.rs
  - crates/app-settings/src/file.rs
  - crates/domain-types/src/activity.rs
  - crates/app-api/src/app/activity.rs
  - crates/app-api/src/events.rs
  - crates/app-api/src/session/view.rs
  - crates/test-server/src/host.rs
  - src/components/repository/ActivityButton.tsx
  - src/components/repository/ActivityLog.tsx
  - src/components/shared/FilterChips.tsx
  - src/pages/ActivityPage.tsx
  - src/components/repository/ActivityPanel.tsx
  - src/components/repository/ActivityPanelContent.tsx
  - src/hooks/useActivityPanel.ts
  - src/stores/activity-panel.ts
  - src/lib/input-modality.ts
  - src/pages/IndexPage.tsx
  - src/stores/settings/appSettings.ts
  - src/components/shared/SearchField.tsx
  - src/lib/activity-window.ts
  - src/lib/activity-window-link.ts
  - src/hooks/useActivityWindowLink.ts
  - src/hooks/useActivityReads.ts
  - src/hooks/useMainWindowLink.ts
  - src/lib/main-menu.ts
  - src-tauri/src/menu.rs
  - src-tauri/src/zoom.rs
  - src/lib/zoom.ts
  - src-tauri/capabilities/sub-windows.json
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/LandedCard.tsx
  - src/components/branch/DroppedCommitList.tsx
  - src/lib/undo-toast.ts
  - src/model/store.ts
  - crates/repository-model/src/table.rs
  - crates/operations/src/rewrite.rs
  - src/lib/errors.ts
  - crates/app-api/src/operation.rs
  - crates/domain-types/src/error.rs
  - crates/app-api/src/api_error.rs
  - src/lib/landed.ts
  - src/lib/push-run.ts
  - src/presenters/pull-request-url.ts
  - src/lib/robot-run.ts
  - src/presenters/actions.ts
  - src/lib/push-summary.ts
  - src/stores/patronus.ts
  - src/stores/review-requests.ts
  - src/lib/review-requests.ts
  - src/stores/update-project.ts
  - src/stores/landed.ts
  - src/lib/landed-run.ts
  - src/model/intents.ts
  - src/lib/remote-status.ts
  - src/presenters/rewrite.ts
  - src/presenters/outcome.ts
---

# Activity

Status: Implemented
Date: 2026-10-15

## Purpose

What Branch Deck did lives in toasts of 5 to 10 seconds: the push that dropped remote commits, the update, what landed,
the Safe Push the user started, the squash and its Undo. Activity keeps them: a `HistoryIcon` button in the header opens
a side panel beside the branch tables (or a window of its own) with the log of the selected repository, newest first
by day, each entry with what happened, the detail and when; chips of the kinds and a search find an entry in a month of
them. A branch name shows its row, a landed entry
has the report of the island, a rewrite has its Undo while the toast's lasts. The log is read from journals: the
operations of the remote, of the app and of `brd`, are the journal of their repository, the rest the page records in the
journal of the app data directory; no window keeps it. It is the history the landing overview leaves out: the overview
tells the present, its line of the last day opens Activity at its Robots.

## Goals

- Answer "what did I push yesterday", "what did the update do", "which robot did I start" after the toast is gone.
- One record of each event, in the words of its toast: Activity adds no news of its own.
- Undo from Activity exactly as from the toast, for as long, once.

## Non-goals

- No history of the repository (that is `git reflog`): only what this app and `brd` did, as the app says it.
- No sync and no fetch of their own in the log: they change nothing the user asked for, a failure is an error entry.
- No Undo after the toast's window, once HEAD moved since or after a restart: the rewrite it would revert may be pushed
  or rewritten again, and its undo moves the refs back only from where the rewrite left them.

## Requirements

### Requirement: Recording: The log is read from journals, never kept by a window

What Branch Deck did MUST be read from journals, never kept by a window.

#### Scenario: The log of each repository comes from the journals
- **WHEN** the log is read
- **THEN** it comes from the journals of the repositories and the page's
- **AND** each repository's log stands apart, newest first
  [@test] src/stores/activity.test.ts (`reads the journals of the repositories and the page's, each repository's log apart, newest first`)

#### Scenario: A reload reads the log again
- **GIVEN** an operation of a repository recorded in its journal
- **WHEN** the window reloads
- **THEN** the log is read from the journal of the repository
  [@test] tests/e2e/activity/activity.spec.ts (`should read the log from the journal of the repository after a reload`)

### Requirement: Recording: Every operation is the journal of its repository

Every operation of the remote, of the app and of `brd` alike (a push, a start of a Safe Push or a Dry Run, a restart
or a cancel of a robot, a retarget, a pull request created, closed or reviewed) MUST be the journal of its repository
(`operations::journal`, `$GIT_COMMON_DIR/branch-deck/journal.jsonl`): an entry once it ended, with each effect and
what it answered.

#### Scenario: An operation that ended is an entry of the journal of its repository
- **WHEN** an operation of the remote, of the app or of `brd` ends
- **THEN** the journal of its repository has an entry of it, with each effect and what it answered
- **AND** the entry shows in every project of that repository, the page's entries in theirs, newest first
  [@test] src/presenters/activity.test.ts (`puts the operations of a journal in every project of its repository, and the page's entries in theirs, newest first`)

### Requirement: Recording: The main window records what only the page knows

What only the page knows (an error it told, an update, what landed, a rewrite, a review request, an approval, the news
of a robot) the main window MUST record in the journal of the app data directory (`record_activity`, the same
writer), always with its repository; a sub-window MUST record nothing.

#### Scenario: The page records into its journal
- **WHEN** the main window records an event only the page knows
- **THEN** it is an entry of the journal of the app data directory, with its repository
  [@test] crates/operations/src/journal_test.rs (`the_page_records_into_its_journal_and_clear_log_forgets_the_log_of_a_repository_in_both`)

#### Scenario: Nothing is recorded without a repository
- **WHEN** the page would record an event without a repository
- **THEN** nothing is recorded
  [@test] src/stores/activity.test.ts (`records nothing without a repository`)

#### Scenario: A sub-window records nothing
- **WHEN** an event happens in a sub-window
- **THEN** the sub-window records nothing

### Requirement: Recording: An error shows in its repository or where its operation ran

An error MUST be recorded in the repository the main window shows when it is told, but one that names an operation of
a journal MUST show where that operation ran, in the projects of its journal.

#### Scenario: An error in the repository the main window shows
- **WHEN** the page tells an error that names no operation
- **THEN** the entry is in the repository the main window shows when it is told
  [@test] src/stores/activity.test.ts (`records the errors the user was told about, in the repository the main window shows`)

#### Scenario: An error of an operation shows where the operation ran
- **WHEN** the page tells an error that names an operation of a journal
- **THEN** the entry shows where that operation ran, in the projects of its journal
  [@test] src/presenters/activity.test.ts (`puts the error the page told of an operation where the operation ran, whichever repository the window showed then`)

### Requirement: Recording: Each event is an entry in the words of its toast

Each event MUST be an entry in the words its toast had: the toast and the entry take their title and description from
one function (`describePush`, `describeRobot`, `presentLandOutcome`, `appliedUpdateToast`; the entries of an operation
`presentActivity` of `src/presenters/activity.ts`).

#### Scenario: The toast and the entry share their words
- **WHEN** an event is told in a toast and recorded
- **THEN** the toast and the entry take their title and description from one function

### Requirement: Recording: An operation is an entry per event it made

An operation MUST be an entry per event it made: each branch it pushed (of a stack push each, of a whole stack its
stack branch), each pull request it retargeted, the robot it started; a push that is a step of another action (a Safe
Push start, a merge request) too, in the words its toast would have.

#### Scenario: A push of several branches
- **WHEN** an operation pushed a stack
- **THEN** each branch it pushed is an entry
- **AND** of a whole stack, its stack branch is the entry

#### Scenario: Retargets and a robot start
- **WHEN** an operation retargeted pull requests or started a robot
- **THEN** each pull request it retargeted and the robot it started is an entry

#### Scenario: A push that is a step of another action
- **WHEN** a push is a step of another action (a Safe Push start, a merge request)
- **THEN** it is an entry too, in the words its toast would have

### Requirement: Recording: A toast replaced in place leaves an entry per state

A toast whose later states replace it in place ([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event):
the merge, then the landing; the failure of a robot, then its restart) MUST leave an entry per state: Activity is the
log of the events, the toasts are one per event.

#### Scenario: The merge, then the landing
- **WHEN** a toast tells the merge, then is replaced in place by the landing
- **THEN** Activity has an entry of each state

### Requirement: Recording: A push says what it sent, dropped or replaced

A push MUST be an entry in the words of its toast: what it sent, of a force push the remote commits it dropped
(`DroppedCommitList`) or what it replaced, and of a remote that moved since its status that it moved and nothing was
pushed.

#### Scenario: A push
- **WHEN** a push sent a branch
- **THEN** the entry says "Pushed branch feature-auth" / "3 commits"
- **AND** a push that created the remote branch says "4 commits · created the remote branch"
  [@test] src/presenters/activity.test.ts (`says each branch a push sent from the status it went over, a force push with the remote commits it dropped`)

#### Scenario: A force push that dropped remote commits
- **WHEN** a force push dropped remote commits
- **THEN** the entry says "Force pushed ABC-123" / "1 commit · dropped 1 remote commit"
- **AND** it has the dropped remote commits (`DroppedCommitList`)
  [@test] src/presenters/activity.test.ts (`says each branch a push sent from the status it went over, a force push with the remote commits it dropped`)
  [@test] tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`)

#### Scenario: A force push that replaced older versions
- **WHEN** a force push replaced older versions of the commits
- **THEN** the entry says "replaced the rebased copies", "replaced 1 amended commit" or "replaced the parent's older commits"
  [@test] src/lib/push-summary.test.ts (`says what the push did from the status it was made against`)
  [@test] src/lib/push-summary.test.ts (`says that a push of an amended or restacked branch replaced the older versions, not that it dropped them`)

#### Scenario: A remote that moved since its status
- **WHEN** the remote moved since the status the push went over
- **THEN** the entry says so ("auth changed on the remote" / "… Nothing was pushed.")
  [@test] src/presenters/activity.test.ts (`says a remote that moved under the push of a branch, nothing pushed`)

### Requirement: Recording: The words of a push come from the status it found

The words of a push MUST come from the remote status the push found, which the journal keeps with the push
(`EffectRecord.remote`: the status the session showed, of Push Up to Here the part it sent, the one it went over once
more, what the remote has now when it moved).

#### Scenario: The words follow the status the push was made against
- **WHEN** a push is described
- **THEN** its words follow the remote status it was made against
  [@test] src/lib/push-summary.test.ts (`says what the push did from the status it was made against`)

#### Scenario: The status a push went over once more
- **WHEN** a push goes over a moved remote once more
- **THEN** the journal keeps the status it went over with the push
  [@test] crates/operations/src/push_test.rs (`pushes_again_over_a_moved_remote_only_when_the_remote_has_no_commits_the_branch_lacks`)

### Requirement: Recording: An update applied or refused is an entry

An update MUST be an entry in the words of its toast, applied or refused; an update with nothing to do MUST NOT be an
entry.

#### Scenario: An applied update
- **WHEN** an update is applied
- **THEN** the entry has the toast's words ("Updated master: rebased 1 commit onto origin/master", or what landed
  first, with the report)
  [@test] tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)

#### Scenario: A refused update
- **WHEN** an update is refused
- **THEN** the entry says "Didn't update master" / "Conflicts in 1 file: src/app.txt. Nothing was changed."
  [@test] src/stores/activity.test.ts (`records a refused update, not one with nothing to do`)
  [@test] tests/e2e/activity/activity.spec.ts (`should record a refused update`)

#### Scenario: An update with nothing to do
- **WHEN** an update has nothing to do
- **THEN** it is no entry
  [@test] src/stores/activity.test.ts (`records a refused update, not one with nothing to do`)

### Requirement: Recording: A landed report is recorded once

A landed report MUST be recorded once: by the update that brought it, or by the sync after an update made elsewhere
(the commits no entry has yet, "Found by a sync"), also while the record of the update is on its way.

#### Scenario: The update brings the report
- **WHEN** an update brings a landed report and the sync after it sends the same report
- **THEN** the report is recorded once, by the update
  [@test] src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)

#### Scenario: The sync sends it while the record of the update is on its way
- **WHEN** the sync after the update sends the report before the record of the update answered
- **THEN** the report is still recorded once
  [@test] src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)

#### Scenario: A sync after an update made elsewhere
- **WHEN** a sync finds commits landed by an update made elsewhere
- **THEN** the commits no entry has yet are an entry, "Found by a sync"
  [@test] src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)

### Requirement: Recording: A start, restart or cancel of a robot is its operation

A Patronus robot started, restarted or canceled MUST be the entry of the operation of the journal, with "Open in
Patronus".

#### Scenario: A robot started
- **WHEN** a start started a robot
- **THEN** the entry says "Safe Push started" / "auth → master · profile Large Scale Change", or the comment Patronus
  accepted
- **AND** it has "Open in Patronus"
  [@test] src/presenters/activity.test.ts (`says the robot a start started in the words of its toast, and the comment Patronus accepted`)

#### Scenario: A robot restarted or canceled
- **WHEN** a robot was restarted or canceled
- **THEN** the entry says the restart, or the cancel ("Safe Push is canceling" / "auth → master")
  [@test] src/presenters/activity.test.ts (`says a restart and a cancel of a robot, and the pull request a creation opened`)

### Requirement: Recording: The news of a robot is recorded by the page

A Patronus robot failing, failed or merged MUST be the news of the session, which the page records.

#### Scenario: Failing, failed or merged
- **WHEN** the session tells a robot failing ("A required check of auth failed"), failed ("Safe Push of auth failed" /
  "Fast Tests · auth → master", [Patronus](../../branch/patronus/spec.md#failed)) or merged ("auth merged into master"
  / "Safe Push")
- **THEN** the page records it, with the page of the robot
  [@test] src/stores/activity.test.ts (`records the news of a robot with its page`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and start a session in the IDE from the toast, with Copy Prompt for Agent as the second option`)

### Requirement: Recording: A new review request is an entry per request

A new review request ([Review Requests](../review-requests/spec.md#new-requests)) MUST be one entry per request, also
when one toast told several, of kind `review`, with "Open Pull Request", the name of the action everywhere
([UX Writing](../../../../docs/contributing/ux-writing.md#actions)).

#### Scenario: Requests told in one toast
- **WHEN** one toast tells several new review requests
- **THEN** each is an entry: "dave requested your review on #104" / its title, kind `review`, with "Open Pull Request"
  [@test] src/stores/review-requests.test.ts (`tells the requests the session tells new in one toast with Review, unread until opened, recorded in Activity`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)

### Requirement: Recording: An approval that made a branch ready is an entry

An approval of the user's pull request that made a branch ready ([Review Requests](../review-requests/spec.md#approval-news))
MUST be an entry of kind `review`, with "Open Pull Request".

#### Scenario: An approval arrives with a poll
- **WHEN** an approval of the user's pull request made a branch ready
- **THEN** the entry says "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform", kind `review`, with "Open
  Pull Request"
  [@test] tests/e2e/next-step/next-step.spec.ts (`should tell the approval that arrives with a poll, with Safe Push of the branch first`)

### Requirement: Recording: A pull request retargeted, created, closed or reviewed is an entry

A pull request the app retargeted from a virtual branch to the baseline ([Stacked Branches](../../branch/stacked-branches/spec.md#pull-requests)),
one it created, one the user closed and a review the user submitted MUST be entries of kind `review`, with "Open Pull
Request", a link to the pull request in the GitHub repository of the repository.

#### Scenario: A retarget out of a virtual branch
- **WHEN** the app retargeted a pull request from a virtual branch to the baseline
- **THEN** the entry says "Retargeted #802 to master" / "login · it went into auth", with the pull request on GitHub
  [@test] src/presenters/activity.test.ts (`says the retarget of a pull request out of a virtual branch, with the pull request on GitHub`)
  [@test] crates/test-server/src/github_host_test.rs (`a_push_retargets_the_pull_request_of_its_branch_out_of_a_virtual_branch_first`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

#### Scenario: A pull request created
- **WHEN** the app created a pull request
- **THEN** the entry says "Created pull request #12" / "auth → master · review requested from bob"
  [@test] src/presenters/activity.test.ts (`says a restart and a cancel of a robot, and the pull request a creation opened`)

#### Scenario: A close the user asked and a review
- **WHEN** the user closed a pull request or submitted a review
- **THEN** the entry says "Closed pull request #12" / "#12 no longer matches the air stack: auth landed", or "Approved #7"
  [@test] src/presenters/activity.test.ts (`says the close the user asked and a review, not the closes of a merged robot`)

### Requirement: Recording: The closes of a merged robot are states of its merge

The closes of a merged robot MUST be states of its merge, not entries of their own.

#### Scenario: A merged robot closes pull requests
- **WHEN** a merged robot closes pull requests
- **THEN** the closes are no entries of their own: they are states of its merge
  [@test] src/presenters/activity.test.ts (`says the close the user asked and a review, not the closes of a merged robot`)

### Requirement: Recording: A rewrite with an Undo toast is an entry of its repository

A history rewrite with an Undo toast (squash, fixups, move, ungroup, rename, stack, unstack, reword) MUST be an
entry, in the repository it rewrote.

#### Scenario: A reword with its Undo
- **WHEN** a reword ran and its toast offers Undo
- **THEN** the reword is an entry of the repository it rewrote, with its Undo for the window of the toast, once
  [@test] src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`)

### Requirement: Recording: Every error toast is an entry

Every error toast (`notifyError`, with its reason) MUST be an entry.

#### Scenario: An error the user was told about
- **WHEN** the page tells an error in a toast
- **THEN** it is an entry, with its reason, in the repository the main window shows
  [@test] src/stores/activity.test.ts (`records the errors the user was told about, in the repository the main window shows`)

### Requirement: Recording: A failed operation of the remote is its error toast's entry

An operation of the remote that failed MUST be the entry of its error toast, which names it (`operationId`), in the
projects of the journal of the operation, whichever repository the main window showed when the toast came.

#### Scenario: A Safe Push fails while another repository is shown
- **GIVEN** the main window shows another repository
- **WHEN** a Safe Push of one repository fails and its error toast names the operation
- **THEN** the entry is in the log of the first repository only
  [@test] src/presenters/activity.test.ts (`puts the error the page told of an operation where the operation ran, whichever repository the window showed then`)

#### Scenario: The page recorded the error of its toast
- **WHEN** an operation failed and the page recorded the error of its toast
- **THEN** the error of the toast is its entry, not a second one of the operation
  [@test] src/presenters/activity.test.ts (`says an operation that failed unless the page recorded the error of its toast, and leaves out the canceled`)

### Requirement: Recording: A failed operation no toast told says why it failed

A failed operation no toast told (of `brd`) MUST say what it was for and why it failed; a canceled one MUST NOT be an
entry.

#### Scenario: A failed operation of `brd`
- **WHEN** an operation of `brd` failed and no toast told it
- **THEN** its entry says what it was for and why it failed ("Cannot push branch auth" / "The remote refused the push: …")
  [@test] src/presenters/activity.test.ts (`says an operation that failed unless the page recorded the error of its toast, and leaves out the canceled`)

#### Scenario: A canceled operation
- **WHEN** an operation was canceled
- **THEN** it is no entry
  [@test] src/presenters/activity.test.ts (`says an operation that failed unless the page recorded the error of its toast, and leaves out the canceled`)

### Requirement: Recording: A rewrite or a pick of `brd` is a rewrite entry

A rewrite or a pick of `brd` (its journal entry, `tool: bd`), which no toast told, MUST be an entry of kind `rewrite`
in the words of the rewrite (`presentRewrite`), an agent's marked as any other; the journal entry of a rewrite of the
app MUST NOT be an entry (what it did is the entry the page records with its toast, above); an update of `brd` tells
nothing of what it moved and MUST NOT be an entry.

#### Scenario: A rewrite and a pick of `brd`
- **WHEN** the journal has a rewrite or a pick of `brd`
- **THEN** it is an entry of kind `rewrite` in the words of the rewrite ("Unstacked beta", "Picked commits")
- **AND** an agent's is marked as any other
- **AND** the journal entry of a rewrite of the app is no entry
- **AND** an update of `brd` is no entry
  [@test] src/presenters/activity.test.ts (`says a rewrite and a pick of bd, which no toast told, an agent's marked, and not the rewrites of the app nor an update of bd`)

### Requirement: Recording: Clear Log forgets the errors of the operations of its journal

Clear Log of a repository MUST forget the errors the page told of the operations of its journal too.

#### Scenario: Clear Log with errors of the journal's operations
- **GIVEN** the page told errors of operations of the journal of a repository
- **WHEN** the user clears the log of that repository
- **THEN** those errors are forgotten too
  [@test] crates/operations/src/journal_test.rs (`clear_log_forgets_the_errors_the_page_told_of_the_operations_of_the_journal`)

### Requirement: Recording: The entry of a command's error keeps its operation

The entry of an error a command returned MUST keep the operation it names as `operationId`, the key to the command's
lines in the app's log. An entry of an operation of the remote is its id (the `operation_id` of its log lines, of its
entry in the journal, of the JSON of `brd`); Copy Diagnostics of the entry copies it (below); an error raised outside
a command has none.

#### Scenario: A failed command names its operation
- **WHEN** the page records the error a command returned
- **THEN** the entry keeps the operation it names as `operationId`, for its lines in the log
  [@test] src/stores/activity.test.ts (`keeps the operation a failed command names, for its lines in the log`)

#### Scenario: An error raised outside a command
- **WHEN** an error was raised outside a command
- **THEN** it names no operation
  [@test] src/lib/errors.test.ts (`returns the operation an ApiError names, and nothing for any other error`)
  [@test] src/stores/activity.test.ts (`keeps the operation a failed command names, for its lines in the log`)

### Requirement: Recording: Every command that can fail runs its body as an operation

Every command that can fail MUST run its body as an operation, the `operation{operation_id=…}` span of
`crates/app-api/src/operation.rs`, so that its lines in the app's log name the id, those of a step it hands to another
thread too, and its error MUST carry the id (`ApiError.operation_id`).

#### Scenario: The scan of the commands
- **WHEN** the commands of app-api are scanned
- **THEN** every command has a span and a fallible one runs as an operation
- **AND** a body is one operation only without work outside it
- **AND** the body of a session command opens no span and no operation of its own
  [@test] crates/app-api/src/command_scan_test.rs (`every_command_has_a_span_and_a_fallible_one_runs_as_an_operation`)
  [@test] crates/app-api/src/command_scan_test.rs (`a_body_is_one_operation_only_without_work_outside_it`)
  [@test] crates/app-api/src/command_scan_test.rs (`the_body_of_a_session_command_opens_no_span_and_no_operation_of_its_own`)

#### Scenario: The lines of a command and its blocking steps name one operation
- **WHEN** a command hands a closure or a future to another thread
- **THEN** it carries the span of its caller, so the async lines of the command and its blocking step name one operation
  [@test] crates/git-executor/src/thread_span_scan_test.rs (`every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller`)
  [@test] crates/git-executor/src/thread_span_scan_test.rs (`the_scan_reads_the_argument_of_a_spawn_past_strings_nested_calls_and_line_breaks`)
  [@test] src-tauri/src/tracing_log_test.rs (`the_async_lines_of_a_command_and_its_blocking_step_name_one_operation`)

#### Scenario: The error of a command carries the id
- **WHEN** a command fails
- **THEN** its error carries the id of its operation (`ApiError.operation_id`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_repository_without_a_prefix_is_refused_with_the_api_error_of_the_app`)
  [@test] src-tauri/src/tracing_log_test.rs (`the_async_lines_of_a_command_and_its_blocking_step_name_one_operation`)

### Requirement: Recording: The app's log keeps the lines for days

The app's log MUST keep the lines for days (the retention as the comment of `LOG_FILE_BYTES` in
`src-tauri/src/lib.rs` describes it).

#### Scenario: The retention comment matches the plugin
- **WHEN** the lockfile pins the log plugin
- **THEN** the retention comment of `LOG_FILE_BYTES` describes that version
  [@test] src-tauri/src/log_rotation_test.rs (`the_log_retention_comment_describes_the_plugin_version_the_lockfile_pins`)

### Requirement: Recording: The page log line of an error carries its operation

The page log line of the error MUST carry the operation it names as `operation_id`.

#### Scenario: The backend log line of an ApiError
- **WHEN** the page logs an error that names an operation
- **THEN** the backend log line carries it as `operation_id`
  [@test] src/lib/errors.test.ts (`passes the operation an ApiError names to the backend log line, as operation_id`)

### Requirement: Recording: A page step throws a command's error as a commandFailure

A page step that throws what a command returned MUST throw it as a `commandFailure` (`src/lib/errors.ts`), which
keeps the id up to the `notifyError` of its caller; a review write MUST hand the error of its effect itself to its
feedback.

#### Scenario: A failed command thrown by a page step
- **WHEN** a page step throws what a command returned
- **THEN** it throws it with its message, its code and the operation it names
- **AND** no new Error of its message drops the operation
  [@test] src/lib/errors.test.ts (`throws what a command returned with its message, its code and the operation it names`)
  [@test] src/lib/errors.test.ts (`is what the page throws for a failed command: no new Error of its message drops the operation`)

#### Scenario: A failed review write
- **WHEN** an approval fails
- **THEN** the error of its effect goes to its feedback as the engine answered it
  [@test] src/stores/review-requests.test.ts (`hands the error of a failed approval to its feedback as the engine answered it, and starts nothing`)

### Requirement: Undo: A rewrite entry offers Undo for the window of its toast, once

A rewrite entry MUST show "Undo" (`xs`) for the 10 s of its toast. Either Undo, the toast's or the entry's, MUST revert
the rewrite once (a second press while it runs does nothing); after the window the button goes, and the entry says
what was done, as its title.

#### Scenario: Undo within the window of the toast
- **WHEN** a rewrite entry is shown within the 10 s of its toast
- **THEN** it shows "Undo" (`xs`), which reverts the rewrite once
- **AND** a second press while it runs does nothing
  [@test] src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`)
  [@test] tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts, in the window`, `should offer the Undo of a squash while the toast's lasts, in the panel`)

#### Scenario: After the window
- **WHEN** the window of the toast ended
- **THEN** the button goes and the entry says what was done, as its title
  [@test] src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`)

### Requirement: Undo: A reverted rewrite closes its toast and says undone

Once either Undo reverted the rewrite, the toast MUST close and the entry MUST say "undone".

#### Scenario: Undo from the toast
- **WHEN** the Undo of the toast undid the rewrite
- **THEN** the toast closes and the entry says "undone"
  [@test] src/stores/activity.test.ts (`marks a rewrite undone from its toast once its undo undid it`)

### Requirement: Undo: An Undo during another rewrite waits in the queue

An Undo pressed while another rewrite of the repository runs MUST wait for it in the queue
([Running Operations](../../../../docs/design/operations.md)), never refused.

#### Scenario: Undo of a move pressed while another rewrite runs
- **WHEN** the Undo of a move is pressed while another rewrite runs after it
- **THEN** the undo runs once that rewrite ended, the toast and its Undo kept until it undid
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should run the Undo of a move pressed while another rewrite runs after it, the toast and its Undo kept until it undid`)

### Requirement: Undo: A failed undo keeps its Undo

An undo that failed (its error toast says why) MUST leave the toast and the entry as they were, with their Undo, which
runs it again.

#### Scenario: The undo fails
- **WHEN** the undo of a rewrite fails
- **THEN** the toast and the entry keep their Undo
- **AND** a second press runs it again
  [@test] src/stores/activity.test.ts (`keeps the Undo of a rewrite whose undo failed, in its toast and in Activity: a second press runs it again`)

### Requirement: Undo: The session offers the Undo of the last rewrite while HEAD is its head

The Undo MUST be the session's (`RepositoryView.undo`): it offers the Undo of the last rewrite of this process that has
one, while HEAD is still the head that rewrite left, as the rewrites of this process and the syncs after it saw HEAD.

#### Scenario: HEAD is the head the rewrite left
- **WHEN** HEAD is still the head the last rewrite with an Undo left
- **THEN** the session offers its Undo
  [@test] crates/repository-model/src/table_test.rs (`the_undo_of_the_last_rewrite_is_offered_while_head_is_the_head_it_left`)
  [@test] crates/operations/src/rewrite_test.rs (`the_session_offers_the_undo_of_the_last_rewrite_while_head_is_the_head_it_left`)

### Requirement: Undo: A later rewrite or a move of HEAD takes the Undo back

A second rewrite MUST take its place (its own Undo, or none after an amend that moved HEAD), an undo leaves none, and
a commit, an update or a rewrite of `brd` MUST take it back once a sync read the HEAD it left; a sync that started
before the rewrite ended read HEAD before it and changes nothing.

#### Scenario: A second rewrite
- **WHEN** a rename runs after a move
- **THEN** the Undo of the move goes from its toast and Activity, and the rename's own Undo works
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should take the Undo of a move back from its toast and Activity once a rename ran after it, whose own Undo works`)
  [@test] crates/repository-model/src/table_test.rs (`the_undo_of_the_last_rewrite_is_offered_while_head_is_the_head_it_left`)
  [@test] crates/operations/src/rewrite_test.rs (`the_session_offers_the_undo_of_the_last_rewrite_while_head_is_the_head_it_left`)

#### Scenario: An amend that moved HEAD, an undo
- **WHEN** a rewrite without an Undo moved HEAD (an amend), or an undo ran
- **THEN** the session offers no Undo
  [@test] crates/repository-model/src/table_test.rs (`the_undo_of_the_last_rewrite_is_offered_while_head_is_the_head_it_left`)
  [@test] crates/operations/src/rewrite_test.rs (`the_session_offers_the_undo_of_the_last_rewrite_while_head_is_the_head_it_left`)

#### Scenario: A commit made outside the app
- **WHEN** a sync read a commit made outside the app after a move
- **THEN** the Undo of the move goes from its toast and Activity
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should take the Undo of a move back from its toast and Activity once a sync read a commit made outside the app`)
  [@test] crates/repository-model/src/table_test.rs (`the_undo_of_the_last_rewrite_is_offered_while_head_is_the_head_it_left`)
  [@test] crates/operations/src/rewrite_test.rs (`the_session_offers_the_undo_of_the_last_rewrite_while_head_is_the_head_it_left`)

#### Scenario: A sync that started before the rewrite ended
- **WHEN** a sync that started before the rewrite ended answers
- **THEN** it read HEAD before the rewrite and changes nothing of the Undo
  [@test] crates/repository-model/src/table_test.rs (`the_undo_of_the_last_rewrite_is_offered_while_head_is_the_head_it_left`)

### Requirement: Undo: An Undo the session took back goes as after the window

Once the session took the Undo back, it MUST go from the toast, which still tells what was done until its time ends,
and from the entry, as after the window; a press of the Activity window that came late MUST be refused with "Cannot
undo …: the branch moved since". An Undo pressed before is not taken back while it runs: its end decides.

#### Scenario: The session no longer offers it
- **WHEN** the session no longer offers the Undo of a rewrite
- **THEN** the Undo goes from the toast and the entry, as after its window
- **AND** a late press of the Activity window is refused with "Cannot undo …: the branch moved since"
  [@test] src/stores/activity.test.ts (`withdraws the Undo of a rewrite the session no longer offers, as after its window, but not while its undo runs`)

#### Scenario: An Undo that runs
- **WHEN** the session takes back an Undo that was pressed before and runs
- **THEN** it is not taken back while it runs: its end decides
  [@test] src/stores/activity.test.ts (`withdraws the Undo of a rewrite the session no longer offers, as after its window, but not while its undo runs`)

### Requirement: Undo: A window of another repository leaves the Undo as it is

A window that shows another repository MUST leave the Undo as it is: its undo checks the refs itself.

#### Scenario: A view of another repository
- **WHEN** a view of another repository comes
- **THEN** the Undo stays as the session of its repository offers it
  [@test] src/model/store.test.ts (`follows the Undo the session offers of a rewrite: withdrawn once a patch takes it back, by no view of another repository`)

### Requirement: Undo: The undo lives in the main window, not in the journal

The undo itself MUST live in the main window, by the id of the entry, not in the journal: after a restart no entry has
an Undo.

#### Scenario: A rewrite of an earlier session
- **WHEN** the log shows a rewrite of an earlier session
- **THEN** its entry has no Undo
  [@test] src/stores/activity.test.ts (`has no Undo for a rewrite of an earlier session`)

### Requirement: Log: The main window reads the journals remembered for the recent projects

The main window MUST read the log (`read_activity`): the journal of the repository of each recent project, which
`subscribe_view` remembered for the project when a window opened its session (`journalPaths` of the settings, by the
path of the project, written by Rust; no window writes it), so the read runs no git process. The journal of the page
comes with them.

#### Scenario: The recent projects and their journals
- **WHEN** the log is read
- **THEN** the recent projects have the journals remembered for them, and one opened before has none
  [@test] crates/app-settings/src/journals_test.rs (`the_recent_projects_have_the_journals_remembered_for_them_and_one_opened_before_has_none`)

#### Scenario: No recent projects
- **WHEN** there are no recent projects
- **THEN** there are no journals to read
  [@test] crates/app-settings/src/journals_test.rs (`no_recent_projects_have_no_journals`)

### Requirement: Log: Each journal is read once, bounded, and an unreadable one is unavailable

Each journal MUST be read once for the projects that share it (the worktrees of one repository), all in parallel on
the blocking pool, each waited for 200 ms (`ACTIVITY_READ_TIMEOUT`), then shown as it answered last; one whose
repository is gone, that can't be read or that never answered MUST be listed unavailable and the others show; a
journal its repository has not written yet is empty.

#### Scenario: A journal shared by worktrees, one that cannot be read
- **WHEN** the log of several projects is read, two of them worktrees of one repository, and one journal cannot be read
  or never answered (a disk that hangs)
- **THEN** the journal of each repository is read once
- **AND** the one it cannot read is listed "unavailable" and the others show
  [@test] crates/operations/src/journal_test.rs (`activity_reads_the_journal_of_each_repository_once_and_lists_one_it_cannot_read`)

#### Scenario: A repository that is gone, a journal not written yet
- **WHEN** the `$GIT_COMMON_DIR` of a repository is not there (a removed disk, a deleted repository)
- **THEN** its journal is unavailable
- **AND** a journal its repository has not written yet is empty
  [@test] crates/operations/src/journal_test.rs (`the_journal_of_a_repository_that_is_gone_is_unavailable_and_one_not_written_yet_is_empty`)

### Requirement: Log: One read of a journal runs at a time

One read of a journal MUST run at a time; a read asked while one runs waits for the next, which begins once that one
answered (the one that runs may have read the journal before the change the read was asked for), and the reads asked
meanwhile share it.

#### Scenario: A read asked while one runs
- **WHEN** a read is asked while one runs
- **THEN** it waits for the next read, which begins once that one answered, shared by the reads asked meanwhile
- **AND** it shows the journal as it is after that one
  [@test] crates/operations/src/journal_test.rs (`a_read_asked_while_one_runs_shows_the_journal_as_it_is_after_that_one`)

### Requirement: Log: A journal that answers late tells the windows

A read that answers after the wait with other entries MUST tell the windows (`activity-changed`), so a journal of a
slow disk shows once it answered, never "unavailable" for good because the machine was loaded.

#### Scenario: A journal that answers after the bound
- **WHEN** a journal answers after the 200 ms with other entries
- **THEN** the windows are told and it shows once it answered
  [@test] crates/operations/src/journal_test.rs (`a_journal_that_answers_after_the_bound_shows_once_it_answered`)

### Requirement: Log: Each window reads the log itself when it opens and when a journal changes

The main window and the Activity window MUST each read the log themselves (`useActivityReads`): when they open, and
again when a journal changes (`activity-changed`: a write of the page, or the watch of the operations registry of a
repository a window shows sees its journal change, an operation of the app or of `brd` ended); the main window also
after each operation and each write of its own.

#### Scenario: An operation of the main window ended
- **WHEN** an operation the main window ran ended
- **THEN** Activity reads the journal it ended in
  [@test] src/model/intents.test.ts (`runs the plan Rust prepared on its fingerprint, then waits for the view of what it changed; Activity reads the journal it ended in`)

#### Scenario: A journal changes
- **WHEN** a journal changes (`activity-changed`)
- **THEN** each window that is open reads the log again
  [@test] tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`, `should mark an unavailable journal`)

### Requirement: Log: An entry the main window recorded shows once, whichever answer comes first

An entry the main window recorded MUST show once and stay, whichever answer comes first: a read that brought it before
its record answered keeps it alone, and a read that answers after the record without it (made before the write) leaves
it, until a read asked after the record answered, which is the log (Clear Log forgets it).

#### Scenario: A read made after the write answers before the record
- **WHEN** the read the write of a rewrite asked (`activity-changed`) answers before the record of the rewrite
- **THEN** the entry shows once, and gets its Undo as the record answers
- **AND** its Undo undoes the rewrite
  [@test] src/stores/activity.test.ts (`shows an entry it recorded once when a read made after the write answered before the record`)
  [@test] tests/e2e/activity/activity.spec.ts (`should show a rewrite once whose record answers after the read its write asked, and undo it there`)

#### Scenario: A read made before the write answers after the record
- **WHEN** a read asked before the record of an entry answered answers after it, without the entry
- **THEN** the entry stays, until a read asked after the record answered
  [@test] src/stores/activity.test.ts (`keeps an entry it recorded when a read made before the write answers after the record, until a read made after it`)

### Requirement: Log: The log of a settings file of before is dropped

The log of a settings file of before (the key `activity`) MUST be dropped by the load.

#### Scenario: A file of before
- **WHEN** a settings file with the key `activity` is loaded
- **THEN** the log is dropped by the load and the next save
  [@test] crates/app-settings/src/file_test.rs (`the_activity_log_of_a_file_of_before_is_dropped_by_the_load_and_the_next_save`)

### Requirement: Log: The operations of the agents are marked and behind the Agents chip

The operations of the agents, whose worktree is a slot of the lease pool of the repository (`leaseSlot` of the entry,
by `leases.json` when it ended), MUST be entries as any other, marked "Agent" (its worktree in the tooltip), and show
only behind the Agents chip (Window).

#### Scenario: An operation of an agent
- **WHEN** an operation ended in a slot of the lease pool of the repository
- **THEN** it is an entry as any other, marked "Agent", its worktree in the tooltip
  [@test] src/presenters/activity.test.ts (`says an operation of an agent as any other, with the worktree of its slot of the lease pool`)

#### Scenario: Behind the Agents chip
- **WHEN** the log has operations of agent leases
- **THEN** they show only behind the Agents chip
  [@test] tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)

### Requirement: Log: A journal of the repository that cannot be read is marked

A journal of the repository the read could not read MUST be a mark above the log, in the window and in the panel, with
the path of the journal; a journal of another repository MUST mark nothing. Without other entries the mark stands for
"Nothing yet", and the header button stays.

#### Scenario: An unavailable journal of the repository
- **WHEN** the read could not read a journal of the repository
- **THEN** a mark above the log, in the window and in the panel, says "Cannot read the journal of the repository" /
  "Its operations show once it answers again." and the path of the journal
- **AND** without other entries the mark stands for "Nothing yet", and the header button stays
  [@test] src/stores/activity.test.ts (`lists the journals of a repository its read could not read, and none of another repository`)
  [@test] tests/e2e/activity/activity.spec.ts (`should mark an unavailable journal`)

#### Scenario: An unavailable journal of another repository
- **WHEN** the read could not read a journal of another repository
- **THEN** it marks nothing
  [@test] src/stores/activity.test.ts (`lists the journals of a repository its read could not read, and none of another repository`)

### Requirement: Log: Each journal keeps 200 entries of 30 days

Each journal MUST keep at most 200 entries, none older than 30 days; the oldest go first.

#### Scenario: More entries than the journal keeps
- **WHEN** a journal has more than 200 entries or entries older than 30 days
- **THEN** it keeps the newest entries of the last thirty days, the oldest going first
  [@test] crates/operations/src/journal_test.rs (`the_journal_keeps_the_newest_entries_of_the_last_thirty_days`)

### Requirement: Log: Clear Log forgets the whole log of the repository

"Clear Log" in the footer of the window MUST forget the whole log of the repository, whatever the filter shows (its
tooltip says it): the journal of its repository empties (`clear_activity`, under `operations.lock`) and the entries of
the page of it leave the journal of the page; the other repositories keep theirs.

#### Scenario: Clear Log
- **WHEN** the user presses "Clear Log" in the footer of the window
- **THEN** the journal of the repository empties and the page's entries of it leave the journal of the page
- **AND** the other repositories keep theirs
  [@test] crates/operations/src/journal_test.rs (`the_page_records_into_its_journal_and_clear_log_forgets_the_log_of_a_repository_in_both`)
  [@test] tests/e2e/activity/activity.spec.ts (`should clear the log of the repository`)

### Requirement: Log: After Clear Log the window says Nothing yet

After a clear the window MUST say "Nothing yet: what Branch Deck does in the repository shows here", Clear Log
disabled, and the header button stays, opening the empty log; a clear that fails MUST be an error toast in the window.

#### Scenario: The log was cleared
- **WHEN** the log of the repository was cleared
- **THEN** the window says "Nothing yet: what Branch Deck does in the repository shows here", Clear Log disabled
- **AND** the header button stays, opening the empty log
  [@test] tests/e2e/activity/activity.spec.ts (`should clear the log of the repository`)

#### Scenario: A clear that fails
- **WHEN** the clear fails
- **THEN** an error toast in the window says so

### Requirement: Side panel: The entry points toggle the panel through the main window

The header button, View → Activity of ☰ (custom title bar) and the native Window → Activity (macOS) or View →
Activity (Windows and Linux) MUST toggle the side panel of the main window; "Show in Activity" of the landing overview
opens it at its chip. Every entry point MUST ask the main window (`openActivity`, the app command `open_activity`),
which shows Activity.

#### Scenario: The header button
- **WHEN** the user presses the header button
- **THEN** the side panel of the main window opens, and a second press closes it
  [@test] tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`)

#### Scenario: The View menu of ☰ and the native menu
- **WHEN** the user chooses View → Activity of ☰, or Activity of the native menu
- **THEN** the main window toggles the side panel
  [@test] tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the View menu of ☰ and from the native menu`)

### Requirement: Side panel: An open names its chip and starts over

An open MUST name its chip ("Show in Activity": Robots; the others none, All) and start over with it and an empty
search.

#### Scenario: Show in Activity of the landing overview
- **WHEN** the user presses "Show in Activity" of the landing overview
- **THEN** Activity opens at the Robots chip, with an empty search
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: The other entry points
- **WHEN** another entry point opens Activity
- **THEN** it names no chip: All, with an empty search

### Requirement: Side panel: The panel shows the log of the selected repository

The panel MUST show the log of the selected repository only, with the chips, the Agents chip and the marks of the
window, and follow a switch of it.

#### Scenario: The marks of the window in the panel
- **WHEN** the panel is open and a journal of the repository is unavailable
- **THEN** the panel shows the mark of the window
  [@test] tests/e2e/activity/activity.spec.ts (`should mark an unavailable journal`)

#### Scenario: A switch of the repository
- **WHEN** the user switches the selected repository
- **THEN** the panel shows the log of the new one only

### Requirement: Side panel: The panel opens only where it has room

The panel MUST open only where the area under the header has room for it beside the islands: their minimum (768 CSS
px, `MIN_ISLANDS_WIDTH`) plus the panel's (320 px). Without the room an entry point MUST open the window `activity`
instead (or bring the open one to the front), and a panel left open shows again once the window is wide enough.

#### Scenario: A window at its minimum width
- **WHEN** an entry point opens Activity in a window narrower than 768 CSS px plus 320 px under the header
- **THEN** the window `activity` opens instead, or the open one comes to the front
  [@test] tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should undo a reword from Activity`)

#### Scenario: The window grows wide enough
- **GIVEN** a panel left open
- **WHEN** the window is wide enough again
- **THEN** the panel shows again

### Requirement: Side panel: Open or closed and the width are kept

Open or closed MUST be a setting (`appSettings.activityPanel`), kept across starts; the width of the panel MUST be kept
once the user dragged its divider (`useDefaultLayout`, as the sidebar of a review).

#### Scenario: A restart with the panel open
- **WHEN** the app starts again after the panel was left open
- **THEN** the panel is open
  [@test] tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`)

#### Scenario: A dragged divider
- **WHEN** the user dragged the divider of the panel
- **THEN** its width is kept

### Requirement: Side panel: The focus enters the panel only from the keyboard

The focus MUST go into the log of the panel only when the keyboard opened it (the last input of the main window was a
key, `isKeyboardInput`); a click and the native menu leave it where it is. Escape MUST close the panel only while the
focus is in it (the search takes its own Escape first) and give the focus back where it was before it came in; Close
(`XIcon`, "Close Activity") does the same.

#### Scenario: Opened by the keyboard, closed by Escape
- **WHEN** the keyboard opens the panel
- **THEN** the focus goes into its log
- **AND** Escape closes it and gives the focus back where it was before
  [@test] tests/e2e/activity/activity.spec.ts (`should take the focus into the panel only when the keyboard opened it, and give it back on Escape`)

#### Scenario: Opened by a click
- **WHEN** a click or the native menu opens the panel
- **THEN** the focus stays where it is
  [@test] tests/e2e/activity/activity.spec.ts (`should take the focus into the panel only when the keyboard opened it, and give it back on Escape`)

#### Scenario: Close
- **WHEN** the user presses Close (`XIcon`, "Close Activity")
- **THEN** the panel closes
  [@test] tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`)

#### Scenario: Close with the focus in the panel
- **WHEN** the user presses Close while the focus is in the panel
- **THEN** the focus goes back where it was before it came in, as on Escape

### Requirement: Side panel: What the panel does runs where it is

What the panel does MUST run where it is: a branch name reveals the row (`requestReveal`), the panel stays; Undo and
Dismiss as in the window. Open in Window MUST close the panel and open the window at the chip of the open.

#### Scenario: A branch name in the panel
- **WHEN** the user clicks a branch name in the panel
- **THEN** the row is revealed and the panel stays
  [@test] tests/e2e/activity/activity.spec.ts (`should toggle the side panel from the header button, show a branch from it, and remember it open`)

#### Scenario: Undo and Dismiss in the panel
- **WHEN** the user presses Undo or Dismiss of an entry in the panel
- **THEN** it runs as in the window
  [@test] tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts, in the panel`, `should keep the landed report after a sync hid the island, in the panel`)

#### Scenario: Open in Window
- **WHEN** the user presses Open in Window
- **THEN** the panel closes and the window opens at the chip of the open
  [@test] tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

### Requirement: Window: Open in Window and an entry point without room open the window

Open in Window of the side panel, and every entry point where the panel has no room, MUST open the window `activity`,
or bring the open one to the front; an open MUST start over with its chip and an empty search.

#### Scenario: An open starts over at its chip
- **GIVEN** the window `activity` shows a chip and a search
- **WHEN** an entry point opens it again
- **THEN** the open one comes to the front, starting over with the chip of the open and an empty search
  [@test] tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

### Requirement: Window: The window shows the log of the repository the main window shows

The window MUST show the log of the repository the main window shows, and follow a switch of it; its title names the
repository ("Activity — branch-deck"). It MUST read the log itself (Log) and show once its first read answered.

#### Scenario: The main window switches the repository
- **WHEN** the main window switches to another repository
- **THEN** the window shows the log of that repository, its title naming it ("Activity — branch-deck")
  [@test] tests/e2e/activity/activity.spec.ts (`should follow the repository the main window shows`, `should clear the log of the repository`)

#### Scenario: The window opens
- **WHEN** the window opens
- **THEN** it reads the log itself and shows once its first read answered

### Requirement: Window: The main window tells the window what only it knows

The main window MUST tell the window what only the main window knows: the branches of its table, the commits of the
undismissed landed report, the links of the landed branches, the GitHub repository the pull requests of the entries
link in and the Undo of this session's rewrites by entry id, whenever one of them or the selection changes
(`@/lib/activity-window-link`, the window link of [Branch Review](../../branch/branch-review/spec.md)); no entry goes
over the link.

#### Scenario: The Undo of the main window's rewrites
- **WHEN** a rewrite of this session offers its Undo
- **THEN** the main window gives the Activity window the Undo by entry id, no entry going over the link
  [@test] src/stores/activity.test.ts (`gives the Activity window the Undo of the main window's rewrites by entry id`)

### Requirement: Window: What the window does runs in the main window

What the window does MUST run in the main window, which refuses a request of a repository it no longer shows: a branch
name brings the main window to the front and reveals the row (`requestReveal`), the window stays; Undo runs the undo of
this session's rewrite (`canUndoActivity`); Dismiss and Clear Log. A refusal MUST be an error toast in the window.

#### Scenario: A branch name in the window
- **WHEN** the user clicks a branch name of an entry in the window
- **THEN** the main window comes to the front and reveals the row, the window stays
  [@test] tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`)

#### Scenario: Undo in the window
- **WHEN** the user presses Undo of an entry in the window
- **THEN** the main window runs the undo of this session's rewrite
  [@test] tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts, in the window`)

#### Scenario: An Undo the main window refuses
- **WHEN** the Undo is pressed past the window of its toast, or once the session took it back
- **THEN** an error toast in the window says "Cannot undo …: its time ran out", or "…: the branch moved since"
  [@test] src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`, `withdraws the Undo of a rewrite the session no longer offers, as after its window, but not while its undo runs`)

#### Scenario: A request of a repository the main window no longer shows
- **WHEN** the window asks for a repository the main window no longer shows
- **THEN** the main window refuses it with an error toast in the window, "Cannot do it from Activity" / "the main window
  shows another repository now"

### Requirement: Window: The chips of the kinds count what the search keeps

The window MUST have the chips All, Robots (Patronus), Pushes, Updates (an update and what it landed), Reviews,
Rewrites and Errors, with their counts of the entries the search keeps, one at a time; the chosen one again goes back
to All; a chip shows while it counts an entry or is chosen.

#### Scenario: The counts of the chips
- **WHEN** the window shows the log with a search
- **THEN** each chip counts the entries of its kind that have the text, and All counts all
  [@test] src/lib/activity-window.test.ts (`counts the entries of each chip that have the text, and of all`)

#### Scenario: A chip chosen and chosen again
- **WHEN** the user chooses a chip, then chooses it again
- **THEN** the window shows the entries of that chip only, then goes back to All
  [@test] tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

### Requirement: Window: The search keeps the entries that have its text

The search MUST keep the entries whose title or description has its text, ignoring case (`filterActivity`). Nothing
kept: "No match for “…”" (or "Nothing of this kind") with Clear Filter.

#### Scenario: The search with a chip
- **WHEN** the user types a text in the search with a chip chosen
- **THEN** the window keeps the entries of the chip whose title or description has the text, ignoring case
  [@test] src/lib/activity-window.test.ts (`keeps the entries of the chip whose title or description has the text, ignoring case`)
  [@test] tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

#### Scenario: Nothing kept
- **WHEN** the search or the chip keeps no entry
- **THEN** the window says "No match for “…”" (or "Nothing of this kind") with Clear Filter
  [@test] tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

### Requirement: Window: The Agents chip shows the operations of the agents

After them the Agents chip MUST be on or off, off at each open: the operations of the agents show only while it is on,
whatever chip is chosen. It shows while it counts an entry or is on; its count is the agents' entries the chosen chip
and the search keep (what a press adds), and the other chips count the agents' only while it is on.

#### Scenario: The Agents chip off and on
- **WHEN** the log has operations of agents and the Agents chip is off
- **THEN** they do not show, and the chip counts what its press adds
- **AND** once it is on they show, whatever chip is chosen, and the other chips count them
  [@test] src/lib/activity-window.test.ts (`shows the operations of the agents only behind their chip, which counts what its press adds`)
  [@test] tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)

### Requirement: Window: A log of only the agents' operations offers Show Agents

A log that shows no entry, with no chip chosen and no search, while the Agents chip is off and counts the agents'
operations, MUST say "Only the operations of agents" with Show Agents, which turns the chip on; with a chip or a search
it says what "Nothing kept" says.

#### Scenario: Only the operations of agents
- **WHEN** the log has only operations of agents, the Agents chip is off, no chip is chosen and the search is empty
- **THEN** the window says "Only the operations of agents" with Show Agents, which turns the chip on

### Requirement: Window: Copy Diagnostics of an entry of an operation

An entry of an operation (`operationId`) MUST have Copy Diagnostics: it copies the title, the description, the time it
ended with `operation_id=<id>`, the field of its lines in the app's log, and the worktree of an agent's.

#### Scenario: Copy Diagnostics
- **WHEN** the user presses Copy Diagnostics of an entry of an operation
- **THEN** it copies the words of the entry, when it ended and `operation_id=<id>`, with the worktree of an agent's
  [@test] src/lib/activity-window.test.ts (`copies the words of an entry, when it ended and its operation as the lines of the log name it, with the worktree of an agent`)
  [@test] tests/e2e/activity/activity.spec.ts (`should hide the operations of agent leases behind the Agents chip`)

### Requirement: Landed entries: A landed entry keeps the report

A landed entry MUST keep the report: "What landed in origin/master" expands to the rows of the Landed island
(`LandedBranchRow`), each expanding to its commits.

#### Scenario: The report after the island is hidden
- **GIVEN** a sync hid the Landed island
- **WHEN** the user expands "What landed in origin/master" of the landed entry, in the window or in the panel
- **THEN** it shows the rows of the Landed island (`LandedBranchRow`), each expanding to its commits
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

### Requirement: Landed entries: Dismiss clears the commits of the undismissed report

While its commits are in the undismissed report (their chips and the "New" marks show, the island may be hidden since,
see [Landed Commits](../landed-commits/spec.md)), the landed entry MUST offer "Dismiss", which clears them as the
island's Dismiss.

#### Scenario: Dismiss of a landed entry
- **GIVEN** the commits of the entry are in the undismissed report, the island hidden by a sync
- **WHEN** the user presses "Dismiss" of the entry
- **THEN** it clears them as the island's Dismiss
  [@test] tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island, in the window`, `should keep the landed report after a sync hid the island, in the panel`)

## User Experience

- The button is a ghost `icon-sm` `HistoryIcon` after the landing indicator, named "Activity" with the tooltip
  "Activity" (an icon-only button, [UI Theme](../../../../docs/design/ui-theme.md)). It is there whenever a repository is
  selected, its log empty too, which it opens with "Nothing yet", as the menus and the palette do: the first entry
  moves nothing in the header (a button that came with it narrowed the repository selector, which gives way to the
  actions at the minimum width beside the traffic lights). The header stays on one line from 960 px.
  [@test] tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`, `should clear the log of the repository`)
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls in place when the first entry of Activity comes, its button opening the empty log before`)
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
  [@test] src/stores/activity.test.ts (`groups by local day: today, yesterday, then the date`)
  [@test] src/lib/zoom.test.ts (`is smaller still for Activity, and wider above 110%`)
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
  the pool of the repository (`journalAgentOperation`, through the registry as `brd` does) and remember the journal of
  the repository on a disk that is gone (`loseJournalDisk`); the test server tells the pages of the repository
  `activity-changed` on their event streams after `record_activity` and `clear_activity`, and when the watch of the
  operations registry sees the journal of the repository change, as the backend does; a test fires it for an operation
  of `brd` it journals itself.
  [@test] crates/test-server/src/app_events_test.rs (`an_operation_journaled_in_the_repository_a_window_shows_tells_its_pages`)

## Open Questions / Risks

- A journal is read whole at each read of Activity: 200 entries with landed reports stay small (a few hundred KB at
  worst), but a much larger cap would not. A line this version can't read (a newer version's) is skipped, never the
  journal.
- A read of a journal that takes longer than 200 ms under load shows what the journal answered before (unavailable
  when it never answered) until its answer comes, which tells the windows to read again.
- A cleared log records a still undismissed report again at the next sync, as "Found by a sync".

## References

- [Landed Commits](../landed-commits/spec.md): the island and its lifetime.
- [Outgoing Commits](../outgoing/spec.md), [Update Project](../update-project/spec.md), [Patronus](../../branch/patronus/spec.md),
  [Squash](../../commit/squash/spec.md).
- [Distributed Settings](../../../../docs/design/distributed-settings.md): persistent and main-only stores.
