---
name: Patronus
description: Requirements for the Patronus status of the virtual branches (Safe Push, Safe Merge, Dry Run), read over the Patronus REST API with the token of the Safe Push CLI, with the least load on Patronus, shown as a chip in the branch row, the branch preview and the outgoing overview; starting a Safe Push or a Dry Run of a branch with the code-owner review it needs, restarting and canceling its robot, and the question before a push into a branch whose Safe Push runs.
style: plain-1
targets:
  - ../../crates/sync-types/src/patronus.rs
  - ../../crates/patronus/src/actions.rs
  - ../../crates/patronus/src/client.rs
  - ../../crates/patronus/src/review.rs
  - ../../crates/patronus/src/service.rs
  - ../../crates/patronus/src/token.rs
  - ../../crates/sync-core/src/patronus.rs
  - ../../crates/sync-core/src/remote_status.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../src/stores/patronus.ts
  - ../../src/lib/patronus.ts
  - ../../src/lib/refresh-gate.ts
  - ../../src/components/branch/PatronusChip.tsx
  - ../../src/components/branch/InlinePatronusStart.tsx
  - ../../src/components/branch/PushGuardDialog.tsx
  - ../../src/components/branch/PatronusReviewBlock.tsx
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/usePatronusRobotDetails.ts
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/lib/git/push.ts
  - ../../src/stores/push-guard.ts
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/PatronusStatusLine.tsx
  - ../../src/hooks/useReviewStatusRefresh.ts
---

# Patronus

Status: Implemented
Date: 2026-10-14

## Summary

Changes reach the master of the IntelliJ monorepo through Patronus: a robot (Safe Push, Safe Merge, Dry Run) rebases
the branch onto the target, runs the TeamCity checks and, unless it is a dry run, pushes. Branch Deck shows, per virtual
branch, the robot of the branch: running, failed, merged or passed, and whether it tests the commit the branch is at.
From the branch, the user starts a Safe Push or a Dry Run (pushing the branch first), restarts a failed robot on the
newest commit, cancels a running one, and is asked before a push that would make a running Safe Push fail. It reads
Patronus with the Space token of the Safe Push CLI and keeps the load on Patronus small: one list now and then, and a
poll of the running robots only while the window is focused.

## Goals

- See which branches have a running, failed or merged Safe Push without opening the Patronus web UI.
- Notice that a robot tests an older commit than the local branch, e.g. after a fix was committed.
- Open the robot in Patronus in one click.
- Run the cycle "fix, push, restart" from Branch Deck, without the web UI and without breaking a running Safe Push.
- Be a well-behaved client: no request with a token Patronus rejected, no background request while Patronus fails, no
  poll while nothing runs or the window is in the background.

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
  the last request). A push lists nothing: whether a robot tests an older commit is known locally.
  [@test] ../../src/stores/patronus.test.ts (`asks after a sync every five minutes at most, on focus when older than two minutes, never after a push`)
- A repository that answered "not on Space" is asked again only by the refresh button.
  [@test] ../../src/stores/patronus.test.ts (`does not ask again for a repository that is not on Space, until the refresh button`)
- The running robots of the branches are polled with `POST /app/rest/v1/robots/fetch-updates` (the known status of
  each; only the changed ones come back) every minute, only while the window is focused and only while a branch has a
  running robot. A blur stops the timer; a focus polls at once, unless the last poll is less than 50 s old.
  [@test] ../../src/stores/patronus.test.ts (`polls the running robots of the branches every minute while the window is focused`)
  [@test] ../../src/stores/patronus.test.ts (`stops on blur and asks at once on focus`)
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
- One HTTP client with a kept-alive connection (connect timeout 5 s, list, poll and details 15 s, the review pre-check
  20 s, actions and merge requests 30 s) and the user agent
  `Branch Deck/<version> (<os>; <arch>)`, so Patronus can tell its load apart. `SAFEPUSH_PATRONUS_URL` (as for the
  CLI) points it at a staging instance.
- An answer keeps the robot objects that did not change, so only the chips of changed robots render again; a failed
  answer keeps the robots shown and only changes the availability.
  [@test] ../../src/stores/patronus.test.ts (`keeps the objects of unchanged robots and the record when nothing changed`)
  [@test] ../../src/stores/patronus.test.ts (`keeps the shown robots when Patronus can't answer`)

### Chip

- The Status cell of a branch row shows the chip after the pull request chip: a shield and the state. A branch without
  a robot shows nothing.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the status cell`)
- Running and canceling are muted, failed `destructive`, failed for missing approvals "Needs review" `warning`, a
  successful Safe Push "Merged" `info`, a successful dry run
  "Dry run passed" `success`, canceled muted. A robot that tests another commit than the local tip gets a `warning`
  dot, except a merged one.
  [@test] ../../src/lib/patronus.test.ts (`names the state in its hue; a successful dry run passed, a successful push merged`)
  [@test] ../../src/lib/patronus.test.ts (`marks a robot that tests another commit than the local tip, except a merged one`)
- Its accessible name is "Safe Push: Running" (the mode, then the state).
- Hovering the chip opens the details: the mode and the state, the name, the target branch, the commit, the start and
  finish times, the cancellation reason, the notice "Tests 01234567; the branch is at 89abcdef now." and "Open in
  Patronus". A click on the chip opens the robot page and does not toggle the row.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say when the robot tests another commit than the local branch`)

### Details

- The popover asks for the details of the robot 300 ms after it opened (a popover that only flashes asks nothing): the
  merge profile, the TeamCity checks ("1 failing (Fast Tests), 9 passed") and the problems of severity error. They are
  `GET /robots/{id}`, `/teamcity-checks` and `/problems?severity=ERROR` in parallel, cached by the backend: a finished
  robot for the session, a running one for a minute. When Patronus can't answer, the popover shows the list data.
  [@test] ../../crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_finished_robot_are_asked_once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Starting a Safe Push or a Dry Run

- The branch menu has "Safe Push…" and "Dry Run…" in a Patronus group when the repository is on Space (Patronus
  answered), disabled while a robot runs on the commit the branch is at (Patronus would refuse it). The preview footer
  has "Safe Push…": it closes the preview and opens the form of the row.
- The form below the row says what happens, offers the merge profiles of the target branch that allow the mode (the
  emergency ones never), "No profile" and, by default, the profile of the last robot of the branch; nothing is
  remembered by Branch Deck. The profiles are `GET /robot-start-supported`, cached for 10 minutes (an error for 1).
  A target branch Patronus does not protect can't be started.
  [@test] ../../crates/patronus/src/actions_test.rs (`start_options_leave_out_the_emergency_profiles_and_default_to_the_profile_of_the_last_robot`)
  [@test] ../../src/lib/patronus.test.ts (`offers a profile for the modes it allows; none listed allows any`)
  [@test] ../../src/lib/patronus.test.ts (`blocks a start while a robot runs on the commit the branch is at`)
- The target branch is the branch of `origin` the baseline tracks (`origin/master` → `master`); a local baseline or one
  on another remote can't be started.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_the_baseline_tracks`)
- A branch whose tip the remote lacks is pushed first, as the push button does it ("Push and Start Safe Push"); a failed
  push starts nothing. Then `POST /robots` starts the robot on the pushed virtual branch, as the web UI does
  (`ROBOT_WEB_UI`, the source branch `refs/heads/{prefix}/virtual/{name}`, kept: delete policy `NEVER`), so a restart
  takes the newest commit of the branch. The robot shows at once and is polled.
  [@test] ../../crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
- A start is never repeated by itself, with one exception: right after the push, Space may not show the branch to
  Patronus yet, so a 400 is asked once more 5 s later.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_start_right_after_the_push_is_asked_once_more_when_patronus_does_not_see_the_branch`)
  [@test] ../../crates/patronus/src/actions_test.rs (`a_failed_start_is_never_repeated`)

### Code-owner review

- Patronus fails a Safe Push whose changes miss the approvals the code owners require (`.ownership/review-rules.yaml`
  of the target branch, PAT-A-109): the problem "Changes require review approval". The approvals count only in the
  Space merge request linked to the robot. A dry run is not checked.
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
  (503) is said and not asked again by itself.
  [@test] ../../crates/patronus/src/review_test.rs (`a_required_review_is_asked_again_on_a_recheck`)
  [@test] ../../crates/patronus/src/review_test.rs (`an_unavailable_review_check_is_not_asked_again_by_itself`)
- The form shows "No code-owner review needed.", "Approved in IJ-CR-7.", or a `warning` block: "Needs review by the
  code owners" or "Waiting for approval in IJ-CR-7", the groups with the available reviewers Patronus suggests ("Air
  Plugin Release Team (suggested: Alice)"), "A Safe Push fails without these approvals.", "Create Merge Request" (the
  branch is pushed first; the merge request opens in the browser) or "Open IJ-CR-7", and Recheck. Start stays possible.
  [@test] ../../src/lib/patronus.test.ts (`names the group and the available reviewers Patronus suggests`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should name the missing approvals, create the merge request and start with it`)
- The start links the merge request of the branch (`spaceReview`), so its approvals count.
- A robot that failed for missing approvals (its problems are read once per session, for the failed robots of the
  branches) is "Needs review" in `warning`; its popover says how to go on and offers "Safe Push…" instead of Restart
  (a restart would check the same merge request).
  [@test] ../../src/lib/patronus.test.ts (`names a robot that failed for missing approvals "Needs review"`)
  [@test] ../../src/stores/patronus.test.ts (`reads the problems of a failed robot of a branch once`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and offer a new Safe Push`)

### Restart and cancel

- Restart (the popover, the branch menu) is offered for a failed, canceled or canceling robot started on the branch,
  and for a passed dry run when the remote branch has a newer commit; Patronus refuses the others. Not for a robot that
  failed for missing approvals. A Safe Push of a
  temporary ref (IDE, CLI) is not restarted from here: it would test its old commit. The restart is a new robot.
- Cancel is offered for a running robot; the robot is canceling until its checks stopped.
  [@test] ../../src/lib/patronus.test.ts (`restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit`)
  [@test] ../../src/lib/patronus.test.ts (`cancels a running robot only`)
  [@test] ../../crates/patronus/src/actions_test.rs (`restart_answers_the_new_robot_and_cancel_the_canceling_one`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
- A failed action is a toast with what Patronus said; without a token it says "Run ./safePush.cmd once to log in to
  Patronus" and sends nothing.
  [@test] ../../crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Pushing into a branch whose Safe Push runs

- Patronus tests the commit it took at the start and fails a Safe Push whose branch changed meanwhile ("branch was
  modified"). Every push of the branch (the row, the overview, the preview, `P`) whose Safe Push runs on the branch reads
  the status of the robot first (`GET /robots/{id}/status`, one row); still running, a dialog asks: "Cancel, Push and
  Restart" (cancel, push, restart on the pushed commit), "Push Anyway" or "Back" (Escape). A dry run is not affected
  and asks nothing; a robot that finished meanwhile asks nothing.
  [@test] ../../src/lib/patronus.test.ts (`guards a push into the branch of a running Safe Push, not of a dry run`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should ask first, and push only when told to`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot, push and restart it on the pushed commit`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push at once when the robot finished meanwhile`)

### Branch preview

- The meta line of the [branch preview](branch-preview.spec.md) ends with the chip; the body has a Patronus section (the
  details of the chip popover) after the pull request section. A branch without a robot has neither.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Outgoing overview

- The [outgoing overview](../repository/outgoing.spec.md) shows the chip next to the pull request chip of each row, as
  the shield alone (the state is its accessible name and in its popover): the branch names need the room.
- The indicator gets a `destructive` dot when a robot failed on the commit the branch is at.
  [@test] ../../src/lib/patronus.test.ts (`is set by a failed robot of the commit the branch is at`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the chips in a column and a dot for a robot that failed on the branch tip`)
- The footer has a line "Patronus checked just now" with a Refresh button (a full list). When Patronus can't answer,
  the line is a quiet hint instead: "Run ./safePush.cmd once to log in to Patronus" or "Patronus: <the error>". It is
  never a toast. A repository that is not on Space has no line.
  [@test] ../../src/lib/patronus.test.ts (`hints at what to do, and says nothing when there is nothing to do`)
  [@test] ../../src/lib/check-counts.test.ts (`lists the failing checks first, then the running, the canceled and the passed ones`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should list the robots again from the overview`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should hint quietly when the Safe Push CLI is not logged in`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say nothing for a repository that is not on Space`)

## User Experience

- The chip is the same text-sized button as the pull request chip (`StatusChip`): `text-xs`, a `size-3.5` shield,
  underlined on hover; the running shield pulses. The dot is `size-1.5 rounded-full`.
- `info` is the merged colour, as for pull requests; push states never use it.
- The details popover is `w-80`; in the preview the same details sit in a nested `rounded-lg` panel.

## Testing / Local Run

- `cargo nextest run --workspace --exclude model-ai` covers the token, the request policy (rejected token, backoff,
  retries, reuse of a full list) against an in-process axum Patronus (`crates/patronus/src/service_test.rs`), and the
  Space URL parsing.
- `pnpm test:unit` covers the list and poll policy, the matching and the chip mapping.
- `pnpm e2e` covers the chip, the popover, the preview section and the overview (`tests/e2e/patronus`). E2E never calls
  Patronus: `POST /repositories/{id}/patronus` sets what `get_patronus_robots` answers (an empty `sourceRevision` means
  the local tip of the robot's branch), what `poll_patronus_robots` answers (the fixture robots whose status differs
  from the known one) and the robot of `get_patronus_robot_details` (without checks); `GET` returns how often the app
  listed and polled; `get_patronus_robot_problems` answers none and `get_patronus_review_requirement` "not required".
  The actions are answered in the page (`page.route`), which records their parameters.
- Against the real Patronus: `pnpm run tauri dev`, open a checkout of `ultimate`; the log has one line per request
  ("listed Patronus robots", "polled Patronus robots") with its duration.

## Open Questions / Risks

- The pre-check is a hint: the review check of the robot decides (the chip shows it). `/space/merge-requests` finds only
  an open merge request from the branch into the target.

- The REST API is marked unstable (PAT-A-11): an unknown status or push mode shows as "Unknown status" instead of
  failing the list.
- Patronus matches `repository` exactly: a repository whose Patronus alias is not its Space name lists no robots.
- The token lives 14 days and is renewed only when the Safe Push CLI runs; the footer says so when it expired.

## References

- [Pull Requests](pull-requests.spec.md): the other review status, the same chip and refresh gate
- [Outgoing Commits](../repository/outgoing.spec.md)
- [Branch Preview](branch-preview.spec.md)
- [Remote Status for Virtual Branches](../../docs/design/remote-status.md): the pushed tip
- Patronus REST API: YouTrack PAT-A-11; Safe Push CLI: PAT-A-51
