---
name: Patronus
description: Requirements for the Patronus status of the virtual branches (Safe Push, Safe Merge, Dry Run), read over the Patronus REST API with the token of the Safe Push CLI, with the least load on Patronus, shown live in the Safe Push slot of the branch row (its time and how long it usually takes, a progress line, the branches it lands, "Failing" once a required check failed) and as a chip in the branch preview and the landing overview; the toast and the OS notification when a robot merged a branch, with a fetch of the baseline, and when a required check of a running robot failed, with its cancel; starting a Safe Push or a Dry Run of a branch (from the row, the branch menu, the preview and the chip popover) with the code-owner review it needs (the approvals in the GitHub pull request; Space merge requests are deprecated), or from its GitHub pull request with a /patronus comment when the branches are reviewed on GitHub; restarting and canceling its robot, and no push of a branch a running Safe Push lands but its Push and Restart.
style: plain-1
targets:
  - ../../src/lib/os-notification.ts
  - ../../crates/domain-types/src/patronus.rs
  - ../../crates/domain-policy/src/review.rs
  - ../../crates/domain-policy/src/actions.rs
  - ../../crates/patronus/src/actions.rs
  - ../../crates/patronus/src/client.rs
  - ../../crates/patronus/src/review.rs
  - ../../crates/patronus/src/review_readiness.rs
  - ../../crates/remote-facts/src/review_readiness.rs
  - ../../crates/sync-core/src/robot_landing.rs
  - ../../crates/object-cache/src/review_answer.rs
  - ../../crates/object-cache/src/patronus_robots.rs
  - ../../crates/bd-remote/src/robot_list.rs
  - ../../crates/patronus/src/service.rs
  - ../../crates/patronus/src/token.rs
  - ../../crates/sync-core/src/patronus.rs
  - ../../crates/remote-facts/src/patronus_command.rs
  - ../../src/lib/pull-request.ts
  - ../../src/presenters/pull-request-url.ts
  - ../../crates/sync-core/src/remote_status.rs
  - ../../crates/app-api/src/remote/patronus.rs
  - ../../src-tauri/src/services.rs
  - ../../crates/trial-check/src/landing_gate.rs
  - ../../crates/domain-types/src/landing.rs
  - ../../src/presenters/landing.ts
  - ../../src/presenters/lock-wait.ts
  - ../../src/presenters/operations.ts
  - ../../crates/operations/src/registry_watch.rs
  - ../../crates/remote-facts/src/patronus_review.rs
  - ../../src/stores/patronus.ts
  - ../../src/lib/patronus-failing.ts
  - ../../crates/remote-facts/src/scheduler.rs
  - ../../crates/remote-facts/src/fact_sources.rs
  - ../../crates/repository-model/src/facts.rs
  - ../../src/lib/patronus.ts
  - ../../crates/domain-policy/src/attention.rs
  - ../../src/lib/review-commit.ts
  - ../../src/lib/robot-coverage.ts
  - ../../src/stores/robot-coverage.ts
  - ../../src/components/commit/RobotCommitMarker.tsx
  - ../../src/components/branch/PatronusChip.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/RobotProgress.tsx
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/hooks/useLandedIn.ts
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/lib/robot-eta.ts
  - ../../src/stores/robot-history.ts
  - ../../src/stores/robot-details.ts
  - ../../src/lib/patronus-icons.ts
  - ../../src/hooks/useRobotTiming.ts
  - ../../src/components/branch/ReviewCommitNotice.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/presenters/land.ts
  - ../../src/presenters/outcome.ts
  - ../../crates/operations/src/reconcile.rs
  - ../../crates/operations/src/forget.rs
  - ../../crates/operations/src/robot_start.rs
  - ../../src/lib/robot-run.ts
  - ../../src/lib/robot-items.ts
  - ../../crates/operations/src/land_execute.rs
  - ../../crates/operations/src/land_session.rs
  - ../../crates/operations/src/engine.rs
  - ../../crates/domain-policy/src/push.rs
  - ../../crates/domain-policy/src/holds.rs
  - ../../crates/domain-policy/src/land.rs
  - ../../crates/domain-policy/src/readiness.rs
  - ../../crates/domain-policy/src/refresh.rs
  - ../../crates/domain-policy/src/reasons.rs
  - ../../crates/domain-types/src/reason.rs
  - ../../crates/repository-model/src/derive.rs
  - ../../src/presenters/reasons.ts
  - ../../crates/sync-core/src/target_conflict.rs
  - ../../crates/git-ops/src/trial_rebase.rs
  - ../../src/stores/safe-push.ts
  - ../../crates/repository-model/src/push_guard.rs
  - ../../crates/operations/src/push.rs
  - ../../crates/operations/src/land.rs
  - ../../crates/app-api/src/session/operations.rs
  - ../../crates/operations/src/robot.rs
  - ../../src/components/branch/CreatePullRequestDialog.tsx
  - ../../src/lib/branch-rules.ts
  - ../../crates/remote-facts/src/branch_rules.rs
  - ../../crates/sync-core/src/patronus_start.rs
  - ../../crates/domain-policy/src/robot_match.rs
  - ../../src/lib/patronus-stack.ts
  - ../../crates/operations/src/close.rs
  - ../../src/presenters/news.ts
  - ../../crates/domain-policy/src/landed.rs
  - ../../crates/domain-policy/src/news.rs
  - ../../src/lib/whole-stack.ts
  - ../../crates/remote-facts/src/pull_request_writes.rs
  - ../../src/hooks/usePatronusReviewRequirement.ts
  - ../../src/hooks/useStackIndependence.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/hooks/useRobotDetails.ts
  - ../../crates/remote-facts/src/scheduler_details.rs
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/hooks/useClosePreview.ts
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/lib/patronus-merge-request.ts
  - ../../src/lib/push-run.ts
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/PatronusStatusLine.tsx
  - ../../src/components/repository/ReviewStatusLine.tsx
  - ../../src/hooks/useReviewStatusRefresh.ts
  - ../../src/stores/update-project.ts
  - ../../crates/sync-core/src/baseline_fetch.rs
  - ../../crates/operations/src/update.rs
  - ../../src/model/intents.ts
  - ../../src/presenters/actions.ts
  - ../../src/components/ui/toast.tsx
  - ../../src/lib/event-toast.ts
  - ../../src/lib/robot-toast.ts
  - ../../src/lib/landing-toast.ts
  - ../../src/components/branch/BranchRowStatus.tsx
  - ../../src/hooks/useRowRobot.ts
  - ../../crates/domain-policy/src/admission.rs
  - ../../crates/operations/src/registry.rs
  - ../../crates/operations/src/witness.rs
  - ../../crates/operations/src/events.rs
  - ../../crates/operations/src/needs.rs
  - ../../crates/operations/src/facts.rs
  - ../../crates/operations/src/registry_file.rs
  - ../../crates/repository-model/src/patronus_view.rs
  - ../../crates/domain-types/src/operation.rs
  - ../../crates/domain-types/src/robot_landing.rs
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
  finished one, then the newest. `bd` and the session of the repository match them with one rule
  (`domain_policy::robot_match`: one index per list of robots); the session names the robot of each row in its view
  (`robotId` of the Patronus part), so the page matches none itself.
  [@test] ../../crates/patronus/src/service_test.rs (`lists_the_robots_newest_first_and_names_the_virtual_branch`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`, `the_table_gets_the_robot_of_each_branch_and_whole_stack_that_has_one`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] ../../src/stores/patronus.test.ts (`takes the robot of each row from the view of the session, which matched it, never from its own matching`)
- Every command of Patronus, of a push and of a pull request names the repository only: Rust resolves the prefix
  `{prefix}` of its branches for the command, as the sync does (`BranchPrefixes`, no git process while it knows the
  repository's git prefix), and the operations of the remote (the engine: the push, the start, the restart, the cancel,
  the retarget) take that prefix, never one from the page. The page keeps its stores by the prefix it
  shows ([Settings](../repository/settings.spec.md)).
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`the_robots_of_the_fixture_come_through_the_real_service`, `a_start_restart_and_cancel_run_the_real_service_against_the_fake`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_of_a_stack_runs_the_processes_of_the_core_push`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`the_robots_of_the_fixture_come_through_the_real_service`)
- The pushed tip comes with the remote status of the branch: the `rev-parse` that checks the remote branch exists
  returns it, so it costs no extra git process.
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)

### Load on Patronus

- The session of the repository lists and polls the robots, not the page: one scheduler of the facts of the sessions
  on the clock of the sessions (`remote_facts::scheduler`, by the rules of `domain_policy::refresh`), one list or poll
  in flight per session, nothing for a session no window shows; each answer reaches the windows as a patch of the
  robots by id (`facts.robots` of the view), which the page stores (`applyRobotsFacts`). The refresh button and the
  list a `/patronus` comment needs after the bot's 👍 ask the session (`refresh_facts`) and wait for the patch of that
  list.
  [@test] ../../crates/repository-model/src/session_test.rs (`the_reads_of_the_facts_reach_the_windows_and_the_scheduler_hears_what_it_asks_by`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_nothing_for_a_session_no_window_watches`, `a_closed_session_asks_nothing_and_answers_a_refresh_with_none`)
- The first list (after the first sync of a session a window shows) and the refresh button ask for the last 7 days;
  every other list asks only for the robots started since 2 minutes before the previous list started. A full list asked again within 10 s (a second window of the
  repository) is answered from the last one, unless the robots changed since (a start, a restart, a cancel, a poll
  that saw a change). The list of a check before an action of the app's page (its push, a restart, a cancel) is asked
  anew every time, never answered from a kept one: a robot the IDE or `bd` started since the page's list must hold, and
  the list it fetched answers the page's next list. A start of a Safe Push or a Dry Run asks none of its own (C2): it
  is an operation of the session (`execute`), whose plan, witness, push and start read the robots the session holds,
  and whose robot goes into them at once.
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_through_execute_asks_no_list_of_its_own`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`lists_the_last_days_first_then_only_the_robots_started_since_shortly_before_the_last_list`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`lists_the_full_window_on_the_refresh_button_asked_by_the_user`)
  [@test] ../../crates/patronus/src/service_test.rs (`reuses_a_fresh_full_list_but_not_an_incremental_one`, `the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere`, `a_poll_that_sees_a_change_drops_the_kept_full_list`)
  [@test] ../../crates/patronus/src/service_test.rs (`a_full_list_in_flight_when_the_robots_changed_is_not_kept`)
  [@test] ../../crates/patronus/src/actions_test.rs (`a_start_a_restart_and_a_cancel_forget_the_full_list`)
- A sync lists at most every 5 minutes, a focus when the robots are older than 2 minutes (and at least a minute after
  the last request). A push lists nothing: whether the branch changed since a robot started is known locally. The
  refresh button and the list after a `/patronus` comment always ask, and once more after a list in flight (which may
  have started before the robot); a sync or a focus during a list joins it.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_after_a_sync_every_five_minutes_at_most_on_focus_when_older_than_two_minutes_never_after_a_push`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_refresh_button_and_a_start_during_a_list_ask_once_more_after_it`, `lists_the_robot_a_start_needs_at_once_since_shortly_before_the_last_list`)
- A repository that answered "not on Space" is asked again only by the refresh button.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`does_not_ask_again_for_a_repository_that_is_not_on_space_until_the_refresh_button`)
- The running robots of the branches and the whole stacks, and the watched ones, are polled with
  `POST /app/rest/v1/robots/fetch-updates` (the known status of each; only the changed ones come back) only while one
  of them runs: every minute while a window of the session is focused (the focus of the window's demand); while none
  is, every 5 minutes and only while a robot that pushes (not a dry run) runs, so its merge is told without the
  window. A focus polls at once, unless the last poll is less than 50 s old. The session matches the robots to the
  tips of its table as the rows do (`domain_policy::robot_match`), a push of the session included; a robot a start, a
  restart or a cancel answered goes into the session at once (`Sessions::write_robot`) and is polled from then on.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`polls_the_running_robots_of_the_branches_every_minute_while_the_window_is_focused`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`stops_on_blur_and_asks_at_once_on_focus`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`polls_a_robot_that_pushes_every_five_minutes_while_the_window_is_not_focused_a_dry_run_not_at_all`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`polls_nothing_without_running_robots`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones`)
  [@test] ../../crates/patronus/src/service_test.rs (`polls_only_running_robots_with_their_known_status`)
- After each list and each poll the session answers, the running robots (not a canceling one) of the branches, the
  whole stacks and the watched ones that are not known failing yet are asked whether they will fail
  ([Failing](#failing)): `GET /robots/{id}/problems` of each, in parallel, in one read of the session, through the
  request policy below. A robot known failing is not asked again: it stays failing until it finishes. So a poll costs
  at most one small request more per running robot, and none once each is known: a poll that changed nothing while
  every running robot is known failing and the problems of the failed ones are known is one request. Only an answer
  asks: a sync end or a focus whose list the policy holds back, and a list or a poll Patronus didn't answer, ask
  nothing. One such question runs per session at a time.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`asks_after_an_answer_only_sync_ends_and_focus_events_the_list_gate_refuses_ask_nothing_each_poll_still_asks`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`an_idle_poll_asks_one_request_and_problems_only_for_changed_robots`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones`)
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
  with the highest number), in parallel, after the details' checks; again once a minute while they stay open (61 s on
  the clock of the session, as Patronus caches the progress for a minute: never faster), never after they closed. The
  details are a fact of the session of the repository (`facts.robotDetails`): a window demands them while a surface
  shows them (`robotDetails` of its demand), the session reads them by its scheduler and drops them once no window
  demands them, so two windows ask them once; the backend caches the progress of each build for 60 s. No request comes
  from the row, the landing indicator, the Dock or a timer of the page. The request goes through the request
  policy above (the token, the backoff, the user agent, a GET asked once more after a transient failure, the log line
  "read the build progress" with its duration).
  [@test] ../../crates/patronus/src/actions_test.rs (`details_of_a_running_robot_have_the_estimate_of_its_running_checks`, `the_progress_of_a_build_is_asked_once_a_minute_and_never_for_a_finished_robot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should ask the session nothing for the hover and the menu of a row, and tell it only the details of the robot whose popover opens`)
- With Settings → Landing, "New Patronus API" (`appSettings.patronusNewApi`, off by default; Rust reads it from the
  settings writer for each command, the page sends nothing), the same estimate comes from one request, `GET /robots/progress?robotId=…`, which Patronus
  folds from the same cache of a minute: the latest estimated finish over the running checks (`estimatedFinishAt`,
  the time left or the overtime against now), the check that ends last and its fraction done, the queued checks (also
  a running check whose build waits for an agent or for its next attempt), whether one probably hangs; a robot
  without a running or waiting check has no progress, as before its checks run. The answer is kept for 60 s per
  robot, as a build's is. The endpoint is behind the `robot.progress.enabled` flag of Patronus: a 404 means it is off
  for the user, then the builds are asked as above and the endpoint rests for 10 minutes. `bd robot` reads the setting
  from the app's settings file ([Agent CLI](../../docs/contributing/agent-cli.md)).
  [@test] ../../crates/patronus/src/actions_test.rs (`with_the_new_api_the_progress_of_a_running_robot_is_one_request_kept_for_a_minute`, `while_patronus_has_no_progress_of_robots_the_builds_are_asked_and_the_endpoint_rests`, `the_progress_patronus_folded_reads_as_the_one_from_the_builds`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`the_details_a_window_demands_ask_the_folded_progress_only_with_the_new_api_of_the_settings`)
  [@test] ../../crates/bd-core/src/context_test.rs (`the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file`)
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
  [@test] ../../crates/bd-remote/src/robot_list_test.rs (`the_robots_are_kept_while_they_hold_then_the_new_ones_are_listed_and_the_running_ones_polled`, `a_rejected_token_and_a_failure_hold_the_requests_and_keep_the_robots`, `status_asks_patronus_once_while_the_kept_robots_hold`)
- An answer patches only the robots it changed, so the page keeps the objects of the others and only the chips of
  changed robots render again; a failed answer keeps the robots shown and only changes the availability (the fact
  `Stale` after robots were listed, `Unavailable` before). Each robot keeps the number of the read or the write that
  wrote it: a list that started before a start or a poll answers after them and rolls neither back.
  [@test] ../../crates/repository-model/src/facts_test.rs (`keeps_the_objects_of_unchanged_robots_and_the_record_when_nothing_changed`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`keeps_the_shown_robots_when_patronus_cant_answer`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`a_list_started_before_a_write_or_a_poll_never_rolls_their_robot_back`, `keeps_the_github_repository_of_a_list_through_a_poll`)

### Robot of a row

- The Status cell of a branch row shows the robot of the branch as the state of the branch, in the first column of the
  cell, where the status text is otherwise ([Branch Row](branch-row.spec.md#status-cell), `BranchRowStatus`,
  `getRowRobotChip`):
  - while a robot runs (or cancels): a rotating ring, how long it runs and how long it has left ([Timing](#timing)),
    "Canceling" while it cancels; one that will fail ([Failing](#failing)) says "18m · Failing" instead of the time
    left, no use waiting for it;
  - a finished robot of the branch as it is (the commit it took, or the same commits rebuilt on a newer baseline): its
    state in its hue, "Failed", "Needs review", "Merged", "Dry run passed", "Canceled". As it is means the piece its
    Safe Push lands: a stacked branch whose remote branch lacks new commits of a branch it is built on
    (`ancestorCommitsAhead` of its remote status) changed since its robot, which tested the older piece; its start and
    its restart push it first ("Push and Restart");
  - a robot of an earlier state of the branch (the warning dot below, a merged robot included: the same name with new
    work): the git status of the branch as it is. The robot stays in the branch preview and the landing overview, with
    the notice of what changed.
  A sync running, an error or a conflict, a block by the stack parent and a landed branch say more than the robot: the
  status text then.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
  [@test] ../../src/lib/patronus.test.ts (`shows a running robot and a finished one of the branch as it is in the Status cell, the git status otherwise`, `is the live robot while it runs, also while it cancels`, `offers "Safe Push…" for a branch changed since its robot, a merged one too`, `names the change and the restart that tests the branch as it is`)
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`, `the_whole_stack_takes_the_rows_of_its_stack_branch`)
  [@test] ../../crates/sync-core/src/detection_remote_status_test.rs (`a_child_whose_parent_got_a_new_commit_lacks_it_on_its_remote`)
- The Actions cell has the Safe Push slot after the push button ([Branch Row](branch-row.spec.md#actions-cell)), in a
  column of its own, so the push buttons, the steps and "⋯" line up down the table and its islands. It shows the next step of the branch, the
  first row of the table of [Next Step](next-step.spec.md) that matches (`domain_policy::next_step`), from the robot, the pull
  request and the readiness Patronus answers ([below](#code-owner-review)), and every other "Safe Push" is disabled
  with the same reason:
  - "Safe Push" without a robot, for one of an earlier state of the branch and after a dry run
    ([below](#starting-a-safe-push-or-a-dry-run));
  - "Restart Safe Push" after a failed or canceled robot of the branch as it is, the action of its details
    ([below](#restart-and-cancel)), disabled with the reason where a start is not possible; "Safe Push" where Patronus
    can't restart it (a Safe Push of a temporary ref); after a robot that failed for missing code-owner approvals only
    once Patronus says they count in the pull request it was started from, else "Open Pull Request" (or "Create Pull
    Request…") while nobody can tell yet, or the wait for the approvals;
  - the wait for the approvals that count: "Awaiting Review" (whom on hover), "Changes Requested", "Request Review…",
    "Create Pull Request…"; "Draft", "Retarget #7" (to master on hover); a stacked branch's by its own pull request;
  - nothing while a robot runs (Cancel is in "⋯", the details and the landing overview), after a merge, for a landed
    branch and a branch a Safe Push holds.
  Nothing hovers in: the state stays in the Status cell where the eye reads it, the step stays the step. A repository
  that is not on Space has no slot; a branch that can't be pushed (an error, no commits) has an empty one.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] ../../src/lib/patronus.test.ts (`offers "Safe Push…" without a robot, nothing where Patronus doesn't answer or the branch can't be pushed`, `shows a finished robot of the branch as it is, and whether a next step can be offered`)
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)
  [@test] ../../src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)
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
- Hovering the robot opens the details: the mode and the state with "Open in Patronus" as an icon at its end, the name,
  the target branch, the commit, the start and finish times, the cancellation reason, the notice about the commit
  (below) and the actions of the robot ([below](#restart-and-cancel)) in two rows: what decides the robot, the restart
  as the primary button and the cancel; then "Copy Prompt for Agent" as a split button whose menu has "Copy Prompt
  with CI Details", and "Open Pull Request #12". A click opens the robot page, leaves the details as they are
  ([hover descriptions](../../docs/design/ui-theme.md)) and does not toggle the row.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)
- With the dot, the details say what changed and how to test the branch as it is: "Started on 3264f30f; the branch has
  changed since (now 88de421d): the result won't cover the newest commits. Restart Safe Push to test the current
  branch.", with "Restart Safe Push". When the remote lacks the change, the restart pushes it first, under the same
  name ([below](#restart-and-cancel)). For a running robot of the branch the remote moved past, "Cancel Safe Push, then
  Restart Safe Push to test the current branch."; one of a branch changed here, "Restart Safe Push to test the current
  branch.": its push cancels the robot first. The advice names the action as its button
  does (`formatRobotRestartLabel`). A robot that can't be restarted from here (a Safe Push of a temporary ref, "Needs
  review") gets no advice.
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../src/lib/patronus.test.ts (`asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say when the branch changed since the robot started, and how to test the branch as it is`)
- A branch only rebuilt on a newer baseline (the remote branch at the robot's commit, the branch up to date, other
  hashes) gets no dot; the details say quietly "Tests the same commits on an older master." ("Tested …" once it
  finished).
  [@test] ../../src/lib/patronus.test.ts (`says quietly that a robot tests the same commits on an older baseline, nothing for the branch as it is`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should say only quietly that a robot tested the same commits on an older baseline`)

### Commits a running robot covers

- The commits of a branch show which of them the running robot that tests it covers: its own robot, else the one that
  lands it with a branch built on it or with the whole stack (the ring and the landing span of the graph). A finished
  robot marks nothing; the [failure prompt](safe-push-failure-prompt.spec.md) lists what it checked.
- While the robot took the branch as it is, or the same commits on an older baseline, it covers every commit. Once the
  branch changed since (the `warning` dot, [above](#robot-of-a-row)), the app asks the commit map of the robot
  (`get_robot_commit_map`, the map of the failure prompt), once per tip of the branch it tests, never during a sync.
  Without the robot's commit, before the remote status loads, or without an answer, the rows keep the plain rail.
  [@test] ../../src/lib/robot-coverage.test.ts (`covers every commit while the robot took the branch as it is, or the same commits on an older baseline`, `needs the commit map once the branch changed since the robot took it`, `tells nothing without the robot's commit, the tip, or the remote status of a changed branch`)
  [@test] ../../src/lib/robot-coverage.test.ts (`takes the commits of the branch by their local hash, and none while the robot's commit is not here`)
  [@test] ../../src/stores/patronus.test.ts (`asks the commit map of a running robot once its branch changed since it started, once per tip, and drops it once the robot covers the branch`, `asks nothing for a finished robot`)
- On the rail of the branch, the line of the robot runs over the commits it covers: a 1.5px stroke in
  `--color-primary`, the colour of the landing span, from the top of the rail or the newest covered commit down to the
  last covered one. The newest covered commit below a commit the robot lacks gets a `--color-primary` dot. A commit
  added or changed since the robot started gets a hollow dot with a 1px `--color-warning` border, on the plain line.
  The full list of a branch draws it, in the table, a stack island and the Parked card alike.
  [@test] ../../src/lib/robot-coverage.test.ts (`runs the line of the robot over the commits it covers and marks the ones added or changed since`, `draws the line from the top of the rail down to the last dot while the robot covers every commit`, `finds the newest covered commit below the added ones also in a list that is oldest first`, `keeps the plain dot for a commit whose state is not known`)
  [@test] ../../tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over the commits it took, and mark the one added after it started`, `should draw the line of the robot over every commit while it took the branch as it is`)
- The row of such a commit in the full list says "not in Safe Push" ("not in Dry Run") after its subject, a `warning`
  chip. Its details: "Added after the Safe Push started on 3264f30f" (or "Changed since…"), the robot and its start,
  "The result won't cover this commit. Restart Safe Push to test the current branch." ("stack" for the robot of the
  whole stack), and the actions of the robot as its own details name them.
  [@test] ../../tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over the commits it took, and mark the one added after it started`)

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
  [@test] ../../crates/repository-model/src/facts_test.rs (`keeps_the_merge_profile_a_poll_the_details_or_the_start_named_when_a_list_which_never_names_it_answers_the_robot`)
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
- The row and the lists of robots say how long the robot runs and how long it has left: "18m · ~22m" (the time left
  rounded up; "about 22 minutes left" in the hover and for the screen reader); past its usual time, the usual time
  instead, "55m · usually ~40m" (the hours only from an hour: "14h · usually ~1h"); without history, "18m". A sentence
  that names no elapsed time says "left": "Safe Push of deploy lands 5 branches · ~23m left". The time ticks with the relative times, every 15 s, without a request.
  [@test] ../../src/lib/robot-eta.test.ts (`says how long it runs and how long it usually takes`, `names the usual time once a robot runs longer, in hours once it runs an hour`, `counts the time left until the usual duration, indeterminate past it and without history`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)
- A thin line under the time of a running robot in the Status cell shows the share of its usual time; without history,
  past it and while it cancels, it runs across (indeterminate).
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)
- While the details of a running robot are open, they show the estimate of the CI in the row "Progress": the time left
  of the running check that ends last (its estimate minus its running time) and its name and percent, "about 9 min
  left · Unit Tests 93%"; "1 check queued" when checks wait in the queue; "Overtime 4 min" when every running check
  ran past its estimate; "probably hanging" in `warning` when the CI says so; no row while the CI tells none of it. The time and the progress line of the row
  count down the time left it read meanwhile ("20m · ~9m left"), and the start line of the details keeps only the usual
  time ("usually ~1 h 50 min"); once the details closed, the time and the line take the estimate
  from history again. A running robot row of the landing overview shows the check and the queued checks too.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did`)
  [@test] ../../src/lib/robot-eta.test.ts (`names the time left, the check the robot waits for, and the queued checks`, `counts down the time left it read while the details were open, for the slot`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
- The details of a running robot say it after the start time, with what the estimate is of: "Today 14:02 · usually ~1 h
  50 min for Large Scale Change · about 1 h 32 min left", "… without a merge profile …", the rough "… · usually ~40 min
  for Safe Push · 55 min, longer than usual", "… · running 18 min · no Safe Push finished in the last 30 days to
  estimate from". The slot doesn't name it. The details have the profile row when the profile is known, also before
  the details answered, and the start line names no profile that row names: "Today 14:02 · usually ~1 h 50 min · about
  1 h 32 min left"; a robot row of the landing overview names it on hover of its time: "Safe Push · Large Scale
  Change".
  [@test] ../../src/lib/robot-eta.test.ts (`explains the estimate and what it is of in the details`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`, `should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

### Chip

- The branch preview, the landing overview and the Landed island show the robot as a chip, the same text-sized button
  as the pull request chip (`StatusChip`): the shield and the state in its hue (the ring while it runs), the warning dot,
  the details on hover, the robot on click. A branch without a robot shows nothing.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

### Details

- While the popover is open the window demands the details of the robot of the session (the demand goes out at most
  four times a second): the merge profile, the TeamCity checks ("1 failing (Fast Tests), 9 passed") and the problems
  of severity error. They are `GET /robots/{id}`, `/teamcity-checks` and `/problems` (every severity: the errors are
  shown, the warning `robot_is_failing` marks the robot failing, [below](#failing)) in parallel, cached by the
  backend: a finished robot for the session, a running one for a minute. The session reads them at once, again when
  a poll changed the robot's status, and of a running robot every 61 s while they are demanded; what they name beyond
  the list goes onto the robot of the session: its merge profile (when the robot has none) and that it will fail (while
  it runs). A check is named as Patronus names it in the profile ("IntelliJ Aggregator"), its build configuration
  ("Aggregator") only without that name. Of a running robot, the progress of its running checks follows
  ([Load on Patronus](#load-on-patronus)). When Patronus can't answer, the popover shows the list data.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`reads_the_details_a_window_shows_at_once_and_of_a_running_robot_every_minute_while_shown`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_details_of_a_demanded_robot_name_its_profile_and_go_with_its_demand`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_demand_of_a_session_is_the_union_of_its_windows_and_goes_with_each`)
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
  [@test] ../../src/presenters/next-step.test.ts (`row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button without toggling the row`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should close the branch preview for the Safe Push of its Patronus section`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)
- Every entry point asks one predicate first, the next step of the branch (the `safe-push` action, [Next
  Step](next-step.spec.md)), which asks the start refusal Rust decides first (`domain_policy::land::start_refusal`, the
  view's `landing.start` and `landing.dryRun`, worded by `src/presenters/reasons.ts`): a start
  is not possible while a robot runs on the commit the branch is at (Patronus would refuse it: "A Safe Push of the last
  commit is running"), nor while it runs on the remote branch that has the same commits by content, only rebuilt
  since on a newer baseline (`domain_policy::land::on_last_commit`, the remote branch up to date by content; a start would test and land
  them again, in the same words), nor for a blocked stacked branch ("Blocked by auth: fix it first, then sync", the push button's
  words). A Safe Push of a stacked branch also waits while a robot that pushes into the target (not a dry run) runs on
  an ancestor (`getLandingAncestor`, the `landingAncestor` of the repository view): the branch carries the ancestor's commits, two robots would land them ("A Safe
  Push of login is running: wait until it merges"), also on a side branch built on an ancestor, which lands that
  ancestor (side-fix built on bazel waits for the Safe Push of runtime, built on bazel too); and while a robot that
  pushes runs on a branch built on it (`getLandingSpan`, the `span` of the view) whose commit has commits of the branch, which lands
  them ("A Safe Push of oauth is running: it lands 1 commit of login with it", the reason before the ancestor's; what
  a robot lands, [below](#pushing-into-a-branch-whose-safe-push-runs)); a dry run waits for neither. `bd safe-push` asks
  the same rule (`repository_model::patronus_view`, its `start` refusal, `took_as_is` with the remote facts of an
  up-to-date remote status).
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_push_of_a_parent_restacked_below_a_running_safe_push_goes_on`)
  Then the button and the menu items are disabled, and their hover says the reason. The row button is disabled while
  the row runs an inline action or a start of the branch runs. After that the next step disables a Safe Push that
  Patronus would fail (the approvals that count missing, a draft, a pull request into another base, landed or merged),
  never a dry run; a start asked anyway (S, a toast, the review window) says the reason in an `info` toast ("Safe Push
  of auth waits for Ilya Korennoy", "Dry Run of auth can't start"), which opens what the reason names: the robot that
  runs on the branch or lands it ("Open in Patronus"), the pull request of a draft ("Open Pull Request",
  `LandPrompt.opens`); missing approvals open the popover, whose review line names them and where they count.
  [@test] ../../src/presenters/land.test.ts (`refuses with the words of the first blocker, offering the push or the object it names: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`, `the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`, `a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`, `the_tip_of_a_branch_the_sync_did_not_build_is_no_build_of_its_commits`, `a_finished_robot_on_the_last_commit_blocks_no_start_a_canceling_one_still_does`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor`)
  [@test] ../../src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_branch_waits_for_a_robot_that_lands_an_ancestor_not_for_a_dry_run`, `a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `the_robot_of_the_whole_stack_lands_every_branch_of_it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should wait for the Safe Push of an ancestor, which lands its commits`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)
- One decision then says what the start does, Rust's (`domain_policy::land::decide`, the rule `bd safe-push` decides
  by) over the facts the session holds: the table and the robots, the merge profiles, the remote statuses, the holds,
  the pull requests, the review as Patronus answered it for the branch as it is (a stale answer is not acted on), the
  rules of the repository, the landing needs. A start and its popover ask the session for the plan of their intent
  (`prepare` of a `Land`, `operations::land`: the decision with the profiles it offers and the fingerprint of what it
  was decided on; a read: no lock, no effect), the popover again with each patch of the view; `bd safe-push` plans its
  start with the same function over what it read. The view carries the decisions of the default starts of every branch
  and whole stack (`RepositoryView.lands`: a Safe Push with the default profile, a Dry Run), which the actions and the
  first check of a click read.
  The page presents the plan (`presentLand`) and decides nothing: the next step's reason first, then what Rust
  refuses, waits for or confirms.
  Each fact without an answer is a wait that names it ("Asking Patronus for the merge profiles…", "Checking auth…",
  "Checking the code-owner review…", "Checking whether the push keeps the approvals of #7…", "Checking what auth needs
  to land…"); what Patronus would refuse or fail is a refusal with the reason ("Patronus does not push into master",
  "#7 is a draft: Patronus merges it once it is ready for review", "Patronus checks the approvals when the robot starts
  and fails without them", also for a repository only on Space whose merge request misses them (D14), a hold of the
  push, a push that would drop commits of the remote branch the branch lacks: "The remote branch of auth has a commit
  that auth lacks: push it from its row, which shows it", D12). A start that can run either starts or confirms first,
  and confirms only for what a person decides (`Confirmation`): it lands more than its row (a stacked branch), it runs
  another profile than the default checks (the branch's own, the profile of its last robot, or one chosen), it cancels
  the Safe Push running on an older commit, Patronus can't judge the approvals before the start (`unknown`, GitHub
  doesn't list the pull requests) or couldn't answer the review (`unavailable`, `ReviewUnknown`), the push dismisses
  approvals Patronus doesn't count, the rules of the repository can't be read for a push into an approved pull
  request, a start without review. S, "Safe Push With Profile…" and the start without review always confirm. What the
  start does then is Rust's too (`PlannedEffect`, run in order by the engine): the retarget of a pull request
  into a virtual branch into the target (D11, the popover says it: "Retargets #8 of oauth from user/virtual/login to
  master first: every pull request of a virtual branch goes into master."), the cancel of the Safe Push it replaces,
  the push, then the robot, from the pull request or through the API with the profile.
- The page runs the plan it showed (`execute` of its intent on the plan's fingerprint, every confirmation of it
  accepted, `startPlanned` of `src/hooks/useSafePush.ts`): one operation of the session of the repository
  (`operations::engine::execute_land`, the run `bd safe-push` runs, `operations::land_execute`), admitted over the
  operations that run there (a second start of the same subject joins it, one that lands a part of it waits for it),
  the plan prepared again from the facts now; a fingerprint or a confirmation the plan no longer has refuses it as
  `plan_changed` before any effect, in the words of the start ("What the Safe Push of auth was decided on changed
  before it began (…); nothing was done: start it again"). A start that waited in the queue (S right after P, while
  the push of its branch sends) runs on the plan the wait left, while the user accepted every confirmation of it: one
  push, then the robot. The operation is in the view from its admission on (`RepositoryView.operations`),
  so every window shows its step in the slot ("Checking…", "Waiting…" for another holder of the repository lock with
  Stop Waiting beside it, "Pushing 45%" with the progress its channel tells the window that asked, "Starting…") and the
  node of the row leaves the user's moves at once (`RunningMove::Starting`). Before the effects the engine checks what
  lands against the newest target and, of a whole stack, its landing gate (`operations::land_session`), takes the
  repository lock only to read the witness and freeze the commits and the leases of the push, then runs the effects
  in order: each with its status in the outcome, the first that fails stops the rest, and a rewrite after the push
  (the virtual branch moved off the pushed commit), a robot of what it lands started meanwhile or a closed pull request
  of its comment skip the start as `plan_changed`: the toast says what changed and what was done ("the Safe Push it
  replaced was canceled and the push stays"), with "Start Safe Push" (a new plan, without the push and the cancel it
  made) and Open in Patronus of the robot it canceled.
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan`, `the_first_effect_that_fails_stops_the_start_and_the_ones_before_it_stay_done`, `a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`, `a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`, `a_robot_of_what_it_lands_started_meanwhile_skips_the_start`, `a_start_that_waited_runs_on_the_plan_the_wait_left_while_the_user_accepted_what_it_asks`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push once and start the Safe Push pressed while the push of its branch sends`)
  [@test] ../../src/presenters/outcome.test.ts (`says the robot it started with the profile of its plan, and the checks that ran on the last fetched target`, `offers the start again when the facts changed after the push, naming the robot it canceled; a failed effect says its error`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_its_plan_refuses_is_refused_before_patronus_or_github_is_asked`)
  [@test] ../../src/presenters/land.test.ts (`starts at once when Rust decided there is nothing to say, and waits for its decision before it`, `opens the popover for a key and for each confirmation of the decision: a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`, `refuses with the reason of the next step first, the reason every surface says`, `waits for the facts Rust waits for, each saying what it waits for, and refuses with why a read failed`, `refuses with the words of the first blocker, offering the push or the object it names: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`, `lets a start without review wait for its profile, and confirm the one that skips the review`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`, `every_fact_without_an_answer_waits_or_confirms_and_never_refuses`, `the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`, `the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`, `a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`, `missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`, `the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)
  [@test] ../../crates/operations/src/land_test.rs (`a_start_is_planned_over_what_the_session_holds_and_reads_the_review_its_first_decision_waits_for`, `the_fingerprint_of_a_plan_changes_with_its_intent_and_the_tips_it_was_decided_on`, `the_plan_of_a_session_waits_for_its_patronus_view`)
- A click waits for the facts its plan needs, its button busy with "Checking…": the session answers each of them,
  a failure too (the review it asks at once, `unavailable` while Patronus can't be asked), so the wait ends; then it
  starts at once, says the refusal in an `info` toast ("Push Branch" its action where the push alone is the step), or opens
  the popover. The second click of
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
  running Safe Push it cancels ("Cancels the Safe Push of auth running on 1a2b3c4."); a push that dismisses approvals,
  or rules that can't be read;
  the way to land a stacked branch alone. The footer says what it waits for or why it can't start, then "Dry Run" (a
  Safe Push only: the start of a dry run instead), Cancel and the start ("Safe Push", "Dry Run", "Safe Push Without
  Review"), which takes the focus once it can run, so Enter starts. A start closes the popover: the slot shows it.
  Escape and a click outside close it. The stack graph highlights the path it lands while it is open.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should give the focus to the start of the popover once it can start`)
  [@test] ../../src/lib/patronus-stack.test.ts (`lands a stacked branch with what it is built on, in one robot, counted`, `names three ancestors and counts the rest, the size of what lands first`, `says that a dry run checks the commits of the ancestors too, and nothing of the children`, `says that the branches built on it stay, and what the next sync after the merge does to them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`, `should say in the Safe Push popover of a branch in the middle what lands and what stays`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
- The profile:
 "Default checks" and the merge profiles of the target branch that allow the mode (the emergency ones
  apart, `emergencyProfiles`), by default the profile of the last robot of the branch (never an emergency one), named
  so: "The profile of the last Safe Push of auth."; its description from Patronus below. Nothing is remembered by
  Branch Deck: the profile is the branch's by its robots. The profiles are `GET /robot-start-supported`, cached for 10
  minutes (an error for 1), a fact of the session for the target of its baseline (`facts.profiles`), which the popover
  when it opens and a click before it decides ask (`refresh_facts { profiles }`): read when unknown, after a failure, a
  minute after the last read, or for another baseline. The default of the last robot is Rust's
  (`domain_policy::land::default_profile`, the profiles a start offers `offered_profiles`, which `prepare` answers
  with the plan, its `alternatives`): the profile an answer named on the robot (a poll, a start, the New API's list), else the one its
  details name, read once for it. A target branch Patronus does not protect can't be started. A profile carries `skipsReview`: true for an emergency profile and
  for the profile of the last robot whose options say `patronus.review.approval.check.skip=true` (false when they say
  otherwise), null for the rest, which Patronus doesn't describe; a robot's profile carries it from its options
  (`robot_profile`), null while Patronus hasn't written them. A profile known to skip the review lets the start run
  without the approvals: the review line says "Starts without the approvals of IJPL Code Platform: Large Scale Change
  skips the code-owner review.". With the New Patronus API (the setting above) a list names the profile of a robot
  where Patronus put it (`mergeProfile`: id, name, `skipsReview`; behind its `robot.list.merge.profile.enabled`
  flag), so the default profile and whether it skips the review come with the list, and the last robot is not read
  for them (`domain_policy::land::default_profile` takes the profile the list named); a null `mergeProfile` leaves the profile unknown, as a list without it, never "none".
  [@test] ../../crates/patronus/src/service_test.rs (`with_the_new_api_a_list_names_the_merge_profile_patronus_put_on_a_robot`)
  [@test] ../../crates/patronus/src/actions_test.rs (`start_options_take_the_profile_the_list_named_without_reading_the_last_robot`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_profiles_are_asked_when_a_start_needs_them_and_a_minute_later`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…`, `should open the popover of a click for the profile of the last Safe Push, and start with the one chosen`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)
  [@test] ../../crates/patronus/src/actions_test.rs (`start_options_keep_the_emergency_profiles_apart_and_default_to_the_profile_of_the_last_robot`)
  [@test] ../../crates/patronus/src/service_test.rs (`the_options_of_a_robot_say_whether_its_profile_skips_the_review`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`, `a_start_offers_the_profiles_of_its_mode_and_without_approvals_the_emergency_ones_and_a_skipping_default`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`)
- The target branch is the branch of `origin` the baseline tracks (`origin/master` → `master`); a local baseline or one
  on another remote can't be started.
  [@test] ../../crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_the_baseline_tracks`)
- A branch whose commits the remote lacks is pushed first, decided by content as the push button decides it
  (the `Push` effect of the decision, over the action Rust decides of the remote status, `RemoteStatusUpdate::push_action`, never by the hash of the tip). The push is part of the start: no note narrates
  it; while it runs the slot button shows it as the compact push of the row does, the spinner and "45%" ("Pushing 45%"
  its name, "Pushing 2 branches 45%" for the one push of a stack in its hover and in the preview), at the width it had
  at rest, the
  one indicator of it: the push button of the row is disabled without its spinner and percent; then "Starting…". A
  branch whose remote branch has the same commits, only rebuilt since on a newer baseline (every sync onto a newer
  baseline does it), is not pushed: Patronus rebases the remote branch itself, and the target conflict check below
  proves that the rebase applies; a push would replace it with copies of the same commits and move the head of its
  pull request for nothing. One reworded since is pushed: Patronus lands the messages of the remote branch. While the
  remote status of the branch is not loaded, the start waits: "Checking auth…". A dry run pushes the branch too: only
  the push into the target is left out.
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`, `every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a branch whose remote branch has its commits under other hashes without a push`, `should wait to start until the remote status of the branch is loaded`)
- A start belongs to the repository it was asked in: the click plans it in the session of that repository and runs it
  there (`execute` names the repository; Rust resolves its branch prefix), so a switch of the repository during the
  start starts nothing in the other one, and its robot lands in the session of its own repository, which shows it on
  return. The step of a start shows only in its repository (the operations of its session): the branch of the same
  name in the other one shows no step and starts on its own. A plan read before the switch starts nothing: "Cannot
  start the Safe Push" says "idea-1 is no longer the open repository".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push in its repository when another repository opens during its checks`)
- A push before the start of a branch with an open pull request into the target that someone approved, in a
  repository whose GitHub rules dismiss stale reviews on push (a `pull_request` rule with `dismiss_stale_reviews_on_push`
  of `GET repos/<owner>/<name>/rules/branches/<target>`, readable with read access; `remote_facts::branch_rules`), is
  told before the start, as a warning of the popover: "The push dismisses the approvals of #7: the rules of the
  repository dismiss stale reviews when new commits are pushed." The rules are a fact of the session (`facts.rules`,
  `peekBranchRules`: `keep`, `dismiss`, or `unknown` with the reason), read once per session for each base of the open
  pull requests someone approved (an unknown base is the target), after the listing that has the approval; the start
  reads them only while it would push into an approved pull request. Until the rules answer, the start waits ("Checking
  whether the push keeps the approvals of #7…"): a start that pushed meanwhile would take the approvals away. Rules
  that can't be read (no token, no access, GitHub failed) are never "keeps the approvals": the start confirms first,
  a warning of the popover ("Cannot check whether the push keeps the approvals of #7: GitHub answered 500. Patronus
  counts them when the robot starts."), which Enter or the start accepts. A failed read is not kept: each start asks
  the session to read again the rules GitHub didn't give (`refresh_facts { branchRules, afterWrite }`), and the click
  waits for that answer. When the approvals the push dismisses are the ones Patronus counts (the readiness is
  `approved`), the robot would start without them: the start is refused with "The push dismisses the approvals of #7: push first, then
  ask for the review again", and "Push Branch" alone is offered (the action of the `info` toast, the button of the popover).
  The toast of a start that can't run is of the robots of its subject: a later start takes its place, and its Push
  closes it when pressed (a step of the start, which the row shows, and no Push is left behind that pushes nothing).
  `bd safe-push` reads the same rules with the same function before the same start, refuses
  `push_dismisses_approvals` where the app refuses, says the dismissed approvals Patronus doesn't count in `rules`, and
  refuses rules GitHub doesn't answer with `rules_unknown` unless `--allow-unknown-rules`
  ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`, `rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should offer only the push when it dismisses the approvals Patronus counts`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should wait for the rules before a push into an approved pull request: Enter must not dismiss its approvals`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should confirm a push into an approved pull request whose rules can't be read, never reading them as keeping the approvals`, `should ask again for the rules a start couldn't read, and start with the key once GitHub gives them`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should refuse the click with the push alone in its toast`, `should say in the popover why it can't start, and push alone from it`)
  [@test] ../../crates/remote-facts/src/branch_rules_test.rs (`a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch`, `a_branch_without_such_a_rule_keeps_the_approvals`, `a_failed_answer_is_an_error_not_a_guess`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`the_rules_of_an_approved_base_are_read_once_and_a_failed_read_again_when_a_start_asks`)
  [@test] ../../crates/repository-model/src/pull_request_facts_test.rs (`the_rules_are_of_the_bases_of_the_approved_open_pull_requests`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should warn before a push that dismisses the approvals of the pull request`)
- git merges a branch that uses a name only another branch adds, and the robot's build fails (the robots of
  2026-10-17 and 2026-10-21). So no start runs past the landing gate, the rule of `bd safe-push` in Rust
  (`trial_check::landing_gate`): a trial check of what lands, with the proven dangling references and, with
  `branchdeck.symbols on`, the missing declarations, the uses by a commit of what lands of a name that a commit outside
  it adds, which no merged Safe Push landed (the commits the repository view names landed, on the robots of the
  session, never the page's). The gate of a branch is its landing needs, which the session computes after its sync
  (below); the gate of a whole stack the engine checks before the effects of its start (`operations::land_session`,
  through the object cache of the repository). A refusal starts nothing and pushes nothing (`missing_declaration`,
  `dangling_reference`, with every refusal and its fix, `ErrorDetails::Landing`): the error toast "Cannot start the
  Safe Push of the whole lane-run stack" says "“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3),
  which “Name lossy stills” of stills-codec adds: git merges it, but the build of the whole lane-run stack fails. Stack
  lane-run on stills-codec to land it too." (one more refusal: "One more name breaks it too: bd check lists them."),
  with the fix as its action, "Stack lane-run on stills-codec", a restack of the root of what lands, so the next start
  lands that branch too, and "Start Anyway" below it, the same start with the override of what refused it (a name the
  user knows is a false alarm), planned again. A commit unassigned, stacked branches off or a restack that would close
  a cycle: the text names the move of the commit, and "Start Anyway" is the only action. A gate of a whole stack that
  can't run holds nothing back, and the start's toast says "Not checked whether the whole lane-run stack uses a name
  only another branch adds: …".
  [@test] ../../tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_whose_build_needs_a_branch_outside_it_is_refused_with_the_fix`)
  [@test] ../../src/presenters/outcome.test.ts (`offers the fix and Start Anyway of the landing gate, Update Project of a conflict with the target, the start again of a changed plan`)
- The row knows it before the press: after a sync Rust computes the refusals of every branch's start by the same rule
  (the landing needs, [Next Step](next-step.spec.md#what-the-safe-push-needs)), and the slot says "Needs ABC-123" with
  the restack. The popover of the start (S, a click, Dry Run) then opens first (a confirm reason, `landing`) and shows the
  reason before the start, in the words of the toast, with the fix as its button ("Stack JIRA-456 on ABC-123", which
  restacks and leaves the popover open to read the start again); its start is "Start Anyway", the start with the
  overrides of what refused it, planned again with them (`allow` of its `Land`). Start Anyway is offered only there
  and in the refusal's toast, never by the row. A dangling reference has the same fix: the restack onto the
  branch of the commit that changes the use, or its move. While the branch that adds the name is landing in a running
  Safe Push (its own, of a branch built on it, of its whole stack: `landingIn`), the reason says so: "ABC-123 is landing
  in its Safe Push, which tests it without this start: a start beside it fails too." Its robot tests ABC-123 without
  JIRA-456, so the start fails all the same; the fix stays the restack, or a start after it merged.
  [@test] ../../tests/e2e/next-step/landing-needs.spec.ts (`should say in the Safe Push popover why the build fails before the start, and stack the branch from it`, `should start anyway from the Safe Push popover with the override of what refuses it`)
  [@test] ../../src/presenters/land.test.ts (`opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)
  [@test] ../../src/presenters/landing.test.ts (`says that the branch the start needs is landing in a running Safe Push, which tests it without this start`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] ../../crates/trial-check/src/landing_gate_test.rs (`a_running_safe_push_of_the_branch_that_adds_the_name_is_named_in_the_reason`)
  [@test] ../../crates/domain-policy/src/landing_needs_test.rs (`a_missing_declaration_names_the_safe_push_that_lands_the_branch_adding_the_name`)
  [@test] ../../src/presenters/landing.test.ts (`refuses the robot of 2026-10-21 with the name, both commits and the restack that lands the branch that adds it`, `counts the other refusals, names an unassigned commit by its hash, and offers no action for a move`, `starts when nothing refuses it`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)
- Patronus rebases what a robot lands onto the newest target before any check, and a conflict fails the robot at once
  ("Rebase … onto master is impossible"). A branch is built on the base of the checked out branch, not on the newest
  target, and the target moves every minute. So the engine checks the rebase before the effects of a start, on its
  own thread, outside the repository lock (`operations::land_session`, the check of `bd safe-push`,
  `sync_core::target_conflict`). It fetches only the target's remote branch, as Update Project does, then replays what
  lands onto its tip as a plain `git rebase` does, with no resolution by words, because Patronus can't resolve by words.
  Meanwhile the slot button says "Checking…". A conflict starts nothing and pushes nothing (`target_conflict`, the
  commit and its files in `ErrorDetails::Conflict`): the error toast "Cannot start the Safe Push of auth" says "Nothing
  was pushed or started: auth conflicts with the newest target in a.txt: Patronus would fail it at once", with "Update
  Project" as its action, of the repository of the start (a fetch only, once the main window shows another one,
  `runUpdateProjectOf`). A failed fetch doesn't stop the check: it replays onto the last fetched tip, and the start's
  toast says so after its description ("The fetch of origin/master failed, so the check used the last fetched
  origin/master: …", `StartNote`). A check that can't run holds nothing back, and the toast says "Not checked whether
  Patronus can rebase auth onto origin/master: …".
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should refuse a Safe Push that conflicts with the newest target, nothing pushed or started, with Update Project`)
  [@test] ../../crates/sync-core/src/target_conflict_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_conflicts`, `a_target_that_moved_without_a_conflict_is_clean`, `a_failed_fetch_checks_the_last_fetched_target_and_says_so`)
- The push is an effect of the start: it has no toast of its own (Activity records it, with the operation), the
  start's toast is the one toast of the action. A failed push starts nothing. It is the push core of every push
  (`operations::push::send`): the commits and the leases frozen under the lock, the network push after it, once more
  over a remote that moved without commits the branch lacks; it never drops someone's remote commits for the user (a
  branch, or an ancestor it pushes, whose remote branch has commits the branch lacks refuses the start before it: "The
  remote branch of auth has 2 commits that auth lacks: push it from its row, which shows them"). A Safe Push of the
  branch still running on an older commit is canceled before that push, not restarted
  ([below](#pushing-into-a-branch-whose-safe-push-runs)). Then `POST /robots` starts the robot on the pushed virtual
  branch, as the web UI does (`ROBOT_WEB_UI`, the source branch `refs/heads/{prefix}/virtual/{name}`, kept: delete
  policy `NEVER`), so a restart takes the newest commit of the branch; the request names the profile chosen
  (`mergeProfileId`, null for the default checks), the mode (`REBASE_AUTOSQUASH`, `DRY_RUN`) and the Space merge request
  whose approvals count (`spaceReview`). The robot goes into the session at once and is polled.
- `execute` decides the start again in Rust before any effect, by the rules of `bd safe-push`, over the facts of the
  session, never the page's: what its plan refuses refuses it, in the words of `bd` (`start_refusal_text`): a parked
  branch, one built on parked and a whole stack with a parked member ("Nothing was pushed or started: login is parked:
  IJPL-1"), a branch or a whole stack merged Safe Pushes landed whole, a branch the sync did not build and a stack
  branch that is not built or conflicts as one branch, then the robot part of the repository view ("A Safe Push of the
  last commit is running", "A Safe Push of login is running: wait until it merges"; a Dry Run waits only for a robot on
  the last commit). A start that raced with another one (the IDE, `bd`) or came by IPC is refused the same way, and a
  refused one comments on no pull request either.
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_its_plan_refuses_is_refused_before_patronus_or_github_is_asked`)
  [@test] ../../crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`)
  [@test] ../../crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)
  [@test] ../../crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`, `should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`, `should name the approvals a Safe Push needs on Space, and start with the merge request found open`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_restart_and_cancel_run_the_real_service_against_the_fake`)
- A started robot is a `success` toast "Safe Push started" ("Dry Run started") that names the branch, the target and
  the profile, never the ref the robot runs on ("auth → master · profile Large Scale Change", `describeRobot`), with
  "Open in Patronus" (the robot page) and below the text "Cancel Safe Push": a start asked by mistake costs only the
  robot (the push and the comment stay), for 10 s; Activity records it in the same words, with the operation (its log
  lines). The toast and the entry are the outcome of `execute` in the page's words (`presentLandOutcome` of
  `src/presenters/outcome.ts`, the robot of `LandResult`). A failed start is an error toast "Cannot start the Safe Push
  of auth" with what Patronus said. It is the toast of the robot
  ([One toast per event](../../docs/contributing/ux-writing.md#one-toast-per-event), `showRobotEventToast`): its Cancel
  only while the robot can be canceled, and its later states take its place (its failure, its merge, its cancel, a
  restart); it takes the place of the toast of the robot it restarts and of the approvals it starts on
  ([Review Requests](../repository/review-requests.spec.md#approval-news)).
  [@test] ../../src/lib/patronus.test.ts (`describes a robot by its branch and target, with the profile it was started with`)
  [@test] ../../src/presenters/outcome.test.ts (`says the robot it started with the profile of its plan, and the checks that ran on the last fetched target`)
  [@test] ../../src/stores/patronus.test.ts (`takes the place of the toast of the robot's start: one toast of the robot`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should turn the toast of the approval into the start of the branch, in place, when the row starts it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)
- A start is never repeated by itself, not even right after the push: Patronus fetches the pushed branch while it
  handles the start.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_failed_start_is_never_repeated`)
- A stacked branch carries the commits of the branches it is built on ([Stacked Branches](stacked-branches.spec.md)): a
  child can't land without them, so its Safe Push lands it with every branch it is built on that isn't in the target
  yet, in one robot ([Landing](../../docs/design/landing.md)); its start always confirms, the popover saying so first.
  Before the start the branches its decision pushes are pushed, parents first, a part of the start, without a toast of
  their own: the branches of the stack that have work for the remote, up to the nearest one a running Safe Push holds
  (its robot lands it as it took it: a push into it would fail it at the end) and past one that landed (a merged Safe
  Push landed it whole, or the fetched baseline has every commit of it), as Push Stack of its row covers them; one
  whose push drops remote commits refuses the start (the `push` effect of `domain_policy::land::decide`, one rule,
  `domain_policy::push::start_push`, which `bd safe-push` pushes too, and no other branch). Every surface asks the start of the branch
  itself, its popover at its own row: its slot, "⋯", S, the preview footer and the review window. The header of a stack
  island asks the start of the whole stack, one robot of its stack branch, its popover at the header ([Stack
  Island](stack-island.spec.md#header), [Whole Stack](whole-stack.spec.md#the-safe-push-of-the-whole-stack-in-the-app));
  of a straight chain, which has none, the start of its last branch. The popover of the root of a stack and of any
  branch of a stack that forks offers the whole stack as its second way, below what it lands: "Land the whole stack:"
  with "Safe Push Whole Stack" ("Check the whole stack:" with "Dry Run Whole Stack" for a Dry Run), the item of the
  menus (`getWholeStackStartItems`), its hover what it lands, disabled with the reason under it; it opens the popover
  of the whole stack.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`, `should land a stacked branch with what it is built on from S and the review window`, `should land a stacked branch with what it is built on from the footer of its preview and S in it, the popover at its row`, `should say in the Safe Push popover of a branch in the middle what lands and what stays`, `should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`a_start_pushes_the_ancestors_with_work_up_to_the_nearest_held_one_past_what_landed`, `a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_start_pushes_what_its_decision_pushes_and_leaves_an_ancestor_a_running_safe_push_holds`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`, `should check the whole stack in a Dry Run from the island's ⋯, its popover at the header`)
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
  [@test] ../../crates/remote-facts/src/patronus_command_test.rs (`a_merge_keeps_the_source_branch_and_a_dry_run_is_plain`, `a_merge_of_a_reviewed_pull_request_deletes_its_branch`)
  [@test] ../../crates/remote-facts/src/patronus_command_test.rs (`posts_the_command_as_a_regular_comment_of_the_pull_request`)
  [@test] ../../crates/domain-policy/src/pull_request_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)
- Patronus merges a pull request into its base: a pull request into another base than the target is never commented
  on (`domain_policy::pull_request::start_pull_request`), and such a branch starts through the API. Every pull request
  of a virtual branch goes into the target ([Pull Requests](pull-requests.spec.md)); an old one of a stacked branch
  into its parent's branch (`{prefix}/virtual/…`) is retargeted, by a push of the branch too ([Stacked
  Branches](stacked-branches.spec.md#pull-requests)), and by the start itself first (D11): the start comments on it
  once GitHub took it, and its approvals count, as the readiness is asked with it. The slot offers "Retarget #8 to
  master" while one is open (`retargetToTarget`, [Next Step](next-step.spec.md#the-table)).
  [@test] ../../crates/domain-policy/src/pull_request_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_discrepancy_cases_name_every_id_as_the_register_does`, `the_next_step_row_cases_answer_alike_in_the_app_and_bd`)
- The popover says it: "Comments /patronus merge on #7: Patronus reports there and closes it once merged." A comment
  can't choose a merge profile: the popover offers none, and "Safe Push With Profile…" is disabled ("A comment on #7
  runs the default checks: it can't choose a profile"); a branch whose own profile is another one says "A comment runs
  the default checks: it can't choose Large Scale Change.". Patronus doesn't merge a draft: "Safe Push" of a branch with
  a draft pull request is disabled, "#7 is a draft: Patronus merges it once it is ready for review", its slot the quiet
  "Draft" that opens it ([Next Step](next-step.spec.md)). A dry run of a draft can.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)
- The bot answers the comment with a reaction: 👍 started, 👎 not, with a comment starting `⚠️` that says why. The
  engine of the start reads it (`operations::robot_start`, the same for `bd safe-push`): right after the comment, then
  every 3 s on the clock of the session (`domain_policy::refresh`, never a timer of the page) for up to a minute counted
  from the comment (the checks of the start before it take none of it), the reactions of its comment (`GET
  repos/<owner>/<name>/issues/comments/<id>`, conditional: a poll that sees no change costs no rate limit); after 👎 the
  first later `⚠️` comment of a bot is the reason, looked for once more if it isn't there yet; after 👍 an incremental
  list (`since` shortly before the comment) finds the robot of the pull request (`githubPrUrl`), which goes into the
  session and is polled: of that pull request, a Dry Run for a Dry Run and a Safe Push for a Safe Push, none the
  session listed finished before the comment nor the one the start replaced, the newest (`comment_robot`, as `bd
  safe-push` matches it), so a Dry Run of #7 canceled a minute before is never the new robot. A refusal is the error
  toast "Cannot start the Safe Push of auth" with the bot's reason; no answer within a minute is a start whose answer
  was lost (below): the warning toast "Cannot tell whether the Safe Push of auth started", "Patronus did not answer the
  comment on #7 within a minute. It holds auth until Patronus lists its robot, or until Forget This Start on the row",
  with Open Pull Request; a read of the reply after it reached GitHub that fails ends the wait the same way, never as a
  failure. A 👍 is the start: when Patronus
  lists no robot of it within the minute, the start of the app goes on looking for it every 15 s for 5 minutes more,
  and holds its subject meanwhile: the slot says "Starting…" in every window, a second click or key starts nothing, and
  a repeat of the same start joins it instead of posting a second comment, which Patronus would answer with a second
  robot or 👎. The hold is the session's: a switch to another repository shows nothing on its branch of the same name,
  and back in the repository the slot still says "Starting…". Once listed, the toast is "Safe Push started" of the
  robot; none within the 5 minutes, it is "Safe Push started", "Patronus accepted the comment on #7: the robot shows
  once Patronus lists it", with "Open Pull Request #7", and the robot comes with a later list of the session.
  [@test] ../../crates/remote-facts/src/patronus_command_test.rs (`a_comment_without_reactions_is_pending_and_a_thumbs_up_is_accepted`)
  [@test] ../../crates/remote-facts/src/patronus_command_test.rs (`a_thumbs_down_reads_the_reason_from_the_bot_comment`)
  [@test] ../../crates/remote-facts/src/patronus_command_test.rs (`the_reason_is_a_later_bot_comment_and_a_user_warning_is_not_one`)
  [@test] ../../crates/operations/src/robot_start_test.rs (`a_comment_the_bot_accepts_starts_the_newest_robot_of_its_pull_request_nobody_knew`, `a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute`, `an_accepted_comment_looks_for_its_robot_after_the_minute_until_patronus_lists_it`)
  [@test] ../../src/presenters/outcome.test.ts (`says a comment Patronus accepted whose robot it lists not yet, and a comment it never answered as a start not known yet that opens it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should hold a start Patronus accepted until it lists the robot: a repeat posts no second comment`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should keep holding a start Patronus accepted through a switch to another repository and back`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say why Patronus refused the comment`)
- A branch without an open pull request starts through the API, as on Space, without a merge request; that start counts
  no approval, so with approvals required its next step is "Create Pull Request…" ([Next Step](next-step.spec.md)).
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`)
- A robot of a pull request (`type: GITHUB_PR`, `githubPrUrl`) knows it: its actions have "Open Pull Request #7". A
  restart keeps the pull request (Patronus restarts with it).
  [@test] ../../crates/patronus/src/service_test.rs (`a_robot_started_by_a_pull_request_comment_links_the_pull_request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

### Merged

- A robot known as running or canceling that an answer of the session (a poll, a list, the robot of a cancel) shows
  successful and not a dry run merged its branch (`domain_policy::news::robot_finished`): news of the session (`News`,
  `robotMerged`, with the pull requests its landing closes, [below](#merged)), told once the Patronus view of the robots
  it merged in stands (at once while the session has no table to derive it of), which the main window tells once
  (`tellRobotNews`; none of those its first view of the session had). A robot first seen finished is no news.
  [@test] ../../crates/domain-policy/src/news_test.rs (`a_robot_known_running_that_finished_merged_or_failed`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why`, `a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes`)
  [@test] ../../src/stores/patronus.test.ts (`does not tell a successful dry run or a robot first seen merged`)
- With the window focused, a `success` toast says "auth merged into master" (the branch, or the subject of a Safe Push
  of a temporary ref) with the push mode ("Safe Push"), for 10 s; an issue key of the title ("IJPL-1 merged into
  master") links to the tracker, as in the branch name ([Toasts](../../docs/contributing/ux-writing.md#toasts)). Its
  actions are below the text (the `robot` body of the toast): "Update Project", the primary one, which picks the merge
  up in one click (it fetches the baseline first, then updates onto it; the click is the consent, and a conflict, local
  changes or a lock say so as Update Project does), then "Open in Patronus", which opens the robot page and leaves the
  toast for the update. A merge in a repository the main window doesn't show offers Fetch instead (Update Project
  updates the shown one), or only "Open in Patronus" once its baseline was fetched since the merge (`lastFetchTime`:
  Branch Deck's fetch, or one elsewhere by `FETCH_HEAD`). Its "Update Project" acts on the repository of the merge
  (`runUpdateProjectOf`): clicked after a switch of the repository, it only fetches that repository, never updates the
  one shown since; the toast says Fetch from the switch on, and Update Project again on the way back.
  [@test] ../../src/stores/update-project.test.ts (`updates the repository a toast is about only while it is shown, else only fetches it: never the one shown since`)
  [@test] ../../src/lib/patronus.test.ts (`names the branch, else the subject of a robot of a temporary ref`)
  [@test] ../../src/stores/patronus.test.ts (`tells once in a toast with Update Project and the robot page when a running robot of a branch merged`, `offers Update Project also after a fetch since the merge: the update is what picks it up`)
  [@test] ../../src/lib/landing-toast.test.ts (`offers Fetch once the main window shows another repository, and Update Project again on the way back`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push merged, and update the project on request`)
- The toast of the merge is the toast of the landing, until the update lands it
  ([One toast per event](../../docs/contributing/ux-writing.md#one-toast-per-event), `showMergeToast` of
  `src/lib/landing-toast.ts`): it takes the place of the toast of the robot (its start), and while an update or a
  fetch of its repository runs, started from the toast, the header or a key, its Update Project is "Updating…"
  ("Fetching…"), disabled, and the toast stays. The outcome of that update takes its place, in place: what landed
  ("auth landed in origin/master, squashed" / "Updated master: rebased 104 commits", the toast of the update of
  [Landed Commits](../repository/landed-commits.spec.md#after-the-update), several merges in one), or "master is
  already up to date with origin/master"; a run that ended otherwise (a conflict, whose dialog says it, an error toast,
  the fetch of a repository not shown) closes it. A landing of its branches that a sync found after an update
  elsewhere (`bd update`, the IDE) says itself in place: "auth landed in origin/master" / "Found by a sync", the words
  of its Activity entry; one that came before the toast showed (it waits for the focus) is said at once. Activity
  keeps both entries, the merge and the landing.
  [@test] ../../src/lib/landing-toast.test.ts (`is one toast from the merge to the landing: Update Project, Updating… while the update runs, then what the update landed in place`, `says in place what a sync found landed after an update elsewhere (bd update, the IDE)`, `closes after an update that ended without an outcome of its own: the dialog of the conflict says it`, `shows the landing at once when an update elsewhere landed it before the toast showed: the toast waited for the focus`)
  [@test] ../../src/stores/patronus.test.ts (`takes the place of the toast of the robot's start: one toast of the robot`)
  [@test] ../../src/stores/update-project.test.ts (`takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date`)
  [@test] ../../tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed`, `should turn the toast of a merge into what the update landed, in place, when the update starts from the header`, `should say in the toast of a merge what a sync found landed after an update outside the app`)
- A watched robot of no branch (the Safe Push of a reviewed pull request, `watchRobot`) is polled with the robots of
  the branches, and its merge is named by what it tested: "#101 Rework the session store merged into master".
  [@test] ../../src/stores/patronus.test.ts (`polls a watched robot of no branch and names its merge by what it tests`)
- A robot of a stacked branch that merged landed the branches it is built on too, the robot of the whole stack every
  branch of the stack ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app), "Whole air stack merged into
  master"): the open pull requests of those branches are closed with a comment, as `gh pr close --comment` does
  ("Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch."; one already closed
  gets no second comment, a merged one can't be closed), and the toast and Activity say it ("Safe Push · closes the
  pull request of auth: it landed with oauth", "closes the pull requests of 6 branches: they landed with the whole
  stack"). One rule decides which, in Rust (`domain_policy::landed::pull_requests_to_close`, D7), for the app and
  `bd robot` (`closes`, which `bd` lists and never closes): by content, the branches the robot landed whole besides its
  own, as the Patronus view of the robots it merged in names them (`landedByRobot`: while the robot ran on the branch,
  the stack branch, as it is, its tip or the same commits rebuilt on a newer baseline, or a commit of it the branch built
  new ones on since; after a change below its commit by the commits it tested), so a branch with commits added after the
  landed ones keeps its pull request; and only a pull request that holds nothing its branch lacks: its head is the
  branch's tip, or it is the remote branch and that has no commit the branch lacks (older versions of commits amended
  here included: their newer versions landed). The pull request the robot started from is Patronus'. The session tells
  the merge once that view stands, with the pull requests it closes (`RobotMerged.closes`), so the toast names them at
  once. The page asks the session of the robot's repository to close them (`CloseLandedPullRequests`, an operation of
  that session, `crates/operations/src/close.rs`): Rust plans them again from the merge in its facts, the comment of
  each its own, each close holding the state of its pull request, and the session has each closed at once; a failed
  close is an error toast "Cannot close pull request #5", and the others go on. A robot first seen merged closes
  nothing. The closes are of the robot's repository, also when the main window shows another one by then.
  [@test] ../../src/stores/patronus.test.ts (`closes the pull requests in the robot's repository when the main window shows another one`)
  [@test] ../../src/stores/patronus.test.ts (`closes in the session of the robot's repository the pull requests the news of the merge says its landing closes, and says so`, `closes the pull requests of the branches the whole stack landed`)
  [@test] ../../src/presenters/news.test.ts (`says which pull requests the merge closes: of the branches, or how many the whole stack landed`)
  [@test] ../../crates/operations/src/close_test.rs (`the_closes_of_a_merged_robot_close_what_its_landing_closes_with_a_comment_of_the_robot`, `a_close_closes_an_open_pull_request_with_its_comment_and_nothing_else`)
  [@test] ../../crates/domain-policy/src/landed_test.rs (`closes_the_pull_requests_of_the_branches_a_merged_robot_landed_whole_not_of_its_own_branch`, `leaves_a_pull_request_with_a_commit_the_branch_lacks_one_of_a_branch_with_work_left_and_one_the_table_lacks`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes`)
  [@test] ../../crates/bd-remote/src/robot_test.rs (`a_merged_robot_lists_the_pull_requests_its_landing_closes_and_closes_none`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should close by content the pull requests of the branches a Safe Push landed after a change below its commit, not of one with work left`)
  [@test] ../../crates/remote-facts/src/pull_request_writes_test.rs (`closes_a_pull_request_with_a_comment`, `a_closed_pull_request_gets_no_second_comment_and_a_merged_one_is_not_closed`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)
- Until the update archives them, the branches a merged robot landed that way (the same view, a robot first seen merged
  too: `getLandedBy`) say so in their rows, "Landed with oauth", "Landed with the whole air stack"
  (`LandedWithChip`, `info`, beside the status "landed" and the hollow `info` node of a branch in master), its hover "In master with the
  Safe Push of oauth · the update removes the local copies" and when it merged, a click the robot in Patronus; the
  preview shows it too. The branch the robot ran on landed too: it keeps its robot chip, "Merged". They offer no Safe
  Push and no Dry Run ([Next Step](next-step.spec.md#the-table), row 1: "Landed in master with oauth", "Merged by
  Patronus") and no push ([below](#pushing-into-a-branch-whose-safe-push-runs)); the "to push" counts leave them out.
  Once the fetched baseline has their commits, the Landed chip takes its place.
  [@test] ../../src/lib/patronus.test.ts (`names who landed a branch in its row: a branch built on it, the whole stack, its own robot with the work left`)
  [@test] ../../src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`, `new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`, `a_dry_run_lands_nothing`)
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_missing_source_lands_nothing`)
  [@test] ../../src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`, `should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`, `should offer no Safe Push, Dry Run or push of the branches the whole stack landed, each saying why, the hover of their chip saying so`)
- A change below the commit the robot took (a new commit or an amend of a branch it is built on, a reword; for the
  whole stack any change of a branch of it) gives the branches new copies, so the hashes no longer tell what landed:
  the repository view asks by content (`robot_landings_of_grouped` of `crates/sync-core/src/robot_landing.rs`, the
  same for `bd`, [Agent CLI](../cli/agent-cli.spec.md#bd-status)): the robot's tested commits (the same commit, a
  patch-equivalent copy, or a copy with the same changes on a newer base, as the robot commit map compares them)
  against each branch as the table has it. A branch whose oldest
  commits the robot tested and whose other commits were added after them landed, those commits the only work left:
  "Landed with oauth · 1 commit after it" ("Landed · 1 commit after it" of the robot's own branch), its push and Safe
  Push offered for them. A branch with a commit changed since, or a new one below a tested one, is not landed. Asked
  once per state of the branches (`ContentMemo`, keyed by the robot's commit and the commits of the branches), on the
  table the last sync left; of several robots that landed a branch, the one that left the fewest commits after it counts.
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`)
  [@test] ../../src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
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
  the files (the intent `fetchBaseline`, an operation of the session of the robot's repository). Meanwhile the Update
  button of the robot's repository shows the fetch ("Fetching 45%"), never the button of another repository selected
  meanwhile; then the sync its session asks checks the update against the fetched baseline, without expanding or
  scrolling. A failed fetch is an error toast.
  [@test] ../../src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)
  [@test] ../../crates/operations/src/update_test.rs (`an_update_during_a_fetch_goes_on_from_that_fetch`)
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
  by what it tests), for 10 s, with "Cancel Safe Push" first (the action of the robot, `getRobotActionItems`) and "Open in
  Patronus" below the text, recorded in Activity. In the background an OS notification says it at once while "Safe
  Push failures" is on in Settings → Notifications (`notifyPatronusFailures`, on by default), and the toast shows on the next focus.
  [@test] ../../src/stores/patronus.test.ts (`tells once that a running robot will fail, with Cancel Safe Push and the failed checks`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Failed

- A robot that failed is told once, as a merge is (news of the session, `robotFailed`: running or canceling in one
  answer, failed in the next; a robot first seen failed is no news), once the session read its problems, which say
  whether it failed for missing code-owner approvals; without them when their read failed, and at once for a robot
  whose problems the session doesn't read (a watched robot, one of no branch of the table):
  - an `error` toast "Safe Push of auth failed" / "Fast Tests · auth → master" (the checks that failed from its
    details, else what it tested), with "Copy Prompt for Agent" first, or "Restart Safe Push" (the action of the robot,
    `getRobotActionItems`, where a restart is possible) when its builds never started, and "Open in Patronus" below the
    text ([Safe Push Failure Prompt](safe-push-failure-prompt.spec.md#the-failed-toast));
  - for missing approvals a `warning` toast "Safe Push of auth needs review" / "The code owners did not approve the
    changes" (a robot started with a profile: "Large Scale Change doesn't skip the code-owner review: the code owners
    did not approve the changes", what a [Safe Push without review](#safe-push-without-review) learns only now), with
    "Open Pull Request", or "Create Pull Request…" for a pushed branch without one: the approvals count in the pull
    request, a restart before them fails again;
  - recorded in Activity, and in the background the OS notification of "Safe Push failures" (`notifyPatronusFailures`),
    the toast on the next focus.
  The issue keys of the toast's title and description ("Safe Push of JIRA-456 failed" / "JIRA-456 → master") link to the
  tracker. A robot told failing while it ran ([above](#failing)) is told no second time: its Activity entry says it
  failed, and its toast, while still open, says it in place, with the step of the failure. The toast of the failure
  takes the place of the toast of the robot's start. The watched robot of a reviewed pull request is told by what it
  tests.
  [@test] ../../crates/repository-model/src/facts_test.rs (`a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and copy the prompt for an agent from the toast`, `should restart from the toast a Safe Push whose builds never started`)
  [@test] ../../src/stores/patronus.test.ts (`tells once that a robot failed, with Copy Prompt for Agent and the failed checks, once its problems are read`, `tells a robot that failed for missing approvals "needs review", with its pull request`, `tells no second time a robot told failing while it ran: its toast, while open, says in place that it failed`, `tells nothing of a robot told failing whose toast is gone: Activity says it failed`, `tells a robot whose problems didn't come without them`, `names the profile of a robot started without the approvals whose profile checks them all the same`)
- The step of a failing or failed toast is the step of the robot as the stores have it, rendered again on every change
  of the robots, the branches, the running operations and the shown repository
  ([One toast per event](../../docs/contributing/ux-writing.md#one-toast-per-event)), and a click runs the step shown
  now: Cancel Safe Push leaves the toast once the robot finished or was canceled, a step that runs is disabled, a
  disabled one is not offered, and a robot of another repository than the shown one offers no step of a branch (the
  branch of the same name there is another one). A restart made here takes the place of the toast; one made elsewhere
  (a newer robot of the same subject and push mode in the next list) turns it into an `info` toast "Safe Push of auth
  was restarted" / "auth → master" with "Open in Patronus" of the newer robot. A state of an older robot never takes
  the place of the toast of a newer robot of its subject: the failure of the robot a start replaced, told after the
  start's toast (a robot that finished while the start checked), leaves "Safe Push started" as it is, and Activity
  records the failure. The pull request step opens what the branch has at the click.
  [@test] ../../src/stores/patronus.test.ts (`drops Cancel Safe Push once the robot was canceled elsewhere, in place: the toast keeps the robot page`, `says in place that the robot was restarted elsewhere, and opens the newer one`, `leaves the toast of the start that replaced a robot whose failure is told after it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push that replaces a robot which finished meanwhile, without a cancel`)

### Code-owner review

- Patronus fails a Safe Push whose changes miss the approvals the code owners require (`.ownership/review-rules.yaml`
  of the target branch, PAT-A-109): the problem "Changes require review approval". The approvals count only in the
  review linked to the robot: the pull request whose comment started it, else the Space merge request of the start. A
  dry run is not checked.
- Patronus is the only judge of whose approvals count; the app and `bd` ask it and render its answer
  ([Git Operations](../../docs/design/git-operations.md#patronus-review-pre-check)).
- A Safe Push checks the review before the start (never a dry run, never for the chip) with the readiness the session
  of the repository reads ([Next Step](next-step.spec.md#the-readiness-store): what a surface needs at once in a call
  of its own, the rest a queue of at most 4 at a time after the answers the object cache keeps), with the core
  `bd safe-push` shares (`remote_facts::review_readiness`): the commits of the branches at the tips of the session's
  view (an answer is keyed by its branch and tip), from one `git log --topo-order` of all the branches of a call (a
  stacked branch shares its parents' commits; 500 commits or more of a branch skip its check, as the IDE does), their
  changed paths from the `commit_files` store of the object cache (one `diff-tree --stdin` for the commits it misses; a
  rename is a delete and an add, a deleted path counts, a merge has none), then one
  `POST /pre-merge-approval-required-check` per branch whose kept answer doesn't hold.
  [@test] ../../crates/remote-facts/src/patronus_review_test.rs (`reads_the_commits_of_the_branch_with_their_changed_paths`)
  [@test] ../../crates/remote-facts/src/patronus_review_test.rs (`reads_several_branches_in_one_log`)
  [@test] ../../crates/remote-facts/src/patronus_review_test.rs (`does_not_read_a_branch_with_too_many_commits`)
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`the_kept_answers_come_from_the_cache_without_asking_patronus`)
  [@test] ../../crates/test-server/src/review_readiness_test.rs (`the_session_asks_the_readiness_a_window_needs_from_the_fake_patronus`)
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
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`a_rest_start_is_checked_without_the_pull_request`)
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
  [@test] ../../crates/remote-facts/src/review_readiness_test.rs (`the_answer_about_a_pull_request_is_kept_until_a_review_changes`, `a_failed_check_is_unavailable_and_not_kept`, `an_answer_with_the_evaluation_holds_a_day_one_without_it_minutes`, `a_tip_gone_since_the_page_sent_it_makes_only_its_branch_unavailable`)
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
  fails for them. `unknown` lets it start, as the robot decides. `unavailable` (the pre-check failed) is no answer: it
  refuses the start as `review_unknown` until the user says to start anyway (`--allow-unknown-review`, the decision's
  confirmation `ReviewUnknown`, D14). It collects the approvals in the GitHub pull request
  (`--create-pull-request`), never a Space merge request ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`, `a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed`)
- Space merge requests are deprecated: the approvals go into the GitHub pull request of the branch, and Branch Deck
  never creates a merge request. One Patronus finds open still counts: the start links it.
- Where the approvals a start misses count is one rule of the app and `bd` (`domain_policy::review::review_state`): the
  pull request the robot starts from, else the GitHub pull request the branch doesn't have yet; a repository only on
  Space is read once there, whatever reason the answer gives: the merge request Patronus found open, else nowhere known
  (its GitHub repository unknown). No answer is pending, never missing.
  [@test] ../../crates/domain-policy/src/review_test.rs (`the_review_says_where_the_missing_approvals_count_and_reads_a_repository_only_on_space_once`)
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
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../src/lib/patronus.test.ts (`names a robot that failed for missing approvals "Needs review"`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_problems_of_a_failed_robot_of_the_table_are_asked_until_known`)
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`an_idle_poll_asks_one_request_and_problems_only_for_changed_robots`, `a_change_of_the_table_asks_the_problems_of_a_failed_robot_it_matched_anew`)
- The reads of the robots and of their problems must be of the session of the repository: another repository lists
  its robots and asks why its failed robot failed while a list of the first is held.
  [@test] ../../crates/remote-facts/src/scheduler_test.rs (`another_repository_lists_and_asks_its_problems_while_the_list_of_the_first_is_held`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the pull request a robot that failed for missing approvals was started from, from its details`)

### Safe Push without review

Some merge profiles skip the code-owner review (Large Scale Change; an emergency profile skips all checks). Patronus
doesn't say which before a start, and a `/patronus` comment can't choose a profile, so a Safe Push without review is an
explicit choice of the user, offered only where the approvals count in a pull request the branch doesn't have, or may
(GitHub signed out or failing: no pull request is known, [Next Step](next-step.spec.md#facts-without-an-answer)).

- The "Create pull request" dialog of a branch at row 7 of the [next step](next-step.spec.md) (missing approvals, no
  open pull request) has "Safe Push Without Review…" at the left of its footer, a ghost button with the
  `ShieldOffIcon`, which takes neither ⌘↵ nor the focus; its hover: "Starts the Safe Push with a merge profile that
  skips the code-owner review, such as Large Scale Change", disabled with the reason of the branch (a sync, a running
  robot), never the approvals (`getSafePushWithoutReviewItem`, `BranchNextStep.withoutReview`). A start that skips the
  approvals (this one, or a default start whose profile is known to skip the review) is held back by the step only by
  the reasons of the subject on every row (a sync, a failure, no commits, the robots, parked, landed, what runs on the
  row: `BranchNextStep.subjectBlocker`; its decision has no fact of a sync), never by the step's own (the approvals, a
  draft, another base), the click and the popover alike (`startStepBlocker`); its decision says the rest. It closes
  the dialog,
  reveals the row and opens the popover of its Safe Push without review, also when the popover of the row is open
  (it opens again in the other mode). The dialog of any other step has no such button.
  [@test] ../../crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] ../../src/presenters/next-step.test.ts (`row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly`, `rows 8 to 10: changes requested, Request Review…, the wait naming the people asked`)
  [@test] ../../src/presenters/next-step.test.ts (`is the step's for a Safe Push, of a start that skips the approvals the reasons of the subject on every row, of a Dry Run its own`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`, `should not offer it in the dialog of a branch whose Safe Push needs no pull request`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover without review from the pull request dialog`)
- The popover is "Safe Push of auth" with the start "Safe Push Without Review". Its review line names the missing
  approvals: "Starts without the approvals of IJPL Code Platform: the profile you choose has to skip the code-owner
  review.". The profile Select (the profiles the plan offers, `Plan.alternatives`) has the groups "Skip the code-owner review" (`skipsReview` true),
  "Other profiles" ("Profiles" without the first group; `skipsReview` null) and "Emergency"; a profile known to check
  the review is left out, and there are no default checks. None is chosen unless the last robot's profile skips the
  review: the footer says "Choose the profile that skips the code-owner review" and the start waits. Below the Select,
  the profile's description from Patronus and a line: "Large Scale Change skips the code-owner review: the last Safe
  Push of the branch ran with it.", or "Patronus tells only when the robot starts whether Large Scale Change skips the
  code-owner review: if it doesn't, the Safe Push fails at once.", or for an emergency profile in `text-destructive`
  with the warning triangle: "Emergency skips all checks of the Safe Push, not only the code-owner review: Patronus
  pushes into the target without testing it." The start is the REST start, announced and recorded in Activity as any;
  a profile that checks the review fails it before any build, told as [failed](#failed) with the profile named. Once a
  pull request is open or the approvals are no longer missing, the popover is the usual one. `bd safe-push` decides the
  same: a start through the API with a profile known to skip the review (`--profile`, or the last robot's) starts
  without the approvals, its `review.skipped_by` `profile` ([Agent CLI](../cli/agent-cli.spec.md#bd-safe-push)).
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_start_offers_the_profiles_of_its_mode_and_without_approvals_the_emergency_ones_and_a_skipping_default`)
  [@test] ../../crates/bd-remote/src/safe_push_test.rs (`a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`, `should say that an emergency profile skips all checks, and start with it`, `should start with the profile of the last robot when its options said it skips the review`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Restart and cancel

- Restart (the popover, the branch menu) is offered for a failed, canceled or canceling robot started on the branch,
  and for a passed dry run when the remote branch has a newer commit; Patronus refuses the others. Not for a robot that
  failed for missing approvals in a Space merge request; a robot of a pull request that did is restarted, as the restart
  counts the approvals the pull request has by then (its popover says so). A Safe Push of a
  temporary ref (IDE, CLI) is not restarted from here: it would test its old commit. The restart is a new robot, on the
  target as it is at the restart: the branch of an old robot needs no push of a newer baseline. What a restart does is
  one rule of Rust (`domain_policy::holds::restart_action`, the view's `restarts` by robot id): a plain restart while
  the remote has the branch, a push first while the branch has work the remote lacks ("Push and Restart", also of a
  running robot, which the push cancels first); the page only presents it.
- A running robot a required check of which failed ("Failing": it will fail) is restarted too, without waiting for the
  rest of its checks: its restart cancels it, then Patronus restarts the canceling robot on the commit of the remote
  branch, as Patronus restarts no running robot. The restart is offered first, before "Cancel Safe Push", in the
  details, the branch menu and the landing overview; its hover says "Cancels the run on 78ca3b88 and starts it again on
  the remote branch". A running robot no check of which failed has no restart: its cancel is the way.
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_robot_offers_its_restart_with_why_it_cannot_start_now`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`an_answer_that_a_running_robot_is_failing_derives_the_patronus_view_again`)
  [@test] ../../src/presenters/robot-actions.test.ts (`names what a restart of a running robot a failed required check dooms does: it cancels it, then starts it again`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart it from its details: canceled first, then started again on the commit it tested`, `should restart it from the branch menu`)
- A restart is a start: it waits where "Safe Push" waits (`domain_policy::land::restart_refusal`, the `refusal` of the
  view's restart), disabled with the same reason on every surface, for a blocked branch, and, for a Safe Push, while a robot lands an ancestor of the branch or a branch built
  on it lands the branch: two robots would land the same commits. A dry run pushes nothing into the target, so only the
  block holds its restart back. Once a merged Safe Push landed all of the branch ([below](#merged)), its restart is
  refused, of a Dry Run too, as its Safe Push and Dry Run are ("Landed in master with oauth", row 1 of
  [Next Step](next-step.spec.md#the-table)): its commits are in the target. One with commits after the landed ones
  restarts, for them. A parked branch stays local, and so does a whole stack with a parked member: its restart is
  refused before anything else ("Parked: Unpark it to push"), as its start is; a restart that pushes first is refused for
  a branch the sync did not build. On the cached frame of an earlier run (its table, its robots) the restart and the
  cancel wait for the first sync ("Waits for the first sync", [Branch Row](branch-row.spec.md#actions-cell)): the robot
  may have finished or been restarted since.
  [@test] ../../crates/domain-policy/src/actions_test.rs (`on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays`)
  [@test] ../../src/presenters/robot-actions.test.ts (`presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_restart_is_refused_by_the_table_first_then_as_a_start_without_its_own_robot_then_by_a_landing`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should refuse the restart of the failed robot of a branch a merged Safe Push of a branch built on it landed, in its menu, its robot's details and its preview`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- Cancel is offered for a running robot; the robot is canceling until its checks stopped.
- A restart and a cancel are intents of the engine (`RestartRobot`, `CancelRobot`; `operations::robot`), decided in
  Rust over the facts of the session, never the page's, and decided again by `execute` before the first effect: the
  plan of a restart is the restart the view offers, refused by the refusal of the view ("Nothing was restarted: …"); a
  robot of a temporary ref, a running one on the commit of its branch no required check of which failed and a merged
  one have none. A plain restart moves no ref: no lock; of a running robot it cancels it first (`[CancelRobot,
  RestartRobot]`). A restart that pushes first retargets the pull request of the branch out of a virtual branch,
  cancels the running robot, pushes the branch (the lock only for the witness and the freeze) and restarts the robot
  after the push, skipped when the pushed branch moved since; a push that drops commits of the remote the branch lacks
  refuses it ("the remote branch of auth has 2 commits auth lacks"). A cancel of a robot that finished meanwhile (as
  the session or its status says) is answered as it ended, nothing canceled ("Safe Push already finished"); the
  robot a start or a restart replaces is canceled the same way, so the start or the push goes on.
  [@test] ../../crates/operations/src/robot_test.rs (`a_restart_restarts_alone_while_the_remote_has_the_branch_and_pushes_first_a_branch_that_changed`, `a_restart_that_pushes_first_cancels_the_running_robot_pushes_and_restarts_it_on_the_pushed_commit`, `a_restart_on_the_remote_branch_takes_no_lock`, `a_restart_of_a_failing_robot_cancels_it_and_restarts_it_on_the_same_commit`, `a_cancel_of_a_robot_that_finished_is_as_it_ended_and_cancels_nothing`)
  [@test] ../../crates/patronus/src/actions_test.rs (`a_fake_of_the_state_only_lists_the_robots_a_start_and_a_restart_start_and_cancels_them`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_start_restart_and_cancel_run_the_real_service_against_the_fake`, `a_cancel_of_a_robot_that_finished_is_answered_as_it_ended_and_a_start_goes_on`)
  [@test] ../../src/lib/robot-run.test.ts (`answers a robot that finished meanwhile as it ended: nothing was canceled`, `says what its plan refuses it by, and runs nothing`)
- A restart or a cancel runs once per robot (an operation of the session, [Running Operations](../../docs/design/operations.md)):
  while it runs, it is running on every surface of the robot (the slot, the details, the landing overview: disabled,
  a spinner in place of its icon, `aria-busy`), a second click joins it, a cancel during a restart waits for it; the
  restart is the move of its row from the click on (the row lands). A Safe Push of what the robot lands waits for it.
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should restart a robot once while its restart runs, running in the slot and in its details, landing from the click`)
  [@test] ../../crates/operations/src/robot_test.rs (`a_second_restart_of_a_robot_joins_the_first_and_its_cancel_and_a_start_of_its_branch_wait_for_it`)
  [@test] ../../src/presenters/robot-actions.test.ts (`runs the restart of the engine from both restarts, and shows one running in the session running on every surface`)
- Rust decides the actions of a robot (`domain_policy::actions::robot_actions`, `robot:<id>` of the view's `actions`):
  a restart while one is offered (`restarts` of the view), disabled while Patronus didn't tell why it failed, while the
  step of its branch misses approvals (a restart would fail again: the words of that step) or a start is refused; the
  cancel of a running robot; the prompts of a failed robot (not for approvals) or of a running one whose required check
  failed, disabled as the restart while why it failed is to come. One builder presents that list
  (`getRobotActionItems`), and every surface of the robot renders it: the branch menu, the details (the chip popover,
  the preview section), the robots indicator, the toasts of the robot and the [landing overview](../repository/landing-overview.spec.md#overview).
  In this order: "Restart Safe Push" or "Restart Safe Push" (below), "Cancel Safe Push", "Open in Patronus",
  and "Open Pull Request #12" for a robot started from a pull request (the branch menu leaves it out when it is the
  branch's own pull request, which its "Open Pull Request" opens). Each names the mode of the robot ("Restart Dry Run");
  a bare "Cancel" in a popover reads like closing it. The menu of a branch that can't change (a conflict, a sync error, a
  sync running) keeps "Cancel Safe Push" and "Open in Patronus": they don't change the branch.
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)
  [@test] ../../src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`, `presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`)
  [@test] ../../crates/patronus/src/actions_test.rs (`restart_answers_the_new_robot_and_cancel_the_canceling_one`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
- A restart is the toast "Safe Push restarted", a cancel "Safe Push is canceling", each naming the branch and the
  target ("auth → master") with "Open in Patronus" (the new robot's page for a restart), as a start does, in place of
  the toast of the robot they restart or cancel; Activity records them in the same words. A failure is "Cannot restart
  the Safe Push" / "Cannot cancel the Safe Push"; a refusal of the plan and facts that changed after the push say why in
  an info toast under the same title.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
  [@test] ../../src/stores/patronus.test.ts (`is replaced in place by the toast of its restart, a newer robot of the branch`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should restart from the toast a Safe Push whose builds never started`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
  [@test] ../../src/lib/robot-run.test.ts (`restarts through the engine and says it in the toast of the robot`, `cancels through the engine: the robot is canceling`, `says the change of the facts after its push in an info toast, and a failure of Patronus in an error toast`)
- Patronus restarts on the newest commit of the remote branch. When the branch has work the remote lacks (by content,
  or commits reworded since its push: the remote has them with the old messages, which Patronus would land), every
  surface offers "Restart Safe Push" instead of Restart: the push of the row, then the restart on the pushed commit,
  also for a running robot, which it cancels first ([below](#pushing-into-a-branch-whose-safe-push-runs)); a passed dry run counts
  the pushed commit as the newer one. It is disabled for a blocked branch, with the reason. The restart is the action:
  its toast "Safe Push restarted" is the one toast, the push has none (Activity records both).
  [@test] ../../src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../src/presenters/robot-actions.test.ts (`presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`, `names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview, landing from the click`)
- A failed action is a toast with what Patronus said; without a token it says "Run ./safePush.cmd once to log in to
  Patronus" and sends nothing.
  [@test] ../../crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Pushing into a branch whose Safe Push runs

- A branch is held (`PushHold`) while a Safe Push that pushes into the target runs on it (started on the branch, not of
  a temporary ref) or on a branch built on it whose commit has commits of it, which lands them ("lands with oauth"). A
  push of a held branch won't land: Patronus merges the commit it took at the start, and a push into its own branch
  fails it at the end ("branch was modified"). A dry run and a canceling robot hold nothing. The Safe Push of the whole
  stack (on its stack branch) holds every branch of the stack its commit has: "A Safe Push of the whole air stack is
  running: it lands 1 commit of auth with it" ([Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)). The
  reason counts the commits of the branch the robot lands once that is told, as `bd` does. A branch a merged Safe Push
  landed whole ([Merged](#merged)), the branch it ran on included, is held too, before any other hold, until the update
  archives it: "Landed in master with oauth: nothing of it is left to push", "Merged by Patronus: nothing of it is left
  to push"; one with commits added after the landed ones is not: they are work for the remote. A landed branch is only
  left out of the pushes of a stack: the branches built on it push without it (Push with Children of a landed root,
  [Stacked Branches](stacked-branches.spec.md#pull-requests)), while a running Safe Push holds every push of its stack.
  The hold is a rule of Rust, one for the app and `bd` (`domain_policy::holds::branch_hold`, the view's
  `landing.hold`): a landing that left nothing, else the branch's own running Safe Push, else the first running Safe
  Push that lands it; the page only words it (`src/presenters/reasons.ts`).
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
- The stack branch of a whole stack ([Whole Stack](whole-stack.spec.md)) is held as a branch is, by one rule of Rust
  for the app and `bd push --with-children` (`domain_policy::holds::stack_branch_hold`, the view's `landing.hold` of
  the stack): a parked member keeps it local ("Holds parked login: land the stack in pieces"); then merged Safe
  Pushes, of pieces or of the whole stack, that landed every branch of the stack whole, which says who landed them,
  each robot once, in the order they merged ("Landed in master with deploy and side-test: nothing of it is left to
  push", "Merged by Patronus: …" of its own robot alone); then its own running robot ("A Safe Push of the whole air
  stack is running: wait until it merges, or restart it on the current stack"); then a stack branch the sync did not
  build. Push with Children leaves it then, and its push alone (the push step of the restart of its robot, of Request
  Review… and of the creation of its pull request, `pushWholeStack`) is refused with that reason.
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build`, `every_member_landed_by_the_robot_of_the_whole_stack_is_its_own_landing`)
  [@test] ../../src/stores/patronus.test.ts (`holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them`)
  [@test] ../../src/presenters/reasons.test.ts (`says that nothing is left to push of a stack branch merged Safe Pushes landed, and nothing for no hold`)
- The app and `bd` decide what the robots land, landed and hold with one rule over the branch table of the last sync
  (`repository_model::facts::derive_patronus`, [Session facts](../../docs/design/git-operations.md#session-facts)):
  the session of the repository derives it again when its table committed, its robots changed or a push moved a
  remote branch, in the blocking pool, and sends it as the part `patronus` of its view; until Patronus answered and
  the first view is derived it is `loading` (the rows wait for it), while a newer one is derived `stale` (no row waits),
  and a view that can't be derived is `unavailable` with why, derived again after the next answer of Patronus:
  - the robot of a branch is the one the session matches (`domain_policy::robot_match`, named in the view as
    `robotId`): running over canceling over finished, then the newest, then the first of the list
    (`crates/domain-policy/src/robot_match_cases.json`);
  - of the robots of branches built on a branch, the first of the table lands it ("lands with"), and of those that run
    the first holds it: a canceling robot says it lands the branch and hides no running one;
  - a branch's own robot is never the robot of an ancestor it waits for: its Safe Push replaces it (the view's
    `landing.replaces`: its own robot on an older commit, which the push of the start cancels);
  - a start says first that the branch is blocked, then that a robot runs on its last commit, then that a robot built
    on it lands it, then that a robot lands an ancestor (`domain_policy::land::start_refusal`, the view's
    `landing.start`); a Dry Run says the first two, then, while its start pushes, that a running Safe Push of a branch
    built on it lands the branch: its push would fail that Safe Push at the end, as any push into what a running Safe
    Push lands (`landing.dryRun`, D15: `bd safe-push --mode dry-run` refuses it too, `robot_running`);
  - the push an action needs (a pull request, a review request, a start) is refused, before it is made, by a branch of
    its chain up to the first held ancestor, past one that landed (a merged Safe Push landed it whole, or the fetched
    baseline has every commit of it, as the decision of a start passes it), that the sync did not build or whose push
    drops remote commits (`domain_policy::holds::push_blocker`, the view's `landing.push`);
  - the hashes tell, without git, when the robot took its branch as it is (its tip, or its remote branch that has the
    same commits) or a commit of it the branch built new ones on: the copies a sync wrote say which copy is a commit of
    the branch now, so an amend below the robot's commit in the same sync is told by content.
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`, `the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it`, `the_robot_of_the_whole_stack_lands_every_branch_of_it`, `the_merged_robot_that_left_the_least_work_names_the_landing`, `a_dry_run_lands_nothing`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_dry_run_whose_push_goes_into_a_running_safe_push_is_held`, `the_push_of_an_action_is_refused_by_an_unbuilt_or_dropping_branch_of_its_chain`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed`)
  [@test] ../../crates/repository-model/src/branch_table_test.rs (`the_table_of_a_sync_carries_the_branches_the_fetched_baseline_has_whole`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
  [@test] ../../crates/domain-policy/src/land_test.rs (`a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor`, `a_dry_run_waits_for_the_last_commit_and_for_the_span_hold_of_the_push_it_makes`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`the_push_of_an_action_stops_at_a_held_ancestor_and_refuses_a_branch_not_built_or_dropping_commits`)
  [@test] ../../crates/repository-model/src/patronus_view_content_test.rs (`the_hashes_answer_a_robot_at_its_tip_and_one_under_new_commits_without_git`, `a_root_restacked_below_a_running_robot_is_told_by_content_once`, `an_amend_below_the_robot_is_told_by_the_copies_the_sync_wrote`)
  [@test] ../../crates/domain-policy/src/robot_match_test.rs (`the_robots_match_their_branches_by_the_cases`)
  [@test] ../../crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`, `a_view_that_failed_is_derived_again_after_the_next_answer_of_patronus`)
  [@test] ../../crates/repository-model/src/facts_test.rs (`the_session_knows_the_remote_branches_a_push_moved_without_a_sync`)
  [@test] ../../src/stores/patronus.test.ts (`waits for the view of the robots as they are while the session derives it, then takes it`)
- What a running robot lands is what its commit has, not the stack as it is now: while it took the branches as they
  are (it runs on the tip of its branch, or on a commit of it with new ones on top) every branch it is built on, with
  all their commits; after a restack or an amend below its commit, which gives the branches new copies, the content
  tells (the repository view, `robot_spans_of_grouped` and the walks of [Robot landing](../../docs/design/git-operations.md#robot-landing)): a branch
  is held while the robot's commit has a commit of it, the same, a copy on a newer base or an earlier version (amended
  since). A parent a restack placed below the robot's branch after its start has none: its push, its Create Pull
  Request…, its Safe Push and its Dry Run go on, its row says no "lands with", and the span of the graph and the count
  of the island start below it. A robot of a temporary ref is asked for the branch it was matched to by the tip. Until
  the answer comes, and when the repository can't tell (it lacks the robot's commit: started on another machine,
  pruned), the stack as it is now holds, the number of commits not named. A push decides on its holds after the answer
  for the branches as the sync leaves them: the push of a branch alone on the view Rust derives of the robots it asked
  for the push (`operations::push`), a push of a stack too (`push_plan`). Not looked for: a commit moved out of the robot's
  branch into a branch that is no ancestor of it, as the landing of a merged robot doesn't.
  [@test] ../../crates/repository-model/src/patronus_view_content_test.rs (`a_root_restacked_below_a_running_robot_is_told_by_content_once`)
  [@test] ../../src/stores/patronus.test.ts (`holds only the ancestors the view says the commit of a running Safe Push has, and counts their commits`)
  [@test] ../../src/lib/stack-lanes.test.ts (`starts below a root the robot's commit doesn't have: a restack placed it there after the start`)
  [@test] ../../crates/sync-core/src/robot_landing_test.rs (`a_running_robot_at_the_tip_lands_the_branch_and_what_it_is_built_on`, `a_parent_restacked_below_a_running_robot_is_not_in_its_span`, `a_parent_split_out_of_the_commits_of_a_running_robot_is_in_its_span`, `an_amended_commit_below_a_running_robot_is_in_its_span_as_an_earlier_version`, `a_commit_moved_into_a_branch_that_is_no_ancestor_is_not_in_the_span`, `a_missing_source_leaves_the_span_unknown`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`the_span_of_a_running_robot_after_a_restack`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a branch a restack placed below a running Safe Push after it started, and hold what its commit has`)
  [@test] ../../src/presenters/reasons.test.ts (`says why a held branch offers no push: its own Safe Push, one that lands it with the commits it counts`, `says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)
  [@test] ../../src/lib/patronus.test.ts (`holds pushes with a running Safe Push only: a dry run or a cancel holds nothing`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)
  [@test] ../../crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `a_dry_run_lands_nothing`, `new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`)
  [@test] ../../src/stores/patronus.test.ts (`holds a branch whose Safe Push runs and the ancestors it lands; not once the robot no longer runs`, `holds every branch the whole-stack Safe Push lands`, `holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`, `holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)
- A held branch offers no push: the place of the row's push button stays empty, as its Safe Push slot does; its Status
  cell says why (the robot chip, "lands with oauth", "Landed with oauth"). P does nothing; Push in the menu, the preview footer, the outgoing
  overview and the review window is disabled with the reason: "A Safe Push of auth is running: wait until it merges, or
  restart it on the current branch", "A Safe Push of oauth is running: it lands 1 commit of auth with it". The description of the
  preview's push offers what it names, from the robot's actions (`getRobotItems`): "Restart Safe Push" of its own
  robot and "Open in Patronus" of the robot that holds it (`push-hold-actions`). A push of several branches leaves it
  out (`domain_policy::push::push_plan`, `Kept::Held`): Push Stack sends only the branches above it, Push with Children
  of the island's header and of the root's menu skips it.
  [@test] ../../src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open and restart the robot from the description of the preview's push, which says why it can't push`)
  [@test] ../../crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`, `push_with_children_leaves_out_what_asks_first_is_held_or_landed`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should offer no push: it won't land, and the menu says why`)
- A push that raced with a start (the IDE, `bd`, another window) goes to Rust, which reads the user's robots now
  (below): still running, it pushes nothing and says why ("Cannot push branch auth", with "Open in Patronus" of the robot the
  view says holds it); finished meanwhile, it pushes. A branch a merged Safe Push landed is refused by the plan of the
  push (the hold `landed`): the robot finished, its commits are in the target. Nothing asks: no dialog.
  [@test] ../../src/lib/push-run.test.ts (`is refused in the words of its blocker, and the toast opens the Safe Push that holds the branch`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes`)
- The app's push itself (`execute` of the intent `Push`, of a branch alone and of a stack, the E2E server's route of
  it) checks the same in Rust before it pushes, by the rules of `bd push` (the plan of the push, `derive::alone_push_of`
  and `domain_policy::push::push_plan` over the holds of `push_guard::guard_facts`, one sequence of both hosts,
  `operations::push::execute_push`): it reads the user's robots first (one full list asked now, never one
  kept for the page, a background request: none off Space, without a token, while Patronus fails or its requests back
  off, as `bd push` reads its holds; none for unapplied branches alone, on which no Safe Push runs; no network request
  runs under the repository lock), then under the lock refuses a branch the last sync has no row of ("… is no branch of the last
  sync"), a branch a merged Safe Push landed whole ("… merged into master with its Safe Push (…): nothing of it is left
  to push"), a branch whose own Safe Push runs, one a Safe Push of a branch built on it or of its whole stack lands,
  and the stack branch of a whole stack while its robot runs on it, every branch of it landed whole or it is not
  `built` (a sync since the page's decision left it conflicted or stale): "Nothing was pushed: …", shown in the error
  toast of the push ("Cannot push branch auth"). The robots are the ones Rust read for the push, never the page's and never a
  list kept before it: a robot the page canceled before its restart is canceling and holds nothing, a robot that
  finished holds nothing, and a robot the IDE or `bd` started since the page's list holds. A push raced with a start,
  or one that came by IPC without the page's checks, pushes nothing then.
  [@test] ../../crates/patronus/src/service_test.rs (`the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere`, `the_checks_send_nothing_while_the_requests_back_off`)
  [@test] ../../crates/operations/src/push_test.rs (`the_push_of_an_unapplied_branch_reads_no_robots`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes`)
  [@test] ../../crates/operations/src/push_test.rs (`the_push_of_an_unapplied_branch_reads_no_robots`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push nothing into a branch whose Safe Push started after the page's list: Rust refuses it, and the toast says why`)
- The app's push must hold the repository lock only while it decides: the checks above and the commits that go,
  fixed under it; the network push after it sends those commits (`<commit>:refs/heads/<name>`), each leased on the
  remote head the row showed, so a sync or a rewrite that moves a branch meanwhile changes nothing of what goes. Safe
  Pushes (or pushes) pressed on several branches one after another push side by side; none waits for another one's
  network push, and none is refused with the app itself as the holder ("`branch-deck` (pid …, for 10 s) holds the
  repository"). Only the pushes of one remote ref (the same branch twice, two whole stacks of one stack branch) wait
  for each other, without a bound: the registry of the operations of the session admits a push per ref it writes
  (`Queued` behind the push before it; a second click of the same push joins it), and the second then finds the
  remote at its commit. A push that sends holds no lock: another branch's push, a rewrite and a sync finish meanwhile,
  and it sends the commit it froze. A push whose window closes runs to its end in the task of its command, its refs
  held until its `git push` ended, and its outcome goes to the session.
  [@test] ../../crates/operations/src/push_test.rs (`the_pushes_of_two_branches_run_side_by_side`, `the_pushes_of_one_ref_wait_for_each_other_and_both_push`)
  [@test] ../../crates/operations/src/push_test.rs (`a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze`)
  [@test] ../../crates/sync-core/src/push_test.rs (`a_prepared_push_sends_the_commit_it_was_prepared_for`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Pushes of two branches pressed one after the other, side by side`)
- While another holder has the repository lock (`bd pick --follow` of an agent for 48 s, a rewrite of the app), the
  push must wait as long as the holder lives, never refused after 10 s, and say whom it waits for: the step of the
  Safe Push and the push button show "Waiting…", their name and hover "Waiting for `bd pick` (pid 4242, for 12 s)",
  updated about once a second, in every window (the step of the operation in the view names the holder,
  `WaitingForLock`), also for a start that waits behind another start of the app in the app's line; once the holder
  releases the lock, the push goes. Only a lock that no record names a live holder of (a holder that writes none)
  refuses the push after 10 s: "Nothing was pushed: another Branch Deck operation holds the repository; try again when
  it is done".
  [@test] ../../crates/operations/src/push_test.rs (`a_push_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`the_wait_for_a_live_holder_names_it_ends_when_canceled_and_refuses_an_unnamed_one`)
  [@test] ../../crates/history-rewrite/src/repository_lock_test.rs (`every_wait_of_this_process_names_the_other_process_that_holds_the_lock`)
  [@test] ../../src/presenters/lock-wait.test.ts (`says whom an operation waits for as the rewrites of the app say it: bd by its command line, the app by its name`)
  [@test] ../../src/presenters/lock-wait.test.ts (`reads whom an operation of the view waits for from its step, and nothing of another step`)
  [@test] ../../src/lib/transfer-progress.test.ts (`names the holder of the repository lock the push waits for, without a percent`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should wait for another holder of the repository lock as long as it lives, and then start`)
- Stop Waiting must end the wait of a push for another holder of the repository lock, before anything was pushed: in
  the hover of the push button and beside the step of a Safe Push while its push waits. The push answers
  the code `canceled`, the info toast "Canceled the push of auth before it started" says "Nothing was pushed", and the
  action whose step the push is (the Safe Push start, a review request) starts nothing and shows no error; the same
  step pushes and starts once pressed again. Safe Pushes and a move pressed while the holder holds the lock all run
  once it released it.
  [@test] ../../crates/operations/src/push_test.rs (`a_push_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] ../../src/lib/transfer-progress.test.ts (`offers Stop Waiting of a push only while it waits for another holder of the repository lock`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should stop the wait of a Safe Push for another holder of the repository lock with Stop Waiting beside its step, nothing pushed or started`)
  [@test] ../../tests/e2e/repository/repository-lock.spec.ts (`should say whom a push waits for in the hover of its button, whose Stop Waiting pushes nothing`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should run two Safe Pushes and a move started while another process holds the repository lock once it released it`)
- A Safe Push, a Dry Run or a push that another process of the repository runs (an agent's `bd safe-push auth`, in this
  worktree or another one under the same branch prefix) must show on its row in every window while it runs: the slot
  of the row busy with "bd: Safe Push of auth…", its hover naming the process ("`bd` (pid 4242) runs the Safe Push of
  auth, which holds what it needs until it ends"). S, a click and the menu say why the start can't start in an info
  toast ("Safe Push of auth can't start" with those words) and open no popover; the push of the branch, which needs
  what it holds, is disabled with it before Rust is asked (`Busy`: "bd: Safe Push of auth…" on its item and its
  button, P says it in an info toast), and a push or a start of the app that reaches Rust all the same is refused as
  `busy` before anything runs, its toast naming it ("Cannot push branch auth: `bd` (pid 4242) has run the Safe Push of auth
  for 12 s"); another branch pushes and starts beside it. Once the other process
  ended (its record left the operations registry of the repository), the row offers the start again and it starts once.
  The other way round, `bd safe-push auth` beside the app's Safe Push of auth is `busy` naming the app's operation.
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`the_apps_safe_push_beside_bd_of_the_same_branch_is_busy_naming_bd`)
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`bd_beside_the_apps_safe_push_of_the_same_branch_is_busy_naming_the_app`)
  [@test] ../../crates/operations/src/registry_watch_test.rs (`a_window_shows_the_operations_another_process_runs_under_its_prefix_while_it_watches`)
  [@test] ../../src/presenters/operations.test.ts (`words an operation of another process as its row says it and as the hover names its pid`)
  [@test] ../../tests/e2e/operations/foreign-operation.spec.ts (`should show the Safe Push bd runs on its row, refuse a start and a push of it, and push another branch beside it`)
  [@test] ../../tests/e2e/operations/foreign-operation.spec.ts (`should start the Safe Push of a branch once the operation of bd that held it ended`)
- A start whose answer was lost must never be sent again by itself, and must hold its subject for every process until
  it is known. Before its request goes out, the record of the start in the operations registry tells what it sends
  (its source ref, the commit, the route, the pull request, the time; the comment once GitHub answered its post). A
  request whose answer was lost after it went out (a timeout, a connection that broke off, an answer that doesn't
  read, a 504 of a gateway; for a comment also a bot that did not answer within the minute) is unknown, never failed:
  Patronus got the start once, and the outcome says why it is not known; its toast "Cannot tell whether the Safe Push of
  auth started" (Open Pull Request for a comment). The record stays after its run, as one of a process that was killed
  between its push and its start: the row of every window shows "Start Unknown" in its slot ("Cannot tell whether the
  Safe Push of auth started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start;
  Patronus may still run it"), with "Open in Patronus" ("Open Pull Request #7" for a comment) and "Forget This Start"
  beside it; S says it can't start, and a start of `bd` or the app that needs what it holds is `busy` naming it
  ("Cannot tell whether the Safe Push of auth that `bd` (pid 4242) sent started", `bd operation forget <op>`).
  A read of Patronus settles it: at an admission it holds back, and after each list of the robots of the session. Its
  robot is one of the user's robots of its source ref in its mode started since it was sent (on its commit when
  Patronus names one), or of its pull request; found, it is started (the admission is refused as `plan_changed`, the
  robot in the facts of the session that settled it and of the worktree that sent it, so neither row offers Safe Push
  of it again); a list of the whole window asked 60 s after it was sent at least (the 30 s a start waits for its
  answer, and as long again for a request Patronus still handles) without it means none started (the record leaves,
  and the start goes as planned, without the push that went before); a list asked earlier proves nothing, however late
  its answer came, so a list of the session (which may be one kept for the windows from before) proves only that it
  started, and without its robot a start that settled is asked once more since it was sent (one request for all such
  starts of the prefix, since the oldest), and no list kept from before hides the robot it finds. A comment is settled
  by the reply of the bot to it (found by its text among the comments of its pull request since it was sent when its
  post lost its answer): accepted started, rejected or never posted failed. Forget This Start (and
  `bd operation forget`) takes the record out without asking Patronus: the toast "Forgot the start of auth" says
  Patronus may still run it.
  [@test] ../../crates/patronus/src/actions_test.rs (`a_start_whose_answer_was_lost_is_lost_and_sent_once`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_lost_start_patronus_does_not_list_once_it_settled_leaves_and_the_admission_goes_in`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`the_robot_of_a_lost_start_is_of_its_ref_and_mode_since_it_was_sent`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_list_of_the_whole_window_proves_a_lost_start_absent_once_it_settled`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_lost_comment_is_found_by_its_text_and_settled_by_the_reply_of_the_bot`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_list_of_the_robots_of_a_session_settles_its_lost_starts`)
  [@test] ../../crates/operations/src/registry_file_test.rs (`a_start_of_this_process_whose_answer_was_lost_holds_its_own_admissions_after_its_entry`)
  [@test] ../../crates/operations/src/registry_file_test.rs (`a_forget_takes_out_a_start_whose_answer_was_lost_and_refuses_one_that_runs`)
  [@test] ../../crates/operations/src/forget_test.rs (`forget_this_start_takes_the_record_out_and_frees_what_it_held`)
  [@test] ../../crates/domain-policy/src/actions_test.rs (`a_start_whose_answer_was_lost_offers_open_in_patronus_and_forget_this_start`)
  [@test] ../../crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once`)
  [@test] ../../src/presenters/operations.test.ts (`words a start whose answer was lost: whether it started is not known, and how it lets go`)
  [@test] ../../src/presenters/outcome.test.ts (`says a start whose answer was lost as not known yet, with why`)
  [@test] ../../src/presenters/actions.test.ts (`offers Open in Patronus where its robot would be and Forget This Start, which says Patronus may still run it`)
  [@test] ../../tests/e2e/operations/foreign-operation.spec.ts (`should show a start whose answer was lost on its row, refuse a start of it, and start again once it is forgotten`)
  [@test] ../../tests/e2e/operations/foreign-operation.spec.ts (`should open the pull request of a comment start whose answer was lost`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`a_list_the_service_kept_from_before_a_lost_start_settled_never_proves_it_absent`)
  [@test] ../../crates/operations/src/reconcile_test.rs (`the_lost_starts_of_a_prefix_settle_by_one_list_since_the_oldest`)
- After a mutation of `bd` (an agent's `bd group`, `bd push`), the session a window shows must sync once what a sync
  reads changed, without a focus: `bd` touched the stamp of the repository, which the window watches.
  [@test] ../../crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows`)
  [@test] ../../crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_of_bd_runs_only_when_the_inputs_changed_as_a_focus`)
  [@test] ../../tests/e2e/operations/foreign-operation.spec.ts (`should sync when bd touched the stamp of the repository after a mutation, without a focus`)
- The one push into a held branch is a robot action: "Restart Safe Push" of a running robot whose branch
  changed since (the menu, the robot details, the landing overview; "Cancels the run on 1a2b3c4, pushes auth and starts
  it again on the pushed commit") cancels it, pushes and restarts it on the pushed commit, the steps of its plan in
  Rust (`RestartRobot`); the push shows on the push button of the branch (of the whole stack: "Pushing the whole stack
  45%"). The restart is the action: its toast "Safe Push restarted" with "Open in Patronus" is the one toast; Activity
  records the push and the restart. The advice of the details names it: "… Restart Safe Push to test the current
  branch." A push of the page that restarts a robot (the toast of a moved remote) cancels it and restarts it through
  the same intents.
  [@test] ../../src/lib/patronus.test.ts (`asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref`)
  [@test] ../../crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] ../../src/presenters/robot-actions.test.ts (`names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)
  [@test] ../../crates/operations/src/robot_test.rs (`a_restart_that_pushes_first_cancels_the_running_robot_pushes_and_restarts_it_on_the_pushed_commit`)
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_push_that_restarts_the_safe_push_of_its_branch_cancels_pushes_and_restarts_it`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot, push and restart it on the pushed commit, from Restart Safe Push`)
- A Safe Push of a branch whose Safe Push runs on an older commit replaces it, and confirms first: "Cancels the Safe
  Push of auth running on 1a2b3c4." Its push cancels the robot at the first branch that robot holds (the branch, or an
  ancestor the stack push takes along), never restarts it; the start then starts the one robot of the branch. Without
  a push (the remote branch has the commits by content) the start cancels it itself. A robot that finished while the
  start checked is not canceled: nothing is left to cancel, and the start goes on. The push of "Create Pull
  Request…" replaces nothing: it is refused as any push, and goes on as any push for a branch the robot's commit
  doesn't have. A Dry Run of a held branch that needs a push is refused with
  why: a dry run doesn't replace the Safe Push.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot for a new Safe Push from the popover, and not restart it`, `should cancel the robot it replaces without a push when the remote branch has the commits`, `should start the Safe Push that replaces a robot which finished meanwhile, without a cancel`)
  [@test] ../../crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
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
  nor one that something else holds: a running Safe Push of a branch built on it or of the whole stack holds it, a
  merged one landed it whole (the hold of the view), the baseline has all of it, a running Safe Push of a branch it is
  built on lands a part of it (a Dry Run lands nothing; a canceling one still counts, as for the start), or it is parked;
  nor the whole stack while one of a branch of it does; its row waits ([Next Step](next-step.spec.md)), and the old
  failure is in the line of the last day of the landing overview. Once that robot merged or failed, the failure is
  judged by content again. Rust decides it (`domain_policy::attention`), the view names each subject whose robot needs
  the user (`attention`, derived with every patch over the rows as they are, `derive::attention_of`), and the page lists
  what the view names. No robot of the cached frame of an earlier run needs the user (its table until the first sync of
  this run commits, its robots until Patronus answers this run): no move, no badge that a fresh answer takes back.
  [@test] ../../crates/domain-policy/src/attention_test.rs (`a_robot_failed_on_the_subject_as_it_is_only_on_its_tip_or_on_the_same_commits_rebuilt`)
  [@test] ../../crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_branch_needs_the_user_unless_something_else_holds_the_branch`)
  [@test] ../../crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands`)
  [@test] ../../crates/repository-model/src/derive_test.rs (`the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are`)
  [@test] ../../src/lib/robot-overview.test.ts (`asks nothing for a failure the view does not name, nor for one it names of another robot than the row's: it is finished history`)
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
  dot is, `h-6`, no padding, in the state column of the Status cell, at least `13ch` wide ("59m · ~1h 59m" fits, so
  its ticking time moves no column; "usually ~40m" past its usual time widens the column once) and at most 10rem; its text truncates. The elapsed time is `text-foreground`, the
  rest muted, in tabular figures. The Safe Push slot is the step column of the Actions cell, at most 10rem
  ([Branch columns](../../docs/design/ui-theme.md#branch-columns)); its step is a ghost `xs` button ("Restart Safe Push" with the `RotateCcwIcon`, the pull request with its
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
- "not in Safe Push" on a commit row is a `StatusChip` in `warning` without an icon, as "in origin/master" of a landed
  commit. Its details have the anatomy of the robot details: the state line with the `ShieldEllipsisIcon`, the title,
  the `dl` rows, the sentence in `text-xs text-muted-foreground`, and the actions as `outline` `xs` buttons.
- The details popover is `w-80`; in the preview the same details sit in a nested `rounded-lg` panel.
- "Safe Push" (`SafePushButton`) is an `xs` button with the `ShieldCheckIcon`, ghost in the row and the preview footer,
  outline among the popover's buttons, in the foreground colour (the push colour is the push button's), its label in
  tabular figures while a start runs ("Pushing 45%", a spinner for the shield); in a slot (`compact`) at least `min-w-26`
  ("Checking…"), and while a start runs at the width it had at rest, its push the spinner and "45%". Disabled, it keeps the focus and the
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
  10:00Z, where a robot a test starts starts too. E2E never calls Patronus: the test server runs the commands through
  the real service against a fake Patronus of each test repository ([E2E Testing](../../docs/design/e2e-testing.md#remote-services)).
  `POST /repositories/{id}/patronus` sets its robots (an empty `sourceRevision` means the tip of the robot's branch
  when Patronus answers), `…/patronus-details` their checks and problems, `…/patronus-profiles` the merge profiles of
  a start, `…/patronus-start-refusal` a refusal of every start with the message the app shows; a start lists a new
  running robot `started-<n>` on the commit its branch is at, a restart `restarted-<n>`, a cancel makes the robot
  canceling. `GET …/patronus` (`getPatronusLog`) returns how often the app listed, polled and
  read details, and the start requests, the restarts, the cancels and the status reads the fake got, in order.
  The readiness reads of the sessions run the real core against the fake's pre-check (`TestFactSources::readiness`, logged with
  their subjects and `recheck`), which answers per branch what `POST /repositories/{id}/review-answers` sets (with
  `mergeRequest` the Space merge request found open for every branch and the answers asked with its review), else by the code owners of the branch's paths
  (none without `OWNERSHIP` files). The `/patronus` comment goes to the fake GitHub, whose bot replies as `…/patronus-replies` says
  (👍 by default); a test asserts the comment GitHub got. A push of a Safe Push, a restart or a row goes to the remote
  of the test repository, a bare `.git/upstream.git`, and the test asserts what it has (`expectPushedToRemote`): a
  push held there shows its progress, a test reads what Patronus had done when it came, and a branch pushed before or
  moved by someone else is a push or a commit there behind the app's back
  ([The remote of a repository](../../docs/design/e2e-testing.md#the-remote-of-a-repository)).
  No test answers a Patronus write in the page (`routeCommand`, a fulfilling `page.route`): the registry test fails it,
  and the page answers of the reads are pinned and only go down (`ROUTED_IN_PAGE`).
  [@test] ../../crates/test-server/src/command_registry_test.rs (`no_e2e_test_answers_a_write_in_the_page`)
  [@test] ../../crates/test-server/src/command_registry_test.rs (`the_page_answers_of_the_e2e_tests_are_pinned`)
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
  the robot didn't test: its result is then only in the preview, the line of the last day of the landing overview and
  Activity.

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
