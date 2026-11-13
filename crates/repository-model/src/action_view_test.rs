use super::landed_with_children;
use domain_types::actions::NothingBeyond;
use domain_types::fixtures::{commit, robot, robot_of, whole_stack_robot_of};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::{BranchPatronus, LandedByView, PatronusView, RobotOf};
use domain_types::session::RobotsView;
use pretty_assertions::assert_eq;

fn merged(id: &str, finish_time: u32) -> PatronusRobot {
  PatronusRobot {
    finish_time: Some(finish_time),
    ..robot(id, None, PatronusRobotStatus::Success, 1)
  }
}

/// The Patronus view where `by` landed each branch, with `after` the work left of it
fn view(landed: &[(&str, RobotOf, &[&str])]) -> PatronusView {
  let branches = landed
    .iter()
    .map(|(branch, by, after)| {
      let facts = BranchPatronus {
        landed_by: Some(LandedByView {
          by: by.clone(),
          own: by.branch == *branch && !by.whole_stack,
          landed: vec![format!("{branch}-1")],
          after: after.iter().map(ToString::to_string).collect(),
        }),
        ..BranchPatronus::default()
      };
      ((*branch).to_string(), facts)
    })
    .collect();
  PatronusView {
    branches,
    ..PatronusView::default()
  }
}

fn robots(robots: &[PatronusRobot]) -> RobotsView {
  RobotsView {
    robots: robots.iter().map(|robot| (robot.id.clone(), robot.clone())).collect(),
    ..RobotsView::default()
  }
}

fn children(names: &[&str]) -> Vec<String> {
  names.iter().map(ToString::to_string).collect()
}

/// Push with Children has nothing left once merged Safe Pushes landed the branch and every branch stacked on it whole:
/// each robot once in the order they merged, "own" for the robot of its whole stack; a branch with work left after what
/// landed, a robot that didn't merge, a branch without children: none
#[test]
fn push_with_children_is_landed_once_merged_safe_pushes_landed_the_branch_and_every_branch_stacked_on_it() {
  let (session, login) = (merged("session-run", 300), merged("login-run", 200));
  let all = robots(&[session, login.clone()]);
  let landed = view(&[
    ("auth", robot_of("login", "login-run"), &[]),
    ("login", robot_of("login", "login-run"), &[]),
    ("session", robot_of("session", "session-run"), &[]),
  ]);
  assert_eq!(
    landed_with_children(&landed, &all, |_| None, "auth", &children(&["login", "session"])),
    Some(NothingBeyond::Landed {
      by: vec![robot_of("login", "login-run"), robot_of("session", "session-run")],
      own: false,
      target: "master".to_string(),
    })
  );
  assert_eq!(landed_with_children(&landed, &all, |_| None, "login", &[]), None, "no children");
  let left = view(&[("auth", robot_of("login", "login-run"), &[]), ("login", robot_of("login", "login-run"), &["fix"])]);
  assert_eq!(landed_with_children(&left, &all, |_| None, "auth", &children(&["login"])), None, "work left after it");
  let running = robots(&[PatronusRobot {
    status: PatronusRobotStatus::Running,
    ..login
  }]);
  assert_eq!(landed_with_children(&landed, &running, |_| None, "auth", &children(&["login"])), None, "not merged");
  // the view is of another table: login has a commit since, which nothing landed
  let grown = [commit("login-2"), commit("login-1")];
  let commits_of = |name: &str| (name == "login").then_some(&grown[..]);
  assert_eq!(landed_with_children(&landed, &all, commits_of, "auth", &children(&["login"])), None, "a commit since");

  let whole = whole_stack_robot_of("auth", "stack-run");
  let by_stack = view(&[("auth", whole.clone(), &[]), ("login", whole.clone(), &[])]);
  assert_eq!(
    landed_with_children(&by_stack, &robots(&[merged("stack-run", 400)]), |_| None, "auth", &children(&["login"])),
    Some(NothingBeyond::Landed {
      by: vec![whole],
      own: true,
      target: "master".to_string(),
    })
  );
}
