use crate::review_request_chains::{request_rows, stack_layers};
use domain_types::fixtures::review_request;
use domain_types::review_request::{RequestRow, ReviewRequest};
use pretty_assertions::assert_eq;

/// The request `number` of the head branch `head` into `base`, waiting since `number`
fn request(number: u32, head: &str, base: &str) -> ReviewRequest {
  let mut request = review_request(number, number);
  head.clone_into(&mut request.head_ref_name);
  base.clone_into(&mut request.base_ref_name);
  request.base_is_default = base == "master";
  request
}

fn single(number: u32) -> RequestRow {
  RequestRow::Single { number }
}

fn stack(layers: &[u32]) -> RequestRow {
  RequestRow::Stack { layers: layers.to_vec() }
}

#[test]
fn a_chain_of_requests_is_one_stack_bottom_first_at_the_place_of_its_longest_waiting_layer() {
  let cases: Vec<(&str, Vec<ReviewRequest>, Vec<RequestRow>)> = vec![
    (
      "requests into master are rows of their own",
      vec![request(1, "a", "master"), request(2, "b", "master")],
      vec![single(1), single(2)],
    ),
    (
      "a chain into the head branches of each other is one stack, the bottom first",
      vec![request(10, "parts/33", "master"), request(11, "parts/34", "parts/33"), request(12, "parts/35", "parts/34")],
      vec![stack(&[10, 11, 12])],
    ),
    (
      "an upper layer that waits longer puts the stack at its place, the layers still bottom first",
      vec![
        request(1, "parts/35", "parts/34"),
        request(2, "other", "master"),
        request(3, "parts/33", "master"),
        request(4, "parts/34", "parts/33"),
      ],
      vec![stack(&[3, 4, 1]), single(2)],
    ),
    (
      "a bottom not requested from the user: the stack starts at the lowest requested layer",
      vec![request(11, "parts/34", "parts/33"), request(12, "parts/35", "parts/34")],
      vec![stack(&[11, 12])],
    ),
    (
      "a branch two requests go into links neither of them",
      vec![request(1, "base", "master"), request(2, "left", "base"), request(3, "right", "base")],
      vec![single(1), single(2), single(3)],
    ),
    (
      "a fork below the top keeps the chain below it",
      vec![request(1, "a", "master"), request(2, "b", "a"), request(3, "left", "b"), request(4, "right", "b")],
      vec![stack(&[1, 2]), single(3), single(4)],
    ),
    (
      "a request from a fork is never a layer below another",
      vec![
        {
          let mut fork = request(1, "feature", "master");
          fork.is_cross_repository = true;
          fork
        },
        request(2, "upper", "feature"),
      ],
      vec![single(1), single(2)],
    ),
    (
      "a request into the default branch is the bottom, even when another request's head has its name",
      vec![request(1, "master", "release"), request(2, "fix", "master")],
      vec![single(1), single(2)],
    ),
    (
      "names that go round in a cycle are one stack, each once",
      vec![request(1, "a", "b"), request(2, "b", "a")],
      vec![stack(&[2, 1])],
    ),
  ];
  for (name, requests, rows) in cases {
    assert_eq!(request_rows(&requests), rows, "{name}");
  }
}

/// A case of the stack of a review window: the pull requests a listing named, those of the last listing, the top and the
/// bottom the window names, the layers it gets
struct StackCase {
  name: &'static str,
  requests: Vec<ReviewRequest>,
  listed: &'static [u32],
  top: u32,
  bottom: u32,
  expected: Vec<u32>,
}

#[test]
fn the_stack_of_a_review_window_is_the_chain_under_its_top_as_the_known_pull_requests_link_it() {
  let mut from_fork = request(5, "parts/33", "master");
  from_fork.is_cross_repository = true;
  let cases = vec![
    StackCase {
      name: "the chain from the top down to the bottom, the bottom first",
      requests: vec![request(10, "parts/33", "master"), request(11, "parts/34", "parts/33"), request(12, "parts/35", "parts/34")],
      listed: &[10, 11, 12],
      top: 12,
      bottom: 10,
      expected: vec![10, 11, 12],
    },
    StackCase {
      name: "a layer the user reviewed left the listing and is still a layer",
      requests: vec![request(10, "parts/33", "master"), request(11, "parts/34", "parts/33"), request(12, "parts/35", "parts/34")],
      listed: &[10, 12],
      top: 12,
      bottom: 10,
      expected: vec![10, 11, 12],
    },
    StackCase {
      name: "a merged bottom: the layer above it went into master, the stack starts there",
      requests: vec![request(10, "parts/33", "master"), request(11, "parts/34", "master"), request(12, "parts/35", "parts/34")],
      listed: &[11, 12],
      top: 12,
      bottom: 10,
      expected: vec![11, 12],
    },
    StackCase {
      name: "a layer put between two layers is one",
      requests: vec![
        request(10, "parts/33", "master"),
        request(13, "parts/33b", "parts/33"),
        request(11, "parts/34", "parts/33b"),
      ],
      listed: &[10, 13, 11],
      top: 11,
      bottom: 10,
      expected: vec![10, 13, 11],
    },
    StackCase {
      name: "a pull request below the bottom is not a layer",
      requests: vec![request(9, "parts/32", "master"), request(10, "parts/33", "parts/32"), request(11, "parts/34", "parts/33")],
      listed: &[9, 10, 11],
      top: 11,
      bottom: 10,
      expected: vec![10, 11],
    },
    StackCase {
      name: "of two pull requests of the base branch, the listed one, else the newest",
      requests: vec![request(3, "parts/33", "master"), request(10, "parts/33", "master"), request(11, "parts/34", "parts/33")],
      listed: &[3, 11],
      top: 11,
      bottom: 3,
      expected: vec![3, 11],
    },
    StackCase {
      name: "of two pull requests of the base branch, none listed: the newest",
      requests: vec![request(3, "parts/33", "master"), request(10, "parts/33", "master"), request(11, "parts/34", "parts/33")],
      listed: &[11],
      top: 11,
      bottom: 3,
      expected: vec![10, 11],
    },
    StackCase {
      name: "a pull request from a fork is never a layer below",
      requests: vec![from_fork, request(11, "parts/34", "parts/33")],
      listed: &[11],
      top: 11,
      bottom: 5,
      expected: vec![11],
    },
    StackCase {
      name: "a cycle of names stops where it began",
      requests: vec![request(1, "a", "b"), request(2, "b", "a")],
      listed: &[1, 2],
      top: 1,
      bottom: 3,
      expected: vec![2, 1],
    },
    StackCase {
      name: "a top no listing named has no stack",
      requests: vec![request(1, "a", "master")],
      listed: &[1],
      top: 2,
      bottom: 1,
      expected: vec![],
    },
  ];
  for case in cases {
    let known: Vec<&ReviewRequest> = case.requests.iter().collect();
    assert_eq!(stack_layers(case.top, case.bottom, &known, case.listed), case.expected, "{}", case.name);
  }
}
