---
name: Safe Push Statistics
description: Requirements for the statistics of the Safe Pushes of a repository: the user's robots into the target of the last 70 days against everyone else's as the median person, the time lost in failures, the failure causes and flaky checks of the last 14 days, and the hours robots ran fastest; one Rust core for the app and `bd robot-stats`, Patronus read week by week and kept in the object cache.
style: plain-1
targets:
  - ../../crates/domain-types/src/robot_stats.rs
  - ../../crates/domain-types/src/calendar.rs
  - ../../crates/patronus/src/stats.rs
  - ../../crates/patronus/src/client.rs
  - ../../crates/patronus/src/fake_patronus.rs
  - ../../crates/object-cache/src/patronus_history.rs
  - ../../crates/remote-facts/src/robot_stats.rs
  - ../../crates/remote-facts/src/robot_stats_aggregate.rs
  - ../../crates/bd-remote/src/robot_stats_command.rs
  - ../../src-tauri/src/commands/patronus.rs
  - ../../crates/test-server/src/tauri_command_bridge.rs
  - ../../crates/test-server/src/remote_fixtures.rs
  - ../../src/components/landing/LandingView.tsx
  - ../../src/stores/robot-stats.ts
---

# Safe Push Statistics

Status: Implemented
Date: 2026-10-22

## Summary

The work of a branch is done when it is in master ([Landing](../../docs/design/landing.md)), and a Safe Push is how it
gets there. The statistics answer what that costs: how many robots one merge takes, how long a merged robot runs, how
much time failed robots and the waits after them took, why the robots failed, which checks pass only on a retry, and
at which hours robots run fastest. The user's numbers stand beside everyone else's in the repository, as the median
person, so a slow week says whether it was the branch or the queue.

One Rust core reads them for the app and `bd robot-stats` (`remote_facts::robot_stats`), the rules that count are pure
(`remote_facts::robot_stats_aggregate`), and what Patronus answered is kept in the object cache, so a second read asks
it for little.

## Goals

- Count what lands into master and in how many robots, alike for the user and for everyone else.
- Show nothing of another person: no name, no branch, no number of one person.
- Read Patronus little: a week is listed once it settled, a finished robot's checks once.

## Non-goals

- No chain of restarts is guessed for "robots per merge": a Safe Push of the IDE runs on a temporary ref, and its retry
  is a new robot of another source branch.
- No split of a robot's time into the queue of each check: the wait for an agent is inside a check's run, and the user
  cannot change the queue.
- No statistics of another repository, nor of a release branch.

## Requirements

### What counts

- A robot counts when a person started it (no application), it is no cherry-pick, no dry run and no emergency robot,
  it started in the last 70 days and it finished. Dry runs and emergency robots of the user are counted apart.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`dry_runs_emergency_robots_applications_cherry_picks_and_old_robots_do_not_count`)
- Robots per merge are the finished robots over the merged ones; the time to merge runs from the start to the finish
  of a merged robot (median, 90th percentile by the nearest rank).
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`robots_per_merge_are_the_finished_robots_over_the_merged_ones`)
- The ten weeks of a delivery are the local weeks from Monday; a robot counts in the week it finished.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`weeks_count_by_the_local_monday_a_robot_finished_in`)
  [@test] ../../crates/domain-types/src/calendar_test.rs (`a_week_starts_on_the_local_monday`)
- Everyone else is the median person: the median of each person's robots per merge and time to merge, of the people
  with 3 merges; none while fewer than 5 people have.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`everyone_else_is_the_median_person_of_those_with_three_merges`, `everyone_else_needs_five_people`)
- A merge of the user is first try when it is no restart and no failed or canceled robot of its branch finished in the
  3 days before; an estimate, of the merges whose branch is known.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`first_try_is_a_merge_without_a_restart_or_a_failure_of_its_branch_in_three_days`)
- The time of the user's robots goes into merged, failed and canceled robots, and between a failed or canceled robot
  and the next robot of the same branch started within 3 days.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`time_goes_into_merged_failed_and_canceled_robots_and_between_a_failure_and_the_next_robot`)
- What a merged robot landed: the commits Patronus delivered, per merge of the last 14 days, and the merges of a whole
  stack (`{prefix}/stack/<root>`).
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`commits_and_whole_stack_merges_are_what_the_merged_robots_landed`)
- The failure causes are of the user's failed robots of the last 14 days: by kind, as the failure prompt decides it
  (`spec/branch/safe-push-failure-prompt.spec.md`), and the 10 checks that failed the most robots, by count.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`failure_causes_count_the_kinds_and_checks_of_the_failed_robots_of_the_runs_window`)
  [@test] ../../crates/patronus/src/stats_test.rs (`a_failed_run_reads_the_checks_and_problems_a_merged_one_the_checks_and_changes`)
- A flaky check passed on attempt 2 or later; its share of its runs is given from 5 runs.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`a_flaky_check_passed_on_a_later_attempt_and_has_a_share_from_five_runs`)
- The start hours are the 24 local hours of the workdays: the median time to merge of everyone else's merged robots
  that started then, shown with 5 people, and the user's, shown with 3 of them, and a level 0 (fastest) to 4 of the
  hours shown; the local zone follows its daylight saving changes.
  [@test] ../../crates/remote-facts/src/robot_stats_aggregate_test.rs (`the_start_hours_are_local_workday_hours_shown_with_five_people_and_three_of_mine`)
  [@test] ../../crates/domain-types/src/calendar_test.rs (`weekday_and_hour_are_local`, `the_hour_follows_the_daylight_saving_change`)

### Privacy

- Of another person's robot only what the rules count is kept: no name, no email, no branch, the owner as the first 8
  bytes of a SHA-256 with a random salt of the cache. The output names no person, only how many people the median
  covers.
  [@test] ../../crates/patronus/src/stats_test.rs (`the_history_lists_everyone_into_the_target_and_keeps_no_name_and_no_branch_of_others`, `the_owner_key_depends_on_the_salt`)

### Load on Patronus

- The robots are listed without an owner, into the target, by UTC week (`start`, `end`): 11 lists cover the 70 days.
  A week is listed once it settled (it ended 2 days ago and each of its robots finished); another week again once its
  list is 5 minutes old, or on Refresh.
  [@test] ../../crates/remote-facts/src/robot_stats_test.rs (`the_weeks_are_listed_once_then_only_those_that_have_not_settled_after_the_ttl`)
- The checks of a finished robot of the user are read once, with its problems when it failed and its changes when it
  merged; only of the merged and failed robots of the last 14 days, at most 4 robots at once and 120 per read, the
  newest first: the next read asks for the rest.
  [@test] ../../crates/remote-facts/src/robot_stats_test.rs (`a_read_asks_for_at_most_the_newest_runs_and_the_next_one_for_the_rest`)
  [@test] ../../crates/patronus/src/stats_test.rs (`at_most_four_robots_are_read_at_once_and_a_rejected_token_stops_the_reads`)
- A failure keeps what was kept: the numbers are of it and say they are stale; without a token nothing is asked.
  [@test] ../../crates/remote-facts/src/robot_stats_test.rs (`a_failure_keeps_what_was_kept_and_says_it_is_stale`, `nothing_is_asked_without_a_token_and_nothing_kept_has_no_numbers`)
- The kept weeks, runs and salt live in the `patronus_history` store of the object cache, which the app and `bd`
  share (`$GIT_COMMON_DIR/branch-deck/cache`).

### `bd robot-stats`

- `bd robot-stats` must print the statistics as one object (`mine`, `everyone`, `first_try`, `value`, `time`,
  `failures`, `start_hours`, `window`, `coverage`, `stale`, `fetched_at`), write no ref, config, index or working tree,
  and match `bd schema` (`robot_stats`); a second run asks Patronus nothing, `--refresh` lists the weeks that may still
  change.
  [@test] ../../crates/bd-remote/src/robot_stats_test.rs (`the_statistics_count_the_users_robots_against_everyone_elses_and_write_nothing`)
- With kept numbers and a failing Patronus it must exit 0 with `stale: true` and a `warning`; without anything kept it
  must be `patronus_error`, and without the token `not_logged_in`.
  [@test] ../../crates/bd-remote/src/robot_stats_test.rs (`a_failure_after_a_read_prints_the_kept_numbers_with_a_warning`, `the_statistics_say_why_there_are_none`)
- It must start only the git that finds the Space repository and the branch of origin the baseline tracks.
  [@test] ../../crates/bd-cli/src/process_budget_test.rs (`a_robot_stats`)

### The app

- `get_robot_stats` must answer the same `RobotStats` from the same core, the progress and the numbers of the lists
  (`RobotStatsEvent::Partial`) through its channel before the checks are read; two reads of one repository at once
  read once. The E2E server answers it from the real core at the page's clock, in UTC.
  [@test] ../../crates/test-server/src/remote_fixtures_test.rs (`a_history_robot_reads_back_through_the_parser_of_the_statistics`)
- The Landing view of the main window shows them ([Main Views](main-views.spec.md)).
  [@test] ../../tests/e2e/landing/statistics.spec.ts (`should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours`)

## Testing / Local Run

- `cargo nextest run -E 'test(/robot_stats|calendar|stats_test/)'`.
- `bd robot-stats -C <a clone on Space>`: the first run reads 11 weeks (a few megabytes each in a large repository) and
  up to 120 robots; later runs read the current week.

## Open Questions / Risks

- A user with hundreds of robots a week reads their runs over several opens: `coverage.runs_missing` says how many
  wait.
- Everyone else's list of a week of a large repository is a few megabytes; Patronus has no smaller answer of it.

## References

- [Landing Work into Master](../../docs/design/landing.md), [Patronus](../branch/patronus.spec.md),
  [Safe Push Failure Prompt](../branch/safe-push-failure-prompt.spec.md), [Agent CLI](../cli/agent-cli.spec.md).
