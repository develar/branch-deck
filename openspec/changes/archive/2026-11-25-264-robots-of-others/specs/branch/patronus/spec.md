## ADDED Requirements

### Requirement: Reading robots: A robot someone else started on a pushed branch of the user is the branch's

The robots MUST be the user's and, whoever started them, those of the pushed branches of the user: each virtual branch
and stack branch the remote has (`BranchTable::robot_sources`), but a branch the baseline has whole. A robot someone
else started there (a reviewer's Safe Merge from the merge request, a `/patronus` comment of theirs, a start in the web
UI) MUST be the robot of its branch as the user's own is. A robot of someone else on a temporary ref or on a branch
that is not the user's MUST NOT be read at all, so it never matches a branch by its revision. It MUST NOT be the robot
of a start of the user: neither of a lost start nor of a `/patronus` comment of the user.

#### Scenario: A reviewer's Safe Merge from the merge request of a pushed branch
- **GIVEN** a branch of the user is pushed, and another person started Safe Merge from its merge request
- **WHEN** the robots are listed
- **THEN** that robot is the robot of the branch, with its state, who started it and its link to Patronus
  [@test] crates/patronus/src/service_test.rs (`a_full_list_asks_the_users_robots_and_those_of_each_pushed_branch`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show a Safe Merge a colleague started on a pushed branch with who started it, and refuse its push while it runs`)

#### Scenario: A robot of someone else that is not of a branch of the user
- **GIVEN** someone else runs a robot on a temporary ref or on a branch of their own
- **WHEN** everyone's robots of a short window are listed
- **THEN** it is not read, so no branch of the user matches it by its revision
  [@test] crates/patronus/src/service_test.rs (`a_short_incremental_list_asks_everyone_once_and_keeps_the_users_robots_and_those_of_the_pushed_branches`)

#### Scenario: The pushed branches
- **WHEN** the pushed branches of a table are read
- **THEN** they are the virtual branches and the stack branches the remote has, but a branch the baseline has whole; of
  a subject those of its stack and the stack branches of it, by the remote of the session, which a push moved since the sync
  [@test] crates/repository-model/src/branch_table_test.rs (`the_robot_sources_are_the_pushed_branches_and_stack_branches_not_in_the_baseline`)

#### Scenario: A start of the user
- **GIVEN** someone else started a robot on the branch, or commented `/patronus` on its pull request, after a start of the user was sent
- **WHEN** the robot of the start of the user is looked for
- **THEN** that robot is never it
  [@test] crates/operations/src/robot_start_test.rs (`a_comment_start_takes_no_robot_someone_else_started`)

### Requirement: Reading robots: A robot someone else started names who started it

A robot someone else started MUST name who (`startedBy`, the name of the owner Patronus lists against the principal
that asks, `me`): the details of its chip ("Started by"), the words of a hold and of the refusal of a push ("A Safe Push
of auth that Jane Doe started is running"), the cancel of a start that replaces it, what its Cancel and its Restart
stop, its toasts and Activity. A poll and the details, which do not say who asks, MUST name the owner by the principal
of the last list, and an answer that does not name it MUST keep the name a list gave.

#### Scenario: The details of its chip
- **WHEN** the details of a robot someone else started open
- **THEN** they say who started it ("Started by Jane Doe")
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show a Safe Merge a colleague started on a pushed branch with who started it, and refuse its push while it runs`)

#### Scenario: The words of its hold and of its actions
- **WHEN** a robot someone else started holds the branch, or its Cancel or Restart is offered
- **THEN** the words name who started it, in Rust and on the page
  [@test] crates/repository-model/src/push_guard_test.rs (`the_hold_words_name_who_started_a_robot_of_someone_else`)
  [@test] src/presenters/reasons.test.ts (`names who started a Safe Push of someone else that holds the branch, as Rust's refusal of a push does`)
  [@test] src/presenters/robot-actions.test.ts (`names who started a running robot of someone else in what its cancel and restart stop`)

#### Scenario: A poll
- **WHEN** a poll answers a robot someone else started
- **THEN** it is named by the principal of the last list, and before any list a poll forgets no name a list gave
  [@test] crates/patronus/src/service_test.rs (`a_poll_names_who_started_a_robot_by_the_principal_of_the_last_list`)
  [@test] crates/repository-model/src/facts_test.rs (`a_poll_without_the_owner_keeps_who_started_the_robot`)

### Requirement: Load on Patronus: A full list asks one list per pushed branch, a short one everyone's once

Patronus filters a list by one source branch, and a week of everyone's robots of the monorepo is megabytes. A list of
the full window MUST ask the user's robots (`owner=me`) and one list of each pushed branch (`sourceBranch`), at most 4 in
flight. An incremental list of at most 30 minutes MUST ask everyone's robots of its minutes once, with the whole window
of each branch pushed since a list answered for it (`TableCommitted` tells the session's scheduler the pushed branches
of the table); a longer one asks as a full list. A list that failed for one branch MUST fail the read: a full one
without the robots of a branch would drop them from the session.

#### Scenario: The lists of a full and of an incremental window
- **WHEN** the session lists the robots of the full window, then an incremental window after a branch was pushed
- **THEN** the full list asks the user's robots and each pushed branch, the incremental one everyone's robots of its
  minutes and the whole window of the newly pushed branch
  [@test] crates/remote-facts/src/scheduler_test.rs (`lists_the_pushed_branches_of_the_committed_table_and_a_newly_pushed_one_for_the_whole_window`)
  [@test] crates/patronus/src/service_test.rs (`a_short_incremental_list_asks_everyone_once_and_keeps_the_users_robots_and_those_of_the_pushed_branches`)

#### Scenario: The list of one branch fails
- **WHEN** the list of one pushed branch fails
- **THEN** the read fails, and the session keeps the robots it has
  [@test] crates/patronus/src/service_test.rs (`a_full_list_that_failed_for_one_branch_fails_whole`)

### Requirement: Pushing into a branch whose Safe Push runs: A Safe Push someone else started holds the branch as the user's own

A running Safe Push someone else started on a pushed branch of the user MUST hold the branch, and the branches it lands,
as the user's own does, and its hold names who started it. The same actions MUST stay offered of it (Cancel, Restart,
Push and Restart, a start that replaces it): the branch and its work are the user's, and Patronus lets anyone with write
access to the repository cancel it; a restart starts a robot of the user.

#### Scenario: A reviewer's Safe Merge runs on the branch
- **GIVEN** someone else's Safe Push runs on a pushed branch, which no list of the user's robots has
- **WHEN** the user pushes the branch
- **THEN** the push is refused before git pushes anything, as for the user's own: "A Safe Push of auth that Jane Doe
  started is running: a push moves the branch it took its commit from…"
  [@test] crates/domain-policy/src/holds_test.rs (`a_robot_someone_else_started_holds_the_branch_as_the_users_own`)
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_push_into_a_branch_whose_safe_push_someone_else_runs_is_refused_before_git_pushes`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show a Safe Merge a colleague started on a pushed branch with who started it, and refuse its push while it runs`)

## MODIFIED Requirements

### Requirement: Reading robots: A robot belongs to the virtual branch of its source branch

The robots MUST be read with `GET /app/rest/v1/robots?repository=<name>&start=<time>`, the user's with `owner=me` and
those of a pushed branch of the user with `sourceBranch` ([below](#requirement-load-on-patronus-a-full-list-asks-one-list-per-pushed-branch-a-short-one-everyones-once)),
newest first. A robot MUST belong to the virtual branch whose pushed name, `{prefix}/virtual/{name}`, is its source
branch; a robot on a temporary ref (a Safe Push of the IDE or the CLI) MUST belong to the branch whose local or pushed
tip it took.

#### Scenario: The robots of the pushed virtual branches
- **WHEN** the robots are listed
- **THEN** they come newest first
- **AND** a robot whose source branch is `{prefix}/virtual/{name}` belongs to that virtual branch
  [@test] crates/patronus/src/service_test.rs (`lists_the_robots_newest_first_and_names_the_virtual_branch`)

#### Scenario: A robot on a temporary ref
- **WHEN** a robot runs on a temporary ref (a Safe Push of the IDE or the CLI)
- **THEN** it belongs to the branch whose local or pushed tip it took
  [@test] crates/domain-policy/src/robot_match_test.rs (`the_robot_of_a_branch_is_the_running_one_then_the_newest_and_a_temporary_ref_by_its_revision`)

### Requirement: Load on Patronus: The list of a check before an action is asked anew every time

The list of a check before an action of the app's page (its push, a restart, a cancel) MUST be asked anew every time,
never answered from a kept one: a robot the IDE, `brd` or someone else started since the page's list must hold, and the
lists it fetched answer the page's next list. It MUST ask the user's robots and those of the pushed branches that may
hold the push (`BranchTable::robot_sources_of`: the branches of its stack and the stack branches of it).

#### Scenario: A robot started elsewhere since the page's list
- **GIVEN** the IDE or `brd` started a robot since the page's list
- **WHEN** a check before an action of the page asks the robots
- **THEN** it asks Patronus, sees that robot and holds
- **AND** the list it fetched answers the page's next list
  [@test] crates/patronus/src/service_test.rs (`the_checks_ask_patronus_every_time_and_see_a_robot_started_elsewhere`)

#### Scenario: A robot someone else started on a pushed branch
- **GIVEN** someone else started a Safe Push on a pushed branch the push may move
- **WHEN** the check of the push asks the robots
- **THEN** it lists that branch whoever started its robots, and sees that robot
  [@test] crates/patronus/src/service_test.rs (`the_checks_see_a_robot_someone_else_started_on_a_pushed_branch`)

### Requirement: Load on Patronus: `brd status` keeps the user's robots in the object cache

`brd status` (`landed_by`, [Agent CLI](../../cli/agent-cli/spec.md#brd-status)) MUST ask Patronus by the same rules,
though every call is a process of its own: the robots, the user's and those of the pushed branches, are kept in the
`patronus_robots` store of the object cache (`robot_list.rs`), keyed by a hash of the server, the token, the Space
repository and the branch prefix, with the pushed branches listed for the whole window. A branch pushed since MUST be
asked for the whole window once; a command that listed fewer branches keeps the robots of someone else of the others.

#### Scenario: Two calls of `brd status`
- **WHEN** `brd status` runs twice while the kept robots hold
- **THEN** it asks Patronus once
  [@test] crates/bd-remote/src/robot_list_test.rs (`status_asks_patronus_once_while_the_kept_robots_hold`)

#### Scenario: A branch pushed since the kept list
- **WHEN** `brd status` lists again after a branch was pushed
- **THEN** it asks everyone's robots of the minutes since once, and the whole window of the newly pushed branch once
  [@test] crates/bd-remote/src/robot_list_test.rs (`status_lists_a_newly_pushed_branch_for_the_whole_window_once`)

### Requirement: Pushing into a branch whose Safe Push runs: The robot of a lost start is of its ref and mode since it was sent

The robot of a lost start MUST be one of the user's robots of its source ref in its mode started since it was sent (on
its commit when Patronus names one), or of its pull request; a robot someone else started on the branch MUST NOT be it.
Found, the start MUST be started: the admission is refused as `plan_changed`, the robot in the facts of the session that
settled it and of the worktree that sent it.

#### Scenario: The robot of a lost start is found
- **WHEN** a list has a robot of the user of its source ref in its mode, started since it was sent (on its commit when Patronus names one), or of its pull request
- **THEN** the start is started: the admission is refused as `plan_changed`
- **AND** the robot is in the facts of the session that settled it and of the worktree that sent it, so neither row offers Safe Push of it again
  [@test] crates/operations/src/reconcile_test.rs (`the_robot_of_a_lost_start_is_of_its_ref_and_mode_since_it_was_sent`)

#### Scenario: A robot someone else started since
- **GIVEN** someone else started a robot on the branch of a lost start since it was sent
- **WHEN** the robot of the lost start is looked for
- **THEN** that robot is never it, and the start stays unknown until the user's own robot shows
  [@test] crates/operations/src/reconcile_test.rs (`the_robot_of_a_lost_start_is_never_one_someone_else_started`)
