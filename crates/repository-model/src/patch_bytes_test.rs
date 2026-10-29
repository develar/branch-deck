//! The bytes of the patches a window gets of the session of `derive_budget_test::session()`, the 141 rows of the
//! `branch_table_large` template with every fact answered (no git): the first patch, which is the snapshot a window
//! subscribes to, the patch of one robot that fails, the patch of one readiness answer, and nothing for a flush that
//! changes nothing. Each is a budget (B2 and B15 of docs/design/rust-domain-authority.md, the bytes the channel of the
//! app carries as `serde_json`): a part that grows, or a patch that carries more than what changed, fails here before
//! the page counts it in `pnpm perf:table`. The bytes are printed (`--no-capture`).

use crate::derive::derive_budget_test::{at, session};
use crate::facts::{FactResult, RobotsRead};
use crate::readiness_facts::ReadinessAnswer;
use crate::state::View;
use domain_types::patronus::{ApprovedReviewGroup, BranchReviewReadiness, PatronusAvailability, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult, Person, ReviewReadiness};
use domain_types::session::ViewPatch;

/// The first patch of the view of the 141 rows, every part from nothing (measured 509 235 bytes: the actions 143 KB,
/// the branches 111 KB, the landing decisions 77 KB, the next steps 72 KB, the facts 68 KB, the Patronus part 29 KB)
const FIRST_PATCH_BUDGET: usize = 560_000;
/// The patch of one robot that fails: its fact, and the next step, the landing decisions, the actions and the attention
/// of its branch (measured 3 075 bytes)
const FAILED_ROBOT_BUDGET: usize = 3_400;
/// The patch of the readiness one subject is answered: its fact, and the next step, the landing decisions and the
/// actions of its row (measured 2 460 bytes)
const READINESS_ANSWER_BUDGET: usize = 2_700;

/// The bytes of the JSON a window gets of each item, as the channel of the app carries it
pub(super) fn json_bytes<T: serde::Serialize>(items: &[T]) -> usize {
  items.iter().map(|item| serde_json::to_vec(item).expect("the JSON of an item").len()).sum()
}

/// The bytes of each part `patch` carries, the part that grew to find
fn bytes_of_parts(patch: &ViewPatch) -> Vec<(String, usize)> {
  let serde_json::Value::Object(parts) = serde_json::to_value(&patch.changes).expect("the JSON of a patch") else {
    unreachable!("the changes of a patch are an object");
  };
  parts.iter().map(|(part, value)| (part.clone(), json_bytes(&[value]))).collect()
}

/// The bytes of `patch`, printed as `what`, within `budget`
fn assert_bytes(what: &str, patch: Option<ViewPatch>, budget: usize) {
  let patch = patch.unwrap_or_else(|| panic!("{what}: a patch"));
  let bytes = json_bytes(&[&patch]);
  eprintln!("{what}: {bytes} bytes (budget {budget})");
  assert!(bytes <= budget, "{what}: {bytes} bytes, over the budget of {budget}, by part: {:?}", bytes_of_parts(&patch));
}

/// The first patch of the view of 141 rows, the patch of one robot that fails and the patch of one readiness answer
/// stay within their bytes, and a flush that changes nothing sends no patch
#[test]
fn the_patches_of_the_view_of_141_rows_stay_within_their_bytes() {
  let (state, mut facts) = session();
  let mut view = View::new(1);
  assert_bytes("the first patch", view.diff(&state, &facts), FIRST_PATCH_BUDGET);
  assert!(view.diff(&state, &facts).is_none(), "nothing changed, nothing to send");

  // the poll of the session finds one running robot failed
  let robot = facts.robots.view().robots["r0"].clone();
  assert_eq!(robot.status, PatronusRobotStatus::Running);
  let failed = PatronusRobot {
    status: PatronusRobotStatus::Failure,
    ..robot
  };
  let result = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots: vec![failed],
    github_repository: None,
  };
  assert!(facts.apply(at(6), FactResult::Robots { read: RobotsRead::Poll, result }));
  assert_bytes("one robot failed", view.diff(&state, &facts), FAILED_ROBOT_BUDGET);

  // the approvals of one subject that missed them arrive
  let subject = facts.readiness.digest().subjects[0].clone();
  let answer = ReadinessAnswer {
    result: Ok(BranchReviewReadiness {
      branch_name: subject.ask.branch_name.clone(),
      tip: subject.ask.tip.clone(),
      requirement: None,
      readiness: ReviewReadiness::Approved {
        groups: vec![ApprovedReviewGroup {
          name: "IJPL Code Platform".to_owned(),
          approved_by: vec![Person {
            login: Some("reviewer".to_owned()),
            full_name: None,
          }],
        }],
      },
    }),
    subject,
  };
  assert!(facts.apply(at(7), FactResult::Readiness(vec![answer])));
  assert_bytes("one readiness answer", view.diff(&state, &facts), READINESS_ANSWER_BUDGET);
  assert!(view.diff(&state, &facts).is_none(), "nothing changed since, nothing to send");
}
