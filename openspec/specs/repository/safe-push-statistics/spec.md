---
name: Safe Push Statistics
description: Requirements for the statistics of the Safe Pushes of a repository: the user's robots into the target of a period (30 days by default, 14 days to 6 months) against everyone else's as the median person, the time lost in failures, the failure causes and flaky checks of the last 14 days, and the hours robots ran fastest; one Rust core for the app and `brd robot-stats`, Patronus read week by week and kept in the object cache.
style: plain-1
targets:
  - crates/domain-types/src/robot_stats.rs
  - crates/domain-types/src/calendar.rs
  - crates/patronus/src/stats.rs
  - crates/patronus/src/client.rs
  - crates/patronus/src/fake_patronus.rs
  - crates/object-cache/src/patronus_history.rs
  - crates/remote-facts/src/robot_stats.rs
  - crates/remote-facts/src/robot_stats_aggregate.rs
  - crates/bd-remote/src/robot_stats_command.rs
  - crates/app-api/src/remote/patronus.rs
  - src-tauri/src/services.rs
  - crates/test-server/src/host.rs
  - crates/test-server/src/remote_fixtures.rs
  - src/components/landing/LandingView.tsx
  - src/stores/robot-stats.ts
  - src/hooks/useRobotStats.ts
  - src/stores/settings/appSettings.ts
  - src/lib/robot-stats.ts
---

# Safe Push Statistics

Status: Implemented
Date: 2026-10-22

## Purpose

The work of a branch is done when it is in master ([Landing](../../../../docs/design/landing.md)), and a Safe Push is how it
gets there. The statistics answer what that costs: how many robots one merge takes, how long a merged robot runs, how
much time failed robots and the waits after them took, why the robots failed, which checks pass only on a retry, and
at which hours robots run fastest. The user's numbers stand beside everyone else's in the repository, as the median
person, so a slow week says whether it was the branch or the queue.

One Rust core reads them for the app and `brd robot-stats` (`remote_facts::robot_stats`), the rules that count are pure
(`remote_facts::robot_stats_aggregate`), and what Patronus answered is kept in the object cache, so a second read asks
it for little.

## Goals

- Count what lands into master and in how many robots, alike for the user and for everyone else.
- Answer how landing goes now by default: 30 days are recent enough to show the queue and the habits of today, and
  long enough for a trend of four weeks and the medians of everyone else; a longer period is a look back.
- Show nothing of another person: no name, no branch, no number of one person.
- Read Patronus little: a week is listed once it settled, a finished robot's checks once.

## Non-goals

- No chain of restarts is guessed for "robots per merge": a Safe Push of the IDE runs on a temporary ref, and its retry
  is a new robot of another source branch.
- No split of a robot's time into the queue of each check: the wait for an agent is inside a check's run, and the user
  cannot change the queue.
- No statistics of another repository, nor of a release branch.

## Requirements

### Requirement: What counts: The period is one choice of the app and of brd

The period MUST be 14 days, 30 days (the default), 10 weeks or 6 months before now (`StatsPeriod`), one choice of the
app and of `brd robot-stats --period`.

#### Scenario: A period sets the window and its weeks
- **WHEN** the statistics are read for a period
- **THEN** the window starts that period before now and the weeks are the Mondays inside it
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`the_period_sets_the_window_and_its_weeks_from_the_mondays_inside_it`)

#### Scenario: A period is written by its id, 30 days by default
- **WHEN** a period is written or read back
- **THEN** it is written by its id, and the default is 30 days
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`a_period_is_written_by_its_id`)

### Requirement: What counts: Every number counts the period but the runs

Every number MUST count the period but the runs: the failure causes, the flaky checks and the commits per merge MUST be
of the last 14 days in every period.

#### Scenario: A longer period
- **GIVEN** a period longer than 14 days
- **WHEN** the statistics are counted
- **THEN** every number counts the period
- **AND** the failure causes, the flaky checks and the commits per merge are of the last 14 days
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`failure_causes_count_the_kinds_and_checks_of_the_failed_robots_of_the_runs_window`)

### Requirement: What counts: The robots that count

A robot MUST count when a person started it (no application), it is no cherry-pick, no dry run and no emergency robot,
it started in the period and it finished. Dry runs and emergency robots of the user MUST be counted apart.

#### Scenario: Dry runs, emergency robots, applications, cherry-picks and old robots
- **GIVEN** a dry run, an emergency robot, a robot of an application, a cherry-pick and a robot started before the period
- **WHEN** the statistics are counted
- **THEN** none of them counts
- **AND** the dry runs and the emergency robots of the user are counted apart
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`dry_runs_emergency_robots_applications_cherry_picks_and_old_robots_do_not_count`)

### Requirement: What counts: Robots per merge and the time to merge

Robots per merge MUST be the finished robots over the merged ones; the time to merge MUST run from the start to the
finish of a merged robot (median, 90th percentile by the nearest rank).

#### Scenario: Merged, failed, canceled and running robots
- **GIVEN** merged, failed and canceled robots and one still running
- **WHEN** the statistics are counted
- **THEN** robots per merge are the finished robots over the merged ones
- **AND** the time to merge is the median and the 90th percentile by the nearest rank of the merged robots' runs from
  start to finish
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`robots_per_merge_are_the_finished_robots_over_the_merged_ones`)

### Requirement: What counts: The weeks are the local weeks whose Monday is in the period

The weeks of a delivery MUST be the local weeks from Monday whose Monday is in the period, the current one always, so
no week is cut by the start of the period.

#### Scenario: The number of weeks of each period
- **WHEN** the weeks of a period are counted
- **THEN** they are 2 of 14 days, 4 or 5 of 30, 10 of 10 weeks, 26 of 6 months, the current one always
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`the_period_sets_the_window_and_its_weeks_from_the_mondays_inside_it`)
  [@test] crates/domain-types/src/calendar_test.rs (`a_week_starts_on_the_local_monday`)

### Requirement: What counts: A robot counts in the week it finished

A robot MUST count in the week it finished, each week with its merged, failed and canceled robots and the median time
to merge of its merges; a robot that finished before the first Monday MUST count in the totals only.

#### Scenario: Robots in the weeks and before the first Monday
- **GIVEN** robots that finished in the weeks of the period and one that finished before its first Monday
- **WHEN** the weeks are counted
- **THEN** each week has its merged, failed and canceled robots and the median time to merge of its merges
- **AND** the robot that finished before the first Monday counts in the totals only
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`weeks_count_by_the_local_monday_a_robot_finished_in_with_their_median_time_to_merge`)

### Requirement: What counts: A week has everyone else's median, the time lost and now

Each week MUST also have everyone else's median time to merge of the merges that finished in it (with 5 people), the
user's time lost in it (failed and canceled robots by their finish, and the wait after each), and whether it is the
week of now.

#### Scenario: The week of now and an earlier one
- **WHEN** the weeks are counted
- **THEN** each has everyone else's median time to merge of the merges that finished in it, with 5 people
- **AND** the user's time lost in it: the failed and canceled robots by their finish, and the wait after each
- **AND** whether it is the week of now
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`a_week_has_everyone_elses_median_its_lost_time_and_says_whether_it_is_the_current_one`)

### Requirement: What counts: Everyone else is the median person

Everyone else MUST be the median person: the median of each person's robots per merge and time to merge, of the people
with 3 merges; none while fewer than 5 people have.

#### Scenario: People with 3 merges
- **WHEN** everyone else is counted
- **THEN** it is the median of each person's robots per merge and time to merge, of the people with 3 merges
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`everyone_else_is_the_median_person_of_those_with_three_merges`)

#### Scenario: Fewer than 5 people
- **GIVEN** fewer than 5 people with 3 merges
- **WHEN** everyone else is counted
- **THEN** there is none
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`everyone_else_needs_five_people`)

### Requirement: What counts: A first-try merge

A merge of the user MUST be first try when it is no restart and no failed or canceled robot of its branch finished in
the 3 days before; it is an estimate, of the merges whose branch is known.

#### Scenario: Merges after a restart, after a failure and without either
- **WHEN** the first-try merges are counted
- **THEN** a merge is first try when it is no restart and no failed or canceled robot of its branch finished in the 3
  days before
- **AND** only the merges whose branch is known are counted
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`first_try_is_a_merge_without_a_restart_or_a_failure_of_its_branch_in_three_days`)

### Requirement: What counts: Where the time of the user's robots goes

The time of the user's robots MUST go into merged, failed and canceled robots, and between a failed or canceled robot
and the next robot of the same branch started within 3 days.

#### Scenario: A failure followed by the next robot of its branch
- **GIVEN** a failed or canceled robot and the next robot of the same branch started within 3 days
- **WHEN** the time is counted
- **THEN** it goes into merged, failed and canceled robots, and into the wait between the two
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`time_goes_into_merged_failed_and_canceled_robots_and_between_a_failure_and_the_next_robot`)

### Requirement: What counts: What a merged robot landed

What a merged robot landed MUST be counted: the commits Patronus delivered, per merge of the last 14 days, and the
merges of a whole stack (`{prefix}/stack/<root>`).

#### Scenario: Commits and whole-stack merges
- **WHEN** the merged robots are counted
- **THEN** the commits Patronus delivered are counted per merge of the last 14 days
- **AND** the merges of a whole stack (`{prefix}/stack/<root>`) are counted
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`commits_and_whole_stack_merges_are_what_the_merged_robots_landed`)

### Requirement: What counts: The failure causes of the last 14 days

The failure causes MUST be of the user's failed robots of the last 14 days: by kind, as the failure prompt decides it
(`openspec/specs/branch/safe-push-failure-prompt/spec.md`), and the 10 checks that failed the most robots, by count,
each with the page of the newest robot it failed.

#### Scenario: Kinds and checks of the failed robots
- **WHEN** the failure causes are counted
- **THEN** the failed robots of the last 14 days count by kind, as the failure prompt decides it
- **AND** the 10 checks that failed the most robots are listed by count, each with the page of the newest robot it
  failed
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`failure_causes_count_the_kinds_and_checks_of_the_failed_robots_of_the_runs_window`)

#### Scenario: The checks and problems of a failed run are read
- **WHEN** the run of a failed robot is read
- **THEN** its checks and problems are read, and of a merged one its checks and changes
  [@test] crates/patronus/src/stats_test.rs (`a_failed_run_reads_the_checks_and_problems_a_merged_one_the_checks_and_changes`)

### Requirement: What counts: A flaky check

A flaky check MUST be one that passed on attempt 2 or later; its share of its runs MUST be given from 5 runs, with the
page of the newest robot it passed on a retry in.

#### Scenario: A check that passed on a later attempt
- **WHEN** a check passed on attempt 2 or later
- **THEN** it is flaky, with the page of the newest robot it passed on a retry in
- **AND** its share of its runs is given from 5 runs
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`a_flaky_check_passed_on_a_later_attempt_and_has_a_share_from_five_runs`)

### Requirement: What counts: The start hours are the local hours of the workdays

The start hours MUST be the 24 local hours of the workdays: the median time to merge of everyone else's merged robots
that started then, shown with 5 people, and the user's, shown with 3 of them, and a level 0 (fastest) to 4 of the hours
shown.

#### Scenario: The hours shown and their levels
- **WHEN** the start hours are counted
- **THEN** an hour shows everyone else's median with 5 people and the user's with 3 of them
- **AND** the hours shown have a level 0 (fastest) to 4
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`the_start_hours_are_local_workday_hours_shown_with_five_people_and_three_of_mine`)

### Requirement: What counts: The local zone follows its daylight saving changes

The weekday and the hour of a start MUST be local, and the local zone MUST follow its daylight saving changes.

#### Scenario: Local weekday and hour
- **WHEN** the weekday and the hour of a start are taken
- **THEN** they are local
  [@test] crates/domain-types/src/calendar_test.rs (`weekday_and_hour_are_local`)

#### Scenario: A daylight saving change
- **WHEN** the local zone changes its daylight saving
- **THEN** the hour follows the change
  [@test] crates/domain-types/src/calendar_test.rs (`the_hour_follows_the_daylight_saving_change`)

### Requirement: What counts: The fastest hours

The fastest hours MUST be the longest run of the hours of level 0 (the lowest median of everyone else's merges that
started in it among equal ones) against the slowest hour; none while no two hours differ.

#### Scenario: A longest run of level 0
- **WHEN** the fastest hours are counted
- **THEN** they are the longest run of the hours of level 0, the lowest median among equal ones, against the slowest
  hour
- **AND** there are none while no two hours differ
  [@test] crates/remote-facts/src/robot_stats_aggregate_test.rs (`the_fastest_hours_are_the_longest_run_of_the_fastest_against_the_slowest_hour`)

### Requirement: Privacy: Of another person's robot only what the rules count is kept

Of another person's robot only what the rules count MUST be kept: no name, no email, no branch, the owner as the first
8 bytes of a SHA-256 with a random salt of the cache.

#### Scenario: Everyone's robots into the target are listed
- **WHEN** the history lists everyone's robots into the target
- **THEN** no name, no email and no branch of another person is kept
  [@test] crates/patronus/src/stats_test.rs (`the_history_lists_everyone_into_the_target_and_keeps_no_name_and_no_branch_of_others`)

#### Scenario: The owner key and the salt
- **WHEN** the owner of a robot is kept
- **THEN** it is the first 8 bytes of a SHA-256 with a random salt of the cache, so the key depends on the salt
  [@test] crates/patronus/src/stats_test.rs (`the_owner_key_depends_on_the_salt`)

### Requirement: Privacy: The output names no person

The output MUST name no person, only how many people the median covers.

#### Scenario: Everyone else's numbers
- **WHEN** the statistics show everyone else's numbers
- **THEN** they name no person, only how many people the median covers

### Requirement: Load on Patronus: The robots are listed by UTC epoch week

The robots MUST be listed without an owner, into the target, by UTC epoch week (`start`, `end`, from Thursday).
Patronus refuses a list wider than 70 days, not an older one.

#### Scenario: The lists of each period
- **WHEN** the robots of a period are listed
- **THEN** 3 lists cover 14 days, 5 or 6 the 30 days, 11 the 10 weeks, 27 the 6 months
- **AND** no list is wider than 70 days
- **AND** no list names an owner
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`, `the_period_is_the_days_that_count`)

### Requirement: Load on Patronus: A week is kept by its start

A week MUST be kept by its start, so every period reads the same kept weeks, and a longer period MUST list only the
weeks not kept.

#### Scenario: A longer period after a shorter one, and back
- **GIVEN** the weeks of a period are kept
- **WHEN** a longer period is read
- **THEN** it lists only the weeks not kept
- **AND** a shorter one then asks nothing
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_longer_period_lists_only_the_weeks_not_kept_and_a_shorter_one_asks_nothing`)

### Requirement: Load on Patronus: A settled week is listed once

A week MUST be listed once it settled (it ended 2 days ago and each of its robots finished); another week again once
its list is 5 minutes old, or on Refresh.

#### Scenario: Settled weeks and the others after the TTL
- **GIVEN** the weeks of a period were listed
- **WHEN** the next read comes after the list is 5 minutes old
- **THEN** a week that settled, as it ended 2 days ago and each of its robots finished, is not listed again
- **AND** the others are listed again
  [@test] crates/remote-facts/src/robot_stats_test.rs (`the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl`)

#### Scenario: Refresh
- **WHEN** the user asks for Refresh
- **THEN** the weeks that have not settled are listed again
  [@test] crates/remote-facts/src/robot_stats_test.rs (`the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl`)

### Requirement: Load on Patronus: A read with nothing to ask answers from the cache at once

A read with nothing to ask (every week kept within its TTL, every run kept) MUST answer from the cache at once,
whatever another read is asking.

#### Scenario: Another read is asking
- **GIVEN** another read is asking Patronus
- **WHEN** a read with nothing to ask comes
- **THEN** it answers from the cache at once
- **AND** a read that must ask, a Refresh, waits for the running one
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_read_with_nothing_to_ask_does_not_wait_for_a_running_read`)

### Requirement: Load on Patronus: Two reads at once ask once

A read that asks MUST lead the read of its repository and target, and one that comes meanwhile MUST show its progress
and wait for that read to end, then read the cache again, so two reads at once ask once; nothing MUST be locked across
the lists.

#### Scenario: Two reads at once
- **WHEN** two reads of one repository and target come at once
- **THEN** the second waits for the first, then reads the cache again
- **AND** Patronus is asked once, and both answer the same numbers
  [@test] crates/remote-facts/src/robot_stats_test.rs (`two_reads_at_once_ask_once`)

#### Scenario: The read that waits shows the progress of the one it waits for
- **WHEN** a read waits for one that asks
- **THEN** it shows the progress of the read it waits for
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_refresh_that_waits_for_a_running_read_shows_its_progress_and_lists_nothing_it_listed`)

#### Scenario: Nothing is locked across the lists
- **GIVEN** a read is listing the weeks
- **WHEN** another read comes
- **THEN** no lock of the first is held across its lists: the other answers from the cache or waits for the end of
  the first

### Requirement: Load on Patronus: Only the read that asks makes the salt

Only the read that asks MUST make the salt of the owner keys.

#### Scenario: Two reads at once
- **WHEN** two reads come at once and one of them asks Patronus
- **THEN** only the one that asks makes the salt of the owner keys

### Requirement: Load on Patronus: A week the awaited read listed is not listed again

A week the read it waited for listed MUST NOT be listed again, not even on Refresh.

#### Scenario: Two Refreshes at once
- **WHEN** two Refreshes come at once and the second waits for the first
- **THEN** the second lists none of the weeks the first listed
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_refresh_that_waits_for_a_running_read_shows_its_progress_and_lists_nothing_it_listed`)

### Requirement: Load on Patronus: A read that waited writes its own start

A read that waited MUST write its own start as when it listed.

#### Scenario: After a wait
- **GIVEN** a read waited for another one
- **WHEN** it lists the weeks the other did not list
- **THEN** it writes its own start as when it listed: the weeks are kept as listed at the start of the read, not at the
  end of the wait

### Requirement: Load on Patronus: A run Patronus keeps failing keeps the reads asking

A run Patronus keeps failing MUST keep the next reads on the path that asks.

#### Scenario: A run Patronus keeps failing
- **GIVEN** Patronus keeps failing the read of a run
- **WHEN** the next reads come
- **THEN** they stay on the path that asks: the run is not kept, so none answers from the cache at once

### Requirement: Load on Patronus: Each two weeks are kept once listed

The weeks MUST be listed two at once, and each two MUST be kept as soon as they are listed: a read canceled or failing
later, and one that comes meanwhile, ask only for the rest.

#### Scenario: A read canceled or failing after the first two weeks
- **GIVEN** a read listed two weeks
- **WHEN** it is canceled or fails later, or another read comes meanwhile
- **THEN** the two weeks are kept and the next read asks only for the rest
  [@test] crates/remote-facts/src/robot_stats_test.rs (`each_two_weeks_are_kept_once_listed`)

### Requirement: Load on Patronus: The checks of a finished robot are read once

The checks of a finished robot of the user MUST be read once, with its problems when it failed and its changes when it
merged; only of the merged and failed robots of the last 14 days, at most 4 robots at once and 120 per read, the
newest first: the next read asks for the rest.

#### Scenario: The runs of the user's merged and failed robots, read once
- **GIVEN** the user's merged, failed and canceled robots, an older one and another person's
- **WHEN** a read reads the checks, and a later one again
- **THEN** only the checks of the user's merged and failed robots of the last 14 days are read, the problems of the
  failed one only
- **AND** the later read reads none of them again
  [@test] crates/remote-facts/src/robot_stats_test.rs (`the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl`)

#### Scenario: More runs than one read takes
- **GIVEN** more merged and failed robots of the last 14 days than 120
- **WHEN** a read reads their checks
- **THEN** it reads at most the 120 newest, and the next read asks for the rest
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_read_asks_for_at_most_the_newest_runs_and_the_next_one_for_the_rest`)

#### Scenario: At most 4 robots at once
- **WHEN** the checks of the robots are read
- **THEN** at most 4 robots are read at once
- **AND** a rejected token stops the reads
  [@test] crates/patronus/src/stats_test.rs (`at_most_four_robots_are_read_at_once_and_a_rejected_token_stops_the_reads`)

### Requirement: Load on Patronus: A failure keeps what was kept

A failure MUST keep what was kept: the numbers are of it and say they are stale.

#### Scenario: Patronus fails after a read
- **GIVEN** numbers kept of an earlier read
- **WHEN** Patronus fails
- **THEN** the numbers are of what was kept and say they are stale
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_failure_keeps_what_was_kept_and_says_it_is_stale`)

### Requirement: Load on Patronus: Without a token nothing is asked

Without a token nothing MUST be asked.

#### Scenario: No token
- **GIVEN** no Patronus token
- **WHEN** the statistics are read
- **THEN** Patronus is asked nothing, and with nothing kept there are no numbers
  [@test] crates/remote-facts/src/robot_stats_test.rs (`nothing_is_asked_without_a_token_and_nothing_kept_has_no_numbers`)

### Requirement: Load on Patronus: The kept weeks live in the object cache

The kept weeks, runs and salt MUST live in the `patronus_history` store of the object cache, which the app and `brd`
share (`$GIT_COMMON_DIR/branch-deck/cache`).

#### Scenario: The app and brd read the same kept weeks
- **WHEN** the app or `brd` keeps weeks, runs or the salt
- **THEN** they live in the `patronus_history` store of the object cache, `$GIT_COMMON_DIR/branch-deck/cache`

### Requirement: `brd robot-stats`: The statistics are one object

`brd robot-stats` MUST print the statistics as one object (`mine`, `everyone`, `first_try`, `value`, `time`,
`failures`, `start_hours`, `window` with its `period`, `coverage`, `stale`, `fetched_at`), write no ref, config, index
or working tree, and match `brd schema` (`robot_stats`).

#### Scenario: A run counts the user's robots against everyone else's
- **WHEN** `brd robot-stats` runs
- **THEN** it prints the statistics as one object that matches `brd schema` (`robot_stats`)
- **AND** it writes no ref, config, index or working tree
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)

### Requirement: `brd robot-stats`: A second run asks nothing and --refresh lists again

A second run of `brd robot-stats` MUST ask Patronus nothing; `--refresh` MUST list the weeks that may still change.

#### Scenario: A second run and a Refresh
- **GIVEN** a run of `brd robot-stats` kept the weeks and the runs
- **WHEN** it runs again
- **THEN** it asks Patronus nothing
- **AND** with `--refresh` it lists the weeks that may still change
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)

### Requirement: `brd robot-stats`: --period chooses the period

`--period 14d|30d|10w|6m` MUST choose the period of `brd robot-stats` (30 days by default).

#### Scenario: A chosen period
- **WHEN** `brd robot-stats` runs with `--period`
- **THEN** it counts the robots of that period, 30 days without it
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_period_is_the_days_that_count`, `the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)

### Requirement: `brd robot-stats`: A failure prints the kept numbers or says why there are none

With kept numbers and a failing Patronus `brd robot-stats` MUST exit 0 with `stale: true` and a `warning`; without
anything kept it MUST be `patronus_error`, and without the token `not_logged_in`.

#### Scenario: Kept numbers and a failing Patronus
- **GIVEN** numbers kept of an earlier run
- **WHEN** Patronus fails
- **THEN** `brd robot-stats` exits 0 with `stale: true` and a `warning`
  [@test] crates/bd-remote/src/robot_stats_test.rs (`a_failure_after_a_read_prints_the_kept_numbers_with_a_warning`)

#### Scenario: Nothing kept, or no token
- **WHEN** Patronus fails with nothing kept, or there is no token
- **THEN** the error is `patronus_error`, or `not_logged_in` without the token
  [@test] crates/bd-remote/src/robot_stats_test.rs (`the_statistics_say_why_there_are_none`)

### Requirement: `brd robot-stats`: It starts only the git that finds the repository

`brd robot-stats` MUST start only the git that finds the Space repository and the branch of origin the baseline tracks.

#### Scenario: The processes of a run
- **WHEN** `brd robot-stats` runs
- **THEN** the only git it starts finds the Space repository and the branch of origin the baseline tracks
  [@test] crates/bd-cli/src/process_budget_test.rs (`a_robot_stats`)

### Requirement: The app: get_robot_stats answers from the same core

`get_robot_stats` MUST answer the same `RobotStats` of the period it is asked for from the same core, and send the
progress and the numbers of the lists (`RobotStatsEvent::Partial`) through its channel before the checks are read.

#### Scenario: A read of the app
- **WHEN** the app asks `get_robot_stats` for a period
- **THEN** it answers the same `RobotStats` as `brd robot-stats` from the same core
- **AND** the progress and the numbers of the lists come through its channel before the checks are read

#### Scenario: The numbers of the lists before the runs
- **WHEN** the core reads the statistics
- **THEN** it sends the progress, then the numbers of the lists (`RobotStatsEvent::Partial`), before the runs are read
  [@test] crates/remote-facts/src/robot_stats_test.rs (`the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl`)

### Requirement: The app: Two reads of one repository at once read once

Two reads of one repository at once by `get_robot_stats` MUST read once, and one from the cache MUST wait for none.

#### Scenario: Two reads at once and one from the cache
- **WHEN** two reads of one repository come at once
- **THEN** they read once
  [@test] crates/remote-facts/src/robot_stats_test.rs (`two_reads_at_once_ask_once`)
- **AND** a read from the cache waits for none
  [@test] crates/remote-facts/src/robot_stats_test.rs (`a_read_with_nothing_to_ask_does_not_wait_for_a_running_read`)

### Requirement: The app: The open of the object cache runs off the async workers

The open of the object cache of `get_robot_stats` MUST run off the async workers.

#### Scenario: The blocking reads of the command
- **WHEN** `get_robot_stats` opens the object cache
- **THEN** the open runs off the async workers
  [@test] crates/app-api/src/command_scan_test.rs (`an_async_function_makes_the_blocking_reads_off_the_async_workers`)

### Requirement: The app: The read waits for no sync

`get_robot_stats` MUST wait for no sync: shown during one, the view reads the statistics at once.

#### Scenario: The Landing view shown during a held sync
- **GIVEN** a sync is held
- **WHEN** the Landing view is shown
- **THEN** it reads the statistics at once, and the sync ends once
  [@test] tests/e2e/landing/statistics.spec.ts (`should read the statistics of the Landing view shown during a held sync at once, the sync ending once`)

### Requirement: The app: The numbers of the lists stay while another period starts

The numbers of the lists of a read MUST stay while the read of another period starts, until it has its own or ends.

#### Scenario: Another period is chosen during a read
- **GIVEN** a read has the numbers of its lists
- **WHEN** the read of another period starts
- **THEN** those numbers stay until the new read has its own or ends
  [@test] src/stores/robot-stats.test.ts (`keeps the numbers of the lists of a read while the read of another period starts, until it has its own`)

### Requirement: The app: The E2E server answers from the real core

The E2E server MUST answer `get_robot_stats` from the real core at the page's clock, in UTC, without a cache.

#### Scenario: A robot of the fake history
- **WHEN** the E2E server answers the statistics
- **THEN** a history robot of its fixtures reads back through the parser of the statistics of the real core
  [@test] crates/test-server/src/remote_fixtures_test.rs (`a_history_robot_reads_back_through_the_parser_of_the_statistics`)

#### Scenario: The clock, the zone and the cache of the E2E server
- **WHEN** the E2E server answers the statistics
- **THEN** it counts at the page's clock, in UTC, without a cache

### Requirement: The app: The Landing view shows the statistics

The Landing view of the main window MUST show the statistics, in the period of its header
([Main Views](../main-views/spec.md)).

#### Scenario: How the Safe Pushes went
- **WHEN** the Landing view is shown
- **THEN** it shows the merges against everyone else, the time lost, the failure causes and the hours
  [@test] tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

#### Scenario: The chosen period
- **WHEN** the user chooses the period in the header of the Landing view
- **THEN** the view counts the chosen period: its robots and its weeks
- **AND** the choice is kept
  [@test] tests/e2e/landing/statistics.spec.ts (`should count the chosen period: its robots, its weeks, and keep the choice`)

## Testing / Local Run

- `cargo nextest run -E 'test(/robot_stats|calendar|stats_test/)'`.
- `brd robot-stats -C <a clone on Space>`: the first run reads the 5 or 6 weeks of 30 days (a few megabytes each in a
  large repository) and up to 120 robots; later runs read the current week, `--period 6m` once the weeks not kept.
- Measured on the IntelliJ monorepo (2026-10-23): a week lists 3,000 to 3,700 robots in 0.4 to 6 s; 30 days are about
  10,200 robots, 6 months about 49,900, and Patronus answers the weeks older than 70 days; the 120 runs of a read take
  about 8 s.

## Open Questions / Risks

- A user with hundreds of robots a week reads their runs over several opens: `coverage.runs_missing` says how many
  wait.
- Everyone else's list of a week of a large repository is a few megabytes; Patronus has no smaller answer of it.

## References

- [Landing Work into Master](../../../../docs/design/landing.md), [Patronus](../../branch/patronus/spec.md),
  [Safe Push Failure Prompt](../../branch/safe-push-failure-prompt/spec.md), [Agent CLI](../../cli/agent-cli/spec.md).
