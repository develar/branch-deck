---
name: Patronus
description: Requirements for the Patronus status of the virtual branches (Safe Push, Safe Merge, Dry Run), read over the Patronus REST API with the token of the Safe Push CLI, with the least load on Patronus, shown live in the Safe Push slot of the branch row (its time and how long it usually takes, a progress line, the branches it lands) and as a chip in the branch preview and the outgoing overview; the toast and the OS notification when a robot merged a branch, with a fetch of the baseline; starting a Safe Push or a Dry Run of a branch (from the row, the branch menu, the preview and the chip popover) with the code-owner review it needs (the approvals in the GitHub pull request; Space merge requests are deprecated), or from its GitHub pull request with a /patronus comment when the branches are reviewed on GitHub; restarting and canceling its robot, and the question before a push into a branch whose Safe Push runs.
style: plain-1
targets:
  - ../../src/lib/os-notification.ts
  - ../../crates/sync-types/src/patronus.rs
  - ../../crates/patronus/src/actions.rs
  - ../../crates/patronus/src/client.rs
  - ../../crates/patronus/src/review.rs
  - ../../crates/patronus/src/service.rs
  - ../../crates/patronus/src/token.rs
  - ../../crates/sync-core/src/patronus.rs
  - ../../crates/sync-core/src/patronus_command.rs
  - ../../src/lib/pull-request.ts
  - ../../src/lib/git/new-pull-request.ts
  - ../../crates/sync-core/src/remote_status.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../crates/sync-core/src/patronus_review.rs
  - ../../src/stores/patronus.ts
  - ../../src/lib/patronus.ts
  - ../../src/lib/review-commit.ts
  - ../../src/lib/refresh-gate.ts
  - ../../src/components/branch/PatronusChip.tsx
  - ../../src/components/branch/SafePushSlot.tsx
  - ../../src/components/branch/RobotProgress.tsx
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/lib/robot-eta.ts
  - ../../src/stores/robot-history.ts
  - ../../src/stores/robot-progress.ts
  - ../../src/lib/patronus-icons.ts
  - ../../src/hooks/useRobotTiming.ts
  - ../../src/components/branch/ReviewCommitNotice.tsx
  - ../../src/components/branch/InlinePatronusStart.tsx
  - ../../src/lib/patronus-stack.ts
  - ../../src/lib/landed-stack.ts
  - ../../crates/sync-core/src/pull_request_writes.rs
  - ../../src/components/branch/PushGuardDialog.tsx
  - ../../src/components/branch/PatronusReviewBlock.tsx
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/usePatronusRobotDetails.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useOpenRowForm.ts
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/lib/patronus-merge-request.ts
  - ../../src/lib/git/push.ts
  - ../../src/stores/push-guard.ts
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/PatronusStatusLine.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/stores/update-project.ts
  - ../../crates/sync-core/src/baseline_fetch.rs
  - ../../src-tauri/src/commands/fetch_baseline.rs
  - ../../src/lib/patronus-actions.ts
  - ../../src/components/ui/toast.tsx
---

# Patronus

Status: Implemented
Date: 2026-10-16

## Summary

Changes reach the master of the IntelliJ monorepo through Patronus: a robot (Safe Push, Safe Merge, Dry Run) rebases
the branch onto the target, runs the TeamCity checks and, unless it is a dry run, pushes. Branch Deck shows, per virtual
branch, the robot of the branch: running, failed, merged or passed, and whether the branch changed since it started.
While a robot runs, the Safe Push slot of its row is the robot: how long it runs and how long it usually takes, learnt
from the robots already listed, with a progress line under the row and "lands with oauth" on the branches it lands.
From the branch, the user starts a Safe Push or a Dry Run (pushing the branch first), restarts a failed robot on the
newest commit, cancels a running one, and is asked before a push that would make a running Safe Push fail. It reads
Patronus with the Space token of the Safe Push CLI and keeps the load on Patronus small: one list now and then, and a
poll of the running robots, every minute while the window is focused, every five minutes in the background while a
robot that pushes runs. When a robot merged a branch, a toast says so (an OS notification in the background) and
offers to fetch the baseline, so the header shows that an update brings the branch in. When the branches are reviewed
in GitHub pull requests (the committed Space settings mirror the repository to GitHub), a Safe Push of a
branch with a pull request is the `/patronus merge` comment of it: Patronus counts the approvals of the pull request,
reports in it and closes it once merged.

## Goals

- See which branches have a running, failed or merged Safe Push without opening the Patronus web UI, and when a running
  one is likely done, without a request more to Patronus.
- Notice that the branch changed since its robot started, e.g. after a fix was committed, without a false alarm after a
  sync that only rebuilt the branch on a newer baseline.
- Open the robot in Patronus in one click.
- Run the cycle "fix, push, restart" from Branch Deck, without the web UI and without breaking a running Safe Push.
- Learn that a Safe Push merged without watching it, also with the window in the background.
- Be a well-behaved client: no request with a token Patronus rejected, no background request while Patronus fails, no
  poll while nothing runs, only a slow one in the background while a robot that pushes runs.

## Non-goals

- Logging in: the Safe Push CLI (`./safePush.cmd`) logs in to Space and renews its 14-day token when it runs.
- The robots of others, and of other repositories than the one `origin` pushes to.
- The TeamCity checks of a robot in detail: Patronus links them.
- Other push modes than Safe Push (rebase with autosquash) and Dry Run, emergency pushes, cherry-picks into other
  branches: the web UI and the CLI do them.

## Requirements

### Reading robots

- The repository must be the Space repository `origin` pushes to (`git remote get-url --push origin`): SSH, scp-like and
  HTTPS URLs of `git.jetbrains.team/<project>/<repository>`. Another host, a local path or no `origin` is "not on Space"
  without a request; the answer is cached for 10 minutes, so a poll starts no git process.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`parses_space_remote_urls`)
  [@test] ../../crates/sync-core/src/patronus_test.rs (`rejects_remote_urls_outside_space`)
- The token is the one of the Safe Push CLI, `~/.patronus/code.jetbrains.team-identity-token.json` (`{name, token,
  expires}`), else `$SPACE_TOKEN`. An expired token is never sent. Without a token nothing is sent and the status is
  "not logged in".
  [@test] ../../crates/patronus/src/token_test.rs (`reads_the_token_of_the_safe_push_cli`)
  [@test] ../../crates/patronus/src/token_test.rs (`never_sends_an_expired_token`)
  [@test] ../../crates/patronus/src/token_test.rs (`falls_back_to_the_environment_variable`)
  [@test] ../../crates/patronus/src/service_test.rs (`sends_nothing_without_a_token`)
- The robots are `GET /app/rest/v1/robots?owner=me&repository=<name>&start=<time>`, newest first. A robot belongs to the
  virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its source branch; a robot on a temporary ref (a Safe
  Push of the IDE or the CLI) belongs to the branch whose local or pushed tip it took. A running robot wins over a
  finished one, then the newest.
  [@test] ../../crates/patronus/src/service_test.rs (`lists_the_robots_newest_first_and_names_the_virtual_branch`)
  [@test] ../../src/lib/patronus.test.ts (`matches a robot started on the branch, and a Safe Push of a temporary ref by the local or the pushed tip`)
  [@test] ../../src/lib/patronus.test.ts (`prefers a running robot over a canceling one over a finished one, then the newest`)
- The pushed tip comes with the remote status of the branch: the `rev-parse` that checks the remote branch exists
  returns it, so it costs no extra git process.
  [@test] ../../crates/integration-detection-tests/src/remote_status_tests.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)

### Load on Patronus

- The first list and the refresh button ask for the last 7 days; every other list asks only for the robots started
  since 2 minutes before the previous list started. A full list asked again within 10 s (a second window of the
  repository) is answered from the last one.
  [@test] ../../src/stores/patronus.test.ts (`lists the last days first, then only the robots started since shortly before the last list`)
  [@test] ../../src/stores/patronus.test.ts (`lists the full window on the refresh button, asked by the user`)
  [@test] ../../crates/patronus/src/service_test.rs (`reuses_a_fresh_full_list_but_not_an_incremental_one`)
- A sync lists at most every 5 minutes, a focus when the robots are older than 2 minutes (and at least a minute after
  the last request). A push lists nothing: whether the branch changed since a robot started is known locally.
  [@test] ../../src/stores/patronus.test.ts (`asks after a sync every five minutes at most, on focus when older than two minutes, never after a push`)
- A repository that answered "not on Space" is asked again only by the refresh button.
  [@test] ../../src/stores/patronus.test.ts (`does not ask again for a repository that is not on Space, until the refresh button`)
- The running robots of the branches are polled with `POST /app/rest/v1/robots/fetch-updates` (the known status of
  each; only the changed ones come back) only while a branch has a running robot: every minute while the window is
  focused; while it is not, every 5 minutes and only while a robot that pushes (not a dry run) runs, so its merge is
  told without the window. A focus polls at once, unless the last poll is less than 50 s old.
  [@test] ../../src/stores/patronus.test.ts (`polls the running robots of the branches every minute while the window is focused`)
  [@test] ../../src/stores/patronus.test.ts (`stops on blur and asks at once on focus`)
  [@test] ../../src/stores/patronus.test.ts (`polls a robot that pushes every five minutes while the window is not focused, a dry run not at all`)
  [@test] ../../src/stores/patronus.test.ts (`polls nothing without running robots`)
  [@test] ../../crates/patronus/src/service_test.rs (`polls_only_running_robots_with_their_known_status`)
- A token Patronus rejects (401) is not sent again by a background request until the token file changes (the CLI
  renewed it): Patronus checks every unknown token against Space. The refresh button still asks.
  [@test] ../../crates/patronus/src/service_test.rs (`a_rejected_token_is_not_sent_in_the_background_until_it_changes`)
- After a server or network failure, background requests back off: 1 s, doubled up to 10 minutes, reset by the next
  answer. A GET is asked again once after a transient failure; a 4xx is not asked again. A POST or a PUT is never
  repeated by itself.
  [@test] ../../crates/patronus/src/service_test.rs (`background_requests_back_off_after_a_server_failure`)
  [@test] ../../crates/patronus/src/service_test.rs (`a_client_error_is_not_asked_again`)
- The progress of the running checks of a robot (`GET /teamcity-proxy/build-progress/{buildId}`, the estimate of the CI
  for a build, as the robot page of the web UI shows it) is asked only with the details of a running robot, while they
  are open: the popover of the Safe Push slot, the Patronus section of the branch preview, or a running row of the list
  of the robots indicator. One request per running check, for the build of its current attempt (the running attempt
  with the highest number), in parallel, after the details' checks, and so 300 ms after the details opened; again once
  a minute while they stay open (61 s, as Patronus caches the progress for a minute: never faster), never after they
  closed. The backend caches the progress of each build for 60 s, so two windows don't ask it twice. No request comes
  from the row, the robots indicator's button, the Dock or a timer of its own. The request goes through the request
  policy above (the token, the backoff, the user agent, a GET asked once more after a transient failure, the log line
  "read the build progress" with its duration).
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_running_robot_have_the_estimate_of_its_running_checks`, `the_progress_of_a_build_is_asked_once_a_minute_and_never_for_a_finished_robot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
- One HTTP client with a kept-alive connection (connect timeout 5 s, list, poll and details 15 s, the review pre-check
  20 s, actions and merge requests 30 s) and the user agent
  `Branch Deck/<version> (<os>; <arch>)`, so Patronus can tell its load apart. `SAFEPUSH_PATRONUS_URL` (as for the
  CLI) points it at a staging instance.
- An answer keeps the robot objects that did not change, so only the chips of changed robots render again; a failed
  answer keeps the robots shown and only changes the availability.
  [@test] ../../src/stores/patronus.test.ts (`keeps the objects of unchanged robots and the record when nothing changed`)
  [@test] ../../src/stores/patronus.test.ts (`keeps the shown robots when Patronus can't answer`)

### Safe Push slot

- The Actions cell of a branch row has the Safe Push slot after the push button
  ([Branch Row](branch-row.spec.md#actions-cell)), one width in every row, so the push buttons and "⋯" line up down the
  table. It is the robot of the branch, or the start of one (`getSafePushSlot`):
  - without a robot, "Safe Push…" ([below](#starting-a-safe-push-or-a-dry-run));
  - while a robot runs (or cancels), the robot: a rotating ring, how long it runs and how long it has left
    ([Timing](#timing)), "Canceling" while it cancels;
  - a finished robot of the branch as it is (the commit it took, or the same commits rebuilt on a newer baseline): its
    state in its hue, "Failed", "Needs review", "Merged", "Dry run passed", "Canceled";
  - a robot of an earlier state of the branch (the warning dot below, a merged robot included: the same name with new
    work): "Safe Push…", as a new robot is the way to test the branch as it is. The robot stays in the branch preview
    and the outgoing overview, with the notice of what changed.
  A repository that is not on Space has no slot; a branch that can't be pushed (an error, no commits) has an empty one.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Safe Push slot`)
  [@test] ../../src/lib/patronus.test.ts (`offers "Safe Push…" without a robot, nothing where Patronus doesn't answer or the branch can't be pushed`, `is the live robot while it runs, also while it cancels`, `offers "Safe Push…" for a branch changed since its robot, a merged one too`)
- The row hover (or the keyboard focus in the row) gives the state text of a finished robot to "Safe Push…" when a start
  is possible and useful: not after a merge, not while `canStartRobot` refuses. The shield stays in the state's hue and
  keeps the details on hover and the robot on click; while they are open the slot stays so, as the pointer on its way
  into them leaves the row.
  [@test] ../../src/lib/patronus.test.ts (`shows a finished robot of the branch as it is, and offers a restart on hover where a start is possible, never after a merge`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should offer a new Safe Push of a failed robot on the row hover, the state staying on its shield`)
- Running and canceling are muted, failed `destructive`, failed for missing approvals "Needs review" `warning`, a
  successful Safe Push "Merged" `info`, a successful dry run
  "Dry run passed" `success`, canceled muted. The ring of a running robot is `primary`, as the spinner of a running push.
  [@test] ../../src/lib/patronus.test.ts (`names the state in its hue; a successful dry run passed, a successful push merged`)
- A robot gets a `warning` dot when the branch changed since it took its commit: the commit is not the one the remote
  branch is at now (`remoteTip`), or the branch is not up to date with the remote by content (work the remote lacks, or
  remote commits the branch lacks, as the [remote status](../../docs/design/remote-status.md) counts them). A merged
  robot gets none. Neither does a robot of the local tip, nor one whose remote branch is at its commit while the branch
  is up to date: every sync rebuilds the virtual branches on the newer baseline, so their hashes change while their
  commits don't. Until the remote status of the branch is loaded, there is no dot. In the slot only a running robot
  keeps the dot (a finished one gives way to "Safe Push…"); the chip of the preview and the overview has it.
  [@test] ../../src/lib/patronus.test.ts (`marks a robot whose branch changed since its commit, not one whose branch was only rebuilt, never a merged one`)
  [@test] ../../src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)
  [@test] ../../src/lib/review-commit.test.ts (`takes the branch as it is for its own tip, and while nothing says otherwise`)
- Its accessible name is the mode, then the state: "Safe Push: Failed"; for a running robot with its time, "Safe Push:
  Running, 18 minutes, about 22 minutes left" ("…, 14 hours, longer than usual").
- Hovering the robot opens the details: the mode and the state, the name, the target branch, the commit, the start and
  finish times, the cancellation reason, the notice about the commit (below) and the actions of the robot
  ([below](#restart-and-cancel)). A click opens the robot page and does not toggle the row.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)
- With the dot, the details say what changed and how to test the branch as it is: "Started on 3264f30f; the branch has
  changed since (now 88de421d): the result won't cover the newest commits. Restart Safe Push to test the current
  branch.", with "Restart Safe Push". When the remote lacks the change, a restart would test the commit the remote has:
  "Push and Restart Safe Push to test the current branch.", with "Push and Restart Safe Push"
  ([below](#restart-and-cancel)). For a running robot of the branch, "Cancel Safe Push, then Restart Safe Push to test
  the current branch." ("Cancel Safe Push, then Push and Restart Safe Push …"). The advice names the action as its button
  does (`formatRobotRestartLabel`). A robot that can't be restarted from here (a Safe Push of a temporary ref, "Needs
  review") gets no advice.
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is, pushed first when the remote lacks it`)
  [@test] ../../src/lib/patronus.test.ts (`asks to cancel a running robot first, and offers nothing for a robot of a temporary ref`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say when the branch changed since the robot started, and how to test the branch as it is`)
- A branch only rebuilt on a newer baseline (the remote branch at the robot's commit, the branch up to date, other
  hashes) gets no dot; the details say quietly "Started on an earlier build of the same commits."
  [@test] ../../src/lib/patronus.test.ts (`says quietly that a robot started on an earlier build of the same commits, nothing for the branch as it is`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say only quietly that a robot started on an earlier build of the same commits`)

### Timing

- A running robot has two estimates. The one from history is always there and costs nothing: no request is made for
  it, it comes from the robots the app already listed and polled, and the polling stays as [above](#load-on-patronus).
  The estimate of the CI is precise but costs a request per running check: it is read only while the details of the
  robot are open ([above](#load-on-patronus)), and used only while they are.
- The merge profile decides the checks of a robot, and so how long it runs (a Large Scale Change runs for hours, the
  default checks for minutes). A list doesn't name it; the answers that do carry it onto the robot at no cost: a poll
  (`fetch-updates` answers a robot with its options once its status changed, so every robot polled to its finish), the
  details (`GET /robots/{id}`) and a start (the profile chosen in the form, also before Patronus wrote it into the
  robot's options). A later list keeps it: the robot merges, the profile isn't overwritten with "not known".
  [@test] ../../crates/patronus/src/service_test.rs (`polls_only_running_robots_with_their_known_status`)
  [@test] ../../crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
  [@test] ../../src/stores/patronus.test.ts (`keeps the merge profile a poll, the details or the start named when a list, which never names it, answers the robot`)
- The main window keeps a history of the finished robots it saw (`robotHistory` in `settings.json`, the robot, its
  profile when known, the push mode, the target branch, the start, the finish and the status), at most 200 robots and
  none that finished more than 30 days ago, so the estimate survives a restart and learns beyond the 7 days of a list.
  [@test] ../../src/stores/robot-history.test.ts (`adds the finished robots of an answer, never a running one, and learns a profile an answer named since`, `keeps at most 200 robots and none that finished more than 30 days ago, the newest first`, `records the robots of a repository and gives them back as samples of its estimates`)
- How long a robot usually runs is the median duration of the successful robots of its merge profile (those without one
  for a robot without one) and push mode started in the last 30 days, of the listed robots and the history, when 3 or
  more of them finished; only those into its target branch when there are 3 or more (`estimateRobotDuration`). While
  the profile of the robot is not known, or fewer than 3 robots of it finished, the estimate is the rough one of its
  push mode. A failed robot stopped early and doesn't count. Without such a robot the run is indeterminate.
  [@test] ../../src/lib/robot-eta.test.ts (`takes the median of the successful robots of the push mode started in the last 30 days`, `takes the robots of the same merge profile once three of them finished, and those without one for a robot without one`, `falls back to the push mode, roughly, while the profile is not known or too few of its robots finished`, `is indeterminate without a finished robot to learn from`, `counts only the robots into the same target once there are enough of them`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)
- The slot says how long the robot runs and how long it has left: "18m · ~22m left" (the time left rounded up); past its
  usual time, the usual time instead, "55m · usually ~40m" (the hours only from an hour: "14h · usually ~1h"); without
  history, "18m". The time ticks with the relative times, every 15 s, without a request.
  [@test] ../../src/lib/robot-eta.test.ts (`says how long it runs and how long it usually takes`, `names the usual time once a robot runs longer, in hours once it runs an hour`, `counts the time left until the usual duration, indeterminate past it and without history`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)
- A thin line under the row of a running robot shows the share of its usual time; without history, past it and while it
  cancels, it runs across (indeterminate).
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)
- While the details of a running robot are open, they show the estimate of the CI in the row "Progress": the time left
  of the running check that ends last (its estimate minus its running time) and its name and percent, "about 9 min
  left · Unit Tests 93%"; "1 check queued" when checks wait in the queue; "Overtime 4 min" when every running check
  ran past its estimate; "probably hanging" in `warning` when the CI says so. The slot and the progress line of the row
  count down the time left it read meanwhile ("20m · ~9m left"), and the start line of the details keeps only the usual
  time ("usually ~1 h 50 min for Large Scale Change"); once the details closed, the slot and the line take the estimate
  from history again. A running row of the robots indicator's list shows the check and the queued checks too.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did`)
  [@test] ../../src/lib/robot-eta.test.ts (`names the time left, the check the robot waits for, and the queued checks`, `counts down the time left it read while the details were open, for the slot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
- The details of a running robot say it after the start time, with what the estimate is of: "Today 14:02 · usually ~1 h
  50 min for Large Scale Change · about 1 h 32 min left", "… without a merge profile …", the rough "… · usually ~40 min
  for Safe Push · 55 min, longer than usual", "… · running 18 min · no Safe Push finished in the last 30 days to
  estimate from". The slot doesn't name it. The details have the profile row when the profile is known, also before
  the details answered; the list of the robots indicator names it in a row: "Safe Push · Large Scale Change · 18m ·
  ~1h 32m left".
  [@test] ../../src/lib/robot-eta.test.ts (`explains the estimate and what it is of in the details`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`, `should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

### Chip

- The branch preview, the outgoing overview and the Landed island show the robot as a chip, the same text-sized button
  as the pull request chip (`StatusChip`): the shield and the state in its hue (the ring while it runs), the warning dot,
  the details on hover, the robot on click. A branch without a robot shows nothing.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Details

- The popover asks for the details of the robot 300 ms after it opened (a popover that only flashes asks nothing): the
  merge profile, the TeamCity checks ("1 failing (Fast Tests), 9 passed") and the problems of severity error. They are
  `GET /robots/{id}`, `/teamcity-checks` and `/problems?severity=ERROR` in parallel, cached by the backend: a finished
  robot for the session, a running one for a minute. Of a running robot, the progress of its running checks follows
  ([Load on Patronus](#load-on-patronus)), and the details are asked again every 61 s while open. When Patronus can't
  answer, the popover shows the list data.
  [@test] ../../crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_finished_robot_are_asked_once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Starting a Safe Push or a Dry Run

- When the repository is on Space (Patronus answered), "Safe Push…" opens the form below the row from five places: the
  Safe Push slot of the row ([above](#safe-push-slot)), the
  Patronus group of the branch menu (the right-click and "⋯"; with "Dry Run…"), the preview footer and the popover of a
  robot that needs review; and S on the focused row of the branch table ([Branch Row](branch-row.spec.md#keyboard)),
  which the hover of the row button and the menu item show. A click on the row button doesn't toggle the row. The
  preview footer's "Safe Push…" closes the preview and opens the form of the row; so does "Safe Push…" of the robot
  details inside the preview (its Patronus section, the chip popover of its meta line), one helper (`openRowForm`) for
  both.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should close the branch preview for the Safe Push form of its Patronus section`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push form of the focused row with S and start it with Enter`)
- Every entry point asks one predicate, `canStartRobot(branch, robot, landingAncestor)` (`src/lib/patronus.ts`): a start
  is not possible while a robot runs on the commit the branch is at (Patronus would refuse it: "A Safe Push of the last
  commit is running") nor for a blocked stacked branch ("Blocked by auth: fix it first, then sync", the push button's
  words). A Safe Push of a stacked branch also waits while a robot that pushes into the target (not a dry run) runs on
  an ancestor (`findLandingAncestor`): the branch carries the ancestor's commits, two robots would land them ("A Safe
  Push of login is running: wait until it merges"); and while a robot that pushes runs on a branch built on it
  (`findLandingDescendants`), which lands the branch ("A Safe Push of oauth is running: it lands login with it"); a dry
  run waits for neither.
  Then the button and the menu items are disabled, and their hover says the reason; "Safe Push…" that can start says
  "Checks the code-owner review, then starts the robot." The row button is disabled while the
  row runs an inline action too.
  [@test] ../../src/lib/patronus.test.ts (`starts a robot unless one runs on the commit the branch is at or the branch is blocked, and says why`, `waits for a robot that lands an ancestor of a stacked branch, not for a dry run of it`, `knows the ancestors a running robot of a stacked branch lands, and refuses their own Safe Push`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should wait for the Safe Push of an ancestor, which lands its commits`)
- The branches a running robot that pushes lands, the ones its branch is built on, say it in the third column of their
  Status cell, muted: "lands with oauth", with the tooltip "A Safe Push of oauth lands this branch with it"
  (`useLandingSpan`, the inverse of `useLandingAncestor`). It takes the place of "on master". In the stack graph the
  lines from the root down to oauth are in the colour of the robot, and the node of oauth has the ring of its slot
  ([Branch Row](branch-row.spec.md#tree)). The open Safe Push or Dry Run form of a branch keeps the path from the root
  down to it highlighted, what the start lands or checks. Their form says why it
  can't start: "A Safe Push of oauth is running: oauth is built on login, so it pushes login's commits into master too."
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
- The form below the row ("Safe Push of auth") says what happens: "Patronus rebases auth onto master, runs the checks
  and pushes it into master when they pass.", for a dry run "Patronus rebases auth onto master and runs the checks,
  without pushing into master.". It offers the merge profiles of the target branch that allow the mode (the emergency
  ones never), "No profile" and, by default, the profile of the last robot of the branch; nothing is remembered by
  Branch Deck. The profiles are `GET /robot-start-supported`, cached for 10 minutes (an error for 1). A target branch
  Patronus does not protect can't be started. Its start button takes the focus once the profiles are loaded, so Enter
  starts.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)
  [@test] ../../crates/patronus/src/actions_test.rs (`start_options_leave_out_the_emergency_profiles_and_default_to_the_profile_of_the_last_robot`)
  [@test] ../../src/lib/patronus.test.ts (`offers a profile for the modes it allows; none listed allows any`)
  [@test] ../../src/lib/patronus.test.ts (`blocks a start while a robot runs on the commit the branch is at`)
- The target branch is the branch of `origin` the baseline tracks (`origin/master` → `master`); a local baseline or one
  on another remote can't be started.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_the_baseline_tracks`)
- A branch whose tip the remote lacks is pushed first, as the push button does it ("Push and Start Safe Push"), and the
  form says so: "auth isn't on the remote yet: it is pushed first, then Patronus starts from the remote branch." ("The
  last commit of auth isn't on the remote yet: …" when the remote branch is older). A dry run pushes the branch too:
  only the push into the target is left out. The push is a step of the start: it has no toast of its own (Activity
  records it), the start's toast is the one toast of the action. A failed push starts nothing. A Safe Push of the branch still running on
  an older commit is canceled for that push, not restarted ([below](#pushing-into-a-branch-whose-safe-push-runs)). Then
  `POST /robots` starts the robot on the pushed virtual branch, as the web UI does (`ROBOT_WEB_UI`, the source branch
  `refs/heads/{prefix}/virtual/{name}`, kept: delete policy `NEVER`), so a restart takes the newest commit of the
  branch. The robot shows at once and is polled.
  [@test] ../../crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
- A started robot is a `success` toast "Safe Push started" ("Dry Run started") that names the branch, the target and
  the profile, never the ref the robot runs on ("auth → master · profile Large Scale Change", `describeRobot`), with
  "Open in Patronus" (the robot page), for 10 s; Activity records it in the same words. A failed start is an error
  toast "Cannot start the Safe Push" with what Patronus said.
  [@test] ../../src/lib/patronus.test.ts (`describes a robot by its branch and target, with the profile it was started with`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
- A start is never repeated by itself, not even right after the push: Patronus fetches the pushed branch while it
  handles the start.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_failed_start_is_never_repeated`)
- A stacked branch carries the commits of the branches it is built on ([Stacked Branches](stacked-branches.spec.md)): a
  child can't land without them, and its Safe Push lands them too. The form of a Safe Push of a branch built on
  branches that aren't in the target yet leads root first, above the profiles: "oauth is built on auth and login,
  which aren't in master yet. Safe Push auth first, then the next: after a merge, the next sync moves the branches
  built on it onto master." with a `Safe Push auth…` button (the form of the root-most ancestor). Its own start is the
  explicit alternative, muted: "Or start oauth now: the Safe Push lands auth and login with it." From three
  ancestors on it counts them and their commits first: "Or start IJAI-4074 now: the Safe Push lands
  21 branches · 140 commits with it: vm-lane-daemon, air-fake-agent, air-lane-document and 18 more." Enter doesn't start it
  (no focus on the submit). Before the start the branches of the stack that have work for the remote are pushed,
  parents first, as the push button pushes them ("Push Stack and Start Safe Push"; "auth, login and oauth have commits
  the remote lacks: they are pushed first, parents first, then Patronus starts from the remote branch of oauth."),
  without a toast of their own. A dry run says "It checks the commits of auth and login too: oauth is built on them.", counted from three
  ancestors on: "It checks 21 branches · 140 commits too: vm-lane-daemon, air-fake-agent, air-lane-document and 18 more,
  which IJAI-4074 is built on."
  A Safe Push of a parent says what its merge does: "login and oauth are built on auth: after the merge, the next sync
  moves them onto master; their pull requests are retargeted to master when they are pushed." (the clause about the
  pull requests only while one of them has an open one).
  [@test] ../../src/lib/patronus-stack.test.ts (`leads a Safe Push of a stacked branch root first, and says that its own start lands the ancestors`, `says that a dry run checks the ancestors' commits too`, `says what the merge of a parent does to the branches built on it`, `counts the branches and the commits a Safe Push lands with the branch, from three ancestors on`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should lead a stacked branch root first, and land the stack in one Safe Push when told to`, `should open the Safe Push of the root from the lead, and say what its merge does to its children`)

### Pull requests

- The branches are reviewed on GitHub when the robots list names the GitHub repository they are mirrored to: the
  mirror of the committed Space settings, or `branchdeck.githubRepository`
  ([Pull Requests](pull-requests.spec.md#reading-pull-requests), `resolve_github_repository`, cached with the Space
  repository for 10 minutes; a poll doesn't carry it). No repository is built into the app.
  [@test] ../../src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)
- Patronus links a robot to a pull request only when a comment of the pull request started it; only then the approvals
  of the pull request count, Patronus reports in it, and closes it with the label `merged` once merged. The REST API
  can't start such a robot. So the form of a branch with an open pull request posts the command as a regular comment
  (`gh api --method POST repos/<owner>/<name>/issues/<n>/comments`): `/patronus merge --keep-source-branch` for a Safe
  Push (the virtual branch stays; the sync archives it once merged, as after a start through the API), `/patronus
  dry-run` for a dry run. The text of the comment is built by the backend only, by its subject
  (`PatronusCommandSubject`): a pull request of someone else the user reviewed is `/patronus merge`, which deletes its
  branch once merged ([Review Requests](../repository/review-requests.spec.md#safe-push-of-a-reviewed-pull-request)).
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`a_merge_keeps_the_source_branch_and_a_dry_run_is_plain`, `a_merge_of_a_reviewed_pull_request_deletes_its_branch`)
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`posts_the_command_as_a_regular_comment_of_the_pull_request`)
  [@test] ../../src/lib/patronus.test.ts (`starts from the open pull request of a branch reviewed on GitHub, a draft only for a dry run`)
- Patronus merges a pull request into its base: a pull request into another base than the target, the one of a
  stacked branch into its parent's branch, is never commented on (`isPullRequestIntoTarget`). Such a branch starts
  through the API, and the form says that the approvals of the pull request don't count: "The approvals of pull
  request #8 don't count: it compares oauth with login, and Patronus merges a pull request into its base." Root
  first, the child's pull request is retargeted to the target once its parent merged, and then starts from its comment.
  [@test] ../../src/lib/patronus.test.ts (`never comments on a pull request into another base than the target: Patronus would merge it there`)
  [@test] ../../src/lib/patronus-stack.test.ts (`leads a Safe Push of a stacked branch root first, and says that its own start lands the ancestors`)
- The form says it: "Comments /patronus merge on pull request #7: Patronus rebases auth onto master, runs the checks
  and pushes it into master when they pass. It reports in the pull request and closes it once merged." A comment can't
  choose a merge profile: instead of the profiles the form says "A pull request comment starts the default checks of
  the target branch: it can't choose a merge profile." Patronus doesn't merge a draft: a Safe Push of a draft pull
  request can't start, "Pull request #7 is a draft: Patronus merges it once it is ready for review." A dry run of a
  draft can.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not merge a draft pull request, and count the approvals in it`)
- The bot answers the comment with a reaction: 👍 started, 👎 not, with a comment starting `⚠️` that says why. After the
  comment, every 3 s for up to a minute, the app reads the reactions of its comment (`gh api
  repos/<owner>/<name>/issues/comments/<id>`); after 👎 the first later `⚠️` comment of a bot is the reason, looked for
  once more if it isn't there yet; after 👍 an incremental list (`since` shortly before the comment) finds the robot of
  the pull request (`githubPrUrl`), which shows at once and is polled. The loading toast says "Asking Patronus on pull
  request #7…". A refusal is the error toast "Cannot start the Safe Push" with the bot's reason; no answer within a
  minute is "Patronus did not start a robot within a minute: see pull request #7".
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`a_comment_without_reactions_is_pending_and_a_thumbs_up_is_accepted`)
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`a_thumbs_down_reads_the_reason_from_the_bot_comment`)
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`the_reason_is_a_later_bot_comment_and_a_user_warning_is_not_one`)
  [@test] ../../src/stores/patronus.test.ts (`posts the command, waits for the thumbs up and returns the robot of the pull request`)
  [@test] ../../src/stores/patronus.test.ts (`throws the reason the bot gave with its thumbs down`)
  [@test] ../../src/stores/patronus.test.ts (`gives up after a minute without an answer`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say why Patronus refused the comment`)
- A branch without an open pull request starts through the API, as on Space, without a merge request.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and start a branch without one through the API`)
- A robot of a pull request (`type: GITHUB_PR`, `githubPrUrl`) knows it: its actions have "Open Pull Request #7". A
  restart keeps the pull request (Patronus restarts with it).
  [@test] ../../crates/patronus/src/service_test.rs (`a_robot_started_by_a_pull_request_comment_links_the_pull_request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

### Merged

- A robot known as running or canceling that an answer (a poll, a list, a status read) shows successful and not a dry
  run merged its branch. A robot first seen finished is no news; each robot is told once.
  [@test] ../../src/lib/patronus.test.ts (`finds a running or canceling robot that succeeded, not a dry run and not a robot first seen finished`)
  [@test] ../../src/stores/patronus.test.ts (`does not tell a successful dry run or a robot first seen merged`)
- With the window focused, a `success` toast says "auth merged into master" (the branch, or the subject of a Safe Push
  of a temporary ref) with the push mode ("Safe Push"), for 10 s. Its actions are below the text (the `robot` body of
  the toast): Fetch, the primary one, then "Open in Patronus", which opens the robot page and leaves the toast for
  Fetch.
  [@test] ../../src/lib/patronus.test.ts (`names the branch, else the subject of a robot of a temporary ref`)
  [@test] ../../src/stores/patronus.test.ts (`tells once in a toast with Fetch and the robot page when a running robot of a branch merged`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push merged, and fetch the baseline on request`)
- A watched robot of no branch (the Safe Push of a reviewed pull request, `watchRobot`) is polled with the robots of
  the branches, and its merge is named by what it tested: "#101 Rework the session store merged into master".
  [@test] ../../src/stores/patronus.test.ts (`polls a watched robot of no branch and names its merge by what it tests`)
- A robot of a stacked branch that merged landed the branches it is built on too: the open pull requests of those
  ancestors are closed (`gh pr close <n> --comment "Landed in master with #7, the Safe Push of
  user/virtual/oauth, which is built on this branch."`), and the toast and Activity say it ("Safe Push · closes the
  pull request of auth: it landed with oauth"). Only while the robot ran on the branch's tip as the table has it, and
  only a pull request whose head is the ancestor's tip: then the merge has all of it. A failed close is an error
  toast "Cannot close pull request #5".
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the ancestors a stacked branch landed, and says so`)
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`closes_a_pull_request_with_a_comment`)
- In the background (no Branch Deck window has the focus), an OS notification says it at once ("auth merged into
  master", "Safe Push · Fetch and update to pick it up") while "Safe Push merges" is on in Settings
  ([Review Requests](../repository/review-requests.spec.md#settings), `notifyPatronusMerges`, on by default); the toast
  shows on the next focus. The permission is asked on the first notification, never at start; without it, or without
  the plugin, nothing is said but the toast (`src/lib/os-notification.ts`).
  [@test] ../../src/stores/patronus.test.ts (`tells a merge in the background by an OS notification, and in a toast on focus`)
  [@test] ../../src/lib/os-notification.test.ts (`sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same`)
- Fetch fetches only the baseline's remote branch, as Update Project does, without updating the branch, the index or
  the files (`fetch_baseline`). Meanwhile the Update button of the robot's repository shows the fetch ("Fetching 45%"),
  never the button of another repository selected meanwhile; then a sync of the robot's repository, if it is still the
  selected one, checks the update against the fetched baseline, without expanding or scrolling. A failed fetch is an
  error toast.
  [@test] ../../src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)
  [@test] ../../crates/sync-core/src/baseline_fetch_test.rs (`fetch_moves_only_the_tracking_ref_of_the_baseline`)
  [@test] ../../crates/sync-core/src/baseline_fetch_test.rs (`local_baseline_has_nothing_to_fetch`)
- Branch Deck never updates by itself: the update stays a click.

### Code-owner review

- Patronus fails a Safe Push whose changes miss the approvals the code owners require (`.ownership/review-rules.yaml`
  of the target branch, PAT-A-109): the problem "Changes require review approval". The approvals count only in the
  review linked to the robot: the pull request whose comment started it, else the Space merge request of the start. A
  dry run is not checked.
- The Safe Push form checks the review before the start (never for a dry run, never for the chip): the commits of the
  branch with their paths from one `git log` (a deleted path too; 500 commits or more skip the check, as the IDE does),
  then `POST /pre-merge-approval-required-check`, in the cheap order: first without a merge request (Patronus asks Space
  nothing); only when approvals are missing, the open merge request of the branch (`GET /space/merge-requests`), then
  the check with it.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`reads_the_commits_of_the_branch_with_their_changed_paths`)
  [@test] ../../crates/sync-core/src/patronus_test.rs (`does_not_read_a_branch_with_too_many_commits`)
  [@test] ../../crates/patronus/src/review_test.rs (`no_merge_request_is_looked_up_when_no_approval_is_needed`)
  [@test] ../../crates/patronus/src/review_test.rs (`names_the_missing_approvals_and_checks_again_with_the_merge_request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)
- The pre-check is the heaviest call of the API: its answer is cached by the commits it checked (the tip and the oldest
  commit): "no approval needed" for 10 minutes, "approvals missing" for 2 minutes, and Recheck asks at once. A found
  merge request is kept for the session, "no merge request" for 5 minutes (a recheck asks again). An unavailable check
  (503) is said and not asked again by itself. The answers the backend keeps are bounded per kind (the least used go
  first), so a long session does not grow them with every push.
  [@test] ../../crates/patronus/src/review_test.rs (`a_required_review_is_asked_again_on_a_recheck`)
  [@test] ../../crates/patronus/src/review_test.rs (`an_unavailable_review_check_is_not_asked_again_by_itself`)
- `bd safe-push` checks the same review before a Safe Push, but missing approvals refuse its start
  (`approvals_missing`) unless `--without-approvals`: an agent never sends a robot that fails for them. It collects the
  approvals in the GitHub pull request (`--create-pull-request`), never a Space merge request, and a pull request
  someone approved lets it start ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
- Space merge requests are deprecated: the approvals go into the GitHub pull request of the branch, and Branch Deck
  never creates a merge request. One Patronus finds open still counts: the start links it.
- The form shows "No code-owner review needed.", "Approved in IJ-CR-7.", or a `warning` block: "Needs review by the
  code owners" or "Waiting for approval in IJ-CR-7", the groups with the available reviewers Patronus suggests ("Air
  Plugin Release Team (suggested: Alice)"), "A Safe Push fails without these approvals.", "Review Changes" and
  Recheck. With an open merge request it says "Patronus counts the approvals of IJ-CR-7." with "Open IJ-CR-7";
  without one, and without a GitHub repository of the branches, "Patronus counts code-owner approvals in the GitHub
  pull request of the branch. The GitHub repository of the branches is unknown: run git config
  branchdeck.githubRepository <owner>/<name>.", and no button creates anything. Start stays possible.
- Each group names the files it has to review ("· 2 files", a disclosure of the paths; "· all files" for a branch-wide
  group): `paths` and `branchWide` of the group when Patronus sends them, else the changed paths whose owner by the
  `OWNERSHIP` files of the baseline has the name of the group; a group Patronus delegates to, or names as the reviewer
  of another owner, stays without files. A path and "Review Changes" open the review window of the branch on the whole
  stack, the groups as filters ([Branch Review](branch-review.spec.md#the-files-of-each-reviewer-group-safe-push-form)).
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should list the files of each group and open them in the review window`)
- The UI must name the system the approvals count in, and a review button the system it creates or opens in.
  [@test] ../../src/lib/patronus.test.ts (`names the group and the available reviewers Patronus suggests`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should name the missing approvals and the GitHub pull request they count in, and start with a merge request found open`)
- The start links an open merge request of the branch (`spaceReview`), so its approvals count.
- When the branches are reviewed on GitHub ([above](#pull-requests)), no Space merge request is looked up:
  the pre-check runs without one (it can't take a pull request, so it never sees GitHub approvals). With approvals
  missing the block says "Needs review by the code owners", the groups, the approvals the pull request already has
  ("Approved on GitHub by alice; the Safe Push counts them, the check before it can't."), "A Safe Push fails without
  these approvals." and "Patronus counts code-owner approvals in pull request #7." with "Open Pull Request #7", or,
  without a pull request, "Patronus counts code-owner approvals in the pull request of the branch; the Safe Push starts
  from it." with "Create Pull Request" (the branch is pushed first, without a push toast, then GitHub's form opens).
  [@test] ../../crates/patronus/src/review_test.rs (`a_branch_reviewed_on_github_looks_up_no_merge_request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not merge a draft pull request, and count the approvals in it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and start a branch without one through the API`)
- The Patronus group of the branch menu creates no merge request: "Create Pull Request" of its history group opens the
  pull request the approvals count in.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible`)
- A robot that failed for missing approvals (its problems are read once per session, for the failed robots of the
  branches) is "Needs review" in `warning`; its popover says how to go on and offers "Safe Push…" instead of Restart
  (a restart would check the same merge request). A robot of a pull request offers Restart instead: "The code owners
  did not approve the changes in pull request #7. Restart once they approve: the restart counts the approvals the pull
  request has by then."
  [@test] ../../src/lib/patronus.test.ts (`restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit`)
  [@test] ../../src/lib/patronus.test.ts (`names a robot that failed for missing approvals "Needs review"`)
  [@test] ../../src/stores/patronus.test.ts (`reads the problems of a failed robot of a branch once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and offer a new Safe Push`)

### Restart and cancel

- Restart (the popover, the branch menu) is offered for a failed, canceled or canceling robot started on the branch,
  and for a passed dry run when the remote branch has a newer commit; Patronus refuses the others. Not for a robot that
  failed for missing approvals in a Space merge request; a robot of a pull request that did is restarted, as the restart
  counts the approvals the pull request has by then (its popover says so). A Safe Push of a
  temporary ref (IDE, CLI) is not restarted from here: it would test its old commit. The restart is a new robot.
- Cancel is offered for a running robot; the robot is canceling until its checks stopped.
- One builder lists the actions of a robot (`getRobotActions`), and every surface of the robot renders that list: the
  branch menu, the details (the chip popover, the preview section) and the [robots indicator](../repository/robots-indicator.spec.md).
  In this order: "Restart Safe Push" or "Push and Restart Safe Push" (below), "Cancel Safe Push", "Open in Patronus",
  and "Open Pull Request #12" for a robot started from a pull request (the branch menu leaves it out when it is the
  branch's own pull request, which its "Open Pull Request" opens). Each names the mode of the robot ("Restart Dry Run");
  a bare "Cancel" in a popover reads like closing it. The menu of a branch that can't change (a conflict, a sync error, a
  sync running) keeps "Cancel Safe Push" and "Open in Patronus": they don't change the branch.
  [@test] ../../src/lib/patronus.test.ts (`restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit`)
  [@test] ../../src/lib/patronus.test.ts (`cancels a running robot only`)
  [@test] ../../src/lib/patronus-actions.test.ts (`cancels a running robot, and opens it in Patronus and the pull request it was started from`)
  [@test] ../../src/lib/patronus-actions.test.ts (`restarts the robot of a branch without a row on its remote branch, never a robot of a temporary ref or one that waits for approvals in a merge request`)
  [@test] ../../crates/patronus/src/actions_test.rs (`restart_answers_the_new_robot_and_cancel_the_canceling_one`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
- A restart is the toast "Safe Push restarted", a cancel "Safe Push is canceling", each naming the branch and the
  target ("auth → master") with "Open in Patronus" (the new robot's page for a restart), as a start does; Activity
  records them in the same words. A failure is "Cannot restart the Safe Push" / "Cannot cancel the Safe Push".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
- Patronus restarts on the newest commit of the remote branch. When the branch has work the remote lacks, every surface
  offers "Push and Restart Safe Push" instead of Restart: the push of the row (asking first as any push,
  [below](#pushing-into-a-branch-whose-safe-push-runs)), then the restart on the pushed commit; a passed dry run counts
  the pushed commit as the newer one. It is disabled for a blocked branch, with the reason. The restart is the action:
  its toast "Safe Push restarted" is the one toast, the push has none (Activity records both).
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is, pushed first when the remote lacks it`)
  [@test] ../../src/lib/patronus-actions.test.ts (`restarts a failed robot, pushing first while the branch has work the remote lacks, and not for a blocked branch`)
  [@test] ../../src/lib/git/push.test.ts (`says only the restart for Push and Restart, recorded once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview`)
- A failed action is a toast with what Patronus said; without a token it says "Run ./safePush.cmd once to log in to
  Patronus" and sends nothing.
  [@test] ../../crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Pushing into a branch whose Safe Push runs

- Patronus tests the commit it took at the start and fails a Safe Push whose branch changed meanwhile ("branch was
  modified"). Every push of the branch (the row, the overview, the preview, `P`) whose Safe Push runs on the branch reads
  the status of the robot first (`GET /robots/{id}/status`, one row); still running, a dialog asks: "Cancel, Push and
  Restart" (cancel, push, restart on the pushed commit, the focused button), "Push Anyway" or back (`Esc Back`). A dry run is not affected
  and asks nothing; a robot that finished meanwhile asks nothing. The push is the action: its toast is the one toast,
  "Pushed auth" / "1 commit · Safe Push restarted" with "Open in Patronus" (the restarted robot); Activity records the
  push and the restart. A failed restart is the push toast, then "Cannot restart the Safe Push". The push takes the
  branch as it is after the answer: a sync while the dialog was open is not pushed over.
  [@test] ../../src/lib/patronus.test.ts (`guards a push into the branch of a running Safe Push, not of a dry run`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should ask first, and push only when told to`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot, push and restart it on the pushed commit`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push at once when the robot finished meanwhile`)
- The push of the Safe Push form ("Push and Start Safe Push", or the push before "Create Pull Request") is for a
  new robot: the dialog offers "Cancel and Push" instead of the restart and says "The new Safe Push takes the pushed
  commit." The form then starts the one robot of the branch, with the merge request; a restart would run the branch
  twice, the restarted robot without the merge request. The push of the Dry Run form keeps the restart: a dry run
  doesn't replace the Safe Push.
  [@test] ../../src/lib/git/push.test.ts (`cancels the running Safe Push for a push of a new one, and never restarts it`)
  [@test] ../../src/lib/git/push.test.ts (`restarts the canceled Safe Push on the pushed commit for any other push, in the one toast of the push`)
  [@test] ../../src/lib/git/push.test.ts (`pushes the branch as a sync left it while the question waited`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot for a new Safe Push from the form, and not restart it`)
- A push that finds its remote branch moved pushes nothing and offers the push over the fetched remote in a toast
  ([Outgoing Commits](../repository/outgoing.spec.md#push)); that push must ask the same question first. When it drops
  the commits the remote has now, the toast's action first opens the confirm of the row's push button, and its Force
  Push asks the question. A robot canceled for a push that did not happen must stay canceled until the toast's push,
  which restarts it on the pushed commit; the toast says so ("The Safe Push is canceled: the push restarts it."). With
  nothing left to push it is restarted at once, and the toast says that instead ("The Safe Push restarted on it.") with
  "Open in Patronus".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should ask again before the push of the remote-moved toast`, `should restart a robot canceled for a push the remote refused once the toast pushes`)

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body has a Patronus section (the
  details of the chip popover) after the pull request section. A branch without a robot has neither.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Outgoing overview

- The [outgoing overview](../repository/outgoing.spec.md) shows the chip next to the pull request chip of each row, as
  the shield alone (the state is its accessible name and in its popover): the branch names need the room.
- A robot that failed on the branch as it is (the local tip, or the same commits before a sync rebuilt them on a newer
  baseline, [above](#safe-push-slot)) needs the user: the robots indicator after the outgoing indicator says "1 failed"
  ("1 needs review"), its list has the robot, the Dock its badge ([Robots Indicator](../repository/robots-indicator.spec.md)).
  The outgoing indicator has no dot for it: it is told once. A branch changed since (a fix) needs no attention.
  [@test] ../../src/lib/patronus.test.ts (`needs the user for a failed robot of the branch as it is, also after a sync rebuilt the branch`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the chips in a column, and leave a robot that failed on the branch tip to the robots indicator`)
- The footer has a line "Patronus checked just now" (it ages while shown, as every relative time:
  [Update Project](../repository/update-project.spec.md)) with a Refresh button (a full list), named "Refresh
  Patronus". When Patronus can't answer, the line is a quiet hint instead: "Run ./safePush.cmd once to log in to
  Patronus" or "Patronus: <the error>". It is never a toast. A repository that is not on Space has no line.
  [@test] ../../src/lib/patronus.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../src/lib/check-counts.test.ts (`lists the failing checks first, then the running, the canceled and the passed ones`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should list the robots again from the overview`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should hint quietly when the Safe Push CLI is not logged in`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say nothing for a repository that is not on Space`)

## User Experience

- The chip is the same text-sized button as the pull request chip (`StatusChip`): `text-xs`, a `size-3.5` shield,
  underlined on hover. The dot is `size-1.5 rounded-full`.
- The Safe Push slot is `w-40` (10rem: "55m · usually ~40m" fits); its robot looks like the `xs` ghost buttons of the
  cell (`h-6`, `pl-2`, a `size-3` icon, `hover:bg-muted`), so the icon of every state sits in one column; its text
  truncates. The elapsed time is `text-foreground`, the rest muted, in tabular figures.
- A running robot has the rotating ring (`RobotRing`: a track and an arc, `animate-robot-ring`, 2.4 s a turn, slower
  than a spinner), `text-primary`, in the slot, in the chip and around its node in the stack graph; nothing pulses. The progress line is `h-0.5`, `bg-primary`
  on a `bg-primary/15` track, over the divider below the row (a row of no height after it: WebKit can't position a bar
  in a `tr`; the divider, a background of the cells, paints nothing in it); indeterminate, a third of it runs across (`animate-carousel`). "lands with oauth" is `text-xs
  text-muted-foreground`.
- The notice of a changed branch is `text-xs text-warning` after the warning triangle; the line of a rebuilt one is
  `text-xs text-muted-foreground`, without an icon (`ReviewCommitNotice`, shared with the pull request details).
- `info` is the merged colour, as for pull requests; push states never use it.
- The details popover is `w-80`; in the preview the same details sit in a nested `rounded-lg` panel.
- "Safe Push…" (`SafePushButton`) is an `xs` button with the `ShieldCheckIcon` (without it after the shield of a finished
  robot in the slot), ghost in the row and the preview footer,
  outline among the popover's buttons, in the foreground colour (the push colour is the push button's). Disabled, it
  keeps the focus and the hover (`focusableWhenDisabled`) and is dimmed to `opacity-50`; its reason is a `Tooltip`.
- A menu item's reason or description is a `Tooltip` beside it (`side="inline-end"`); a disabled item keeps the
  pointer for it. Escape closes the menu together with the tooltip.

## Testing / Local Run

- `cargo nextest run --workspace` covers the token, the request policy (rejected token, backoff,
  retries, reuse of a full list) against an in-process axum Patronus (`crates/patronus/src/service_test.rs`), and the
  Space URL parsing.
- `pnpm test:unit` covers the list and poll policy, the matching, the chip mapping, the slot, the landing span, the
  estimate (`robot-eta.test.ts`) and the merge toast and notification.
- `pnpm e2e` covers the slot, the chip, the popover, the preview section and the overview (`tests/e2e/patronus`); the
  timing runs on the fake clock of the page (`page.clock.fastForward`), the robots' times relative to its 2024-01-02
  10:00Z. E2E never calls
  Patronus: `POST /repositories/{id}/patronus` sets what `get_patronus_robots` answers (an empty `sourceRevision` means
  the local tip of the robot's branch), what `poll_patronus_robots` answers (the fixture robots whose status differs
  from the known one) and the robot of `get_patronus_robot_details` (without checks); `GET` returns how often the app
  listed and polled; `get_patronus_robot_problems` answers none and `get_patronus_review_requirement` "not required".
  The actions are answered in the page (`page.route`), which records their parameters. The E2E repositories have no
  remote: a push answered in the page (`routePush`, `pushBranchFromRow`) gives a branch its remote tip.
- Against the real Patronus: `pnpm run tauri dev`, open a checkout of `ultimate`; the log has one line per request
  ("listed Patronus robots", "polled Patronus robots") with its duration.

## Open Questions / Risks

- The pre-check is a hint: the review check of the robot decides (the chip shows it). `/space/merge-requests` finds only
  an open merge request from the branch into the target.
- The code-owner check counts the approvals of a GitHub pull request only for a robot its `/patronus` comment started;
  the pre-check and the REST start can't take a pull request. The block before the start therefore can't tell whether
  the approvals the pull request has are enough.
- A mirror configured only in Space, not in its committed settings, needs `branchdeck.githubRepository`; without it the
  branches are treated as on Space.

- The REST API is marked unstable (PAT-A-11): an unknown status or push mode shows as "Unknown status" instead of
  failing the list.
- Patronus matches `repository` exactly: a repository whose Patronus alias is not its Space name lists no robots.
- The token lives 14 days and is renewed only when the Safe Push CLI runs; the footer says so when it expired.
- The estimate knows only the user's own robots, and the profile only of the robots the app polled to their finish or
  whose details were open: a robot finished while the app was closed is a sample of its push mode only. Half of the
  robots run past the median, so "usually ~40m" shows near the end of many runs.
- The changed-since rule replaces a finished robot in the slot with "Safe Push…" as soon as the branch has a commit
  the robot didn't test: its result is then only in the preview, the outgoing overview and the list of the robots
  indicator, under "Last 24 hours".

## References

- [Branch Row](branch-row.spec.md): the Actions cell with the Safe Push slot, the Status cell and the anatomy of the chip
  details
- [Pull Requests](pull-requests.spec.md): the other review status, the same chip and refresh gate
- [Outgoing Commits](../repository/outgoing.spec.md)
- [Robots Indicator](../repository/robots-indicator.spec.md): the running and failed robots in the header and the Dock
- [Branch Preview](branch-preview.spec.md)
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): the pushed tip
- Patronus REST API: YouTrack PAT-A-11; Safe Push CLI: PAT-A-51
