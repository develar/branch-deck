//! What a robot of the user does to a push and a start: the push that fails it, the modes a profile allows, the words
//! of a mode and of what a robot lands. Pure functions over the robots of the user; whether a robot still runs and
//! pushes is `domain_policy::holds`, which robot belongs to a branch `domain_policy::robot_match`, what they land and
//! landed the repository view (`repository_model::patronus_view`).

use domain_policy::holds::{is_active, is_pushing};
use domain_policy::robot_match::is_started_on;
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot};
use domain_types::repository_view::Carried;

/// A push into the source branch makes its running Safe Push fail at the end ("branch was modified"); a dry run is not
/// affected
pub fn is_push_into_robot_risky(robot: &PatronusRobot, subject: &Subject) -> bool {
  is_active(robot) && is_pushing(robot) && is_started_on(robot, subject)
}

/// "auth", "auth and login", "auth, login and oauth"
pub fn join_names(names: &[String]) -> String {
  match names {
    [] => String::new(),
    [only] => only.clone(),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  }
}

/// How many commit ids a refusal lists
const LISTED_COMMITS: usize = 3;

/// What a robot lands of `subject`, as a refusal says it: "login" when only the stack tells, else "2 commits of login"
pub fn carried_words(subject: &str, carried: Option<&Carried>) -> String {
  match carried {
    None => subject.to_string(),
    Some(carried) if carried.commits.len() == 1 => format!("1 commit of {subject}"),
    Some(carried) => format!("{} commits of {subject}", carried.commits.len()),
  }
}

/// The ids of the carried commits for `bd`, after the sentence: " (1a2b3c4d, 5e6f7a8b amended since)", the first three;
/// empty when only the stack tells
pub fn carried_ids(carried: Option<&Carried>) -> String {
  let Some(carried) = carried else {
    return String::new();
  };
  let mut listed: Vec<String> = carried
    .commits
    .iter()
    .take(LISTED_COMMITS)
    .map(|id| {
      let short = &id[..id.len().min(8)];
      if carried.amended.contains(id) {
        format!("{short} amended since")
      } else {
        short.to_string()
      }
    })
    .collect();
  if carried.commits.len() > LISTED_COMMITS {
    listed.push("…".to_string());
  }
  format!(" ({})", listed.join(", "))
}

/// The profile allows the mode; no modes listed = any
pub fn allows_push_mode(profile: &PatronusProfile, mode: PatronusPushMode) -> bool {
  profile.allowed_push_modes.is_empty() || profile.allowed_push_modes.contains(&mode)
}

/// "Safe Push", "Dry Run", as the app names a mode
pub const fn push_mode_name(mode: PatronusPushMode) -> &'static str {
  match mode {
    PatronusPushMode::DryRun => "Dry Run",
    PatronusPushMode::Rebase => "Safe Push (rebase)",
    PatronusPushMode::RebaseAutosquash => "Safe Push",
    PatronusPushMode::SquashAll => "Safe Push (squash)",
    PatronusPushMode::Merge => "Safe Merge",
    PatronusPushMode::Unknown => "Robot",
  }
}
