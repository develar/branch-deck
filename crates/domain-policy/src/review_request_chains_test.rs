use crate::review_request_chains::request_rows;
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
