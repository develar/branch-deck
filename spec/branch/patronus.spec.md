---
name: Patronus
description: Requirements for the Patronus status of the virtual branches (Safe Push, Safe Merge, Dry Run), read over the Patronus REST API with the token of the Safe Push CLI, with the least load on Patronus, shown live in the Safe Push slot of the branch row (its time and how long it usually takes, a progress line, the branches it lands, "Failing" once a required check failed) and as a chip in the branch preview and the landing overview; the toast and the OS notification when a robot merged a branch, with a fetch of the baseline, and when a required check of a running robot failed, with its cancel; starting a Safe Push or a Dry Run of a branch (from the row, the branch menu, the preview and the chip popover) with the code-owner review it needs (the approvals in the GitHub pull request; Space merge requests are deprecated), or from its GitHub pull request with a /patronus comment when the branches are reviewed on GitHub; restarting and canceling its robot, and no push of a branch a running Safe Push lands but its Push and Restart.
style: plain-1
targets:
  - ../../src/lib/os-notification.ts
  - ../../crates/sync-types/src/patronus.rs
  - ../../crates/patronus/src/actions.rs
  - ../../crates/patronus/src/client.rs
  - ../../crates/patronus/src/review.rs
  - ../../crates/patronus/src/review_readiness.rs
  - ../../crates/sync-core/src/review_readiness.rs
  - ../../crates/sync-core/src/robot_landing.rs
  - ../../crates/object-cache/src/review_answer.rs
  - ../../crates/object-cache/src/patronus_robots.rs
  - ../../crates/bd-cli/src/robot_list.rs
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
  - ../../src/lib/patronus-failing.ts
  - ../../src/lib/patronus.ts
  - ../../src/lib/review-commit.ts
  - ../../src/lib/refresh-gate.ts
  - ../../src/components/branch/PatronusChip.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/RobotProgress.tsx
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/hooks/useLandedIn.ts
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/lib/robot-eta.ts
  - ../../src/stores/robot-history.ts
  - ../../src/stores/robot-progress.ts
  - ../../src/lib/patronus-icons.ts
  - ../../src/hooks/useRobotTiming.ts
  - ../../src/components/branch/ReviewCommitNotice.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/lib/safe-push-decision.ts
  - ../../src/lib/safe-push-start.ts
  - ../../src/stores/safe-push.ts
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/hooks/useDismissesStaleReviews.ts
  - ../../crates/sync-core/src/branch_rules.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../src/lib/patronus-stack.ts
  - ../../src/lib/landed-stack.ts
  - ../../src/lib/whole-stack.ts
  - ../../crates/sync-core/src/pull_request_writes.rs
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/useStackIndependence.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/hooks/usePatronusRobotDetails.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useClosePreview.ts
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/lib/patronus-merge-request.ts
  - ../../src/lib/git/push.ts
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
  - ../../src/components/branch/BranchRowStatus.tsx
  - ../../src/hooks/useRowRobot.ts
---

# Patronus

Status: Implemented
Date: 2026-10-16

## Summary

Changes reach the master of the IntelliJ monorepo through Patronus: a robot (Safe Push, Safe Merge, Dry Run) puts the
branch on the target as it is at its start (a Safe Push rebases the branch onto it, a Dry Run merges it into the
branch), runs the TeamCity checks and, unless it is a dry run, pushes, rebased once more onto the target as it is then.
Branch Deck shows, per virtual branch, the robot of the branch: running, failed, merged or passed, and whether the
branch changed since it started.
The Status cell of a row says the robot as the state of the branch: while it runs, how long it runs and how long it
usually takes, learnt from the robots already listed, with a progress line under its time and "lands with oauth" on the
branches it lands; once it finished, its result. The Safe Push slot of the Actions cell offers the next step: "Safe
Push…", "Restart Safe Push" after a failure, the pull request after a robot that needs the code-owner review.
From the branch, the user starts a Safe Push or a Dry Run (pushing the branch first), restarts a failed robot on the
newest commit, cancels a running one, and is offered no push of a branch a running Safe Push lands: it won't land, and
it would fail the robot; "Restart Safe Push" is the way through. It reads
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
- The TeamCity checks of a robot in detail beyond the counts: Patronus links them, and the prompt for an agent of a
  failed robot carries its builds, problems and failed tests ([Safe Push Failure Prompt](safe-push-failure-prompt.spec.md)).
- Other push modes than Safe Push (rebase with autosquash) and Dry Run, cherry-picks into other branches: the web UI
  and the CLI do them. An emergency profile is offered only by a Safe Push without review ([below](#safe-push-without-review)),
  told in red; `bd safe-push` never takes one.
- Knowing before a start which profile skips the code-owner review: Patronus tells it only of an emergency profile
  (`isEmergency`) and, once a robot runs, in its options (`patronus.review.approval.check.skip`).

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
- After each list and each poll, the running robots (not a canceling one) of the branches and the watched ones that are
  not known failing yet are asked whether they will fail ([Failing](#failing)): `GET /robots/{id}/problems` of each, in
  parallel, in one command (`get_patronus_failing_robots`), through the request policy below. A robot known failing is
  not asked again: it stays failing until it finishes. So a poll costs at most one small request more per running
  robot, and none once each is known.
  [@test] ../../src/stores/patronus.test.ts (`asks whether the running robots will fail after a list and each poll, until known, never for a canceling one`)
  [@test] ../../crates/patronus/src/actions_test.rs (`a_running_robot_whose_required_check_failed_is_failing`)
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
  are open: the popover of the robot in the row, the Patronus section of the branch preview, or a running row of the
  landing overview. One request per running check, for the build of its current attempt (the running attempt
  with the highest number), in parallel, after the details' checks, and so 300 ms after the details opened; again once
  a minute while they stay open (61 s, as Patronus caches the progress for a minute: never faster), never after they
  closed. The backend caches the progress of each build for 60 s, so two windows don't ask it twice. No request comes
  from the row, the landing indicator, the Dock or a timer of its own. The request goes through the request
  policy above (the token, the backoff, the user agent, a GET asked once more after a transient failure, the log line
  "read the build progress" with its duration).
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_running_robot_have_the_estimate_of_its_running_checks`, `the_progress_of_a_build_is_asked_once_a_minute_and_never_for_a_finished_robot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
- One HTTP client with a kept-alive connection (connect timeout 5 s, list, poll and details 15 s, the review pre-check
  20 s, actions and merge requests 30 s) and the user agent
  `Branch Deck/<version> (<os>; <arch>)`, so Patronus can tell its load apart. `SAFEPUSH_PATRONUS_URL` (as for the
  CLI) points it at a staging instance.
- `bd status` (`landed_by`, [Agent CLI](../cli/agent-cli.spec.md#bd-status)) asks Patronus by the same rules,
  though every call is a process of its own: the user's robots are kept in the `patronus_robots` store of the object
  cache (`robot_list.rs`), keyed by a hash of the server, the token, the Space repository and the branch prefix. A
  kept list holds for a minute while a robot of it runs and for five minutes while none does (only a running robot can
  merge); then one list of the robots started since 2 minutes before the kept list was asked and one poll of the
  running ones it knows (`fetch-updates`) are merged into it; the first list asks for the last 7 days. A rejected
  token is not sent again until it changes (another key); after a failure nothing is asked for 1 s, doubled up to 10
  minutes, reset by the next answer, and the kept robots stay. `bd push`, `bd safe-push` and `bd robot` decide on the
  robots: they list them fresh and keep the list. `--no-cache` asks every time.
  [@test] ../../crates/bd-cli/src/robot_list_test.rs (`the_robots_are_kept_while_they_hold_then_the_new_ones_are_listed_and_the_running_ones_polled`, `a_rejected_token_and_a_failure_hold_the_requests_and_keep_the_robots`, `status_asks_patronus_once_while_the_kept_robots_hold`)
- An answer keeps the robot objects that did not change, so only the chips of changed robots render again; a failed
  answer keeps the robots shown and only changes the availability.
  [@test] ../../src/stores/patronus.test.ts (`keeps the objects of unchanged robots and the record when nothing changed`)
  [@test] ../../src/stores/patronus.test.ts (`keeps the shown robots when Patronus can't answer`)

### Robot of a row

- The Status cell of a branch row shows the robot of the branch as the state of the branch, in the first column of the
  cell, where the status text is otherwise ([Branch Row](branch-row.spec.md#status-cell), `BranchRowStatus`,
  `getRowRobotChip`):
  - while a robot runs (or cancels): a rotating ring, how long it runs and how long it has left ([Timing](#timing)),
    "Canceling" while it cancels; one that will fail ([Failing](#failing)) says "18m · Failing" instead of the time
    left, no use waiting for it;
  - a finished robot of the branch as it is (the commit it took, or the same commits rebuilt on a newer baseline): its
    state in its hue, "Failed", "Needs review", "Merged", "Dry run passed", "Canceled";
  - a robot of an earlier state of the branch (the warning dot below, a merged robot included: the same name with new
    work): the git status of the branch as it is. The robot stays in the branch preview and the landing overview, with
    the notice of what changed.
  A sync running, an error or a conflict, a block by the stack parent and a landed branch say more than the robot: the
  status text then.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
  [@test] ../../src/lib/patronus.test.ts (`shows a running robot and a finished one of the branch as it is in the Status cell, the git status otherwise`, `is the live robot while it runs, also while it cancels`, `offers "Safe Push…" for a branch changed since its robot, a merged one too`)
- The Actions cell has the Safe Push slot after the push button ([Branch Row](branch-row.spec.md#actions-cell)), one
  width in every row, so the push buttons and "⋯" line up down the table. It shows the next step of the branch, the
  first row of the table of [Next Step](next-step.spec.md) that matches (`getBranchNextStep`), from the robot, the pull
  request and the readiness Patronus answers ([below](#code-owner-review)), and every other "Safe Push" is disabled
  with the same reason:
  - "Safe Push" without a robot, for one of an earlier state of the branch and after a dry run
    ([below](#starting-a-safe-push-or-a-dry-run));
  - "Restart Safe Push" after a failed or canceled robot of the branch as it is, the action of its details
    ([below](#restart-and-cancel)), disabled with the reason where a start is not possible; "Safe Push" where Patronus
    can't restart it (a Safe Push of a temporary ref); after a robot that failed for missing code-owner approvals only
    once Patronus says they count in the pull request it was started from, else "Open Pull Request" (or "Create Pull
    Request…") while nobody can tell yet, or the wait for the approvals;
  - the wait for the approvals that count: "Awaiting Ilya Korennoy", "Changes Requested", "Request Review…",
    "Create Pull Request…"; "Draft", "Retarget #7 to master"; a stacked branch's by its own pull request;
  - nothing while a robot runs (Cancel is in "⋯", the details and the landing overview), after a merge, for a landed
    branch and a branch a Safe Push holds.
  Nothing hovers in: the state stays in the Status cell where the eye reads it, the step stays the step. A repository
  that is not on Space has no slot; a branch that can't be pushed (an error, no commits) has an empty one.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] ../../src/lib/patronus.test.ts (`offers "Safe Push…" without a robot, nothing where Patronus doesn't answer or the branch can't be pushed`, `shows a finished robot of the branch as it is, and whether a next step can be offered`)
  [@test] ../../src/lib/branch-next-step.test.ts (`row 2: restarts a failed or canceled robot, or offers Safe Push… where Patronus can't restart it`, `row 3: restarts a robot that failed for approvals only from the pull request they now count in`)
- Running and canceling are muted, failing `destructive`, failed `destructive`, failed for missing approvals "Needs
  review" `warning`, a successful Safe Push "Merged" `info`, a successful dry run
  "Dry run passed" `success`, canceled muted. The ring of a running robot is `primary`, as the spinner of a running push;
  of a failing one `destructive`, its progress line too.
  [@test] ../../src/lib/patronus.test.ts (`names the state in its hue; a successful dry run passed, a successful push merged`)
  [@test] ../../src/lib/patronus.test.ts (`names a running robot whose required check failed "Failing", still a running one`)
- A robot gets a `warning` dot when the branch changed since it took its commit: the commit is not the one the remote
  branch is at now (`remoteTip`), or the branch is not up to date with the remote by content (work the remote lacks, or
  remote commits the branch lacks, as the [remote status](../../docs/design/remote-status.md) counts them). A merged
  robot gets none. Neither does a robot of the local tip, nor one whose remote branch is at its commit while the branch
  is up to date: every sync rebuilds the virtual branches on the newer baseline, so their hashes change while their
  commits don't. Until the remote status of the branch is loaded, there is no dot. In the row only a running robot
  keeps the dot (a finished one gives way to the git status and "Safe Push"); the chip of the preview and the
  overview has it.
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
  branch.", with "Restart Safe Push". When the remote lacks the change, the restart pushes it first, under the same
  name ([below](#restart-and-cancel)). For a running robot of the branch the remote moved past, "Cancel Safe Push, then
  Restart Safe Push to test the current branch."; one of a branch changed here, "Restart Safe Push to test the current
  branch.": its push cancels the robot first. The advice names the action as its button
  does (`formatRobotRestartLabel`). A robot that can't be restarted from here (a Safe Push of a temporary ref, "Needs
  review") gets no advice.
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is, pushed first when the remote lacks it`)
  [@test] ../../src/lib/patronus.test.ts (`pushes and restarts a running robot of a branch changed since, asks to cancel one the remote moved past, and offers nothing for a robot of a temporary ref`)
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
  details (`GET /robots/{id}`) and a start (the profile it was started with, also before Patronus wrote it into the
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
- A thin line under the time of a running robot in the Status cell shows the share of its usual time; without history,
  past it and while it cancels, it runs across (indeterminate).
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)
- While the details of a running robot are open, they show the estimate of the CI in the row "Progress": the time left
  of the running check that ends last (its estimate minus its running time) and its name and percent, "about 9 min
  left · Unit Tests 93%"; "1 check queued" when checks wait in the queue; "Overtime 4 min" when every running check
  ran past its estimate; "probably hanging" in `warning` when the CI says so. The time and the progress line of the row
  count down the time left it read meanwhile ("20m · ~9m left"), and the start line of the details keeps only the usual
  time ("usually ~1 h 50 min for Large Scale Change"); once the details closed, the time and the line take the estimate
  from history again. A running robot row of the landing overview shows the check and the queued checks too.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did`)
  [@test] ../../src/lib/robot-eta.test.ts (`names the time left, the check the robot waits for, and the queued checks`, `counts down the time left it read while the details were open, for the slot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
- The details of a running robot say it after the start time, with what the estimate is of: "Today 14:02 · usually ~1 h
  50 min for Large Scale Change · about 1 h 32 min left", "… without a merge profile …", the rough "… · usually ~40 min
  for Safe Push · 55 min, longer than usual", "… · running 18 min · no Safe Push finished in the last 30 days to
  estimate from". The slot doesn't name it. The details have the profile row when the profile is known, also before
  the details answered; a robot row of the landing overview names it: "Safe Push · Large Scale Change · 18m ·
  ~1h 32m left".
  [@test] ../../src/lib/robot-eta.test.ts (`explains the estimate and what it is of in the details`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`, `should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

### Chip

- The branch preview, the landing overview and the Landed island show the robot as a chip, the same text-sized button
  as the pull request chip (`StatusChip`): the shield and the state in its hue (the ring while it runs), the warning dot,
  the details on hover, the robot on click. A branch without a robot shows nothing.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Details

- The popover asks for the details of the robot 300 ms after it opened (a popover that only flashes asks nothing): the
  merge profile, the TeamCity checks ("1 failing (Fast Tests), 9 passed") and the problems of severity error. They are
  `GET /robots/{id}`, `/teamcity-checks` and `/problems` (every severity: the errors are shown, the warning
  `robot_is_failing` marks the robot failing, [below](#failing)) in parallel, cached by the backend: a finished
  robot for the session, a running one for a minute. A check is named as Patronus names it in the profile ("IntelliJ
  Aggregator"), its build configuration ("Aggregator") only without that name. Of a running robot, the progress of its running checks follows
  ([Load on Patronus](#load-on-patronus)), and the details are asked again every 61 s while open. When Patronus can't
  answer, the popover shows the list data.
  [@test] ../../crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_finished_robot_are_asked_once`)
  [@test] ../../crates/patronus/src/actions_test.rs (`the_problems_of_a_finished_robot_are_its_errors`)
  [@test] ../../crates/patronus/src/client.rs (`a_failed_check_of_a_running_robot_reads_as_patronus_answers_it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Starting a Safe Push or a Dry Run

A start lands the work into master; the user who presses Safe Push on a row has chosen the piece (the branch and what
it is built on). So a start is an action, not a form: it asks only what a person decides (a profile, a running Safe
Push it cancels, approvals nobody can judge yet), and says it in a popover at the button that was pressed.

- When the repository is on Space (Patronus answered), "Safe Push" starts from six places: the Safe Push slot of the
  row ([above](#robot-of-a-row)), the Patronus group of the branch menu (the right-click and "⋯", with "Safe Push With
  Profile…" and "Dry Run"), the preview footer, the popover of a robot that needs review, the review window and the
  toast of an approval ([Next Step](next-step.spec.md)); and S on the focused row of the branch table ([Branch
  Row](branch-row.spec.md#keyboard)) and in the preview, which the hover of the row button and the menu item show. One
  function asks it everywhere (`requestSafePush`): the row is revealed first (a collapsed stack or island), the preview
  closes first (`useClosePreview`), a click on the row button doesn't toggle the row. Its name says what it lands: "Safe
  Push", "Safe Push · 3" for a stacked branch that lands with the two branches it is built on (`formatStartLabel`), no
  "…": it asks nothing unless it has something to say; its hover "Lands auth into master once the checks pass", "Lands
  oauth with the 2 branches it is built on into master in one Safe Push" (`describeStart`).
  [@test] ../../src/lib/branch-next-step.test.ts (`names the Safe Push of a stacked branch by the branches it lands, and says what it lands on hover`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button without toggling the row`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should close the branch preview for the Safe Push of its Patronus section`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter`)
- Every entry point asks one predicate first, the next step of the branch (`getSafePushBlocker`, [Next
  Step](next-step.spec.md)), which asks `canStartRobot(branch, robot, landingAncestor)` (`src/lib/patronus.ts`) first: a start
  is not possible while a robot runs on the commit the branch is at (Patronus would refuse it: "A Safe Push of the last
  commit is running"), nor while it runs on the remote branch that has the same commits by content, only rebuilt
  since on a newer baseline (`isStartBlocked`, the `rebased` state of `getReviewCommitState`; a start would test and land
  them again, in the same words), nor for a blocked stacked branch ("Blocked by auth: fix it first, then sync", the push button's
  words). A Safe Push of a stacked branch also waits while a robot that pushes into the target (not a dry run) runs on
  an ancestor (`findLandingAncestor`): the branch carries the ancestor's commits, two robots would land them ("A Safe
  Push of login is running: wait until it merges"), also on a side branch built on an ancestor, which lands that
  ancestor (side-fix built on bazel waits for the Safe Push of runtime, built on bazel too); and while a robot that
  pushes runs on a branch built on it (`findLandingDescendants`), which lands the branch ("A Safe Push of oauth is
  running: it lands login with it", the reason before the ancestor's); a dry run waits for neither. `bd safe-push` asks
  the same (`find_landing_ancestor`, `is_start_blocked` with the remote head of an up-to-date remote status).
  Then the button and the menu items are disabled, and their hover says the reason. The row button is disabled while
  the row runs an inline action or a start of the branch runs. After that the next step disables a Safe Push that
  Patronus would fail (the approvals that count missing, a draft, a pull request into another base, landed or merged),
  never a dry run; a start asked anyway (S, a toast, the review window) says the reason in an `info` toast ("Safe Push
  of auth waits for Ilya Korennoy", "Dry Run of auth can't start"), which opens what the reason names: the robot that
  runs on the branch or lands it ("Open in Patronus"), the pull request of a draft ("Open Pull Request",
  `SafePushDecision.opens`).
  [@test] ../../src/lib/safe-push-decision.test.ts (`refuses what Patronus would refuse or fail: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`, `tells a push that dismisses approvals Patronus doesn't count, and a dry run that pushes into a held branch can't`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)
  [@test] ../../src/lib/patronus.test.ts (`blocks a start while a robot runs on the remote branch that has the same commits, rebuilt on a newer baseline`)
  [@test] ../../crates/sync-core/src/patronus_start_test.rs (`a_start_waits_for_a_robot_on_the_last_commit_and_a_push_guards_a_running_safe_push`)
  [@test] ../../src/lib/patronus.test.ts (`starts a robot unless one runs on the commit the branch is at or the branch is blocked, and says why`, `waits for a robot that lands an ancestor of a stacked branch, not for a dry run of it`, `knows the ancestors a running robot of a stacked branch lands, and refuses their own Safe Push`, `waits for a robot of a side branch that lands an ancestor: the branch would land its commits a second time`)
  [@test] ../../crates/sync-core/src/patronus_start_test.rs (`a_safe_push_waits_for_a_robot_that_lands_an_ancestor_or_the_branch_itself`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should wait for the Safe Push of an ancestor, which lands its commits`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- One pure decision then says what the start does (`getSafePushDecision`), from the facts read when it is asked
  (`readSafePush`): the next step with the review only as Patronus answered it for the branch as it is (a stale answer
  is not acted on), the merge profiles, the remote status, the holds, the pull request the start comments on, the rules
  of the repository. Each fact that loads is a wait that names it ("Asking Patronus for the merge profiles…", "Checking
  auth…", "Checking the code-owner review…", "Checking whether the push keeps the approvals of #7…"); what Patronus
  would refuse or fail is a refusal with the reason ("Patronus does not push into master", "#7 is a draft: Patronus
  merges it once it is ready for review", "Patronus checks the approvals when the robot starts and fails without
  them", a hold of the push). A start that can run either starts or confirms first, and confirms only for what a
  person decides: it lands more than its row (a stacked branch), it runs another profile than the default checks (the
  branch's own, the profile of its last robot, or one chosen), it cancels the Safe Push running on an older commit,
  Patronus can't judge the approvals before the start (`unknown`, `unavailable`, GitHub doesn't list the pull requests,
  a repository only on Space whose merge request needs approvals), the push dismisses approvals Patronus doesn't count,
  a start without review. S, "Safe Push With Profile…" and the start without review always confirm.
  [@test] ../../src/lib/safe-push-decision.test.ts (`starts a lone branch at once when there is nothing to say: default checks, approved or not needed`, `opens the popover for a key, a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`, `refuses with the reason of the next step first, the reason every surface says`, `waits for the profiles, the remote status, the review and the rules, each saying what it waits for`, `refuses what Patronus would refuse or fail: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`, `tells a push that dismisses approvals Patronus doesn't count, and a dry run that pushes into a held branch can't`, `lets a profile that skips the review start without the approvals, and a start without review wait for its profile`)
- A click waits for the facts its decision needs, at most 20 s (`SAFE_PUSH_SETTLE_MS`), its button busy with
  "Checking…"; then it starts at once, says the refusal in an `info` toast ("Push" its action where the push alone is
  the step), or opens the popover (also after the 20 s: the popover says what it still waits for). The second click of
  a double click and a click on a button that just took the place of another step under a pointer that stayed (a poll
  changed the step, 700 ms) are ignored: they were meant for what was there before.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
- The popover (`SafePushPopover`) opens at the slot of the row (or of the island header, [Stack
  Island](stack-island.spec.md#header)), a dialog named "Safe Push of auth". Its title is what lands: "Lands auth into
  master: 3 commits.", for a stacked branch "Lands oauth with auth and login, which it is built on: 3 branches · 12
  commits into master." (from three ancestors on the names are cut: "Lands IJAI-4074 with vm-lane-daemon,
  air-fake-agent, air-lane-document and 18 more, which it is built on: 22 branches · 152 commits into master."), a dry
  run "Checks oauth with auth and login, which it is built on: 3 branches · 12 commits." or "Checks auth on master
  without pushing into it: 3 commits.". Below it only what applies, in sentences: the pull request it comments on
  ([below](#pull-requests)); the branches built on it, muted ("oauth and device are built on login: they stay, and
  after the merge the next sync builds them on master."); the profile; the review ([below](#code-owner-review)); the
  running Safe Push it cancels ("Cancels the Safe Push of auth running on 1a2b3c4."); a push that dismisses approvals;
  the way to land a stacked branch alone. The footer says what it waits for or why it can't start, then "Dry Run" (a
  Safe Push only: the start of a dry run instead), Cancel and the start ("Safe Push", "Dry Run", "Safe Push Without
  Review"), which takes the focus once it can run, so Enter starts. A start closes the popover: the slot shows it.
  Escape and a click outside close it. The stack graph highlights the path it lands while it is open.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should give the focus to the start of the popover once it can start`)
  [@test] ../../src/lib/patronus-stack.test.ts (`lands a stacked branch with what it is built on, in one robot, counted`, `names three ancestors and counts the rest, the size of what lands first`, `says that a dry run checks the commits of the ancestors too, and nothing of the children`, `says that the branches built on it stay, and what the next sync after the merge does to them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should say in the Safe Push popover of a branch in the middle what lands and what stays`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
- The profile:
 "Default checks" and the merge profiles of the target branch that allow the mode (the emergency ones
  apart, `emergencyProfiles`), by default the profile of the last robot of the branch (never an emergency one), named
  so: "The profile of the last Safe Push of auth."; its description from Patronus below. Nothing is remembered by
  Branch Deck: the profile is the branch's by its robots. The profiles are `GET /robot-start-supported`, cached for 10
  minutes (an error for 1), asked by the app once a minute per branch and last robot (`loadStartOptions`). A target
  branch Patronus does not protect can't be started. A profile carries `skipsReview`: true for an emergency profile and
  for the profile of the last robot whose options say `patronus.review.approval.check.skip=true` (false when they say
  otherwise), null for the rest, which Patronus doesn't describe; a robot's profile carries it from its options
  (`robot_profile`), null while Patronus hasn't written them. A profile known to skip the review lets the start run
  without the approvals: the review line says "Starts without the approvals of IJPL Code Platform: Large Scale Change
  skips the code-owner review.".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…`, `should open the popover of a click for the profile of the last Safe Push, and start with the one chosen`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)
  [@test] ../../crates/patronus/src/actions_test.rs (`start_options_keep_the_emergency_profiles_apart_and_default_to_the_profile_of_the_last_robot`)
  [@test] ../../crates/patronus/src/service_test.rs (`the_options_of_a_robot_say_whether_its_profile_skips_the_review`)
  [@test] ../../src/lib/patronus.test.ts (`offers a profile for the modes it allows; none listed allows any`)
  [@test] ../../src/lib/patronus.test.ts (`blocks a start while a robot runs on the commit the branch is at`)
- The target branch is the branch of `origin` the baseline tracks (`origin/master` → `master`); a local baseline or one
  on another remote can't be started.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_the_baseline_tracks`)
- A branch whose commits the remote lacks is pushed first, decided by content as the push button decides it
  (`getStartPush` over `getPushAction`, never by the hash of the tip). The push is part of the start: no note narrates
  it; while it runs the slot button shows it ("Pushing 45%", "Pushing 2 branches 45%" for the one push of a stack), the
  one indicator of it: the push button of the row is disabled without its spinner and percent; then "Starting…". A
  branch whose remote branch has the same commits, only rebuilt since on a newer baseline (every sync onto a newer
  baseline does it), is not pushed: Patronus rebases the remote branch itself; a push would replace it with copies of
  the same commits and move the head of its pull request for nothing. While the remote status of the branch is not
  loaded, the start waits: "Checking auth…". A dry run pushes the branch too: only the push into the target is left out.
  [@test] ../../src/lib/patronus.test.ts (`pushes before a start only what the remote lacks by content, and waits for the remote status`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a branch whose remote branch has its commits under other hashes without a push`, `should wait to start until the remote status of the branch is loaded`)
- A push before the start of a branch with an open pull request into the target that someone approved, in a
  repository whose GitHub rules dismiss stale reviews on push (a `pull_request` rule with `dismiss_stale_reviews_on_push`
  of `GET repos/<owner>/<name>/rules/branches/<target>`, readable with read access; `get_branch_rules`,
  `sync_core::branch_rules`), is told before the start, as a warning of the popover: "The push dismisses the approvals
  of #7: the rules of the repository dismiss stale reviews when new commits are pushed." The rules are asked once per
  repository and target in a session (`useDismissesStaleReviews`), only while the start would push into an approved
  pull request; an error (no token, no access) is no warning and is asked again next time. Until the rules answer, the
  start waits ("Checking whether the push keeps the approvals of #7…"): a start that pushed meanwhile would take the
  approvals away. When the approvals the push dismisses are the ones Patronus counts (the readiness is `approved`), the
  robot would start without them: the start is refused with "The push dismisses the approvals of #7: push first, then
  ask for the review again", and "Push" alone is offered (the action of the `info` toast, the button of the popover).
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should offer only the push when it dismisses the approvals Patronus counts`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should wait for the rules before a push into an approved pull request: Enter must not dismiss its approvals`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should refuse the click with the push alone in its toast`, `should say in the popover why it can't start, and push alone from it`)
  [@test] ../../crates/sync-core/src/branch_rules_test.rs (`a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch`, `a_branch_without_such_a_rule_keeps_the_approvals`, `a_failed_answer_is_an_error_not_a_guess`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should warn before a push that dismisses the approvals of the pull request`)
- The push is a step of the start (`startSafePush`): it has no toast of its own (Activity records it), the start's
  toast is the one toast of the action. A failed push starts nothing. The push is `pushForAction`, the push of every action
  that needs the remote (the pull request dialog, Request Review): it never drops someone's remote commits for the
  user, and a branch, or an ancestor it pushes, whose remote branch has commits the branch lacks refuses the start: "The
  remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them". A Safe Push of the branch still running on
  an older commit is canceled for that push, not restarted ([below](#pushing-into-a-branch-whose-safe-push-runs)). Then
  `POST /robots` starts the robot on the pushed virtual branch, as the web UI does (`ROBOT_WEB_UI`, the source branch
  `refs/heads/{prefix}/virtual/{name}`, kept: delete policy `NEVER`), so a restart takes the newest commit of the
  branch. The robot shows at once and is polled.
  [@test] ../../crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
  [@test] ../../src/lib/git/push.test.ts (`pushes the branch for the action without a toast, Activity records it`, `pushes the ancestors with work for the remote first, root first`, `refuses a push that drops remote commits of the branch or of an ancestor: the row asks first`, `returns the reason of a hold instead of a toast`)
- A started robot is a `success` toast "Safe Push started" ("Dry Run started") that names the branch, the target and
  the profile, never the ref the robot runs on ("auth → master · profile Large Scale Change", `describeRobot`), with
  "Open in Patronus" (the robot page) and below the text "Cancel Safe Push": a start asked by mistake costs only the
  robot (the push and the comment stay), for 10 s; Activity records it in the same words. A failed start is an error
  toast "Cannot start the Safe Push" with what Patronus said.
  [@test] ../../src/lib/patronus.test.ts (`describes a robot by its branch and target, with the profile it was started with`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
- A start is never repeated by itself, not even right after the push: Patronus fetches the pushed branch while it
  handles the start.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_failed_start_is_never_repeated`)
- A stacked branch carries the commits of the branches it is built on ([Stacked Branches](stacked-branches.spec.md)): a
  child can't land without them, so its Safe Push lands it with every branch it is built on that isn't in the target
  yet, in one robot ([Landing](../../docs/design/landing.md)); its start always confirms, the popover saying so first.
  Before the start the branches of the stack that have work for the remote are pushed, parents first, as the push
  button pushes them, a part of the start, without a toast of their own. Every surface asks the start of the branch
  itself, its popover at its own row: its slot, "⋯", S, the preview footer and the review window. The header of a stack
  island asks the start of the whole stack, one robot of its stack branch, its popover at the header ([Stack
  Island](stack-island.spec.md#header), [Whole Stack](whole-stack.spec.md#the-safe-push-of-the-whole-stack-in-the-app));
  of a straight chain, which has none, the start of its last branch. The popover of the root of a stack and of any
  branch of a stack that forks offers the whole stack as its second way, below what it lands: "Land the whole stack:"
  with "Safe Push Whole Stack" ("Check the whole stack:" with "Dry Run Whole Stack" for a Dry Run), the item of the
  menus (`getWholeStackStartItems`), its hover what it lands, disabled with the reason under it; it opens the popover
  of the whole stack.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should say in the Safe Push popover of a branch in the middle what lands and what stays`, `should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
- A stored stack parent can stay after the branch stopped needing it ([Stack Parents](stack-parents.spec.md#a-stacked-branch-that-builds-alone)).
  So the popover of a Safe Push of a stacked branch asks the trial build of the branch alone (`get_stack_independence`)
  when it opens, and again when the local tip of the branch changes; it never holds the start back. While it runs, a
  muted line says "Checking whether perf-part-2 builds alone…". Its answer is the second way, muted below the landing,
  never the lead:
  - The branch builds alone and uses no name its parent adds: "To land perf-part-2 alone: it applies on the baseline
    without IJPL-257337 and uses no name it adds. Not checked by a compiler." with "Unstack from IJPL-257337". The
    trial build is on the base the sync builds on, not on the newest target, so the words name the baseline. The
    unstack waits for its sync. Then the popover reads the branch alone (a root now: no landing line), with its
    review, and the user presses Safe Push, which lands and pushes only the branch: an unstack never starts a Safe
    Push. With `branchdeck.symbols` `report` or `off` the names are not read, and the sentence ends ". Compile
    dependencies are not checked."
  - The branch builds alone but uses a name its parent adds (`uses`), a warning: "\"Cache the parsed icon\" uses
    parsedIcon, which \"Parse the icon off the EDT\" of IJPL-257337 adds. perf-part-2 applies on the baseline
    without IJPL-257337, but needs it to compile. Found by names, not by a compiler." (two uses both: "uses parsedIcon
    and iconCache, which …" of one commit and what one commit adds, else a sentence each; from three on "uses
    parsedIcon and 2 more names, which …", never "and 1 more name") "Unstack from IJPL-257337"
    follows as a link: names found the use, so the unstack is offered, never decided.
  - The branch needs commits that can move: "To land perf-part-2 alone, move what it needs here: \"Cache the parsed
    icon\" changes lines that \"Parse the icon off the EDT\" of IJPL-257337 adds." (with more commits, "2 commits of
    IJPL-257337 add") with "Move 1 Commit Here", a move into the branch; then the check runs again. The count holds the
    fixups that move with their target. The check reads HEAD, and the branch list lags until the next sync: while the
    list misses one of the commits, it is as if they can't move.
  - The needed commits can't move (another branch needs them too), a conflict without a needed commit, a failed or a
    running check: no way alone; the landing stays the only one.
  The popover offers each of them and decides none: the stack parents don't say why a branch is stacked.
  [@test] ../../src/lib/patronus-stack.test.ts (`offers the unstack when the branch applies on the baseline without its parent, after the landing`, `warns that a branch that applies alone uses a name its parent adds, the unstack still offered`, `offers to move the commits it needs when no other branch needs them`, `offers no way alone when the needed commits can't move, nor while the check runs or after it failed`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to land only a stacked branch that applies alone`, `should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`, `should warn that a stacked branch that applies alone uses a name its parent adds, the unstack still offered`)

### Pull requests

- The branches are reviewed on GitHub when the robots list names the GitHub repository they are mirrored to: the
  mirror of the committed Space settings, or `branchdeck.githubRepository`
  ([Pull Requests](pull-requests.spec.md#reading-pull-requests), `resolve_github_repository`, cached with the Space
  repository for 10 minutes; a poll doesn't carry it). No repository is built into the app.
  [@test] ../../src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)
- Patronus links a robot to a pull request only when a comment of the pull request started it; only then the approvals
  of the pull request count, Patronus reports in it, and closes it with the label `merged` once merged. The REST API
  can't start such a robot. So the start of a branch with an open pull request posts the command as a regular comment
  (`POST repos/<owner>/<name>/issues/<n>/comments` of the GitHub API, with the token of [Pull
  Requests](pull-requests.spec.md#the-token)): `/patronus merge --keep-source-branch` for a Safe
  Push (the virtual branch stays; the sync archives it once merged, as after a start through the API), `/patronus
  dry-run` for a dry run. The text of the comment is built by the backend only, by its subject
  (`PatronusCommandSubject`): a pull request of someone else the user reviewed is `/patronus merge`, which deletes its
  branch once merged ([Review Requests](../repository/review-requests.spec.md#safe-push-of-a-reviewed-pull-request)).
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`a_merge_keeps_the_source_branch_and_a_dry_run_is_plain`, `a_merge_of_a_reviewed_pull_request_deletes_its_branch`)
  [@test] ../../crates/sync-core/src/patronus_command_test.rs (`posts_the_command_as_a_regular_comment_of_the_pull_request`)
  [@test] ../../src/lib/patronus.test.ts (`starts from the open pull request of a branch reviewed on GitHub, a draft only for a dry run`)
- Patronus merges a pull request into its base: a pull request into another base than the target is never commented
  on (`isPullRequestIntoTarget`), and such a branch starts through the API. Every pull request of a virtual branch goes
  into the target ([Pull Requests](pull-requests.spec.md)); an old one of a stacked branch into its parent's branch
  (`{prefix}/virtual/…`) is retargeted, by a push of the branch too ([Stacked Branches](stacked-branches.spec.md#pull-requests)). While one is open the start is refused with the
  reason of row 5 ("Approvals in #8 don't count: its base is user/virtual/login"), and the slot offers "Retarget #8 to
  master" (`retargetToTarget`, [Next Step](next-step.spec.md#the-table)); once GitHub took it, the start comments on
  it, and its approvals count.
  [@test] ../../src/lib/patronus.test.ts (`never comments on a pull request into another base than the target: Patronus would merge it there`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count`)
- The popover says it: "Comments /patronus merge on #7: Patronus reports there and closes it once merged." A comment
  can't choose a merge profile: the popover offers none, and "Safe Push With Profile…" is disabled ("A comment on #7
  runs the default checks: it can't choose a profile"); a branch whose own profile is another one says "A comment runs
  the default checks: it can't choose Large Scale Change.". Patronus doesn't merge a draft: "Safe Push" of a branch with
  a draft pull request is disabled, "#7 is a draft: Patronus merges it once it is ready for review", its slot the quiet
  "Draft" that opens it ([Next Step](next-step.spec.md)). A dry run of a draft can.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)
- The bot answers the comment with a reaction: 👍 started, 👎 not, with a comment starting `⚠️` that says why. After the
  comment, every 3 s for up to a minute, the app reads the reactions of its comment (`GET
  repos/<owner>/<name>/issues/comments/<id>`, conditional: a poll that sees no change costs no rate limit); after 👎 the first later `⚠️` comment of a bot is the reason, looked for
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
- A branch without an open pull request starts through the API, as on Space, without a merge request; that start counts
  no approval, so with approvals required its next step is "Create Pull Request…" ([Next Step](next-step.spec.md)).
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`)
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
  the toast): "Update Project", the primary one, which picks the merge up in one click (it fetches the baseline first,
  then updates onto it; the click is the consent, and a conflict, local changes or a lock say so as Update Project
  does), then "Open in Patronus", which opens the robot page and leaves the toast for the update. A merge in a
  repository the main window doesn't show offers Fetch instead (Update Project updates the shown one), or only "Open in
  Patronus" once its baseline was fetched since the merge (`lastFetchTime`: Branch Deck's fetch, or one elsewhere by
  `FETCH_HEAD`).
  [@test] ../../src/lib/patronus.test.ts (`names the branch, else the subject of a robot of a temporary ref`)
  [@test] ../../src/stores/patronus.test.ts (`tells once in a toast with Update Project and the robot page when a running robot of a branch merged`, `offers Update Project also after a fetch since the merge: the update is what picks it up`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push merged, and update the project on request`)
- A watched robot of no branch (the Safe Push of a reviewed pull request, `watchRobot`) is polled with the robots of
  the branches, and its merge is named by what it tested: "#101 Rework the session store merged into master".
  [@test] ../../src/stores/patronus.test.ts (`polls a watched robot of no branch and names its merge by what it tests`)
- A robot of a stacked branch that merged landed the branches it is built on too, the robot of the whole stack every
  branch of the stack ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app), "Whole air stack merged into
  master"): the open pull requests of those branches are closed with a comment, as `gh pr close --comment` does
  ("Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch."; one already closed
  gets no second comment, a merged one can't be closed), and the toast and Activity say it ("Safe Push · closes the
  pull request of auth: it landed with oauth", "closes the pull requests of 6 branches: they landed with the whole
  stack"). By content: only while the robot ran on the branch (the stack branch) as it is, its tip or the same commits
  rebuilt on a newer baseline, or (of a branch) a commit of it the branch built new ones on since: the copies below it
  are the same objects, so its ancestors are as the robot took them (`findLandedBranches`); after a change below its
  commit (a new commit or an amend of a branch it is built on), which made new copies, by the commits it tested
  (`get_robot_landings`, as the rows decide it, [below](#merged)): the toast waits for that answer (the sync that runs,
  one request), and a branch with commits added after the landed ones keeps its pull request; and only a pull request
  that holds nothing its branch lacks: its head is the branch's tip, or it is the remote branch and that has no commit
  the branch lacks (older versions of commits amended here included: their newer versions landed). The pull request the
  robot started from is Patronus'. A failed close is an error toast "Cannot close pull request #5". A robot first seen
  merged closes nothing.
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests of the ancestors a stacked branch landed, and says so`, `closes the pull requests of the branches the whole stack landed`, `closes by content the pull requests of what a merged robot landed after a change below its commit, the toast waiting for the answer`)
  [@test] ../../src/lib/landed-stack.test.ts (`closes the pull requests of the ancestors a merged piece landed, by content`, `leaves a pull request with a commit the branch lacks, and a robot of another state of the branch`, `decides by content after a change below the robot's commit: closes the pull requests of the branches it landed whole, not of one with work left`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should close by content the pull requests of the branches a Safe Push landed after a change below its commit, not of one with work left`)
  [@test] ../../crates/sync-core/src/pull_request_writes_test.rs (`closes_a_pull_request_with_a_comment`, `a_closed_pull_request_gets_no_second_comment_and_a_merged_one_is_not_closed`)
- Until the update archives them, the branches a merged robot landed that way (the same `findLandedBranches`, a robot
  first seen merged too: `findLandedBy`) say so in their rows, "Landed with oauth", "Landed with the whole air stack"
  (`LandedWithChip`, `info`, beside the status "landed" and the hollow `info` node of a branch in master), its hover "In master with the
  Safe Push of oauth · the update removes the local copies" and when it merged, a click the robot in Patronus; the
  preview shows it too. The branch the robot ran on landed too: it keeps its robot chip, "Merged". They offer no Safe
  Push and no Dry Run ([Next Step](next-step.spec.md#the-table), row 1: "Landed in master with oauth", "Merged by
  Patronus") and no push ([below](#pushing-into-a-branch-whose-safe-push-runs)); the "to push" counts leave them out.
  Once the fetched baseline has their commits, the Landed chip takes its place.
  [@test] ../../src/lib/patronus.test.ts (`knows the ancestors a merged piece carried and every branch a merged whole stack landed, and names who landed them`, `keeps the ancestors landed while the branch has the commit the robot took: rebuilt on a newer baseline, or with new commits on it`, `lands nothing for a dry run, a robot that didn't merge, one of another state of the branch, or of a temporary ref`)
  [@test] ../../src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`, `should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer no Safe Push or push of them`)
- A change below the commit the robot took (a new commit or an amend of a branch it is built on, a reword; for the
  whole stack any change of a branch of it) gives the branches new copies, so the hashes no longer tell what landed:
  the app asks by content (`get_robot_landings`, `robot_landings` of `crates/sync-core/src/robot_landing.rs`, the same
  for `bd`, [Agent CLI](../cli/agent-cli.spec.md#bd-status)): the robot's tested commits (the same commit, a
  patch-equivalent copy, or a copy with the same changes on a newer base, as the robot commit map compares them)
  against each branch as the table has it. A branch whose oldest
  commits the robot tested and whose other commits were added after them landed, those commits the only work left:
  "Landed with oauth · 1 commit after it" ("Landed · 1 commit after it" of the robot's own branch), its push and Safe
  Push offered for them. A branch with a commit changed since, or a new one below a tested one, is not landed. Asked
  once per state of the branches (`landingKey`: the robot's commit and the commits of the branches), never while a sync
  runs; of several robots that landed a branch, the one that left the fewest commits after it counts.
  [@test] ../../src/lib/patronus.test.ts (`decides by content once a change below the robot's commit gave the branches new copies: a commit added after the landed ones is the only work left`, `takes the landing that left the fewest commits after a branch, of equals the first merge`)
  [@test] ../../src/stores/patronus.test.ts (`asks what a merged robot landed by content once a change below its commit gave the branches new copies, once per state of them`)
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_robot_at_the_tip_landed_the_branch_and_what_it_is_built_on`, `a_commit_added_below_the_robots_commit_is_the_only_work_left`, `a_reworded_commit_below_keeps_the_branches_landed`, `an_amended_commit_leaves_its_branch_not_landed`, `a_commit_not_synced_yet_is_after_the_landed_ones`, `the_robot_of_the_whole_stack_landed_every_branch_of_it`, `a_missing_source_lands_nothing`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`the_landing_of_a_merged_robot_after_a_change_below_its_commit`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
- A merged robot of a branch that landed the root of a stack whose whole stack has an open pull request leaves that
  pull request matching nothing: the toast says it ("Safe Push · #12 no longer matches the air stack: oauth landed"),
  and the user decides on it ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)); nothing closes it.
  [@test] ../../src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)
- In the background (no Branch Deck window has the focus), an OS notification says it at once ("auth merged into
  master", "Safe Push": what merged, as the toast says it) while "Safe Push merges" is on in Settings → Notifications
  ([Review Requests](../repository/review-requests.spec.md#settings), `notifyPatronusMerges`, on by default); the toast
  shows on the next focus. The permission is asked on the first notification, never at start; without it, or without
  the plugin, nothing is said but the toast (`src/lib/os-notification.ts`).
  [@test] ../../src/stores/patronus.test.ts (`tells a merge in the background by an OS notification, and in a toast on focus`)
  [@test] ../../src/lib/os-notification.test.ts (`sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same`)
- Fetch (of a repository not shown) fetches only the baseline's remote branch, as Update Project does, without updating the branch, the index or
  the files (`fetch_baseline`). Meanwhile the Update button of the robot's repository shows the fetch ("Fetching 45%"),
  never the button of another repository selected meanwhile; then a sync of the robot's repository, if it is still the
  selected one, checks the update against the fetched baseline, without expanding or scrolling. A failed fetch is an
  error toast.
  [@test] ../../src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)
  [@test] ../../crates/sync-core/src/baseline_fetch_test.rs (`fetch_moves_only_the_tracking_ref_of_the_baseline`)
  [@test] ../../crates/sync-core/src/baseline_fetch_test.rs (`local_baseline_has_nothing_to_fetch`)
- Branch Deck never updates by itself: the update stays a click.

### Failing

- A robot fails once a required check failed, but Patronus runs its other checks on (for an hour, maybe) and says so
  only by a warning problem, `uniqueId` `robot_is_failing` ("This Safe Push will fail because required checks have
  already failed"), removed when the robot finishes. A running robot with that warning is failing (asked as in
  [Load on Patronus](#load-on-patronus), or read with its details); a canceling one never is: the user decided already.
  [@test] ../../crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)
- A failing robot is still a running one (its ring, its time, Cancel), named "Failing" in `destructive`: in the Status
  cell of its row ("18m · Failing", accessible name "Safe Push: Failing"), the chip, the ring of its node in the stack graph, and the
  [landing overview](../repository/landing-overview.spec.md#overview), a move under "Landing", where its row names the checks
  that failed: "Failed: Bazel Community Build Aggregator".
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
- Each failing robot is told once: a `warning` toast "A required check of auth failed" / "Bazel Community Build
  Aggregator · the Safe Push will fail" (the checks from its details, which the warning doesn't name; a watched robot
  by what it tests), for 10 s, with "Cancel Safe Push" first (the action of the robot, `getRobotActions`) and "Open in
  Patronus" below the text, recorded in Activity. In the background an OS notification says it at once while "Safe
  Push failures" is on in Settings → Notifications (`notifyPatronusFailures`, on by default), and the toast shows on the next focus.
  [@test] ../../src/stores/patronus.test.ts (`tells once that a running robot will fail, with Cancel Safe Push and the failed checks`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Failed

- A robot that failed is told once, as a merge is (`findFailedRobots`: running or canceling in one answer, failed in the
  next; a robot first seen failed is no news), once its problems are read (`loadReviewProblems`), which say whether it
  failed for missing code-owner approvals, at most 15 s later without them (`FAILURE_PROBLEMS_WAIT_MS`):
  - an `error` toast "Safe Push of auth failed" / "Fast Tests · auth → master" (the checks that failed from its
    details, else what it tested), with "Copy Prompt for Agent" first, or "Restart Safe Push" (the action of the robot,
    `getRobotActions`, where a restart is possible) when its builds never started, and "Open in Patronus" below the
    text ([Safe Push Failure Prompt](safe-push-failure-prompt.spec.md#the-failed-toast));
  - for missing approvals a `warning` toast "Safe Push of auth needs review" / "The code owners did not approve the
    changes" (a robot started with a profile: "Large Scale Change doesn't skip the code-owner review: the code owners
    did not approve the changes", what a [Safe Push without review](#safe-push-without-review) learns only now), with
    "Open Pull Request", or "Create Pull Request…" for a pushed branch without one: the approvals count in the pull
    request, a restart before them fails again;
  - recorded in Activity, and in the background the OS notification of "Safe Push failures" (`notifyPatronusFailures`),
    the toast on the next focus.
  A robot told failing while it ran ([above](#failing)) is told no second time: its Activity entry says it failed. The
  watched robot of a reviewed pull request is told by what it tests.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and copy the prompt for an agent from the toast`, `should restart from the toast a Safe Push whose builds never started`)
  [@test] ../../src/stores/patronus.test.ts (`tells once that a robot failed, with Copy Prompt for Agent and the failed checks, once its problems are read`, `tells a robot that failed for missing approvals "needs review", with its pull request`, `tells no second time a robot told failing while it ran`, `tells a robot whose problems never come without them`, `names the profile of a robot started without the approvals whose profile checks them all the same`)
- The step of a failing or failed toast is decided when it shows and again at the click: a toast stays 10 s, longer in
  the background. A step on the robot (Cancel, Restart, Copy Prompt for Agent) whose robot was
  restarted (a newer robot of the branch), finished or canceled since, or that is disabled now, does nothing and says
  why in an `info` toast (a precondition, [UX Writing](../../docs/guidelines/ux-writing.md)): "Safe Push of auth was
  restarted" / "auth → master", "Safe Push of auth merged" / "Cancel Safe Push no longer applies", with "Open in
  Patronus" (the newer robot). The pull request step opens what the branch has at the click.
  [@test] ../../src/stores/patronus.test.ts (`cancels nothing once the robot finished: the toast says it merged`, `restarts nothing once the branch has a newer robot: the toast says it was restarted`)

### Code-owner review

- Patronus fails a Safe Push whose changes miss the approvals the code owners require (`.ownership/review-rules.yaml`
  of the target branch, PAT-A-109): the problem "Changes require review approval". The approvals count only in the
  review linked to the robot: the pull request whose comment started it, else the Space merge request of the start. A
  dry run is not checked.
- Patronus is the only judge of whose approvals count; the app and `bd` ask it and render its answer
  ([Git Operations](../../docs/design/git-operations.md#patronus-review-pre-check)).
- A Safe Push checks the review before the start (never a dry run, never for the chip) with
  `get_branch_review_readiness`, which `bd safe-push` shares (`sync_core::review_readiness`): the commits of the
  branches at the tips the page sends (an answer is keyed by its branch and tip), from one `git log --topo-order` of
  all of them (a stacked branch shares its parents' commits; 500 commits or more of a branch skip its check, as the IDE
  does), their changed paths from the `commit_files` store of the object cache (one `diff-tree --stdin` for the commits
  it misses; a rename is a delete and an add, a deleted path counts, a merge has none), then one
  `POST /pre-merge-approval-required-check` per branch, one after another.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`reads_the_commits_of_the_branch_with_their_changed_paths`)
  [@test] ../../crates/sync-core/src/patronus_test.rs (`reads_several_branches_in_one_log`)
  [@test] ../../crates/sync-core/src/patronus_test.rs (`does_not_read_a_branch_with_too_many_commits`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)
- The check takes the pull request the robot would start from (`githubPullRequest`, `ReviewStart::PullRequest`: the
  open pull request into the target), and Patronus evaluates its reviews as that robot would: every group the changes
  need with `approved` and its counts (`requiredApprovals`, `collectedApprovals`, `selfApprovalAllowed`), every member
  with its `approvalState`. The readiness (`patronus::review_readiness`) only arranges that answer:
  - `notRequired`; `approved`, with the members whose approval counts;
  - `missing`: the groups still short of approvals, each with how many more count, who approved already, who is
    requested and hasn't approved, who requested changes and whom Patronus suggests; the approvers of the pull request
    no group counts (`notCounted`, a bot or a colleague outside the groups); the reason `noEligibleReviewer` when a
    missing group has nobody whose approval could count, else `approvalsMissing`. Changes requested by a member of an
    approved group don't block, as Patronus doesn't block on them, nor by a member asked again since (the wait for
    them); a re-requested approval counts as Patronus says; required with no group short of approvals is `unknown`;
  - a start through the API (`ReviewStart::Rest`: no pull request, or one into another base) counts no approval of a
    pull request: the check runs without one and its answer is the verdict, `missing` with `noPullRequestContext`;
  - `unknown` for a branch too long to check (`skipped`), `unavailable` when Patronus doesn't answer.
  [@test] ../../crates/patronus/src/review_readiness_test.rs (`a_bot_approval_is_not_counted_and_the_requested_owner_is_awaited`, `a_re_requested_approver_counts_as_patronus_says`, `a_self_approval_counts_where_the_group_allows_it`, `several_groups_partly_approved_list_the_missing_ones`, `a_group_that_needs_two_approvals_with_one_needs_one_more`, `a_group_whose_members_cannot_approve_has_no_eligible_reviewer`, `a_rest_start_counts_no_pull_request`, `a_skipped_branch_is_unknown`, `not_required_by_patronus_is_not_required_whatever_the_groups_say`, `a_member_re_requested_after_changes_is_awaited`, `required_without_a_missing_group_is_unknown`)
  [@test] ../../crates/patronus/src/review_test.rs (`a_check_with_a_pull_request_sends_it_and_reads_the_evaluation`)
  [@test] ../../crates/sync-core/src/review_readiness_test.rs (`a_rest_start_is_checked_without_the_pull_request`)
- Until Patronus evaluates pull requests (an older server, or its flag `review.precheck.github.pull.request.enabled`
  off: the field is ignored), the answer has no `approved`: without any approval in the pull request the approvals are
  `missing` for certain; with one, nobody but Patronus can tell whether it counts: `unknown`
  (`approvalsNotEvaluated`), and the robot decides.
  [@test] ../../crates/patronus/src/review_readiness_test.rs (`an_answer_without_the_evaluation_is_certain_only_without_approvals`)
- The pre-check is the heaviest call of the API: its answer is kept on disk, in the `review_answer.v1` store of the
  object cache the app and `bd` share, keyed by a hash of what Patronus decides on: the Patronus server and token, the
  Space repository, the target, the sorted changed paths, and with a pull request its number, its author and its
  reviews (login, state, request). A sync onto a new baseline keeps the key; a new review changes it, and Patronus is
  asked once. An answer with the evaluation of the pull request holds 24 hours; one without it 2 minutes while
  approvals are missing, 10 without. Recheck asks at once and keeps the new answer; errors are never kept. A check
  without Recheck is not sent while Patronus backs off after a failure (one backoff with the robot lists), so a 503
  ("The review check of Patronus is unavailable right now") holds back the rest of a batch. A tip gone since the page
  sent it fails the one `git log` of the batch: each tip is read alone then, only its branch is unavailable.
  [@test] ../../crates/sync-core/src/review_readiness_test.rs (`the_answer_about_a_pull_request_is_kept_until_a_review_changes`, `a_failed_check_is_unavailable_and_not_kept`, `an_answer_with_the_evaluation_holds_a_day_one_without_it_minutes`, `a_tip_gone_since_the_page_sent_it_makes_only_its_branch_unavailable`)
  [@test] ../../crates/patronus/src/review_test.rs (`an_unavailable_check_backs_off_the_background_checks`)
- A repository only on Space asks in the cheap order: first without a merge request (Patronus asks Space nothing);
  only when approvals are missing, the open merge request of the branch (`GET /space/merge-requests`), then the check
  with it. Its answers are kept in memory by the commits checked (the tip and the oldest commit): "no approval needed"
  for 10 minutes, "approvals missing" for 2 minutes, Recheck asks at once. A found merge request is kept for the
  session, "no merge request" for 5 minutes (a recheck asks again). An unavailable check (503) is said and not asked
  again by itself. The answers kept in memory are bounded per kind (the least used go first).
  [@test] ../../crates/patronus/src/review_test.rs (`no_merge_request_is_looked_up_when_no_approval_is_needed`)
  [@test] ../../crates/patronus/src/review_test.rs (`names_the_missing_approvals_and_checks_again_with_the_merge_request`)
  [@test] ../../crates/patronus/src/review_test.rs (`a_required_review_is_asked_again_on_a_recheck`)
  [@test] ../../crates/patronus/src/review_test.rs (`an_unavailable_review_check_is_not_asked_again_by_itself`)
- `bd safe-push` checks the same review before a Safe Push, always asked again (the answer is kept for the app), and
  `missing` refuses its start (`approvals_missing`) unless `--without-approvals`: an agent never sends a robot that
  fails for them. `unknown` lets it start, as the robot decides. It collects the approvals in the GitHub pull request
  (`--create-pull-request`), never a Space merge request ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
  [@test] ../../crates/bd-cli/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`)
- Space merge requests are deprecated: the approvals go into the GitHub pull request of the branch, and Branch Deck
  never creates a merge request. One Patronus finds open still counts: the start links it.
- A repository only on Space (no GitHub repository of the branches) keeps the merge request path: its start is not
  held back (merge requests are deprecated and the robot decides, [Next Step](next-step.spec.md#the-table)), but one
  whose approvals are required confirms first, its popover saying "Needs review by the code owners: a Safe Push fails
  without these approvals, which count in IJ-CR-7.", a line per group with the available reviewers Patronus suggests
  and its files ("Air Plugin Release Team (suggested: Alice) · 2 files", "· all files" for a branch-wide group), and
  "Open IJ-CR-7"; without a merge request "… They count in the GitHub pull request of the branch, and its GitHub
  repository is unknown." with
  "Set GitHub Repository…", which opens Settings → Repository at the field of `branchdeck.githubRepository`
  ([Settings](../repository/settings.spec.md#repository)); "Review Changes" beside it. No button creates anything.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should name the approvals a Safe Push needs on Space, and start with the merge request found open`, `should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)
- Each group names the files it has to review: `paths` and `branchWide` of the group when Patronus sends them, else
  the changed paths whose owner by the `OWNERSHIP` files of the baseline has the name of the group; a group Patronus
  delegates to, or names as the reviewer of another owner, stays without files. "Review Changes" of the popover (and
  D on the row) reviews the branch in its review window, on the whole stack, the groups as filters ([Branch
  Review](branch-review.spec.md#the-files-of-each-reviewer-group-safe-push-form)).
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)
- A start reads the review again when the local tip of the branch changes: after an unstack the popover reads the
  review of the branch alone, no longer that of its former parents; until the new answer arrives the start waits.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)
- The UI must name the system the approvals count in, and a review button the system it creates or opens in.
  [@test] ../../src/lib/patronus.test.ts (`names the group and the available reviewers Patronus suggests`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should name the approvals a Safe Push needs on Space, and start with the merge request found open`)
- The start links an open merge request of the branch (`spaceReview`), so its approvals count.
- When the branches are reviewed on GitHub ([above](#pull-requests)), no Space merge request is looked up: the
  pre-check runs with the pull request the robot starts from (above), and the "Create pull request" dialog asks it
  without one, for the code owners of the changes. The readiness is the next step's ([Next Step](next-step.spec.md)):
  missing approvals are its steps (Create Pull Request…, Request Review…, the wait, Changes Requested), and the start
  is refused with their reason; the popover, when it opens, says the readiness in one line:
  - `notRequired`: "No code-owner review needed."; `approved`: a `success` line "Approved by Ilya Korennoy for IJPL
    Code Platform.";
  - `unknown` and `unavailable`, which confirm first: an `info` line naming the approvals nobody but Patronus can judge
    yet: "Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval.", or
    "Patronus didn't answer the review check (…): the robot checks the approvals when it starts.";
  - `missing` with a profile known to skip the review or [without review](#safe-push-without-review): a `warning`
    "Starts without the approvals of IJPL Code Platform: Large Scale Change skips the code-owner review." with "Review
    Changes".
  While GitHub doesn't list the pull requests: "GitHub doesn't list the pull requests of the repository: Patronus checks
  the approvals when the robot starts." Once the pull request is created, the start comments on it: a `/patronus merge`.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`, `should ask the code owner Patronus names, and the row then awaits the review in the pull request`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing`, `should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`, `should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts`, `should start the Safe Push of a branch that needs no review with a click, and offer Create Pull Request… in its menu`)
- The Patronus group of the branch menu creates no merge request: "Create Pull Request…" of its history group opens the
  pull request the approvals count in.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
- A robot that failed for missing approvals (its problems are read once per session, for the failed robots of the
  branches) is "Needs review" in `warning`; its popover says how to go on and offers "Safe Push" instead of Restart:
  "The code owners did not approve the changes. Get their approvals in the pull request of the branch, then start a
  new Safe Push from it: a restart has no pull request to count them in." A robot of a pull request offers Restart instead: "The code owners
  did not approve the changes in pull request #7. Restart once they approve: the restart counts the approvals the pull
  request has by then.", "#7" a link that opens the pull request.
  [@test] ../../src/lib/patronus.test.ts (`restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit`)
  [@test] ../../src/lib/patronus.test.ts (`names a robot that failed for missing approvals "Needs review"`)
  [@test] ../../src/stores/patronus.test.ts (`reads the problems of a failed robot of a branch once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the pull request a robot that failed for missing approvals was started from, from its details`)

### Safe Push without review

Some merge profiles skip the code-owner review (Large Scale Change; an emergency profile skips all checks). Patronus
doesn't say which before a start, and a `/patronus` comment can't choose a profile, so a Safe Push without review is an
explicit choice of the user, offered only where the approvals count in a pull request the branch doesn't have.

- The "Create pull request" dialog of a branch at row 7 of the [next step](next-step.spec.md) (missing approvals, no
  open pull request) has "Safe Push Without Review…" at the left of its footer, a ghost button with the
  `ShieldOffIcon`, which takes neither ⌘↵ nor the focus; its hover: "Starts the Safe Push with a merge profile that
  skips the code-owner review, such as Large Scale Change", disabled with the reason of the branch (a sync, a running
  robot), never the approvals (`getSafePushWithoutReviewItem`, `BranchNextStep.withoutReview`). It closes the dialog,
  reveals the row and opens the popover of its Safe Push without review, also when the popover of the row is open
  (it opens again in the other mode). The dialog of any other step has no such button.
  [@test] ../../src/lib/branch-next-step.test.ts (`row 7: missing approvals without an open pull request: Create Pull Request…, the code owners named`, `row 8: changes requested by a member of a missing group: a destructive quiet step that opens the pull request`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`, `should not offer it in the dialog of a branch whose Safe Push needs no pull request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover without review from the pull request dialog`)
- The popover is "Safe Push of auth" with the start "Safe Push Without Review". Its review line names the missing
  approvals: "Starts without the approvals of IJPL Code Platform: the profile you choose has to skip the code-owner
  review.". The profile Select (`getBypassProfiles`) has the groups "Skip the code-owner review" (`skipsReview` true),
  "Other profiles" ("Profiles" without the first group; `skipsReview` null) and "Emergency"; a profile known to check
  the review is left out, and there are no default checks. None is chosen unless the last robot's profile skips the
  review: the footer says "Choose the profile that skips the code-owner review" and the start waits. Below the Select,
  the profile's description from Patronus and a line: "Large Scale Change skips the code-owner review: the last Safe
  Push of the branch ran with it.", or "Patronus tells only when the robot starts whether Large Scale Change skips the
  code-owner review: if it doesn't, the Safe Push fails at once.", or for an emergency profile in `text-destructive`
  with the warning triangle: "Emergency skips all checks of the Safe Push, not only the code-owner review: Patronus
  pushes into the target without testing it." The start is the REST start, announced and recorded in Activity as any;
  a profile that checks the review fails it before any build, told as [failed](#failed) with the profile named. Once a
  pull request is open or the approvals are no longer missing, the popover is the usual one.
  [@test] ../../src/lib/patronus.test.ts (`offers a Safe Push without review the profiles known to skip it, the unknown ones and the emergency ones apart`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`, `should say that an emergency profile skips all checks, and start with it`, `should start with the profile of the last robot when its options said it skips the review`)

### Restart and cancel

- Restart (the popover, the branch menu) is offered for a failed, canceled or canceling robot started on the branch,
  and for a passed dry run when the remote branch has a newer commit; Patronus refuses the others. Not for a robot that
  failed for missing approvals in a Space merge request; a robot of a pull request that did is restarted, as the restart
  counts the approvals the pull request has by then (its popover says so). A Safe Push of a
  temporary ref (IDE, CLI) is not restarted from here: it would test its old commit. The restart is a new robot, on the
  target as it is at the restart: the branch of an old robot needs no push of a newer baseline.
- A restart is a start: it waits where "Safe Push" waits (`canStartRobot`), disabled with the same reason on every
  surface, for a blocked branch, and, for a Safe Push, while a robot lands an ancestor of the branch or a branch built
  on it lands the branch: two robots would land the same commits. A dry run pushes nothing into the target, so only the
  block holds its restart back. Once a merged Safe Push landed all of the branch ([below](#merged)), its restart is
  refused, of a Dry Run too, as its Safe Push and Dry Run are ("Landed in master with oauth", row 1 of
  [Next Step](next-step.spec.md#the-table)): its commits are in the target. One with commits after the landed ones
  restarts, for them.
  [@test] ../../src/lib/patronus-actions.test.ts (`waits for a robot that lands an ancestor of the branch, as a start does; a dry run lands nothing and needn't`, `refuses the restart of a branch a merged Safe Push landed whole, a Dry Run too, and restarts one with commits after the landed ones`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should refuse the restart of the failed robot of a branch a merged Safe Push of a branch built on it landed, in its menu, its robot's details and its preview`)
- Cancel is offered for a running robot; the robot is canceling until its checks stopped.
- A restart or a cancel runs once per robot (an operation of the robot, [Running Operations](../../docs/design/operations.md)):
  while it runs, it is running on every surface of the robot (the slot, the details, the landing overview: disabled,
  a spinner in place of its icon, `aria-busy`), a second click joins it, the other one of the robot waits with its busy
  state ("Restarting the Safe Push…").
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should restart a robot once while its restart runs, running in the slot and in its details`)
  [@test] ../../src/stores/operations.test.ts (`lets a remote action wait for one of the same object or of a branch it works on`)
- One builder lists the actions of a robot (`getRobotActions`), and every surface of the robot renders that list: the
  branch menu, the details (the chip popover, the preview section) and the [landing overview](../repository/landing-overview.spec.md#overview).
  In this order: "Restart Safe Push" or "Restart Safe Push" (below), "Cancel Safe Push", "Open in Patronus",
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
  offers "Restart Safe Push" instead of Restart: the push of the row, then the restart on the pushed commit,
  also for a running robot, which it cancels first ([below](#pushing-into-a-branch-whose-safe-push-runs)); a passed dry run counts
  the pushed commit as the newer one. It is disabled for a blocked branch, with the reason. The restart is the action:
  its toast "Safe Push restarted" is the one toast, the push has none (Activity records both).
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is, pushed first when the remote lacks it`)
  [@test] ../../src/lib/patronus-actions.test.ts (`restarts a failed robot, pushing first while the branch has work the remote lacks, and not for a blocked branch`)
  [@test] ../../src/lib/git/push.test.ts (`says only the restart for the restart of a finished robot that pushes first, recorded once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview`)
- A failed action is a toast with what Patronus said; without a token it says "Run ./safePush.cmd once to log in to
  Patronus" and sends nothing.
  [@test] ../../crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Pushing into a branch whose Safe Push runs

- A branch is held (`PushHold`) while a Safe Push that pushes into the target runs on it (started on the branch, not of
  a temporary ref) or on a branch built on it, which lands it ("lands with oauth"). A push of a held branch won't land:
  Patronus merges the commit it took at the start, and a push into its own branch fails it at the end ("branch was
  modified"). A dry run and a canceling robot hold nothing. Every Safe Push in a stack holds its root. The Safe Push of
  the whole stack (on its stack branch) holds every branch of the stack: "A Safe Push of the whole air stack is running:
  it lands auth with it" ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)). A branch a merged Safe Push
  landed whole ([Merged](#merged)), the branch it ran on included, is held too, before any other hold, until the update
  archives it: "Landed in master with oauth: nothing of it is left to push", "Merged by Patronus: nothing of it is left
  to push"; one with commits added after the landed ones is not: they are work for the remote. A landed branch is only
  left out of the pushes of a stack: the branches built on it push without it (Push with Children of a landed root,
  [Stacked Branches](stacked-branches.spec.md#pull-requests)), while a running Safe Push holds every push of its stack.
- The stack branch of a whole stack ([Whole Stack](whole-stack.spec.md)) is held as a branch is
  (`getStackBranchHoldReason`): while its own robot runs ("A Safe Push of the whole air stack is running: wait until
  it merges, or restart it on the current stack"), and once merged Safe Pushes, of pieces or of the whole stack, landed
  every branch of the stack whole, which says who landed them, each robot once, in the order they merged ("Landed in
  master with deploy and side-test: nothing of it is left to push", "Merged by Patronus: …" of its own robot alone,
  `formatLandedWholeReason`). Push with Children leaves it then, and its push alone (`pushWholeStack`: the restart of its
  robot, Request Review… and the creation of its pull request) is refused with that reason.
  [@test] ../../src/lib/patronus.test.ts (`holds the push of a branch whose Safe Push runs, or that a running Safe Push of a branch built on it lands; not for a dry run or a cancel`, `holds the push of a branch a merged Safe Push landed, before a robot of its own`, `says why a group of branches has nothing left once every one of them landed whole: who landed them, in the order they merged`)
  [@test] ../../src/stores/patronus.test.ts (`holds a branch whose Safe Push runs and the ancestors it lands; not for a dry run or a cancel`, `holds every branch the whole-stack Safe Push lands`, `holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`, `holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them`)
  [@test] ../../src/lib/git/push.test.ts (`is held once merged Safe Pushes landed every branch of the stack: Push with Children leaves it, its push alone says why`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)
- A held branch offers no push: the place of the row's push button stays empty, as its Safe Push slot does; its Status
  cell says why (the robot chip, "lands with oauth", "Landed with oauth"). P does nothing; Push in the menu, the preview footer, the outgoing
  overview and the review window is disabled with the reason: "A Safe Push of auth is running: wait until it merges, or
  restart it on the current branch", "A Safe Push of oauth is running: it lands auth with it". The description of the
  preview's push offers what it names, from the robot's actions (`getGatedRobotActions`): "Restart Safe Push" of its own
  robot and "Open in Patronus" of the robot that holds it (`push-hold-actions`). The "to push" counts leave it out (the
  header, the stack chip, the island, the filter chip).
  [@test] ../../src/hooks/usePushButton.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open and restart the robot from the description of the preview's push, which says why it can't push`)
  [@test] ../../src/lib/push-summary.test.ts (`skips a branch a running Safe Push holds: its row offers no push`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer no push: it won't land, and the menu says why`)
- A push that raced with a start (the IDE, `bd`, another window) reads the status of the robot first (`GET
  /robots/{id}/status`, one row): still running, it pushes nothing and says why ("Cannot push auth", with "Open in
  Patronus" of the robot); finished meanwhile,
  it pushes. A branch a merged Safe Push landed is refused without a read: the robot finished, its commits are in the
  target. Nothing asks: no dialog.
  [@test] ../../src/lib/git/push.test.ts (`refuses the push: it won't land, and the Safe Push would fail at the end`, `pushes when the Safe Push finished meanwhile: the status is read fresh`, `refuses a branch a Safe Push of a branch built on it lands`, `refuses a branch a merged Safe Push of a branch built on it landed, without reading its status`)
- The one push into a held branch is a robot action: "Restart Safe Push" of a running robot whose branch
  changed since (the menu, the robot details, the landing overview; "Cancels the run on 1a2b3c4, pushes auth and starts
  it again on the pushed commit") cancels it, pushes and restarts it on the pushed commit. The restart is the action: its
  toast "Safe Push restarted" with "Open in Patronus" is the one toast; Activity records the push and the restart. The
  advice of the details names it: "… Restart Safe Push to test the current branch."
  [@test] ../../src/lib/patronus.test.ts (`pushes and restarts a running robot of a branch changed since, asks to cancel one the remote moved past, and offers nothing for a robot of a temporary ref`)
  [@test] ../../src/lib/patronus-actions.test.ts (`pushes and restarts a running robot of a branch changed since: it cancels it first`)
  [@test] ../../src/lib/git/push.test.ts (`cancels, pushes and restarts the running Safe Push for its restart, in the one toast of the restart`, `refuses a restart of another robot: only the robot the push restarts lets it through`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot, push and restart it on the pushed commit, from Restart Safe Push`)
- A Safe Push of a branch whose Safe Push runs on an older commit replaces it, and confirms first: "Cancels the Safe
  Push of auth running on 1a2b3c4." Its push cancels the robot at the first branch that robot holds (the branch, or an
  ancestor the stack push takes along), never restarts it; the start then starts the one robot of the branch. Without
  a push (the remote branch has the commits by content) the start cancels it itself. The push of "Create Pull
  Request…" replaces nothing: it is refused as any push. A Dry Run of a held branch that needs a push is refused with
  why: a dry run doesn't replace the Safe Push.
  [@test] ../../src/lib/git/push.test.ts (`cancels the running Safe Push for the start that replaces it, and never restarts it`, `cancels the robot the start replaces at the first branch it holds, and pushes its ancestors with it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot for a new Safe Push from the popover, and not restart it`, `should cancel the robot it replaces without a push when the remote branch has the commits`)
- A push that finds its remote branch moved pushes nothing and offers the push over the fetched remote in a toast
  ([Outgoing Commits](../repository/outgoing.spec.md#push)). When it drops the commits the remote has now, the toast's
  action first opens the confirm of the row's push button. A robot its restart canceled for a push that did not
  happen must stay canceled until the toast's push, which restarts it on the pushed commit; the toast says so ("The Safe
  Push is canceled: the push restarts it."). With nothing left to push it is restarted at once, and the toast says that
  instead ("The Safe Push restarted on it.") with "Open in Patronus".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a robot canceled for a push the remote refused once the toast pushes`)
- `bd push` holds the same branches ([Agent CLI](../cli/agent-cli.spec.md#bd-push)).

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body has a Patronus section (the
  details of the chip popover) after the pull request section. A branch without a robot has neither.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Landing overview

- The [landing overview](../repository/landing-overview.spec.md#overview) shows the chip next to the pull request chip of each row, as
  the shield alone (the state is its accessible name and in its popover): the branch names need the room.
- A robot that failed on the branch as it is (the local tip, or the same commits before a sync rebuilt them on a newer
  baseline, [above](#robot-of-a-row)) needs the user: the [landing indicator](../repository/landing-overview.spec.md#indicator) counts it as a move, the
  overview shows it once, as the move of its row with the robot's actions, the Dock its badge
  ([Robots in the Dock](../repository/robots-indicator.spec.md#dock)). A branch changed since (a fix) needs no attention,
  nor one that something else carries (`needsBranchRobotAttention`): a running Safe Push of a branch built on it or of
  the whole stack holds it, a merged one landed it whole (`PushHold`), or the baseline has all of it; its row waits
  ([Next Step](next-step.spec.md)), and the old failure is in "Last 24 hours".
  [@test] ../../src/lib/patronus.test.ts (`needs the user for a failed robot of the branch as it is, also after a sync rebuilt the branch`)
  [@test] ../../src/lib/robot-overview.test.ts (`asks nothing for the failure of a branch another robot holds or landed, or the baseline has: it is finished history`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should not count the failed robot of a branch while the Safe Push of a branch built on it lands it`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows`)
- The footer has a line "Patronus checked just now" (it ages while shown, as every relative time:
  [Update Project](../repository/update-project.spec.md)) with a Refresh button (a full list), named "Refresh
  Patronus". When Patronus can't answer, the line is a quiet hint instead: "Run ./safePush.cmd once to log in to
  Patronus", with "Copy Command" (`cd <repository> && ./safePush.cmd`, PowerShell's `cd <repository>; .\safePush.cmd`
  on Windows, `formatSafePushLoginCommand`), or "Patronus: <the error>". It is never a toast. A repository that is not on Space has no line.
  [@test] ../../src/lib/patronus.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../src/lib/check-counts.test.ts (`lists the failing checks first, then the running, the canceled and the passed ones`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should list the robots again from the overview`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should hint quietly when the Safe Push CLI is not logged in`)
  [@test] ../../src/lib/patronus.test.ts (`names the command of the login hint, into the repository, quoted for the shell of the platform`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say nothing for a repository that is not on Space`)

## User Experience

- The chip is the same text-sized button as the pull request chip (`StatusChip`): `text-xs`, a `size-3.5` shield,
  underlined on hover. The dot is `size-1.5 rounded-full`.
- The robot of a row reads as the status text it replaces: the chip at `text-sm` with a `size-3.5` icon where the status
  dot is, `h-6`, no padding, in the 9rem of the first column of the Status cell ("55m · usually ~40m" fits); its text
  truncates. The elapsed time is `text-foreground`, the rest muted, in tabular figures. The Safe Push slot is `w-40`
  (10rem); its step is a ghost `xs` button ("Restart Safe Push" with the `RotateCcwIcon`, the pull request with its
  icon), dimmed with the reason on hover while disabled.
- A running robot has the rotating ring (`RobotRing`: a track and an arc, `animate-robot-ring`, 2.4 s a turn, slower
  than a spinner), `text-primary` (`text-destructive` while failing, `robotRingColor`), in the Status cell, in the chip
  and around its node in the stack graph; nothing pulses. The progress line is `h-0.5`, `bg-primary` on a
  `bg-primary/15` track, as wide as the first column, under the time in the padding of the cell, so the row keeps its
  height; indeterminate, a third of it runs across (`animate-carousel`). "lands with oauth" is `text-xs
  text-muted-foreground`.
- The notice of a changed branch is `text-xs text-warning` after the warning triangle; the line of a rebuilt one is
  `text-xs text-muted-foreground`, without an icon (`ReviewCommitNotice`, shared with the pull request details).
- `info` is the merged colour, as for pull requests; push states never use it.
- The details popover is `w-80`; in the preview the same details sit in a nested `rounded-lg` panel.
- "Safe Push" (`SafePushButton`) is an `xs` button with the `ShieldCheckIcon`, ghost in the row and the preview footer,
  outline among the popover's buttons, in the foreground colour (the push colour is the push button's), its label in
  tabular figures while a start runs ("Pushing 45%", a spinner for the shield). Disabled, it keeps the focus and the
  hover (`focusableWhenDisabled`) and is dimmed to `opacity-50`; its reason is a `Tooltip`.
- The popover of a start is a `Popover` at the slot, `w-[28rem]`, aligned to its end; its title `text-sm font-medium`,
  its lines in sentences (muted, `warning` or `info` with their icon), its footer the status left of Dry Run, Cancel
  and the primary start.
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
  listed and polled; `get_patronus_robot_problems` answers none, `get_branch_review_readiness` the answer of
  `POST /repositories/{id}/review-answers` per branch, else "not required" ([E2E Testing](../../docs/design/e2e-testing.md)).
  The actions are answered in the page (`page.route`), which records their parameters. The E2E repositories have no
  remote: a push answered in the page (`routePush`, `pushBranchFromRow`) gives a branch its remote tip.
- Against the real Patronus: `pnpm run tauri dev`, open a checkout of `ultimate`; the log has one line per request
  ("listed Patronus robots", "polled Patronus robots") with its duration.

## Open Questions / Risks

- The pre-check is a hint: the review check of the robot decides (the chip shows it). `/space/merge-requests` finds only
  an open merge request from the branch into the target.
- The code-owner check counts the approvals of a GitHub pull request only for a robot its `/patronus` comment started;
  a REST start counts none. Until Patronus evaluates pull requests in the pre-check (its flag), an approval in the
  pull request makes the readiness `unknown`, and the robot decides.
- A mirror configured only in Space, not in its committed settings, needs `branchdeck.githubRepository` (Settings →
  Repository, GitHub repository); without it the branches are treated as on Space.

- The REST API is marked unstable (PAT-A-11): an unknown status or push mode shows as "Unknown status" instead of
  failing the list.
- Patronus matches `repository` exactly: a repository whose Patronus alias is not its Space name lists no robots.
- The token lives 14 days and is renewed only when the Safe Push CLI runs; the footer says so when it expired.
- The estimate knows only the user's own robots, and the profile only of the robots the app polled to their finish or
  whose details were open: a robot finished while the app was closed is a sample of its push mode only. Half of the
  robots run past the median, so "usually ~40m" shows near the end of many runs.
- The changed-since rule replaces a finished robot in the slot with "Safe Push" as soon as the branch has a commit
  the robot didn't test: its result is then only in the preview and the landing overview, under "Last 24 hours".

## References

- [Branch Row](branch-row.spec.md): the Actions cell with the Safe Push slot, the Status cell and the anatomy of the chip
  details
- [Pull Requests](pull-requests.spec.md): the other review status, the same chip and refresh gate
- [Outgoing Commits](../repository/outgoing.spec.md)
- [Landing Overview](../repository/landing-overview.spec.md): the running and failed robots in the header
- [Robots in the Dock](../repository/robots-indicator.spec.md#dock): the Dock badge and progress
- [Branch Preview](branch-preview.spec.md)
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): the pushed tip
- Patronus REST API: YouTrack PAT-A-11; Safe Push CLI: PAT-A-51
