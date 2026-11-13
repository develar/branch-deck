use crate::review_readings::{ReadAheadAnswer, ReadAheadState, ReviewReadingsFact};
use crate::review_request_facts::ReviewRequestsFact;
use crate::review_request_facts::review_request_facts_test::{at, available};
use domain_types::fixtures::review_request;
use domain_types::range_reading::{FileRole, RangeReading, RequestReadingState, RoleReading};
use domain_types::review_request::ReviewRequest;
use pretty_assertions::assert_eq;

/// The waiting request `number` at the head `head`
fn at_head(number: u32, head: &str) -> ReviewRequest {
  ReviewRequest {
    head_ref_oid: head.to_string(),
    base_ref_oid: "b".repeat(40),
    ..review_request(number, 100)
  }
}

fn told(request: &ReviewRequest, state: ReadAheadState) -> ReadAheadAnswer {
  ReadAheadAnswer {
    number: request.number,
    head: request.head_ref_oid.clone(),
    base: request.base_ref_oid.clone(),
    state,
  }
}

fn numbers(requests: &[ReviewRequest]) -> Vec<u32> {
  requests.iter().map(|request| request.number).collect()
}

/// openspec/specs/repository/review-requests/spec.md, "Read ahead: A listing reads ahead the requests without their last
/// reading": a request is asked until its last reading at the head the listing names; one being read is not asked
/// again, one that failed or waits for the index is, and a new head asks it again and shows nothing of the old one
#[test]
fn a_listing_asks_the_waiting_requests_without_their_last_reading() {
  let mut requests = ReviewRequestsFact::default();
  let mut readings = ReviewReadingsFact::default();
  let (one, two, three) = (at_head(1, &"1".repeat(40)), at_head(2, &"2".repeat(40)), at_head(3, &"3".repeat(40)));
  requests.answer(at(1), available(vec![one.clone(), two.clone(), three.clone()]));
  assert!(!readings.listed(requests.view()), "nothing is read yet: nothing shows");
  assert_eq!(numbers(&readings.asks(requests.view())), vec![1, 2, 3]);

  let last = RangeReading {
    roles: vec![RoleReading {
      role: FileRole::Code,
      files: 1,
      files_to_read: 1,
      changes: 1,
      to_read: 1,
    }],
    ..RangeReading::default()
  };
  let answers = vec![
    told(&one, ReadAheadState::Checking),
    told(
      &two,
      ReadAheadState::Read {
        reading: last.clone(),
        last: true,
      },
    ),
    told(&three, ReadAheadState::Read { reading: last, last: false }),
  ];
  assert!(readings.answer(answers, requests.view()));
  assert_eq!(
    readings
      .view()
      .requests
      .iter()
      .map(|reading| (reading.number, matches!(reading.state, RequestReadingState::Checking)))
      .collect::<Vec<_>>(),
    vec![(1, true), (2, false), (3, false)]
  );
  assert_eq!(
    numbers(&readings.asks(requests.view())),
    vec![3],
    "#1 is being read, #2 read for good, #3 waits for the index"
  );

  assert!(readings.answer(vec![told(&one, ReadAheadState::Unread)], requests.view()));
  assert_eq!(numbers(&readings.asks(requests.view())), vec![1, 3], "a failure is asked again");

  // #2 got a commit, #3 no longer waits
  let moved = at_head(2, &"4".repeat(40));
  requests.answer(at(2), available(vec![one, moved]));
  assert!(readings.listed(requests.view()));
  assert_eq!(readings.view().requests, Vec::new(), "a reading of the old head shows nothing");
  assert_eq!(numbers(&readings.asks(requests.view())), vec![1, 2]);
  assert!(
    !readings.answer(vec![told(&three, ReadAheadState::Checking)], requests.view()),
    "a request that no longer waits is no reading"
  );
}
