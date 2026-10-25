use crate::refresh::{
  ActiveRobots, DetailsRead, FactTiming, ListWindow, ProfilesRead, READINESS_PARALLEL, ReadState, ReadinessAsks, ReadinessPriority, ReadinessRead, RefreshReason, ServiceState,
  branch_rules_due, profiles_due, pull_requests_due, pull_requests_follows_up, pull_requests_push_recheck_at, readiness_asks, readiness_due, readiness_priority,
  review_requests_due, review_requests_follows_up, review_requests_poll_delay, robot_details_due, robot_failing_due, robot_problems_due, robots_list_due, robots_list_follows_up,
  robots_list_window, robots_poll_at, robots_poll_due,
};
use domain_types::patronus::PatronusRobotStatus;
use domain_types::pull_request::{PullRequestAvailability, TokenSource};
use pretty_assertions::assert_eq;
use std::time::Duration;

use RefreshReason::{Demand, Focus, Manual, Poll, Push, Sync};

/// The clock of every table: an hour after the start of the session
const NOW: Duration = Duration::from_secs(3600);

const fn secs(seconds: u64) -> Duration {
  Duration::from_secs(seconds)
}

/// The last request started `started` seconds ago and its answer came `answered` seconds ago; `None` = never
fn ago(started: Option<u64>, answered: Option<u64>) -> FactTiming {
  FactTiming {
    started_at: started.map(|seconds| NOW.checked_sub(secs(seconds)).unwrap()),
    answered_at: answered.map(|seconds| NOW.checked_sub(secs(seconds)).unwrap()),
  }
}

const ON_SERVICE: ServiceState = ServiceState {
  off_service: false,
  retry_at: None,
};
const OFF_SERVICE: ServiceState = ServiceState {
  off_service: true,
  retry_at: None,
};

/// A rate limit that lets the background ask again in `seconds`
fn rate_limited_for(seconds: u64) -> ServiceState {
  ServiceState {
    off_service: false,
    retry_at: Some(NOW + secs(seconds)),
  }
}

/// `(reason, seconds since the last request started, seconds since its last answer, due)`
type Row = (RefreshReason, Option<u64>, Option<u64>, bool);

fn check_rows(rows: &[Row], due: impl Fn(RefreshReason, FactTiming) -> bool) {
  for &(reason, started, answered, expected) in rows {
    assert_eq!(
      due(reason, ago(started, answered)),
      expected,
      "{reason:?}, started {started:?} s ago, answered {answered:?} s ago"
    );
  }
}

#[test]
fn the_robots_are_listed_after_a_sync_every_five_minutes_never_after_a_push_and_at_once_for_a_start() {
  let rows: &[Row] = &[
    (Manual, Some(0), Some(0), true),
    (Demand, Some(0), Some(0), true),
    (Sync, None, None, true),
    (Sync, Some(299), Some(1), false),
    (Sync, Some(300), Some(299), true),
    (Push, None, None, false),
    (Poll, None, None, false),
  ];
  check_rows(rows, |reason, timing| robots_list_due(reason, NOW, timing, ON_SERVICE));
  assert!(robots_list_follows_up(Manual) && robots_list_follows_up(Demand));
  assert!(!robots_list_follows_up(Sync) && !robots_list_follows_up(Focus));
}

/// The same rule for the three lists: the robots, the pull requests, the review requests
#[test]
fn a_focus_lists_what_is_older_than_two_minutes_at_most_a_minute_after_the_last_request() {
  let rows: &[Row] = &[
    (Focus, None, None, true),
    (Focus, Some(59), Some(200), false),
    (Focus, Some(60), Some(119), false),
    (Focus, Some(60), Some(120), true),
  ];
  check_rows(rows, |reason, timing| robots_list_due(reason, NOW, timing, ON_SERVICE));
  check_rows(rows, |reason, timing| pull_requests_due(reason, NOW, timing, ON_SERVICE));
  check_rows(rows, |reason, timing| review_requests_due(reason, NOW, timing, ON_SERVICE, true));
}

#[test]
fn a_repository_not_on_space_is_listed_again_only_by_the_refresh_button() {
  for reason in [Sync, Focus, Push, Poll, Demand] {
    assert!(!robots_list_due(reason, NOW, FactTiming::default(), OFF_SERVICE), "{reason:?}");
  }
  assert!(robots_list_due(Manual, NOW, ago(Some(0), Some(0)), OFF_SERVICE));
}

#[test]
fn the_first_list_and_the_refresh_button_ask_the_full_window_the_others_since_two_minutes_before_the_last_list() {
  assert_eq!(robots_list_window(Sync, None), ListWindow::Full);
  assert_eq!(robots_list_window(Manual, Some(NOW)), ListWindow::Full);
  assert_eq!(robots_list_window(Focus, Some(secs(1000))), ListWindow::Since(secs(880)));
  assert_eq!(
    robots_list_window(Sync, Some(secs(100))),
    ListWindow::Since(Duration::ZERO),
    "never before the session's clock began"
  );
}

#[test]
fn the_running_robots_are_polled_every_minute_focused_and_every_five_minutes_unfocused_only_while_one_pushes() {
  let none = ActiveRobots::default();
  let dry_run = ActiveRobots { any: true, pushing: false };
  let pushing = ActiveRobots { any: true, pushing: true };
  assert_eq!(robots_poll_at(NOW, true, pushing), Some(NOW + secs(60)));
  assert_eq!(robots_poll_at(NOW, true, dry_run), Some(NOW + secs(60)));
  assert_eq!(robots_poll_at(NOW, true, none), None);
  assert_eq!(robots_poll_at(NOW, false, pushing), Some(NOW + secs(300)));
  assert_eq!(robots_poll_at(NOW, false, dry_run), None, "a dry run waits for the focus");
  assert_eq!(robots_poll_at(NOW, false, none), None);
}

#[test]
fn a_focus_polls_at_once_unless_the_last_poll_is_less_than_fifty_seconds_old() {
  let running = ActiveRobots { any: true, pushing: false };
  assert!(robots_poll_due(NOW, None, running));
  assert!(!robots_poll_due(NOW, Some(NOW.checked_sub(secs(49)).unwrap()), running));
  assert!(robots_poll_due(NOW, Some(NOW.checked_sub(secs(50)).unwrap()), running));
  assert!(!robots_poll_due(NOW, None, ActiveRobots::default()), "nothing runs: no poll");
}

#[test]
fn a_running_robot_is_asked_whether_it_will_fail_until_known_never_a_canceling_one() {
  assert!(robot_failing_due(PatronusRobotStatus::Running, false));
  assert!(!robot_failing_due(PatronusRobotStatus::Running, true));
  assert!(!robot_failing_due(PatronusRobotStatus::Canceling, false));
  assert!(!robot_failing_due(PatronusRobotStatus::Failure, false));
}

#[test]
fn the_problems_of_a_failed_robot_are_read_once_and_a_failed_read_again_only_after_an_answer() {
  let failure = PatronusRobotStatus::Failure;
  assert!(robot_problems_due(failure, ReadState::Never, false));
  assert!(robot_problems_due(failure, ReadState::Never, true));
  assert!(!robot_problems_due(failure, ReadState::Failed, false), "a change of the table asks no failed read again");
  assert!(robot_problems_due(failure, ReadState::Failed, true));
  assert!(!robot_problems_due(failure, ReadState::Known, true));
  for status in [PatronusRobotStatus::Running, PatronusRobotStatus::Canceled, PatronusRobotStatus::Success] {
    assert!(!robot_problems_due(status, ReadState::Never, true), "{status:?}");
  }
}

#[test]
fn the_details_of_a_demanded_robot_are_read_once_again_on_a_new_status_and_every_61_seconds_while_it_runs() {
  let read = |seconds: u64, status| {
    Some(DetailsRead {
      started_at: NOW.checked_sub(secs(seconds)).unwrap(),
      status,
    })
  };
  let (running, failure) = (PatronusRobotStatus::Running, PatronusRobotStatus::Failure);
  assert!(robot_details_due(NOW, running, None));
  assert!(!robot_details_due(NOW, running, read(60, running)));
  assert!(robot_details_due(NOW, running, read(61, running)));
  assert!(robot_details_due(NOW, failure, read(1, running)), "a poll moved it on");
  assert!(!robot_details_due(NOW, failure, read(3000, failure)), "a finished robot's details never change");
}

#[test]
fn the_profiles_of_a_target_are_kept_a_minute_a_failed_read_not_at_all() {
  let read = |seconds: u64, failed| {
    Some(ProfilesRead {
      started_at: NOW.checked_sub(secs(seconds)).unwrap(),
      failed,
    })
  };
  assert!(profiles_due(Demand, NOW, None));
  assert!(!profiles_due(Demand, NOW, read(59, false)));
  assert!(profiles_due(Demand, NOW, read(60, false)));
  assert!(profiles_due(Demand, NOW, read(0, true)));
  assert!(profiles_due(Manual, NOW, read(0, false)));
}

#[test]
fn the_pull_requests_are_listed_after_a_push_always_after_a_sync_every_minute() {
  let rows: &[Row] = &[
    (Push, Some(0), Some(0), true),
    (Manual, Some(0), Some(0), true),
    (Sync, None, None, true),
    (Sync, Some(59), Some(200), false),
    (Sync, Some(60), Some(1), true),
    (Poll, None, None, false),
    (Demand, None, None, false),
  ];
  check_rows(rows, |reason, timing| pull_requests_due(reason, NOW, timing, ON_SERVICE));
  assert!(pull_requests_follows_up(Push) && pull_requests_follows_up(Manual));
  assert!(!pull_requests_follows_up(Sync) && !pull_requests_follows_up(Focus));
}

#[test]
fn the_pull_requests_off_github_and_before_the_retry_of_a_rate_limit_are_asked_only_by_the_refresh_button() {
  for service in [OFF_SERVICE, rate_limited_for(1)] {
    for reason in [Push, Sync, Focus] {
      assert!(!pull_requests_due(reason, NOW, FactTiming::default(), service), "{reason:?}, {service:?}");
    }
    assert!(pull_requests_due(Manual, NOW, FactTiming::default(), service), "{service:?}");
  }
  assert!(pull_requests_due(Push, NOW, FactTiming::default(), rate_limited_for(0)), "the retry time came");
}

#[test]
fn a_pull_request_that_shows_an_old_head_after_a_push_is_asked_once_more_five_seconds_later() {
  assert_eq!(pull_requests_push_recheck_at(NOW, false), Some(NOW + secs(5)));
  assert_eq!(pull_requests_push_recheck_at(NOW, true), None);
}

#[test]
fn the_branch_rules_of_a_target_are_kept_for_the_session_a_failed_read_asked_again_when_needed() {
  for reason in [Sync, Focus, Push, Manual, Poll, Demand] {
    assert!(branch_rules_due(reason, ReadState::Never), "{reason:?}");
    assert!(!branch_rules_due(reason, ReadState::Known), "{reason:?}");
  }
  assert!(branch_rules_due(Demand, ReadState::Failed));
  assert!(branch_rules_due(Manual, ReadState::Failed));
  for reason in [Sync, Focus, Push, Poll] {
    assert!(!branch_rules_due(reason, ReadState::Failed), "{reason:?}");
  }
}

#[test]
fn the_readiness_is_asked_for_new_inputs_a_recheck_and_five_minutes_after_patronus_failed_never_for_the_same_inputs() {
  let read = |inputs: u64, seconds: u64, unavailable| {
    Some(ReadinessRead {
      inputs,
      answered_at: NOW.checked_sub(secs(seconds)).unwrap(),
      unavailable,
    })
  };
  assert!(readiness_due(NOW, 1, None, false));
  assert!(!readiness_due(NOW, 1, read(1, 3000, false), false));
  assert!(readiness_due(NOW, 2, read(1, 0, false), false));
  assert!(readiness_due(NOW, 1, read(1, 0, false), true));
  assert!(!readiness_due(NOW, 1, read(1, 299, true), false));
  assert!(readiness_due(NOW, 1, read(1, 300, true), false));
}

#[test]
fn a_demanded_readiness_is_asked_at_once_then_the_shown_ones_then_the_hidden_that_wait_for_an_approval() {
  assert_eq!(readiness_priority(true, false, false), Some(ReadinessPriority::Demanded));
  assert_eq!(readiness_priority(true, true, true), Some(ReadinessPriority::Demanded));
  assert_eq!(readiness_priority(false, true, true), Some(ReadinessPriority::Shown));
  assert_eq!(readiness_priority(false, false, true), Some(ReadinessPriority::HiddenAwaiting));
  assert_eq!(readiness_priority(false, false, false), None);

  let due = [
    (ReadinessPriority::HiddenAwaiting, "docs"),
    (ReadinessPriority::Shown, "auth"),
    (ReadinessPriority::Demanded, "login"),
    (ReadinessPriority::Shown, "oauth"),
    (ReadinessPriority::HiddenAwaiting, "api"),
    (ReadinessPriority::Demanded, "ui"),
  ];
  assert_eq!(
    readiness_asks(due),
    ReadinessAsks {
      now: vec!["login", "ui"],
      queued: vec!["auth", "oauth", "docs", "api"],
    },
    "each class in table order"
  );
  assert_eq!(READINESS_PARALLEL, 4);
}

#[test]
fn the_review_requests_are_polled_every_five_minutes_and_listed_after_a_sync_every_minute() {
  let rows: &[Row] = &[
    (Manual, Some(0), Some(0), true),
    (Poll, None, None, true),
    (Poll, Some(269), Some(269), false),
    (Poll, Some(270), Some(270), true),
    (Sync, Some(59), Some(200), false),
    (Sync, Some(60), Some(1), true),
    (Push, None, None, false),
    (Demand, None, None, false),
  ];
  check_rows(rows, |reason, timing| review_requests_due(reason, NOW, timing, ON_SERVICE, true));
  assert!(!review_requests_due(Poll, NOW, FactTiming::default(), ON_SERVICE, false), "offline: the poll asks nothing");
  assert!(review_requests_due(Manual, NOW, FactTiming::default(), ON_SERVICE, false));
  assert!(review_requests_follows_up(Manual) && !review_requests_follows_up(Poll));
  for service in [OFF_SERVICE, rate_limited_for(1)] {
    assert!(!review_requests_due(Poll, NOW, FactTiming::default(), service, true), "{service:?}");
    assert!(review_requests_due(Manual, NOW, FactTiming::default(), service, true), "{service:?}");
  }
}

#[test]
fn the_review_requests_poll_backs_off_after_failures_waits_out_a_rate_limit_and_stops_without_access() {
  let error = PullRequestAvailability::Error { message: "offline".to_string() };
  let rate_limited = PullRequestAvailability::RateLimited { retry_at: 0 };
  let available = PullRequestAvailability::Available {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  };
  assert_eq!(review_requests_poll_delay(&available, 0), Some(secs(300)));
  assert_eq!(review_requests_poll_delay(&error, 1), Some(secs(600)));
  assert_eq!(review_requests_poll_delay(&error, 2), Some(secs(1200)));
  assert_eq!(review_requests_poll_delay(&error, 3), Some(secs(1800)), "doubled up to 30 minutes");
  assert_eq!(review_requests_poll_delay(&error, 40), Some(secs(1800)));
  assert_eq!(review_requests_poll_delay(&rate_limited, 1), Some(secs(900)), "15 minutes at least");
  assert_eq!(review_requests_poll_delay(&rate_limited, 2), Some(secs(1200)));
  let stopped = [
    PullRequestAvailability::NotGitHub,
    PullRequestAvailability::SignedOut,
    PullRequestAvailability::NotAuthenticated { source: TokenSource::Gh },
    PullRequestAvailability::SsoRequired { url: None },
    PullRequestAvailability::NoAccess {
      source: TokenSource::Gh,
      message: "Resource not accessible".to_string(),
    },
  ];
  for availability in stopped {
    assert_eq!(review_requests_poll_delay(&availability, 1), None, "{availability:?}");
  }
}
