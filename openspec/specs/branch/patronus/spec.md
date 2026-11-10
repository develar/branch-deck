---
name: Patronus
description: Requirements for the Patronus status of the virtual branches (Safe Push, Safe Merge, Dry Run), read over the Patronus REST API with the token of the Safe Push CLI, with the least load on Patronus, shown live in the Safe Push slot of the branch row (its time and how long it usually takes, a progress line, the branches it lands, "Failing" once a required check failed) and as a chip in the branch preview and the landing overview; the toast and the OS notification when a robot merged a branch, with a fetch of the baseline, and when a required check of a running robot failed, with its cancel; starting a Safe Push or a Dry Run of a branch (from the row, the branch menu, the preview and the chip popover) with the code-owner review it needs (the approvals in the GitHub pull request; Space merge requests are deprecated), or from its GitHub pull request with a /patronus comment when the branches are reviewed on GitHub; restarting and canceling its robot, and no push of a branch a running Safe Push lands but its Push and Restart.
style: plain-1
targets:
  - src/lib/os-notification.ts
  - crates/domain-types/src/patronus.rs
  - crates/domain-policy/src/review.rs
  - crates/domain-policy/src/reviewer_groups.rs
  - crates/domain-policy/src/actions.rs
  - crates/patronus/src/actions.rs
  - crates/patronus/src/client.rs
  - crates/patronus/src/review.rs
  - crates/patronus/src/review_readiness.rs
  - crates/remote-facts/src/review_readiness.rs
  - crates/sync-core/src/robot_landing.rs
  - crates/object-cache/src/review_answer.rs
  - crates/object-cache/src/patronus_robots.rs
  - crates/bd-remote/src/robot_list.rs
  - crates/patronus/src/service.rs
  - crates/patronus/src/token.rs
  - crates/sync-core/src/patronus.rs
  - crates/remote-facts/src/patronus_command.rs
  - src/lib/pull-request.ts
  - src/presenters/pull-request-url.ts
  - crates/sync-core/src/remote_status.rs
  - crates/app-api/src/remote/patronus.rs
  - src-tauri/src/services.rs
  - crates/trial-check/src/landing_gate.rs
  - crates/domain-types/src/landing.rs
  - src/presenters/landing.ts
  - src/presenters/lock-wait.ts
  - src/presenters/operations.ts
  - crates/operations/src/registry_watch.rs
  - crates/remote-facts/src/patronus_review.rs
  - src/stores/patronus.ts
  - src/lib/patronus-failing.ts
  - crates/remote-facts/src/scheduler.rs
  - crates/remote-facts/src/scheduler_readiness.rs
  - crates/remote-facts/src/fact_sources.rs
  - crates/repository-model/src/facts.rs
  - src/lib/patronus.ts
  - crates/domain-policy/src/attention.rs
  - src/lib/review-commit.ts
  - src/lib/robot-coverage.ts
  - src/stores/robot-coverage.ts
  - src/components/commit/RobotCommitMarker.tsx
  - src/components/branch/PatronusChip.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/RobotProgress.tsx
  - src/components/branch/LandingSpanLabel.tsx
  - src/components/branch/LandedChip.tsx
  - src/hooks/useLandedIn.ts
  - src/components/branch/StackLanes.tsx
  - src/lib/robot-eta.ts
  - src/stores/robot-history.ts
  - src/stores/robot-details.ts
  - src/lib/patronus-icons.ts
  - src/hooks/useRobotTiming.ts
  - src/components/branch/ReviewCommitNotice.tsx
  - src/components/branch/SafePushPopover.tsx
  - src/hooks/useSafePush.ts
  - src/presenters/land.ts
  - src/presenters/outcome.ts
  - crates/operations/src/reconcile.rs
  - crates/operations/src/forget.rs
  - crates/operations/src/robot_start.rs
  - src/lib/robot-run.ts
  - src/lib/robot-items.ts
  - crates/operations/src/land_execute.rs
  - crates/operations/src/land_session.rs
  - crates/operations/src/engine.rs
  - crates/operations/src/pull_request.rs
  - crates/operations/src/run_frame.rs
  - crates/domain-policy/src/push.rs
  - crates/domain-policy/src/holds.rs
  - crates/domain-policy/src/land.rs
  - crates/domain-policy/src/readiness.rs
  - crates/domain-policy/src/refresh.rs
  - crates/domain-policy/src/reasons.rs
  - crates/domain-types/src/reason.rs
  - crates/repository-model/src/derive.rs
  - src/presenters/reasons.ts
  - crates/sync-core/src/target_conflict.rs
  - crates/git-ops/src/trial_rebase.rs
  - src/stores/safe-push.ts
  - crates/repository-model/src/push_guard.rs
  - crates/operations/src/push.rs
  - crates/operations/src/land.rs
  - crates/app-api/src/session/operations.rs
  - crates/operations/src/robot.rs
  - src/components/branch/CreatePullRequestDialog.tsx
  - src/lib/branch-rules.ts
  - crates/remote-facts/src/branch_rules.rs
  - crates/sync-core/src/patronus_start.rs
  - crates/domain-policy/src/robot_match.rs
  - src/lib/patronus-stack.ts
  - crates/operations/src/close.rs
  - src/presenters/news.ts
  - crates/domain-policy/src/landed.rs
  - crates/domain-policy/src/news.rs
  - src/lib/whole-stack.ts
  - crates/remote-facts/src/pull_request_writes.rs
  - src/hooks/usePatronusReviewRequirement.ts
  - src/hooks/useStackIndependence.ts
  - src/hooks/useStackActions.ts
  - src/hooks/useRobotDetails.ts
  - crates/remote-facts/src/scheduler_details.rs
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useClosePreview.ts
  - src/components/branch/SafePushButton.tsx
  - src/components/branch/BranchActions.tsx
  - src/components/shared/MenuItems.tsx
  - src/lib/patronus-merge-request.ts
  - src/lib/push-run.ts
  - src/components/branch/StatusChip.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchPreviewDialog.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/components/repository/PatronusStatusLine.tsx
  - src/components/repository/ReviewStatusLine.tsx
  - src/hooks/useReviewStatusRefresh.ts
  - src/hooks/useSessionFactStores.ts
  - src/stores/update-project.ts
  - crates/sync-core/src/baseline_fetch.rs
  - crates/operations/src/update.rs
  - src/model/intents.ts
  - src/presenters/actions.ts
  - src/components/ui/toast.tsx
  - src/lib/event-toast.ts
  - src/lib/robot-toast.ts
  - src/lib/landing-toast.ts
  - src/components/branch/BranchRowStatus.tsx
  - src/hooks/useRowRobot.ts
  - crates/domain-policy/src/admission.rs
  - crates/operations/src/registry.rs
  - crates/operations/src/witness.rs
  - crates/operations/src/events.rs
  - crates/operations/src/needs.rs
  - crates/operations/src/facts.rs
  - crates/operations/src/registry_file.rs
  - crates/repository-model/src/patronus_view.rs
  - crates/domain-types/src/operation.rs
  - crates/domain-types/src/robot_landing.rs
---

# Patronus

Status: Implemented
Date: 2026-10-16

## Purpose

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
  failed robot carries its builds, problems and failed tests ([Safe Push Failure Prompt](../safe-push-failure-prompt/spec.md)).
- Other push modes than Safe Push (rebase with autosquash) and Dry Run, cherry-picks into other branches: the web UI
  and the CLI do them. An emergency profile is offered only by a Safe Push without review ([below](#safe-push-without-review)),
  told in red; `brd safe-push` never takes one.
- Knowing before a start which profile skips the code-owner review: Patronus tells it only of an emergency profile
  (`isEmergency`) and, once a robot runs, in its options (`patronus.review.approval.check.skip`).

## Concepts

### Starting a Safe Push or a Dry Run

A start lands the work into master; the user who presses Safe Push on a row has chosen the piece (the branch and what
it is built on). So a start is an action, not a form: it asks only what a person decides (a profile, a running Safe
Push it cancels, approvals nobody can judge yet), and says it in a popover at the button that was pressed.

### The readiness of a request

A code owner approves the pull request of someone else in its review window ([Review
Requests](../../repository/review-requests/spec.md)); the approval of a code owner is what lets a Safe Push through. So
the session asks Patronus about the pull request a review window shows, as the robot started from it would decide.

## Requirements

### Requirement: Reading robots: The repository is the Space repository origin pushes to

The repository MUST be the Space repository `origin` pushes to (`git remote get-url --push origin`): SSH, scp-like and
HTTPS URLs of `git.jetbrains.team/<project>/<repository>`.

#### Scenario: SSH, scp-like and HTTPS URLs of Space
- **WHEN** the push URL of `origin` is an SSH, scp-like or HTTPS URL of `git.jetbrains.team/<project>/<repository>`
- **THEN** it names the Space repository of the robots
  [@test] crates/sync-core/src/patronus_test.rs (`parses_space_remote_urls`)

### Requirement: Reading robots: Another host is not on Space, without a request

Another host, a local path or no `origin` MUST be "not on Space" without a request; the answer MUST be cached for 10
minutes, so a poll starts no git process.

#### Scenario: A remote outside Space
- **WHEN** `origin` is on another host or a local path, or there is no `origin`
- **THEN** the repository is "not on Space" and no request is sent
  [@test] crates/sync-core/src/patronus_test.rs (`rejects_remote_urls_outside_space`)

#### Scenario: A poll within the cached answer
- **GIVEN** the answer was read less than 10 minutes ago
- **WHEN** a poll runs
- **THEN** it starts no git process

### Requirement: Reading robots: The token is the one of the Safe Push CLI

The token MUST be the one of the Safe Push CLI, `~/.patronus/code.jetbrains.team-identity-token.json` (`{name, token,
expires}`), else `$SPACE_TOKEN`.

#### Scenario: The token file of the CLI
- **WHEN** the token file of the Safe Push CLI exists
- **THEN** its token is sent
  [@test] crates/patronus/src/token_test.rs (`reads_the_token_of_the_safe_push_cli`)

#### Scenario: No token file
- **WHEN** there is no token file and `$SPACE_TOKEN` is set
- **THEN** the token of the environment variable is sent
  [@test] crates/patronus/src/token_test.rs (`falls_back_to_the_environment_variable`)

### Requirement: Reading robots: An expired token is never sent

An expired token MUST NOT be sent.

#### Scenario: The token expired
- **WHEN** the token is past its `expires`
- **THEN** it is never sent
  [@test] crates/patronus/src/token_test.rs (`never_sends_an_expired_token`)

### Requirement: Reading robots: Without a token nothing is sent

Without a token nothing MUST be sent, and the status MUST be "not logged in".

#### Scenario: No token
- **WHEN** there is no token
- **THEN** nothing is sent and the status is "not logged in"
  [@test] crates/patronus/src/service_test.rs (`sends_nothing_without_a_token`)

#### Scenario: An action without a token
- **WHEN** an action (a cancel) or the check of a push asks Patronus without a token
- **THEN** nothing is sent: the action tells how to log in, and the check sees no robots
  [@test] crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Requirement: Reading robots: A robot belongs to the virtual branch of its source branch

The robots MUST be read with `GET /app/rest/v1/robots?owner=me&repository=<name>&start=<time>`, newest first. A robot
MUST belong to the virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its source branch; a robot on a
temporary ref (a Safe Push of the IDE or the CLI) MUST belong to the branch whose local or pushed tip it took.

#### Scenario: The robots of the pushed virtual branches
- **WHEN** the robots are listed
- **THEN** they come newest first
- **AND** a robot whose source branch is `{prefix}/virtual/{name}` belongs to that virtual branch
  [@test] crates/patronus/src/service_test.rs (`lists_the_robots_newest_first_and_names_the_virtual_branch`)

#### Scenario: A robot on a temporary ref
- **WHEN** a robot runs on a temporary ref (a Safe Push of the IDE or the CLI)
- **THEN** it belongs to the branch whose local or pushed tip it took
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`)

### Requirement: Reading robots: A running robot wins over a finished one, then the newest

Of the robots of a branch, a running robot MUST win over a finished one, then the newest.

#### Scenario: Several robots of one branch
- **WHEN** a branch has a running and a finished robot, or several finished ones
- **THEN** its robot is the running one, else the newest
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`)

### Requirement: Reading robots: One rule matches the robots for `brd` and the session

`brd` and the session of the repository MUST match the robots with one rule (`domain_policy::robot_match`: one index
per list of robots); the session MUST name the robot of each row in its view (`robotId` of the Patronus part), so the
page matches none itself.

#### Scenario: The table of the session
- **WHEN** the session matches the robots of a list to its table
- **THEN** each branch and whole stack that has one gets its robot
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_table_gets_the_robot_of_each_branch_and_whole_stack_that_has_one`)

#### Scenario: The view names the robot of each row
- **WHEN** Patronus answered
- **THEN** the session derives the Patronus part of its view, `robotId` of each row included
  [@test] crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)

#### Scenario: The page matches no robot itself
- **WHEN** the page shows the robot of a row
- **THEN** it takes the robot the view of the session names, never one of its own matching
  [@test] src/stores/patronus.test.ts (`takes the robot of each row from the view of the session, which matched it, never from its own matching`)

### Requirement: Reading robots: A command names the repository only, Rust resolves the prefix

Every command of Patronus, of a push and of a pull request MUST name the repository only: Rust resolves the prefix
`{prefix}` of its branches for the command, as the sync does (`BranchPrefixes`, no git process while it knows the
repository's git prefix). The operations of the remote (the engine: the push, the start, the restart, the cancel, the
retarget) MUST take that prefix, never one from the page.

#### Scenario: A list, a start, a restart and a cancel through the real service
- **WHEN** the robots are listed, and a robot is started, restarted and canceled, by commands that name the repository
- **THEN** they run the real service with the prefix Rust resolved
  [@test] crates/test-server/src/remote_fixtures_test.rs (`the_robots_of_the_fixture_come_through_the_real_service`, `a_start_restart_and_cancel_run_the_real_service_against_the_fake`)

#### Scenario: A push of a stack
- **WHEN** a stack is pushed
- **THEN** the push takes the prefix Rust resolved and runs the processes of the core push
  [@test] crates/operations/src/push_test.rs (`a_push_of_a_stack_runs_the_processes_of_the_core_push`)

#### Scenario: The robots of the branches under the resolved prefix
- **WHEN** the robots of the fixture are listed for a repository
- **THEN** they are matched to the branches under the prefix Rust resolved
  [@test] crates/test-server/src/remote_fixtures_test.rs (`the_robots_of_the_fixture_come_through_the_real_service`)

### Requirement: Reading robots: The page keeps its stores by the prefix it shows

The page MUST keep its stores by the prefix it shows ([Settings](../../repository/settings/spec.md)).

#### Scenario: The prefix changes
- **WHEN** the page shows the branches of a prefix
- **THEN** its stores are kept by that prefix

### Requirement: Reading robots: The pushed tip comes with the remote status of the branch

The pushed tip MUST come with the remote status of the branch: the `rev-parse` that checks the remote branch exists
returns it, so it costs no extra git process.

#### Scenario: The remote status reads the pushed tip
- **WHEN** the remote status of a branch is read
- **THEN** the `rev-parse` that checks the remote branch exists returns its pushed tip, with no extra git process
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`test_rebased_branch_with_new_commit_is_not_fast_forward`)

### Requirement: Load on Patronus: The session lists and polls the robots, not the page

The session of the repository MUST list and poll the robots, not the page: one scheduler of the facts of the sessions
on the clock of the sessions (`remote_facts::scheduler`, by the rules of `domain_policy::refresh`), one list or poll in
flight per session, nothing for a session no window shows.

#### Scenario: A session no window shows
- **WHEN** no window watches a session, or the session is closed
- **THEN** the scheduler asks nothing for it
- **AND** a refresh of a closed session is answered with none
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_nothing_for_a_session_no_window_watches`, `a_closed_session_asks_nothing_and_answers_a_refresh_with_none`)

#### Scenario: One request in flight per session
- **WHEN** a list or a poll of a session is in flight
- **THEN** no other list or poll of that session starts

### Requirement: Load on Patronus: Each answer reaches the windows as a patch of the robots

Each answer MUST reach the windows as a patch of the robots by id (`facts.robots` of the view), which the page stores
(`applyRobotsFacts`). The refresh button and the list a `/patronus` comment needs after the bot's 👍 MUST ask the
session (`refresh_facts`) and wait for the patch of that list.

#### Scenario: The reads of the facts reach the windows
- **WHEN** the session reads the robots, or the refresh button asks it (`refresh_facts`)
- **THEN** the answer reaches the windows as a patch of the robots by id
- **AND** the scheduler hears what it asks by
  [@test] crates/repository-model/src/session_test.rs (`the_reads_of_the_facts_reach_the_windows_and_the_scheduler_hears_what_it_asks_by`)

### Requirement: Load on Patronus: The first list asks for 7 days, the next ones since the last

The first list (after the first sync of a session a window shows) and the refresh button MUST ask for the last 7 days;
every other list MUST ask only for the robots started since 2 minutes before the previous list started.

#### Scenario: The first list and the next ones
- **WHEN** the first list of a session runs, then another one
- **THEN** the first asks for the last 7 days
- **AND** the next asks only for the robots started since 2 minutes before the previous list started
  [@test] crates/remote-facts/src/scheduler_test.rs (`lists_the_last_days_first_then_only_the_robots_started_since_shortly_before_the_last_list`)

#### Scenario: The refresh button
- **WHEN** the user presses the refresh button
- **THEN** the list asks for the last 7 days
  [@test] crates/remote-facts/src/scheduler_test.rs (`lists_the_full_window_on_the_refresh_button_asked_by_the_user`)

### Requirement: Load on Patronus: A full list asked again within 10 s is answered from the last one

A full list asked again within 10 s (a second window of the repository) MUST be answered from the last one, unless
the robots changed since (a start, a restart, a cancel, a poll that saw a change).

#### Scenario: A full list asked again
- **WHEN** a full list is asked again within 10 s
- **THEN** it is answered from the last one
- **AND** an incremental list is never answered from a kept one
  [@test] crates/patronus/src/service_test.rs (`reuses_a_fresh_full_list_but_not_an_incremental_one`)

#### Scenario: A poll saw a change
- **WHEN** a poll saw a change since the kept full list
- **THEN** the kept list is dropped and the next full list is asked anew
  [@test] crates/patronus/src/service_test.rs (`a_poll_that_sees_a_change_drops_the_kept_full_list`)

#### Scenario: The robots changed while a full list was in flight
- **WHEN** the robots changed while a full list was in flight
- **THEN** that list is not kept
  [@test] crates/patronus/src/service_test.rs (`a_full_list_in_flight_when_the_robots_changed_is_not_kept`)

#### Scenario: A start, a restart or a cancel
- **WHEN** a robot is started, restarted or canceled
- **THEN** the kept full list is forgotten
  [@test] crates/patronus/src/actions_test.rs (`a_start_a_restart_and_a_cancel_forget_the_full_list`)

### Requirement: Load on Patronus: The list of a check before an action is asked anew every time

The list of a check before an action of the app's page (its push, a restart, a cancel) MUST be asked anew every time,
never answered from a kept one: a robot the IDE or `brd` started since the page's list must hold, and the list it
fetched answers the page's next list.

#### Scenario: A robot started elsewhere since the page's list
- **GIVEN** the IDE or `brd` started a robot since the page's list
- **WHEN** a check before an action of the page asks the robots
- **THEN** it asks Patronus, sees that robot and holds
- **AND** the list it fetched answers the page's next list
  [@test] crates/patronus/src/service_test.rs (`the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere`)

### Requirement: Load on Patronus: A start of a Safe Push or a Dry Run asks no list of its own

A start of a Safe Push or a Dry Run MUST ask no list of its own: it is an operation of the session (`execute`),
whose plan, witness, push and start read the robots the session holds, and whose robot goes into them at once.

#### Scenario: A start through the engine
- **WHEN** a Safe Push or a Dry Run starts through `execute`
- **THEN** it asks no list of its own
- **AND** its robot goes into the robots of the session at once
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_through_execute_asks_no_list_of_its_own`)

### Requirement: Load on Patronus: A sync lists at most every 5 minutes, a push never

A sync MUST list at most every 5 minutes, a focus when the robots are older than 2 minutes (and at least a minute after
the last request). A push MUST list nothing: whether the branch changed since a robot started is known locally.

#### Scenario: A sync, a focus and a push
- **WHEN** syncs, focuses and pushes follow each other
- **THEN** a sync lists at most every 5 minutes, a focus only when the robots are older than 2 minutes
- **AND** a push lists nothing
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_after_a_sync_every_five_minutes_at_most_on_focus_when_older_than_two_minutes_never_after_a_push`)

### Requirement: Load on Patronus: The refresh button and a `/patronus` comment always ask

The refresh button and the list after a `/patronus` comment MUST always ask, and once more after a list in flight
(which may have started before the robot); a sync or a focus during a list MUST join it.

#### Scenario: The list after a start
- **WHEN** a start needs its robot (the list after a `/patronus` comment)
- **THEN** the robots are listed at once, those started since 2 minutes before the last list
  [@test] crates/remote-facts/src/scheduler_test.rs (`lists_the_robot_a_start_needs_at_once_since_shortly_before_the_last_list`)

#### Scenario: A list is in flight
- **WHEN** the refresh button is pressed, or a start needs its robot, while a list is in flight
- **THEN** the list is asked once more after it
- **AND** a sync or a focus during a list joins it
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_refresh_button_and_a_start_during_a_list_ask_once_more_after_it`)

### Requirement: Load on Patronus: A repository not on Space is asked again only by the refresh button

A repository that answered "not on Space" MUST be asked again only by the refresh button.

#### Scenario: A repository that is not on Space
- **GIVEN** a repository answered "not on Space"
- **WHEN** a sync or a focus comes
- **THEN** it is not asked again until the refresh button
  [@test] crates/remote-facts/src/scheduler_test.rs (`does_not_ask_again_for_a_repository_that_is_not_on_space_until_the_refresh_button`)

### Requirement: Load on Patronus: The running robots are polled only while one of them runs

The running robots of the branches and the whole stacks, and the watched ones, MUST be polled with
`POST /app/rest/v1/robots/fetch-updates` (the known status of each; only the changed ones come back) only while one of
them runs.

#### Scenario: No robot runs
- **WHEN** no robot runs
- **THEN** nothing is polled
  [@test] crates/remote-facts/src/scheduler_test.rs (`polls_nothing_without_running_robots`)

#### Scenario: The running robots of the table and the watched ones
- **WHEN** robots of the table run, or watched ones
- **THEN** the poll asks about those robots with the known status of each
- **AND** only the changed ones come back
  [@test] crates/repository-model/src/facts_test.rs (`the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones`)
  [@test] crates/patronus/src/service_test.rs (`polls_only_running_robots_with_their_known_status`)

### Requirement: Load on Patronus: The poll runs every minute while a window is focused

The poll MUST run every minute while a window of the session is focused (the focus of the window's demand); while none
is, every 5 minutes and only while a robot that pushes (not a dry run) runs, so its merge is told without the window. A
focus MUST poll at once, unless the last poll is less than 50 s old.

#### Scenario: A focused window
- **WHEN** a window of the session is focused while a robot runs
- **THEN** the running robots are polled every minute
  [@test] crates/remote-facts/src/scheduler_test.rs (`polls_the_running_robots_of_the_branches_every_minute_while_the_window_is_focused`)

#### Scenario: A blur and a focus
- **WHEN** the window loses the focus, then gets it again
- **THEN** the poll every minute stops on the blur
- **AND** the focus polls at once, unless the last poll is less than 50 s old
  [@test] crates/remote-facts/src/scheduler_test.rs (`stops_on_blur_and_asks_at_once_on_focus`)

#### Scenario: No window is focused
- **WHEN** no window of the session is focused
- **THEN** a robot that pushes is polled every 5 minutes, so its merge is told without the window
- **AND** a dry run is not polled at all
  [@test] crates/remote-facts/src/scheduler_test.rs (`polls_a_robot_that_pushes_every_five_minutes_while_the_window_is_not_focused_a_dry_run_not_at_all`)

### Requirement: Load on Patronus: The session matches the polled robots to its table

The session MUST match the robots to the tips of its table as the rows do (`domain_policy::robot_match`), a push of the
session included; a robot a start, a restart or a cancel answered MUST go into the session at once
(`Sessions::write_robot`) and is polled from then on.

#### Scenario: A start answered
- **WHEN** a start, a restart or a cancel answers a robot
- **THEN** the robot goes into the session at once and is polled from then on

### Requirement: Load on Patronus: The running robots are asked whether they will fail

After each list and each poll the session answers, the running robots (not a canceling one) of the branches, the whole
stacks and the watched ones that are not known failing yet MUST be asked whether they will fail ([Failing](#failing)):
`GET /robots/{id}/problems` of each, in parallel, in one read of the session, through the request policy below.

#### Scenario: A list or a poll answered
- **WHEN** a list or a poll answers
- **THEN** the problems of each running robot of the table and the watched ones not known failing yet are asked
  [@test] crates/repository-model/src/facts_test.rs (`the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones`)

#### Scenario: A required check of a running robot failed
- **WHEN** the problems of a running robot say a required check failed
- **THEN** the robot is failing
  [@test] crates/patronus/src/actions_test.rs (`a_running_robot_whose_required_check_failed_is_failing`)

### Requirement: Load on Patronus: A robot known failing is not asked again

A robot known failing MUST NOT be asked again: it stays failing until it finishes. So a poll costs at most one small
request more per running robot, and none once each is known: a poll that changed nothing while every running robot is
known failing and the problems of the failed ones are known MUST be one request.

#### Scenario: An idle poll
- **GIVEN** every running robot is known failing and the problems of the failed ones are known
- **WHEN** a poll changes nothing
- **THEN** it is one request
- **AND** the problems are asked only for changed robots
  [@test] crates/remote-facts/src/scheduler_test.rs (`an_idle_poll_asks_one_request_and_problems_only_for_changed_robots`)

### Requirement: Load on Patronus: Only an answer asks about the problems

Only an answer MUST ask: a sync end or a focus whose list the policy holds back, and a list or a poll Patronus didn't
answer, ask nothing. One such question MUST run per session at a time.

#### Scenario: A sync end or a focus the list gate refuses
- **WHEN** a sync ends or a focus comes and the policy holds its list back
- **THEN** no problems are asked
- **AND** each poll still asks
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_after_an_answer_only_sync_ends_and_focus_events_the_list_gate_refuses_ask_nothing_each_poll_still_asks`)

#### Scenario: Patronus didn't answer
- **WHEN** a list or a poll Patronus didn't answer ends
- **THEN** it asks nothing

### Requirement: Load on Patronus: A rejected token is not sent again in the background

A token Patronus rejects (401) MUST NOT be sent again by a background request until the token file changes (the CLI
renewed it): Patronus checks every unknown token against Space. The refresh button SHALL still ask.

#### Scenario: Patronus rejected the token
- **GIVEN** Patronus rejected the token (401)
- **WHEN** a background request comes
- **THEN** the token is not sent until the token file changes
- **AND** the refresh button still asks
  [@test] crates/patronus/src/service_test.rs (`a_rejected_token_is_not_sent_in_the_background_until_it_changes`)

### Requirement: Load on Patronus: Background requests back off after a failure

After a server or network failure, background requests MUST back off: 1 s, doubled up to 10 minutes, reset by the next
answer.

#### Scenario: A server failure
- **WHEN** the server fails a background request
- **THEN** the next ones wait 1 s, doubled up to 10 minutes
- **AND** the next answer resets the wait
  [@test] crates/patronus/src/service_test.rs (`background_requests_back_off_after_a_server_failure`)

### Requirement: Load on Patronus: A GET is asked again once after a failure, a POST never

A GET MUST be asked again once after a failure of the server or the connection, never after a timeout, which already
waited its full time (it still backs off); a 4xx MUST NOT be asked again. A POST or a PUT MUST NOT be repeated by
itself.

#### Scenario: A list timed out
- **WHEN** a list timed out
- **THEN** it is not asked again
- **AND** the background requests still back off
  [@test] crates/patronus/src/service_test.rs (`a_list_that_timed_out_is_not_asked_again_and_backs_off`)

#### Scenario: A client error
- **WHEN** Patronus answers a GET with a 4xx
- **THEN** it is not asked again
  [@test] crates/patronus/src/service_test.rs (`a_client_error_is_not_asked_again`)

#### Scenario: A POST or a PUT failed
- **WHEN** a POST or a PUT fails
- **THEN** it is never repeated by itself

### Requirement: Load on Patronus: The progress of the running checks is asked only with open details

The progress of the running checks of a robot (`GET /teamcity-proxy/build-progress/{buildId}`, the estimate of the CI
for a build, as the robot page of the web UI shows it) MUST be asked only with the details of a running robot, while
they are open: the popover of the robot in the row, the Patronus section of the branch preview, or a running row of
the landing overview.

#### Scenario: The details of a running robot open
- **WHEN** the details of a running robot open
- **THEN** they have the estimate of its running checks
  [@test] crates/patronus/src/actions_test.rs (`details_of_a_running_robot_have_the_estimate_of_its_running_checks`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)

### Requirement: Load on Patronus: One progress request per running check, again once a minute

The progress MUST be asked with one request per running check, for the build of its current attempt (the running
attempt with the highest number), in parallel, after the details' checks; again once a minute while they stay open
(61 s on the clock of the session, as Patronus caches the progress for a minute: never faster), never after they
closed.

#### Scenario: The details stay open
- **WHEN** the details of a running robot stay open
- **THEN** the progress of each build is asked once a minute, never faster
- **AND** never for a finished robot, never after the details closed
  [@test] crates/patronus/src/actions_test.rs (`the_progress_of_a_build_is_asked_once_a_minute_and_never_for_a_finished_robot`)

### Requirement: Load on Patronus: The details are a fact of the session, read once for all windows

The details MUST be a fact of the session of the repository (`facts.robotDetails`): a window demands them while a
surface shows them (`robotDetails` of its demand), the session reads them by its scheduler and drops them once no
window demands them, so two windows ask them once; the backend caches the progress of each build for 60 s.

#### Scenario: Two windows show the details
- **WHEN** two windows of the session show the details of one robot
- **THEN** the session asks them once
- **AND** it drops them once no window demands them

### Requirement: Load on Patronus: No progress request comes from the row or a timer of the page

No request SHALL come from the row, the landing indicator, the Dock or a timer of the page. The request MUST go through
the request policy above (the token, the backoff, the user agent, a GET asked once more after a transient failure, the
log line "read the build progress" with its duration).

#### Scenario: The hover and the menu of a row
- **WHEN** the user hovers a row and opens its menu
- **THEN** the session is asked nothing
- **AND** it is told only the details of the robot whose popover opens
  [@test] tests/e2e/patronus/patronus.spec.ts (`should ask the session nothing for the hover and the menu of a row, and tell it only the details of the robot whose popover opens`)

### Requirement: Load on Patronus: The new Patronus API reads the progress in one request

With Settings → Landing, All repositories, "New Patronus API" (`appSettings.patronusNewApi`, off by default; Rust reads
it from the settings writer for each command, the page sends nothing), the same estimate MUST come from one request,
`GET /robots/progress?robotId=…`, which Patronus folds from the same cache of a minute.

#### Scenario: The new API is on
- **WHEN** the setting is on and a window demands the details of a running robot
- **THEN** the progress is one request of the robot
- **AND** the builds are asked only when the setting is off
  [@test] crates/patronus/src/actions_test.rs (`with_the_new_api_the_progress_of_a_running_robot_is_one_request_kept_for_a_minute`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`the_details_a_window_demands_ask_the_folded_progress_only_with_the_new_api_of_the_settings`)

### Requirement: Load on Patronus: The folded progress reads as the one from the builds

The folded answer MUST give the latest estimated finish over the running checks (`estimatedFinishAt`, the time left or
the overtime against now), the check that ends last and its fraction done, the queued checks (also a running check
whose build waits for an agent or for its next attempt), whether one probably hangs; a robot without a running or
waiting check has no progress. It MUST be kept for 60 s per robot, as a build's is.

#### Scenario: The folded progress of a running robot
- **WHEN** Patronus answers the folded progress
- **THEN** it reads as the progress from the builds: the time left or the overtime, the check that ends last, the queued
  checks, whether one probably hangs
  [@test] crates/patronus/src/actions_test.rs (`the_progress_patronus_folded_reads_as_the_one_from_the_builds`)

#### Scenario: The answer is kept
- **WHEN** the progress of a robot is asked again within 60 s
- **THEN** the kept answer is used
  [@test] crates/patronus/src/actions_test.rs (`with_the_new_api_the_progress_of_a_running_robot_is_one_request_kept_for_a_minute`)

#### Scenario: A robot whose checks have not run yet
- **WHEN** a robot has no running or waiting check, as before its checks run
- **THEN** it has no progress

### Requirement: Load on Patronus: Without the progress endpoint the builds are asked

The endpoint is behind the `robot.progress.enabled` flag of Patronus: a 404 MUST mean it is off for the user, then the
builds are asked as above and the endpoint rests for 10 minutes.

#### Scenario: Patronus answers 404
- **WHEN** the progress endpoint answers 404
- **THEN** the builds are asked as above
- **AND** the endpoint rests for 10 minutes
  [@test] crates/patronus/src/actions_test.rs (`while_patronus_has_no_progress_of_robots_the_builds_are_asked_and_the_endpoint_rests`)

### Requirement: Load on Patronus: `brd robot` reads the setting from the app's settings file

`brd robot` MUST read the setting from the app's settings file ([Agent CLI](../../../../docs/contributing/agent-cli.md)).

#### Scenario: The setting of the app
- **WHEN** `brd robot` runs
- **THEN** it reads the new Patronus API setting from the app's settings file
  [@test] crates/bd-core/src/context_test.rs (`the_new_patronus_api_setting_of_the_app_is_read_from_its_settings_file`)

### Requirement: Load on Patronus: One HTTP client with a kept-alive connection and its user agent

Patronus MUST be asked by one HTTP client with a kept-alive connection (connect timeout 5 s, list, poll and details 15 s,
the review pre-check 20 s, actions and merge requests 30 s) and the user agent `Branch Deck/<version> (<os>; <arch>)`,
so Patronus can tell its load apart.

#### Scenario: A request of Branch Deck
- **WHEN** Branch Deck asks Patronus
- **THEN** the request goes over the kept-alive connection of the one client, with the user agent
  `Branch Deck/<version> (<os>; <arch>)` and the timeout of its kind

### Requirement: Load on Patronus: `SAFEPUSH_PATRONUS_URL` points the client at a staging instance

`SAFEPUSH_PATRONUS_URL` (as for the CLI) SHALL point the client at a staging instance.

#### Scenario: The variable is set
- **WHEN** `SAFEPUSH_PATRONUS_URL` is set
- **THEN** the requests go to that instance

### Requirement: Load on Patronus: `brd status` keeps the user's robots in the object cache

`brd status` (`landed_by`, [Agent CLI](../../cli/agent-cli/spec.md#brd-status)) MUST ask Patronus by the same rules,
though every call is a process of its own: the user's robots are kept in the `patronus_robots` store of the object
cache (`robot_list.rs`), keyed by a hash of the server, the token, the Space repository and the branch prefix.

#### Scenario: Two calls of `brd status`
- **WHEN** `brd status` runs twice while the kept robots hold
- **THEN** it asks Patronus once
  [@test] crates/bd-remote/src/robot_list_test.rs (`status_asks_patronus_once_while_the_kept_robots_hold`)

### Requirement: Load on Patronus: A kept list holds a minute while a robot runs, else five

A kept list MUST hold for a minute while a robot of it runs and for five minutes while none does (only a running robot
can merge); then one list of the robots started since 2 minutes before the kept list was asked and one poll of the
running ones it knows (`fetch-updates`) are merged into it; the first list asks for the last 7 days.

#### Scenario: The kept list no longer holds
- **WHEN** the kept list is older than it holds
- **THEN** the new robots are listed and the running ones polled, and both are merged into it
  [@test] crates/bd-remote/src/robot_list_test.rs (`the_robots_are_kept_while_they_hold_then_the_new_ones_are_listed_and_the_running_ones_polled`)

### Requirement: Load on Patronus: A rejected token and a failure keep the robots of `brd`

A rejected token MUST NOT be sent again until it changes (another key); after a failure nothing MUST be asked for 1 s,
doubled up to 10 minutes, reset by the next answer, and the kept robots stay.

#### Scenario: A rejected token or a failure
- **WHEN** Patronus rejected the token, or a request failed
- **THEN** the requests are held
- **AND** the kept robots stay
  [@test] crates/bd-remote/src/robot_list_test.rs (`a_rejected_token_and_a_failure_hold_the_requests_and_keep_the_robots`)

### Requirement: Load on Patronus: The commands that decide on the robots list them fresh

`brd push`, `brd safe-push` and `brd robot` decide on the robots: they MUST list them fresh and keep the list.
`--no-cache` SHALL ask every time.

#### Scenario: A command that decides on the robots
- **WHEN** `brd push`, `brd safe-push` or `brd robot` runs
- **THEN** it lists the robots fresh and keeps the list

#### Scenario: Without the cache
- **WHEN** a command runs with `--no-cache`
- **THEN** it asks every time

### Requirement: Load on Patronus: An answer patches only the robots it changed

An answer MUST patch only the robots it changed, so the page keeps the objects of the others and only the chips of
changed robots render again.

#### Scenario: An answer that changed some robots or none
- **WHEN** an answer changed some robots, or none
- **THEN** the objects of the unchanged robots are kept, and the record when nothing changed
  [@test] crates/repository-model/src/facts_test.rs (`keeps_the_objects_of_unchanged_robots_and_the_record_when_nothing_changed`)

### Requirement: Load on Patronus: A failed answer keeps the robots shown

A failed answer MUST keep the robots shown and only change the availability (the fact `Stale` after robots were listed,
`Unavailable` before).

#### Scenario: Patronus can't answer
- **WHEN** Patronus can't answer
- **THEN** the robots shown stay
- **AND** the availability is `Stale` after robots were listed, `Unavailable` before
  [@test] crates/repository-model/src/facts_test.rs (`keeps_the_shown_robots_when_patronus_cant_answer`)

### Requirement: Load on Patronus: A list started before a write or a poll rolls neither back

Each robot MUST keep the number of the read or the write that wrote it: a list that started before a start or a poll
answers after them and rolls neither back.

#### Scenario: A list answers after a start or a poll
- **WHEN** a list that started before a start or a poll answers after them
- **THEN** their robot is not rolled back
  [@test] crates/repository-model/src/facts_test.rs (`a_list_started_before_a_write_or_a_poll_never_rolls_their_robot_back`)

#### Scenario: A poll after a list
- **WHEN** a poll answers after a list
- **THEN** the GitHub repository the list named is kept
  [@test] crates/repository-model/src/facts_test.rs (`keeps_the_github_repository_of_a_list_through_a_poll`)

### Requirement: Robot of a row: The Status cell shows the robot as what happens to the branch

The Status cell of a branch row MUST show the robot of the branch as what happens to the branch, in the first column of
the cell, where the status text is otherwise ([Branch Row](../branch-row/spec.md#status-cell), `BranchRowStatus`; Rust
decides it, `domain_policy::row_status`, the slot of the robot `next_step::slot`).

#### Scenario: A robot runs or cancels
- **WHEN** a robot of the branch runs (or cancels)
- **THEN** the cell shows a rotating ring, how long it runs and how long it has left ([Timing](#timing))
- **AND** "Canceling" while it cancels
  [@test] src/lib/patronus.test.ts (`is the live robot while it runs, also while it cancels`)

#### Scenario: A running robot that will fail
- **WHEN** a running robot will fail ([Failing](#failing))
- **THEN** the cell says "18m · Failing" instead of the time left, no use waiting for it

#### Scenario: A finished robot of the branch as it is
- **GIVEN** a finished robot of the branch as it is (the commit it took, or the same commits rebuilt on a newer
  baseline)
- **WHEN** the row shows
- **THEN** the cell shows its state in its hue, "Failed", "Needs review", "Merged", "Dry run passed", "Canceled"
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)

### Requirement: Robot of a row: As it is means the piece its Safe Push lands

A branch as it is MUST mean the piece its Safe Push lands: a stacked branch whose remote branch lacks new commits of a
branch it is built on (`ancestorCommitsAhead` of its remote status) changed since its robot, which tested the older
piece; its start and its restart MUST push it first ("Push and Restart").

#### Scenario: The parent of a stacked branch got a new commit
- **GIVEN** a branch it is built on got a new commit that the remote branch of the stacked branch lacks
- **WHEN** the remote status of the stacked branch is read
- **THEN** it counts the commits in `ancestorCommitsAhead`, and the branch changed since its robot
  [@test] crates/sync-core/src/detection_remote_status_test.rs (`a_child_whose_parent_got_a_new_commit_lacks_it_on_its_remote`)

#### Scenario: The restart of a robot that tested the older piece
- **WHEN** the branch changed since its robot
- **THEN** the robot names the change and the restart that tests the branch as it is, pushing it first ("Push and
  Restart")
  [@test] src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)

### Requirement: Robot of a row: A robot of an earlier state gives way to the git status

For a robot of an earlier state of the branch (the warning dot below, a merged robot included: the same name with new
work) the Status cell MUST show the git status of the branch as it is. The robot MUST stay in the branch preview and the
landing overview, with the notice of what changed.

#### Scenario: A finished robot of an earlier state
- **GIVEN** a finished robot of an earlier state of the branch, a merged one included
- **WHEN** the row shows
- **THEN** the cell shows the git status of the branch as it is
  [@test] crates/domain-policy/src/row_status_test.rs (`a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs`)

### Requirement: Robot of a row: A running robot is said first, a finished one after what landed it

A running robot MUST be said first, whatever else holds the branch: a block by its stack parent, a conflict, an error,
what landed it stay beside it as a mark that names them ([Branch Row](../branch-row/spec.md#status-cell)); a sync
running lays over it as it passes. A finished robot MUST come after what landed all of it, a failed sync, parked and a
block, and stays beside them when it failed.

#### Scenario: Other facts hold the branch
- **WHEN** a block by its stack parent, a conflict, an error or what landed it holds a branch whose robot runs
- **THEN** the running robot is said first and the others stay beside it as a mark that names them
- **AND** no fact of the row drops, and the node and the cell tell one story
  [@test] crates/domain-policy/src/row_status_test.rs (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`)

#### Scenario: The whole stack parked or landed
- **WHEN** a whole stack has a parked member, or merged Safe Pushes landed it
- **THEN** it takes the rows of its stack branch: its step says the parked member, or what landed it
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)

### Requirement: Robot of a row: The Safe Push slot follows the push button in a column of its own

The Actions cell MUST have the Safe Push slot after the push button ([Branch Row](../branch-row/spec.md#actions-cell)),
in a column of its own, so the push buttons, the steps and "⋯" line up down the table and its islands.

#### Scenario: The rows of a table and its islands
- **WHEN** the table shows rows with and without a step
- **THEN** the push buttons, the steps and "⋯" line up down the table and its islands

### Requirement: Robot of a row: The slot shows the next step of the branch

The slot MUST show the next step of the branch, the first row of the table of [Next Step](../next-step/spec.md) that
matches (`domain_policy::next_step`), from the robot, the pull request and the readiness Patronus answers
([below](#code-owner-review)), and every other "Safe Push" MUST be disabled with the same reason.

#### Scenario: No robot, a robot of an earlier state, a dry run
- **WHEN** the branch has no robot, one of an earlier state of the branch, or a dry run
- **THEN** the slot is "Safe Push" ([below](#starting-a-safe-push-or-a-dry-run))
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
  [@test] crates/domain-policy/src/next_step_test.rs (`row_11_safe_push_and_the_start_without_review_while_github_is_signed_out`)

#### Scenario: A failed or canceled robot of the branch as it is
- **WHEN** the robot of the branch as it is failed or was canceled
- **THEN** the slot is "Restart Safe Push", the action of its details ([below](#restart-and-cancel)), disabled with the
  reason where a start is not possible
- **AND** "Safe Push" where Patronus can't restart it (a Safe Push of a temporary ref)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)

#### Scenario: A robot that failed for missing code-owner approvals
- **WHEN** the robot failed for missing code-owner approvals
- **THEN** the slot is "Restart Safe Push" only once Patronus says they count in the pull request it was started from
- **AND** else "Open Pull Request" (or "Create Pull Request…") while nobody can tell yet, or the wait for the approvals
  [@test] src/presenters/next-step.test.ts (`rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart`)
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_2_and_3_restart_a_failed_robot_or_keep_the_pull_request_while_nobody_can_tell`)

#### Scenario: The wait for the approvals that count
- **WHEN** the branch waits for the approvals that count
- **THEN** the slot is "Awaiting Review" (whom on hover), "Changes Requested", "Request Review…", "Create Pull Request…";
  "Draft", "Retarget #7" (to master on hover)
- **AND** a stacked branch's by its own pull request

#### Scenario: Nothing to offer
- **WHEN** a robot runs, after a merge, for a landed branch and a branch a Safe Push holds
- **THEN** the slot shows nothing (Cancel is in "⋯", the details and the landing overview)
- **AND** both starts are disabled while the Status cell says why
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_offers_nothing_while_the_status_cell_says_why_and_disables_both_starts`)

### Requirement: Robot of a row: Nothing hovers into the slot

Nothing MUST hover in: the state MUST stay in the Status cell where the eye reads it, the step stays the step.

#### Scenario: A failed robot restarted from its row
- **WHEN** the user restarts a failed robot from its row
- **THEN** the state stays in the Status cell and the step in the slot
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart a failed robot from its row, the state staying in the Status cell`)

### Requirement: Robot of a row: No slot outside Space, an empty one for a branch that can't be pushed

A repository that is not on Space MUST have no slot; a branch that can't be pushed (an error, no commits) MUST have an
empty one.

#### Scenario: A repository that is not on Space
- **WHEN** the repository is not on Space
- **THEN** its rows have no Safe Push slot

#### Scenario: A branch that can't be pushed
- **WHEN** a branch can't be pushed (an error, no commits)
- **THEN** its slot is empty

### Requirement: Robot of a row: Each robot state has its hue

Running and canceling MUST be muted, failing `destructive`, failed `destructive`, failed for missing approvals "Needs
review" `warning`, a successful Safe Push "Merged" `info`, a successful dry run "Dry run passed" `success`, canceled
muted.

#### Scenario: The states of a robot
- **WHEN** a robot is running, canceling, failed, failed for missing approvals, successful or canceled
- **THEN** its state is named in its hue: a successful dry run "Dry run passed", a successful push "Merged"
  [@test] src/lib/patronus.test.ts (`names the state in its hue; a successful dry run passed, a successful push merged`)

#### Scenario: A running robot whose required check failed
- **WHEN** a required check of a running robot failed
- **THEN** it is named "Failing" in `destructive`, still a running one
  [@test] src/lib/patronus.test.ts (`names a running robot whose required check failed "Failing", still a running one`)

### Requirement: Robot of a row: The ring of a running robot is primary, of a failing one destructive

The ring of a running robot MUST be `primary`, as the spinner of a running push; of a failing one `destructive`, its
progress line too.

#### Scenario: A running and a failing robot
- **WHEN** a robot runs, or runs and will fail
- **THEN** its ring is `primary`, or `destructive` with its progress line for the failing one

### Requirement: Robot of a row: A robot gets a warning dot when the branch changed since its commit

A robot MUST get a `warning` dot when the branch changed since it took its commit: the commit is not the one the remote
branch is at now (`remoteTip`), or the branch is not up to date with the remote by content (work the remote lacks, or
remote commits the branch lacks, as the [remote status](../../../../docs/design/remote-status.md) counts them).

#### Scenario: The branch changed since the robot's commit
- **WHEN** the remote branch is at another commit than the robot's, or the branch is not up to date with it by content
- **THEN** the robot gets a `warning` dot
  [@test] src/lib/patronus.test.ts (`marks a robot whose branch changed since its commit, not one whose branch was only rebuilt, never a merged one`)
  [@test] src/lib/review-commit.test.ts (`compares with the remote branch and the remote status by content, not with the hash of the local tip`)

### Requirement: Robot of a row: No dot for a merged robot, the local tip or a rebuilt branch

A merged robot MUST get no dot. Neither does a robot of the local tip, nor one whose remote branch is at its commit
while the branch is up to date: every sync rebuilds the virtual branches on the newer baseline, so their hashes change
while their commits don't. Until the remote status of the branch is loaded, there MUST be no dot.

#### Scenario: A merged robot and a rebuilt branch
- **WHEN** the robot merged, or the branch was only rebuilt on a newer baseline
- **THEN** the robot gets no dot
  [@test] src/lib/patronus.test.ts (`marks a robot whose branch changed since its commit, not one whose branch was only rebuilt, never a merged one`)

#### Scenario: The robot of the local tip, or no remote status yet
- **WHEN** the robot took the local tip, or nothing says otherwise yet (the remote status of the branch is not loaded)
- **THEN** the branch is taken as it is, and the robot gets no dot
  [@test] src/lib/review-commit.test.ts (`takes the branch as it is for its own tip, and while nothing says otherwise`)

### Requirement: Robot of a row: In the row only a running robot keeps the dot

In the row only a running robot MUST keep the dot (a finished one gives way to the git status and "Safe Push"); the chip
of the preview and the overview SHALL have it.

#### Scenario: A finished robot with the dot
- **WHEN** a finished robot has the dot
- **THEN** the row shows the git status and "Safe Push"
- **AND** the chip of the preview and the overview shows the dot

### Requirement: Robot of a row: The accessible name is the mode, then the state

The accessible name of the robot MUST be the mode, then the state: "Safe Push: Failed"; for a running robot with its
time.

#### Scenario: A running robot
- **WHEN** a robot runs
- **THEN** its accessible name is "Safe Push: Running, 18 minutes, about 22 minutes left"
- **AND** past its usual time "…, 14 hours, longer than usual"

### Requirement: Robot of a row: Hovering the robot opens its details

Hovering the robot MUST open the details: the mode and the state with "Open in Patronus" as an icon at its end, the
name, the target branch, the commit, the start and finish times, the cancellation reason, the notice about the commit
(below) and the actions of the robot ([below](#restart-and-cancel)).

#### Scenario: The hover of a robot
- **WHEN** the user hovers the robot
- **THEN** the details show its mode and state with "Open in Patronus", its name, target branch, commit, times, the
  cancellation reason, the notice about the commit and its actions
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)

### Requirement: Robot of a row: The actions of the details stand in two rows

The actions of the details MUST stand in two rows: what decides the robot, the restart as the primary button and the
cancel; then "Copy Prompt for Agent" as a split button whose menu has "Copy Prompt with CI Details", and "Open Pull
Request #12".

#### Scenario: The actions of a robot
- **WHEN** the details of a robot with a pull request show
- **THEN** the first row holds the restart as the primary button and the cancel
- **AND** the second "Copy Prompt for Agent" with "Copy Prompt with CI Details" in its menu, and "Open Pull Request #12"

### Requirement: Robot of a row: A click on the robot opens the robot page

A click MUST open the robot page, leave the details as they are
([hover descriptions](../../../../docs/design/ui-theme.md)) and not toggle the row.

#### Scenario: A click on the robot
- **WHEN** the user clicks the robot
- **THEN** the robot page opens, the details stay as they are and the row does not toggle
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)

### Requirement: Robot of a row: With the dot the details say what changed and how to test it

With the dot, the details MUST say what changed and how to test the branch as it is, with "Restart Safe Push".

#### Scenario: The branch changed since the robot started
- **WHEN** the robot has the dot
- **THEN** the details say "Started on 3264f30f; the branch has changed since (now 88de421d): the result won't cover the newest commits. Restart Safe Push to test the current branch.", with "Restart Safe Push"
  [@test] src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should say when the branch changed since the robot started, and how to test the branch as it is`)

### Requirement: Robot of a row: The restart pushes first a change the remote lacks

When the remote lacks the change, the restart MUST push it first, under the same name
([below](#restart-and-cancel)).

#### Scenario: The remote lacks the change
- **WHEN** the user restarts a robot whose branch has a change the remote lacks
- **THEN** the restart pushes it first, under the same name, else it restarts on the remote
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)

### Requirement: Robot of a row: The advice of a running robot names its action as its button does

For a running robot of the branch the remote moved past, the advice MUST be "Cancel Safe Push, then Restart Safe Push
to test the current branch."; one of a branch changed here, "Restart Safe Push to test the current branch.": its push
cancels the robot first. The advice MUST name the action as its button does (`formatRobotRestartLabel`).

#### Scenario: A running robot the remote moved past
- **WHEN** the remote moved past a running robot of the branch
- **THEN** the details ask to cancel it, then restart
  [@test] src/lib/patronus.test.ts (`asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref`)

#### Scenario: A running robot of a branch changed here
- **WHEN** the branch of a running robot changed here
- **THEN** the details advise the restart, whose push cancels the robot first

### Requirement: Robot of a row: A robot that can't be restarted from here gets no advice

A robot that can't be restarted from here (a Safe Push of a temporary ref, "Needs review") MUST get no advice.

#### Scenario: A Safe Push of a temporary ref
- **WHEN** the robot is a Safe Push of a temporary ref
- **THEN** its details offer nothing
  [@test] src/lib/patronus.test.ts (`asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref`)

### Requirement: Robot of a row: A branch only rebuilt says quietly it tests the same commits

A branch only rebuilt on a newer baseline (the remote branch at the robot's commit, the branch up to date, other
hashes) MUST get no dot; the details MUST say quietly "Tests the same commits on an older master." ("Tested …" once it
finished).

#### Scenario: A branch only rebuilt on a newer baseline
- **WHEN** the remote branch is at the robot's commit and the branch is up to date, with other hashes
- **THEN** the robot gets no dot and the details say quietly "Tests the same commits on an older master."
- **AND** nothing for the branch as it is
  [@test] src/lib/patronus.test.ts (`says quietly that a robot tests the same commits on an older baseline, nothing for the branch as it is`)

#### Scenario: A finished robot of a rebuilt branch
- **WHEN** such a robot finished
- **THEN** the details say "Tested …"
  [@test] tests/e2e/patronus/patronus.spec.ts (`should say only quietly that a robot tested the same commits on an older baseline`)

### Requirement: Commits a running robot covers: The commits show which the running robot covers

The commits of a branch MUST show which of them the running robot that tests it covers: its own robot, else the one
that lands it with a branch built on it or with the whole stack (the ring and the landing span of the graph). A
finished robot MUST mark nothing; the [failure prompt](../safe-push-failure-prompt/spec.md) lists what it checked.

#### Scenario: A robot runs for the branch or lands it
- **WHEN** the branch's own robot runs, or one that lands it with a branch built on it or with the whole stack
- **THEN** its commits show which of them that robot covers

#### Scenario: A finished robot
- **WHEN** the robot of the branch finished
- **THEN** its commits mark nothing

### Requirement: Commits a running robot covers: The robot covers every commit of the branch as it took it

While the robot took the branch as it is, or the same commits on an older baseline, it MUST cover every commit.

#### Scenario: The robot took the branch as it is
- **WHEN** the robot took the branch as it is, or the same commits on an older baseline
- **THEN** it covers every commit
  [@test] src/lib/robot-coverage.test.ts (`covers every commit while the robot took the branch as it is, or the same commits on an older baseline`)

### Requirement: Commits a running robot covers: A changed branch asks the commit map of the robot

Once the branch changed since (the `warning` dot, [above](#robot-of-a-row)), the app MUST ask the commit map of the
robot (`get_robot_commit_map`, the map of the failure prompt), once per tip of the branch it tests, never during a
sync. Without the robot's commit, before the remote status loads, or without an answer, the rows MUST keep the plain
rail.

#### Scenario: The branch changed since the robot took it
- **WHEN** the branch changed since the robot took it
- **THEN** the commit map of the robot is needed
  [@test] src/lib/robot-coverage.test.ts (`needs the commit map once the branch changed since the robot took it`)

#### Scenario: The map is asked once per tip
- **WHEN** a running robot's branch changed since it started
- **THEN** its commit map is asked once per tip, and dropped once the robot covers the branch
- **AND** nothing is asked for a finished robot
  [@test] src/stores/patronus.test.ts (`asks the commit map of a running robot once its branch changed since it started, once per tip, and drops it once the robot covers the branch`, `asks nothing for a finished robot`)

#### Scenario: Nothing to tell
- **WHEN** the robot's commit, the tip or the remote status of a changed branch is not known, or the map has no answer
- **THEN** the rows keep the plain rail
  [@test] src/lib/robot-coverage.test.ts (`tells nothing without the robot's commit, the tip, or the remote status of a changed branch`)

#### Scenario: The commits by their local hash
- **WHEN** the map answers
- **THEN** the commits of the branch are taken by their local hash, and none while the robot's commit is not here
  [@test] src/lib/robot-coverage.test.ts (`takes the commits of the branch by their local hash, and none while the robot's commit is not here`)

### Requirement: Commits a running robot covers: The line of the robot runs over the commits it covers

On the rail of the branch, the line of the robot MUST run over the commits it covers: a 1.5px stroke in
`--bd-robot-line`, the colour of its ring and landing span, from the node of the branch while it covers the first
row, else from the newest covered commit, down to the last covered one. The dots of the covered commits MUST be in
that colour; a robot that will fail draws them `destructive`.

#### Scenario: Commits added since the robot started
- **WHEN** commits were added or changed since the robot started
- **THEN** the line of the robot runs over the commits it covers
- **AND** the newest covered commit below the added ones gets a dot in the colour of the line, also in a list that is
  oldest first, and the branch row keeps the plain line down from its node
  [@test] src/lib/robot-coverage.test.ts (`runs the line of the robot over the commits it covers and marks the ones added or changed since`, `finds the newest covered commit below the added ones also in a list that is oldest first`)
  [@test] src/lib/robot-coverage.test.ts (`gives the dots of the commits the robot took the colour of its line, and starts no line above a first row it lacks`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over the commits it took, and mark the one added after it started`)

#### Scenario: The robot covers every commit
- **WHEN** the robot covers every commit
- **THEN** the line runs from the top of the rail down to the last dot, every dot in its colour
  [@test] src/lib/robot-coverage.test.ts (`draws the line from the top of the rail down to the last dot while the robot covers every commit`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over every commit while it took the branch as it is`)

#### Scenario: The line starts at the node
- **WHEN** the robot covers the first expanded commit of the branch: its own, one a robot below lands (a side branch
  through the fork curve), or a branch of the whole stack
- **THEN** the line of the branch row from the node down is the robot's, one line from the ring into the rail, the ring,
  the line and the dots in one colour
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should start the line of the robot at the ring of its node, in the colour of the ring`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should run the line of the robot of a side branch from the fork through its node into its commits`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should start the line of the robot of the whole stack at the node of each expanded branch`)

#### Scenario: A robot that will fail
- **WHEN** a required check of the running robot failed
- **THEN** its line, its span in the graph and its dots are `destructive`, as its ring
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line and the dots of a robot that will fail in the colour of its ring`)
  [@test] src/lib/stack-lanes.test.ts (`marks the span of a robot that will fail, the lanes passing the rows between too, and leaves the others`)

### Requirement: Commits a running robot covers: A commit added or changed since gets a hollow dot

A commit added or changed since the robot started MUST get a hollow dot with a 1px `--color-warning` border, on the
plain line. The full list of a branch MUST draw it, in the table, a stack island and the Parked card alike.

#### Scenario: A commit added after the robot started
- **WHEN** a commit was added or changed since the robot started
- **THEN** it gets a hollow dot with a 1px `--color-warning` border, on the plain line
  [@test] src/lib/robot-coverage.test.ts (`runs the line of the robot over the commits it covers and marks the ones added or changed since`)

#### Scenario: A commit whose state is not known
- **WHEN** the state of a commit is not known
- **THEN** it keeps the plain dot
  [@test] src/lib/robot-coverage.test.ts (`keeps the plain dot for a commit whose state is not known`)

#### Scenario: The table, a stack island and the Parked card
- **WHEN** the full list of a branch shows in the table, a stack island or the Parked card
- **THEN** it draws the line and the dots alike

### Requirement: Commits a running robot covers: A commit the robot lacks says not in Safe Push

The row of such a commit in the full list MUST say "not in Safe Push" ("not in Dry Run") after its subject, a `warning`
chip. Its details MUST say "Added after the Safe Push started on 3264f30f" (or "Changed since…"), the robot and its
start, "The result won't cover this commit. Restart Safe Push to test the current branch." ("stack" for the robot of
the whole stack), and the actions of the robot as its own details name them.

#### Scenario: A commit added after the robot started
- **WHEN** a commit was added after the robot started
- **THEN** its row says "not in Safe Push" after its subject, a `warning` chip
- **AND** its details say "Added after the Safe Push started on 3264f30f", the robot and its start, and offer the
  actions of the robot
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line of the robot over the commits it took, and mark the one added after it started`)

### Requirement: Timing: The estimate from history costs no request

A running robot has two estimates. The one from history MUST always be there and cost nothing: no request is made for
it, it comes from the robots the app already listed and polled, and the polling stays as
[above](#load-on-patronus).

#### Scenario: A running robot with history
- **WHEN** a robot runs
- **THEN** its estimate from history comes from the robots the app already listed and polled, with no request of its
  own

### Requirement: Timing: The estimate of the CI is read only while the details are open

The estimate of the CI is precise but costs a request per running check: it MUST be read only while the details of the
robot are open ([above](#load-on-patronus)), and used only while they are.

#### Scenario: The details open and close
- **WHEN** the details of a running robot open, then close
- **THEN** the estimate of the CI is read and used only while they are open

### Requirement: Timing: The answers that name the merge profile carry it onto the robot

The merge profile decides the checks of a robot, and so how long it runs (a Large Scale Change runs for hours, the
default checks for minutes). A list doesn't name it; the answers that do MUST carry it onto the robot at no cost: a
poll (`fetch-updates` answers a robot with its options once its status changed, so every robot polled to its finish),
the details (`GET /robots/{id}`) and a start.

#### Scenario: A poll names the profile
- **WHEN** a poll answers a robot whose status changed
- **THEN** the robot has the merge profile of its options
  [@test] crates/patronus/src/service_test.rs (`polls_only_running_robots_with_their_known_status`)

#### Scenario: A start names the profile
- **WHEN** a robot starts
- **THEN** it has the profile it was started with, also before Patronus wrote it into the robot's options
  [@test] crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)

### Requirement: Timing: A later list keeps the merge profile of the robot

A later list MUST keep the merge profile: the robot merges, the profile isn't overwritten with "not known".

#### Scenario: A list after a poll, the details or the start named the profile
- **GIVEN** a poll, the details or the start named the merge profile of a robot
- **WHEN** a list, which never names it, answers the robot
- **THEN** the robot keeps the profile
  [@test] crates/repository-model/src/facts_test.rs (`keeps_the_merge_profile_a_poll_the_details_or_the_start_named_when_a_list_which_never_names_it_answers_the_robot`)

### Requirement: Timing: The main window keeps a history of the finished robots

The main window MUST keep a history of the finished robots it saw (`robotHistory` in `settings.json`, the robot, its
profile when known, the push mode, the target branch, the start, the finish and the status), at most 200 robots and
none that finished more than 30 days ago, so the estimate survives a restart and learns beyond the 7 days of a list.

#### Scenario: An answer with finished robots
- **WHEN** an answer names finished robots
- **THEN** they are added to the history, never a running one
- **AND** a profile an answer named since is learnt
  [@test] src/stores/robot-history.test.ts (`adds the finished robots of an answer, never a running one, and learns a profile an answer named since`)

#### Scenario: The limits of the history
- **WHEN** the history grows
- **THEN** it keeps at most 200 robots and none that finished more than 30 days ago, the newest first
  [@test] src/stores/robot-history.test.ts (`keeps at most 200 robots and none that finished more than 30 days ago, the newest first`)

#### Scenario: The history of a repository
- **WHEN** the estimates of a repository are made
- **THEN** the history gives back the robots of that repository as samples
  [@test] src/stores/robot-history.test.ts (`records the robots of a repository and gives them back as samples of its estimates`)

### Requirement: Timing: How long a robot usually runs is the median of its profile

How long a robot usually runs MUST be the median duration of the successful robots of its merge profile (those without
one for a robot without one) and push mode started in the last 30 days, of the listed robots and the history, when 3 or
more of them finished; only those into its target branch when there are 3 or more (`estimateRobotDuration`).

#### Scenario: Successful robots of the push mode
- **WHEN** successful robots of the push mode started in the last 30 days
- **THEN** the usual time is their median
  [@test] src/lib/robot-eta.test.ts (`takes the median of the successful robots of the push mode started in the last 30 days`)

#### Scenario: Enough robots of the merge profile
- **WHEN** three robots of the same merge profile finished
- **THEN** the median takes those, and those without one for a robot without one
  [@test] src/lib/robot-eta.test.ts (`takes the robots of the same merge profile once three of them finished, and those without one for a robot without one`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

#### Scenario: Enough robots into the same target
- **WHEN** there are 3 or more of them into the target branch of the robot
- **THEN** only those count
  [@test] src/lib/robot-eta.test.ts (`counts only the robots into the same target once there are enough of them`)

### Requirement: Timing: Without the profile the estimate is the rough one of the push mode

While the profile of the robot is not known, or fewer than 3 robots of it finished, the estimate MUST be the rough one
of its push mode. A failed robot stopped early and MUST NOT count. Without such a robot the run MUST be indeterminate.

#### Scenario: The profile is not known or too few of its robots finished
- **WHEN** the profile of the robot is not known, or fewer than 3 robots of it finished
- **THEN** the estimate falls back to the push mode, roughly
  [@test] src/lib/robot-eta.test.ts (`falls back to the push mode, roughly, while the profile is not known or too few of its robots finished`)

#### Scenario: No finished robot to learn from
- **WHEN** no successful robot finished
- **THEN** the run is indeterminate
  [@test] src/lib/robot-eta.test.ts (`is indeterminate without a finished robot to learn from`)

### Requirement: Timing: The row says how long the robot runs and how long it has left

The row and the lists of robots MUST say how long the robot runs and how long it has left: "18m · ~22m" (the time left
rounded up; "about 22 minutes left" in the hover and for the screen reader); past its usual time, the usual time
instead, "55m · usually ~40m" (the hours only from an hour: "14h · usually ~1h"); without history, "18m".

#### Scenario: A robot within its usual time
- **WHEN** a robot runs within its usual time
- **THEN** the row says "18m · ~22m", and "about 22 minutes left" in the hover and for the screen reader
  [@test] src/lib/robot-eta.test.ts (`says how long it runs and how long it usually takes`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)

#### Scenario: A robot past its usual time
- **WHEN** a robot runs longer than usual
- **THEN** the row names the usual time, "55m · usually ~40m", in hours once it runs an hour, "14h · usually ~1h"
  [@test] src/lib/robot-eta.test.ts (`names the usual time once a robot runs longer, in hours once it runs an hour`)

#### Scenario: The time left
- **WHEN** a robot runs within its usual duration, past it, or without history
- **THEN** the time left counts down until the usual duration, indeterminate past it, and the row says "18m" without
  history
  [@test] src/lib/robot-eta.test.ts (`counts the time left until the usual duration, indeterminate past it and without history`)

### Requirement: Timing: A sentence that names no elapsed time says left

A sentence that names no elapsed time MUST say "left": "Safe Push of deploy lands 5 branches · ~23m left". The time
MUST tick with the relative times, every 15 s, without a request.

#### Scenario: The sentence of a landing
- **WHEN** a sentence names the time of a robot without its elapsed time
- **THEN** it says "Safe Push of deploy lands 5 branches · ~23m left"

#### Scenario: The time ticks
- **WHEN** 15 s pass
- **THEN** the time ticks with the relative times, without a request

### Requirement: Timing: A thin line under the time shows the share of the usual time

A thin line under the time of a running robot in the Status cell MUST show the share of its usual time; without
history, past it and while it cancels, it MUST run across (indeterminate).

#### Scenario: The line of a running robot
- **WHEN** a robot runs with history, or without it, past its usual time or while it cancels
- **THEN** the line under its time shows the share of its usual time, else runs across
  [@test] tests/e2e/patronus/patronus.spec.ts (`should count the time of a running robot and the time it usually takes`)

### Requirement: Timing: The open details show the estimate of the CI in the row Progress

While the details of a running robot are open, they MUST show the estimate of the CI in the row "Progress": the time
left of the running check that ends last (its estimate minus its running time) and its name and percent, "about 9 min
left · Unit Tests 93%"; "1 check queued" when checks wait in the queue; "Overtime 4 min" when every running check ran
past its estimate; "probably hanging" in `warning` when the CI says so; no row while the CI tells none of it.

#### Scenario: The slowest check and the queued ones
- **WHEN** the CI estimates the running checks of the robot
- **THEN** the row "Progress" says "about 9 min left · Unit Tests 93%" for the check that ends last
- **AND** "1 check queued" when checks wait in the queue
  [@test] src/lib/robot-eta.test.ts (`names the time left, the check the robot waits for, and the queued checks`)

#### Scenario: Every running check ran past its estimate
- **WHEN** every running check ran past its estimate
- **THEN** the row says "Overtime 4 min"
  [@test] crates/patronus/src/actions_test.rs (`a_robot_ends_with_its_slowest_check_and_runs_over_when_every_check_did`)

#### Scenario: A check probably hangs, or the CI tells nothing
- **WHEN** the CI says a check probably hangs, or tells none of it
- **THEN** the row says "probably hanging" in `warning`, or there is no row

### Requirement: Timing: The time of the row counts down what the open details read

While the details are open, the time and the progress line of the row MUST count down the time left it read meanwhile
("20m · ~9m left"), and the start line of the details keeps only the usual time ("usually ~1 h 50 min"); once the
details closed, the time and the line MUST take the estimate from history again. A running robot row of the landing
overview MUST show the check and the queued checks too.

#### Scenario: The details are open
- **WHEN** the details read the estimate of the CI
- **THEN** the slot counts down the time left it read, "20m · ~9m left"
  [@test] src/lib/robot-eta.test.ts (`counts down the time left it read while the details were open, for the slot`)

#### Scenario: The details close
- **WHEN** the details close
- **THEN** the time and the line take the estimate from history again
  [@test] tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)

### Requirement: Timing: The details say what the estimate is of

The details of a running robot MUST say the estimate after the start time, with what the estimate is of; the slot
doesn't name it. The details MUST have the profile row when the profile is known, also before the details answered,
and the start line names no profile that row names; a robot row of the landing overview names it on hover of its
time: "Safe Push · Large Scale Change".

#### Scenario: The estimates of the details
- **WHEN** the details of a running robot show
- **THEN** the start line says "Today 14:02 · usually ~1 h 50 min for Large Scale Change · about 1 h 32 min left",
  "… without a merge profile …", the rough "… · usually ~40 min for Safe Push · 55 min, longer than usual", or "… ·
  running 18 min · no Safe Push finished in the last 30 days to estimate from"
  [@test] src/lib/robot-eta.test.ts (`explains the estimate and what it is of in the details`)

#### Scenario: The profile row names the profile
- **WHEN** the profile of the robot is known, also before the details answered
- **THEN** the details have the profile row, and the start line says "Today 14:02 · usually ~1 h 50 min · about 1 h 32
  min left"
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`, `should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it`)

#### Scenario: A robot row of the landing overview
- **WHEN** the user hovers the time of a robot row of the landing overview
- **THEN** it names the profile, "Safe Push · Large Scale Change"

### Requirement: Chip: The robot shows as a chip outside the row

The branch preview, the landing overview and the Landed island MUST show the robot as a chip, the same text-sized
button as the pull request chip (`StatusChip`): the shield and the state in its hue (the ring while it runs), the
warning dot, the details on hover, the robot on click. A branch without a robot MUST show nothing.

#### Scenario: The robot in the branch preview
- **WHEN** the branch preview shows a branch with a robot
- **THEN** the chip shows the shield and the state in its hue, the ring while it runs, and the warning dot
- **AND** its hover shows the details and its click opens the robot
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

#### Scenario: A branch without a robot
- **WHEN** a branch has no robot
- **THEN** no chip shows

### Requirement: Details: The open popover demands the details of the robot of the session

While the popover is open the window MUST demand the details of the robot of the session (the demand goes out at most
four times a second): the merge profile, the TeamCity checks ("1 failing (Fast Tests), 9 passed") and the problems of
severity error.

#### Scenario: Two windows demand details
- **WHEN** the windows of a session open and close popovers
- **THEN** the demand of the session is the union of its windows and goes with each
  [@test] crates/repository-model/src/session_test.rs (`the_demand_of_a_session_is_the_union_of_its_windows_and_goes_with_each`)

#### Scenario: The popover shows the details
- **WHEN** the popover of a robot is open and Patronus answered
- **THEN** it shows the profile, the checks ("1 failing (Fast Tests), 9 passed") and the problems of severity error
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Requirement: Details: The details are three requests in parallel, cached by the backend

The details MUST be `GET /robots/{id}`, `/teamcity-checks` and `/problems` (every severity: the errors are shown, the
warning `robot_is_failing` marks the robot failing, [below](#failing)) in parallel, cached by the backend: a finished
robot for the session, a running one for a minute.

#### Scenario: The details of a robot
- **WHEN** the details of a robot are read
- **THEN** they have the profile, the checks and the error problems
  [@test] crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)

#### Scenario: The warning that the robot is failing
- **WHEN** the problems of a running robot hold the warning `robot_is_failing`
- **THEN** the robot is marked failing ([below](#failing))
  [@test] crates/patronus/src/client.rs (`a_failed_check_of_a_running_robot_reads_as_patronus_answers_it`)

#### Scenario: The details of a finished robot
- **WHEN** the details of a finished robot are demanded again
- **THEN** they are asked once
- **AND** its problems are its errors
  [@test] crates/patronus/src/actions_test.rs (`details_of_a_finished_robot_are_asked_once`)
  [@test] crates/patronus/src/actions_test.rs (`the_problems_of_a_finished_robot_are_its_errors`)

### Requirement: Details: The session reads the details at once, and every 61 s of a running robot

The session MUST read the details at once, again when a poll changed the robot's status, and of a running robot every
61 s while they are demanded.

#### Scenario: A window shows the details
- **WHEN** a window shows the details of a robot
- **THEN** the session reads them at once
- **AND** of a running robot every minute while they are shown
  [@test] crates/remote-facts/src/scheduler_test.rs (`reads_the_details_a_window_shows_at_once_and_of_a_running_robot_every_minute_while_shown`)

### Requirement: Details: What the details name beyond the list goes onto the robot

What the details name beyond the list MUST go onto the robot of the session: its merge profile (when the robot has
none) and that it will fail (while it runs).

#### Scenario: The details of a demanded robot
- **WHEN** the details of a demanded robot name its profile
- **THEN** the robot of the session has that profile
- **AND** the details go with its demand
  [@test] crates/repository-model/src/facts_test.rs (`the_details_of_a_demanded_robot_name_its_profile_and_go_with_its_demand`)

#### Scenario: A running robot that will fail
- **WHEN** the details of a running robot say that it will fail
- **THEN** the robot of the session is failing while it runs

### Requirement: Details: A check is named as Patronus names it in the profile

A check MUST be named as Patronus names it in the profile ("IntelliJ Aggregator"), its build configuration
("Aggregator") only without that name.

#### Scenario: A check with and without a name in the profile
- **WHEN** the profile names a check "IntelliJ Aggregator", or names none for the build configuration "Aggregator"
- **THEN** the check is named "IntelliJ Aggregator", or "Aggregator"
  [@test] crates/patronus/src/client.rs (`a_failed_check_of_a_running_robot_reads_as_patronus_answers_it`)

### Requirement: Details: Of a running robot the progress of its running checks follows

Of a running robot, the progress of its running checks MUST follow ([Load on Patronus](#load-on-patronus)).

#### Scenario: The details of a running robot
- **WHEN** the details of a running robot are read
- **THEN** the progress of its running checks is read after them

### Requirement: Details: When Patronus can't answer the popover shows the list data

When Patronus can't answer, the popover MUST show the list data.

#### Scenario: Patronus can't answer the details
- **WHEN** Patronus can't answer the details
- **THEN** the popover shows what the list said of the robot

### Requirement: Starting a Safe Push or a Dry Run: Safe Push starts from six places and from S

When the repository is on Space (Patronus answered), "Safe Push" SHALL start from six places, and from S on the
focused row of the branch table ([Branch Row](../branch-row/spec.md#keyboard)) and in the preview, which the hover of
the row button and the menu item show.

#### Scenario: The six places of a start
- **GIVEN** the repository is on Space (Patronus answered)
- **WHEN** the user looks for the start of a branch
- **THEN** "Safe Push" starts from the Safe Push slot of the row ([above](#robot-of-a-row)), the Patronus group of the
  branch menu (the right-click and "⋯", with "Safe Push With Profile…" and "Dry Run"), the preview footer, the popover
  of a robot that needs review, the review window and the toast of an approval ([Next Step](../next-step/spec.md))
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)

#### Scenario: S on the focused row
- **WHEN** the user presses S on the focused row of the branch table or in the preview
- **THEN** the start of its branch is asked
- **AND** the hover of the row button and the menu item show S
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)

### Requirement: Starting a Safe Push or a Dry Run: One function asks the start everywhere

One function MUST ask the start everywhere (`requestSafePush`): the row is revealed first (a collapsed stack or
island), the preview closes first (`useClosePreview`), and a click on the row button MUST NOT toggle the row.

#### Scenario: A click on the row button
- **WHEN** the user clicks the Safe Push button of a row
- **THEN** the start is asked and the row doesn't toggle
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button without toggling the row`)

#### Scenario: A start from the preview
- **WHEN** the start is asked from the Patronus section of the branch preview
- **THEN** the preview closes first
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should close the branch preview for the Safe Push of its Patronus section`)

#### Scenario: A row in a collapsed stack or island
- **WHEN** the start is asked for a row of a collapsed stack or island
- **THEN** the row is revealed first

### Requirement: Starting a Safe Push or a Dry Run: The name of the start says what it lands

The name of the start MUST say what it lands, without "…": it asks nothing unless it has something to say. Its hover
SHALL say what lands into master (`describeStart`).

#### Scenario: A branch alone and a stacked branch
- **WHEN** the start of a branch is shown
- **THEN** it is named "Safe Push", and "Safe Push · 3" for a stacked branch that lands with the two branches it is
  built on (`formatStartLabel`)
- **AND** its hover says "Lands auth into master once the checks pass", "Lands oauth with the 2 branches it is built on
  into master in one Safe Push"
  [@test] src/presenters/next-step.test.ts (`row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request`)

### Requirement: Starting a Safe Push or a Dry Run: Every entry point asks the next step first

Every entry point MUST ask one predicate first, the next step of the branch (the `safe-push` action, [Next
Step](../next-step/spec.md)), which asks the start refusal Rust decides first (`domain_policy::land::start_refusal`,
the view's `landing.start` and `landing.dryRun`, worded by `src/presenters/reasons.ts`).

#### Scenario: The reason of a refusal is worded as Rust wrote it
- **WHEN** the start refusal Rust decided names a reason
- **THEN** every surface words it from the catalog of reasons
  [@test] src/presenters/reasons.test.ts (`words every reason of the catalog Rust writes as the page says it`)

### Requirement: Starting a Safe Push or a Dry Run: No start while a robot runs on the last commit

A start MUST NOT be possible while a robot runs on the commit the branch is at, nor while it runs on the remote branch
that has the same commits by content, only rebuilt since on a newer baseline (`domain_policy::land::on_last_commit`,
the remote branch up to date by content): a start would test and land them again.

#### Scenario: A robot runs on the commit the branch is at
- **WHEN** a robot runs on the commit the branch is at
- **THEN** a start is not possible: Patronus would refuse it: "A Safe Push of the last commit is running"
  [@test] crates/domain-policy/src/land_test.rs (`a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_finished_robot_on_the_last_commit_blocks_no_start_a_canceling_one_still_does`, `the_tip_of_a_branch_the_sync_did_not_build_is_no_build_of_its_commits`)

#### Scenario: A robot runs on the remote branch rebuilt since on a newer baseline
- **WHEN** a robot runs on the remote branch that has the same commits by content, only rebuilt since on a newer
  baseline
- **THEN** a start is not possible, in the same words
  [@test] crates/domain-policy/src/land_test.rs (`a_robot_on_the_last_commit_is_one_that_runs_on_the_branch_as_it_is`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`)

### Requirement: Starting a Safe Push or a Dry Run: No start of a blocked stacked branch

A start MUST NOT be possible for a blocked stacked branch, in the push button's words.

#### Scenario: A stacked branch is blocked
- **WHEN** a stacked branch is blocked by a branch it is built on
- **THEN** a start is not possible: "Blocked by auth: fix it first, then sync"
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor`)

### Requirement: Starting a Safe Push or a Dry Run: A Safe Push waits for a robot that lands an ancestor

A Safe Push of a stacked branch MUST also wait while a robot that pushes into the target (not a dry run) runs on an
ancestor (`getLandingAncestor`, the `landingAncestor` of the repository view): the branch carries the ancestor's
commits, two robots would land them. It MUST wait also on a side branch built on an ancestor, which lands that ancestor.

#### Scenario: A robot runs on an ancestor
- **WHEN** a robot that pushes into the target runs on an ancestor of the branch
- **THEN** its Safe Push waits: "A Safe Push of login is running: wait until it merges"
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_branch_waits_for_a_robot_that_lands_an_ancestor_not_for_a_dry_run`, `a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should wait for the Safe Push of an ancestor, which lands its commits`)

#### Scenario: A side branch built on an ancestor
- **WHEN** a robot runs on a branch built on an ancestor of a side branch, which lands that ancestor
- **THEN** the side branch waits too: side-fix built on bazel waits for the Safe Push of runtime, built on bazel too
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

### Requirement: Starting a Safe Push or a Dry Run: A Safe Push waits for a robot that lands its commits

A Safe Push MUST wait while a robot that pushes runs on a branch built on it (`getLandingSpan`, the `span` of the view)
whose commit has commits of the branch, which lands them; this reason comes before the ancestor's. A dry run MUST wait
for neither this nor an ancestor's robot.

#### Scenario: A robot of a branch built on it lands its commits
- **WHEN** a robot that pushes runs on a branch built on it whose commit has commits of the branch
- **THEN** its Safe Push waits: "A Safe Push of oauth is running: it lands 1 commit of login with it", the reason
  before the ancestor's (what a robot lands, [below](#pushing-into-a-branch-whose-safe-push-runs))
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`, `the_robot_of_the_whole_stack_lands_every_branch_of_it`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`)

#### Scenario: A dry run waits for neither
- **WHEN** a dry run is asked while such a robot runs
- **THEN** it does not wait for it
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_of_a_parent_restacked_below_a_running_safe_push_goes_on`)

### Requirement: Starting a Safe Push or a Dry Run: `brd safe-push` asks the same start rule

`brd safe-push` MUST ask the same rule (`repository_model::patronus_view`, its `start` refusal, `took_as_is` with the
remote facts of an up-to-date remote status).

#### Scenario: The app and `brd` answer alike
- **WHEN** `brd safe-push` and the app decide a start over the same facts
- **THEN** they answer alike
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Starting a Safe Push or a Dry Run: A start not possible disables its button and items

When a start is not possible, the button and the menu items MUST be disabled, and their hover SHALL say the reason. The
row button MUST be disabled while the row runs an inline action or a start of the branch runs.

#### Scenario: The hover says why a start is not possible
- **WHEN** a start is not possible
- **THEN** the button and the menu items are disabled, and their hover says the reason
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)

#### Scenario: The row runs an inline action or a start
- **WHEN** the row runs an inline action or a start of the branch runs
- **THEN** the row button is disabled

### Requirement: Starting a Safe Push or a Dry Run: The next step disables a Safe Push Patronus would fail

After that the next step MUST disable a Safe Push that Patronus would fail (the approvals that count missing, a draft, a
pull request into another base, landed or merged), never a dry run.

#### Scenario: Patronus would fail the Safe Push
- **WHEN** the approvals that count are missing, the pull request is a draft or into another base, landed or merged
- **THEN** the Safe Push is disabled
- **AND** a dry run is not
  [@test] crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`, `the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`, `a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

### Requirement: Starting a Safe Push or a Dry Run: A start asked anyway says the reason in an info toast

A start asked anyway (S, a toast, the review window) MUST say the reason in an `info` toast, which opens what the reason
names: the robot that runs on the branch or lands it, the pull request of a draft (`LandPrompt.opens`); missing
approvals MUST open the popover, whose review line names them and where they count.

#### Scenario: The toast of a start that can't run
- **WHEN** a disabled start is asked anyway
- **THEN** an `info` toast says the reason ("Safe Push of auth waits for Ilya Korennoy", "Dry Run of auth can't start")
- **AND** it opens the robot that runs on the branch or lands it ("Open in Patronus"), the pull request of a draft
  ("Open Pull Request")
  [@test] src/presenters/land.test.ts (`refuses with the words of the first blocker, offering the push or the object it names: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`)

#### Scenario: Missing approvals
- **WHEN** a start is asked anyway while the approvals are missing
- **THEN** the popover opens, its review line naming them and where they count
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

### Requirement: Starting a Safe Push or a Dry Run: One decision of Rust says what the start does

One decision, Rust's (`domain_policy::land::decide`, the rule `brd safe-push` decides by), MUST say what the start does,
over the facts the session holds: the table and the robots, the merge profiles, the remote statuses, the holds, the
pull requests, the review as Patronus answered it for the branch as it is, the rules of the repository, the landing
needs. A stale answer of the review MUST NOT be acted on.

#### Scenario: The decision over the facts of the session
- **WHEN** a start is decided
- **THEN** the decision reads the facts the session holds, never the page's
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`, `the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Starting a Safe Push or a Dry Run: A start and its popover ask the session for the plan

A start and its popover MUST ask the session for the plan of their intent (`prepare` of a `Land`, `operations::land`:
the decision with the profiles it offers and the fingerprint of what it was decided on; a read: no lock, no effect), the
popover again with each patch of the view. `brd safe-push` SHALL plan its start with the same function over what it read.

#### Scenario: The plan of a start
- **WHEN** a start or its popover asks for its plan
- **THEN** the session plans it over what it holds, with the fingerprint of the intent and the tips it was decided on
- **AND** the plan of a session waits for its Patronus view
  [@test] crates/operations/src/land_test.rs (`a_start_is_planned_over_what_the_session_holds_and_reads_the_review_its_first_decision_waits_for`, `the_fingerprint_of_a_plan_changes_with_its_intent_and_the_tips_it_was_decided_on`, `the_plan_of_a_session_waits_for_its_patronus_view`)

### Requirement: Starting a Safe Push or a Dry Run: The view carries the decisions of the default starts

The view MUST carry the decisions of the default starts of every branch and whole stack (`RepositoryView.lands`: a Safe
Push with the default profile, a Dry Run), which the actions and the first check of a click read.

#### Scenario: A click with nothing to say
- **WHEN** Rust decided that the default start has nothing to say
- **THEN** the click starts at once, and before that decision it waits for it
  [@test] src/presenters/land.test.ts (`starts at once when Rust decided there is nothing to say, and waits for its decision before it`)

### Requirement: Starting a Safe Push or a Dry Run: The page presents the plan and decides nothing

The page MUST present the plan (`presentLand`) and decide nothing: the next step's reason first, then what Rust refuses,
waits for or confirms.

#### Scenario: The reason of the next step first
- **WHEN** the next step has a reason and Rust refuses, waits for or confirms too
- **THEN** the page says the next step's reason first, the reason every surface says
  [@test] src/presenters/land.test.ts (`refuses with the reason of the next step first, the reason every surface says`)

### Requirement: Starting a Safe Push or a Dry Run: Each fact without an answer is a wait that names it

Each fact without an answer MUST be a wait that names it, never a refusal.

#### Scenario: The waits of a start
- **WHEN** a fact the start needs has no answer yet
- **THEN** the start waits, saying for what: "Asking Patronus for the merge profiles…", "Checking auth…", "Checking the
  code-owner review…", "Checking whether the push keeps the approvals of #7…", "Checking what auth needs to land…"
  [@test] src/presenters/land.test.ts (`waits for the facts Rust waits for, each saying what it waits for, and refuses with why a read failed`)
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)

### Requirement: Starting a Safe Push or a Dry Run: What Patronus would refuse or fail is a refusal

What Patronus would refuse or fail MUST be a refusal with the reason: a target Patronus does not push into, a draft,
missing approvals (also for a repository only on Space whose merge request misses them), a hold of the push, a
push that would drop commits of the remote branch the branch lacks.

#### Scenario: The refusals of a start
- **WHEN** Patronus would refuse or fail the start
- **THEN** it is refused with the reason: "Patronus does not push into master", "#7 is a draft: Patronus merges it once
  it is ready for review", "Patronus checks the approvals when the robot starts and fails without them", "The remote
  branch of auth has a commit that auth lacks: push it from its row, which shows it"
  [@test] crates/domain-policy/src/land_test.rs (`the_table_refuses_first_then_the_target_then_a_push_that_drops_remote_commits_unless_allowed`, `missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`, `the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)
  [@test] src/presenters/land.test.ts (`refuses with the words of the first blocker, offering the push or the object it names: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones`)

### Requirement: Starting a Safe Push or a Dry Run: A start confirms only for what a person decides

A start that can run MUST either start or confirm first, and MUST confirm only for what a person decides
(`Confirmation`). S, "Safe Push With Profile…" and the start without review MUST always confirm.

#### Scenario: A start that lands more or replaces a robot
- **WHEN** a start lands more than its row (a stacked branch), runs another profile than the default checks (the
  branch's own, the profile of its last robot, or one chosen), or cancels the Safe Push running on an older commit
- **THEN** it confirms first
  [@test] src/presenters/land.test.ts (`opens the popover for a key and for each confirmation of the decision: a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open`)

#### Scenario: Approvals nobody can judge before the start
- **WHEN** Patronus can't judge the approvals before the start (`unknown`, GitHub doesn't list the pull requests) or
  couldn't answer the review (`unavailable`, `ReviewUnknown`)
- **THEN** it confirms first
  [@test] crates/domain-policy/src/land_test.rs (`missing_approvals_refuse_unless_skipped_and_approvals_judged_at_the_start_confirm`)

#### Scenario: A push that dismisses approvals, unread rules, a start without review
- **WHEN** the push dismisses approvals Patronus doesn't count, the rules of the repository can't be read for a push
  into an approved pull request, or it is a start without review
- **THEN** it confirms first
  [@test] src/presenters/land.test.ts (`lets a start without review wait for its profile, and confirm the one that skips the review`)

#### Scenario: Nothing to confirm
- **WHEN** a start can run and nothing needs a person's decision
- **THEN** it starts at once, unless it was asked by S, "Safe Push With Profile…" or as the start without review
  [@test] src/presenters/land.test.ts (`starts at once when Rust decided there is nothing to say, and waits for its decision before it`)

### Requirement: Starting a Safe Push or a Dry Run: The effects of a start are Rust's, run in order

What the start does then MUST be Rust's too (`PlannedEffect`, run in order by the engine): the retarget of a pull
request into a virtual branch into the target, the cancel of the Safe Push it replaces, the push, then the robot,
from the pull request or through the API with the profile.

#### Scenario: The effects in order
- **WHEN** a start with nothing in its way runs
- **THEN** it cancels the robot it replaces, pushes what it lands root first and starts the robot, in the order of its
  plan
  [@test] crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`)
  [@test] crates/operations/src/land_execute_test.rs (`a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan`)

#### Scenario: The retarget of a pull request into a virtual branch
- **WHEN** the pull request of the branch goes into a virtual branch
- **THEN** the start retargets it into the target first, and the popover says it: "Retargets #8 of oauth from
  user/virtual/login to master first: every pull request of a virtual branch goes into master."

### Requirement: Starting a Safe Push or a Dry Run: The page runs the plan it showed as one operation

The page MUST run the plan it showed (`execute` of its intent on the plan's fingerprint, every confirmation of it
accepted, `startPlanned` of `src/hooks/useSafePush.ts`): one operation of the session of the repository
(`operations::engine::execute_land`, the run `brd safe-push` runs, `operations::land_execute`), admitted over the
operations that run there, the plan prepared again from the facts now.

#### Scenario: A second start of the same subject
- **WHEN** a start of the same subject comes while one runs
- **THEN** it joins it, and one that lands a part of it waits for it

### Requirement: Starting a Safe Push or a Dry Run: A plan that changed is refused before any effect

A fingerprint or a confirmation the plan no longer has MUST refuse the start as `plan_changed` before any effect, in the
words of the start.

#### Scenario: What the start was decided on changed
- **WHEN** the plan prepared again no longer has the fingerprint or a confirmation the user accepted
- **THEN** the start is refused before any effect: "What the Safe Push of auth was decided on changed before it began
  (…); nothing was done: start it again"
  [@test] crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`)

### Requirement: Starting a Safe Push or a Dry Run: A start that waited runs on the plan the wait left

A start that waited in the queue MUST run on the plan the wait left, while the user accepted every confirmation of it:
one push, then the robot.

#### Scenario: S right after P
- **WHEN** S is pressed right after P, while the push of its branch sends
- **THEN** the start waits, then runs on the plan the wait left: one push, then the robot
  [@test] crates/operations/src/land_execute_test.rs (`a_start_that_waited_runs_on_the_plan_the_wait_left_while_the_user_accepted_what_it_asks`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push once and start the Safe Push pressed while the push of its branch sends`)

### Requirement: Starting a Safe Push or a Dry Run: Every window shows the step of a start in the slot

The operation MUST be in the view from its admission on (`RepositoryView.operations`), so every window shows its step in
the slot, and the node of the row MUST leave the user's moves at once (`RunningMove::Starting`).

#### Scenario: The steps of a start
- **WHEN** a start runs
- **THEN** the slot shows "Checking…", "Waiting…" for another holder of the repository lock with Stop Waiting beside
  it, "Pushing 45%" with the progress its channel tells the window that asked, "Starting…"
- **AND** the node of the row leaves the user's moves at once
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)

### Requirement: Starting a Safe Push or a Dry Run: The engine checks what lands, then runs the effects in order

Before the effects the engine MUST check what lands against the newest target and, of a whole stack, its landing gate
(`operations::land_session`), and MUST take the repository lock only to read the witness and freeze the commits and
the leases of the push. It MUST then run the effects in order, each with its status in the outcome; the first that
fails stops the rest.

#### Scenario: An effect fails
- **WHEN** an effect of the start fails
- **THEN** the effects after it do not run and the ones before it stay done, each with its status in the outcome
  [@test] crates/operations/src/land_execute_test.rs (`the_first_effect_that_fails_stops_the_start_and_the_ones_before_it_stay_done`, `a_start_cancels_the_robot_it_replaces_pushes_and_starts_in_the_order_of_its_plan`)

### Requirement: Starting a Safe Push or a Dry Run: A change during the effects skips the start

A rewrite after the push (the virtual branch moved off the pushed commit), a robot of what it lands started meanwhile or
a closed pull request of its comment MUST skip the start as `plan_changed`: the toast says what changed and what was
done, with "Start Safe Push" (a new plan, without the push and the cancel it made) and Open in Patronus of the robot it
canceled.

#### Scenario: A rewrite while the push sends
- **WHEN** the virtual branch moves off the pushed commit while the push sends
- **THEN** the start is skipped and the toast says what was done ("the Safe Push it replaced was canceled and the push
  stays")
  [@test] crates/operations/src/land_execute_test.rs (`a_rewrite_while_the_push_sends_skips_the_start_and_the_outcome_tells_what_was_done`)
  [@test] src/presenters/outcome.test.ts (`offers the start again when the facts changed after the push, naming the robot it canceled; a failed effect says its error`)

#### Scenario: A robot of what it lands started meanwhile
- **WHEN** a robot of what the start lands started meanwhile
- **THEN** the start is skipped
  [@test] crates/operations/src/land_execute_test.rs (`a_robot_of_what_it_lands_started_meanwhile_skips_the_start`)

### Requirement: Starting a Safe Push or a Dry Run: A start pressed during a sync starts at once

A Safe Push start pressed while a sync is running MUST start at once on the table of the last sync. It MUST NOT wait for
that sync or start another one: the held sync ends once.

#### Scenario: A start during a sync
- **WHEN** a Safe Push start is pressed while a sync is running
- **THEN** it starts at once, on the table of the last sync, and the sync ends once
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push pressed during a sync at once, on the table of the last sync, the sync ending once`)

#### Scenario: A start that comments during a sync
- **WHEN** a start that comments on its pull request is pressed during a sync
- **THEN** it comments once, and the sync ends once
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should comment once for a start pressed during a sync, the sync ending once`)

### Requirement: Starting a Safe Push or a Dry Run: A click waits for the facts its plan needs

A click MUST wait for the facts its plan needs, its button busy with "Checking…": the session answers each of them, a
failure too (the review it asks at once, `unavailable` while Patronus can't be asked), so the wait ends. Then it MUST
start at once, say the refusal in an `info` toast, or open the popover.

#### Scenario: The wait of a click ends
- **WHEN** the user clicks a start whose plan needs facts the session has not answered
- **THEN** the button is busy with "Checking…" until the session answers each of them, a failure too
- **AND** then it starts at once, says the refusal in an `info` toast ("Push Branch" its action where the push alone is
  the step), or opens the popover
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)

### Requirement: Starting a Safe Push or a Dry Run: A click meant for what was there before is ignored

The second click of a double click and a click on a button that just took the place of another step under a pointer
that stayed MUST be ignored: they were meant for what was there before.

#### Scenario: A double click or a step that changed under the pointer
- **WHEN** the second click of a double click comes, or a click on a button that just took the place of another step
  under a pointer that stayed (a poll changed the step, 700 ms)
- **THEN** it is ignored

### Requirement: Starting a Safe Push or a Dry Run: The popover opens at the slot as a named dialog

The popover (`SafePushPopover`) MUST open at the slot of the row (or of the island header, [Stack
Island](../stack-island/spec.md#header)), a dialog named "Safe Push of auth".

#### Scenario: The popover of a row
- **WHEN** the start of a branch opens its popover, from the "⋯" of its row or "Safe Push With Profile…" of its menu
- **THEN** a dialog named "Safe Push of auth" opens at the slot of the row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`)

#### Scenario: The popover of an island header
- **WHEN** the start of a whole stack opens its popover from the island header
- **THEN** the popover opens at the header
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: Starting a Safe Push or a Dry Run: The title of the popover is what lands

The title of the popover MUST be what lands, of a Safe Push or a dry run; from three ancestors on the names MUST be cut.

#### Scenario: A branch alone
- **WHEN** the popover of a branch alone opens
- **THEN** its title is "Lands auth into master: 3 commits."

#### Scenario: A stacked branch
- **WHEN** the popover of a stacked branch opens
- **THEN** its title is "Lands oauth with auth and login, which it is built on: 3 branches · 12 commits into master."
  [@test] src/lib/patronus-stack.test.ts (`lands a stacked branch with what it is built on, in one robot, counted`)

#### Scenario: Three ancestors and more
- **WHEN** the branch is built on three ancestors or more
- **THEN** the names are cut: "Lands IJAI-4074 with vm-lane-daemon, air-fake-agent, air-lane-document and 18 more,
  which it is built on: 22 branches · 152 commits into master."
  [@test] src/lib/patronus-stack.test.ts (`names three ancestors and counts the rest, the size of what lands first`)

#### Scenario: A dry run
- **WHEN** the popover of a dry run opens
- **THEN** its title is "Checks oauth with auth and login, which it is built on: 3 branches · 12 commits." or "Checks
  auth on master without pushing into it: 3 commits."
  [@test] src/lib/patronus-stack.test.ts (`says that a dry run checks the commits of the ancestors too, and nothing of the children`)

### Requirement: Starting a Safe Push or a Dry Run: Below the title only what applies, in sentences

Below the title the popover MUST say only what applies, in sentences: the pull request it comments on
([below](#pull-requests)); the branches built on it, muted; the profile; the review ([below](#code-owner-review)); the
running Safe Push it cancels; a push that dismisses approvals, or rules that can't be read; the way to land a stacked
branch alone.

#### Scenario: The branches built on it
- **WHEN** branches are built on the branch that lands
- **THEN** a muted line says "oauth and device are built on login: they stay, and after the merge the next sync builds
  them on master."
  [@test] src/lib/patronus-stack.test.ts (`says that the branches built on it stay, and what the next sync after the merge does to them`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say in the Safe Push popover of a branch in the middle what lands and what stays`)

#### Scenario: The running Safe Push it cancels
- **WHEN** the start cancels a running Safe Push
- **THEN** the popover says "Cancels the Safe Push of auth running on 1a2b3c4."

### Requirement: Starting a Safe Push or a Dry Run: The footer of the popover and its start

The footer MUST say what it waits for or why it can't start, then "Dry Run" (a Safe Push only: the start of a dry run
instead), Cancel and the start ("Safe Push", "Dry Run", "Safe Push Without Review"), which MUST take the focus once it
can run, so Enter starts.

#### Scenario: The start takes the focus once it can run
- **WHEN** the popover can start
- **THEN** its start takes the focus, and Enter starts
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should give the focus to the start of the popover once it can start`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`)

#### Scenario: Dry Run of the popover of a Safe Push
- **WHEN** the user presses "Dry Run" in the popover of a Safe Push
- **THEN** a dry run starts instead
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)

### Requirement: Starting a Safe Push or a Dry Run: A start, Escape and a click outside close the popover

A start MUST close the popover: the slot shows it. Escape and a click outside SHALL close it. The stack graph SHALL
highlight the path it lands while it is open.

#### Scenario: The popover closes
- **WHEN** the user starts from the popover, presses Escape or clicks outside
- **THEN** the popover closes, and after a start the slot shows it

#### Scenario: The stack graph while the popover is open
- **WHEN** the popover is open
- **THEN** the stack graph highlights the path it lands

### Requirement: Starting a Safe Push or a Dry Run: The profiles a start offers and its default

The profile MUST be "Default checks" or one of the merge profiles of the target branch that allow the mode (the
emergency ones apart, `emergencyProfiles`), by default the profile of the last robot of the branch (never an emergency
one), with its description from Patronus below. Branch Deck MUST NOT remember a profile: the profile is the branch's
by its robots.

#### Scenario: The default is the profile of the last robot
- **WHEN** the popover of a branch whose last robot ran a profile opens
- **THEN** that profile is chosen, named so: "The profile of the last Safe Push of auth."
  [@test] crates/patronus/src/actions_test.rs (`start_options_keep_the_emergency_profiles_apart_and_default_to_the_profile_of_the_last_robot`)
  [@test] crates/domain-policy/src/land_test.rs (`the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…`, `should open the popover of a click for the profile of the last Safe Push, and start with the one chosen`)

### Requirement: Starting a Safe Push or a Dry Run: The profiles are a fact of the session

The profiles are `GET /robot-start-supported`, cached for 10 minutes (an error for 1), a fact of the session for the
target of its baseline (`facts.profiles`), which the popover when it opens and a click before it decides MUST ask
(`refresh_facts { profiles }`).

#### Scenario: When the profiles are read
- **WHEN** a start asks for the profiles
- **THEN** they are read when unknown, after a failure, a minute after the last read, or for another baseline
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_profiles_are_asked_when_a_start_needs_them_and_a_minute_later`)

#### Scenario: Another baseline while a read of the old one is in flight
- **WHEN** the profiles of another baseline are asked while a read of the old one is still in flight
- **THEN** they are read at once beside it, and a refresh waits only for the read of its baseline
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_refresh_of_the_profiles_waits_only_for_the_read_of_its_baseline`)

### Requirement: Starting a Safe Push or a Dry Run: The default of the last robot is Rust's

The default of the last robot MUST be Rust's (`domain_policy::land::default_profile`, the profiles a start offers
`offered_profiles`, which `prepare` answers with the plan, its `alternatives`): the profile an answer named on the robot
(a poll, a start, the New API's list), else the one its details name, read once for it.

#### Scenario: The default while it is offered
- **WHEN** the profile of the last robot is offered
- **THEN** it is the default, and another profile is confirmed
  [@test] crates/domain-policy/src/land_test.rs (`the_default_profile_is_the_last_robots_while_offered_and_a_profile_is_confirmed`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_profile_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Starting a Safe Push or a Dry Run: A target Patronus does not protect can't be started

A target branch Patronus does not protect MUST NOT be started.

#### Scenario: An unprotected target
- **WHEN** Patronus does not protect the target branch
- **THEN** the start is not possible

### Requirement: Starting a Safe Push or a Dry Run: A profile carries whether it skips the review

A profile MUST carry `skipsReview`: true for an emergency profile and for the profile of the last robot whose options
say `patronus.review.approval.check.skip=true` (false when they say otherwise), null for the rest, which Patronus
doesn't describe. A robot's profile MUST carry it from its options (`robot_profile`), null while Patronus hasn't
written them.

#### Scenario: The options of a robot
- **WHEN** the options of the last robot say whether the review is skipped
- **THEN** its profile's `skipsReview` says it, and it is null while Patronus hasn't written them
  [@test] crates/patronus/src/service_test.rs (`the_options_of_a_robot_say_whether_its_profile_skips_the_review`)

### Requirement: Starting a Safe Push or a Dry Run: A profile that skips the review starts without approvals

A profile known to skip the review SHALL let the start run without the approvals, and the review line MUST say so.

#### Scenario: A profile that skips the review
- **WHEN** the chosen profile is known to skip the review
- **THEN** the start can run without the approvals, the review line saying "Starts without the approvals of IJPL Code
  Platform: Large Scale Change skips the code-owner review."
  [@test] crates/domain-policy/src/land_test.rs (`a_start_offers_the_profiles_of_its_mode_and_without_approvals_the_emergency_ones_and_a_skipping_default`)

### Requirement: Starting a Safe Push or a Dry Run: With the New Patronus API a list names the profile

With the New Patronus API (the setting above) a list SHALL name the profile of a robot where Patronus put it
(`mergeProfile`: id, name, `skipsReview`; behind its `robot.list.merge.profile.enabled` flag), and the last robot MUST
NOT be read for the default profile and whether it skips the review (`domain_policy::land::default_profile` takes the
profile the list named).

#### Scenario: A list names the merge profile
- **WHEN** a list of the New API names the profile of a robot
- **THEN** the default profile and whether it skips the review come with the list, without a read of the last robot
  [@test] crates/patronus/src/service_test.rs (`with_the_new_api_a_list_names_the_merge_profile_patronus_put_on_a_robot`)
  [@test] crates/patronus/src/actions_test.rs (`start_options_take_the_profile_the_list_named_without_reading_the_last_robot`)

#### Scenario: A null merge profile
- **WHEN** the list has a null `mergeProfile`
- **THEN** the profile stays unknown, as a list without it, never "none"

### Requirement: Starting a Safe Push or a Dry Run: The target is the branch of origin the baseline tracks

The target branch MUST be the branch of `origin` the baseline tracks (`origin/master` → `master`); a local baseline or
one on another remote MUST NOT be started.

#### Scenario: The target of a baseline
- **WHEN** the baseline is `origin/master`, a local branch, or a branch of another remote
- **THEN** the target is `master`, and none for the local baseline and the other remote, which can't be started
  [@test] crates/sync-core/src/patronus_test.rs (`the_target_is_the_branch_of_origin_the_baseline_tracks`)

### Requirement: Starting a Safe Push or a Dry Run: The commits the remote lacks are pushed first

A branch whose commits the remote lacks MUST be pushed first, decided by content as the push button decides it (the
`Push` effect of the decision, over the action Rust decides of the remote status, `RemoteStatusUpdate::push_action`),
never by the hash of the tip. A dry run MUST push the branch too: only the push into the target is left out.

#### Scenario: The remote lacks commits of the branch
- **WHEN** the remote lacks commits of the branch
- **THEN** the start pushes it, then starts the robot
  [@test] crates/domain-policy/src/land_test.rs (`a_start_with_nothing_in_its_way_pushes_what_it_lands_root_first_then_starts`)

#### Scenario: A dry run
- **WHEN** a dry run starts on a branch whose commits the remote lacks
- **THEN** the branch is pushed too

### Requirement: Starting a Safe Push or a Dry Run: The slot button is the one indicator of the push

The push MUST be part of the start: no note narrates it. While it runs the slot button MUST show it as the compact push
of the row does, at the width it had at rest, the one indicator of it: the push button of the row is disabled without
its spinner and percent; then "Starting…".

#### Scenario: The slot during the push
- **WHEN** the push of a start runs
- **THEN** the slot button shows the spinner and "45%" ("Pushing 45%" its name, "Pushing 2 branches 45%" for the one
  push of a stack in its hover and in the preview), at the width it had at rest
- **AND** the push button of the row is disabled without its spinner and percent, then the slot says "Starting…"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)

### Requirement: Starting a Safe Push or a Dry Run: A branch only rebuilt on a newer baseline is not pushed

A branch whose remote branch has the same commits, only rebuilt since on a newer baseline (every sync onto a newer
baseline does it), MUST NOT be pushed: Patronus rebases the remote branch itself, and the target conflict check below
proves that the rebase applies; a push would replace it with copies of the same commits and move the head of its pull
request for nothing. One reworded since MUST be pushed: Patronus lands the messages of the remote branch.

#### Scenario: The remote branch has the commits under other hashes
- **WHEN** the remote branch has the commits of the branch, only rebuilt on a newer baseline
- **THEN** the start runs without a push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a branch whose remote branch has its commits under other hashes without a push`)

#### Scenario: A commit reworded since
- **WHEN** a commit of the branch was reworded since the push
- **THEN** the branch is pushed

### Requirement: Starting a Safe Push or a Dry Run: The start waits for the remote status of its branch

While the remote status of the branch is not loaded, the start MUST wait: "Checking auth…".

#### Scenario: The remote status is not loaded
- **WHEN** a start is asked while the remote status of the branch is not loaded
- **THEN** it waits, saying "Checking auth…"
  [@test] crates/domain-policy/src/land_test.rs (`every_fact_without_an_answer_waits_or_confirms_and_never_refuses`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should wait to start until the remote status of the branch is loaded`)

### Requirement: Starting a Safe Push or a Dry Run: A start belongs to the repository it was asked in

A start MUST belong to the repository it was asked in: the click plans it in the session of that repository and runs
it there (`execute` names the repository; Rust resolves its branch prefix), so a switch of the repository during the
start starts nothing in the other one, and its robot lands in the session of its own repository, which shows it on
return.

#### Scenario: Another repository opens during the checks
- **WHEN** another repository opens while a start runs its checks
- **THEN** the start goes on in its repository, and back in it the branch shows its robot
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push in its repository when another repository opens during its checks`)

#### Scenario: A plan read before the switch
- **WHEN** a plan read before the switch is started
- **THEN** it starts nothing: "Cannot start the Safe Push" says "idea-1 is no longer the open repository"

### Requirement: Starting a Safe Push or a Dry Run: The step of a start shows only in its repository

The step of a start MUST show only in its repository (the operations of its session): the branch of the same name in
the other one shows no step and starts on its own.

#### Scenario: A branch of the same name in another repository
- **WHEN** a start runs in one repository and the other one has a branch of the same name
- **THEN** that branch shows no step, and starts on its own

### Requirement: Starting a Safe Push or a Dry Run: A push that dismisses approvals is told before the start

A push before the start of a branch with an open pull request into the target that someone approved, in a repository
whose GitHub rules dismiss stale reviews on push (a `pull_request` rule with `dismiss_stale_reviews_on_push` of
`GET repos/<owner>/<name>/rules/branches/<target>`, readable with read access; `remote_facts::branch_rules`), MUST be
told before the start, as a warning of the popover.

#### Scenario: The rules dismiss stale reviews
- **WHEN** the start would push into an approved pull request whose rules dismiss stale reviews
- **THEN** the popover warns: "The push dismisses the approvals of #7: the rules of the repository dismiss stale reviews
  when new commits are pushed."
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should warn before a push that dismisses the approvals of the pull request`)
  [@test] crates/remote-facts/src/branch_rules_test.rs (`a_pull_request_rule_that_dismisses_stale_reviews_on_push_is_found_among_the_rules_of_the_branch`)

#### Scenario: A branch without such a rule
- **WHEN** the rules of the branch have no such rule
- **THEN** the push keeps the approvals
  [@test] crates/remote-facts/src/branch_rules_test.rs (`a_branch_without_such_a_rule_keeps_the_approvals`)

### Requirement: Starting a Safe Push or a Dry Run: The rules are read for each base of an approved pull request

The rules MUST be a fact of the session (`facts.rules`, `peekBranchRules`: `keep`, `dismiss`, or `unknown` with the
reason), read once per session for each base of the open pull requests someone approved (an unknown base is the
target), after the listing that has the approval. The start MUST read them only while it would push into an approved
pull request.

#### Scenario: The bases whose rules are read
- **WHEN** a listing has an approved open pull request
- **THEN** the rules of its base are read once per session
  [@test] crates/repository-model/src/pull_request_facts_test.rs (`the_rules_are_of_the_bases_of_the_approved_open_pull_requests`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_rules_of_an_approved_base_are_read_once_and_a_failed_read_again_when_a_start_asks`)

### Requirement: Starting a Safe Push or a Dry Run: Until the rules answer, the start waits

Until the rules answer, the start MUST wait ("Checking whether the push keeps the approvals of #7…"): a start that
pushed meanwhile would take the approvals away.

#### Scenario: Enter before the rules answer
- **WHEN** the user presses Enter before the rules answer
- **THEN** the start waits and the push dismisses no approvals
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should wait for the rules before a push into an approved pull request: Enter must not dismiss its approvals`)
  [@test] crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)

### Requirement: Starting a Safe Push or a Dry Run: Rules that can't be read never keep the approvals

Rules that can't be read (no token, no access, GitHub failed) MUST NOT be "keeps the approvals": the start MUST confirm
first, a warning of the popover, which Enter or the start accepts.

#### Scenario: GitHub failed to answer the rules
- **WHEN** the rules of the approved pull request can't be read
- **THEN** the popover warns "Cannot check whether the push keeps the approvals of #7: GitHub answered 500. Patronus
  counts them when the robot starts.", and Enter or the start accepts it
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should confirm a push into an approved pull request whose rules can't be read, never reading them as keeping the approvals`)
  [@test] crates/remote-facts/src/branch_rules_test.rs (`a_failed_answer_is_an_error_not_a_guess`)
  [@test] crates/domain-policy/src/land_test.rs (`the_rules_of_a_push_into_an_approved_pull_request_refuse_confirm_or_wait`)

### Requirement: Starting a Safe Push or a Dry Run: Each start asks again for the rules GitHub didn't give

A failed read MUST NOT be kept: each start MUST ask the session to read again the rules GitHub didn't give
(`refresh_facts { branchRules, afterWrite }`), and the click MUST wait for those reads, never for the read of another
base a listing asked.

#### Scenario: A start after a failed read
- **WHEN** a start comes after a failed read of the rules
- **THEN** the rules are read again, and the key starts once GitHub gives them
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should ask again for the rules a start couldn't read, and start with the key once GitHub gives them`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`the_rules_of_an_approved_base_are_read_once_and_a_failed_read_again_when_a_start_asks`)

#### Scenario: The read of another base
- **WHEN** a listing asked the read of another base
- **THEN** the click doesn't wait for it
  [@test] crates/remote-facts/src/scheduler_test.rs (`a_refresh_of_the_rules_waits_only_for_the_reads_it_asked`)

### Requirement: Starting a Safe Push or a Dry Run: A push that dismisses the counted approvals refuses

When the approvals the push dismisses are the ones Patronus counts (the readiness is `approved`), the robot would start
without them: the start MUST be refused, and "Push Branch" alone offered (the action of the `info` toast, the button of
the popover).

#### Scenario: The push dismisses the counted approvals
- **WHEN** the push dismisses the approvals Patronus counts
- **THEN** the start is refused with "The push dismisses the approvals of #7: push first, then ask for the review
  again", and only the push is offered
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer only the push when it dismisses the approvals Patronus counts`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should refuse the click with the push alone in its toast`, `should say in the popover why it can't start, and push alone from it`)

### Requirement: Starting a Safe Push or a Dry Run: The toast of a start that can't run is of its robots

The toast of a start that can't run MUST be of the robots of its subject: a later start takes its place, and its Push
closes it when pressed (a step of the start, which the row shows, and no Push is left behind that pushes nothing).

#### Scenario: Push of the toast
- **WHEN** the user presses the Push of the toast of a start that can't run
- **THEN** the toast closes and the row shows the push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should refuse the click with the push alone in its toast`)

#### Scenario: A later start
- **WHEN** a later start of the subject comes
- **THEN** its toast takes the place of the one of the start that couldn't run

### Requirement: Starting a Safe Push or a Dry Run: `brd safe-push` reads the same rules before the start

`brd safe-push` MUST read the same rules with the same function before the same start, refuse
`push_dismisses_approvals` where the app refuses, say the dismissed approvals Patronus doesn't count in `rules`, and
refuse rules GitHub doesn't answer with `rules_unknown` unless `--allow-unknown-rules`
([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)).

#### Scenario: `brd safe-push` before a push that dismisses approvals
- **WHEN** `brd safe-push` would push into an approved pull request whose rules dismiss the counted approvals, or
  whose rules GitHub doesn't answer
- **THEN** it refuses with `push_dismisses_approvals`, or with `rules_unknown` unless `--allow-unknown-rules`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_push_that_dismisses_the_approvals_patronus_counts_refuses_the_safe_push`, `rules_github_does_not_answer_refuse_the_safe_push_unless_allowed`)

### Requirement: Starting a Safe Push or a Dry Run: No start runs past the landing gate

A start MUST NOT run past the landing gate, the rule of `brd safe-push` in Rust (`trial_check::landing_gate`): a trial
check of what lands, with the proven dangling references and, with `branchdeck.symbols on`, the missing declarations,
the uses by a commit of what lands of a name that a commit outside it adds, which no merged Safe Push landed (the
commits the repository view names landed, on the robots of the session, never the page's).

#### Scenario: A branch uses a name only another branch adds
- **GIVEN** git merges a branch that uses a name only another branch adds, and the robot's build fails (the robots of
  2026-10-17 and 2026-10-21)
- **WHEN** a start lands a commit that uses a name a commit outside what lands adds, which no merged Safe Push landed
- **THEN** the landing gate refuses it
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_whose_build_needs_a_branch_outside_it_is_refused_with_the_fix`)

### Requirement: Starting a Safe Push or a Dry Run: The gate of a branch and of a whole stack

The gate of a branch MUST be its landing needs, which the session computes after its sync (below); the gate of a whole
stack the engine MUST check before the effects of its start (`operations::land_session`, through the object cache of
the repository).

#### Scenario: The gate of a whole stack
- **WHEN** the Safe Push of a whole stack starts
- **THEN** the engine checks its gate before the effects
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)

### Requirement: Starting a Safe Push or a Dry Run: A refusal of the gate offers the fix and Start Anyway

A refusal of the gate MUST start nothing and push nothing (`missing_declaration`, `dangling_reference`, with every
refusal and its fix, `ErrorDetails::Landing`): its error toast MUST offer the fix as its action, a restack of the root
of what lands, so the next start lands that branch too, and "Start Anyway" below it, the same start with the override
of what refused it (a name the user knows is a false alarm), planned again.

#### Scenario: The toast of a refused whole stack
- **WHEN** the gate refuses the Safe Push of a whole stack
- **THEN** the error toast "Cannot start the Safe Push of the whole lane-run stack" says "“Show each scenario” of
  lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the
  build of the whole lane-run stack fails. Stack lane-run on stills-codec to land it too." (one more refusal: "One more
  name breaks it too: brd check lists them.")
- **AND** its actions are "Stack lane-run on stills-codec" and "Start Anyway" below it
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should refuse the Safe Push of a whole stack whose build needs a branch outside it, with the restack and Start Anyway`)
  [@test] src/presenters/outcome.test.ts (`offers the fix and Start Anyway of the landing gate, Update Project of a conflict with the target, the start again of a changed plan`)
  [@test] src/presenters/landing.test.ts (`refuses the robot of 2026-10-21 with the name, both commits and the restack that lands the branch that adds it`)

### Requirement: Starting a Safe Push or a Dry Run: An unassigned commit or a cycle offers only Start Anyway

For a commit unassigned or a restack that would close a cycle, the text MUST name the move of the commit, and "Start
Anyway" MUST be the only action.

#### Scenario: The fix is the move of a commit
- **WHEN** the fix would be a commit unassigned or a restack that would close a cycle
- **THEN** the text names the move of the commit, and "Start Anyway" is the only action
  [@test] src/presenters/landing.test.ts (`counts the other refusals, names an unassigned commit by its hash, and offers no action for a move`)

### Requirement: Starting a Safe Push or a Dry Run: A gate of a whole stack that can't run holds nothing back

A gate of a whole stack that can't run MUST hold nothing back, and the start's toast MUST say so.

#### Scenario: The gate can't run
- **WHEN** the gate of a whole stack can't run
- **THEN** the start goes on, and its toast says "Not checked whether the whole lane-run stack uses a name only another
  branch adds: …"

### Requirement: Starting a Safe Push or a Dry Run: The row knows the refusal of the gate before the press

After a sync Rust MUST compute the refusals of every branch's start by the same rule (the landing needs, [Next
Step](../next-step/spec.md#what-the-safe-push-needs)), and the slot MUST say "Needs ABC-123" with the restack.

#### Scenario: The landing needs after a sync
- **WHEN** a sync ends after Patronus answered
- **THEN** the session derives the landing needs of every branch, and the slot of a branch they refuse says "Needs
  ABC-123"
  [@test] crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] src/presenters/landing.test.ts (`starts when nothing refuses it`)

### Requirement: Starting a Safe Push or a Dry Run: The popover shows the reason of the gate with its fix

The popover of the start (S, a click, Dry Run) MUST then open first (a confirm reason, `landing`) and show the reason
before the start, in the words of the toast, with the fix as its button, which restacks and leaves the popover open to
read the start again.

#### Scenario: The fix in the popover
- **WHEN** the start of a branch the gate refuses is asked
- **THEN** the popover shows the reason and the fix "Stack JIRA-456 on ABC-123"
- **AND** the fix restacks and leaves the popover open
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say in the Safe Push popover why the build fails before the start, and stack the branch from it`)
  [@test] src/presenters/land.test.ts (`opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start`)

### Requirement: Starting a Safe Push or a Dry Run: Start Anyway starts with the overrides of what refused it

The start of that popover MUST be "Start Anyway", the start with the overrides of what refused it, planned again with
them (`allow` of its `Land`). Start Anyway MUST be offered only there and in the refusal's toast, never by the row.

#### Scenario: Start Anyway from the popover
- **WHEN** the user presses "Start Anyway" in the popover
- **THEN** the start is planned again with the override of what refused it, and runs
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should start anyway from the Safe Push popover with the override of what refuses it`)
  [@test] crates/domain-policy/src/land_test.rs (`the_landing_gate_refuses_unless_allowed_then_a_conflict_with_the_target`)
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`)

### Requirement: Starting a Safe Push or a Dry Run: A dangling reference has the same fix

A dangling reference MUST have the same fix: the restack onto the branch of the commit that changes the use, or its
move.

#### Scenario: A dangling reference
- **WHEN** the gate refuses a dangling reference
- **THEN** the fix is the restack onto the branch of the commit that changes the use, or its move

### Requirement: Starting a Safe Push or a Dry Run: A landing branch that adds the name is named

While the branch that adds the name is landing in a running Safe Push (its own, of a branch built on it, of its whole
stack: `landingIn`), the reason MUST say so: its robot tests ABC-123 without JIRA-456, so the start fails all the same;
the fix stays the restack, or a start after it merged.

#### Scenario: The branch that adds the name is landing
- **WHEN** the branch that adds the name is landing in a running Safe Push
- **THEN** the reason says "ABC-123 is landing in its Safe Push, which tests it without this start: a start beside it
  fails too."
  [@test] src/presenters/landing.test.ts (`says that the branch the start needs is landing in a running Safe Push, which tests it without this start`)
  [@test] crates/trial-check/src/landing_gate_test.rs (`a_running_safe_push_of_the_branch_that_adds_the_name_is_named_in_the_reason`)
  [@test] crates/domain-policy/src/landing_needs_test.rs (`a_missing_declaration_names_the_safe_push_that_lands_the_branch_adding_the_name`)

### Requirement: Starting a Safe Push or a Dry Run: The engine checks the rebase onto the newest target

The engine MUST check the rebase before the effects of a start, on its own thread, outside the repository lock
(`operations::land_session`, the check of `brd safe-push`, `sync_core::target_conflict`). It MUST fetch only the
target's remote branch, as Update Project does, then replay what lands onto its tip as a plain `git rebase` does, with
no resolution by words, because Patronus can't resolve by words. Meanwhile the slot button says "Checking…".

#### Scenario: The target moved since the last sync
- **GIVEN** Patronus rebases what a robot lands onto the newest target before any check, and a conflict fails the robot
  at once ("Rebase … onto master is impossible"); a branch is built on the base of the checked out branch, not on the
  newest target, and the target moves every minute
- **WHEN** the target moved after the last sync, with a conflicting change or without one
- **THEN** the check finds the conflict, or finds the rebase clean
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_target_that_moved_with_a_conflicting_change_after_the_last_sync_conflicts`, `a_target_that_moved_without_a_conflict_is_clean`)

### Requirement: Starting a Safe Push or a Dry Run: A conflict with the newest target starts nothing

A conflict MUST start nothing and push nothing (`target_conflict`, the commit and its files in
`ErrorDetails::Conflict`); its error toast MUST offer "Update Project" of the repository of the start (a fetch only,
once the main window shows another one, `runUpdateProjectOf`).

#### Scenario: The start conflicts with the newest target
- **WHEN** what lands conflicts with the newest target
- **THEN** the error toast "Cannot start the Safe Push of auth" says "Nothing was pushed or started: auth conflicts with
  the newest target in a.txt: Patronus would fail it at once", with "Update Project" as its action
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should refuse a Safe Push that conflicts with the newest target, nothing pushed or started, with Update Project`)

### Requirement: Starting a Safe Push or a Dry Run: A failed fetch checks the last fetched target

A failed fetch MUST NOT stop the check: it replays onto the last fetched tip, and the start's toast MUST say so after
its description (`StartNote`).

#### Scenario: The fetch of the target fails
- **WHEN** the fetch of the target fails
- **THEN** the check replays onto the last fetched tip, and the toast says "The fetch of origin/master failed, so the
  check used the last fetched origin/master: …"
  [@test] crates/sync-core/src/target_conflict_test.rs (`a_failed_fetch_checks_the_last_fetched_target_and_says_so`)
  [@test] src/presenters/outcome.test.ts (`says the robot it started with the profile of its plan, and the checks that ran on the last fetched target`)

### Requirement: Starting a Safe Push or a Dry Run: A target check that can't run holds nothing back

A check that can't run MUST hold nothing back, and the toast MUST say so.

#### Scenario: The check can't run
- **WHEN** the check of the rebase can't run
- **THEN** the start goes on, and the toast says "Not checked whether Patronus can rebase auth onto origin/master: …"

### Requirement: Starting a Safe Push or a Dry Run: The push of a start has no toast of its own

The push MUST be an effect of the start: it has no toast of its own (Activity records it, with the operation), the
start's toast is the one toast of the action. A failed push MUST start nothing.

#### Scenario: One action, one toast
- **WHEN** a start pushes its branch and starts the robot
- **THEN** the push shows no toast of its own, and the start's toast is the one toast
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)

#### Scenario: The push fails
- **WHEN** the push of a start fails
- **THEN** nothing is started

### Requirement: Starting a Safe Push or a Dry Run: The push of a start never drops remote commits

The push MUST be the push core of every push (`operations::push::send`): the commits and the leases frozen under the
lock, the network push after it, once more over a remote that moved without commits the branch lacks. It MUST NOT drop
someone's remote commits for the user.

#### Scenario: The remote branch has commits the branch lacks
- **WHEN** a branch, or an ancestor it pushes, has a remote branch with commits the branch lacks
- **THEN** the start is refused before the push: "The remote branch of auth has 2 commits that auth lacks: push it from
  its row, which shows them"

### Requirement: Starting a Safe Push or a Dry Run: A Safe Push on an older commit is canceled before the push

A Safe Push of the branch still running on an older commit MUST be canceled before that push, not restarted
([below](#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: A robot runs on an older commit
- **WHEN** a Safe Push of the branch runs on an older commit
- **THEN** it is canceled before the push, not restarted
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

### Requirement: Starting a Safe Push or a Dry Run: `POST /robots` starts the robot on the pushed branch

Then `POST /robots` MUST start the robot on the pushed virtual branch, as the web UI does (`ROBOT_WEB_UI`, the source
branch `refs/heads/{prefix}/virtual/{name}`, kept: delete policy `NEVER`), so a restart takes the newest commit of the
branch. The robot MUST go into the session at once and be polled.

#### Scenario: The request of a start
- **WHEN** the robot is started
- **THEN** the request names the profile chosen (`mergeProfileId`, null for the default checks), the mode
  (`REBASE_AUTOSQUASH`, `DRY_RUN`) and the Space merge request whose approvals count (`spaceReview`)
  [@test] crates/patronus/src/actions_test.rs (`starts_a_robot_on_the_pushed_branch_and_keeps_the_branch`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`, `should start a Safe Push with the profile chosen in the popover of S and of Safe Push With Profile…`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`, `should name the approvals a Safe Push needs on Space, and start with the merge request found open`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_restart_and_cancel_run_the_real_service_against_the_fake`)

### Requirement: Starting a Safe Push or a Dry Run: `execute` decides the start again before any effect

`execute` MUST decide the start again in Rust before any effect, by the rules of `brd safe-push`, over the facts of the
session, never the page's: what its plan refuses MUST refuse it, in the words of `brd` (`start_refusal_text`).

#### Scenario: The refusals of `execute`
- **WHEN** the start is of a parked branch, one built on parked or a whole stack with a parked member, a branch or a
  whole stack merged Safe Pushes landed whole, a branch the sync did not build or a stack branch that is not built or
  conflicts as one branch
- **THEN** it is refused before any effect ("Nothing was pushed or started: login is parked: IJPL-1")
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_its_plan_refuses_is_refused_before_patronus_or_github_is_asked`)
  [@test] crates/operations/src/land_execute_test.rs (`a_start_refused_before_its_first_effect_cancels_pushes_and_starts_nothing`)

#### Scenario: The robot part of the repository view
- **WHEN** a robot of the repository view is in the way
- **THEN** the start is refused ("A Safe Push of the last commit is running", "A Safe Push of login is running: wait
  until it merges"); a Dry Run waits only for a robot on the last commit

### Requirement: Starting a Safe Push or a Dry Run: A start that raced or came by IPC is refused the same way

A start that raced with another one (the IDE, `brd`) or came by IPC MUST be refused the same way, and a refused one MUST
comment on no pull request either.

#### Scenario: A refused start comments on nothing
- **WHEN** a start its plan refuses comes by IPC or races with another one
- **THEN** it is refused before Patronus or GitHub is asked, and comments on no pull request
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_its_plan_refuses_is_refused_before_patronus_or_github_is_asked`)

### Requirement: Starting a Safe Push or a Dry Run: A started robot is a success toast with its cancel

A started robot MUST be a `success` toast "Safe Push started" ("Dry Run started") that names the branch, the target and
the profile, never the ref the robot runs on (`describeRobot`), with "Open in Patronus" (the robot page) and below the
text "Cancel Safe Push" for 10 s: a start asked by mistake costs only the robot (the push and the comment stay).

#### Scenario: The toast of a started robot
- **WHEN** a robot starts
- **THEN** the toast says "auth → master · profile Large Scale Change", with "Open in Patronus" and "Cancel Safe Push"
  [@test] src/lib/patronus.test.ts (`describes a robot by its branch and target, with the profile it was started with`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the branch menu, pushing the branch first`)

### Requirement: Starting a Safe Push or a Dry Run: The outcome of a start in the page's words

Activity MUST record a start in the same words as its toast, with the operation (its log lines). The toast and the
entry MUST be the outcome of `execute` in the page's words (`presentLandOutcome` of `src/presenters/outcome.ts`, the
robot of `LandResult`).

#### Scenario: The outcome of a started robot
- **WHEN** `execute` answers with the robot it started
- **THEN** the toast and the Activity entry say the robot with the profile of its plan
  [@test] src/presenters/outcome.test.ts (`says the robot it started with the profile of its plan, and the checks that ran on the last fetched target`)

### Requirement: Starting a Safe Push or a Dry Run: A failed start is an error toast with what Patronus said

A failed start MUST be an error toast "Cannot start the Safe Push of auth" with what Patronus said.

#### Scenario: Patronus refuses the start
- **WHEN** the start of the robot fails
- **THEN** the error toast "Cannot start the Safe Push of auth" says what Patronus said

### Requirement: Starting a Safe Push or a Dry Run: The toast of a start is the toast of the robot

The toast of a start MUST be the toast of the robot ([One toast per
event](../../../../docs/contributing/ux-writing.md#one-toast-per-event), `showRobotEventToast`): its Cancel only while
the robot can be canceled, and its later states take its place (its failure, its merge, its cancel, a restart). It MUST
take the place of the toast of the robot it restarts and of the approvals it starts on ([Review
Requests](../../repository/review-requests/spec.md#approval-news)).

#### Scenario: A later state of the robot
- **WHEN** the robot started fails, merges, is canceled or restarted
- **THEN** that state takes the place of the toast of its start
  [@test] src/stores/patronus.test.ts (`takes the place of the toast of the robot's start: one toast of the robot`)

#### Scenario: The toast of an approval
- **WHEN** the row starts the branch whose approval a toast told
- **THEN** the toast of the approval turns into the start of the branch, in place
  [@test] tests/e2e/next-step/next-step.spec.ts (`should turn the toast of the approval into the start of the branch, in place, when the row starts it`)

### Requirement: Starting a Safe Push or a Dry Run: A start is never repeated by itself

A start MUST NOT be repeated by itself, not even right after the push: Patronus fetches the pushed branch while it
handles the start.

#### Scenario: A start fails
- **WHEN** a start fails
- **THEN** it is not repeated
  [@test] crates/patronus/src/actions_test.rs (`a_failed_start_is_never_repeated`)

### Requirement: Starting a Safe Push or a Dry Run: A stacked branch lands with what it is built on

A stacked branch carries the commits of the branches it is built on ([Stacked Branches](../stacked-branches/spec.md)):
a child can't land without them, so its Safe Push MUST land it with every branch it is built on that isn't in the target
yet, in one robot ([Landing](../../../../docs/design/landing.md)); its start MUST always confirm, the popover saying so
first.

#### Scenario: The Safe Push of a stacked branch
- **WHEN** the user starts the Safe Push of a stacked branch
- **THEN** the popover says what it lands first, and Enter starts one robot of it with what it is built on
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on in one Safe Push, Enter starts it`, `should say in the Safe Push popover of a branch in the middle what lands and what stays`)

### Requirement: Starting a Safe Push or a Dry Run: The branches the decision pushes are pushed parents first

Before the start the branches its decision pushes MUST be pushed, parents first, a part of the start, without a toast of
their own: the branches of the stack that have work for the remote, up to the nearest one a running Safe Push holds
and past one that landed, as Push Stack of its row covers them. One whose push drops remote commits MUST refuse the
start.

#### Scenario: Up to the nearest held one, past what landed
- **WHEN** the start of a stacked branch pushes the branches it is built on
- **THEN** it stops at the nearest one a running Safe Push holds (its robot lands it as it took it: a push into it would
  fail it at the end) and goes past one that landed (a merged Safe Push landed it whole, or the fetched baseline has
  every commit of it)
  [@test] crates/domain-policy/src/push_test.rs (`a_start_pushes_the_ancestors_with_work_up_to_the_nearest_held_one_past_what_landed`, `a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built`)
  [@test] crates/repository-model/src/derive_test.rs (`the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed`)
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_replaces_its_own_older_robot_a_dry_run_is_refused_its_push_and_the_push_stops_at_a_held_ancestor`)

#### Scenario: `brd safe-push` pushes what its decision pushes
- **WHEN** `brd safe-push` starts a stacked branch
- **THEN** it pushes what the decision pushes (the `push` effect of `domain_policy::land::decide`, one rule,
  `domain_policy::push::start_push`), and no other branch
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_start_pushes_what_its_decision_pushes_and_leaves_an_ancestor_a_running_safe_push_holds`)

### Requirement: Starting a Safe Push or a Dry Run: Every surface asks the start of the branch itself

Every surface MUST ask the start of the branch itself, its popover at its own row: its slot, "⋯", S, the preview footer
and the review window.

#### Scenario: The surfaces of a stacked branch
- **WHEN** the user starts a stacked branch from the "⋯" of its row, Safe Push With Profile… of its menu, S, the review
  window or the footer of its preview
- **THEN** the start of that branch is asked, its popover at its row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a stacked branch with what it is built on from the ⋯ of its row and Safe Push With Profile… of its menu`, `should land a stacked branch with what it is built on from S and the review window`, `should land a stacked branch with what it is built on from the footer of its preview and S in it, the popover at its row`)

### Requirement: Starting a Safe Push or a Dry Run: The header of a stack island starts the whole stack

The header of a stack island MUST ask the start of the whole stack, one robot of its stack branch, its popover at the
header ([Stack Island](../stack-island/spec.md#header), [Whole
Stack](../whole-stack/spec.md#the-safe-push-of-the-whole-stack-in-the-app)); of a straight chain, which has none, the
start of its last branch.

#### Scenario: The start of the whole stack from the header
- **WHEN** the user starts from the header of a stack island, its "⋯" or S on it, a Safe Push or a Dry Run
- **THEN** one robot of the stack branch is asked, the popover at the header
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`, `should check the whole stack in a Dry Run from the island's ⋯, its popover at the header`)

#### Scenario: A straight chain
- **WHEN** the stack is a straight chain, which has no stack branch
- **THEN** the start is the one of its last branch

### Requirement: Starting a Safe Push or a Dry Run: The popover offers the whole stack as its second way

The popover of the root of a stack and of any branch of a stack that forks MUST offer the whole stack as its second
way, below what it lands, the item of the menus (`getWholeStackStartItems`), its hover what it lands, disabled with the
reason under it; it opens the popover of the whole stack.

#### Scenario: The whole stack in the popover of its root
- **WHEN** the popover of the root of a stack or of a branch of a stack that forks opens
- **THEN** it offers "Land the whole stack:" with "Safe Push Whole Stack" ("Check the whole stack:" with "Dry Run Whole
  Stack" for a Dry Run), which opens the popover of the whole stack
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)

### Requirement: Starting a Safe Push or a Dry Run: The popover checks whether a stacked branch builds alone

The popover of a Safe Push of a stacked branch MUST ask the trial build of the branch alone (`get_stack_independence`)
when it opens, and again when the local tip of the branch changes; it MUST NOT hold the start back. Its answer is the
second way, muted below the landing, never the lead. The popover offers each way and decides none: the stack parents
don't say why a branch is stacked.

#### Scenario: The check runs
- **GIVEN** a stored stack parent can stay after the branch stopped needing it ([Stack
  Parents](../stack-parents/spec.md#a-stacked-branch-that-builds-alone))
- **WHEN** the popover of a stacked branch opens and the check runs
- **THEN** a muted line says "Checking whether perf-part-2 builds alone…", and the start is not held back

### Requirement: Starting a Safe Push or a Dry Run: A branch that builds alone is offered the unstack

When the branch builds alone and uses no name its parent adds, the popover MUST offer "Unstack from IJPL-257337". The
trial build is on the base the sync builds on, not on the newest target, so the words MUST name the baseline. An
unstack MUST NOT start a Safe Push.

#### Scenario: The branch builds alone
- **WHEN** the branch builds alone and uses no name its parent adds
- **THEN** the popover says "To land perf-part-2 alone: it applies on the baseline
  without IJPL-257337 and uses no name it adds. Not checked by a compiler." with "Unstack from IJPL-257337"
  [@test] src/lib/patronus-stack.test.ts (`offers the unstack when the branch applies on the baseline without its parent, after the landing`)

#### Scenario: After the unstack
- **WHEN** the user unstacks the branch
- **THEN** the unstack waits for its sync, then the popover reads the branch alone (a root now: no landing line), with
  its review, and the user presses Safe Push, which lands and pushes only the branch
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer to land only a stacked branch that applies alone`)

#### Scenario: The names are not read
- **WHEN** `branchdeck.symbols` is `report` or `off`
- **THEN** the names are not read, and the sentence ends ". Compile dependencies are not checked."

### Requirement: Starting a Safe Push or a Dry Run: A branch that uses a name its parent adds is warned

When the branch builds alone but uses a name its parent adds (`uses`), the popover MUST warn, and "Unstack from
IJPL-257337" MUST follow as a link: names found the use, so the unstack is offered, never decided.

#### Scenario: The branch uses a name its parent adds
- **WHEN** the branch builds alone but uses a name its parent adds
- **THEN** a warning says "\"Cache the parsed icon\" uses
  parsedIcon, which \"Parse the icon off the EDT\" of IJPL-257337 adds. perf-part-2 applies on the baseline
  without IJPL-257337, but needs it to compile. Found by names, not by a compiler." (two uses both: "uses parsedIcon
  and iconCache, which …" of one commit and what one commit adds, else a sentence each; from three on "uses
  parsedIcon and 2 more names, which …", never "and 1 more name") "Unstack from IJPL-257337"
  follows as a link
  [@test] src/lib/patronus-stack.test.ts (`warns that a branch that applies alone uses a name its parent adds, the unstack still offered`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should warn that a stacked branch that applies alone uses a name its parent adds, the unstack still offered`)

### Requirement: Starting a Safe Push or a Dry Run: A branch that needs commits that can move is offered the move

When the branch needs commits that can move, the popover MUST offer their move into the branch; then the check runs
again. The count MUST hold the fixups that move with their target. While the branch list misses one of the commits,
they MUST be taken as unable to move.

#### Scenario: The needed commits can move
- **WHEN** the branch needs commits that no other branch needs
- **THEN** the popover says "To land perf-part-2 alone, move what it needs here: \"Cache the parsed
  icon\" changes lines that \"Parse the icon off the EDT\" of IJPL-257337 adds." (with more commits, "2 commits of
  IJPL-257337 add") with "Move 1 Commit Here", a move into the branch; then the check runs again
  [@test] src/lib/patronus-stack.test.ts (`offers to move the commits it needs when no other branch needs them`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

#### Scenario: The branch list lags
- **GIVEN** the check reads HEAD, and the branch list lags until the next sync
- **WHEN** the list misses one of the commits
- **THEN** it is as if they can't move

### Requirement: Starting a Safe Push or a Dry Run: No way alone when the needed commits can't move

When the needed commits can't move (another branch needs them too), on a conflict without a needed commit, and while
the check fails or runs, the popover MUST offer no way alone; the landing stays the only one.

#### Scenario: No way alone
- **WHEN** the needed commits can't move, a conflict has no needed commit, or the check failed or runs
- **THEN** the popover offers no way alone, and the landing stays the only one
  [@test] src/lib/patronus-stack.test.ts (`offers no way alone when the needed commits can't move, nor while the check runs or after it failed`)

### Requirement: Pull requests: The branches are reviewed on GitHub the robots list names

The branches MUST be reviewed on GitHub when the robots list names the GitHub repository they are mirrored to: the
mirror of the committed Space settings, or `branchdeck.githubRepository`
([Pull Requests](../pull-requests/spec.md#reading-pull-requests), `resolve_github_repository`). A repository MUST NOT be
built into the app.

#### Scenario: A poll keeps the GitHub repository of the list
- **GIVEN** the robots list named the GitHub repository of the branches
- **WHEN** a poll comes, which doesn't carry it
- **THEN** the repository stays, cached with the Space repository for 10 minutes
  [@test] src/stores/patronus.test.ts (`keeps the GitHub repository of a list through a poll, and refreshes the pull requests when a robot of one merged`)

### Requirement: Pull requests: A branch with an open pull request starts by a comment

Patronus links a robot to a pull request only when a comment of the pull request started it: only then the approvals
of the pull request count, Patronus reports in it, and closes it with the label `merged` once merged; the REST API
can't start such a robot. So the start of a branch with an open pull request MUST post the command as a regular
comment (`POST repos/<owner>/<name>/issues/<n>/comments` of the GitHub API, with the token of [Pull
Requests](../pull-requests/spec.md#the-token)).

#### Scenario: The command is a regular comment of the pull request
- **WHEN** a branch with an open pull request starts
- **THEN** the command is posted as a regular comment of the pull request, with the token of the pull requests
  [@test] crates/remote-facts/src/patronus_command_test.rs (`posts_the_command_as_a_regular_comment_of_the_pull_request`)

#### Scenario: The open pull request decides the route of the start
- **GIVEN** Patronus names the GitHub repository of the branches
- **WHEN** a branch with an open pull request into the target starts
- **THEN** the start goes from that pull request, by a comment, not through the REST API
  [@test] crates/domain-policy/src/pull_request_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository`)
  [@test] crates/domain-policy/src/land_test.rs (`the_pull_request_decides_the_route_a_draft_refuses_and_another_base_refuses_only_counted_approvals`)

### Requirement: Pull requests: The backend builds the text of the comment by its subject

The text of the comment MUST be built by the backend only, by its subject (`PatronusCommandSubject`):
`/patronus merge --keep-source-branch` for a Safe Push (the virtual branch stays; the sync archives it once merged, as
after a start through the API), `/patronus dry-run` for a dry run; a pull request of someone else the user reviewed is
`/patronus merge`, which deletes its branch once merged
([Review Requests](../../repository/review-requests/spec.md#safe-push-of-a-reviewed-pull-request)).

#### Scenario: A Safe Push keeps the source branch, a dry run is plain
- **WHEN** the start of a branch comments on its pull request
- **THEN** a Safe Push comments `/patronus merge --keep-source-branch`, and the virtual branch stays until the sync archives it once merged
- **AND** a dry run comments `/patronus dry-run`
  [@test] crates/remote-facts/src/patronus_command_test.rs (`a_merge_keeps_the_source_branch_and_a_dry_run_is_plain`)

#### Scenario: A reviewed pull request of someone else
- **WHEN** the Safe Push of a pull request of someone else the user reviewed starts
- **THEN** it comments `/patronus merge`, which deletes its branch once merged
  [@test] crates/remote-facts/src/patronus_command_test.rs (`a_merge_of_a_reviewed_pull_request_deletes_its_branch`)

### Requirement: Pull requests: A pull request into another base is never commented on

Patronus merges a pull request into its base: a pull request into another base than the target MUST NOT be commented
on (`domain_policy::pull_request::start_pull_request`), and such a branch MUST start through the API.

#### Scenario: A pull request into another base
- **GIVEN** the open pull request of a branch goes into another base than the target
- **WHEN** the branch starts
- **THEN** nothing is commented on the pull request, and the branch starts through the API
  [@test] crates/domain-policy/src/pull_request_test.rs (`a_safe_push_starts_from_the_open_pull_request_into_the_target_or_a_virtual_branch_while_patronus_names_the_github_repository`)

### Requirement: Pull requests: An old pull request into the parent's branch is retargeted

Every pull request of a virtual branch goes into the target ([Pull Requests](../pull-requests/spec.md)); an old one of
a stacked branch into its parent's branch (`{prefix}/virtual/…`) MUST be retargeted, by a push of the branch too
([Stacked Branches](../stacked-branches/spec.md#pull-requests)), and by the start itself first: the start
comments on it once GitHub took it, and its approvals count, as the readiness is asked with it.

#### Scenario: The start retargets first
- **GIVEN** a stacked branch has an old pull request into its parent's branch
- **WHEN** the branch starts
- **THEN** the start retargets the pull request into the target first and comments on it once GitHub took it
- **AND** its approvals count, as the readiness is asked with it

#### Scenario: The slot offers the retarget while one is open
- **GIVEN** an old pull request into the parent's branch is open
- **WHEN** the slot of the branch shows
- **THEN** it offers "Retarget #8 to master" (`retargetToTarget`, [Next Step](../next-step/spec.md#the-table))
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_discrepancy_cases_name_every_id_as_the_register_does`, `the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Pull requests: The popover says the comment a start posts

The popover MUST say it: "Comments /patronus merge on #7: Patronus reports there and closes it once merged."

#### Scenario: The popover of a branch with an open pull request
- **GIVEN** a branch has an open pull request
- **WHEN** the popover of its Safe Push opens
- **THEN** it says "Comments /patronus merge on #7: Patronus reports there and closes it once merged."
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

### Requirement: Pull requests: A comment can't choose a merge profile

A comment can't choose a merge profile: the popover MUST offer none, and "Safe Push With Profile…" MUST be disabled.

#### Scenario: Safe Push With Profile is disabled
- **GIVEN** a branch starts by a comment on #7
- **WHEN** the user looks at "Safe Push With Profile…"
- **THEN** it is disabled ("A comment on #7 runs the default checks: it can't choose a profile"), and the popover offers no profile
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

#### Scenario: A branch whose own profile is another one
- **GIVEN** the own profile of the branch is Large Scale Change
- **WHEN** the popover of its start by a comment opens
- **THEN** it says "A comment runs the default checks: it can't choose Large Scale Change."

### Requirement: Pull requests: A draft pull request is not Safe Pushed

Patronus doesn't merge a draft: "Safe Push" of a branch with a draft pull request MUST be disabled, "#7 is a draft:
Patronus merges it once it is ready for review", its slot the quiet "Draft" that opens it
([Next Step](../next-step/spec.md)). A dry run of a draft SHALL be allowed.

#### Scenario: A branch with a draft pull request
- **GIVEN** the pull request of a branch is a draft
- **WHEN** the user looks at its Safe Push
- **THEN** "Safe Push" is disabled, "#7 is a draft: Patronus merges it once it is ready for review"
- **AND** its slot is the quiet "Draft" that opens it, and a dry run runs
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not offer a Safe Push of a draft pull request: its step opens it, a dry run runs`)

### Requirement: Pull requests: The engine of the start reads the bot's reaction to the comment

The bot answers the comment with a reaction: 👍 started, 👎 not, with a comment starting `⚠️` that says why. The
engine of the start MUST read it (`operations::robot_start`, the same for `brd safe-push`): right after the comment,
then every 3 s on the clock of the session (`domain_policy::refresh`, never a timer of the page) for up to a minute
counted from the comment (the checks of the start before it take none of it).

#### Scenario: The reactions of the comment are polled
- **WHEN** the start posted its comment
- **THEN** it reads the reactions of its comment (`GET
  repos/<owner>/<name>/issues/comments/<id>`, conditional: a poll that sees no change costs no rate limit)
- **AND** a comment without reactions is pending, a 👍 accepted
  [@test] crates/remote-facts/src/patronus_command_test.rs (`a_comment_without_reactions_is_pending_and_a_thumbs_up_is_accepted`)

### Requirement: Pull requests: A refusal of the bot is an error toast with its reason

After 👎 the first later `⚠️` comment of a bot MUST be the reason, looked for once more if it isn't there yet. A
refusal MUST be the error toast "Cannot start the Safe Push of auth" with the bot's reason.

#### Scenario: The reason is a later comment of a bot
- **WHEN** the bot answers 👎
- **THEN** the reason is the first later `⚠️` comment of a bot, looked for once more if it isn't there yet
- **AND** a warning of a user is not one
  [@test] crates/remote-facts/src/patronus_command_test.rs (`a_thumbs_down_reads_the_reason_from_the_bot_comment`)
  [@test] crates/remote-facts/src/patronus_command_test.rs (`the_reason_is_a_later_bot_comment_and_a_user_warning_is_not_one`)

#### Scenario: The refusal is told
- **WHEN** the bot refused the comment
- **THEN** the start fails with the error toast "Cannot start the Safe Push of auth" with the bot's reason
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say why Patronus refused the comment`)

### Requirement: Pull requests: An accepted comment starts the newest robot of its pull request

After 👍 an incremental list (`since` shortly before the comment) MUST find the robot of the pull request
(`githubPrUrl`), which goes into the session and is polled: of that pull request, a Dry Run for a Dry Run and a Safe
Push for a Safe Push, none the session listed finished before the comment nor the one the start replaced, the newest
(`comment_robot`, as `brd safe-push` matches it).

#### Scenario: The newest robot of the pull request nobody knew
- **WHEN** the bot accepted the comment
- **THEN** the robot of the start is the newest robot of the pull request of its mode that the session didn't list finished before the comment
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_the_bot_accepts_starts_the_newest_robot_of_its_pull_request_nobody_knew`)

#### Scenario: An earlier robot of the pull request is never the new one
- **GIVEN** a Dry Run of #7 was canceled a minute before
- **WHEN** a new Dry Run of #7 starts by a comment
- **THEN** the canceled one is never the new robot
  [@test] crates/bd-remote/src/safe_push_test.rs (`the_robot_of_a_comment_is_never_an_earlier_robot_of_the_pull_request`)

### Requirement: Pull requests: A comment not answered within a minute is a start whose answer was lost

No answer within a minute MUST be a start whose answer was lost (below), and a read of the reply after it reached
GitHub that fails MUST end the wait the same way, never as a failure.

#### Scenario: The bot never answers
- **WHEN** the bot doesn't answer the comment within a minute
- **THEN** the start is pending: the warning toast "Cannot tell whether the Safe Push of auth started", "Patronus did not answer the
  comment on #7 within a minute. It holds auth until Patronus lists its robot, or until Forget This Start on the row",
  with Open Pull Request
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_the_bot_rejects_fails_with_its_reason_and_one_it_never_answers_is_pending_after_the_minute`)
  [@test] src/presenters/outcome.test.ts (`says a comment Patronus accepted whose robot it lists not yet, and a comment it never answered as a start not known yet that opens it`)

#### Scenario: A read of the reply fails
- **WHEN** a read of the reply fails after the comment reached GitHub
- **THEN** the wait ends as when the bot never answers, never as a failure

### Requirement: Pull requests: An accepted comment is the start, held until Patronus lists its robot

A 👍 MUST be the start: when Patronus lists no robot of it within the minute, the start of the app goes on looking for
it every 15 s for 5 minutes more, and holds its subject meanwhile: the slot says "Starting…" in every window, a second
click or key starts nothing, and a repeat of the same start MUST join it instead of posting a second comment, which
Patronus would answer with a second robot or 👎.

#### Scenario: Patronus lists the robot after the minute
- **GIVEN** the bot accepted the comment and Patronus lists no robot of it within the minute
- **WHEN** the start goes on looking for it every 15 s for 5 minutes more
- **THEN** it finds the robot once Patronus lists it
  [@test] crates/operations/src/robot_start_test.rs (`an_accepted_comment_looks_for_its_robot_after_the_minute_until_patronus_lists_it`)

#### Scenario: A repeat posts no second comment
- **GIVEN** a start Patronus accepted waits for its robot
- **WHEN** the user clicks or presses the start again
- **THEN** the slot says "Starting…" in every window, and the repeat joins the start without a second comment
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should hold a start Patronus accepted until it lists the robot: a repeat posts no second comment`)

### Requirement: Pull requests: The hold of a start is the session's

The hold MUST be the session's: a switch to another repository shows nothing on its branch of the same name, and back
in the repository the slot still says "Starting…".

#### Scenario: A switch to another repository and back
- **GIVEN** a start Patronus accepted is held
- **WHEN** the user switches to another repository and back
- **THEN** the branch of the same name in the other repository shows nothing, and back in the repository the slot still says "Starting…"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should keep holding a start Patronus accepted through a switch to another repository and back`)

### Requirement: Pull requests: The toast of an accepted start

Once listed, the toast MUST be "Safe Push started" of the robot; none within the 5 minutes, it MUST be "Safe Push
started", "Patronus accepted the comment on #7: the robot shows once Patronus lists it", with "Open Pull Request #7",
and the robot comes with a later list of the session.

#### Scenario: Patronus lists no robot within the 5 minutes
- **WHEN** Patronus accepted the comment and lists no robot within the 5 minutes
- **THEN** the toast is "Safe Push started", "Patronus accepted the comment on #7: the robot shows once Patronus lists it", with "Open Pull Request #7"
  [@test] src/presenters/outcome.test.ts (`says a comment Patronus accepted whose robot it lists not yet, and a comment it never answered as a start not known yet that opens it`)

### Requirement: Pull requests: A branch without an open pull request starts through the API

A branch without an open pull request MUST start through the API, as on Space, without a merge request; that start
counts no approval, so with approvals required its next step MUST be "Create Pull Request…"
([Next Step](../next-step/spec.md)).

#### Scenario: Approvals are required and there is no pull request
- **GIVEN** a branch has no open pull request and its changes require approvals
- **WHEN** its next step shows
- **THEN** it is "Create Pull Request…", and no start Patronus would fail
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`)

### Requirement: Pull requests: A robot of a pull request opens it and keeps it on a restart

A robot of a pull request (`type: GITHUB_PR`, `githubPrUrl`) knows it: its actions MUST have "Open Pull Request #7". A
restart MUST keep the pull request (Patronus restarts with it).

#### Scenario: The robot links its pull request
- **GIVEN** a robot was started by a comment of pull request #7
- **WHEN** its actions show
- **THEN** they have "Open Pull Request #7"
  [@test] crates/patronus/src/service_test.rs (`a_robot_started_by_a_pull_request_comment_links_the_pull_request`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the pull request with a /patronus merge comment`)

### Requirement: Merged: A running robot the session sees successful is news of its merge

A robot known as running or canceling that an answer of the session (a poll, a list, the robot of a cancel) shows
successful and not a dry run merged its branch (`domain_policy::news::robot_finished`): it MUST be news of the session
(`News`, `robotMerged`, with the pull requests its landing closes, [below](#merged)). A robot first seen finished MUST
NOT be news.

#### Scenario: A robot known running finished
- **GIVEN** the session knows a robot as running or canceling
- **WHEN** an answer of the session shows it successful and not a dry run
- **THEN** the session has the news `robotMerged` of it, and of a failed one its failure
  [@test] crates/domain-policy/src/news_test.rs (`a_robot_known_running_that_finished_merged_or_failed`)

#### Scenario: The news waits for the view of its robots
- **WHEN** a robot merged
- **THEN** the news is told once the Patronus view of the robots it merged in stands (at once while the session has no table to derive it of)
- **AND** it names the pull requests its landing closes
  [@test] crates/repository-model/src/facts_test.rs (`a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why`, `a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes`)

#### Scenario: The main window tells the news once
- **WHEN** the news of a merge comes
- **THEN** the main window tells it once (`tellRobotNews`; none of those its first view of the session had)

#### Scenario: A successful dry run or a robot first seen merged
- **WHEN** a dry run succeeds, or the session first sees a robot merged
- **THEN** nothing is told
  [@test] src/stores/patronus.test.ts (`does not tell a successful dry run or a robot first seen merged`)

### Requirement: Merged: A toast tells the merge with the push mode

With the window focused, a `success` toast MUST say "auth merged into master" (the branch, or the subject of a Safe
Push of a temporary ref) with the push mode ("Safe Push"), for 10 s; an issue key of the title ("IJPL-1 merged into
master") links to the tracker, as in the branch name ([Toasts](../../../../docs/contributing/ux-writing.md#toasts)).

#### Scenario: The title names the branch, else the subject
- **WHEN** the toast of a merge shows
- **THEN** its title names the branch, else the subject of a robot of a temporary ref
  [@test] src/lib/patronus.test.ts (`names the branch, else the subject of a robot of a temporary ref`)

#### Scenario: A running robot of a branch merged
- **GIVEN** the window is focused
- **WHEN** a running robot of a branch merged
- **THEN** a `success` toast tells it once, with Update Project and the robot page
  [@test] src/stores/patronus.test.ts (`tells once in a toast with Update Project and the robot page when a running robot of a branch merged`)

### Requirement: Merged: The toast of the merge offers Update Project first

Its actions MUST be below the text (the `robot` body of the toast): "Update Project", the primary one, which picks the
merge up in one click (it fetches the baseline first, then updates onto it; the click is the consent, and a conflict,
local changes or a lock say so as Update Project does), then "Open in Patronus", which opens the robot page and leaves
the toast for the update.

#### Scenario: Update Project of the toast
- **WHEN** the user clicks "Update Project" of the toast of a merge
- **THEN** it fetches the baseline first, then updates onto it
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push merged, and update the project on request`)

#### Scenario: The baseline was fetched since the merge in the shown repository
- **GIVEN** the baseline of the shown repository was fetched since the merge
- **WHEN** the toast of the merge shows
- **THEN** it still offers "Update Project": the update is what picks the merge up
  [@test] src/stores/patronus.test.ts (`offers Update Project also after a fetch since the merge: the update is what picks it up`)

### Requirement: Merged: A merge in a repository not shown offers Fetch

A merge in a repository the main window doesn't show MUST offer Fetch instead (Update Project updates the shown one),
or only "Open in Patronus" once its baseline was fetched since the merge (`lastFetchTime`: Branch Deck's fetch, or one
elsewhere by `FETCH_HEAD`).

#### Scenario: The merge of another repository
- **GIVEN** the main window shows another repository than the one of the merge
- **WHEN** the toast of the merge shows
- **THEN** it offers Fetch instead of Update Project, or only "Open in Patronus" once its baseline was fetched since the merge

### Requirement: Merged: Update Project of the toast acts on the repository of the merge

Its "Update Project" MUST act on the repository of the merge (`runUpdateProjectOf`): clicked after a switch of the
repository, it only fetches that repository, never updates the one shown since; the toast says Fetch from the switch
on, and Update Project again on the way back.

#### Scenario: Clicked after a switch of the repository
- **GIVEN** the main window switched to another repository after the merge
- **WHEN** the user clicks the action of the toast
- **THEN** it only fetches the repository of the merge, never updates the one shown since
  [@test] src/stores/update-project.test.ts (`updates the repository a toast is about only while it is shown, else only fetches it: never the one shown since`)

#### Scenario: The switch and the way back
- **WHEN** the main window switches to another repository and back
- **THEN** the toast says Fetch from the switch on, and Update Project again on the way back
  [@test] src/lib/landing-toast.test.ts (`offers Fetch once the main window shows another repository, and Update Project again on the way back`)

### Requirement: Merged: The toast of the merge is the toast of the landing

The toast of the merge MUST be the toast of the landing, until the update lands it
([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event), `showMergeToast` of
`src/lib/landing-toast.ts`): it takes the place of the toast of the robot (its start), and while an update or a fetch
of its repository runs, started from the toast, the header or a key, its Update Project is "Updating…"
("Fetching…"), disabled, and the toast stays.

#### Scenario: The merge replaces the toast of the start
- **GIVEN** the toast of the robot's start is open
- **WHEN** the robot merged
- **THEN** the toast of the merge takes its place: one toast of the robot
  [@test] src/stores/patronus.test.ts (`takes the place of the toast of the robot's start: one toast of the robot`)

#### Scenario: An update runs
- **WHEN** an update of its repository runs, started from the toast, the header or a key
- **THEN** its Update Project is "Updating…" ("Fetching…"), disabled, and the toast stays
  [@test] src/lib/landing-toast.test.ts (`is one toast from the merge to the landing: Update Project, Updating… while the update runs, then what the update landed in place`)
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should turn the toast of a merge into what the update landed, in place, when its Update Project is pressed`, `should turn the toast of a merge into what the update landed, in place, when the update starts from the header`)

### Requirement: Merged: The outcome of the update takes the place of the merge toast

The outcome of that update MUST take its place, in place: what landed (the toast of the update of
[Landed Commits](../../repository/landed-commits/spec.md#after-the-update), several merges in one), or "master is
already up to date with origin/master"; a run that ended otherwise (a conflict, whose dialog says it, an error toast,
the fetch of a repository not shown) MUST close it.

#### Scenario: The update landed the merge
- **WHEN** the update of the repository ends
- **THEN** the toast says in place what landed ("auth landed in origin/master, squashed" / "Updated master: rebased 104 commits"), or "master is already up to date with origin/master"
  [@test] src/lib/landing-toast.test.ts (`is one toast from the merge to the landing: Update Project, Updating… while the update runs, then what the update landed in place`)
  [@test] src/stores/update-project.test.ts (`takes the place of the toasts of the merges of its repository it brought in: what landed, or up to date`)

#### Scenario: The update ended otherwise
- **WHEN** the run ended otherwise: a conflict, whose dialog says it, an error toast, the fetch of a repository not shown
- **THEN** the toast of the merge closes
  [@test] src/lib/landing-toast.test.ts (`closes after an update that ended without an outcome of its own: the dialog of the conflict says it`)

### Requirement: Merged: A landing a sync found after an update elsewhere says itself in place

A landing of its branches that a sync found after an update elsewhere (`brd update`, the IDE) MUST say itself in
place: "auth landed in origin/master" / "Found by a sync", the words of its Activity entry; one that came before the
toast showed (it waits for the focus) is said at once. Activity keeps both entries, the merge and the landing.

#### Scenario: An update outside the app
- **WHEN** a sync found the landing after an update elsewhere (`brd update`, the IDE)
- **THEN** the toast says in place "auth landed in origin/master" / "Found by a sync"
  [@test] src/lib/landing-toast.test.ts (`says in place what a sync found landed after an update elsewhere (brd update, the IDE)`)
  [@test] tests/e2e/update-project/landing-toast.spec.ts (`should say in the toast of a merge what a sync found landed after an update outside the app`)

#### Scenario: The landing came before the toast showed
- **GIVEN** the toast of the merge waits for the focus
- **WHEN** an update elsewhere landed it before the toast showed
- **THEN** the toast says the landing at once
  [@test] src/lib/landing-toast.test.ts (`shows the landing at once when an update elsewhere landed it before the toast showed: the toast waited for the focus`)

### Requirement: Merged: A watched robot of no branch is polled and named by what it tested

A watched robot of no branch (the Safe Push of a reviewed pull request, `watchRobot`) MUST be polled with the robots of
the branches, and its merge MUST be named by what it tested.

#### Scenario: The merge of a reviewed pull request
- **GIVEN** the Safe Push of a reviewed pull request is watched
- **WHEN** it merged
- **THEN** the toast says "#101 Rework the session store merged into master"
  [@test] src/stores/patronus.test.ts (`polls a watched robot of no branch and names its merge by what it tests`)

### Requirement: Merged: The pull requests of the branches a merged robot landed are closed

A robot of a stacked branch that merged landed the branches it is built on too, the robot of the whole stack every
branch of the stack ([Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app), "Whole air stack merged into
master"): the open pull requests of those branches MUST be closed with a comment, as `gh pr close --comment` does, and
the toast and Activity MUST say it.

#### Scenario: The comment of the close
- **WHEN** a pull request of a branch the robot landed is closed
- **THEN** its comment is "Landed in master with #7, the Safe Push of user/virtual/oauth, which is built on this branch."
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`closes_a_pull_request_with_a_comment`)

#### Scenario: A closed or merged pull request
- **WHEN** the pull request is already closed, or merged
- **THEN** one already closed gets no second comment, a merged one can't be closed
  [@test] crates/remote-facts/src/pull_request_writes_test.rs (`a_closed_pull_request_gets_no_second_comment_and_a_merged_one_is_not_closed`)

#### Scenario: The toast and Activity say it
- **WHEN** the merge closes pull requests
- **THEN** the toast and Activity say it: "Safe Push · closes the pull request of auth: it landed with oauth", "closes the pull requests of 6 branches: they landed with the whole
  stack"
  [@test] src/presenters/news.test.ts (`says which pull requests the merge closes: of the branches, or how many the whole stack landed`)
  [@test] src/stores/patronus.test.ts (`closes in the session of the robot's repository the pull requests the news of the merge says its landing closes, and says so`, `closes the pull requests of the branches the whole stack landed`)

### Requirement: Merged: One rule of Rust decides which pull requests a landing closes

One rule MUST decide which, in Rust (`domain_policy::landed::pull_requests_to_close`), for the app and `brd robot`
(`closes`, which `brd` lists and never closes): by content, the branches the robot landed whole besides its own, as
the Patronus view of the robots it merged in names them (`landedByRobot`), and only a pull request that holds nothing
its branch lacks. The pull request the robot started from is Patronus'.

#### Scenario: The branches the robot landed whole, not its own
- **WHEN** a robot merged
- **THEN** the pull requests of the branches it landed whole besides its own are closed: while the robot ran on the branch,
  the stack branch, as it is, its tip or the same commits rebuilt on a newer baseline, or a commit of it the branch built
  new ones on since; after a change below its commit by the commits it tested
- **AND** the pull request the robot started from is Patronus', not closed here
  [@test] crates/domain-policy/src/landed_test.rs (`closes_the_pull_requests_of_the_branches_a_merged_robot_landed_whole_not_of_its_own_branch`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A branch with work left or a pull request with a commit the branch lacks
- **WHEN** a branch has commits added after the landed ones, or its pull request holds a commit the branch lacks
- **THEN** it keeps its pull request: only a pull request whose head is the branch's tip, or that is the remote branch
  and that has no commit the branch lacks (older versions of commits amended here included: their newer versions
  landed), is closed
  [@test] crates/domain-policy/src/landed_test.rs (`leaves_a_pull_request_with_a_commit_the_branch_lacks_one_of_a_branch_with_work_left_and_one_the_table_lacks`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should close by content the pull requests of the branches a Safe Push landed after a change below its commit, not of one with work left`)

#### Scenario: `brd robot` lists the closes
- **WHEN** `brd robot` shows a merged robot
- **THEN** it lists the pull requests its landing closes (`closes`) and closes none
  [@test] crates/bd-remote/src/robot_test.rs (`a_merged_robot_lists_the_pull_requests_its_landing_closes_and_closes_none`)

### Requirement: Merged: The session tells the merge with the pull requests it closes

The session MUST tell the merge once that view stands, with the pull requests it closes (`RobotMerged.closes`), so the
toast names them at once.

#### Scenario: The merge waits for the view
- **WHEN** a robot merged
- **THEN** the session tells it once the view of the robots it merged in stands, naming the pull requests its landing closes
  [@test] crates/repository-model/src/facts_test.rs (`a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes`)

### Requirement: Merged: The session of the robot's repository closes the pull requests

The page MUST ask the session of the robot's repository to close them (`CloseLandedPullRequests`, an operation of that
session, `crates/operations/src/close.rs`): Rust plans them again from the merge in its facts, the comment of each its
own, each close holding the state of its pull request, and the session has each closed at once. A robot first seen
merged MUST close nothing.

#### Scenario: The closes run in the session
- **WHEN** the news of a merge names pull requests its landing closes
- **THEN** the session closes each with its comment of the robot, and nothing else
  [@test] crates/operations/src/close_test.rs (`the_closes_of_a_merged_robot_close_what_its_landing_closes_with_a_comment_of_the_robot`, `a_close_closes_an_open_pull_request_with_its_comment_and_nothing_else`)

#### Scenario: A failed close
- **WHEN** a close fails
- **THEN** it is an error toast "Cannot close pull request #5", and the others go on

#### Scenario: The main window shows another repository
- **GIVEN** the main window shows another repository by then
- **WHEN** the robot merged
- **THEN** the closes are of the robot's repository
  [@test] src/stores/patronus.test.ts (`closes the pull requests in the robot's repository when the main window shows another one`)

### Requirement: Merged: The branches a merged robot landed say so until the update

Until the update archives them, the branches a merged robot landed that way (the same view, a robot first seen merged
too: `getLandedBy`) MUST say so in their rows, "Landed with oauth", "Landed with the whole air stack"
(`LandedWithChip`, `info`, beside the status "landed" and the hollow `info` node of a branch in master); the preview
shows it too. The branch the robot ran on landed too: it MUST keep its robot chip, "Merged".

#### Scenario: Who landed the branch
- **WHEN** a branch was landed by a merged robot
- **THEN** its row names who landed it: a branch built on it, the whole stack, or its own robot with the work left
- **AND** the chip's hover says "In master with the
  Safe Push of oauth · the update removes the local copies" and when it merged, a click opens the robot in Patronus
  [@test] src/lib/patronus.test.ts (`names who landed a branch in its row: a branch built on it, the whole stack, its own robot with the work left`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`)

#### Scenario: The view names the landing
- **WHEN** the Patronus view derives which branches merged robots landed
- **THEN** the merged robot that left the least work names the landing, new commits on its commit leave the hashes telling, a robot on an up-to-date remote branch took the branch as it is, and a dry run or a missing source lands nothing
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`, `new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`, `a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`, `a_dry_run_lands_nothing`)
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_missing_source_lands_nothing`)

#### Scenario: A robot first seen merged
- **WHEN** the session first sees a robot merged
- **THEN** the branches it landed are held all the same, and the rest lands without them
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)

#### Scenario: The branch the robot ran on
- **WHEN** the robot merged
- **THEN** the branch it ran on keeps its robot chip, "Merged"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: Merged: The branches a merged robot landed offer no Safe Push and no push

The branches a merged robot landed MUST offer no Safe Push and no Dry Run
([Next Step](../next-step/spec.md#the-table), row 1: "Landed in master with oauth", "Merged by Patronus") and no push
([below](#pushing-into-a-branch-whose-safe-push-runs)); the "to push" counts MUST leave them out.

#### Scenario: Why a landed branch offers no push
- **WHEN** the user looks at the push of a branch a merged Safe Push landed
- **THEN** it is not offered, and says who landed it, its own robot as the merge
  [@test] src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)

#### Scenario: The branches the whole stack landed
- **WHEN** the whole stack merged
- **THEN** its branches offer no Safe Push, Dry Run or push, each saying why, the hover of their chip saying so
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say on every branch of the stack that the whole stack landed it until the update, and offer its Safe Push no more`, `should offer no Safe Push, Dry Run or push of the branches the whole stack landed, each saying why, the hover of their chip saying so`)

### Requirement: Merged: The Landed chip takes the place once the baseline has the commits

Once the fetched baseline has their commits, the Landed chip MUST take its place.

#### Scenario: The fetched baseline has the commits
- **WHEN** the fetched baseline has the commits of a branch a merged robot landed
- **THEN** the Landed chip takes the place of "Landed with oauth"

### Requirement: Merged: After a change below the robot's commit the landing is asked by content

A change below the commit the robot took (a new commit or an amend of a branch it is built on, a reword; for the
whole stack any change of a branch of it) gives the branches new copies, so the hashes no longer tell what landed:
the repository view MUST ask by content (`robot_landings_of_grouped` of `crates/sync-core/src/robot_landing.rs`, the
same for `brd`, [Agent CLI](../../cli/agent-cli/spec.md#brd-status)).

#### Scenario: The tested commits against each branch
- **WHEN** the branches have new copies since the robot's commit
- **THEN** the robot's tested commits (the same commit, a patch-equivalent copy, or a copy with the same changes on a newer base, as the robot commit map compares them) are matched
  against each branch as the table has it
- **AND** a robot at the tip landed the branch and what it is built on, a reworded commit below keeps the branches landed, the robot of the whole stack landed every branch of it, a missing source lands nothing
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_robot_at_the_tip_landed_the_branch_and_what_it_is_built_on`, `a_reworded_commit_below_keeps_the_branches_landed`, `the_robot_of_the_whole_stack_landed_every_branch_of_it`, `a_missing_source_lands_nothing`)

### Requirement: Merged: Commits added after the landed ones are the only work left

A branch whose oldest commits the robot tested and whose other commits were added after them MUST count as landed,
those commits the only work left, its push and Safe Push offered for them. A branch with a commit changed since, or a
new one below a tested one, MUST NOT be landed.

#### Scenario: Commits added after the tested ones
- **GIVEN** the robot tested the oldest commits of a branch and others were added after them
- **WHEN** the row shows
- **THEN** it says "Landed with oauth · 1 commit after it" ("Landed · 1 commit after it" of the robot's own branch), its push and Safe Push offered for those commits
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`)
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_commit_added_below_the_robots_commit_is_the_only_work_left`, `a_commit_not_synced_yet_is_after_the_landed_ones`)
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

#### Scenario: A commit changed since
- **WHEN** a commit of the branch changed since the robot tested it, or a new one came below a tested one
- **THEN** the branch is not landed
  [@test] crates/sync-core/src/robot_landing_test.rs (`an_amended_commit_leaves_its_branch_not_landed`)

### Requirement: Merged: The content is asked once per state of the branches

The landing by content MUST be asked once per state of the branches (`ContentMemo`, keyed by the robot's commit and
the commits of the branches, locked only to look up and to keep answers, never while git runs), on the table the last
sync left.

#### Scenario: The memo while git runs
- **WHEN** git tells the content of a landing
- **THEN** the memo is free: it is locked only to look up and to keep answers
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`the_memo_is_free_while_git_tells_the_content`)

#### Scenario: The processes of a landing by content
- **WHEN** the landing of a merged robot is asked after a change below its commit
- **THEN** it runs within its process budget
  [@test] crates/sync-core/src/process_budget_test.rs (`the_landing_of_a_merged_robot_after_a_change_below_its_commit`)

### Requirement: Merged: Of several robots the one that left the least work counts

Of several robots that landed a branch, the one that left the fewest commits after it MUST count.

#### Scenario: Two merged robots landed a branch
- **WHEN** several merged robots landed a branch
- **THEN** the one that left the fewest commits after it names the landing
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_merged_robot_that_left_the_least_work_names_the_landing`)

### Requirement: Merged: A landed root leaves the pull request of the whole stack open

A merged robot of a branch that landed the root of a stack whose whole stack has an open pull request leaves that pull
request matching nothing: the toast MUST say it, and the user decides on it
([Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app)); nothing MUST close it.

#### Scenario: A piece landed the root
- **GIVEN** the whole stack has the open pull request #12
- **WHEN** a merged robot of a branch landed the root of the stack
- **THEN** the toast says "Safe Push · #12 no longer matches the air stack: oauth landed", and the pull request is never closed by itself
  [@test] src/stores/patronus.test.ts (`says that a piece that landed the root leaves the pull request of the whole stack, never closed by itself`)

### Requirement: Merged: A merge in the background is an OS notification

In the background (no Branch Deck window has the focus), an OS notification MUST say it at once ("auth merged into
master", "Safe Push": what merged, as the toast says it) while "Safe Push merges" is on in Settings → Notifications
([Review Requests](../../repository/review-requests/spec.md#settings), `notifyPatronusMerges`, on by default); the toast
shows on the next focus.

#### Scenario: A merge while no window has the focus
- **WHEN** a robot merged while no Branch Deck window has the focus
- **THEN** an OS notification says it at once, and the toast shows on the next focus
  [@test] src/stores/patronus.test.ts (`tells a merge in the background by an OS notification, and in a toast on focus`)

#### Scenario: Notifications turned off or not allowed
- **WHEN** Settings turned the kind off, or the permission or the plugin is missing
- **THEN** no OS notification is sent and the toast waits for the focus all the same; the permission is asked on the first notification, never at start (`src/lib/os-notification.ts`)
  [@test] src/lib/os-notification.test.ts (`sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same`)

### Requirement: Merged: Fetch of a repository not shown fetches only the baseline

Fetch (of a repository not shown) MUST fetch only the baseline's remote branch, as Update Project does, without
updating the branch, the index or the files (the intent `fetchBaseline`, an operation of the session of the robot's
repository). A failed fetch MUST be an error toast.

#### Scenario: The fetch moves only the tracking ref of the baseline
- **WHEN** the fetch runs
- **THEN** it moves only the tracking ref of the baseline, and a local baseline has nothing to fetch
  [@test] crates/sync-core/src/baseline_fetch_test.rs (`fetch_moves_only_the_tracking_ref_of_the_baseline`)
  [@test] crates/sync-core/src/baseline_fetch_test.rs (`local_baseline_has_nothing_to_fetch`)

#### Scenario: An update during the fetch
- **WHEN** an update starts during the fetch
- **THEN** it goes on from that fetch
  [@test] crates/operations/src/update_test.rs (`an_update_during_a_fetch_goes_on_from_that_fetch`)

### Requirement: Merged: The fetch shows on the Update button of its own repository

Meanwhile the Update button of the robot's repository MUST show the fetch ("Fetching 45%"), never the button of another
repository selected meanwhile; then the sync its session asks MUST check the update against the fetched baseline,
without expanding or scrolling.

#### Scenario: Another repository selected during the fetch
- **WHEN** the user selects another repository while the fetch runs
- **THEN** its Update button never shows the phase of the other repository
  [@test] src/stores/update-project.test.ts (`belongs to its repository: switching repositories never shows the other repository's phase`)

### Requirement: Merged: Branch Deck never updates by itself

Branch Deck MUST NOT update by itself: the update stays a click.

#### Scenario: A robot merged
- **WHEN** a robot of a branch merged
- **THEN** nothing updates until the user clicks

### Requirement: Failing: A running robot with the warning of a failed required check is failing

A robot fails once a required check failed, but Patronus runs its other checks on (for an hour, maybe) and says so
only by a warning problem, `uniqueId` `robot_is_failing` ("This Safe Push will fail because required checks have
already failed"), removed when the robot finishes. A running robot with that warning MUST be failing (asked as in
[Load on Patronus](#load-on-patronus), or read with its details); a canceling one MUST NOT be: the user decided already.

#### Scenario: The details of a running robot have the warning
- **GIVEN** a robot is running
- **WHEN** its details have the warning problem `robot_is_failing`
- **THEN** the robot is failing
  [@test] crates/patronus/src/actions_test.rs (`details_have_the_profile_the_checks_and_the_error_problems`)

#### Scenario: A canceling robot
- **WHEN** a canceling robot has the warning
- **THEN** it is not failing: the user decided already

### Requirement: Failing: A failing robot is still a running one, named Failing

A failing robot MUST still be a running one (its ring, its time, Cancel), named "Failing" in `destructive`: in the
Status cell of its row, the chip, the ring of its node in the stack graph, and the
[landing overview](../../repository/landing-overview/spec.md#overview), a move under "Landing", where its row names
the checks that failed.

#### Scenario: A running Safe Push whose required check failed
- **WHEN** a required check of a running Safe Push failed
- **THEN** the Status cell of its row says "18m · Failing" (accessible name "Safe Push: Failing")
- **AND** the landing overview counts it as a move under "Landing", its row naming the checks that failed: "Failed: Bazel Community Build Aggregator"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Requirement: Failing: Each failing robot is told once in a warning toast

Each failing robot MUST be told once: a `warning` toast "A required check of auth failed" / "Bazel Community Build
Aggregator · the Safe Push will fail" (the checks from its details, which the warning doesn't name; a watched robot
by what it tests), for 10 s, with "Cancel Safe Push" first (the action of the robot, `getRobotActionItems`) and "Open in
Patronus" below the text, recorded in Activity.

#### Scenario: A running robot will fail
- **WHEN** a running robot turns failing
- **THEN** a `warning` toast tells it once with the failed checks, "Cancel Safe Push" first and "Open in
  Patronus"
  [@test] src/stores/patronus.test.ts (`tells once that a running robot will fail, with Cancel Safe Push and the failed checks`)

#### Scenario: Cancel from the toast
- **WHEN** the user clicks "Cancel Safe Push" of the toast
- **THEN** the robot is canceled
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Requirement: Failing: A failing robot in the background is an OS notification

In the background an OS notification MUST say it at once while "Safe
Push failures" is on in Settings → Notifications (`notifyPatronusFailures`, on by default), and the toast shows on the
next focus.

#### Scenario: A robot turns failing while no window has the focus
- **WHEN** a robot turns failing in the background
- **THEN** an OS notification says it at once, and the toast shows on the next focus

### Requirement: Failed: A failed robot is told once its problems are read

A robot that failed MUST be told once, as a merge is (news of the session, `robotFailed`: running or canceling in one
answer, failed in the next; a robot first seen failed is no news), once the session read its problems, which say
whether it failed for missing code-owner approvals; without them when their read failed, and at once for a robot whose
problems the session doesn't read (a watched robot, one of no branch of the table).

#### Scenario: A failure waits for its problems
- **WHEN** a robot known running failed
- **THEN** the session tells it once its problems say why
  [@test] crates/repository-model/src/facts_test.rs (`a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why`)

#### Scenario: The problems didn't come
- **WHEN** the read of the problems of a failed robot failed
- **THEN** the failure is told without them
  [@test] src/stores/patronus.test.ts (`tells a robot whose problems didn't come without them`)

### Requirement: Failed: A failure is an error toast with the next action

The failure MUST be an `error` toast "Safe Push of auth failed" / "Fast Tests · auth → master" (the checks that failed
from its details, else what it tested), with "Start in Air" first and "Copy Prompt for Agent" second, in the place of
"Open in Patronus", or "Restart Safe Push" (the action of the robot, `getRobotActionItems`, where a restart is
possible) when its builds never started
([Safe Push Failure Prompt](../safe-push-failure-prompt/spec.md#the-failed-toast)).

#### Scenario: A running Safe Push failed
- **WHEN** a running Safe Push failed
- **THEN** the `error` toast tells it once with the failed checks, and its first action, "Start in Air", starts a session in the IDE, with "Copy Prompt for Agent" as the second option
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and start a session in the IDE from the toast, with Copy Prompt for Agent as the second option`)
  [@test] src/stores/patronus.test.ts (`tells once that a robot failed, with Start in Air and Copy Prompt for Agent beside it, and the failed checks, once its problems are read`)

#### Scenario: Its builds never started
- **WHEN** a Safe Push failed whose builds never started
- **THEN** the toast offers "Restart Safe Push", which restarts it, and "Open in Patronus" below the text
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart from the toast a Safe Push whose builds never started`)

#### Scenario: Start in Air is not offered
- **WHEN** a Safe Push failed and the robot offers no "Start in Air"
- **THEN** the toast offers "Copy Prompt for Agent" first, and "Open in Patronus" below the text

### Requirement: Failed: A failure for missing approvals is a warning that it needs review

For missing approvals the toast MUST be a `warning` toast "Safe Push of auth needs review" / "The code owners did not
approve the changes", with "Open Pull Request", or "Create Pull Request…" for a pushed branch without one: the
approvals count in the pull request, a restart before them fails again.

#### Scenario: A robot failed for missing approvals
- **WHEN** a robot failed for missing code-owner approvals
- **THEN** the `warning` toast says "needs review", with its pull request
  [@test] src/stores/patronus.test.ts (`tells a robot that failed for missing approvals "needs review", with its pull request`)

#### Scenario: A robot started with a profile
- **WHEN** a robot started with a profile failed for missing approvals
- **THEN** the toast says "Large Scale Change doesn't skip the code-owner review: the code owners
  did not approve the changes", what a [Safe Push without review](#safe-push-without-review) learns only now
  [@test] src/stores/patronus.test.ts (`names the profile of a robot started without the approvals whose profile checks them all the same`)

### Requirement: Failed: A failure is recorded in Activity and notified in the background

A failure MUST be recorded in Activity, and in the background the OS notification of "Safe Push failures"
(`notifyPatronusFailures`) MUST say it, the toast on the next focus.

#### Scenario: The failure is recorded
- **WHEN** a running Safe Push failed
- **THEN** Activity records it
  [@test] tests/e2e/patronus/patronus.spec.ts (`should tell when a running Safe Push failed, record it and start a session in the IDE from the toast, with Copy Prompt for Agent as the second option`)

#### Scenario: A failure in the background
- **WHEN** a robot failed while no window has the focus
- **THEN** the OS notification of "Safe Push failures" says it, and the toast shows on the next focus

### Requirement: Failed: The issue keys of the failure toast link to the tracker

The issue keys of the toast's title and description MUST link to the tracker.

#### Scenario: An issue key in the title and the description
- **WHEN** the toast of a failure names an issue key
- **THEN** "Safe Push of JIRA-456 failed" / "JIRA-456 → master" link it to the tracker

### Requirement: Failed: A robot told failing while it ran is told no second time

A robot told failing while it ran ([above](#failing)) MUST be told no second time: its Activity entry says it failed,
and its toast, while still open, says it in place, with the step of the failure.

#### Scenario: Its toast is open
- **GIVEN** a robot was told failing and its toast is open
- **WHEN** the robot failed
- **THEN** its toast says in place that it failed
  [@test] src/stores/patronus.test.ts (`tells no second time a robot told failing while it ran: its toast, while open, says in place that it failed`)

#### Scenario: Its toast is gone
- **GIVEN** a robot was told failing and its toast is gone
- **WHEN** the robot failed
- **THEN** nothing is told, and Activity says it failed
  [@test] src/stores/patronus.test.ts (`tells nothing of a robot told failing whose toast is gone: Activity says it failed`)

### Requirement: Failed: The toast of the failure takes the place of the start's toast

The toast of the failure MUST take the place of the toast of the robot's start. The watched robot of a reviewed pull
request MUST be told by what it tests.

#### Scenario: The toast of the start is open
- **GIVEN** the toast of the robot's start is open
- **WHEN** the robot failed
- **THEN** the toast of the failure takes its place

#### Scenario: The watched robot of a reviewed pull request
- **WHEN** the watched robot of a reviewed pull request failed
- **THEN** its toast names it by what it tests

### Requirement: Failed: The step of a failing or failed toast is the robot's step now

The step of a failing or failed toast MUST be the step of the robot as the stores have it, rendered again on every
change of the robots, the branches, the running operations and the shown repository
([One toast per event](../../../../docs/contributing/ux-writing.md#one-toast-per-event)), and a click MUST run the step
shown now. The pull request step opens what the branch has at the click.

#### Scenario: The robot was canceled elsewhere
- **WHEN** the robot finished or was canceled
- **THEN** Cancel Safe Push is dropped from the toast, in place, and the toast keeps the robot page
  [@test] src/stores/patronus.test.ts (`drops Cancel Safe Push once the robot was canceled elsewhere, in place: the toast keeps the robot page`)

#### Scenario: A step that runs or is disabled
- **WHEN** the step of the robot runs, or is disabled
- **THEN** a step that runs is disabled, and a disabled one is not offered

#### Scenario: A robot of another repository
- **WHEN** the robot is of another repository than the shown one
- **THEN** the toast offers no step of a branch (the branch of the same name there is another one)

### Requirement: Failed: A restart made elsewhere turns the toast into an info toast

A restart made here MUST take the place of the toast; one made elsewhere (a newer robot of the same subject and push
mode in the next list) MUST turn it into an `info` toast "Safe Push of auth was restarted" / "auth → master" with
"Open in Patronus" of the newer robot.

#### Scenario: Restarted elsewhere
- **WHEN** the next list has a newer robot of the same subject and push mode
- **THEN** the toast says in place that the robot was restarted, and opens the newer one
  [@test] src/stores/patronus.test.ts (`says in place that the robot was restarted elsewhere, and opens the newer one`)

### Requirement: Failed: A state of an older robot never replaces the toast of a newer one

A state of an older robot MUST NOT take the place of the toast of a newer robot of its subject: the failure of the
robot a start replaced, told after the start's toast (a robot that finished while the start checked), leaves "Safe
Push started" as it is, and Activity records the failure.

#### Scenario: The replaced robot finished while the start checked
- **GIVEN** a start replaces a robot that finished while the start checked
- **WHEN** the failure of the replaced robot is told after the start's toast
- **THEN** "Safe Push started" stays as it is, and Activity records the failure
  [@test] src/stores/patronus.test.ts (`leaves the toast of the start that replaced a robot whose failure is told after it`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push that replaces a robot which finished meanwhile, without a cancel`)

### Requirement: Code-owner review: Patronus fails a Safe Push that misses the code owners' approvals

Patronus fails a Safe Push whose changes miss the approvals the code owners require (`.ownership/review-rules.yaml`
of the target branch, PAT-A-109): the problem "Changes require review approval". The approvals SHALL count only in the
review linked to the robot: the pull request whose comment started it, else the Space merge request of the start. A
dry run is not checked.

#### Scenario: A Safe Push without the approvals
- **GIVEN** the changes of a branch need approvals of the code owners
- **WHEN** its Safe Push runs without them in the review linked to the robot
- **THEN** Patronus fails it with the problem "Changes require review approval"
- **AND** a dry run is not checked

### Requirement: Code-owner review: Patronus is the only judge of whose approvals count

Patronus MUST be the only judge of whose approvals count; the app and `brd` MUST ask it and render its answer
([Git Operations](../../../../docs/design/git-operations.md#patronus-review-pre-check)).

#### Scenario: The app and brd show whose approvals count
- **WHEN** the app or `brd` shows whose approvals count
- **THEN** it renders the answer of Patronus, judging nothing itself

### Requirement: Code-owner review: A Safe Push checks the review before the start

A Safe Push MUST check the review before the start (never a dry run, never for the chip) with the readiness the session
of the repository reads ([Next Step](../next-step/spec.md#the-readiness-store): what a surface needs at once in a call
of its own, the rest a queue of at most 4 at a time after the answers the object cache keeps), with the core
`brd safe-push` shares (`remote_facts::review_readiness`).

#### Scenario: The commits and their changed paths
- **WHEN** the readiness of branches is asked
- **THEN** it takes the commits of the branches at the tips of the session's view (an answer is keyed by its branch and tip), from one `git log --topo-order` of all the branches of a call (a
  stacked branch shares its parents' commits)
- **AND** their changed paths come from the `commit_files` store of the object cache (one `diff-tree --stdin` for the commits it misses; a
  rename is a delete and an add, a deleted path counts, a merge has none), then one
  `POST /pre-merge-approval-required-check` per branch whose kept answer doesn't hold
  [@test] crates/remote-facts/src/patronus_review_test.rs (`reads_the_commits_of_the_branch_with_their_changed_paths`)
  [@test] crates/remote-facts/src/patronus_review_test.rs (`reads_several_branches_in_one_log`)

#### Scenario: A branch with too many commits
- **WHEN** a branch has 500 commits or more
- **THEN** its check is skipped, as the IDE does
  [@test] crates/remote-facts/src/patronus_review_test.rs (`does_not_read_a_branch_with_too_many_commits`)

#### Scenario: The kept answers and the session's ask
- **WHEN** a window needs the readiness
- **THEN** the session asks it from Patronus, and the answers the object cache keeps come without asking Patronus
  [@test] crates/remote-facts/src/review_readiness_test.rs (`the_kept_answers_come_from_the_cache_without_asking_patronus`)
  [@test] crates/test-server/src/review_readiness_test.rs (`the_session_asks_the_readiness_a_window_needs_from_the_fake_patronus`)

#### Scenario: A dry run
- **WHEN** a dry run starts
- **THEN** the review is not checked
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should not check the review of a dry run`)

### Requirement: Code-owner review: The check takes the pull request the robot would start from

The check MUST take the pull request the robot would start from (`githubPullRequest`, `ReviewStart::PullRequest`: the
open pull request into the target), and Patronus evaluates its reviews as that robot would: every group the changes
need with `approved` and its counts (`requiredApprovals`, `collectedApprovals`, `selfApprovalAllowed`), every member
with its `approvalState`.

#### Scenario: A check with a pull request
- **WHEN** the branch has an open pull request into the target
- **THEN** the check sends it and reads the evaluation of Patronus
  [@test] crates/patronus/src/review_test.rs (`a_check_with_a_pull_request_sends_it_and_reads_the_evaluation`)

### Requirement: Code-owner review: The readiness only arranges the answer of Patronus

The readiness (`patronus::review_readiness`) MUST only arrange that answer: `notRequired`; `approved`; `missing`;
`unknown`; `unavailable`.

#### Scenario: Not required, or approved
- **WHEN** Patronus answers that no approval is required, or the groups approved
- **THEN** the readiness is `notRequired` whatever the groups say, or `approved`, with the members whose approval counts
  [@test] crates/patronus/src/review_readiness_test.rs (`not_required_by_patronus_is_not_required_whatever_the_groups_say`, `a_self_approval_counts_where_the_group_allows_it`)

#### Scenario: Approvals are missing
- **WHEN** groups are still short of approvals
- **THEN** the readiness is `missing`: the groups still short of approvals, each with how many more count, who approved already, who is
  requested and hasn't approved, who requested changes and whom Patronus suggests
- **AND** the approvers of the pull request no group counts (`notCounted`, a bot or a colleague outside the groups)
  [@test] crates/patronus/src/review_readiness_test.rs (`a_bot_approval_is_not_counted_and_the_requested_owner_is_awaited`, `several_groups_partly_approved_list_the_missing_ones`, `a_group_that_needs_two_approvals_with_one_needs_one_more`)

#### Scenario: The reason of the missing approvals
- **WHEN** the approvals are missing
- **THEN** the reason is `noEligibleReviewer` when a missing group has nobody whose approval could count, else `approvalsMissing`
  [@test] crates/patronus/src/review_readiness_test.rs (`a_group_whose_members_cannot_approve_has_no_eligible_reviewer`)

#### Scenario: Requested changes and requests again
- **WHEN** a member requested changes, or was asked again
- **THEN** changes requested by a member of an
  approved group don't block, as Patronus doesn't block on them, nor by a member asked again since (the wait for
  them); a re-requested approval counts as Patronus says
  [@test] crates/patronus/src/review_readiness_test.rs (`a_re_requested_approver_counts_as_patronus_says`, `a_member_re_requested_after_changes_is_awaited`)

#### Scenario: Required with no group short of approvals
- **WHEN** Patronus answers required, with no group short of approvals
- **THEN** the readiness is `unknown`
  [@test] crates/patronus/src/review_readiness_test.rs (`required_without_a_missing_group_is_unknown`)

#### Scenario: A branch too long, or no answer
- **WHEN** the branch is too long to check (`skipped`), or Patronus doesn't answer
- **THEN** the readiness is `unknown`, or `unavailable`
  [@test] crates/patronus/src/review_readiness_test.rs (`a_skipped_branch_is_unknown`)

### Requirement: Code-owner review: A start through the API counts no approval of a pull request

A start through the API (`ReviewStart::Rest`: no pull request, or one into another base) MUST count no approval of a
pull request: the check runs without one and its answer is the verdict, `missing` with `noPullRequestContext`.

#### Scenario: No pull request, or one into another base
- **WHEN** the branch starts through the API
- **THEN** the check runs without the pull request, and its answer is the verdict, `missing` with `noPullRequestContext`
  [@test] crates/patronus/src/review_readiness_test.rs (`a_rest_start_counts_no_pull_request`)
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_rest_start_is_checked_without_the_pull_request`)

### Requirement: Code-owner review: An answer without the evaluation is certain only without approvals

Until Patronus evaluates pull requests (an older server, or its flag `review.precheck.github.pull.request.enabled`
off: the field is ignored), the answer has no `approved`: without any approval in the pull request the approvals MUST
be `missing` for certain; with one, nobody but Patronus can tell whether it counts: `unknown`
(`approvalsNotEvaluated`), and the robot decides.

#### Scenario: An answer without the evaluation
- **WHEN** the answer of Patronus has no `approved`
- **THEN** the approvals are `missing` without any approval in the pull request, else `unknown` (`approvalsNotEvaluated`)
  [@test] crates/patronus/src/review_readiness_test.rs (`an_answer_without_the_evaluation_is_certain_only_without_approvals`)

### Requirement: Code-owner review: The answer of the pre-check is kept by what Patronus decides on

The pre-check is the heaviest call of the API: its answer MUST be kept on disk, in the `review_answer.v1` store of the
object cache the app and `brd` share, keyed by a hash of what Patronus decides on: the Patronus server and token, the
Space repository, the target, the sorted changed paths, and with a pull request its number, its author and its
reviews (login, state, request).

#### Scenario: A sync onto a new baseline, and a new review
- **WHEN** a sync goes onto a new baseline, or a new review comes
- **THEN** a sync onto a new baseline keeps the key; a new review changes it, and Patronus is asked once
  [@test] crates/remote-facts/src/review_readiness_test.rs (`the_answer_about_a_pull_request_is_kept_until_a_review_changes`)

### Requirement: Code-owner review: A kept answer holds by what it says

An answer with the evaluation of the pull request MUST hold 24 hours; one without it 2 minutes while approvals are
missing, 10 without. Recheck MUST ask at once and keep the new answer; errors MUST NOT be kept.

#### Scenario: With and without the evaluation
- **WHEN** an answer is kept
- **THEN** one with the evaluation holds 24 hours, one without it minutes
  [@test] crates/remote-facts/src/review_readiness_test.rs (`an_answer_with_the_evaluation_holds_a_day_one_without_it_minutes`)

#### Scenario: A failed check
- **WHEN** the check fails
- **THEN** the readiness is unavailable and nothing is kept
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_failed_check_is_unavailable_and_not_kept`)

### Requirement: Code-owner review: A check waits while Patronus backs off

A check without Recheck MUST NOT be sent while Patronus backs off after a failure (one backoff with the robot lists),
so a 503 ("The review check of Patronus is unavailable right now") holds back the rest of a batch.

#### Scenario: An unavailable check
- **WHEN** the check answers 503
- **THEN** the background checks back off, and the rest of the batch is held back
  [@test] crates/patronus/src/review_test.rs (`an_unavailable_check_backs_off_the_background_checks`)

### Requirement: Code-owner review: A tip gone since makes only its branch unavailable

A tip gone since the page sent it fails the one `git log` of the batch: each tip MUST be read alone then, and only its
branch is unavailable.

#### Scenario: A tip gone since the page sent it
- **WHEN** a tip of the batch is gone since the page sent it
- **THEN** each tip is read alone, and only its branch is unavailable
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_tip_gone_since_the_page_sent_it_makes_only_its_branch_unavailable`)

### Requirement: Code-owner review: A repository only on Space asks in the cheap order

A repository only on Space MUST ask in the cheap order: first without a merge request (Patronus asks Space nothing);
only when approvals are missing, the open merge request of the branch (`GET /space/merge-requests`), then the check
with it.

#### Scenario: No approval is needed
- **WHEN** the check without a merge request needs no approval
- **THEN** no merge request is looked up
  [@test] crates/patronus/src/review_test.rs (`no_merge_request_is_looked_up_when_no_approval_is_needed`)

#### Scenario: Approvals are missing
- **WHEN** the check without a merge request says approvals are missing
- **THEN** it names them and checks again with the open merge request of the branch
  [@test] crates/patronus/src/review_test.rs (`names_the_missing_approvals_and_checks_again_with_the_merge_request`)

### Requirement: Code-owner review: The answers of a repository only on Space are kept in memory

Its answers MUST be kept in memory by the commits checked (the tip and the oldest commit): "no approval needed" for 10
minutes, "approvals missing" for 2 minutes, Recheck asks at once. A found merge request is kept for the session, "no
merge request" for 5 minutes (a recheck asks again). The answers kept in memory are bounded per kind (the least used
go first).

#### Scenario: A recheck of a required review
- **WHEN** the user rechecks a required review
- **THEN** it is asked again at once
  [@test] crates/patronus/src/review_test.rs (`a_required_review_is_asked_again_on_a_recheck`)

### Requirement: Code-owner review: An unavailable check on Space is not asked again by itself

An unavailable check (503) MUST be said and MUST NOT be asked again by itself.

#### Scenario: The check of a repository only on Space is unavailable
- **WHEN** the review check answers 503
- **THEN** it is said, and not asked again by itself
  [@test] crates/patronus/src/review_test.rs (`an_unavailable_review_check_is_not_asked_again_by_itself`)

### Requirement: Code-owner review: `brd safe-push` refuses a start whose approvals are missing

`brd safe-push` MUST check the same review before a Safe Push, always asked again (the answer is kept for the app), and
`missing` refuses its start (`approvals_missing`) unless `--without-approvals`: an agent never sends a robot that
fails for them. `unknown` lets it start, as the robot decides.

#### Scenario: Patronus judges the approvals of the pull request
- **WHEN** `brd safe-push` starts a branch whose approvals are missing
- **THEN** it refuses the start (`approvals_missing`) unless `--without-approvals`
  [@test] crates/bd-remote/src/safe_push_test.rs (`patronus_judges_the_approvals_of_the_pull_request`)

### Requirement: Code-owner review: `brd safe-push` refuses an unavailable review until told

`unavailable` (the pre-check failed) is no answer: it MUST refuse the start as `review_unknown` until the user says to
start anyway (`--allow-unknown-review`, the decision's confirmation `ReviewUnknown`).

#### Scenario: Patronus cannot answer the review
- **WHEN** the pre-check of `brd safe-push` failed
- **THEN** the start is refused as `review_unknown`, unless `--allow-unknown-review`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_review_patronus_cannot_answer_refuses_the_safe_push_unless_allowed`)

### Requirement: Code-owner review: `brd safe-push` collects the approvals in the GitHub pull request

`brd safe-push` MUST collect the approvals in the GitHub pull request (`--create-pull-request`), never a Space merge
request ([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)).

#### Scenario: Approvals to collect
- **WHEN** `brd safe-push --create-pull-request` runs for a branch whose approvals are missing
- **THEN** the approvals are collected in the GitHub pull request, never a Space merge request

### Requirement: Code-owner review: Branch Deck never creates a Space merge request

Space merge requests are deprecated: the approvals go into the GitHub pull request of the branch, and Branch Deck MUST
NOT create a merge request. One Patronus finds open still counts: the start links it.

#### Scenario: A branch whose approvals are missing
- **WHEN** the approvals of a branch are missing
- **THEN** they go into the GitHub pull request of the branch, and no merge request is created
- **AND** a merge request Patronus finds open still counts: the start links it

### Requirement: Code-owner review: One rule says where the missing approvals count

Where the approvals a start misses count MUST be one rule of the app and `brd` (`domain_policy::review::review_state`):
the pull request the robot starts from, else the GitHub pull request the branch doesn't have yet; a repository only on
Space is read once there, whatever reason the answer gives: the merge request Patronus found open, else nowhere known
(its GitHub repository unknown). No answer is pending, never missing.

#### Scenario: Where the approvals count
- **WHEN** a start misses approvals
- **THEN** the review says where they count, and a repository only on Space is read once
  [@test] crates/domain-policy/src/review_test.rs (`the_review_says_where_the_missing_approvals_count_and_reads_a_repository_only_on_space_once`)

### Requirement: Code-owner review: A repository only on Space refuses in its popover a start whose approvals are missing

A repository only on Space (no GitHub repository of the branches) keeps the merge request path: its slot MUST NOT be
held back (merge requests are deprecated and the robot decides, [Next Step](../next-step/spec.md#the-table)), but the
click on the start of one whose approvals are required MUST open its popover, which names them and refuses the start a
robot would fail (as `brd`'s `approvals_missing`). No button creates anything.

#### Scenario: A merge request Patronus found open
- **GIVEN** the approvals of a branch of a repository only on Space are required and Patronus found its merge request IJ-CR-7 open
- **WHEN** its popover opens
- **THEN** it says "Needs review by the code owners: a Safe Push fails
  without these approvals, which count in IJ-CR-7.", a line per group with the available reviewers Patronus suggests
  and its files ("Air Plugin Release Team (suggested: Alice) · 2 files", "· all files" for a branch-wide group), and
  "Open IJ-CR-7"
- **AND** its start is refused while the approvals are missing, and starts with the merge request once Patronus finds them
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should name the approvals a Safe Push needs on Space, and start with the merge request found open`)

#### Scenario: No merge request
- **GIVEN** the approvals of a branch of a repository only on Space are required and it has no merge request
- **WHEN** its popover opens
- **THEN** it says "… They count in the GitHub pull request of the branch, and its GitHub
  repository is unknown." with
  "Set GitHub Repository…", which opens Settings → Landing on the tab of the repository at the field of
  `branchdeck.githubRepository` ([Settings](../../repository/settings/spec.md#landing)); "Review Changes" beside it
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)

### Requirement: Code-owner review: Each reviewer group names the files it has to review

Each group MUST name the files it has to review: `paths` and `branchWide` of the group when Patronus sends them (a
branch-wide group comes without `paths`: every file of the branch is its), else the changed paths whose owner by the
`OWNERSHIP` files of the baseline has the name of the group; a group Patronus delegates to, or names as the reviewer
of another owner, stays without files.

#### Scenario: Patronus sent no paths
- **WHEN** Patronus sends a group without paths
- **THEN** the `OWNERSHIP` files of the baseline give the group its changed paths
  [@test] crates/sync-core/src/patronus_test.rs (`the_ownership_files_give_the_paths_to_the_groups_patronus_sent_without_them`)

#### Scenario: A branch-wide group
- **WHEN** Patronus sends a branch-wide group
- **THEN** it comes without `paths` and reviews every file of the branch
  [@test] crates/patronus/src/review_test.rs (`a_branch_wide_group_comes_without_paths_and_reviews_every_file`)

### Requirement: Code-owner review: Review Changes reviews the branch with the groups as filters

"Review Changes" of the popover (and D on the row) MUST review the branch in its review window, on the whole stack,
the groups as filters ([Branch Review](../branch-review/spec.md#the-files-of-each-reviewer-group-safe-push-popover)).

#### Scenario: Review Changes of the popover
- **WHEN** the user clicks "Review Changes" of the popover, or presses D on the row
- **THEN** the branch opens in its review window, the groups as its filters
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say on Space that the approvals count in the GitHub pull request, and open Settings at its repository`)

### Requirement: Code-owner review: A start reads the review again when the local tip changes

A start MUST read the review again when the local tip of the branch changes; until the new answer arrives the start
waits.

#### Scenario: After an unstack
- **GIVEN** a stacked branch was unstacked
- **WHEN** its popover opens
- **THEN** it reads the review of the branch alone, no longer that of its former parents
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone`)

### Requirement: Code-owner review: The UI names the system the approvals count in

The UI MUST name the system the approvals count in, and a review button the system it creates or opens in.

#### Scenario: The line of a group
- **WHEN** the popover names the approvals a Safe Push needs
- **THEN** it names the group and the available reviewers Patronus suggests, and the system the approvals count in
  [@test] src/lib/patronus.test.ts (`names the group and the available reviewers Patronus suggests`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should name the approvals a Safe Push needs on Space, and start with the merge request found open`)

### Requirement: Code-owner review: The start links an open merge request of the branch

The start MUST link an open merge request of the branch (`spaceReview`), so its approvals count.

#### Scenario: A branch with an open merge request
- **GIVEN** a branch has an open merge request
- **WHEN** it starts
- **THEN** the start links the merge request, and its approvals count

### Requirement: Code-owner review: On GitHub no Space merge request is looked up

When the branches are reviewed on GitHub ([above](#pull-requests)), no Space merge request MUST be looked up: the
pre-check runs with the pull request the robot starts from (above), and the "Create pull request" dialog asks it
without one, for the code owners of the changes.

#### Scenario: The dialog asks the code owners
- **GIVEN** the branches are reviewed on GitHub and a branch has no pull request
- **WHEN** the "Create pull request" dialog opens
- **THEN** it asks the pre-check without a pull request, for the code owners of the changes
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer to create the pull request the approvals count in, and no start Patronus would fail`, `should ask the code owner Patronus names, and the row then awaits the review in the pull request`)

### Requirement: Code-owner review: Missing approvals are the steps of the next step

The readiness MUST be the next step's ([Next Step](../next-step/spec.md)): missing approvals are its steps (Create Pull
Request…, Request Review…, the wait, Changes Requested), and the start MUST be refused with their reason.

#### Scenario: The approvals are missing
- **WHEN** the approvals of a branch are missing
- **THEN** its step is one of Create Pull Request…, Request Review…, the wait, Changes Requested, and the start is refused with their reason
  [@test] tests/e2e/next-step/next-step.spec.ts (`should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing`)

### Requirement: Code-owner review: The popover says the readiness in one line

The popover, when it opens, MUST say the readiness in one line.

#### Scenario: Not required
- **WHEN** the readiness is `notRequired`
- **THEN** the line is "No code-owner review needed."
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start the Safe Push of a branch that needs no review with a click, and offer Create Pull Request… in its menu`)

#### Scenario: Approved
- **WHEN** the readiness is `approved`
- **THEN** the line is a `success` line "Approved by Ilya Korennoy for IJPL
  Code Platform."
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`)

#### Scenario: Unknown or unavailable
- **WHEN** the readiness is `unknown` or `unavailable`, which confirm first
- **THEN** the line is an `info` line naming the approvals nobody but Patronus can judge
  yet: "Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval.", or
  "Patronus didn't answer the review check (…): the robot checks the approvals when it starts."
  [@test] tests/e2e/next-step/next-step.spec.ts (`should say in the popover that Patronus doesn't tell whose approvals count, and refuse the start once they are missing`, `should keep Safe Push while Patronus doesn't tell whose approvals count, and say so before it starts`)

#### Scenario: Missing with a profile that skips the review
- **WHEN** the readiness is `missing` with a profile known to skip the review or [without review](#safe-push-without-review)
- **THEN** the line is a `warning`
  "Starts without the approvals of IJPL Code Platform: Large Scale Change skips the code-owner review." with "Review
  Changes"

#### Scenario: GitHub doesn't list the pull requests
- **WHEN** GitHub doesn't list the pull requests
- **THEN** the line is "GitHub doesn't list the pull requests of the repository: Patronus checks
  the approvals when the robot starts."

### Requirement: Code-owner review: Once the pull request is created the start comments on it

Once the pull request is created, the start MUST comment on it: a `/patronus merge`.

#### Scenario: The pull request was created
- **GIVEN** the pull request of a branch was created and its code owner approved
- **WHEN** the user starts its Safe Push
- **THEN** the start comments `/patronus merge` on the pull request
  [@test] tests/e2e/next-step/next-step.spec.ts (`should offer Safe Push once the code owner approved, name whose approval counts, and start it from the pull request with a click`)

### Requirement: Code-owner review: The Patronus group of the branch menu creates no merge request

The Patronus group of the branch menu MUST create no merge request: "Create Pull Request…" of its history group opens
the pull request the approvals count in.

#### Scenario: The branch menu
- **WHEN** the user opens the "⋯" menu of a branch
- **THEN** its Patronus group creates no merge request, and "Create Pull Request…" of its history group opens the pull request the approvals count in
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)

### Requirement: Code-owner review: A robot that failed for missing approvals needs review

A robot that failed for missing approvals (its problems are read once per session, for the failed robots of the
branches) MUST be "Needs review" in `warning`; its popover MUST say how to go on and offer "Safe Push" instead of
Restart, and a robot of a pull request MUST offer Restart instead.

#### Scenario: The popover of a robot that failed for missing approvals
- **WHEN** the popover of a robot that failed for missing approvals opens
- **THEN** its popover says how to go on and offers "Safe Push" instead of Restart:
  "The code owners did not approve the changes. Get their approvals in the pull request of the branch, then start a
  new Safe Push from it: a restart has no pull request to count them in." A robot of a pull request offers Restart instead: "The code owners
  did not approve the changes in pull request #7. Restart once they approve: the restart counts the approvals the pull
  request has by then.", "#7" a link that opens the pull request
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] src/lib/patronus.test.ts (`names a robot that failed for missing approvals "Needs review"`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the pull request a robot that failed for missing approvals was started from, from its details`)

#### Scenario: The problems of a failed robot are asked until known
- **WHEN** a robot of the table failed
- **THEN** its problems are read once per session, for the failed robots of the branches
  [@test] crates/repository-model/src/facts_test.rs (`the_problems_of_a_failed_robot_of_the_table_are_asked_until_known`)
  [@test] crates/remote-facts/src/scheduler_test.rs (`an_idle_poll_asks_one_request_and_problems_only_for_changed_robots`, `a_change_of_the_table_asks_the_problems_of_a_failed_robot_it_matched_anew`)

### Requirement: Code-owner review: Robots and their problems are read in the session of their repository

The reads of the robots and of their problems MUST be of the session of the repository: another repository lists its
robots and asks why its failed robot failed while a list of the first is held.

#### Scenario: A list of the first repository is held
- **GIVEN** a list of the robots of one repository is held
- **WHEN** another repository lists its robots
- **THEN** it lists them and asks the problems of its failed robot
  [@test] crates/remote-facts/src/scheduler_test.rs (`another_repository_lists_and_asks_its_problems_while_the_list_of_the_first_is_held`)

### Requirement: The readiness of a request: A pull request a review window shows is asked once its head is fetched

The readiness of pull request #n MUST be the fact `request:<n>` of the session, which asks Patronus about it as the
robot started from it would decide. It is asked while a review window shows the pull request (`Demand.reviewWindows`),
needed now, once both hold: its base is the default branch of the repository, and the head the review requests name
(`headRefOid`) is the one the review fetch of the window brought (`refs/branch-deck/review/<n>/head`).

#### Scenario: A review window shows a pull request
- **GIVEN** a review window shows a pull request into the default branch
- **WHEN** the review fetch of the window brought the head the review requests name
- **THEN** the session asks the readiness `request:<n>`, needed now, and the record goes with the window
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_pull_request_a_review_window_shows_is_asked_once_its_head_is_fetched_and_goes_with_the_window`)

### Requirement: The readiness of a request: The ask is about the head branch of the pull request

Its ask MUST be that head, from its pull request (`ReviewStart::PullRequest`), with its author and its reviewers as the
review requests list them (their latest reviews, then the users asked without one); Patronus is asked about its head
branch (`pullRequestHead`), a branch of no prefix, and its answer comes back under `request:<n>`.

#### Scenario: A pull request of someone else
- **WHEN** the readiness of a pull request of someone else is asked
- **THEN** Patronus is asked about it as its head branch, with its author and its reviewers as the review requests list them
- **AND** the answer comes back by its number, under `request:<n>`
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_pull_request_of_someone_else_is_asked_about_as_its_head_branch`)
  [@test] crates/remote-facts/src/review_requests_test.rs (`maps_the_review_requests_of_the_search`)
  [@test] crates/test-server/src/review_readiness_test.rs (`the_fake_patronus_answers_a_pull_request_of_someone_else_by_its_number`)

### Requirement: The readiness of a request: The answer is kept by the pull request, its author and its reviews

Its answer MUST be kept as a branch's (above): the key holds the pull request, its author and its reviews, so a new
head waits for its fetch and is asked again, and a new review asks again; an approval it tells is no news (the review
requests tell their own).

#### Scenario: A new head or a new review
- **WHEN** the pull request has a new head, or a new review
- **THEN** a new head waits for its fetch and is asked again, and a new review asks again
  [@test] crates/remote-facts/src/review_readiness_test.rs (`a_pull_request_of_someone_else_is_asked_about_as_its_head_branch`)
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_pull_request_a_review_window_shows_is_asked_once_its_head_is_fetched_and_goes_with_the_window`)

### Requirement: The readiness of a request: Nothing is asked for another base, before the fetch, or after the close

Nothing MUST be asked for a pull request into another branch, before its fetch, or once its window closed: its record
goes with the window, a fetch that ends after the close asks nothing, and a frame of the session never seeds it.

#### Scenario: The window closed
- **WHEN** the review window of the pull request closed
- **THEN** its record goes with the window, and a fetch that ends after the close asks nothing
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_pull_request_a_review_window_shows_is_asked_once_its_head_is_fetched_and_goes_with_the_window`)

#### Scenario: Another base, or no fetch yet
- **WHEN** the pull request goes into another branch, or its head is not fetched yet
- **THEN** nothing is asked, and a frame of the session never seeds its readiness

### Requirement: The readiness of a request: A new head has no answer until Patronus answered about it

The answer of an earlier head MUST NOT be kept for a new one: from the moment the listing names a head the review
fetch has not brought, the readiness of the request has no answer (so no groups in the view), until the fetch brought
it and Patronus answered; when that fetch fails, it stays without one and the chips come from the `OWNERSHIP` files
alone.

#### Scenario: The listing names a new head
- **WHEN** the listing names a head the review fetch has not brought
- **THEN** the readiness of the request has no answer, and the view no groups, until the fetch brought it and Patronus answered

#### Scenario: The fetch of the new head fails
- **WHEN** the fetch of the new head fails
- **THEN** the readiness stays without an answer, and the chips come from the `OWNERSHIP` files alone

### Requirement: The readiness of a request: Only the answer of the current ask applies

Only the answer of the request's ask now MUST apply: one of an earlier head, or of earlier reviews of the same head,
that comes late is dropped, before or after the ask of the new head fails.

#### Scenario: A late answer of an earlier head
- **WHEN** the answer of an earlier head, or of earlier reviews of the same head, comes late
- **THEN** it is dropped and never becomes the readiness of the pull request
  [@test] crates/repository-model/src/readiness_facts_test.rs (`a_late_answer_of_an_earlier_head_of_a_pull_request_never_becomes_its_readiness`)

### Requirement: The readiness of a request: A pull request back in a review window is asked again

A pull request that comes back to a review window (the window shows another one and returns to it, or closes and
opens it again) MUST be asked again with the same inputs: the schedule of the session forgets what it read of
`request:<n>` once the key leaves the facts (`scheduler_readiness.rs`), and an ask that ends after it left records
nothing. A branch's readiness is remembered as before.

#### Scenario: The window returns to the pull request
- **GIVEN** a review window showed a pull request, then another one
- **WHEN** it returns to the pull request, or closes and opens it again
- **THEN** the readiness is asked again with the same inputs, and the cache of the answers (`review_answer`) still saves the call to Patronus
  [@test] crates/remote-facts/src/scheduler_test.rs (`asks_a_pull_request_of_a_review_window_again_once_it_comes_back`)

### Requirement: The readiness of a request: The view gives a pull request row its reviewer groups

The view MUST give the row `pull:<n>` its reviewer groups (`reviewGroups`, `domain_policy::reviewer_groups`): every
group the changes need, in the order of Patronus, with its paths, `branchWide`, `approved` (Patronus's evaluation,
else whether the readiness still misses it) and `yours`.

#### Scenario: A review window shows a pull request whose changes need approvals
- **WHEN** a review window shows a pull request whose changes need approvals
- **THEN** the view has the reviewer groups of the pull request
  [@test] crates/repository-model/src/review_view_test.rs (`the_view_has_the_reviewer_groups_of_a_pull_request_a_review_window_shows`)

#### Scenario: Patronus didn't evaluate the pull request
- **WHEN** the answer has no evaluation of the pull request
- **THEN** a group is `approved` as the readiness names it: whether it still misses it
  [@test] crates/domain-policy/src/reviewer_groups_test.rs (`a_group_without_the_evaluation_of_the_pull_request_is_approved_as_the_readiness_names_it`)

### Requirement: The readiness of a request: A group is yours by the GitHub login of a member

A group MUST be `yours` when the GitHub login of the app's sign-in is the `githubLogin` of a member (any case) whose
approval can count (its `approvalState` is not `notEligible`). A member without a GitHub login is nobody's.

#### Scenario: The login of the sign-in is a member's
- **WHEN** the GitHub login of the app's sign-in is the `githubLogin` of a member whose approval can count
- **THEN** the group is the user's
  [@test] crates/domain-policy/src/reviewer_groups_test.rs (`a_group_is_yours_by_the_github_login_of_a_member_whose_approval_can_count`)

#### Scenario: The sign-in names no login
- **WHEN** the app's sign-in names no login (signed out, or the token of `gh` or the
  environment)
- **THEN** `yours` is null: nobody knows whose a group is, so no group is the user's and Approve says nothing of
  what it counts for

### Requirement: The readiness of a request: No reviewer groups without an answer that needs approvals

The view MUST have no groups while Patronus doesn't answer (`unavailable`, a repository not on Space), when the changes
need no approval, and for a pull request into another branch: the owners of the `OWNERSHIP` files stay.

#### Scenario: No answer, or no approval needed
- **WHEN** Patronus doesn't answer, or the changes need no approval
- **THEN** the row has no reviewer groups, and the owners of the `OWNERSHIP` files stay
  [@test] crates/domain-policy/src/reviewer_groups_test.rs (`no_groups_without_an_answer_or_when_no_approval_is_needed`)

### Requirement: Safe Push without review: It is an explicit choice where no pull request counts the approvals

Some merge profiles skip the code-owner review (Large Scale Change; an emergency profile skips all checks). Patronus
doesn't say which before a start, and a `/patronus` comment can't choose a profile, so a Safe Push without review MUST
be an explicit choice of the user, offered only where the approvals count in a pull request the branch doesn't have, or
may (GitHub signed out or failing: no pull request is known, [Next Step](../next-step/spec.md#facts-without-an-answer)).

#### Scenario: GitHub signed out or failing
- **GIVEN** the approvals of a branch are missing
- **WHEN** GitHub is signed out or failing, so no pull request is known
- **THEN** a Safe Push without review is offered, as the explicit choice of the user

### Requirement: Safe Push without review: The pull request dialog of row 7 has the button

The "Create pull request" dialog of a branch at row 7 of the [next step](../next-step/spec.md) (missing approvals, no
open pull request) MUST have "Safe Push Without Review…" at the left of its footer, a ghost button with the
`ShieldOffIcon`, which takes neither ⌘↵ nor the focus; its hover: "Starts the Safe Push with a merge profile that
skips the code-owner review, such as Large Scale Change". The dialog of any other step MUST NOT have such a button.

#### Scenario: Row 7 of the next step
- **GIVEN** the approvals of a branch are missing and it has no open pull request
- **WHEN** its "Create pull request" dialog opens
- **THEN** its footer has "Safe Push Without Review…" at the left
  [@test] crates/domain-policy/src/next_step_test.rs (`rows_7_to_10_the_missing_approvals_and_where_they_can_come_from`)
  [@test] src/presenters/next-step.test.ts (`row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly`, `rows 8 to 10: changes requested, Request Review…, the wait naming the people asked`)

#### Scenario: The dialog of another step
- **WHEN** the dialog opens for a branch whose Safe Push needs no pull request
- **THEN** it has no such button
  [@test] tests/e2e/next-step/next-step.spec.ts (`should not offer it in the dialog of a branch whose Safe Push needs no pull request`)

### Requirement: Safe Push without review: The button is disabled by the reasons of the branch only

The button MUST be disabled with the reason of the branch (a sync, a running robot), never the approvals
(`getSafePushWithoutReviewItem`, `BranchNextStep.withoutReview`).

#### Scenario: A sync runs
- **WHEN** a sync runs or a robot of the branch runs
- **THEN** the button is disabled with that reason, never with the missing approvals

### Requirement: Safe Push without review: A start that skips the approvals is held back by the subject only

A start that skips the approvals (this one, or a default start whose profile is known to skip the review) MUST be held
back by the step only by the reasons of the subject on every row (a sync, a failure, no commits, the robots, parked,
landed, what runs on the row: `BranchNextStep.subjectBlocker`; its decision has no fact of a sync), never by the
step's own (the approvals, a draft, another base), the click and the popover alike (`startStepBlocker`).

#### Scenario: The blocker of a start that skips the approvals
- **WHEN** a start skips the approvals
- **THEN** its blocker is the reasons of the subject on every row, never the step's own; its decision says the rest
  [@test] src/presenters/next-step.test.ts (`is the step's for a Safe Push, of a start that skips the approvals the reasons of the subject on every row, of a Dry Run its own`)

### Requirement: Safe Push without review: The button opens the popover of a Safe Push without review

The button MUST close the dialog, reveal the row and open the popover of its Safe Push without review, also when the
popover of the row is open (it opens again in the other mode).

#### Scenario: Clicked in the dialog
- **WHEN** the user clicks "Safe Push Without Review…" in the dialog
- **THEN** the dialog closes, the row is revealed and the popover of its Safe Push without review opens
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover without review from the pull request dialog`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`)

### Requirement: Safe Push without review: The popover names the missing approvals

The popover MUST be "Safe Push of auth" with the start "Safe Push Without Review". Its review line MUST name the
missing approvals: "Starts without the approvals of IJPL Code Platform: the profile you choose has to skip the
code-owner review.".

#### Scenario: The popover opens
- **WHEN** the popover of a Safe Push without review opens
- **THEN** its title is "Safe Push of auth", its start "Safe Push Without Review", and its review line names the missing approvals

### Requirement: Safe Push without review: The profile Select offers only the profiles that may skip the review

The profile Select (the profiles the plan offers, `Plan.alternatives`) MUST have the groups "Skip the code-owner
review" (`skipsReview` true), "Other profiles" ("Profiles" without the first group; `skipsReview` null) and
"Emergency"; a profile known to check the review MUST be left out, and there are no default checks.

#### Scenario: The profiles of a start without the approvals
- **WHEN** the profile Select opens
- **THEN** it offers the profiles of its mode, without the approvals the emergency ones and a skipping default
  [@test] crates/domain-policy/src/land_test.rs (`a_start_offers_the_profiles_of_its_mode_and_without_approvals_the_emergency_ones_and_a_skipping_default`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start from the pull request dialog with the profile the user chooses`)

### Requirement: Safe Push without review: No profile is chosen unless the last robot's skips the review

None MUST be chosen unless the last robot's profile skips the review: the footer says "Choose the profile that skips
the code-owner review" and the start waits.

#### Scenario: No profile chosen
- **WHEN** the last robot's profile is not known to skip the review
- **THEN** the footer says "Choose the profile that skips the code-owner review" and the start waits

#### Scenario: The last robot's profile skips the review
- **WHEN** the options of the last robot said its profile skips the review
- **THEN** that profile is chosen and the start goes with it
  [@test] tests/e2e/next-step/next-step.spec.ts (`should start with the profile of the last robot when its options said it skips the review`)

### Requirement: Safe Push without review: A line below the Select says what the profile skips

Below the Select, the popover MUST show the profile's description from Patronus and a line saying whether the profile
skips the code-owner review.

#### Scenario: The line of the chosen profile
- **WHEN** a profile is chosen
- **THEN** the line is "Large Scale Change skips the code-owner review: the last Safe
  Push of the branch ran with it.", or "Patronus tells only when the robot starts whether Large Scale Change skips the
  code-owner review: if it doesn't, the Safe Push fails at once.", or for an emergency profile in `text-destructive`
  with the warning triangle: "Emergency skips all checks of the Safe Push, not only the code-owner review: Patronus
  pushes into the target without testing it."
  [@test] tests/e2e/next-step/next-step.spec.ts (`should say that an emergency profile skips all checks, and start with it`)

### Requirement: Safe Push without review: The start is the REST start

The start MUST be the REST start, announced and recorded in Activity as any; a profile that checks the review fails it
before any build, told as [failed](#failed) with the profile named. Once a pull request is open or the approvals are no
longer missing, the popover MUST be the usual one.

#### Scenario: A profile that checks the review
- **WHEN** the chosen profile checks the review after all
- **THEN** the robot fails before any build, told as failed with the profile named

#### Scenario: A pull request is open
- **WHEN** a pull request is open or the approvals are no longer missing
- **THEN** the popover is the usual one

### Requirement: Safe Push without review: `brd safe-push` decides the same

`brd safe-push` MUST decide the same: a start through the API with a profile known to skip the review (`--profile`, or
the last robot's) starts without the approvals, its `review.skipped_by` `profile`
([Agent CLI](../../cli/agent-cli/spec.md#brd-safe-push)).

#### Scenario: A profile that skips the review
- **WHEN** `brd safe-push` starts through the API with a profile that skips the review
- **THEN** it starts without the approvals, its `review.skipped_by` `profile`
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_profile_that_skips_the_review_starts_without_the_approvals_through_the_api`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_safe_push_review_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Restart and cancel: Restart is offered for a failed, canceled or canceling robot

Restart (the popover, the branch menu) MUST be offered for a failed, canceled or canceling robot started on the branch,
and for a passed dry run when the remote branch has a newer commit; Patronus refuses the others. It MUST NOT be offered
for a robot that failed for missing approvals in a Space merge request; a robot of a pull request that did is
restarted, as the restart counts the approvals the pull request has by then (its popover says so).

#### Scenario: A failed, canceled or canceling robot
- **WHEN** the robot started on the branch failed, was canceled or is canceling
- **THEN** the popover and the branch menu offer Restart

#### Scenario: A passed dry run
- **WHEN** a dry run passed and the remote branch has a newer commit
- **THEN** Restart is offered

#### Scenario: Missing approvals
- **WHEN** a robot failed for missing approvals in a Space merge request
- **THEN** Restart is not offered
- **AND** a robot of a pull request that failed for them is restarted, its popover saying that the restart counts the approvals the pull request has by then

### Requirement: Restart and cancel: A Safe Push of a temporary ref is not restarted from here

A Safe Push of a temporary ref (IDE, CLI) MUST NOT be restarted from here: it would test its old commit.

#### Scenario: A robot of a temporary ref
- **WHEN** a Safe Push of a temporary ref failed
- **THEN** no restart is offered for it

### Requirement: Restart and cancel: The restart is a new robot on the target as it is

The restart MUST be a new robot, on the target as it is at the restart: the branch of an old robot needs no push of a
newer baseline.

#### Scenario: An old robot
- **WHEN** the robot of a branch ran on an older baseline
- **THEN** its restart runs on the target as it is at the restart, without a push of a newer baseline

### Requirement: Restart and cancel: What a restart does is one rule of Rust

What a restart does MUST be one rule of Rust (`domain_policy::holds::restart_action`, the view's `restarts` by robot
id): a plain restart while the remote has the branch, a push first while the branch has work the remote lacks ("Push
and Restart", `PushAndRestart`, also of a running robot, which the push cancels first; on every surface it is named
"Restart Safe Push" too); the page only presents it.

#### Scenario: The remote has the branch, or lacks its work
- **WHEN** a restart is offered
- **THEN** it restarts on the remote while the remote has the branch, else pushes first
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)

### Requirement: Restart and cancel: A failing robot is restarted without waiting for its other checks

A running robot a required check of which failed ("Failing": it will fail) MUST be restarted too, without waiting for
the rest of its checks: its restart cancels it, then Patronus restarts the canceling robot on the commit of the remote
branch, as Patronus restarts no running robot.

#### Scenario: The restart of a failing robot
- **WHEN** the user restarts a failing robot
- **THEN** it is canceled first, then started again on the commit of the remote branch
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_robot_offers_its_restart_with_why_it_cannot_start_now`)
  [@test] crates/repository-model/src/facts_test.rs (`an_answer_that_a_running_robot_is_failing_derives_the_patronus_view_again`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should restart it from its details: canceled first, then started again on the commit it tested`, `should restart it from the branch menu`)

### Requirement: Restart and cancel: The restart of a failing robot comes before its cancel

The restart MUST be offered first, before "Cancel Safe Push", in the details, the branch menu and the landing overview;
its hover says "Cancels the run on 78ca3b88 and starts it again on the remote branch". A running robot no check of
which failed MUST have no restart: its cancel is the way.

#### Scenario: The hover of the restart
- **WHEN** the user hovers the restart of a failing robot
- **THEN** it says "Cancels the run on 78ca3b88 and starts it again on the remote branch"
  [@test] src/presenters/robot-actions.test.ts (`names what a restart of a running robot a failed required check dooms does: it cancels it, then starts it again`)

#### Scenario: A running robot no check of which failed
- **WHEN** no check of a running robot failed
- **THEN** it has no restart, only its cancel

### Requirement: Restart and cancel: A restart waits where Safe Push waits

A restart is a start: it MUST wait where "Safe Push" waits (`domain_policy::land::restart_refusal`, the `refusal` of
the view's restart), disabled with the same reason on every surface, for a blocked branch, and, for a Safe Push, while
a robot lands an ancestor of the branch or a branch built on it lands the branch: two robots would land the same
commits. A dry run pushes nothing into the target, so only the block holds its restart back.

#### Scenario: The refusal of a restart
- **WHEN** the restart of a robot is refused
- **THEN** it is disabled with the words of its reason on every surface, named by the mode, the step's for missing approvals
  [@test] src/presenters/robot-actions.test.ts (`presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)

#### Scenario: The order of the refusals
- **WHEN** a restart is checked
- **THEN** it is refused by the table first, then as a start without its own robot, then by a landing
  [@test] crates/domain-policy/src/land_test.rs (`a_restart_is_refused_by_the_table_first_then_as_a_start_without_its_own_robot_then_by_a_landing`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Restart and cancel: A branch a merged Safe Push landed is not restarted

Once a merged Safe Push landed all of the branch ([below](#merged)), its restart MUST be refused, of a Dry Run too, as
its Safe Push and Dry Run are ("Landed in master with oauth", row 1 of [Next Step](../next-step/spec.md#the-table)):
its commits are in the target. One with commits after the landed ones restarts, for them.

#### Scenario: A branch built on it landed it
- **WHEN** a merged Safe Push of a branch built on it landed the branch
- **THEN** the restart of the failed robot of the branch is refused in its menu, its robot's details and its preview
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should refuse the restart of the failed robot of a branch a merged Safe Push of a branch built on it landed, in its menu, its robot's details and its preview`)

### Requirement: Restart and cancel: A parked branch is not restarted

A parked branch stays local, and so does a whole stack with a parked member: its restart MUST be refused before
anything else ("Parked: Unpark it to push"), as its start is; a restart that pushes first MUST be refused for a branch
the sync did not build.

#### Scenario: A parked branch
- **WHEN** the user restarts the robot of a parked branch, or of a whole stack with a parked member
- **THEN** the restart is refused, "Parked: Unpark it to push"

### Requirement: Restart and cancel: On a cached frame the restart and the cancel wait for the first sync

On the cached frame of an earlier run (its table, its robots) the restart and the cancel MUST wait for the first sync
("Waits for the first sync", [Branch Row](../branch-row/spec.md#actions-cell)): the robot may have finished or been
restarted since.

#### Scenario: A frame nobody checked
- **WHEN** the window shows the cached frame of an earlier run
- **THEN** every remote action waits for the first sync and the rest stays
  [@test] crates/domain-policy/src/actions_test.rs (`on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays`)

### Requirement: Restart and cancel: Cancel is offered for a running robot

Cancel MUST be offered for a running robot; the robot is canceling until its checks stopped.

#### Scenario: A running robot is canceled
- **WHEN** the user cancels a running robot
- **THEN** it is canceling until its checks stopped

### Requirement: Restart and cancel: A restart and a cancel are intents of the engine

A restart and a cancel MUST be intents of the engine (`RestartRobot`, `CancelRobot`; `operations::robot`), decided in
Rust over the facts of the session, never the page's, and decided again by `execute` before the first effect: the plan
of a restart is the restart the view offers, refused by the refusal of the view ("Nothing was restarted: …").

#### Scenario: A plan refused
- **WHEN** the plan of a restart is refused
- **THEN** nothing runs, and it says what refused it
  [@test] src/lib/robot-run.test.ts (`says what its plan refuses it by, and runs nothing`)

#### Scenario: Robots that have no restart
- **WHEN** the robot is of a temporary ref, a running one on the commit of its branch no required check of which
  failed, or a merged one
- **THEN** it has no restart

#### Scenario: The real service against the fake
- **WHEN** a start, a restart and a cancel run
- **THEN** they run the real service against the fake of the state
  [@test] crates/patronus/src/actions_test.rs (`a_fake_of_the_state_only_lists_the_robots_a_start_and_a_restart_start_and_cancels_them`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_start_restart_and_cancel_run_the_real_service_against_the_fake`)

### Requirement: Restart and cancel: A plain restart moves no ref

A plain restart MUST move no ref: no lock; of a running robot it cancels it first (`[CancelRobot,
RestartRobot]`).

#### Scenario: The remote has the branch
- **WHEN** the remote has the branch
- **THEN** the restart restarts alone and takes no lock
  [@test] crates/operations/src/robot_test.rs (`a_restart_restarts_alone_while_the_remote_has_the_branch_and_pushes_first_a_branch_that_changed`, `a_restart_on_the_remote_branch_takes_no_lock`)

#### Scenario: A failing robot
- **WHEN** a failing robot is restarted
- **THEN** it is canceled, then restarted on the same commit
  [@test] crates/operations/src/robot_test.rs (`a_restart_of_a_failing_robot_cancels_it_and_restarts_it_on_the_same_commit`)

### Requirement: Restart and cancel: A restart that pushes first restarts on the pushed commit

A restart that pushes first MUST retarget the pull request of the branch out of a virtual branch, cancel the running
robot, push the branch (the lock only for the witness and the freeze) and restart the robot after the push, skipped
when the pushed branch moved since; a push that drops commits of the remote the branch lacks MUST refuse it ("the
remote branch of auth has 2 commits auth lacks").

#### Scenario: The branch changed since
- **WHEN** the branch has work the remote lacks
- **THEN** the restart cancels the running robot, pushes and restarts it on the pushed commit
  [@test] crates/operations/src/robot_test.rs (`a_restart_restarts_alone_while_the_remote_has_the_branch_and_pushes_first_a_branch_that_changed`, `a_restart_that_pushes_first_cancels_the_running_robot_pushes_and_restarts_it_on_the_pushed_commit`)

#### Scenario: The remote has commits the branch lacks
- **WHEN** the push would drop commits of the remote the branch lacks
- **THEN** the restart is refused: "the remote branch of auth has 2 commits auth lacks"

### Requirement: Restart and cancel: A cancel of a robot that finished is answered as it ended

A cancel of a robot that finished meanwhile (as the session or its status says) MUST be answered as it ended, nothing
canceled ("Safe Push already finished"); the robot a start or a restart replaces is canceled the same way, so the
start or the push goes on.

#### Scenario: The robot finished meanwhile
- **WHEN** the user cancels a robot that finished meanwhile
- **THEN** the answer is as it ended, "Safe Push already finished", and nothing was canceled
  [@test] crates/operations/src/robot_test.rs (`a_cancel_of_a_robot_that_finished_is_as_it_ended_and_cancels_nothing`)
  [@test] src/lib/robot-run.test.ts (`answers a robot that finished meanwhile as it ended: nothing was canceled`)

#### Scenario: A start replaces a robot that finished
- **WHEN** a start replaces a robot that finished meanwhile
- **THEN** its cancel is answered as it ended, and the start goes on
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_cancel_of_a_robot_that_finished_is_answered_as_it_ended_and_a_start_goes_on`)

### Requirement: Restart and cancel: A restart or a cancel runs once per robot

A restart or a cancel MUST run once per robot (an operation of the session, [Running
Operations](../../../../docs/design/operations.md)): while it runs, it is running on every surface of the robot (the
slot, the details, the landing overview: disabled, a spinner in place of its icon, `aria-busy`), a second click joins
it, a cancel during a restart waits for it; the restart is the move of its row from the click on (the row lands).

#### Scenario: A second click during a restart
- **WHEN** the user restarts a robot while its restart runs
- **THEN** the restart runs once, running in the slot and in its details, and the row lands from the click
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart a robot once while its restart runs, running in the slot and in its details, landing from the click`)
  [@test] src/presenters/robot-actions.test.ts (`runs the restart of the engine from both restarts, and shows one running in the session running on every surface`)

#### Scenario: A cancel and a start during a restart
- **WHEN** a second restart, a cancel or a Safe Push of what the robot lands comes during a restart
- **THEN** the second restart joins the first, and the cancel and the Safe Push wait for it
  [@test] crates/operations/src/robot_test.rs (`a_second_restart_of_a_robot_joins_the_first_and_its_cancel_and_a_start_of_its_branch_wait_for_it`)

### Requirement: Restart and cancel: Rust decides the actions of a robot

Rust MUST decide the actions of a robot (`domain_policy::actions::robot_actions`, `robot:<id>` of the view's
`actions`): a restart while one is offered (`restarts` of the view); the cancel of a running robot; the prompts of a
failed robot (not for approvals) or of a running one whose required check failed.

#### Scenario: A restart disabled
- **WHEN** Patronus didn't tell why the robot failed, the step of its branch misses approvals, or a start is refused
- **THEN** the restart is disabled: while the step misses approvals, a restart would fail again, with the words of that step
  [@test] crates/domain-policy/src/actions_test.rs (`a_robot_restarts_once_patronus_tells_why_it_failed_and_its_approvals_count`)
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)

#### Scenario: The prompts while why it failed is to come
- **WHEN** why the robot failed is to come
- **THEN** its prompts are disabled as the restart is

### Requirement: Restart and cancel: One builder presents the actions of a robot on every surface

One builder MUST present that list (`getRobotActionItems`), and every surface of the robot MUST render it: the branch
menu, the details (the chip popover, the preview section), the robots indicator, the toasts of the robot and the
[landing overview](../../repository/landing-overview/spec.md#overview).

#### Scenario: The popover restarts and cancels
- **WHEN** the user restarts a failed robot and cancels a running one from the popover
- **THEN** each runs through the engine
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
  [@test] crates/patronus/src/actions_test.rs (`restart_answers_the_new_robot_and_cancel_the_canceling_one`)

### Requirement: Restart and cancel: The actions of a robot are in one order, each naming the mode

The actions MUST be in this order: "Restart Safe Push" (also the one that pushes first, [below](#restart-and-cancel)),
"Cancel Safe Push", the prompts of a failed or failing robot, "Open in Patronus", and "Open Pull Request #12" for a
robot started from a pull request (the branch menu leaves it out when it is the branch's own pull request, which its
"Open Pull Request" opens). Each MUST name the mode of the robot ("Restart
Dry Run"); a bare "Cancel" in a popover reads like closing it.

#### Scenario: The actions of a running robot of a pull request
- **WHEN** the actions of a running robot started from a pull request show
- **THEN** "Cancel Safe Push" cancels it through the engine, "Open in Patronus" opens it, and "Open Pull Request #12" the pull request it was started from
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)

#### Scenario: The restart named by the mode
- **WHEN** a restart is offered
- **THEN** it is named by the mode of the robot ("Restart Dry Run" of a dry run)
  [@test] src/presenters/robot-actions.test.ts (`presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`)

#### Scenario: The prompts of a failed robot
- **WHEN** the actions of a failed robot, or of a failing one, show
- **THEN** "Start in Air…", "Copy Prompt for Agent" and "Copy Prompt with CI Details" come after the restart and the cancel, before "Open in Patronus"

### Requirement: Restart and cancel: A branch that can't change keeps Cancel and Open in Patronus

The menu of a branch that can't change (a conflict, a sync error, a sync running) MUST keep "Cancel Safe Push" and
"Open in Patronus": they don't change the branch.

#### Scenario: A sync runs
- **WHEN** the menu of a branch whose sync runs opens while its robot runs
- **THEN** it has "Cancel Safe Push" and "Open in Patronus"

### Requirement: Restart and cancel: A restart and a cancel are told in place of the robot's toast

A restart MUST be the toast "Safe Push restarted", a cancel "Safe Push is canceling", each naming the branch and the
target ("auth → master") with "Open in Patronus" (the new robot's page for a restart), as a start does, in place of
the toast of the robot they restart or cancel; Activity records them in the same words.

#### Scenario: A restart
- **WHEN** a restart runs
- **THEN** the toast of the robot is replaced in place by "Safe Push restarted" of the newer robot
  [@test] src/stores/patronus.test.ts (`is replaced in place by the toast of its restart, a newer robot of the branch`)
  [@test] src/lib/robot-run.test.ts (`restarts through the engine and says it in the toast of the robot`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should restart a failed robot and cancel a running one from the popover`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart from the toast a Safe Push whose builds never started`)

#### Scenario: A cancel
- **WHEN** a cancel runs
- **THEN** the robot is canceling, and the toast says "Safe Push is canceling"
  [@test] src/lib/robot-run.test.ts (`cancels through the engine: the robot is canceling`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Requirement: Restart and cancel: A failed or refused restart or cancel says why

A failure MUST be "Cannot restart the Safe Push" / "Cannot cancel the Safe Push"; a refusal of the plan and facts that
changed after the push MUST say why in an info toast under the same title.

#### Scenario: Facts changed after the push, or Patronus failed
- **WHEN** the facts changed after the push of a restart, or Patronus failed it
- **THEN** an info toast says the change, and an error toast the failure of Patronus
  [@test] src/lib/robot-run.test.ts (`says the change of the facts after its push in an info toast, and a failure of Patronus in an error toast`)

### Requirement: Restart and cancel: A branch with work the remote lacks is pushed before the restart

Patronus restarts on the newest commit of the remote branch. When the branch has work the remote lacks (by content,
or commits reworded since its push: the remote has them with the old messages, which Patronus would land), every
surface MUST offer the restart that pushes first instead of the plain one, named "Restart Safe Push" too:
the push of the row, then the restart on the pushed commit, also for a running robot, which it cancels first
([below](#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: The branch changed since the push
- **WHEN** the branch has work the remote lacks
- **THEN** every surface names the change and the restart that tests the branch as it is: it cancels a running robot, then pushes
- **AND** the restart is still named "Restart Safe Push", its hover "Pushes auth and starts it again on the pushed commit"
  [@test] src/lib/patronus.test.ts (`names the change and the restart that tests the branch as it is`)
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] src/presenters/robot-actions.test.ts (`presents the restart Rust offers, named by the mode, disabled with the words of its reason, the step's for missing approvals`, `names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)

#### Scenario: A passed dry run
- **WHEN** the restart of a passed dry run pushes first
- **THEN** it counts the pushed commit as the newer one

#### Scenario: A blocked branch
- **WHEN** the branch is blocked
- **THEN** the push and restart is disabled, with the reason

### Requirement: Restart and cancel: The restart that pushes first is the one toast

The restart MUST be the action: its toast "Safe Push restarted" is the one toast, the push MUST have none (Activity
records both).

#### Scenario: Push and restart from the branch preview
- **WHEN** the user pushes and restarts a failed robot whose branch changed since, from the branch preview
- **THEN** the toast "Safe Push restarted" is the one toast, and the row lands from the click
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview, landing from the click`)

### Requirement: Restart and cancel: A failed action says what Patronus said

A failed action MUST be a toast with what Patronus said; without a token it MUST say "Run ./safePush.cmd once to log in
to Patronus" and send nothing.

#### Scenario: No token
- **WHEN** an action runs without a token of Patronus
- **THEN** it says "Run ./safePush.cmd once to log in to Patronus" and sends nothing
  [@test] crates/patronus/src/actions_test.rs (`an_action_without_a_token_tells_how_to_log_in`)

### Requirement: Pushing into a branch whose Safe Push runs: A running Safe Push holds the branch it lands

A branch MUST be held (`PushHold`) while a Safe Push that pushes into the target runs on it (started on the branch, not
of a temporary ref) or on a branch built on it whose commit has commits of it, which lands them ("lands with oauth"). A
push of a held branch won't land: Patronus merges the commit it took at the start, and a push into its own branch fails
it at the end ("branch was modified"). A dry run and a canceling robot MUST hold nothing.

#### Scenario: Its own Safe Push runs on the branch
- **GIVEN** a Safe Push that pushes into the target runs on the branch, started on the branch, not of a temporary ref
- **WHEN** the holds of the branch are decided
- **THEN** the branch is held, as a push into it would fail that Safe Push at the end ("branch was modified")
- **AND** it is held no longer once the robot no longer runs
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)
  [@test] src/stores/patronus.test.ts (`holds a branch whose Safe Push runs and the ancestors it lands; not once the robot no longer runs`)

#### Scenario: The Safe Push of a branch built on it lands its commits
- **GIVEN** a Safe Push runs on a branch built on the branch, and its commit has commits of the branch
- **WHEN** the holds of the branch are decided
- **THEN** the branch is held, and its reason says that the robot lands it ("lands with oauth")
  [@test] src/presenters/reasons.test.ts (`says why a held branch offers no push: its own Safe Push, one that lands it with the commits it counts`)

#### Scenario: A dry run or a canceling robot
- **GIVEN** the robot on the branch, or on a branch built on it, is a dry run or is canceling
- **WHEN** the holds of the branch are decided
- **THEN** it holds nothing
  [@test] src/lib/patronus.test.ts (`holds pushes with a running Safe Push only: a dry run or a cancel holds nothing`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_dry_run_lands_nothing`)

### Requirement: Pushing into a branch whose Safe Push runs: The Safe Push of the whole stack holds what its commit has

The Safe Push of the whole stack (on its stack branch) MUST hold every branch of the stack its commit has. The reason
MUST count the commits of the branch the robot lands once that is told, as `brd` does.

#### Scenario: A branch of the stack the whole-stack Safe Push lands
- **GIVEN** a Safe Push of the whole stack runs on its stack branch
- **WHEN** the holds of a branch of the stack its commit has are decided
- **THEN** the branch is held: "A Safe Push of the whole air stack is running: it lands 1 commit of auth with it" ([Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app))
  [@test] src/stores/patronus.test.ts (`holds every branch the whole-stack Safe Push lands`)

### Requirement: Pushing into a branch whose Safe Push runs: A branch a merged Safe Push landed whole is held

A branch a merged Safe Push landed whole ([Merged](#merged)), the branch it ran on included, MUST be held too, before
any other hold, until the update archives it. A branch with commits added after the landed ones MUST NOT be held for
it: they are work for the remote.

#### Scenario: A branch a merged Safe Push landed whole
- **GIVEN** a merged Safe Push landed the branch whole, or ran on it
- **WHEN** the holds of the branch are decided, until the update archives it
- **THEN** the branch is held, before any other hold: "Landed in master with oauth: nothing of it is left to push", "Merged by Patronus: nothing of it is left to push"
  [@test] src/stores/patronus.test.ts (`holds the branches a merged Safe Push landed, also one first seen merged, and lands the rest without them`)
  [@test] src/presenters/reasons.test.ts (`says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge`)

#### Scenario: Commits added after the landed ones
- **GIVEN** a merged Safe Push landed commits of the branch, and commits were added to it after them
- **WHEN** the holds of the branch are decided
- **THEN** the branch is not held: the added commits are the work left for the remote
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left`)

### Requirement: Pushing into a branch whose Safe Push runs: A landed branch is only left out of the pushes of a stack

A landed branch MUST only be left out of the pushes of a stack: the branches built on it push without it (Push with
Children of a landed root, [Stacked Branches](../stacked-branches/spec.md#pull-requests)), while a running Safe Push
MUST hold every push of its stack.

#### Scenario: Push with Children of a landed root
- **GIVEN** the root of a stack is landed
- **WHEN** the user pushes it with children
- **THEN** the branches built on it push without it
  [@test] src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)

#### Scenario: A running Safe Push of the stack
- **GIVEN** a Safe Push runs on a branch of the stack
- **WHEN** the user pushes the stack
- **THEN** the running Safe Push holds every push of its stack

### Requirement: Pushing into a branch whose Safe Push runs: The hold is one rule of Rust for the app and brd

The hold MUST be a rule of Rust, one for the app and `brd` (`domain_policy::holds::branch_hold`, the view's
`landing.hold`): a landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push
that lands it. The page MUST only word it (`src/presenters/reasons.ts`).

#### Scenario: The order of the holds
- **WHEN** a branch is landed whole, its own Safe Push runs and another running Safe Push lands it
- **THEN** the hold is the landing that left nothing, else the branch's own running Safe Push, else the first running Safe Push that lands it
  [@test] crates/domain-policy/src/holds_test.rs (`a_branch_is_held_by_what_landed_it_whole_then_its_own_safe_push_then_one_that_lands_it`)

#### Scenario: The app and brd answer alike
- **WHEN** the app and `brd` decide the hold of the same branch
- **THEN** they answer alike
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

### Requirement: Pushing into a branch whose Safe Push runs: The stack branch of a whole stack is held as a branch is

The stack branch of a whole stack ([Whole Stack](../whole-stack/spec.md)) MUST be held as a branch is, by one rule of
Rust for the app and `brd push --with-children` (`domain_policy::holds::stack_branch_hold`, the view's `landing.hold` of
the stack), in this order: a parked member keeps it local; then merged Safe Pushes, of pieces or of the whole stack,
that landed every branch of the stack whole; then its own running robot; then a stack branch the sync did not build.

#### Scenario: A parked member
- **GIVEN** a member of the whole stack is parked
- **WHEN** the hold of the stack branch is decided
- **THEN** the parked member keeps it local: "Holds parked login: land the stack in pieces"
  [@test] crates/domain-policy/src/holds_test.rs (`a_stack_branch_is_held_by_a_parked_member_then_landed_members_then_its_robot_then_no_build`)

#### Scenario: Merged Safe Pushes landed every branch of the stack
- **GIVEN** merged Safe Pushes, of pieces or of the whole stack, landed every branch of the stack whole
- **WHEN** the hold of the stack branch is decided
- **THEN** the stack branch is held, and the reason says who landed them, each robot once, in the order they merged: "Landed in master with deploy and side-test: nothing of it is left to push"
- **AND** "Merged by Patronus: …" when its own robot alone landed them
  [@test] crates/domain-policy/src/holds_test.rs (`every_member_landed_by_the_robot_of_the_whole_stack_is_its_own_landing`)
  [@test] src/stores/patronus.test.ts (`holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them`)
  [@test] src/presenters/reasons.test.ts (`says that nothing is left to push of a stack branch merged Safe Pushes landed, and nothing for no hold`)

#### Scenario: The landed stack branch is not pushed with what is left
- **GIVEN** every branch of the stack landed, and commits are left above it
- **WHEN** the user pushes the stack
- **THEN** the push of the whole stack branch is held, and what is left pushes without the landed ones
  [@test] src/stores/patronus.test.ts (`holds the stack branch once merged Safe Pushes landed every branch of the whole stack, and says who landed them`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: Its own running robot
- **GIVEN** the Safe Push of the whole stack runs on the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held: "A Safe Push of the whole air stack is running: wait until it merges, or restart it on the current stack"

#### Scenario: A stack branch the sync did not build
- **GIVEN** the sync did not build the stack branch
- **WHEN** the hold of the stack branch is decided
- **THEN** it is held

### Requirement: Pushing into a branch whose Safe Push runs: A held stack branch is left out and its push refused

While the stack branch is held, Push with Children MUST leave it out, and its push alone (the push step of the restart
of its robot, of Request Review… and of the creation of its pull request, `pushWholeStack`) MUST be refused with that
reason.

#### Scenario: Push with Children and the push of the stack branch alone
- **GIVEN** the stack branch of a whole stack is held
- **WHEN** the user pushes with children, or an action pushes the stack branch alone (`pushWholeStack`: the restart of its robot, Request Review…, the creation of its pull request)
- **THEN** Push with Children leaves it out
- **AND** its push alone is refused with the reason of the hold

### Requirement: Pushing into a branch whose Safe Push runs: The app and brd derive what the robots land with one rule

The app and `brd` MUST decide what the robots land, landed and hold with one rule over the branch table of the last
sync (`repository_model::facts::derive_patronus`, [Session facts](../../../../docs/design/git-operations.md#session-facts)):
the session of the repository derives it again when its table committed, its robots changed or a push moved a remote
branch, in the blocking pool, and sends it as the part `patronus` of its view.

#### Scenario: The session derives the view of the robots
- **WHEN** Patronus answered, the table of the session committed, its robots changed or a push moved a remote branch
- **THEN** the session derives what the robots land, landed and hold again, in the blocking pool, and sends it as the part `patronus` of its view
  [@test] crates/repository-model/src/session_test.rs (`the_session_derives_the_patronus_view_and_its_landing_needs_once_patronus_answered`)
  [@test] crates/repository-model/src/facts_test.rs (`the_session_knows_the_remote_branches_a_push_moved_without_a_sync`)

#### Scenario: What the robots land and landed
- **WHEN** the view is derived over the branch table of the last sync
- **THEN** it says what each robot lands (the robot of the whole stack every branch of it, a dry run nothing) and which merged robot names the landing
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_robot_of_the_whole_stack_lands_every_branch_of_it`, `the_merged_robot_that_left_the_least_work_names_the_landing`, `a_dry_run_lands_nothing`)

### Requirement: Pushing into a branch whose Safe Push runs: The view of the robots is loading, stale or unavailable

Until Patronus answered and the first view is derived, the view MUST be `loading` (the rows wait for it); while a newer
one is derived it MUST be `stale` (no row waits); a view that can't be derived MUST be `unavailable` with why, and be
derived again after the next answer of Patronus.

#### Scenario: Before the first view
- **WHEN** Patronus has not answered yet, or the first view is not derived
- **THEN** the view is `loading` and the rows wait for it
- **AND** while a newer one is derived it is `stale` and no row waits
  [@test] src/stores/patronus.test.ts (`waits for the view of the robots as they are while the session derives it, then takes it`)

#### Scenario: A view that can't be derived
- **WHEN** the view can't be derived
- **THEN** it is `unavailable` with why
- **AND** it is derived again after the next answer of Patronus
  [@test] crates/repository-model/src/session_test.rs (`a_view_that_failed_is_derived_again_after_the_next_answer_of_patronus`)

### Requirement: Pushing into a branch whose Safe Push runs: The robot of a branch is the one the session matches

The robot of a branch MUST be the one the session matches (`domain_policy::robot_match`, named in the view as
`robotId`): running over canceling over finished, then the newest, then the first of the list.

#### Scenario: The cases of the match
- **WHEN** several robots match a branch
- **THEN** the running one is taken over a canceling one over a finished one, then the newest, then the first of the list (`crates/domain-policy/src/robot_match_cases.json`)
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robots_match_their_branches_by_the_cases`)

### Requirement: Pushing into a branch whose Safe Push runs: The first robot of the table built on a branch lands it

Of the robots of branches built on a branch, the first of the table MUST land it ("lands with"), and of those that run
the first MUST hold it: a canceling robot says it lands the branch and hides no running one.

#### Scenario: A running robot at its tip lands what it is built on
- **GIVEN** a robot runs at the tip of a branch built on others
- **WHEN** the view is derived
- **THEN** it lands the branches it is built on
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`)

#### Scenario: Several robots of branches built on it
- **GIVEN** robots of several branches built on a branch, one of them canceling
- **WHEN** the view is derived
- **THEN** the first of the table lands it, and of those that run the first holds it
- **AND** the canceling robot says it lands the branch and hides no running one
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_first_robot_of_the_table_lands_a_branch_and_only_a_running_one_holds_it`)

### Requirement: Pushing into a branch whose Safe Push runs: A branch's own robot is never the robot of an ancestor

A branch's own robot MUST never be the robot of an ancestor it waits for: its Safe Push replaces it (the view's
`landing.replaces`: its own robot on an older commit, which the push of the start cancels).

#### Scenario: Its own robot on an older commit
- **GIVEN** the branch's own robot runs on an older commit, and the branch waits for an ancestor
- **WHEN** the view is derived
- **THEN** its own robot is in `landing.replaces`, not the robot of the ancestor, and the push of the start cancels it

### Requirement: Pushing into a branch whose Safe Push runs: A start says why it waits in a fixed order

A start MUST say first that the branch is blocked, then that a robot runs on its last commit, then that a robot built
on it lands it, then that a robot lands an ancestor (`domain_policy::land::start_refusal`, the view's `landing.start`).

#### Scenario: The order of the refusals of a start
- **WHEN** a Safe Push of the branch is started
- **THEN** the start says the first of: the branch is blocked, a robot runs on its last commit, a robot built on it lands it, a robot lands an ancestor
  [@test] crates/domain-policy/src/land_test.rs (`a_safe_push_waits_for_a_block_the_last_commit_a_robot_that_lands_it_then_one_that_lands_an_ancestor`)

#### Scenario: Branches a running Safe Push lands
- **GIVEN** a Safe Push of a branch built on others runs
- **WHEN** the user looks at the branches it lands
- **THEN** each says that the running Safe Push lands it, and none starts on its own
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)

### Requirement: Pushing into a branch whose Safe Push runs: A Dry Run whose push goes into a running Safe Push waits

A Dry Run MUST say the first two refusals of a start, then, while its start pushes, that a running Safe Push of a
branch built on it lands the branch: its push would fail that Safe Push at the end, as any push into what a running
Safe Push lands (`landing.dryRun`; `brd safe-push --mode dry-run` refuses it too, `robot_running`).

#### Scenario: A Dry Run below a running Safe Push
- **GIVEN** a Safe Push of a branch built on the branch runs and lands it
- **WHEN** a Dry Run of the branch whose start pushes is started
- **THEN** it says that the running Safe Push lands the branch (`landing.dryRun`)
- **AND** `brd safe-push --mode dry-run` refuses it too, `robot_running`
  [@test] crates/domain-policy/src/land_test.rs (`a_dry_run_waits_for_the_last_commit_and_for_the_span_hold_of_the_push_it_makes`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_dry_run_whose_push_goes_into_a_running_safe_push_is_held`)

### Requirement: Pushing into a branch whose Safe Push runs: The push an action needs is refused by a branch of its chain

The push an action needs (a pull request, a review request, a start) MUST be refused, before it is made, by a branch of
its chain up to the first held ancestor, past one that landed, that the sync did not build or whose push drops remote
commits (`domain_policy::holds::push_blocker`, the view's `landing.push`).

#### Scenario: An unbuilt or dropping branch of the chain
- **GIVEN** a branch of the chain up to the first held ancestor that the sync did not build, or whose push drops remote commits
- **WHEN** an action that needs a push is taken
- **THEN** the push is refused before it is made
  [@test] crates/repository-model/src/patronus_view_test.rs (`the_push_of_an_action_is_refused_by_an_unbuilt_or_dropping_branch_of_its_chain`)
  [@test] crates/domain-policy/src/holds_test.rs (`the_push_of_an_action_stops_at_a_held_ancestor_and_refuses_a_branch_not_built_or_dropping_commits`)

#### Scenario: Past an ancestor that landed
- **GIVEN** an ancestor in the chain landed: a merged Safe Push landed it whole, or the fetched baseline has every commit of it
- **WHEN** the push of the action is decided
- **THEN** the chain goes past it, as the decision of a start passes it
  [@test] crates/repository-model/src/derive_test.rs (`the_push_of_a_start_is_the_push_of_its_row_past_an_ancestor_that_landed`)
  [@test] crates/repository-model/src/branch_table_test.rs (`the_table_of_a_sync_carries_the_branches_the_fetched_baseline_has_whole`)

### Requirement: Pushing into a branch whose Safe Push runs: The hashes tell what the robot took without git

The hashes MUST tell, without git, when the robot took its branch as it is (its tip, or its remote branch that has the
same commits) or a commit of it the branch built new ones on: the copies a sync wrote say which copy is a commit of the
branch now, so an amend below the robot's commit in the same sync is told by content.

#### Scenario: The robot took the branch as it is
- **WHEN** the robot runs on the tip of the branch, or on its remote branch that has the same commits
- **THEN** the hashes tell that it took the branch as it is
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_robot_on_an_up_to_date_remote_branch_took_the_branch_as_it_is`)
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`the_hashes_answer_a_robot_at_its_tip_and_one_under_new_commits_without_git`)

#### Scenario: New commits on the commit of the robot
- **WHEN** the branch built new commits on the commit of the robot
- **THEN** the hashes tell it, without git
  [@test] crates/repository-model/src/patronus_view_test.rs (`new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`)

#### Scenario: An amend below the robot's commit in the same sync
- **WHEN** a commit below the robot's commit is amended, and the same sync wrote its copies
- **THEN** the copies the sync wrote say which copy is a commit of the branch now, and the amend is told by content
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`an_amend_below_the_robot_is_told_by_the_copies_the_sync_wrote`)

### Requirement: Pushing into a branch whose Safe Push runs: What a running robot lands is what its commit has

What a running robot lands MUST be what its commit has, not the stack as it is now: while it took the branches as they
are (it runs on the tip of its branch, or on a commit of it with new ones on top), every branch it is built on, with all
their commits.

#### Scenario: The robot runs at the tip of its branch
- **GIVEN** a robot runs at the tip of its branch
- **WHEN** what it lands is decided
- **THEN** it lands the branch and every branch it is built on, with all their commits
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_running_robot_at_the_tip_lands_the_branch_and_what_it_is_built_on`)
  [@test] crates/repository-model/src/patronus_view_test.rs (`a_running_robot_at_its_tip_lands_the_branches_it_is_built_on`)

#### Scenario: New commits on top of the robot's commit
- **GIVEN** a robot runs on a commit of its branch with new ones on top
- **WHEN** what it lands is decided
- **THEN** it lands every branch it is built on, with all their commits
  [@test] crates/repository-model/src/patronus_view_test.rs (`new_commits_on_the_commit_of_a_robot_leave_the_hashes_telling`)

### Requirement: Pushing into a branch whose Safe Push runs: After a restack or an amend below its commit the content tells

After a restack or an amend below the robot's commit, which gives the branches new copies, the content MUST tell (the
repository view, `robot_spans_of_grouped` and the walks of [Robot landing](../../../../docs/design/git-operations.md#robot-landing)):
a branch is held while the robot's commit has a commit of it, the same, a copy on a newer base or an earlier version
(amended since).

#### Scenario: A parent split out of the robot's commits
- **WHEN** a parent was split out of the commits of a running robot
- **THEN** it is in the robot's span and held
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_parent_split_out_of_the_commits_of_a_running_robot_is_in_its_span`)

#### Scenario: A commit amended below the robot
- **WHEN** a commit below a running robot was amended since
- **THEN** it is in its span as an earlier version, and its branch is held
  [@test] crates/sync-core/src/robot_landing_test.rs (`an_amended_commit_below_a_running_robot_is_in_its_span_as_an_earlier_version`)

#### Scenario: Only the ancestors the robot's commit has are held
- **WHEN** the view says which ancestors the commit of a running Safe Push has
- **THEN** only those are held, and their commits are counted
  [@test] src/stores/patronus.test.ts (`holds only the ancestors the view says the commit of a running Safe Push has, and counts their commits`)
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`a_root_restacked_below_a_running_robot_is_told_by_content_once`)

### Requirement: Pushing into a branch whose Safe Push runs: A parent restacked below the robot after its start goes on

The robot's commit has no commit of a parent a restack placed below the robot's branch after its start: its push, its
Create Pull Request…, its Safe Push and its Dry Run MUST go on, its row MUST say no "lands with", and the span of the
graph and the count of the island MUST start below it.

#### Scenario: A root restacked below a running robot
- **GIVEN** a restack placed a parent below the branch of a running Safe Push after it started
- **WHEN** the user pushes the parent
- **THEN** the parent is not in the robot's span: its push, Create Pull Request…, Safe Push and Dry Run go on, its row says no "lands with"
- **AND** the span of the graph and the count of the island start below it
  [@test] crates/repository-model/src/patronus_view_content_test.rs (`a_root_restacked_below_a_running_robot_is_told_by_content_once`)
  [@test] src/lib/stack-lanes.test.ts (`starts below a root the robot's commit doesn't have: a restack placed it there after the start`)
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_parent_restacked_below_a_running_robot_is_not_in_its_span`)
  [@test] crates/sync-core/src/process_budget_test.rs (`the_span_of_a_running_robot_after_a_restack`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a branch a restack placed below a running Safe Push after it started, and hold what its commit has`)

### Requirement: Pushing into a branch whose Safe Push runs: A robot of a temporary ref is asked for its branch

A robot of a temporary ref MUST be asked for the branch it was matched to by the tip.

#### Scenario: A robot started of a temporary ref
- **GIVEN** a running robot was started of a temporary ref
- **WHEN** what it lands is decided
- **THEN** it is asked for the branch it was matched to by the tip

### Requirement: Pushing into a branch whose Safe Push runs: The stack as it is now holds when the repository can't tell

Until the answer comes, and when the repository can't tell (it lacks the robot's commit: started on another machine,
pruned), the stack as it is now MUST hold, the number of commits not named.

#### Scenario: The repository lacks the robot's commit
- **GIVEN** the repository lacks the commit of a running robot (started on another machine, pruned)
- **WHEN** what the robot lands is decided
- **THEN** the span is unknown, and the stack as it is now holds, the number of commits not named
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_missing_source_leaves_the_span_unknown`)

### Requirement: Pushing into a branch whose Safe Push runs: A push decides on its holds after the answer

A push MUST decide on its holds after the answer, for the branches as the sync leaves them: the push of a branch alone
on the view Rust derives of the robots it asked for the push (`operations::push`), a push of a stack too (`push_plan`).

#### Scenario: The push of a branch alone and of a stack
- **WHEN** the user pushes a branch alone, or a stack
- **THEN** the push decides on its holds by the view Rust derives of the robots it asked for the push, for the branches as the sync leaves them

### Requirement: Pushing into a branch whose Safe Push runs: A commit moved into a branch that is no ancestor is not looked for

The span of a running robot SHALL NOT look for a commit moved out of the robot's branch into a branch that is no
ancestor of it, as the landing of a merged robot doesn't.

#### Scenario: A commit moved into a branch that is no ancestor
- **WHEN** a commit of the robot's branch was moved into a branch that is no ancestor of it
- **THEN** that branch is not in the robot's span
  [@test] crates/sync-core/src/robot_landing_test.rs (`a_commit_moved_into_a_branch_that_is_no_ancestor_is_not_in_the_span`)

### Requirement: Pushing into a branch whose Safe Push runs: A held branch offers no push

A held branch MUST offer no push: the place of the row's push button stays empty, as its Safe Push slot does; its Status
cell says why (the robot chip, "lands with oauth", "Landed with oauth"). P MUST do nothing.

#### Scenario: The row of a held branch
- **GIVEN** a Safe Push holds the branch
- **WHEN** the user looks at its row
- **THEN** the place of the push button stays empty, as its Safe Push slot does
- **AND** its Status cell says why: the robot chip, "lands with oauth", "Landed with oauth"
- **AND** P does nothing
  [@test] src/presenters/push-button.test.ts (`offers no push while a Safe Push holds the branch, and says why; the children of a landed one push without it`)

### Requirement: Pushing into a branch whose Safe Push runs: Push is disabled with the reason on every surface

Push in the menu, the preview footer, the outgoing overview and the review window MUST be disabled with the reason of
the hold.

#### Scenario: The reasons of a disabled Push
- **GIVEN** a Safe Push holds the branch
- **WHEN** the user opens the menu, the preview footer, the outgoing overview or the review window of it
- **THEN** Push is disabled with the reason: "A Safe Push of auth is running: wait until it merges, or restart it on the current branch", "A Safe Push of oauth is running: it lands 1 commit of auth with it"
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should offer no push: it won't land, and the menu says why`)

### Requirement: Pushing into a branch whose Safe Push runs: The description of the preview's push offers the robot's actions

The description of the preview's push MUST offer what it names, from the robot's actions (`getRobotItems`): "Restart
Safe Push" of its own robot and "Open in Patronus" of the robot that holds it (`push-hold-actions`).

#### Scenario: The actions in the description of the push
- **GIVEN** a Safe Push holds the branch the preview shows
- **WHEN** the user reads the description of the preview's push
- **THEN** it says why it can't push, and offers "Restart Safe Push" of its own robot and "Open in Patronus" of the robot that holds it
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open and restart the robot from the description of the preview's push, which says why it can't push`)

### Requirement: Pushing into a branch whose Safe Push runs: A push of several branches leaves the held branch out

A push of several branches MUST leave a held branch out (`domain_policy::push::push_plan`, `Kept::Held`): Push Stack
sends only the branches above it, Push with Children of the island's header and of the root's menu skips it.

#### Scenario: Push Stack
- **GIVEN** an ancestor of the branch is held
- **WHEN** the user chooses Push Stack
- **THEN** only the branches above the nearest held one are sent
  [@test] crates/domain-policy/src/push_test.rs (`push_stack_covers_the_ancestors_up_to_the_nearest_held_one`)

#### Scenario: Push with Children
- **GIVEN** a branch of the island is held
- **WHEN** the user chooses Push with Children in the island's header or the root's menu
- **THEN** the held branch is skipped
  [@test] crates/domain-policy/src/push_test.rs (`push_with_children_leaves_out_what_asks_first_is_held_or_landed`)

### Requirement: Pushing into a branch whose Safe Push runs: A push that raced with a start is decided by Rust

A push that raced with a start (the IDE, `brd`, another window) MUST go to Rust, which reads the user's robots now:
still running, it pushes nothing and says why; finished meanwhile, it pushes. Nothing MUST ask: no dialog.

#### Scenario: The robot still runs
- **GIVEN** a start of a Safe Push of the branch raced with the push
- **WHEN** Rust reads the robot as still running
- **THEN** it pushes nothing and says why ("Cannot push branch auth", with "Open in Patronus" of the robot the view says holds it), without a dialog
  [@test] src/lib/push-run.test.ts (`is refused in the words of its blocker, and the toast opens the Safe Push that holds the branch`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes`)

#### Scenario: The robot finished meanwhile
- **GIVEN** a start of a Safe Push of the branch raced with the push
- **WHEN** Rust reads the robot as finished
- **THEN** it pushes

### Requirement: Pushing into a branch whose Safe Push runs: A branch a merged Safe Push landed is refused by the plan of the push

A branch a merged Safe Push landed MUST be refused by the plan of the push (the hold `landed`): the robot finished, its
commits are in the target.

#### Scenario: The push of a landed branch
- **GIVEN** a merged Safe Push landed the branch
- **WHEN** the user pushes it
- **THEN** the plan of the push refuses it with the hold `landed`

### Requirement: Pushing into a branch whose Safe Push runs: The app's push checks the holds in Rust before it pushes

The app's push itself (`execute` of the intent `Push`, of a branch alone and of a stack, the E2E server's route of it)
MUST check the same in Rust before it pushes, by the rules of `brd push` (the plan of the push,
`derive::alone_push_of` and `domain_policy::push::push_plan` over the holds of `push_guard::guard_facts`, one sequence
of both hosts, `operations::push::execute_push`).

#### Scenario: The E2E server's route of the push
- **GIVEN** a Safe Push of the branch runs
- **WHEN** the push reaches the E2E server's route of it
- **THEN** it is refused before git pushes
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_push_into_a_branch_whose_safe_push_runs_is_refused_before_git_pushes`)

### Requirement: Pushing into a branch whose Safe Push runs: The push reads the user's robots first as it starts

The push MUST read the user's robots first: one full list asked as it starts, while it waits for the rewrites before it
and the running sync, asked anew as a background request, never answered by a list kept for the page; none off Space,
without a token, while Patronus fails or its requests back off, as `brd push` reads its holds. No network request MUST run under the
repository lock.

#### Scenario: One full list asked as the push starts
- **WHEN** the push starts
- **THEN** it asks Patronus for the full list every time, and sees a robot started elsewhere
  [@test] crates/patronus/src/service_test.rs (`the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere`)
  [@test] crates/operations/src/push_test.rs (`the_push_of_an_unapplied_branch_reads_no_robots`)

#### Scenario: The push waits for the first sync
- **WHEN** the push waits for the rewrites before it and the running sync
- **THEN** it asks the robots while it waits
  [@test] crates/operations/src/rewrite_test.rs (`a_push_asks_the_robots_while_it_waits_for_the_first_sync`)

#### Scenario: No request while Patronus can't be asked
- **WHEN** the repository is off Space, there is no token, or Patronus fails or its requests back off
- **THEN** the push sends no request
  [@test] crates/patronus/src/service_test.rs (`the_checks_send_nothing_while_the_requests_back_off`)

### Requirement: Pushing into a branch whose Safe Push runs: The push of unapplied branches alone reads no robots

The push MUST read no robots for unapplied branches alone, on which no Safe Push runs.

#### Scenario: The push of an unapplied branch
- **WHEN** the user pushes an unapplied branch alone
- **THEN** no list of the robots is asked
  [@test] crates/operations/src/push_test.rs (`the_push_of_an_unapplied_branch_reads_no_robots`)

### Requirement: Pushing into a branch whose Safe Push runs: Under the lock the push refuses a held or unbuilt branch

Under the lock the push MUST refuse a branch the last sync has no row of, a branch a merged Safe Push landed whole, a
branch whose own Safe Push runs, one a Safe Push of a branch built on it or of its whole stack lands, and the stack
branch of a whole stack while its robot runs on it, every branch of it landed whole or it is not `built`: "Nothing was
pushed: …", shown in the error toast of the push ("Cannot push branch auth").

#### Scenario: A branch the last sync has no row of
- **WHEN** the push names a branch the last sync has no row of
- **THEN** it is refused: "… is no branch of the last sync"

#### Scenario: A branch a merged Safe Push landed whole
- **WHEN** the push names a branch a merged Safe Push landed whole
- **THEN** it is refused: "… merged into master with its Safe Push (…): nothing of it is left to push"

#### Scenario: A branch a running Safe Push holds
- **WHEN** the push names a branch whose own Safe Push runs, or one a Safe Push of a branch built on it or of its whole stack lands
- **THEN** it is refused: "Nothing was pushed: …", in the error toast of the push ("Cannot push branch auth")
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push nothing into a branch whose Safe Push started after the page's list: Rust refuses it, and the toast says why`)

#### Scenario: The stack branch of a whole stack
- **WHEN** the push names the stack branch of a whole stack while its robot runs on it, every branch of it landed whole, or it is not `built` (a sync since the page's decision left it conflicted or stale)
- **THEN** it is refused, nothing pushed

### Requirement: Pushing into a branch whose Safe Push runs: The robots are the ones Rust read for the push

The robots of the push MUST be the ones Rust read for the push, never the page's and never a list kept before it: a
robot the page canceled before its restart is canceling and holds nothing, a robot that finished holds nothing, and a
robot the IDE or `brd` started since the page's list holds.

#### Scenario: A robot started since the page's list
- **GIVEN** the IDE or `brd` started a Safe Push of the branch since the page's list
- **WHEN** the app pushes the branch
- **THEN** that robot holds, and nothing is pushed
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push nothing into a branch whose Safe Push started after the page's list: Rust refuses it, and the toast says why`)

#### Scenario: A canceling or finished robot
- **GIVEN** the page canceled the robot before its restart, or the robot finished
- **WHEN** the app pushes the branch
- **THEN** that robot holds nothing

#### Scenario: A push without the page's checks
- **WHEN** a push raced with a start, or one came by IPC without the page's checks, and a running Safe Push holds the branch
- **THEN** it pushes nothing then

### Requirement: Pushing into a branch whose Safe Push runs: The push holds the repository lock only while it decides

The app's push MUST hold the repository lock only while it decides: the checks above and the commits that go, fixed
under it; the network push after it sends those commits (`<commit>:refs/heads/<name>`), each leased on the remote head
the row showed, so a sync or a rewrite that moves a branch meanwhile changes nothing of what goes.

#### Scenario: A push that sends holds no lock
- **GIVEN** a push sends its commits over the network
- **WHEN** another branch's push, a rewrite and a sync run meanwhile
- **THEN** they finish meanwhile, as the push holds no lock
- **AND** it sends the commit it froze
  [@test] crates/operations/src/push_test.rs (`a_push_that_sends_holds_no_lock_and_sends_the_commit_it_froze`)
  [@test] crates/sync-core/src/push_test.rs (`a_prepared_push_sends_the_commit_it_was_prepared_for`)

### Requirement: Pushing into a branch whose Safe Push runs: Pushes of several branches run side by side

Safe Pushes (or pushes) pressed on several branches one after another MUST push side by side: none waits for another
one's network push, and none is refused with the app itself as the holder ("`branch-deck` (pid …, for 10 s) holds the
repository").

#### Scenario: Two branches pressed one after the other
- **WHEN** the user presses Safe Push (or push) on two branches one after the other
- **THEN** both push side by side, neither waits for the other's network push, and neither is refused with the app as the holder
  [@test] crates/operations/src/push_test.rs (`the_pushes_of_two_branches_run_side_by_side`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Pushes of two branches pressed one after the other, side by side`)

### Requirement: Pushing into a branch whose Safe Push runs: The pushes of one remote ref wait for each other

Only the pushes of one remote ref (the same branch twice, two whole stacks of one stack branch) MUST wait for each
other, without a bound: the registry of the operations of the session admits a push per ref it writes (`Queued` behind
the push before it; a second click of the same push joins it), and the second then finds the remote at its commit.

#### Scenario: The same branch pushed twice
- **WHEN** two pushes write the same remote ref
- **THEN** the second is `Queued` behind the first, without a bound, and then finds the remote at its commit
- **AND** a second click of the same push joins it
  [@test] crates/operations/src/push_test.rs (`the_pushes_of_one_ref_wait_for_each_other_and_both_push`)

### Requirement: Pushing into a branch whose Safe Push runs: A push whose window closes runs to its end

A push whose window closes MUST run to its end in the task of its command, its refs held until its `git push` ended, and
its outcome MUST go to the session.

#### Scenario: The window closes during a push
- **WHEN** the window of a running push closes
- **THEN** the push runs to its end, its refs held until its `git push` ended
- **AND** its outcome goes to the session

### Requirement: Pushing into a branch whose Safe Push runs: A push waits while another holder of the lock lives

While another holder has the repository lock (`brd pick --follow` of an agent for 48 s, a rewrite of the app), the push
MUST wait as long as the holder lives, never refused after 10 s, and say whom it waits for, in every window (the step of
the operation in the view names the holder, `WaitingForLock`); once the holder releases the lock, the push goes.

#### Scenario: Whom the push waits for
- **GIVEN** another process holds the repository lock
- **WHEN** the user pushes or starts a Safe Push
- **THEN** the step of the Safe Push and the push button show "Waiting…", their name and hover "Waiting for `brd pick` (pid 4242, for 12 s)", updated about once a second, in every window
  [@test] crates/history-rewrite/src/repository_lock_test.rs (`every_wait_of_this_process_names_the_other_process_that_holds_the_lock`)
  [@test] src/presenters/lock-wait.test.ts (`says whom an operation waits for as the rewrites of the app say it: brd by its command line, the app by its name`)
  [@test] src/presenters/lock-wait.test.ts (`reads whom an operation of the view waits for from its step, and nothing of another step`)
  [@test] src/lib/transfer-progress.test.ts (`names the holder of the repository lock the push waits for, without a percent`)

#### Scenario: The holder releases the lock
- **GIVEN** a push waits for a live holder of the lock, past 10 s
- **WHEN** the holder releases the lock
- **THEN** the push goes
  [@test] crates/operations/src/push_test.rs (`a_push_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should wait for another holder of the repository lock as long as it lives, and then start`)

#### Scenario: A start behind another start of the app
- **WHEN** a start waits behind another start of the app in the app's line
- **THEN** it says whom it waits for, as for another holder

### Requirement: Pushing into a branch whose Safe Push runs: A lock no record names a holder of refuses the push

Only a lock that no record names a live holder of (a holder that writes none) MUST refuse the push after 10 s.

#### Scenario: An unnamed holder of the lock
- **GIVEN** a holder that writes no record holds the repository lock
- **WHEN** the push has waited 10 s
- **THEN** it is refused: "Nothing was pushed: another Branch Deck operation holds the repository; try again when it is done"
  [@test] crates/sync-core/src/repository_lock_test.rs (`the_wait_for_a_live_holder_names_it_ends_when_canceled_and_refuses_an_unnamed_one`)

### Requirement: Pushing into a branch whose Safe Push runs: Stop Waiting ends the wait of a push

Stop Waiting MUST end the wait of a push for another holder of the repository lock, before anything was pushed: in the
hover of the push button and beside the step of a Safe Push while its push waits. The action whose step the push is
(the Safe Push start, a review request) MUST start nothing and show no error; the same step pushes and starts once
pressed again.

#### Scenario: Stop Waiting of a push
- **GIVEN** a push waits for another holder of the repository lock
- **WHEN** the user chooses Stop Waiting in the hover of the push button
- **THEN** the push answers the code `canceled`, and the info toast "Canceled the push of auth before it started" says "Nothing was pushed"
  [@test] crates/operations/src/push_test.rs (`a_push_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] src/lib/transfer-progress.test.ts (`offers Stop Waiting of a push only while it waits for another holder of the repository lock`)
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should say whom a push waits for in the hover of its button, whose Stop Waiting pushes nothing`)

#### Scenario: Stop Waiting beside the step of a Safe Push
- **GIVEN** the push of a Safe Push start waits for another holder of the repository lock
- **WHEN** the user chooses Stop Waiting beside its step
- **THEN** nothing is pushed, the Safe Push starts nothing and shows no error
- **AND** the same step pushes and starts once pressed again
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should stop the wait of a Safe Push for another holder of the repository lock with Stop Waiting beside its step, nothing pushed or started`)

### Requirement: Pushing into a branch whose Safe Push runs: What was pressed while the holder held the lock runs after

Safe Pushes and a move pressed while another holder holds the repository lock MUST all run once it released it.

#### Scenario: Two Safe Pushes and a move behind the lock
- **GIVEN** another process holds the repository lock
- **WHEN** the user presses two Safe Pushes and a move, and the holder releases the lock
- **THEN** all of them run
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should run two Safe Pushes and a move started while another process holds the repository lock once it released it`)

### Requirement: Pushing into a branch whose Safe Push runs: An operation of another process shows on its row

A Safe Push, a Dry Run or a push that another process of the repository runs (an agent's `brd safe-push auth`, in this
worktree or another one under the same branch prefix) MUST show on its row in every window while it runs: the slot of
the row busy with "brd: Safe Push of auth…", its hover naming the process.

#### Scenario: The Safe Push brd runs
- **GIVEN** an agent runs `brd safe-push auth` in this worktree or another one under the same branch prefix
- **WHEN** a window shows the repository
- **THEN** the slot of the row of auth is busy with "brd: Safe Push of auth…"
- **AND** its hover names the process: "`brd` (pid 4242) runs the Safe Push of auth, which holds what it needs until it ends"
  [@test] crates/operations/src/registry_watch_test.rs (`a_window_shows_the_operations_another_process_runs_under_its_prefix_while_it_watches`)
  [@test] src/presenters/operations.test.ts (`words an operation of another process as its row says it and as the hover names its pid`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show the Safe Push bd runs on its row, refuse a start and a push of it, and push another branch beside it`)

### Requirement: Pushing into a branch whose Safe Push runs: The start and the push of what another process runs are refused

S, a click and the menu MUST say why the start can't start in an info toast and open no popover; the push of the
branch, which needs what it holds, MUST be disabled with it before Rust is asked (`Busy`); a push or a start of the app
that reaches Rust all the same MUST be refused as `busy` before anything runs, its toast naming it. Another branch
pushes and starts beside it.

#### Scenario: A start of the branch
- **GIVEN** another process runs the Safe Push of auth
- **WHEN** the user presses S, clicks the start or chooses it in the menu
- **THEN** an info toast says why ("Safe Push of auth can't start" with those words), and no popover opens

#### Scenario: The push of the branch
- **GIVEN** another process runs the Safe Push of auth
- **WHEN** the user looks at the push of the branch, or presses P
- **THEN** it is disabled before Rust is asked (`Busy`: "brd: Safe Push of auth…" on its item and its button), and P says it in an info toast

#### Scenario: A push or a start that reaches Rust
- **GIVEN** another process runs the Safe Push of auth
- **WHEN** a push or a start of the app of it reaches Rust all the same
- **THEN** it is refused as `busy` before anything runs, its toast naming it ("Cannot push branch auth: `brd` (pid 4242) has run the Safe Push of auth for 12 s")
- **AND** another branch pushes and starts beside it
  [@test] crates/bd-cli/src/cross_process_test.rs (`the_apps_safe_push_beside_bd_of_the_same_branch_is_busy_naming_bd`)

### Requirement: Pushing into a branch whose Safe Push runs: The start is offered again once the other process ended

Once the other process ended (its record left the operations registry of the repository), the row MUST offer the start
again, and it MUST start once.

#### Scenario: The operation of brd ended
- **GIVEN** the Safe Push of `brd` held the branch
- **WHEN** its record left the operations registry of the repository
- **THEN** the row offers the start again, and it starts once
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should start the Safe Push of a branch once the operation of bd that held it ended`)

### Requirement: Pushing into a branch whose Safe Push runs: brd beside the app's Safe Push is busy naming the app

The other way round, `brd safe-push auth` beside the app's Safe Push of auth MUST be `busy` naming the app's operation.

#### Scenario: brd beside the app's Safe Push of the same branch
- **GIVEN** the app runs the Safe Push of auth
- **WHEN** an agent runs `brd safe-push auth`
- **THEN** it is `busy`, naming the app's operation
  [@test] crates/bd-cli/src/cross_process_test.rs (`bd_beside_the_apps_safe_push_of_the_same_branch_is_busy_naming_the_app`)

### Requirement: Pushing into a branch whose Safe Push runs: A start whose answer was lost is never sent again

A start whose answer was lost MUST never be sent again by itself, and MUST hold its subject for every process until it
is known.

#### Scenario: The answer of a start was lost
- **WHEN** the answer of a start was lost
- **THEN** the start is not sent again by itself: Patronus got it once
  [@test] crates/patronus/src/actions_test.rs (`a_start_whose_answer_was_lost_is_lost_and_sent_once`)

#### Scenario: A lost start holds its subject
- **GIVEN** a start of this process whose answer was lost
- **WHEN** another admission of its subject comes after its entry
- **THEN** the lost start holds it
  [@test] crates/operations/src/registry_file_test.rs (`a_start_of_this_process_whose_answer_was_lost_holds_its_own_admissions_after_its_entry`)

### Requirement: Pushing into a branch whose Safe Push runs: The record of a start tells what it sends before it goes out

Before its request goes out, the record of the start in the operations registry MUST tell what it sends: its source
ref, the commit, the route, the pull request, the time; the comment once GitHub answered its post.

#### Scenario: A start about to be sent
- **WHEN** the request of a start is about to go out
- **THEN** its record in the operations registry already tells its source ref, the commit, the route, the pull request and the time
- **AND** for a comment, the comment once GitHub answered its post

### Requirement: Pushing into a branch whose Safe Push runs: A start whose answer was lost is unknown, never failed

A request whose answer was lost after it went out MUST be unknown, never failed: Patronus got the start once, and the
outcome says why it is not known.

#### Scenario: The answer of a start is lost
- **WHEN** the answer is lost after the request went out: a timeout, a connection that broke off, an answer that doesn't read, a 504 of a gateway; for a comment also a bot that did not answer within the minute
- **THEN** the start is unknown, never failed, and the outcome says why it is not known
- **AND** its toast says "Cannot tell whether the Safe Push of auth started" (Open Pull Request for a comment)
  [@test] crates/operations/src/reconcile_test.rs (`a_start_whose_answer_was_lost_is_unknown_once_and_holds_until_patronus_lists_its_robot`)
  [@test] src/presenters/outcome.test.ts (`says a start whose answer was lost as not known yet, with why`)

### Requirement: Pushing into a branch whose Safe Push runs: The row of every window shows a lost start

The record of a lost start MUST stay after its run, as one of a process that was killed between its push and its start:
the row of every window MUST show "Start Unknown" in its slot, with "Open in Patronus" ("Open Pull Request #7" for a
comment) and "Forget This Start" beside it.

#### Scenario: The row of a lost start
- **GIVEN** a start whose answer was lost
- **WHEN** a window shows its row
- **THEN** the row shows "Start Unknown" in its slot ("Cannot tell whether the Safe Push of auth started: its answer was lost. It holds until Patronus lists its robot, or until Forget This Start; Patronus may still run it"), with "Open in Patronus" ("Open Pull Request #7" for a comment) and "Forget This Start" beside it; S says it can't start, and a start of `brd` or the app that needs what it holds is `busy` naming it ("Cannot tell whether the Safe Push of auth that `brd` (pid 4242) sent started", `brd operation forget <op>`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_start_whose_answer_was_lost_offers_open_in_patronus_and_forget_this_start`)
  [@test] src/presenters/operations.test.ts (`words a start whose answer was lost: whether it started is not known, and how it lets go`)
  [@test] src/presenters/actions.test.ts (`offers Open in Patronus where its robot would be and Forget This Start, which says Patronus may still run it`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should show a start whose answer was lost on its row, refuse a start of it, and start again once it is forgotten`)

#### Scenario: A lost comment start
- **GIVEN** a comment start whose answer was lost
- **WHEN** the user looks at its row
- **THEN** "Open Pull Request #7" is offered for it
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should open the pull request of a comment start whose answer was lost`)

#### Scenario: A process killed between its push and its start
- **GIVEN** a `brd` was killed between its push and its start
- **WHEN** the app reads the operations registry
- **THEN** the record stays as a lost start, which the app settles and starts once
  [@test] crates/bd-cli/src/cross_process_test.rs (`a_bd_killed_between_its_push_and_its_start_leaves_a_lost_start_the_app_settles_and_starts_once`)

### Requirement: Pushing into a branch whose Safe Push runs: A start that needs what a lost start holds is busy

While a lost start holds its subject, S MUST say it can't start, and a start of `brd` or the app that needs what it holds
MUST be `busy` naming it.

#### Scenario: A start beside a lost start
- **GIVEN** a start of auth whose answer was lost
- **WHEN** a start of `brd` or of the app needs what it holds
- **THEN** it is `busy` naming it ("Cannot tell whether the Safe Push of auth that `brd` (pid 4242) sent started", `brd operation forget <op>`)
- **AND** S says it can't start

### Requirement: Pushing into a branch whose Safe Push runs: A read of Patronus settles a lost start

A read of Patronus MUST settle a lost start: at an admission it holds back, and after each list of the robots of the
session.

#### Scenario: A list of the robots of the session
- **GIVEN** a lost start
- **WHEN** the session lists the robots, or an admission it holds back comes
- **THEN** the lost start is settled by that read
  [@test] crates/operations/src/reconcile_test.rs (`a_list_of_the_robots_of_a_session_settles_its_lost_starts`)

### Requirement: Pushing into a branch whose Safe Push runs: The robot of a lost start is of its ref and mode since it was sent

The robot of a lost start MUST be one of the user's robots of its source ref in its mode started since it was sent (on
its commit when Patronus names one), or of its pull request. Found, the start MUST be started: the admission is refused
as `plan_changed`, the robot in the facts of the session that settled it and of the worktree that sent it.

#### Scenario: The robot of a lost start is found
- **WHEN** a list has a robot of the user of its source ref in its mode, started since it was sent (on its commit when Patronus names one), or of its pull request
- **THEN** the start is started: the admission is refused as `plan_changed`
- **AND** the robot is in the facts of the session that settled it and of the worktree that sent it, so neither row offers Safe Push of it again
  [@test] crates/operations/src/reconcile_test.rs (`the_robot_of_a_lost_start_is_of_its_ref_and_mode_since_it_was_sent`)

### Requirement: Pushing into a branch whose Safe Push runs: Only a list asked 60 s after a lost start proves none started

A list of the whole window asked 60 s after the start was sent at least (the 30 s a start waits for its answer, and as
long again for a request Patronus still handles) without its robot MUST mean that none started: the record leaves, and
the start goes as planned, without the push that went before.

#### Scenario: A list of the whole window without the robot
- **GIVEN** a lost start sent 60 s ago at least
- **WHEN** a list of the whole window asked since then has no robot of it
- **THEN** none started: the record leaves
  [@test] crates/operations/src/reconcile_test.rs (`a_list_of_the_whole_window_proves_a_lost_start_absent_once_it_settled`)

#### Scenario: The admission goes in
- **GIVEN** a lost start Patronus does not list once it settled
- **WHEN** its record leaves
- **THEN** the admission goes in, and the start goes as planned, without the push that went before
  [@test] crates/operations/src/reconcile_test.rs (`a_lost_start_patronus_does_not_list_once_it_settled_leaves_and_the_admission_goes_in`)

### Requirement: Pushing into a branch whose Safe Push runs: A list asked earlier proves nothing of a lost start

A list asked earlier MUST prove nothing, however late its answer came: a list of the session (which may be one kept for
the windows from before) proves only that the start started; without its robot a start that settled MUST be asked once
more since it was sent (one request for all such starts of the prefix, since the oldest), and no list kept from before
MUST hide the robot it finds.

#### Scenario: A list kept from before
- **GIVEN** a lost start, and a list the service kept from before it settled
- **WHEN** that list has no robot of it
- **THEN** it does not prove the start absent, and no list kept from before hides the robot that is found
  [@test] crates/operations/src/reconcile_test.rs (`a_list_the_service_kept_from_before_a_lost_start_settled_never_proves_it_absent`)

#### Scenario: Several lost starts of a prefix
- **GIVEN** several settled starts of the prefix without their robot
- **WHEN** they are asked once more
- **THEN** one request since the oldest of them settles them all
  [@test] crates/operations/src/reconcile_test.rs (`the_lost_starts_of_a_prefix_settle_by_one_list_since_the_oldest`)

### Requirement: Pushing into a branch whose Safe Push runs: A lost comment is settled by the reply of the bot

A comment start MUST be settled by the reply of the bot to it, found by its text among the comments of its pull request
since it was sent when its post lost its answer: accepted is started, rejected or never posted is failed.

#### Scenario: The reply of the bot to a lost comment
- **GIVEN** a comment start whose post lost its answer
- **WHEN** the comments of its pull request since it was sent are read
- **THEN** the comment is found by its text, and the reply of the bot settles it: accepted started, rejected or never posted failed
  [@test] crates/operations/src/reconcile_test.rs (`a_lost_comment_is_found_by_its_text_and_settled_by_the_reply_of_the_bot`)

### Requirement: Pushing into a branch whose Safe Push runs: Forget This Start takes the record out

Forget This Start (and `brd operation forget`) MUST take the record out without asking Patronus, and its toast MUST say
that Patronus may still run it.

#### Scenario: Forget This Start of a lost start
- **GIVEN** a start whose answer was lost
- **WHEN** the user chooses Forget This Start, or runs `brd operation forget`
- **THEN** the record is taken out without asking Patronus, and what it held is free
- **AND** the toast "Forgot the start of auth" says Patronus may still run it
  [@test] crates/operations/src/registry_file_test.rs (`a_forget_takes_out_a_start_whose_answer_was_lost_and_refuses_one_that_runs`)
  [@test] crates/operations/src/forget_test.rs (`forget_this_start_takes_the_record_out_and_frees_what_it_held`)

### Requirement: Pushing into a branch whose Safe Push runs: A mutation of brd syncs the session a window shows

After a mutation of `brd` (an agent's `brd group`, `brd push`), the session a window shows MUST sync once what a sync
reads changed, without a focus: `brd` touched the stamp of the repository, which the window watches.

#### Scenario: brd touched the stamp of the repository
- **GIVEN** a window shows the session of the repository
- **WHEN** `brd` touched the stamp of the repository after a mutation
- **THEN** the session syncs without a focus, only when what a sync reads changed
  [@test] crates/operations/src/registry_watch_test.rs (`a_stamp_of_bd_syncs_the_session_a_window_shows`)
  [@test] crates/repository-model/src/sync_scheduler_test.rs (`a_stamp_of_bd_runs_only_when_the_inputs_changed_as_a_focus`)
  [@test] tests/e2e/operations/foreign-operation.spec.ts (`should sync when bd touched the stamp of the repository after a mutation, without a focus`)

### Requirement: Pushing into a branch whose Safe Push runs: Restart Safe Push is the one push into a held branch

The one push into a held branch MUST be a robot action: "Restart Safe Push" of a running robot whose branch changed
since (the menu, the robot details, the landing overview) cancels it, pushes and restarts it on the pushed commit, the
steps of its plan in Rust (`RestartRobot`).

#### Scenario: Restart Safe Push of a branch changed since
- **GIVEN** a running robot whose branch changed since
- **WHEN** the user chooses "Restart Safe Push" in the menu, the robot details or the landing overview
- **THEN** it says what it does: "Cancels the run on 1a2b3c4, pushes auth and starts it again on the pushed commit"
- **AND** it cancels the robot, pushes and restarts it on the pushed commit
  [@test] src/lib/patronus.test.ts (`asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref`)
  [@test] crates/domain-policy/src/holds_test.rs (`a_restart_restarts_on_the_remote_or_pushes_first_by_what_the_remote_has`)
  [@test] src/presenters/robot-actions.test.ts (`names what a restart of a robot of a branch changed since does: it cancels a running one, then pushes`)
  [@test] crates/operations/src/robot_test.rs (`a_restart_that_pushes_first_cancels_the_running_robot_pushes_and_restarts_it_on_the_pushed_commit`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_push_that_restarts_the_safe_push_of_its_branch_cancels_pushes_and_restarts_it`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot, push and restart it on the pushed commit, from Restart Safe Push`)

### Requirement: Pushing into a branch whose Safe Push runs: The push of a restart shows on the push button of the branch

The push of a restart MUST show on the push button of the branch, and of the whole stack on the header's push.

#### Scenario: The restart of the robot of a whole stack
- **GIVEN** the robot of a whole stack runs and the stack changed since
- **WHEN** the user restarts it
- **THEN** the push of the stack branch alone shows on the header's push: "Pushing the whole stack 45%"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)

### Requirement: Pushing into a branch whose Safe Push runs: The restart is the action and has the one toast

The restart MUST be the action: its toast "Safe Push restarted" with "Open in Patronus" is the one toast, and Activity
MUST record the push and the restart. The advice of the details MUST name it.

#### Scenario: The toast and Activity of a restart
- **WHEN** a restart that pushes first succeeds
- **THEN** the one toast is "Safe Push restarted" with "Open in Patronus"
- **AND** Activity records the push and the restart

#### Scenario: The advice of the details
- **GIVEN** a running robot whose branch changed since
- **WHEN** the user opens the robot details
- **THEN** the advice names the restart: "… Restart Safe Push to test the current branch."

### Requirement: Pushing into a branch whose Safe Push runs: A push of the page that restarts a robot uses the same intents

A push of the page that restarts a robot (the toast of a moved remote) MUST cancel it and restart it through the same
intents.

#### Scenario: The toast of a moved remote
- **WHEN** the push of the toast of a moved remote restarts a robot
- **THEN** it cancels and restarts it through the same intents as Restart Safe Push

### Requirement: Pushing into a branch whose Safe Push runs: A Safe Push replaces the one on an older commit

A Safe Push of a branch whose Safe Push runs on an older commit MUST replace it, and confirm first. Its push MUST cancel
the robot at the first branch that robot holds (the branch, or an ancestor the stack push takes along), never restart
it; the start then starts the one robot of the branch.

#### Scenario: A new Safe Push from the popover
- **GIVEN** the Safe Push of auth runs on an older commit
- **WHEN** the user starts a Safe Push of auth from the popover
- **THEN** it confirms first: "Cancels the Safe Push of auth running on 1a2b3c4."
- **AND** its push cancels the robot and does not restart it, and the start starts the one robot of the branch
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot for a new Safe Push from the popover, and not restart it`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: No push needed
- **GIVEN** the Safe Push of the branch runs on an older commit, and the remote branch has the commits by content
- **WHEN** the user starts a Safe Push of the branch
- **THEN** the start cancels the robot itself, without a push
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should cancel the robot it replaces without a push when the remote branch has the commits`)

#### Scenario: The robot finished while the start checked
- **GIVEN** the robot to replace finished while the start checked
- **WHEN** the start goes on
- **THEN** the robot is not canceled: nothing is left to cancel
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start the Safe Push that replaces a robot which finished meanwhile, without a cancel`)

### Requirement: Pushing into a branch whose Safe Push runs: The push of Create Pull Request… replaces nothing

The push of "Create Pull Request…" MUST replace nothing: it is refused as any push, and goes on as any push for a
branch the robot's commit doesn't have.

#### Scenario: Create Pull Request… of a held branch
- **GIVEN** a running Safe Push holds the branch
- **WHEN** the user chooses "Create Pull Request…"
- **THEN** its push is refused as any push, and the robot is not replaced

#### Scenario: Create Pull Request… of a branch the robot's commit doesn't have
- **GIVEN** a running robot whose commit doesn't have the branch
- **WHEN** the user chooses "Create Pull Request…"
- **THEN** its push goes on as any push

### Requirement: Pushing into a branch whose Safe Push runs: A Dry Run of a held branch that needs a push is refused

A Dry Run of a held branch that needs a push MUST be refused with why: a dry run doesn't replace the Safe Push.

#### Scenario: A Dry Run that would push into a held branch
- **GIVEN** a running Safe Push holds the branch
- **WHEN** the user starts a Dry Run that needs a push
- **THEN** it is refused with why: a dry run doesn't replace the Safe Push

### Requirement: Pushing into a branch whose Safe Push runs: A push that finds its remote moved offers the push in a toast

A push that finds its remote branch moved MUST push nothing and offer the push over the fetched remote in a toast
([Outgoing Commits](../../repository/outgoing/spec.md#push)). When it drops the commits the remote has now, the toast's
action MUST first open the confirm of the row's push button.

#### Scenario: The remote branch moved
- **WHEN** a push finds its remote branch moved
- **THEN** it pushes nothing, and a toast offers the push over the fetched remote
- **AND** when that push drops the commits the remote has now, the toast's action first opens the confirm of the row's push button

### Requirement: Pushing into a branch whose Safe Push runs: A robot canceled for a push that did not happen stays canceled

A robot its restart canceled for a push that did not happen MUST stay canceled until the toast's push, which restarts it
on the pushed commit; with nothing left to push it MUST be restarted at once.

#### Scenario: The remote refused the push of a restart
- **GIVEN** a restart canceled the robot, and the remote refused its push
- **WHEN** the toast of the moved remote shows
- **THEN** it says so: "The Safe Push is canceled: the push restarts it."
- **AND** the toast's push restarts the robot on the pushed commit
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should restart a robot canceled for a push the remote refused once the toast pushes`)

#### Scenario: Nothing left to push
- **GIVEN** a restart canceled the robot, and its push did not happen
- **WHEN** nothing is left to push
- **THEN** the robot is restarted at once, and the toast says that instead ("The Safe Push restarted on it.") with "Open in Patronus"

### Requirement: Pushing into a branch whose Safe Push runs: brd push holds the same branches

`brd push` MUST hold the same branches ([Agent CLI](../../cli/agent-cli/spec.md#brd-push)).

#### Scenario: brd push of a held branch
- **WHEN** an agent runs `brd push` of a branch the app holds
- **THEN** it holds the same branches

### Requirement: Branch preview: The preview shows the chip and a Patronus section

The meta line of the [branch preview](../branch-preview/spec.md) MUST end with the chip; the body MUST have a Patronus
section (the details of the chip popover) after the pull request section. A branch without a robot MUST have neither.

#### Scenario: A branch with a robot
- **GIVEN** a branch with a robot
- **WHEN** the user opens its preview
- **THEN** the meta line ends with the chip
- **AND** the body has a Patronus section, the details of the chip popover, after the pull request section
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the robot in the branch preview`)

#### Scenario: A branch without a robot
- **GIVEN** a branch without a robot
- **WHEN** the user opens its preview
- **THEN** it has neither the chip nor the Patronus section

### Requirement: Landing overview: The chip of a row is the shield alone

The [landing overview](../../repository/landing-overview/spec.md#overview) MUST show the chip next to the pull request
chip of each row, as the shield alone (the state is its accessible name and in its popover): the branch names need the
room.

#### Scenario: The chip in the landing overview
- **GIVEN** a branch with a robot
- **WHEN** the user opens the landing overview
- **THEN** its row shows the shield alone next to the pull request chip
- **AND** the state is its accessible name and in its popover

### Requirement: Landing overview: A robot that failed on the branch as it is needs the user

A robot that failed on the branch as it is (the local tip, or the same commits before a sync rebuilt them on a newer
baseline, [above](#robot-of-a-row)) MUST need the user: the [landing indicator](../../repository/landing-overview/spec.md#indicator)
counts it as a move, the overview shows it once, as the move of its row with the robot's actions, the Dock its badge
([Robots in the Dock](../../repository/robots-indicator/spec.md#dock)).

#### Scenario: A robot failed on the branch tip
- **GIVEN** a robot failed on the local tip of the branch, or on the same commits before a sync rebuilt them on a newer baseline
- **WHEN** the user opens the landing overview
- **THEN** the landing indicator counts it as a move, and the overview shows it once, as the move of its row with the robot's actions, with the chips of the other rows
- **AND** the Dock shows its badge
  [@test] crates/domain-policy/src/attention_test.rs (`a_robot_failed_on_the_subject_as_it_is_only_on_its_tip_or_on_the_same_commits_rebuilt`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows`)

### Requirement: Landing overview: A failed robot needs no attention when the branch changed or is held

A branch changed since (a fix) MUST need no attention, nor one that something else holds: a running Safe Push of a
branch built on it or of the whole stack holds it, a merged one landed it whole (the hold of the view), the baseline has
all of it, a running Safe Push of a branch it is built on lands a part of it, or it is parked; nor the whole stack while
a running Safe Push of a branch of it lands a part of it.

#### Scenario: Something else holds the branch
- **GIVEN** the robot of a branch failed
- **WHEN** a running Safe Push of a branch built on it or of the whole stack holds it, a merged one landed it whole (the hold of the view), the baseline has all of it, a running Safe Push of a branch it is built on lands a part of it (a Dry Run lands nothing; a canceling one still counts, as for the start), or it is parked
- **THEN** it needs no attention: its row waits ([Next Step](../next-step/spec.md)), and the old failure is in the line of the last day of the landing overview
  [@test] crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_branch_needs_the_user_unless_something_else_holds_the_branch`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should not count the failed robot of a branch while the Safe Push of a branch built on it lands it`)

#### Scenario: The branch changed since
- **GIVEN** the robot of a branch failed
- **WHEN** the branch changed since (a fix)
- **THEN** it needs no attention
  [@test] crates/domain-policy/src/attention_test.rs (`a_robot_failed_on_the_subject_as_it_is_only_on_its_tip_or_on_the_same_commits_rebuilt`)

#### Scenario: The failed robot of a whole stack
- **GIVEN** the robot of a whole stack failed
- **WHEN** a running Safe Push of a branch of the stack lands a part of it
- **THEN** the whole stack needs no attention while it does
- **AND** it needs the user only on a stack branch the sync built
  [@test] crates/domain-policy/src/attention_test.rs (`the_failed_robot_of_a_whole_stack_needs_the_user_only_on_a_built_stack_branch_no_robot_of_a_branch_lands`)

### Requirement: Landing overview: The failure is judged by content again once the holding robot ended

Once the robot that holds the branch merged or failed, the failure MUST be judged by content again.

#### Scenario: The holding robot merged or failed
- **GIVEN** a failed robot needs no attention while another robot holds its branch
- **WHEN** that robot merged or failed
- **THEN** the failure is judged by content again

### Requirement: Landing overview: Rust decides which robots need the user

Rust MUST decide which robots need the user (`domain_policy::attention`): the view names each subject whose robot needs
the user (`attention`, derived with every patch over the rows as they are, `derive::attention_of`), and the page MUST
list what the view names.

#### Scenario: The view names the robots that need the user
- **WHEN** the view is derived with a patch
- **THEN** it names each subject whose robot needs the user, over the rows as they are
  [@test] crates/repository-model/src/derive_test.rs (`the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are`)

#### Scenario: A failure the view does not name
- **WHEN** a row has a failed robot the view does not name, or the view names another robot than the row's
- **THEN** the page asks nothing for it: it is finished history
  [@test] src/lib/robot-overview.test.ts (`asks nothing for a failure the view does not name, nor for one it names of another robot than the row's: it is finished history`)

### Requirement: Landing overview: No robot of the cached frame of an earlier run needs the user

No robot of the cached frame of an earlier run MUST need the user (its table until the first sync of this run commits,
its robots until Patronus answers this run): no move, no badge that a fresh answer takes back.

#### Scenario: The warm start from a cached frame
- **GIVEN** the cached frame of an earlier run has a failed robot
- **WHEN** the window starts, before the first sync of this run commits or Patronus answers this run
- **THEN** no move and no badge show for it

### Requirement: Landing overview: The footer says when Patronus was checked

The footer MUST have a line "Patronus checked just now" (it ages while shown, as every relative time:
[Update Project](../../repository/update-project/spec.md)) with a Refresh button (a full list), named "Refresh
Patronus".

#### Scenario: Refresh from the overview
- **GIVEN** Patronus answered
- **WHEN** the user presses "Refresh Patronus" in the footer
- **THEN** the robots are listed again, in full, and the line says "Patronus checked just now", aging while shown
  [@test] tests/e2e/patronus/patronus.spec.ts (`should list the robots again from the overview`)
  [@test] src/lib/check-counts.test.ts (`lists the failing checks first, then the running, the canceled and the passed ones`)

### Requirement: Landing overview: When Patronus can't answer the footer line is a quiet hint

When Patronus can't answer, the line MUST be a quiet hint instead, never a toast. A repository that is not on Space MUST
have no line.

#### Scenario: The Safe Push CLI is not logged in
- **WHEN** Patronus can't answer as the Safe Push CLI is not logged in
- **THEN** the line is a quiet hint: "Run ./safePush.cmd once to log in to Patronus", with "Copy Command"
- **AND** it is no toast
  [@test] tests/e2e/patronus/patronus.spec.ts (`should hint quietly when the Safe Push CLI is not logged in`)
  [@test] src/lib/patronus.test.ts (`hints at what to do, and says nothing when there is nothing to do`)

#### Scenario: The command of the login hint
- **WHEN** the user presses "Copy Command"
- **THEN** it copies `cd <repository> && ./safePush.cmd`, PowerShell's `cd <repository>; .\safePush.cmd` on Windows (`formatSafePushLoginCommand`)
  [@test] src/lib/patronus.test.ts (`names the command of the login hint, into the repository, quoted for the shell of the platform`)

#### Scenario: Another error of Patronus
- **WHEN** Patronus can't answer for another reason
- **THEN** the line is "Patronus: <the error>"

#### Scenario: A repository not on Space
- **WHEN** the repository is not on Space
- **THEN** the footer has no line of Patronus
  [@test] tests/e2e/patronus/patronus.spec.ts (`should say nothing for a repository that is not on Space`)

## User Experience

- The chip is the same text-sized button as the pull request chip (`StatusChip`): `text-xs`, a `size-3.5` shield,
  underlined on hover. The dot is `size-1.5 rounded-full`.
- The robot of a row reads as the status text it replaces: the chip at `text-sm` with a `size-3.5` icon where the status
  dot is, `h-6`, no padding, in the state column of the Status cell, at least `13ch` wide ("59m · ~1h 59m" fits, so
  its ticking time moves no column; "usually ~40m" past its usual time widens the column once) and at most 10rem; its text truncates. The elapsed time is `text-foreground`, the
  rest muted, in tabular figures. The Safe Push slot is the step column of the Actions cell, at most 10rem
  ([Branch columns](../../../../docs/design/ui-theme.md#branch-columns)); its step is a ghost `xs` button ("Restart Safe Push" with the `RotateCcwIcon`, the pull request with its
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
  the real service against a fake Patronus of each test repository ([E2E Testing](../../../../docs/design/e2e-testing.md#remote-services)).
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
  ([The remote of a repository](../../../../docs/design/e2e-testing.md#the-remote-of-a-repository)).
  No test answers a Patronus write in the page (`routeCommand`, a fulfilling `page.route`): the registry test fails it,
  and the page answers of the reads are pinned and only go down (`ROUTED_IN_PAGE`).
  [@test] crates/test-server/src/command_registry_test.rs (`no_e2e_test_answers_a_write_in_the_page`)
  [@test] crates/test-server/src/command_registry_test.rs (`the_page_answers_of_the_e2e_tests_are_pinned`)
- Against the real Patronus: `pnpm run tauri dev`, open a checkout of `ultimate`; the log has one line per request
  ("listed Patronus robots", "polled Patronus robots") with its duration.

## Open Questions / Risks

- The pre-check is a hint: the review check of the robot decides (the chip shows it). `/space/merge-requests` finds only
  an open merge request from the branch into the target.
- The code-owner check counts the approvals of a GitHub pull request only for a robot its `/patronus` comment started;
  a REST start counts none. Until Patronus evaluates pull requests in the pre-check (its flag), an approval in the
  pull request makes the readiness `unknown`, and the robot decides.
- A mirror configured only in Space, not in its committed settings, needs `branchdeck.githubRepository` (Settings →
  Landing, GitHub repository); without it the branches are treated as on Space.

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

- [Branch Row](../branch-row/spec.md): the Actions cell with the Safe Push slot, the Status cell and the anatomy of the chip
  details
- [Pull Requests](../pull-requests/spec.md): the other review status, the same chip and refresh gate
- [Outgoing Commits](../../repository/outgoing/spec.md)
- [Landing Overview](../../repository/landing-overview/spec.md): the running and failed robots in the header
- [Robots in the Dock](../../repository/robots-indicator/spec.md#dock): the Dock badge and progress
- [Branch Preview](../branch-preview/spec.md)
- [Remote Status for Virtual Branches](../../../../docs/design/remote-status.md): the pushed tip
- Patronus REST API: YouTrack PAT-A-11; Safe Push CLI: PAT-A-51
