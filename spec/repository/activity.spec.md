---
name: Activity
description: Requirements for the log of what Branch Deck did in a repository (pushes, updates, landed reports, Patronus robots, pull request retargets, history rewrites, errors) and the review requests it told about, opened from the header, with the Undo of a rewrite while its toast's lasts, in a window of its own with the chips of the kinds and a search.
style: plain-1
targets:
  - ../../src/stores/activity.ts
  - ../../src/components/repository/ActivityButton.tsx
  - ../../src/components/repository/ActivityLog.tsx
  - ../../src/pages/ActivityPage.tsx
  - ../../src/lib/activity-window.ts
  - ../../src/lib/activity-window-link.ts
  - ../../src/hooks/useActivityWindowLink.ts
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
  - ../../src-tauri/src/command_error.rs
  - ../../src/lib/landed.ts
  - ../../src/lib/git/push.ts
  - ../../src/lib/git/new-pull-request.ts
  - ../../src/lib/patronus-actions.ts
  - ../../src/lib/push-summary.ts
  - ../../src/stores/patronus.ts
  - ../../src/stores/review-requests.ts
  - ../../src/lib/review-requests.ts
  - ../../src/stores/update-project.ts
  - ../../src/stores/landed.ts
  - ../../src/stores/settings/app-store.ts
---

# Activity

Status: Implemented
Date: 2026-10-15

## Summary

What Branch Deck did lives in toasts of 5 to 10 seconds: the push that dropped remote commits, the update, what landed,
the Safe Push the user started, the squash and its Undo. Activity keeps them: a `HistoryIcon` button in the header opens
a window with the log of the selected repository, newest first by day, each entry with what happened, the detail and
when; chips of the kinds and a search find an entry in a month of them. A branch name shows its row, a landed entry
has the report of the island, a rewrite has its Undo while the toast's lasts. It is the history the landing overview
leaves out: the overview tells the present, its line of the last day opens Activity at its Robots.

## Goals

- Answer "what did I push yesterday", "what did the update do", "which robot did I start" after the toast is gone.
- One record of each event, in the words of its toast: Activity adds no news of its own.
- Undo from Activity exactly as from the toast, for as long, once.

## Non-goals

- No history of the repository (that is `git reflog`): only what this app did, as it said it.
- No sync and no fetch of their own in the log: they change nothing the user asked for, a failure is an error entry.
- No Undo after the toast's window or after a restart: the rewrite it would revert may be pushed or rewritten again.

## Requirements

### Recording

- Every event that toasts in the main window is recorded for its repository, the selected one by default, in the words
  of its toast: the toast and the entry take their title and description from one function (`describePush`,
  `describeRobot`, `appliedUpdateToast`). A push that is a step of another action, which has no toast of its own (a
  Safe Push start, a merge request), and each branch of a stack push are recorded too, in the words their toast would
  have. A sub-window records nothing: the main window owns the log, the Activity window is told it over its link.
  [@test] ../../src/stores/activity.test.ts (`keeps the log of each repository apart, newest first`, `records nothing without a repository`)
- A push: "Pushed feature-auth" / "3 commits", "4 commits · created the remote branch"; a force push "Force pushed
  ABC-123" / "1 commit · dropped 1 remote commit" with the dropped remote commits (`DroppedCommitList`), or "replaced the
  rebased copies", "replaced 1 amended commit", "replaced the parent's older commits"; a remote that moved since its
  status says so, nothing pushed. The words come from the remote status
  the push was made against.
  [@test] ../../src/lib/push-summary.test.ts (`says what the push did from the status it was made against`)
  [@test] ../../src/stores/activity.test.ts (`keeps the remote commits a force push dropped`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`)
- An update: applied, with the toast's words ("Updated master: rebased 1 commit onto origin/master", or what landed
  first, with the report); refused, "Didn't update master" / "Conflicts in 1 file: src/app.txt. Nothing was changed.";
  an update with nothing to do is no entry.
  [@test] ../../src/stores/activity.test.ts (`records a refused update, not one with nothing to do`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`, `should record a refused update`)
- A landed report is recorded once: by the update that brought it, or by the sync after an update made elsewhere (the
  commits no entry has yet, "Found by a sync").
  [@test] ../../src/stores/activity.test.ts (`records a landed report once, whether the update or the sync after one brings it`)
- A Patronus robot started ("Safe Push started" / "auth → master · profile Large Scale Change"), restarted, canceled
  ("Safe Push is canceling" / "auth → master"), failing ("A required check of auth failed"), failed ("Safe Push of auth
  failed" / "Fast Tests · auth → master", [Patronus](../branch/patronus.spec.md#failed)) or merged ("auth merged into
  master" / "Safe Push"), with "Open in Patronus".
  [@test] ../../src/stores/activity.test.ts (`records a robot with its page`)
- A new review request ([Review Requests](review-requests.spec.md#new-requests)), one entry per request also when one
  toast told several: "dave requested your review on #104" / its title, kind `review`, with "Open Pull Request", the
  name of the action everywhere ([UX Writing](../../docs/guidelines/ux-writing.md#actions)).
  [@test] ../../src/stores/review-requests.test.ts (`tells nothing for the first answer, then one toast with Review for what is new, recorded in Activity`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should tell a request that comes with a poll in a toast whose Review opens it, unread until opened`)
- An approval of the user's pull request that made a branch ready ([Review Requests](review-requests.spec.md#approval-news)):
  "Ilya Korennoy approved auth" / "#1078 · IJPL Code Platform", kind `review`, with "Open Pull Request".
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should tell the approval that arrives with a poll, with Safe Push of the branch first`)
- A pull request the app retargeted from a virtual branch to the baseline ([Stacked Branches](../branch/stacked-branches.spec.md#pull-requests)):
  before a push, "Retargeted #802 to master" / "login · it went into auth"; by the next step, the words of its toast
  ("Retargeted #7 to master" / "auth · its approvals count for a Safe Push now"); kind `review`, with "Open Pull
  Request".
  [@test] ../../src/lib/git/push.test.ts (`retargets a pull request into a virtual branch to the baseline before the push, and Activity records it`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)
- A history rewrite with an Undo toast (squash, fixups, move, ungroup, rename, stack, unstack, reword), and every
  error toast (`notifyError`, with its reason).
  [@test] ../../src/stores/activity.test.ts (`records the errors the user was told about`)
  [@test] ../../src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`)
- The entry of an error a command returned keeps the operation it names as `operationId`, the key to the command's
  lines in the app's log: every command that can fail runs its body as an operation, the `operation{operation_id=…}`
  span of `src-tauri/src/operation.rs`, and its error carries the id (`CommandError.operation_id`); the app's log keeps
  the lines for days (the retention as the comment of `LOG_FILE_BYTES` in `src-tauri/src/lib.rs` describes it). The page
  log line of the error carries it as `operation_id`. It is stored, not shown yet; an error raised outside a command has
  none.
  [@test] ../../src/stores/activity.test.ts (`keeps the operation a failed command names, for its lines in the log`)
  [@test] ../../src/lib/errors.test.ts (`returns the operation a CommandError names, and nothing for any other error`)
  [@test] ../../src/lib/errors.test.ts (`passes the operation a CommandError names to the backend log line, as operation_id`)
  [@test] ../../src-tauri/src/command_scan_test.rs (`every_command_has_a_span_and_a_fallible_one_runs_as_an_operation`)
  [@test] ../../src-tauri/src/command_scan_test.rs (`a_body_is_one_operation_only_without_work_outside_it`)
  [@test] ../../crates/git-executor/src/thread_span_scan_test.rs (`every_closure_or_future_handed_to_another_thread_carries_the_span_of_its_caller`)
  [@test] ../../crates/git-executor/src/thread_span_scan_test.rs (`the_scan_reads_the_argument_of_a_spawn_past_strings_nested_calls_and_line_breaks`)
  [@test] ../../src-tauri/src/tracing_log_test.rs (`the_async_lines_of_a_command_and_its_blocking_step_name_one_operation`)
  [@test] ../../src-tauri/src/log_rotation_test.rs (`the_log_retention_comment_describes_the_plugin_version_the_lockfile_pins`)

### Undo

- A rewrite entry shows "Undo" (`xs`) for the 10 s of its toast. Either Undo, the toast's or the entry's, reverts the
  rewrite once, closes the toast, and the entry says "undone"; after the window the button goes, and the entry says
  what was done, as its title.
  [@test] ../../src/stores/activity.test.ts (`offers the Undo of a rewrite for the window of its toast, once`, `marks a rewrite undone from its toast`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should offer the Undo of a squash while the toast's lasts`)
- The undo itself is not persisted: after a restart no entry has an Undo.
  [@test] ../../src/stores/activity.test.ts (`has no Undo for a rewrite of an earlier session`)

### Log

- The log is persisted by the main window (`createPersistentStore("activity", { isArray: true, isMainOnly: true })`)
  and parsed by its Zod schema on load; it is not in the store snapshot of sub-windows: the Activity window gets the
  log of the selected repository over its link (below).
  [@test] ../../src/stores/activity.test.ts (`starts from the stored log and parses what is stored or received`)
- At most 200 entries over all repositories, none older than 30 days; the oldest go first.
  [@test] ../../src/stores/activity.test.ts (`keeps at most 200 entries and none older than 30 days`)
- "Clear Log" in the footer of the window forgets the whole log of the repository, whatever the filter shows (its
  tooltip says it); the others stay. The main window clears it, so an entry it records meanwhile is not lost; the
  window then says "Nothing yet: what Branch Deck does in the repository shows here", Clear Log disabled, and the
  header button goes.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should clear the log of the repository`)

### Window

- The header button, View → Activity of ☰ (custom title bar), the native Window → Activity (macOS) or View → Activity
  (Windows and Linux) and "Show in Activity" of the landing overview open the window `activity`, or bring the open one
  to the front: every entry point asks the main window (`openActivity`, the app command `open_activity`), which owns
  the log. An open names its chip ("Show in Activity": Robots; the others none, All) and starts over with it and an
  empty search.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should open from the View menu of ☰ and from the native menu`, `should filter by the chips of the kinds and the search, an open starting over at its chip`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)
- The window shows the log of the repository the main window shows, and follows a switch of it; its title names the
  repository ("Activity — branch-deck"). The main window tells it the entries, the branches of its table, the commits
  of the undismissed landed report and the links of the landed branches whenever one of them or the selection changes
  (`@/lib/activity-window-link`, the window link of [Branch Review](../branch/branch-review.spec.md)).
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should follow the repository the main window shows`)
- What the window does runs in the main window, which refuses a request of a repository it no longer shows: a branch
  name brings the main window to the front and reveals the row (`requestReveal`), the window stays; Undo runs the undo
  of this session's rewrite (`canUndoActivity`: "Cannot undo …: its time ran out" past the window); Dismiss and Clear
  Log. A refusal is an error toast in the window.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record the pushes with the remote commits a force push dropped, and show the branch`, `should offer the Undo of a squash while the toast's lasts`)
- The chips All, Robots (Patronus), Pushes, Updates (an update and what it landed), Reviews, Rewrites and Errors, with
  their counts of the entries the search keeps, one at a time; the chosen one again goes back to All; a chip shows
  while it counts an entry or is chosen. The search keeps the entries whose title or description has its text,
  ignoring case (`filterActivity`). Nothing kept: "No match for “…”" (or "Nothing of this kind") with Clear Filter.
  [@test] ../../src/lib/activity-window.test.ts (`keeps the entries of the chip whose title or description has the text, ignoring case`, `counts the entries of each chip that have the text, and of all`)
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should filter by the chips of the kinds and the search, an open starting over at its chip`)

### Landed entries

- A landed entry keeps the report: "What landed in origin/master" expands to the rows of the Landed island
  (`LandedBranchRow`), each expanding to its commits. While its commits are in the undismissed report (their chips and
  the "New" marks show, the island may be hidden since, see [Landed Commits](landed-commits.spec.md)), "Dismiss" clears
  them as the island's Dismiss.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should keep the landed report after a sync hid the island`)

## User Experience

- The button is a ghost `icon-sm` `HistoryIcon` after the landing indicator, named "Activity" with the tooltip
  "Activity" (an icon-only button, [UI Theme](../../docs/design/ui-theme.md)); it is hidden while the log of the
  repository is empty. The header stays on one line from 960 px.
  [@test] ../../tests/e2e/activity/activity.spec.ts (`should record an applied update, the button on the header line at the minimum window width`)
- The window opens at 560 × 720, at least 440 × 400 (wider above a zoom of 110%: `sub_window_min_size`); "Activity" in
  its title bar. On the canvas the search (`SearchField`, `w-60`, `/` focuses it) and the chips (`ChoiceChip`, as the
  filter bar of the branch table), below them the log in one `Card` island, the entries newest first grouped by day
  ("Today", "Yesterday", the date) with the day headings sticky on `bg-card`, then on the canvas the footer with Clear
  Log and "Esc Close".
  [@test] ../../src/stores/activity.test.ts (`groups by local day: today, yesterday, then the date`)
  [@test] ../../src/lib/zoom.test.ts (`is smaller still for Activity, and wider above 110%`)
- An entry: the icon of its kind (`UploadIcon` push, `ArrowDownIcon` update, `ShieldCheckIcon` robot, `PenLineIcon`
  rewrite, `UserCheckIcon` review request), `PackageCheckIcon` in `info` for what landed, `TriangleAlertIcon` in `warning` for a refusal, a dropping
  push or a moved remote, `OctagonXIcon` in `destructive` for an error; the title, the description muted, the relative
  time (`RelativeTime`). Commit subjects show through `CommitSubjectLine`, with their issue links.
- A word of the title or description that names a branch of the table is a button: it reveals the row in the main
  window, as a branch name of the landing overview.
- The rows of a landed report have the pull request ("#7", "Open Pull Request") and the robot (`ShieldCheckIcon`, "Open
  in Patronus") of the branch as links, the actions of the chips of the island, whose state is the main window's.

## Testing / Local Run

- `pnpm test:unit`: `src/stores/activity.test.ts`, `src/lib/activity-window.test.ts` (the chips and the search),
  `src/lib/push-summary.test.ts` (the words of a push).
- `pnpm e2e`: `tests/e2e/activity/activity.spec.ts` with the templates `issue_links` (pushes through `routePush`, a
  squash), `simple` (a second repository), `behind_upstream`, `update_conflict` and `landed_upstream`. The window is a
  linked page of its own (`openActivityWindow` of `tests/e2e/helpers/activity-helpers.ts`, `openLinkedSubWindow`).
  Test ids: `activity-button`, `activity-window`, `activity-filter`, `activity-list`, `activity-empty`,
  `activity-day`, `activity-entry` with `activity-title`, `activity-description`, `activity-branch`, `activity-undo`,
  `activity-undone`, `activity-landed-toggle`, `activity-landed-dismiss`, `activity-landed-report`;
  `activity-clear`.

## Open Questions / Risks

- The log lives in `settings.json`, read by the main window at startup: 200 entries with landed reports stay small
  (a few hundred KB at worst), but a much larger cap would not.
- An entry that doesn't parse (written by a newer version) drops the whole log on load, as any invalid stored value.
- A cleared log records a still undismissed report again at the next sync, as "Found by a sync".

## References

- [Landed Commits](landed-commits.spec.md): the island and its lifetime.
- [Outgoing Commits](outgoing.spec.md), [Update Project](update-project.spec.md), [Patronus](../branch/patronus.spec.md),
  [Squash](../commit/squash.spec.md).
- [Distributed Settings](../../docs/design/distributed-settings.md): persistent and main-only stores.
