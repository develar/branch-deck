use crate::landing_needs::name_landing_robots;
use domain_types::landing::{LandingFix, LandingRefusal, StackCommit};
use domain_types::repository_view::RobotOf;
use pretty_assertions::assert_eq;

fn commit(subject: &str, branch: &str) -> StackCommit {
  StackCommit {
    id: format!("{subject}-id"),
    subject: subject.to_string(),
    branch: branch.to_string(),
  }
}

fn missing(declared_in: &str) -> LandingRefusal {
  LandingRefusal::MissingDeclaration {
    branch: "flow".to_string(),
    identifier: "controlUrl".to_string(),
    file: "src/b/Use.kt".to_string(),
    line: 3,
    used_by: commit("Start with controlUrl", "flow"),
    declared_by: commit("Add controlUrl", declared_in),
    fix: LandingFix::Restack {
      branch: "flow".to_string(),
      onto: declared_in.to_string(),
    },
    landing_in: None,
  }
}

fn landing_in(refusal: &LandingRefusal) -> Option<&RobotOf> {
  match refusal {
    LandingRefusal::MissingDeclaration { landing_in, .. } => landing_in.as_ref(),
    LandingRefusal::DanglingReference { .. } => None,
  }
}

/// A missing declaration names the running Safe Push of the branch that adds the name; one an unassigned commit adds
/// names none, nor one of a branch no robot lands
#[test]
fn a_missing_declaration_names_the_safe_push_that_lands_the_branch_adding_the_name() {
  let robot = RobotOf {
    branch: "util".to_string(),
    robot_id: "robot-1".to_string(),
    whole_stack: false,
  };
  let mut refusals = vec![missing("util"), missing(""), missing("other")];
  name_landing_robots(&mut refusals, |branch| (branch == "util" || branch.is_empty()).then(|| robot.clone()));
  assert_eq!(refusals.iter().map(landing_in).collect::<Vec<_>>(), vec![Some(&robot), None, None]);
}
